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

- 数据库合同§3–4、开发手册§3.1：新增 `database/migrations/0102_comment_study_productization_schema.sql` 候选。恰好三张批准的新表：clean_cache、start_request、model_request；policy/run/target/membership仅增加约定字段。旧方法/指纹/匹配版本不倒填，旧原字段不UPDATE。安装pg_trgm并建立清洗检索索引。
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

## 下一步

继续同一P1：复用共享作品标题读取完成作品分页、评论详情与历史清单，统一source资格和父语境；补齐隔离测试并接用户评论页。P2输入指纹和方法/启动完成后开放input_changed。完整schema/constraints/views与执行控制集成后才注册迁移，实际共享升级仍先完成P0现场盘点和drain。每次回到本记录及固定手册，不新增Coverage实体或第二引擎。
