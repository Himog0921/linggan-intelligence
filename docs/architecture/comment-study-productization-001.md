# 评论研究产品化：架构、来源与差异

> 状态：技术设计定稿，待用户批准开发；不是已实现或已验收声明
> 交付包：COMMENT-STUDY-PRODUCTIZATION-001 · 文档版 1.0
> 核对日期：2026-09-22
> 源码基线：`main@c74d72e3d17b9d5ecfb9953de025713c47e4560e`
> 责任：本包设计由 ChatGPT 整理；开发、共享库操作、真实模型调用与发布由 Mog 单独授权
> 事实与设计：标为“现状”的内容来自固定版本源码；“规定／新增／必须”为本次目标合同

## 1. 已核对事实

本次核对固定在 `c74d72e3d17b9d5ecfb9953de025713c47e4560e`。没有连接用户的本机数据库、没有调用真实模型，也没有对本机浏览器进行运行验收。

| 来源 | 当前事实 | 本包处理 |
|---|---|---|
| S01 `database/bootstrap/comment-study-001.sql` | clean-study 表在 bootstrap，非 numbered migration；已有 policy/run/target/batch/signal/problem/revision/resolution/pair/membership/comparison | 通过增量 migration 升级，不重新执行 bootstrap |
| S02 `comment_study_source.rs` | 作者身份、清洗、领域与受限来源资格；准备页作品截断 100；候选 `.fetch_all`；父语境查询缺少同等 restriction 过滤 | 共用资格函数；分页；先确定最新版本再判断可读；父语境补同等资格 |
| S03 `comment_study_run.rs` | 从活动 policy 建 prepared Run；每 Target 存一次完整 workContext；Run 完成只取决于 Target 终态 | 新命令显式 policy；新 Target 不重复作品语境；Run 语义结束与归并进度分开 |
| S04 `comment_study_batch.rs` | 同篇合批，上限 12；请求目标含 researchText，没有 rawText；prepared 可被 Worker 打包 | 保留合批；新请求补原文；开始动作即授权，不伪造待启动 |
| S05 `model_runner.rs` | 实际先 pair→resolution，后缓存；回收靠后；早期步骤进展或错误可能阻止语义执行 | 先回收，缓存先于比较外发；小型轮转、逐阶段错误隔离 |
| S06 `comment_study_resolution_worker.rs` | 候选定义取 claim 时的 current revision；缓存写入发生在合同接纳之前；Schema 对候选描述很宽 | 冻结 revision 与完整定义；先验证后缓存；严格 Schema |
| S07 `comment_study_comparison_cache.rs` 与 `comment_study_problem_resolution.rs` | 缓存 verdict 判定使用 equivalent/compatible，实际合同使用 same/different/unknown | 复用同一个判定函数；新键命名空间，不删除旧记录 |
| S08 `comment_study_read.rs` | run 列表同时按 run 联接 work/target，计数有放大风险；部分受限返回仍有 researchText 或 problemFrame | 独立聚合；统一所有派生文本限制传播 |
| S09 `0040_model_pi.sql` | 通用模型配置、模型项、连接版本已有不可变约束与独立调用账本 | 直接绑定；不重造供应商设置与账本 |
| S10 `comment_study_pair_worker.rs` | pair 使用 first Signal 所属 Run 的配置；调用外已持久 invocation 指针，但故障结算需补齐 | 明确 ownerRun；有限尝试与晚到结果 fencing |
| S11 `scripts/local-runtime.sh` | numbered migration 注册到 0101；`serve` 实际先调用 migrate；clean bootstrap 不在该注册清单 | 修正“任何启动都不迁移”的旧口头表述；升级入口必须显式受控 |
| S12 `comment_study_embedding.rs` | 编码 eligible Signal 与 active Problem core，而非全评论库；缓存及让路机制已存在 | 不更换；仅使候选遵守有效结果与来源可读性 |

这些是源码审查结论；“存在风险”的项仍需回归用例证明受影响范围，不能把风险数量写成生产故障数量。

## 2. 一条链路、两个发起者

```text
本轮：本地 UI → HTTP DTO ───────────────┐
下阶段：计划到期扫描 → 内部可信上下文 ───┤
                                      v
                          start_study_run(command, origin)
                            /  选择 + 幂等 + 冻结  /
                                      v
                           已有 Run → Work → Target
                                      v
            已有 same-work batch → semantic → Signal
                                      v
                   已有 WeMM / recall / comparison cache
                                      v
                    resolution / pair → Problem + membership
                                      v
                         安全读取 → 四个 Tab
```

语义提取阶段不创建长期问题；Embedding 只召回候选；确定性接受器决定记录什么。自动化只改变“谁在何时要求选一批”，不改变证据规则、模型请求路径或 UI 的状态含义。

## 3. 当前只新增三张表

| 表 | 唯一职责 | 为什么不能直接省掉 | 可否删除后再生 |
|---|---|---|---|
| `linggan_comment_study_clean_cache` | 缓存确定性清洗结果与理由，支持检索 | 每次搜索把全库拉进 Rust 清洗，会破坏分页和增长能力；直接复用旧 V1 派生表则复活旧系统 | 可定向再生，不是原始来源；不因此触发模型 |
| `linggan_comment_study_start_request` | 保存一次开始请求的幂等与无工作回执 | Run 上单个 requestRef 无法记住“该请求当时确实无工作”，重试可能在未来变成收费动作 | 不自动删除；是用户动作事实，不是任务队列 |
| `linggan_comment_study_model_request` | 某次 invocation 的真实不可变请求与外发领取凭据 | 只存最新 prompt 会丢失重试前输入；通用账本当前刻意不放评论／提示词 | 不自动删除；受来源限制控制，不参与公开日志 |

不新增 Method 表、Coverage 表、CommentIdentity 表、ResearchTask 表、ResultRevision 表。已有 `policy` 就是方法版本，稳定评论使用原有复合身份，有效研究结果使用只读关系。下一阶段最多另增一张本模块 `schedule` 表，见单独手册。

## 4. Rust 模块与文件职责

以下“新增”为本包拟定的文件，不宣称当前存在。

| 文件，相对 `crates/intelligence/src/` | 处理 | 唯一职责／公开入口 |
|---|---|---|
| `comment_cleaning.rs`、`comment_cleaning_noise.rs` | 复用，不改清洗语义 | `clean`、偏移映射、cleaner 版本 |
| `comment_study_source.rs` | 收束现有逻辑 | 领域／版本／作者／受限来源资格；批量语境冻结；禁止调用模型 |
| `comment_study_catalog.rs` | 新增 | `refresh_clean_cache`、`read_comment_catalog`、`read_work_catalog`；全库分页与缓存覆盖 |
| `comment_study_selection.rs` | 新增 | `preview_study_selection`、事务内 `select_study_targets`；四种选择模式和 fingerprint |
| `comment_study_policy.rs` | 新增 | `create_study_policy_version`、`read_study_policies`、`activate_study_policy`；不可变方法快照 |
| `comment_study_run.rs` | 扩展 | `start_study_run`；Run 冻结、幂等回执、暂停／恢复／停止；不执行模型 |
| `comment_study_model_request.rs` | 新增 | 请求快照、token 预留、领取、失效判定、有限故障恢复；不做语义判断 |
| `comment_study_batch*.rs`、`comment_study_model_dispatch.rs`、`comment_study_model_runner.rs` | 局部修改 | 保留批处理和接纳；接入统一请求留痕与预算；完整 rawText |
| `comment_study_resolution_worker.rs`、`comment_study_pair_worker.rs` | 局部修改 | 保留三阶段，复用方法和请求辅助函数 |
| `comment_study_problem_resolution.rs` | 复用并公开纯判定帮助函数 | same/different/unknown 的唯一确定性标准 |
| `comment_study_problem_store.rs`、`comment_study_candidate_recall.rs` | 局部修改 | 固定候选 revision、当前有效 Signal、合法 membership、retrieval_incomplete 恢复 |
| `comment_study_comparison_cache.rs` | 修复 | 合法比较的缓存；命中不产生 invocation |
| `comment_study_read.rs` | 保留入口，拆内部子模块 | Run 及汇总投影；不得继续扩为一个大 SQL 字符串文件 |
| `comment_study_read/corpus.rs`、`runs.rs`、`problems.rs` | 按职责拆出 | 被 `comment_study_read.rs` 私有引入；不增加二次业务服务 |
| `model_runner.rs` | 小型调度修复 | 回收优先、轮转、公平、drain；不成为材料选择者 |

`catalog` 负责材料读与清洗缓存，`read/corpus.rs` 仅组合研究历史和 UI DTO；不能两处各写一套来源查询。重复 SQL 用一个静态 SQL 定义或共享函数生成，不用通用查询 DSL。简单 DTO 与所在功能同文件；超过职责边界再拆，不按任意行数拆空壳。

### 4.1 HTTP / 前端

保留 `apps/api/src/local_web/comment_study.rs` 作为路由与本地 guard 入口；新增其私有 `comment_study_api.rs` 承担 DTO 映射和状态码，业务不放 handler。保留 `comment_study.html/css/js` 和共享 shell，不改站点 Token。

JS 采用原生 ES modules：`comment_study.js` 为入口；只拆 `comment_study_client.js`（请求／错误／URL）、`comment_study_views.js`（四视图渲染）。只有实际复杂度需要时再按四视图拆文件；不引入前端构建框架或全局状态管理库。所有新增 assets 由现有 host 显式挂载；模块 MIME、相对路径、缓存版本必须测试。

### 4.2 共享数据库关系

新增只读 SQL view `linggan_comment_study_effective_target`，定义有效 Target head 的稳定选择；它不是一张状态表。清洗缓存记录只含确定性字段，不保存作者是否可信、是否已研究、是否有价值等动态推论。权限在每次查询／发送／接纳时检查，不能依赖缓存时间或 UI 标签。

## 5. 简单性约束

一个表有一个不可替代事实；一个状态有一个明确用户含义；一个 Worker 辅助函数不创建新调度系统。沿用 SQLx，不增加 ORM。继续使用既有 PostgreSQL 与 pgvector；可新增 `pg_trgm` 本地扩展索引，但不部署搜索服务。普通字符检索不宣传语义搜索。

不为“百万库量”预先增加分片、流处理、双写、物化多份统计；先用有界 SQL、必要索引、按页取语境、EXPLAIN 与基准数据证明是否需要升级。性能阈值是验收目标，不是当前性能声明。

## 6. 必须同时修复的断点

1. **raw / clean 合同：**原声证据校验 against raw，但现有 batch 只给 researchText；新请求同时给 rawText 与 researchText，evidence 及 basis 只能复制 rawText。每个 workContext 仍只给一次。
2. **方法漂移：**历史不猜；新 Run 绑定不可变 policy；semantic、resolution、pair 全部读取该版本，不能某阶段继续硬编码另一套 prompt。
3. **缓存漂移：**统一枚举；先验证后缓存；缓存查询先于外发；key 覆盖实际比较 payload、方法阶段和定义边界。保留旧缓存但新合同不读取旧命名空间。
4. **候选漂移：**取出 candidate set 的同时封存 revision；模型回程发现 revision 已变化，不算 no_match，不直接建新 Problem。
5. **假完成：**Run.state 只描述语义提取。下游未归并、缺向量、预算暂停不冒充“全部研究完成”。
6. **跨请求并发：**等锁后的重复检查要在新快照中运行；不能仅在现有 Repeatable Read 内等 advisory lock 就认为已看见前一个提交。
7. **限制传播：**禁止只遮原评论而继续返回 problemFrame、basis、研究快照或缓存正文。
8. **饥饿与悬挂：**回收不可因某阶段一直进展而推迟；一个 embedding 或 pair 异常不能永远阻断正常 semantic。

## 7. 固定来源目录

所有仓库链接均固定到本次 SHA；安装／运行状态以现场证明为准。下列文件构成本包事实依据，不要求 Agent 先通读所有历史文件。

- [AGENTS.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/AGENTS.md)
- [docs/README.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/README.md)
- [docs/current-state.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/current-state.md)
- [docs/governance/agent-collaboration.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/governance/agent-collaboration.md)
- [docs/decisions/0006-comment-research-clean-rebuild.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/decisions/0006-comment-research-clean-rebuild.md)
- [docs/design/lids/README.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/design/lids/README.md)
- [docs/agents/ui-execution-contract.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/agents/ui-execution-contract.md)
- [docs/design/README.md](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/docs/design/README.md)
- [database/bootstrap/comment-study-001.sql](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/database/bootstrap/comment-study-001.sql)
- [database/migrations/0016_material_social_lanes.sql](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/database/migrations/0016_material_social_lanes.sql)
- [database/migrations/0025_comment_current_projection.sql](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/database/migrations/0025_comment_current_projection.sql)
- [database/migrations/0040_model_pi.sql](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/database/migrations/0040_model_pi.sql)
- [database/migrations/0094_corpus_evidence_read_recovery.sql](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/database/migrations/0094_corpus_evidence_read_recovery.sql)
- [crates/intelligence/src/comment_study_source.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_source.rs)
- [crates/intelligence/src/comment_study_run.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_run.rs)
- [crates/intelligence/src/comment_cleaning.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_cleaning.rs)
- [crates/intelligence/src/comment_study_batch.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_batch.rs)
- [crates/intelligence/src/comment_study_batch_worker.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_batch_worker.rs)
- [crates/intelligence/src/comment_study_model_dispatch.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_model_dispatch.rs)
- [crates/intelligence/src/comment_study_model_runner.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_model_runner.rs)
- [crates/intelligence/src/comment_study_resolution_worker.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_resolution_worker.rs)
- [crates/intelligence/src/comment_study_pair_worker.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_pair_worker.rs)
- [crates/intelligence/src/comment_study_problem_resolution.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_problem_resolution.rs)
- [crates/intelligence/src/comment_study_problem_store.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_problem_store.rs)
- [crates/intelligence/src/comment_study_comparison_cache.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_comparison_cache.rs)
- [crates/intelligence/src/comment_study_read.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_read.rs)
- [crates/intelligence/src/comment_study_embedding.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_embedding.rs)
- [crates/intelligence/src/model_runner.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/model_runner.rs)
- [apps/api/src/local_web/comment_study.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/apps/api/src/local_web/comment_study.rs)
- [apps/api/src/local_web/comment_study.js](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/apps/api/src/local_web/comment_study.js)
- [scripts/local-runtime.sh](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/scripts/local-runtime.sh)
- [scripts/test-comment-study-rebuild-postgres.sh](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/scripts/test-comment-study-rebuild-postgres.sh)
- [crates/intelligence/tests/comment_study_rebuild_postgres.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/tests/comment_study_rebuild_postgres.rs)

- [apps/api/src/local_web/comment_study.html](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/apps/api/src/local_web/comment_study.html)
- [crates/intelligence/src/comment_study_canonical.rs](https://github.com/Himog0921/linggan-intelligence/blob/c74d72e3d17b9d5ecfb9953de025713c47e4560e/crates/intelligence/src/comment_study_canonical.rs)

### 外部技术依据，仅用于验证数据库机制

- PostgreSQL 16 事务隔离文档：本包用 READ COMMITTED + 事务级锁 + 单条冻结查询，避免把锁等待误当新快照。https://www.postgresql.org/docs/16/transaction-iso.html
- PostgreSQL 16 锁文档：事务级 advisory lock 的释放、行锁与死锁要求。https://www.postgresql.org/docs/16/explicit-locking.html
- PostgreSQL 16 部分索引：用于当前在途评论的唯一性约束。https://www.postgresql.org/docs/16/indexes-partial.html
- PostgreSQL 16 pg_trgm：用于字面子串检索索引；短模式可能无法有效使用 trigram，不据此宣称中文语义检索。https://www.postgresql.org/docs/16/pgtrgm.html

## 8. 文档冲突裁定

本包批准入库时，只替代旧页面的五 Tab 和“先保存策略再创建 prepared”的交互合同，保留现有研究证据与 Problem 门槛。DEC-0006 的 reset 授权按历史事项保留，不扩展。`database/README.md` 的旧 V1 章节及“serve 不迁移”式描述需要标为历史／纠正；不得因为文档老而重新恢复旧表。

上传的设计 HTML 只提供表达规则，不提供真实数据或动作权限。其 hover-lift、覆盖一级导航的原型 drawer 等与当前更严格合同冲突的部分不继承；当前 token 真源仍为 `lids_tokens.css`。
