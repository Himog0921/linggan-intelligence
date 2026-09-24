# P1 实施记录：批次计数与目录基础

> 状态: 代码候选；Rust和隔离PostgreSQL尚未执行
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 的P1增量
> 事实来源: main@c74d72e3、PR #338及本轮实际命令回执
> 冲突时以谁为准: 已批准数据库/HTTP合同、真实测试与对应commit diff

## 首个增量：批次计数（历史记录）

对应架构S08、数据库合同§8.1、开发手册P1及T44中的JOIN乘法反例。修复已有read_runs：先限定Run页，再分别按run_ref统计Work和Target，避免多个作品把同批Target重复计数。SQL独立文件仍由原函数include_str加载，不创建服务/ORM/第二读取系统。

返回字段和read.v1合同不改变；现有LIMIT和domain筛选保留。空Run保留且各数量为0。此项不代表已经支持全库评论检索、300目标翻页、稳定身份去重、有效知识或方法管理。

`comment_study_productization_postgres.rs`使用既有隔离fixture与clean bootstrap，在两篇作品中构造六个目标，断言targetCount=6而不是12、各状态分别计数、空Run不消失、limit与domain过滤成立。该次没有编写migration。Rust和PostgreSQL未运行，T44保持NOT_RUN。

## 2026-09-23 继续：schema-phase和清洗缓存

Mog 已授权继续开发，无需其手动建表。入手重新核对main=c74d72e3、PR head=496d8351；无新增PR评论。没有访问本机数据库；容器的Git clone因DNS不可用失败，改用GitHub connector读取/提交及隔离staging核对，不冒充已有本机worktree。

### 对照手册与实际文件

- 数据库合同§3–4、开发手册§3.1：新增 `database/migrations/0103_comment_study_productization_schema.sql` 候选。恰好三张批准的新表：clean_cache、start_request、model_request；policy/run/target/membership仅增加约定字段。旧方法/指纹/匹配版本不倒填，旧原字段不UPDATE。安装pg_trgm并建立清洗检索索引。
- 架构文件地图和开发手册§4：新增既定 `comment_study_catalog.rs`，先实现 `refresh_clean_cache`。继续调用现有 `comment-clean.v2`，不存offsets、不建第二清洗器、不创建Run或模型调用。
- SQL从领域内每条稳定评论的最新已接纳版本选取，再判断正文/限制；不从UNKNOWN新版本回退旧正文。一次最多200条，最多传回16001 scalar用于判超长；完整原文哈希仍针对完整UTF-8。全库latest投影只保存元信息，不物化全库正文。
- ON CONFLICT DO NOTHING后比对hash、文本、状态和原因；不覆盖竞争写入。写入前再次检查来源限制；缓存存在不代表有研究或外发资格。读取权限JOIN仍是后续目录接口的必做项。
- HTTP合同§2：实现字面子串转义纯函数，保留1/2字中文，转义百分号、下划线和反斜杠。这不是搜索API已上线的声明。
- 新增3个Rust单元测试和4个隔离PG测试：复用清洗器、长文本、短词转义、旧Run字段保留、不可盲重跑DDL、缓存有界/幂等、最新UNKNOWN/受限不回退、并发缓存不重复。原计数用例保留。

### 迁移与执行安全边界

0102只是手册的schema逻辑阶段，**没有注册到local-runtime.sh，也没有在共享库执行**。下一阶段constraints必须补足：同领域方法校验、不可变版本/请求、新Target写约束、稳定身份回填/唯一索引、membership复合外键；views及所有Worker dispatch gate也尚未完成。不可把目前的单列revision FK误当成完整约束，也不可把dispatch_state列存在误当成旧Worker已经遵守暂停。不能单独将0102应用到正在运行旧Worker的库。

原bootstrap和已应用migration保持字节不变。本增量不创建schedule表、不配置自动计划。现有页面/HTTP/Worker尚未调用新catalog函数；只有隔离测试调用。完整目录、跨批次排重、提示词管理和页面上线仍未完成。

### 实际验证与未验证

- 本地临时staging的13项源码形状检查：PASS；范围包括恰好三表、无破坏性DDL/旧行写入、候选未注册、输入有界、先选最新、限制重核、include路径及测试数量。这不是SQL执行或Rust编译。
- `bash -n scripts/test-comment-study-productization-postgres.sh`：PASS。
- 实际尝试执行proof脚本：退出1，`Cargo is unavailable`，尚未创建Docker资源或数据库。Rust/fmt/PG/EXPLAIN均NOT_RUN。
- 固定手册正文和T01–T54台账本轮不改；T05/T07/T45/T50/T51仅增加部分测试候选，不勾PASS；T52的ledger重放证明尚未实现。
- 全仓库治理、运行入口注册、真实模型、浏览器、部署：NOT_RUN。无共享库副作用。

## 2026-09-23 继续：评论目录、字面搜索与历史读取

本轮重新核对 main=c74d72e3、PR head=b967970，继续同一授权和PR；没有新建任务系统或物理表。对应数据库合同§7–8、HTTP合同§1–2、P1以及T05/T06/T37/T38/T39/T45/T46的部分读取场景。源码可检查，不等于本机已运行。

### 已写代码及接缝

- `comment_study_catalog/read.rs`：独立于runRef的评论目录和汇总，默认50、上限100、limit+1判断下一页，短READ ONLY事务；不在GET中清洗、建Run或调用模型。
- `comment_study_catalog/cursor.rs`：私有base64url编码，严格版本／字段／2048字节边界，scopeHash绑定领域、文字、作品、声音、状态、cleaner与排序；页大小可变。时间以绑定参数进行日历校验，游标不是权限凭证。
- `comment_study_catalog/comments.sql`：先取稳定身份的最新接纳版本，再判权限与清洗；关键词为字面ILIKE。历史按关系批量读取，分别选择最后尝试和最新succeeded/no_signal，不按每条评论查询历史。原声只由有界结果页返回，SQL结果不是全库正文数组。
- 作者未知的有效文字可以显示但不研究；作品作者声音默认隐藏，可显式筛选；dropped/anomaly不进入正常评论页。后来的失败不覆盖旧成功head，新的no_signal替换旧成功head；重采版本按作品＋平台评论ID合并。
- 实际增加两个受原Host/Origin和no-store保护的GET：`/api/local/comment-study/comments`、`/api/local/comment-study/catalog-summary`。新的私有`comment_study_api.rs`只做DTO/安全错误映射，既有API文件仅增加模块声明与merge，保留原路由和测试。
- 查询失败返回503而非空数组；旧库缺候选字段/表返回study_schema_unavailable，不自动初始化。清洗未完成时给partial；带文字搜索且未知未清洗材料能否命中时pendingCount=NULL，不虚报搜遍全库。

### 本次新增汇总字段的读取口径

`summary`包含displayableCommentCount、eligibleCommentCount、studiedCommentCount、inProgressCommentCount、needsContextCount、failedCount、creatorVoiceCount、unknownIdentityCount、textNotResearchableCount、sourceRestrictedCount、bodyUnavailableCount。所有数量对应当前筛选而非当前页；不同维度可以重叠，不加总成互斥进度。studied表示存在成功研究head，当前正文改变另用effectiveState提示，不能将它展示为当前有效知识量。文字查询无法安全判断受限／不可读正文是否命中，后两项返回NULL。indexCoverage独立返回已索引/待索引口径，dropped缓存也属于已索引。

### 明确未完成与禁止假象

- `/works`完整分页和标题搜索尚未实现。现有作品选择弹窗仍有100篇限制，本轮没有宣称T43解决。作品标题必须复用Evidence的共享显示标题解析器；不得为快交付另拼一套native/OCR回退或N+1全作品读取。
- `input_changed`暂明确拒绝，不能只用正文不同冒充完整输入变化。P2将统一保留作品/父语境fingerprint；此阶段已知正文不同可以报告changed，其余inputComparison为unknown，不猜same。该限制是分阶段未完成，不是修改已批准最终合同。
- 当前成功head为P1读取投影；P4统一effective_target view、下游召回、Problem支持去重及来源依赖传播仍需完成。不能据此声称整个研究链已排重或受限语境全部解决。
- 评论详情、全部历史批次明细、共享source选择器、父/作品语境资格收束和新版用户评论UI尚未接完。新GET不改变旧写路径；最终按手册同版切换，不能长期保留分叉资格规则。
- 未注册0102、不创建0103/0104、不触发cache tick、模型或计划。不得为了试新目录单独将0102迁到运行中的共享库。

### 验证证据

本轮新增6个单元测试候选（游标2、目录参数2、API错误2），以及5个隔离PG用例候选：124评论完整翻页/字面通配符、声音与索引覆盖、新UNKNOWN/受限不回退、成功head/no_signal/后失败、跨筛选游标与全量汇总。proof脚本保留前两组测试并加入新test target；使用合成材料，无真实调用。

实际完成：13项staging源码形状/范围检查PASS；包含SQL参数1–11与bind数、read-only、有界页、受控路由、无GET写调用、原API只新增4行，以及base64**测试向量**用Python标准库核对。此检查没有执行Rust编解码器或PostgreSQL。修改前API/catalog/proof脚本/本记录均核对原Git blob哈希，避免重构抓取文本时截断。

`bash -n scripts/test-comment-study-productization-postgres.sh`通过；实际执行脚本退出1、Cargo is unavailable，尚未进入Docker。Rust编译/fmt、SQL语法执行、PG证明、EXPLAIN、全仓库治理、浏览器均NOT_RUN；T01–T54不改PASS。本轮不证明百万规模性能，15秒statement_timeout也不代表达到延迟验收目标。

## 2026-09-23 继续：评论详情与完整分页历史

本轮继续同一授权／PR，基线为 ff8f2064，main仍为c74d72e3；未发现新审查意见。对照HTTP§1–2的评论详情／完整历史要求、数据库§7–8和P1；未修改批准手册原文。读取子项先形成可审查代码，不把已写接口声明为前端或真实数据库验收完成。

### 实现与边界

- `comment_study_catalog/detail.rs`负责当前评论检查器、父语境和两类元数据历史；不是新的服务层。当前原声复用`read.rs`的`read_one_projection`和原`comments.sql`，只增加私有精确comment ID参数，不复制评论可显示／可研究资格规则。普通目录绑定NULL，原公开筛选与分页响应不改变。
- 评论详情一次短只读事务中取当前原声及父语境、前50条研究历史和前50条材料版本。历史集合都返回完整totalCount和独立nextCursor，不将前50条当全部。单条评论最新版本UNKNOWN、受限、待索引或无意义时返回状态及metadata，正文为NULL；不回退旧原文填空。
- `parent.sql`先取同作品、确知parent ID的最新已接纳版本，再检查正文和当前restriction；不能先过滤KNOWN而复活旧父评论。只传回最多16001 scalar，由同一clean.v2区分超长／无语义；作品作者的父评论标contextOnly，不变成用户依据。此处修复的是新检查器读取，**原冻结选择器的父语境SQL尚未统一**，不能据此宣称全链路T07/T39通过。
- `history.sql`跨材料版本按稳定评论身份读取全部Target尝试，只返回状态、方法元信息、时间、Signal数量和runRef/targetRef。最近尝试与最近成功分别标注；历史方法缺记录返回legacy_unrecorded，不将活动方法倒填进去。完整输入比较仍为unknown，留给P2冻结fingerprint；成功head标记不冒充当前Problem支持数。
- `versions.sql`返回材料版本元信息而非历史正文；isCurrent按实际观察时间、接收时间和material_ref判定，翻页按created_at/material_ref，两个顺序不混用。材料版本数量不能展示成用户评论数。无论来源是否受限，这两个历史集合均不返回raw/researchText、frame/basis、prompt或模型输出。
- 原私有cursor扩展到两个实际位置类型，复用base64url和边界检查；history/versions按resource、domain、稳定评论键和排序绑定。改变页大小允许，换评论或资源必须拒绝游标。没有增加通用查询DSL、依赖库或数据库表。

### 精确HTTP接缝

均由`comment_study_api.rs`挂在既有Host/Origin/no-store guard下，GET无清洗／Run／模型副作用。

| 路由 | 参数 | 新读取形状 |
|---|---|---|
| `/comments/detail` | domain、workRef、commentExternalId | contract/domainRef/asOf/commentKey/workRef/source/comment/parentContext/studyHistory/materialVersions/indexCoverage |
| `/comments/history` | 同上＋cursor/limit | contract/domainRef/commentKey/items/totalCount/page；Target尝试元数据 |
| `/comments/versions` | 同上＋cursor/limit | 相同分页外壳；原材料版本元数据 |

后两个是手册“完整历史可查”的有界继续读取入口，不增加新的领域对象或写操作。默认50、上限100；page字段与既有合同相同。source含commentKey/sourceRef/sourceState/displayState/cleanState/parentCommentKey；displayState为displayable/index_pending/not_displayable/body_unavailable/restricted。comment为目录同型对象或NULL。parentContext与comment分开，并明确contextOnly。找不到当前域的稳定评论返回安全404/resource_not_found，不暴露其他领域内容。

### 未完成项，不得扩大声明

`/works`服务端分页、显示标题搜索和原作品选择器100篇限制本轮仍未解决。共享Evidence的title/OCR选择拥有独立权威，不能在评论模块复制native/OCR回退SQL，也不使用N+1全作品读取冒充有界实现。detail目前给workRef，尚未返回完整作品上下文和共享显示标题；本轮没有修改Evidence、HTML/CSS/JS、Worker、schema或migration。

完整冻结选择器、语境权限传播、P2启动默认排重／方法／fingerprint和P4有效知识仍需推进。三个新GET不意味着批次手动流程、四个Tab或自动研究已完成。0102继续未注册、未执行，用户无需手工建表。

### 实际验证与治理回执

本轮新增5个单元测试候选（详情参数3、游标2）和5个隔离PG用例候选，覆盖当前评论／父语境、最新UNKNOWN和受限、无效文本／未知作者、124次同评论研究历史、105个材料版本、相同时间UUID翻页与资源越界。proof脚本保留既有三组test target并加入inspection组。用例只使用合成材料，不外发模型；这些是T05/T06/T07/T38/T39/T44/T46的部分场景，T01–T54继续NOT_RUN。

实际执行36项本地staging静态检查通过：修改前7份文件Git blob完整校验、SQL参数集合与对应bind数、只读SQL／事务、共用目录查询、无历史正文输出、独立总数、GET路由与404、无越界文件、测试候选登记等。该数量包含基础文件校验，**不等于36个业务测试，也未执行Rust或PostgreSQL**。`bash -n`通过；尝试proof脚本退出1/Cargo is unavailable，未进入Docker或数据库。Rust编译/fmt/clippy、隔离SQL/并发、EXPLAIN、全仓库治理和浏览器均NOT_RUN。

原月报`docs/progress/2026-09.md`为875681字节，本轮未使用截断读取覆盖它；在小型`docs/progress/README.md`增加本P1回执入口。集成前仍须将对应条目追加进完整月报并执行全仓库治理，不能将索引链接当作月报正文已更新。现有审计记录均保留，原手册和验收台账不改写。

## 下一步

继续P1的作品分页／共享显示标题、完整作品上下文、统一冻结来源资格及用户评论页接入；现已写的检查器与历史必须先通过隔离SQL和Rust证明，再用于UI。P2统一方法／选择／输入指纹后开放input_changed和默认排重。完整schema/constraints/views与执行控制就绪才注册迁移；共享升级仍需P0现场只读盘点和drain。不新增Coverage实体或第二引擎。
