# 评论研究产品化：开发、数据库升级与验收手册

> 状态：技术设计定稿，待用户批准开发；不是已实现或已验收声明
> 交付包：COMMENT-STUDY-PRODUCTIZATION-001 · 文档版 1.0
> 核对日期：2026-09-22
> 源码基线：`main@c74d72e3d17b9d5ecfb9953de025713c47e4560e`
> 责任：本包设计由 ChatGPT 整理；开发、共享库操作、真实模型调用与发布由 Mog 单独授权
> 事实与设计：标为“现状”的内容来自固定版本源码；“规定／新增／必须”为本次目标合同

## 1. 从这里开始，先不动业务库

实施前输入：Mog 的当前交付包授权、唯一 Issue/Claim、独立 branch/worktree、exact baseline、允许修改文件和是否允许真实外发。没有这些时只可继续只读检查与文档修订。本包不自动委托并发或子 Agent。

先读项目 `AGENTS.md`、`docs/README.md`、`docs/current-state.md`，再读本包计划、架构与本次负责模块。UI 必须再读现有 UI 执行合同和 LIDS 分册。不要把所有历史 V1/V2 计划塞入上下文来拼新实现。

## 2. P0：只读现场盘点

代码基线：确认 origin/main 是否仍是本文 SHA；变动则只比较本包列出的研究／API／migration／shared shell 文件，记录新增事实，禁止忽略已合并的他人工作。确认 worktree 干净、文件所有权未冲突。

授权访问本机后，先检查数据库名字、server major、migration ledger、clean-study schema 的列／约束／索引以及 Worker 状态。下列 SQL 仅示范只读盘点，凭据由现有安全入口提供，不写进命令记录：

```sql
BEGIN READ ONLY;
SELECT current_database(), current_setting('server_version');
SELECT migration_id, migration_sha256, applied_at
FROM linggan_local_schema_migration ORDER BY applied_at, migration_id;
SELECT table_name,column_name,data_type,is_nullable
FROM information_schema.columns
WHERE table_schema='public'
  AND (table_name LIKE 'linggan_comment_study_%'
       OR table_name IN ('linggan_material_comment','linggan_model_invocation'))
ORDER BY table_name,ordinal_position;
SELECT state,count(*) FROM linggan_comment_study_run GROUP BY state;
SELECT state,count(*) FROM linggan_comment_study_target GROUP BY state;
SELECT state,count(*) FROM linggan_comment_study_batch GROUP BY state;
COMMIT;
```

另外核对：存在 clean bootstrap 但 ledger 未记录这一历史初始化是正常待说明现象，不因此 reset；模型连接启用不证明 Worker 正在运行；root 里旧 Next.js 记忆不证明当前 Web 技术栈。记录真实在途请求数量，不把历史总计当作正在收费。

### 2.1 数据保护基线

用只读 COUNT 分别记录当前 raw comment/content/media、Run/Work/Target/Signal/Problem/revision/resolution/pair/membership/通用 ledger 数量；保存关系完整性摘要。需要精确“不改旧行”证明时，对研究层已有行按主键有序哈希（不包含本次新列），结果只记 hash 与行数。正文／dump／秘密只可留在授权本机的私有备份位置，不进 Git。

## 3. 增量数据库交付

### 3.1 编写三个增量 migration

依次为 schema、constraints、views 三个逻辑阶段。正式数字前缀从 exact HEAD 分配，完整 migration ID 按现行 ledger 校验。原 bootstrap、已应用 migrations 的字节不能改变。

- schema：创建三表、增加可空历史列、默认停止旧未记录执行；不清洗全库。
- constraints：准确回填 Target 稳定评论 ID；增加新写触发器、有效唯一索引、immutable policy / request 字段约束；不填未知历史方法。
- views：统一 effective_target 与独立计数读取关系；不把 SQL view 当权限自动成立，所有应用读取仍加现行来源限制。

发现旧同名表／索引却字段不一致，RAISE EXCEPTION，不以 IF NOT EXISTS 静默跳过。列／constraint 的兼容性用系统目录精确核对，不只检测 to_regclass。migration 脚本必须在测试中验证第二次由 ledger 跳过以及哈希改变被拒绝。

### 3.2 两条安装路径，不能混用

**已有 clean-study：**仅执行新 delta。不调用 bootstrap，不调用 reset。缺必需表／列时报告 schema mismatch，不能自认成空库。

**第一次安装 clean-study：**新增受控脚本 `scripts/init-comment-study.sh`，只在整个 clean-study 基础表集合均不存在、保留材料／通用模型 schema 完整、无模型进程外发、用户明确授权时执行原 `database/bootstrap/comment-study-001.sql`，再由普通 migrate 应用 delta。它只建新 clean 表，不读取／转换旧 V1 研究数据，不运行 comment-study-reset.sql；如果 clean 基础表只存在一部分，拒绝并要求调查。历史迁移形成的旧空表不是新运行路径，不借此接回旧代码。

`local-runtime.sh migrate` 的新 delta 注册前显式检查 clean schema 前提：未初始化则退出并说明初始化入口，不自动创建或重置。普通 `serve` 当前会先 migrate，因而带真实副作用；上线必须预先独立执行并验收 migrate，禁止把 serve 当只读试启动命令。

测试 fixture：保留当前 rebuild proof 使用的 raw/material fixture 与 bootstrap 初始化；在新 productization fixture 中先初始化 bootstrap，然后顺序追加 delta。不能直接修改原 bootstrap 让升级测试“看起来通过”，也不能把新约束提前加到所有旧 fixture 而不测试已有数据升级。

### 3.3 历史队列处理

升级前 drain 旧 Worker，等待已发出的请求结束或超时并完成记账；再次确认无运行旧进程。新列默认将旧 Run 的未来外发设为 stopped/legacy_unrecorded，现有 Run/Target/Signal 原字段保持原样。旧有效结果参与安全只读与去重，不因方法未知而全部消失。

旧未完成项显示“历史执行未继续”，不伪称成功或丢失。需要续做时，用户明确选这些稳定评论，使用完整新方法和 retry_failed/reanalyse 建新 Run；不得自动套新 prompt 到旧 Run。对旧 queued/running、实际已停的目标，retry_failed 的受控续做识别规则为：所属 Run dispatch_state=stopped/legacy_unrecorded 且没有存活 invocation；新命令需说明理由。旧历史记录不改、不复活旧执行。

这个安全停留不妨碍新材料研究。禁止新旧 Worker 同时写入；无法确认 drain 时停止发布，不停止其他纯文档／合成开发工作。

## 4. P1：材料读取与清洗缓存

先修复 source 最新版本选择与父语境限制；抽出共用 source 判定，加入纯规则 fixtures。实现 cache 后端刷新及目录 keyset、字面搜索、作品分页；按页取语境，不能每个列表行各查询整篇文本和完整研究历史。

新增 `catalog` DTO，接入 HTTP；研究历史先按稳定身份聚合。完成新版用户评论页，其他 Tab 暂不改业务语义。清洗 cache 稳定性与权限 JOIN 必须同时做，不把“已有缓存”当作源仍可外发。

## 5. P2：方法与原子启动

将三个现有 prompt 的固定规则移入可生成、可测试的模板函数；扩展 policy 表作为方法版本，不额外建提示词系统。三个严格 outputSchema 与 Rust 类型逐字段回归，包括 nullable reason、frame、pair proposedProblem。

实现 preview 与 start 共用选择器：preview 无持久副作用，start 在数据库事务里重新判断。HTTP 只映射 DTO，客户端不能传入 origin 或篡改 scope 数量限制。最后提交方法／预算、持久 requestRef，确保网络重试与用户新意图不同。

同时变更所有能准备／领取 batch 的入口：新 Run 直接 queued 且有完整 execution_manifest；禁止用旧 prepared 的名字伪造未授权状态。Target 插入、收尾、停止与唯一索引语义一起测试。

## 6. P3：执行正确性与自动化基础

接入三阶段请求 snapshot、统一预算与领取 fence；按固定锁序改接受器。usage 要在解析失败前记账；snapshot 不进入通用事件流。修复缓存顺序、枚举、合法性以及 candidate revision。

实现 paused/resume/stop 和 control_version CAS；到期恢复放 tick 最前，三阶段都覆盖。停止时不销毁已接纳结果，不承诺已发远端立即停止。一个阶段异常不影响另外可运行阶段；embedding 缺失不阻断提取。

扩展现有 drain 行为测试，避免发布时旧进程仍在调用。禁止为了通过测试把真实 provider endpoint 偷换成公网可调用默认值；测试默认使用 synthetic model、可控时钟与 local adapter stub。

## 7. P4：有效知识与四 Tab 接通

实现 effective_target、当前来源一致性与复合身份去重。所有 Signal recall／embedding 输入／pair 种子／membership 读取逐项走查：不能只修 UI，后台还重复使用旧重研 Signal。Problem 定义受限种子和普通支持受限要分开处理。

将五 Tab 改为四 Tab；保留概览上半区和观察图，取消前100条数据拼总量的逻辑。inline IIFE、原 JS 入口和新 ES module 在同一提交中清理为单一初始化；迁移 inline CSS 不叠覆盖。以 URL 统一下钻和返回。

## 8. P5：验证矩阵

下列均为**待实现并执行的验收**。ID 固定，实施回执逐项写 PASS/FAIL/NOT RUN 与证据；不得默认勾选。

### 8.1 数据与选择

| ID | 合成场景 | 必须证明 |
|---|---|---|
| T01 | 688 条合格，100 已成功，再 new_only 100 | 新100不与前100重叠；不同评论统计200 |
| T02 | 同一稳定评论换 material_ref、正文未变 | 不新增独立评论，不算 input_changed |
| T03 | raw 标点变化／仅点赞变化 | 前者输入可比较变化，后者不变 |
| T04 | 新 OCR 未入保留语境／真正填补父语境 | 前者不触发，后者 fingerprint 改变 |
| T05 | 纯emoji、纯mention、混合语义、不可识别mention边界 | 仅确定无效剔除；原文证据保留 |
| T06 | 评论或作品作者身份未知 | 原声可显示但不研究；作者声音不是用户问题种子 |
| T07 | 最新父评论受限／UNKNOWN，旧版已知 | 不回退到旧可读父语境 |
| T08 | 60已索引可用＋40待清洗、预算100 | 冻结60并明确部分；不阻塞这60，不向 Run 追加后来40 |
| T09 | 0可用已索引但仍有待清洗 | index_pending，不建空 Run，不显示全库无结果 |
| T10 | 只有 needs_context 的一批目标 | Run 提取阶段可终结，不永久 running |

### 8.2 并发、额度与故障

| ID | 合成场景 | 必须证明 |
|---|---|---|
| T11 | 两连接，同 requestRef 同 payload | 恰好一个 Run，回执相同 |
| T12 | 同 requestRef 不同额度或方法 | 409，无第二次副作用 |
| T13 | 不同 requestRef 同评论同时提交 | 最多一个获准在途 Target，不同时收费 |
| T14 | 第二事务等锁后，第一事务刚提交 | 第二次看到已提交选择；不能借 RR 旧快照重复选 |
| T15 | 响应丢失后相同 requestRef 重试 | 返回同 Run；不重建 Target |
| T16 | 无工作回执后又来了新评论 | 旧 requestRef 仍 no_work；新意图才可能创建 |
| T17 | 编辑预算而未独立保存策略 | Run 实际预算等于最终确认值 |
| T18 | 两 Worker 同时预留最后一份 token 额度 | 只一个成功预留，三个阶段统一计费 |
| T19 | usage缺失／明确未外发／实际超预留 | 分别保留预留／记0／真实超额并停下次调用 |
| T20 | timeout=60、请求用了>60秒的旧租约边界 | 新 deadline 正确；晚到不得冒充新执行 |
| T21 | reserve后崩溃、CAS后崩溃、返回后写入前崩溃 | 按矩阵有界恢复；每 invocation 仅一次本地外发领取 |
| T22 | 100次tick持续失败secret/JSON/adapter | 同比较上下文请求≤3；不无限重置计数 |
| T23 | 12目标中2合法、1遗漏，其余结构错误 | 合法2保留；遗漏不变 no_signal；重试只受影响项 |
| T24 | pair/resolution ledger已失败但pointer仍挂着 | 恢复器结算，不永久 pending |
| T25 | 一条超大输入不能装入单批 | 明确 input_limit_exceeded；后续正常目标与其他 Run 仍推进 |
| T26 | embedding故障且semantic可运行 | semantic 不被卡死；召回如实 incomplete |
| T27 | pause/stop 与 provider进行中竞争 | 不发新请求，已授权且未过期结果可结算，已存结果不删除 |
| T28 | 重复或过期controlVersion | 409或读取既有终态，不能覆盖较新的控制动作 |

### 8.3 方法、缓存与知识

| ID | 合成场景 | 必须证明 |
|---|---|---|
| T29 | 三阶段 Schema 与 Rust DTO | 字段、nullable、enum 完全一致，Schema字节上限成立 |
| T30 | fixed same四维命中／unknown一维／different一维 | 使用唯一判定函数；旧 equivalent 不进入新缓存 |
| T31 | 一个严格可用缓存命中 | 0个新 invocation；不是先调用后缓存 |
| T32 | 无效模型JSON碰巧包含dimensions | 不写合法比较缓存，不写 membership |
| T33 | 比较期间current revision改变 | 不套新定义接纳，不将失败当 novel |
| T34 | 方法说明、模型、include/exclude变化 | 比较 key 改变；纯时间变化不制造新语义比较 |
| T35 | raw与clean的全角／空白／emoji不同 | 引用和basis仍逐字命中 raw，不因净化而无原文可引 |
| T36 | 成功重研相同评论10次 | 不同评论／作者支持数不随 Target 行数膨胀 |
| T37 | 最新成功是no_signal，旧有problem信号 | 当前head无Signal，历史可查但不填当前统计 |
| T38 | 最新尝试failed，旧有合法结果 | 显示最后失败，保留旧有效head；不假装本次成功 |
| T39 | 评论受限，frame/basis/prompt曾有正文 | 相关读取面全部脱敏；无日志／快照旁路 |
| T40 | Problem seed受限 vs普通membership受限 | 分别隐藏定义或只剔除对应支持；其他问题继续显示 |
| T41 | 新增2条合法membership、其他仍pending | 这2条立即前端可见，无全批发布门闸 |
| T42 | 没有Problem但有solution/experience | 能读办法与经历；不强制生成问题填页面 |

### 8.4 页面、性能与升级

| ID | 合成场景 | 必须证明 |
|---|---|---|
| T43 | 搜索第101篇之后作品 | 真正服务器检索与翻页，不仅过滤当前100 |
| T44 | 300目标、多个作品 | targetCount=300且全部可翻页；无JOIN乘法 |
| T45 | 1/2/3/长中文词及%/_/反斜杠 | 字面查询正确；短词慢不伪装0结果 |
| T46 | 切domain/Run/筛选时旧请求晚到 | 不覆盖新状态；不越域复用cursor |
| T47 | 四Tab下钻、Back、Esc和关闭抽屉 | 完整路径可达且保留阅读位置／焦点 |
| T48 | 初用/无匹配/错误/部分/受限/超长/极大数字 | 独立空态；1440/1024/390与200%缩放可用 |
| T49 | 新旧布局脚本同时存在的回归 | 只有一个初始化和事件绑定，图表切换仍有效 |
| T50 | 升级已有clean-study含真实形状合成历史 | 旧原始／派生字段哈希与行数保持，未知方法不伪填 |
| T51 | bootstrap→delta 与已有baseline→delta | 目标schema一致；partial schema拒绝；不执行reset |
| T52 | 同migration重跑／checksum变动 | ledger跳过或拒绝，不重复DDL／回填 |
| T53 | 回滚API版本但数据库已有新合同 | 不启动不懂新协议的旧Worker；不为回滚删新数据 |
| T54 | 合成scheduled origin经过同启动函数 | 同选择／排重／预算合同，不需要UI才能运行 |

## 9. 可执行的测试入口与证据等级

已有入口（本轮文档生成没有执行）：

```bash
cargo test -p linggan-intelligence --locked
cargo test -p linggan-api --locked
./scripts/check-rust-boundaries.sh
./scripts/check-invariants.sh
./scripts/check-project-governance.sh
./scripts/verify-ui-design-handbook.sh
./scripts/test-comment-study-rebuild-postgres.sh
./scripts/test-local-runtime.sh
```

新建 `crates/intelligence/tests/comment_study_productization_postgres.rs`，用独立 fixture 实现 T01–T54 对应数据库用例。新增受控 `scripts/test-comment-study-productization-postgres.sh`，复用既有随机 proof DB/container/volume 模式与 cleanup；不得默认连开发库。脚本负责过滤真实外部调用，只使用 synthetic secrets/local provider stub；不为跑测试自动启动用户 Docker。

新脚本的测试命令目标固定为：

```bash
RUST_TEST_THREADS=1 cargo test -p linggan-intelligence \
  --test comment_study_productization_postgres --locked \
  -- --ignored --nocapture --test-threads=1
```

测试函数名、夹具与错误码按本包ID和合同落地，不仅写静态 HTML includes 断言。浏览器自动测试可用当前项目可用环境；环境缺失则记录 NOT RUN，不能声称“页面已验证”。关键用户动作必须用本地 HTTP＋隔离库证明回执，不用 mocked toast 替代数据库事实。

分层报告：L1编译/单元；L2隔离PostgreSQL/并发；L3浏览器真实接口；L4经单独授权的有限真实模型与人工标注质量；L5发布/用户业务验收。低层成功不推出高层成功。

## 10. 性能验证与可迭代边界

用合成 1万、10万、100万 stable comments，包含重采版本、中文短词、缺身份、受限、不同作品倾斜。分别记录 cold/warm、索引覆盖、响应大小、EXPLAIN(ANALYZE,BUFFERS)、Rust峰值内存、事务持锁时间，不只测一页无历史的空库。

工程目标：已索引常规列表50条 warm p95≤1秒，复杂短词≤3秒，启动100目标一般≤3秒；这些不是当前测量结论。未达标先消除fetch_all、N+1、JOIN放大、缺索引与重复context，不能立即加分片／搜索服务。百万规模若确证现有视图和索引不足，记录瓶颈与可独立升级的投影，不隐瞒或虚报已支持百万级。

保持 Mac mini M4 16G 的总体资源边界：索引清洗和SQL允许降速，Embedding继续让路OCR／ASR；页面只取当前页。禁止为了测试打开十个常驻WeMM实例或把全库JSON送给模型。

## 11. 发布与回退

经 Mog 明确授权后：锁 exact head → 本机备份与只读基线 → drain所有旧模型Worker → 确认无在途／过期已结算 → 单独 migrate → 约束／回填核验 → 新API与新Worker同版本启动 → 本地只读烟测 → 有限授权模型试运行 → Mog业务验收。自动计划保持关闭。

默认不带任何已保存的旧自动计划启用标记；既有 generic active_auto_plan_ref 或旧 automation 数据不可被新流程默认为授权。

回退优先暂停新外发、保留数据、修复前进。已应用增量 migration 不执行破坏性 down；旧二进制若不理解新method/request/fencing合同不得重新接管执行。UI可回退到安全只读页，不把新的cancelled字段硬映射成功。数据库备份恢复只能作为明确批准的灾难恢复，不作为普通回滚步骤。

## 12. 入库文档与交接

本包批准后，在 docs/README 中登记计划、架构、两个合同、页面规格、运行手册和自动化下一阶段。current-state 只写简短阶段及未验证边界；progress记录真实发生事项。旧页面合同的五Tab／旧启动路径标为被本包替代；其历史测试与研究原理不删除。

最终交接必须附：exact head、受影响文件、migration完整ID和SHA、已执行测试与未执行项、数据库历史保留摘要、请求快照／预算证明、浏览器路径、真实模型授权与样本范围、明确未验证准确性。不得只给“全部完成”或一个绿色测试数量。
