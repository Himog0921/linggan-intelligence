# COLLECTION-UPGRADE-001 · 采集执行、交付恢复与材料完成收口

> 状态: 活跃计划
> 最后核对: 2026-09-21
> 适用范围: 采集链路的关键词详情缺口准入、派发资格、跨工单重试预算、交付恢复、统一状态读取与运行诊断
> 事实来源: Mog 派定的交付包 `linggan-collection-upgrade-handoff-2026-09-21`、`origin/main@1ef5830c` 的代码与迁移、隔离 PostgreSQL 基线运行结果
> 冲突时以谁为准: 交付包合同与 Mog 最新确认；实现事实以当前代码、迁移与真实验证输出为准

## 1. 派定、授权与不做的事

Mog 于 2026-09-21 派定交付包 `~/Downloads/linggan-collection-upgrade-handoff-2026-09-21 2/`，要求「按照这个包进行系统的修复升级」。交付包内含 S0–S6 步骤、T01–T34 验收矩阵与 E01–E10 事实表，是本包的唯一用户结果授权。

包内优先级：**必修 A** = S1a 交付恢复、S1b 缺输入止转，**必修 B** = S2 跨工单重试预算；**配套 C** = S3 统一材料完成判据与当前运行状态、S4 结构化日志/就绪分级/统一 tick/选择器诊断；**条件 D** = S5 有证据支持才实施的性能与结构优化；S6 为集成交付与历史处置准备。

本包**未获授权**的动作（缺少单独授权时不得执行）：

- 共享库 migration 应用、任何共享数据库写入或历史数据处置
- merge、push、部署或 runtime 切换
- 插件重新加载、发布包发布
- 真实平台访问（不打开小红书/抖音页面，不消耗平台访问预算）

交付包同时要求「不得制造成功」：不把历史失败状态改写成完成、不清空 outbox、不改挂旧 Attempt、不刷新失败预算。

## 2. Claim 与固定工作区

- 派定: Mog 交付包（见 §1）。**本包没有对应的 GitHub Issue**：`gh issue create` 被自动模式分类器拒绝（在用户未要求的情况下以用户身份创建包含内部交付内容的公开 Issue，且与仓库治理「Issue 由 Mog 派定」冲突）。按 `docs/agents/issue-tracker.md`，受保护交付的派定可以是「Issue/交付包」二者之一，故以交付包作为派定，Claim 记录在本节，Issue 缺口在交付回执中如实报告给 Mog。
- Stable task-id: `collection-upgrade-001`
- Exact base: `1ef5830c2d892afb0e09dd4c7ef0d395f46a7eac`（`origin/main`）
- Branch: `fix/collection-upgrade-001`
- Worktree: `/Users/moglenny/proma/linggan-intelligence/.worktrees/collection-upgrade-001`
- Preflight: branch/HEAD/目录与本节一致，工作树起始干净。共享 checkout 与 `runtime-main` 均不作为写入位置。
- 并行所有权: 本机另一个会话在 `main` 上有未提交的插件发布删除与未跟踪的 `docs/audit/`，按交付包要求原样保留，不纳入本包。

## 3. S0 基线事实

| 项 | 事实 | 来源 |
|---|---|---|
| 交付包所述基线 | `1511725c`（= PR #321 合并点） | 包 README |
| 实际 `origin/main` | `1ef5830c`（= PR #323 合并点），比包所述多 21 个提交 | `git rev-list --count 1511725c..HEAD` |
| 最高 migration | 基线最高 `0095_detail_page_url_rejection.sql`；本分支已用 `0096_detail_page_session_lane_delivery_identities.sql`，S2 台账顺延 `0097`。交付包 §4.1 要求「执行时取下一个空闲编号、不预占」——**合并前必须重新 fetch 复核 `0096/0097` 是否仍空闲，撞号则整体顺延并同步 5 处登记**（migration 文件、两处 control 测试、`material_fixture`、`full_schema_fixture`、`scripts/local-runtime.sh`）。S2 就在 `0097` 上追加（`execution_input_frozen_at`、`prior_failures_unverified` 两列）：该文件从未应用到任何共享库、分支未合并，追加比拆一张只含两列的 `0098` 更好读；现 sha256 `fde9a03309bcad985bc20ba0e4a38b1c9a6f9927c0902c2ed1df3a65f77f5b2e`，三处登记已同步 | `database/migrations/`、交付包 03 §5 |
| live runtime | `~/Library/Application Support/Linggan Intelligence/runtime-main` HEAD = `1ef5830c`，与 `origin/main` 一致 | 只读核对 |
| 插件版本 | `0.8.54`（release manifest 在 `plugins/linggan-intelligence-browser/releases/`） | `manifest.json` |
| 隔离 PostgreSQL 基线 | `./scripts/test-collection-dispatch-sequence-postgres.sh` 通过：**21 passed / 0 failed**，容器/卷/库自建自清 | 2026-09-21 运行，日志 `/tmp/s0-baseline-dispatch-proof.log` |

### 3.1 对交付包的基线更正

包 README 称 `1511725c → 1ef5830` 的改动是「评论研究及相关文档」。该描述只对 PR #323 成立。中间的 **PR #322 `detail-page-url-invalid-001`** 直接改动采集模块：`crates/evidence/src/dispatch.rs`（+110 行）、`database/migrations/0095_detail_page_url_rejection.sql`、`crates/evidence/tests/collection_dispatch_sequence_postgres.rs`（+148 行）、插件 `background.js`/`adapter.js`/`content/index.js`/`shared/deadPageSignals.js`、插件版本 0.8.53→0.8.54 与发布包。

后果：交付包内所有以行号给出的插件与 `dispatch.rs` 定位均视为**过期**，一律以符号名重新定位（包 §02 亦要求「符号名比旧行号可靠」）。本计划下列引用均为当前 `1ef5830c` 的实测行号。

### 3.2 PR #322 已收口、不在本包重复改造的部分

`detail_page_url_invalid`（同一签名 URL 被最终页面否定后不再派发）已经实现并有隔离 PostgreSQL 覆盖。本包 S1b 的「缺输入止转」处理的是**另一个**问题：地址从未验证过就被派发的工单，两者不重叠，也不互相放宽。

## 4. 修前反例（S0 退出条件）

交付包要求先构造六个修前反例，记录真实调用链，并逐条标注「已复现／已被新版本修复／未证实」。当前状态与证据：

| # | 反例 | 状态 | 调用链证据（`1ef5830c`） |
|---|---|---|---|
| E1 | 缺地址反复入队 | **已复现；已修复；隔离 PostgreSQL 证明通过** | 修前（`1ef5830c`）：`dispatch.rs` pending 分支在取不到签名 locator 时调用 `record_recoverable_dispatch_failure_in_transaction(…, "execution_locator_unavailable")`（`dispatch.rs:1411`）→ 释放租约、按 60/120/240/480/900 秒阶梯回队（`dispatch.rs:1235`）。`in_progress` 分支直接返回 `ExecutionLocatorUnavailable` 而不释放（`dispatch.rs:1633`）。**运行级复现**：反例 `a_missing_locator_stops_only_its_own_material_without_requeueing_forever` 在修前代码上失败——同一张工单里地址完好的那一篇**没被派出去**，派发给出的是 `ExecutionLocatorUnavailable { reason: "这篇作品当前没有带 xsec_token 的已接纳发现链接；已释放许可并进入冷却重试。" }`，即缺地址的一篇把整张工单拖回了冷却。修后通过；另有整张租约都缺输入的一例（`a_lease_whose_members_all_lack_execution_input_ends_as_a_stop_not_a_completion`）证明它停在 `input_blocked`（释放原因同为 `input_blocked`、工单 `cancelled`、零 Attempt 与零 Package），且之后连续 100 轮调度不再新建租约、不再新增停止事实 |
| E2 | 同一对象跨新工单重复 `page_read_failed` | **已复现；已修复；隔离 PostgreSQL 证明通过（S2）** | 修前（`1ef5830c`）：`page_read_failure_count_for_detail_in_transaction` 的 WHERE 以 `lease.work_order_ref = $1` 为界（`dispatch.rs:1157`）→ 预算按工单而非按需求范围累计，每建一张新工单都从零开始。修后：预算落在资格台账的当前行上，`detail_read_failure_budget_follows_the_requirement_scope_across_new_work_orders` 证明换工单不清零（三张新工单、同一份输入，第三次失败后停止），`the_same_content_under_two_targets_keeps_two_independent_detail_budgets` 证明两个目标各记各的，`legacy_unfrozen_orders_record_unverified_prior_failures_without_spending_the_budget` 证明台账建立前的旧失败不进预算也不封禁，`a_late_detail_read_failure_never_spends_the_budget_of_an_already_accepted_material` 证明迟到失败不推翻已接纳的详情，`duplicate_detail_failure_reports_count_once_and_keep_one_current_row` 证明同一事件并发重报只记一次。口径见 §5.3 |
| E3 | API 已接纳但响应丢失后，插件重新 `startAttempt` 被拒 | **已复现；已修复；测试通过** | 修前：`start_producer_attempt` 先做活租约原子校验、通过后才查已有 Attempt（`producer_runtime.rs:801` → `:839`），所以租约一旦关闭（**投递自己正常完成也会关**），已合法提交的包在重放时拿不到幂等回执；插件侧 `flushLocalOutboxOnce` 把该结果按 4xx 判为终态并 `outbox.terminal(...)`。修前复现 `a_lost_submission_response_still_replays_after_its_lease_closed` 失败、修后通过；插件侧 `a closed claim still asks the submission route…` 同法验证 |
| E4 | 读取完成而 API 离线 | **代码级已定位；运行级复现待建（T01/T03）** | 本地 outbox 独立于租约，`queueCapturePackage` 先落 Dexie 再异步 flush（`background.js:240`）；flush 走 createTask→startAttempt→submit。**详情会话的四条冻结通道**在导航前已取得服务端身份（S1a 第二步），因此租约关闭后仍能如实按原身份开始并提交（隔离 PostgreSQL 已证）；非详情任务（如首页发现）不在本次机制内，仍落入 E3 的终态路径 |
| E5 | 详情 `title` 为 NULL | **代码级已定位；运行级复现待建（T19）** | 完成判据需要按 `target_catalog.rs` / `archive_completeness.rs` 的现有读取逐点核实后再改；本包 S3 只把「详情已取得」的唯一判据改为「存在合格详情材料及其来源」 |
| E6 | session 标记落后于 Receipt | **代码级已定位；运行级复现待建（T22/T27）** | `completed` 工单下的 `collection_detail_page_session.delivery_pending` 与已存在的 Receipt 无对账入口；S3 增加以冻结通道回执/明确终止结果归并的终结判断 |

「代码级已定位」≠「已复现」。交付包 S0 的退出条件是每个断言落到三类标签之一，因此 E1–E6 在对应的 T 编号测试建起来并**修前失败**后，才升级为「已复现」；测试先证明修前失败，再实施修复。

## 5. 问题—改动—测试对照表

本表是交付包 S0 要求建立的对照表，随实现推进逐行更新（状态列：待实施／已实施／测试通过／收益未证实不改）。

| 问题 | 改动 | 主要落点 | 验收编号 | 状态 |
|---|---|---|---|---|
| E3 已接纳包因租约关闭无法重放 | 身份匹配的已有 Attempt 先返回幂等重放；新 Attempt 仍要求活租约原子校验 | `producer_runtime.rs::start_producer_attempt`、`live_scheduled_claim_exists` | T01–T04, T10, T24 | **已实施；T02 与 T24 相邻路径测试通过**（T01/T03/T04 待浏览器生命周期层证明） |
| 插件把可恢复的 4xx 一律判为终态 | 按机器码分类：仅「活权不在了」继续走投递路由恢复原 Receipt，其余 4xx 保持终态 | `adapter.js::isRecoverableDeliveryRegistrationRefusal`、`background.js` | T06–T08, T31 | **已实施；T31 能力协商测试通过**（服务端只在 `/health` 广告后接受该字段；请求了却无合规回执则不开页） |
| `LOST_AUTHORITY` 回执在页面上显示英文机器码 | 按 S3 状态词典给出「材料已保存，原执行权已失效」 | `collection_tasks_view.rs` | T24, T27 | **已实施（S3c）**：任务行与 Receipt 面板都出中文，机器码降为 Mono 旁注（`data-task-effect-code`／`data-task-inspector-effect-code`，没有回执时留空而不是落成 `UNKNOWN`）；页面单测 `a_receipt_with_lost_authority_is_not_rendered_as_a_plain_success`。UI 变更清单在动页面之前已写入（§9.1–§9.8） |
| 采集完成才登记 Attempt，断网后新包无服务端身份 | 导航前在授权事务内登记全部冻结通道的稳定 Attempt 身份，按 capability 协商启用 | `0096`、`dispatch.rs::grant_detail_page_session_with_lane_deliveries`、`producer_runtime.rs::prepared_lane_delivery_exists`、`apps/api/src/local_web.rs`、`adapter.js`/`detailPageSessionStore.js`/`background.js` | T05, T09, T31 | **已实施；T31 与 T05 的隔离 PostgreSQL + 插件测试通过**（T09 逐通道如实显示待 S3；变异验证见 §8） |
| E1 缺地址反复入队 | 输入资格前移；新工单冻结执行输入引用；原范围事务化停止 | `keyword_archive_detail.rs` 候选/批量判据、`dispatch.rs` pending 分支、`work_order_lease.rs` 剩余工作、新 `execution_input_eligibility.rs` | T11–T15, T18 | **已实施；隔离 PostgreSQL 证明通过**（S2 补齐了渐进建档这一侧的候选判据：「此刻解析得出地址」与「停过且输入未变」两条入口判据现在是两个面共用的同一句，见 §5.1；T11/T12/T15 有测试；T13 的两处语义见 §5.1；**T18 单独立了测试** `an_in_progress_replay_with_a_dead_locator_keeps_the_attempt_and_never_renavigates`——它守的是**修前就已正确**的那条路（`in_progress` 重放不释放、不销毁），本次改造没有碰它，因此这是回归守卫而不是修复本身；变异验证见 §8；**T28 的三个例子已齐**：①「明确页面不可用」新增 `an_explicitly_unavailable_page_stops_only_its_lanes_without_fabricating_deletion`（原先没有任何用例驱动 `page_unavailable` 这个码），②「暂时超时」复用 `failed_browser_start_is_audited_then_returns_work_order_to_shared_queue`，③「选择器失配」复用 `unavailable_detail_is_audited_without_blocking_later_materials`（详情通道落在 `detail_page_session_recovery_required`）与非详情通道的 `repeated_detail_read_failure_becomes_blocked_without_stalling_later_materials`；三例是三个不同的失败码（处置：①③停止、②有界退避；详情通道的失配也停止，因为导航许可已消费而重开比停下更危险，但原因分开记），都不伪造删除/耗尽，变异验证见 §8。T14 由 S2 的 `detail_read_failure_budget_follows_the_requirement_scope_across_new_work_orders` 覆盖，见 §5.3） |
| E2 失败预算随新工单清零 | 预算落在资格台账的**当前行**上：按需求范围跨工单累计、按事件去重、退避阶梯 60/120/240/480/900，第三次用尽即停止自动重试；建单时冻结输入（`execution_input_frozen_at`），台账建立前的旧失败只记待核实不进预算 | `execution_input_eligibility.rs`（`record_detail_page_read_failure_in_transaction`／`budget_blocks_new_work_predicate`／`budget_exhausted_object_refs_in_transaction`）、`dispatch.rs`、`work_order_lease.rs`、`acquisition_chain.rs`（建单冻时刻 + 渐进候选）、`keyword_archive_detail.rs`（四个入口） | T12–T18, T23 | **已实施；隔离 PostgreSQL 证明通过**（口径与测试见 §5.3；`0097` 在 S2 就地追加两列——它从未应用到任何共享库、分支未合并，实施前仍须按 §3 复核编号） |
| E5 title 非空被当作「详情已取得」 | 统一材料完成判据 = 合格详情材料及其来源；标题缺失是字段覆盖度 | 新 `qualified_detail.rs`（全库一处定义）、`target_catalog.rs`、`archive_completeness.rs`、`archive_ledger.rs`、`keyword_archive_detail.rs`、`acquisition_chain.rs`、`execution_input_eligibility.rs`、`target_drawer.rs` | T19–T21, T23 | **已实施（S3a）；隔离 PostgreSQL 20 目标全套 232 passed / 0 failed**（判据收成一处：目录行、覆盖统计、待补齐、关键词建档候选与缺口计数共用；详情已取到而平台没给标题时说「标题未收录」而不是「标题待取得」；写路径上两处更宽的守卫〔退役确认、材料补采〕有意保留原判据——它们问的是「有没有任何一行材料」，方向是把关更严，见 §9.8） |
| E1/E3 的展示面：欠详情的作品只说「待采集」 | 作品行欠详情时说出**欠的原因**（等新输入 / 还在退避 / 自动重试已停止），判据只从 `0097` 台账读一处 | `execution_input_eligibility.rs::read_material_execution_states`、`target_catalog.rs`、`target_drawer.rs` | T20, T21, T23 | **已实施（S3b）；两侧隔离证明通过**（判定规则与「看板为什么没排」共用同一句 `current_row_pauses_work_sql`；候选行压过停止行——停止是历史，不是永久封禁；变异验证见 §8） |
| E6 session 终结与 Receipt 不对账 | 统一当前状态 DTO（原因/来源/最后确认时间/允许动作）+ 会话终结归并 | `collection_tasks_view.rs`、`collection_task_read.rs`、`runtime_capacity.rs`、`queue_position.rs` | T22, T27 | **已实施（S3c）**：新增只读交付对账投影 `read_detail_delivery_reconciliation`（`DetailDeliveryReconciliation` + `DeliveryConclusion{Delivered, AwaitingDelivery, RecoveryUnverified, Closed}`），结论由冻结通道的回执数出来、不由会话自己写的 `state` 出来；`/collection/tasks` 新增「交付对账」小节与两个读数；终结压过一切且晚到 progress 在写入侧即被拒（`SessionNotHeld`）。`runtime_capacity.rs`／`queue_position.rs` **未改**：本卡不动工位与排队口径，交付结论不反向写进它们。证据见 §9.9；变异验证见 §8 |
| 故障无法按一条链路串起来 | 结构化事件字段 + 复用现有巡检账本的统一 tick 关联 | 采集/worker 入口、`collection_scheduler_run` | T29–T32 | 待实施 |
| 性能候选缺少证据 | 建 SQL 次数/耗时/计划基线；无稳定收益则明确「收益未证实，不改」 | 见交付包 S5 表 | T33–T34 | 待实施 |

### 5.1 输入资格台账的两条键（T13 的实现口径）

交付包 `01-目标合同` 对同一张台账给了三条必须同时成立的约束，初版 0097 把前两条读成了一条键，因而与第三条冲突：

| 包内位置 | 约束 |
|---|---|
| 01:50 | 建表建议的唯一范围**含** `execution_input_fingerprint` |
| 01:52 | 失败预算的键**不含**指纹——「不能靠刷新签名 token 清零页面失败次数」 |
| 01:54 | `retry_epoch` 不能由新建 WorkOrder、**地址变化**或普通版本升级自动增加；且「不能因输入指纹不同就并行派发同一缺口」 |

初版把后继行写成 `retry_epoch + 1`，于是地址一变就开了一个新的预算世代：指纹虽然不在预算键里，`retry_epoch` 在——「换个签名 token 重置失败次数」照样能达成。改为两条键分工：

- **身份键**（含指纹，`NULLS NOT DISTINCT`）：管「同一件事没有被记两遍」。「当初一条地址都没有」与「后来有了地址」是两条各自成立的事实，都留下；停止行沿用 `NULLS NOT DISTINCT` 才不会每轮攒出一条重复的「当时没有地址」。
- **当前键**（不含指纹，部分唯一索引，只覆盖非停止行）：管「此刻谁说了算」。输入变了是**就地更新当前行的输入列**，不派生第二条可执行资格；`state` 与 `deduplicated_failure_count` 不在更新列里，所以输入变化既不会把 `budget_exhausted` 悄悄改回 `eligible`，也不会清零已累计的失败次数。S2 的预算读写因此落在**当前行**上，不是对多行求和。

**T13 的两处语义由此定死**：① 重复发现同一地址、只改时间戳 → 指纹不变，不写任何新行；② 重新签发的 token 与原 token 不同 → 指纹不同，算「输入变了」，但 epoch 不动、预算不清零。第 ② 条与 01:44「不得推断作品永久删除，更不得全局封禁该 content」一致：换链接是恢复路径，不是免责通道。

S1b 留给 S2 的两条边界，S2 的处理与结论：

- **两条键并存不是欠账，是设计。** 输入在「当前行已是可执行」的状态下消失时，停止行与那条当前行并存：能否执行由执行判据（`blocked_object_refs_in_transaction` / `unchanged_input_block_predicate`）回答，当前行只承载预算与审计，从不单独授权执行。S2 复核后维持——把它们合成一条，等于要求「一次停止」同时表达「此刻不能跑」，而停止行恰恰可能是一条不再成立的历史。
- **候选面：两个面现在问同一句话。** 渐进建档的候选查询（`acquisition_chain.rs`）在 S2 补上了关键词侧早已有的两条入口判据——「此刻解析得出执行地址」与「已经因此停过、且输入没变」——再加上预算判据。此前它只按「有没有详情／在不在途／有没有被判读不出来」直读，一条只有 `input_blocked` 通道的作品会被反复挑中：每轮新建一张注定被停在成员展开处的工单（**不派发、不开页**，只是白占一张工单和一次调度），随后终结为 `cancelled`。新用例 `a_directory_member_without_a_signed_link_never_enters_a_doomed_work_order` 先在修前失败（`queued: [目标]`），修后通过；变异记录见 §8。
  - 这条用例同时暴露了夹具的不诚实：`bounded_root_submission` 的 200 张发现卡**没有签名链接**，而真实插件上报的卡片带它。判据补上之后，三张用例（批量滚动、并发继续、被封详情计数）当场变红——红的原因是夹具少了生产本来就有的字段，不是新判据错。夹具已按真实形状补上链接；这也意味着「渐进目录的成员天生就能解析出地址」现在是测试里的事实，而不是假设。
- **反复建单/终结是否消耗授权配额：不消耗。** 授权额度是**一次请求的上界**（`max_targets` / `max_works_per_target` 只判定「这一次的请求有没有超出授权」），不是累计计数器；真正的资源消耗发生在原子领取处（工位、账号、风险、额度），而被停的工单走不到那里。被消耗的只有队列轮次——这正是 S2 要止住的浪费，不是额外支出。

### 5.2 明确不做（交付包非目标）

不合并三套 Dexie、不合并两域材料表、不为 `materialization_ref` 主键查询另造 blob 索引、不把 `include!` 改语法当作修复、不定期 `pg_stat_reset`、不默认开启全量 SQL 日志、不装 `pg_stat_statements`、不重做五张大卡片布局。

### 5.3 跨工单失败预算的实现口径（T14–T17、T23）

**记在哪。** 预算记在资格台账的**当前行**上（`state <> 'input_blocked'` 那一条，键不含 locator 指纹）：`deduplicated_failure_count` 是次数，`next_retry_at` 是下一次可试的时刻，`state='budget_exhausted'` 是停止，`reason_code='page_read_budget_exhausted'` 与 `policy_version='page_read_budget_v1'` 说明「为什么停」和「按哪套判据停的」。换工单不是新事实，换地址也不是——只有受控重新准入才开启新的 `retry_epoch`（**该通道尚未实现**，见下）。

**怎么累计。** 每次 `page_read_failed` 让次数 +1，退避按阶梯 60/120/240/480/900 秒写在 `next_retry_at` 上；`DETAIL_PAGE_READ_BUDGET = 3` 用尽即置 `budget_exhausted`，该成员不再自动重试。停止判据取**两者中更严**的一条：`max(本工单内计数, 台账计数) ≥ 3`，或台账已 `exhausted`——新规则只能收紧，不能放松已上线的「同一工单三次」。

**去重。** 按 `last_event_ref`：同一个失败事件重报只算一次，并把当前落点如实回放（`Replay`）；并发的两次重报由数据库唯一性与行锁收口，一次记账、一次回放（`duplicate_detail_failure_reports_count_once_and_keep_one_current_row` 用 `tokio::join!` 真并发跑出这个结果）。

**哪些失败不算数。** ①**材料已经有合格详情**：迟到的失败不回退材料事实，也不推进预算——预算问的是「还要不要再试」，这里已经没有要补的东西（`a_late_detail_read_failure_never_spends_the_budget_of_an_already_accepted_material`）。②**台账建立之前的旧工单**：`collection_work_order.execution_input_frozen_at` 为空的工单没有冻过输入，它的失败只证明「试过」、不证明「试的是同一份输入」，因此只记进 `prior_failures_unverified` 并且**不进预算**（返回 `None`，调用方退回「同工单内三次」）。凭一批无法对齐输入的历史失败把一篇作品按 content ID 封禁，正是合同禁止的那件事（`legacy_unfrozen_orders_record_unverified_prior_failures_without_spending_the_budget`）。

**冻结点。** `write_work_order` 在**建单事务里**写下 `execution_input_frozen_at`。判据不能反过来由台账推：一次停止也会为某个对象写下 `input_source_status='frozen'`，于是「这个对象停过」会被读成「这张工单冻过输入」——两件事方向相反。

**两个面共用同一句话。** 关键词侧的四个入口（`next_evidence_detail_batch`、`keyword_targets_pending_detail`、`keyword_target_has_pending_detail_in`、`next_detail_batch`）与渐进建档的候选，都带同一条预算判据：**用尽（停止）或还在退避里（等一下）都不再自动排新工作**。少了它，同一个缺口每被新建一张工单就重新起一轮——共享库里同一篇作品进过 21、13、20、18 张有匹配任务的工单，每张都从零开始数。

**停止之后怎么收尾。** 成员展开时，预算用尽的成员与「缺输入停过」的成员一起被摘掉（`budget_exhausted_object_refs_in_transaction`）；`remaining_steps_for_work_order` 把两类都算进 `stopped_members`，剩下的任务为空时工单如实终结为 `cancelled`，回绝码 `work_order_only_stopped_members_remain`（S1b 曾叫 `work_order_only_input_blocked_members_remain`，因该版本从未发布，直接改名而不是留别名）。两类停止合成一句回绝，是因为**对这张工单的处置相同**（都不再自动重试）；它们彼此的区别——一个等更好的输入自己回来，一个要另一次明确的决定——记在台账的状态里。

**渐进建档的空候选集有三种解释**（此前只有一种，且错的那种会收口一份没齐的基线）：在途 → `detail_batch_in_flight`；还欠着但排不进去 → `detail_gap_not_schedulable`（**不收口**，根保持 `active`，等输入变了再接着跑）；确实齐了 → 收口并记 `archive_baseline_complete`。空候选集不是「完成」的证据，只是「此刻没有可排的活」。

**如实记下的边界与未做项：**

- **受控重新准入没有实现。** 预算用尽是一次**停止**，恢复它按合同要另一次明确的决定（新的 `retry_epoch`）。台账已经能承载这个世代（`retry_epoch` 列与身份键都在），但没有任何入口会去写它——恢复通道属于产品决定，S3 的状态读取会先把「预算用尽」作为一种可见状态呈现出来，再由 Mog 决定恢复动作长什么样。在那之前，预算用尽的成员**不会被任何自动路径复活**。
- **同一篇作品属于两个目标时，各有各的预算**（键含 `target_ref`），一个目标的失败不牵连另一个（`the_same_content_under_two_targets_keeps_two_independent_detail_budgets`）。
- **材料已有合格详情时，新工单里的详情通道仍可被派发**：那条路径由既有的通道合同约束，不归预算管；它的失败按上面的①处理，不会把任何东西推向停止。

## 6. 实施顺序与依赖

```text
S0 基线 + 修前反例
   ↓
S1a 交付恢复（不依赖新 schema，先行）
   ↓
S1b 缺输入止转 ── 出口条件依赖 S2 的资格台账，两者连续实施
   ↓
S2 跨工单预算与调度一致性（预算落在 S1b 已建好的 0097 台账上）
   ↓
S3 统一状态读取（消费 S2 的资格事实）── 变更清单见 §9，实施前先写入
   ↓
S4 运行诊断与就绪 → S5 有证据才优化 → S6 集成与历史处置预览
```

## 7. 验证分层与声明纪律

- 单元测试证明局部规则；隔离 PostgreSQL 证明事务与并发；浏览器生命周期证明持久化恢复；各层不可互相代替。
- `fake-indexeddb` 只覆盖部分证明，浏览器持久化必须另有真实 worker 生命周期中断证据。
- 未执行的验收行写 `NOT VERIFIED`，不给混合完成百分比。
- 共享库应用、runtime、插件与真实平台操作分别记录实际授权与结果。

## 8. 反例（变异）验证记录

新断言必须证明能测出它声称守住的缺陷，而不是恰好与被测代码同向。

| 被验证的断言 | 变异（临时移除） | 观察到的失败 |
|---|---|---|
| 导航前登记的交付身份在租约关闭后仍可用（隔离 PostgreSQL `navigation_time_lane_identities_outlive_a_closed_lease_without_impersonating_execution`） | 从 `start_producer_attempt` 的 `None` 分支去掉 `prepared_lane_delivery_exists` 判据 | 该测试在 `a delivery identity registered before navigation survives its closed lease` 处失败（`ScheduledTaskNotClaimed` 路径） |
| 广告了能力却拿不到合规回执时必须不开页（插件 `an announced handshake is requested, and a missing receipt never authorizes a page open`） | 把 adapter 的 `grant_lane_preparation_missing` 分支置为不可达 | 该测试失败 |
| 同一条地址再准入不得再多一条当前资格（隔离 PostgreSQL `a_new_signed_address_after_a_stop_opens_a_successor_eligibility_through_admission`） | 从后继写入里去掉 `ON CONFLICT (…) WHERE state <> 'input_blocked' DO UPDATE …` | 该用例在 `a second advance still answers` 处失败：`23505 duplicate key … collection_execution_input_eligibility_input_identity`，准入事务整笔回滚 |
| 停止事实每通道只留一条（同一用例末段） | 去掉身份键的 `NULLS NOT DISTINCT` | **没有变红**。「同一范围再停一次」根本走不到写库：发租时 `blocked_object_refs_in_transaction` 已把输入未变的停止成员排除在任务之外，同一篇不会再产生 `pending` 任务去触发第二次停止。因此这一条断言**不构成对 `NULLS NOT DISTINCT` 的证明**，它只守住「四通道各一条、不多不少」；该键如实降级为守卫，理由与限制已写进 `0097` 的注释，不改实现 |
| `in_progress` 重放遇死地址时「就地保住现场、什么都不写」（T18，`an_in_progress_replay_with_a_dead_locator_keeps_the_attempt_and_never_renavigates`） | 在 `dispatch.rs` 的 `in_progress` 分支把「不释放、直接作答」换成 `stop_member_for_missing_execution_input(…)`——即把**已经交出去的任务**按「从未开始」处理 | 该测试变红，且失败点是 `execution_scene` 的全量比对而不是某个单点断言：`input_stop_events` 0→1、`eligibility_rows` 0→4（一条已开始的四个通道全被记成「缺输入」），`claimed_at`/`claimed_by`/租约/工单状态不变。这说明变异造成的破坏**正是台账与停止事实**，也说明只有「整场执行事实一起比」才抓得住它——单看任务状态会以为一切正常 |
| 「明确页面不可用」必须停在 `unavailable` 而不是退避（T28①，`an_explicitly_unavailable_page_stops_only_its_lanes_without_fabricating_deletion`） | 从 `dispatch.rs` 的判定表里拿掉 `(DispatchFailureCode::PageUnavailable, Some(content_external_id))` 那个分支，只留 `DetailPageSessionRecoveryRequired`——即把生产方确认的不可用降级成「读了一次没读成，退避再来」 | 该测试在派发结果处变红：`left: Requeued { retry_after_seconds: 60 } != right: Unavailable`。这正说明它钉住的是分类本身（停止 vs 有界退避），不是同义的重复断言 |

| 空候选集里「还欠着详情」必须压过「完成」（隔离 PostgreSQL `a_detail_that_spent_its_page_read_budget_keeps_the_baseline_open`） | 把新分支改成 `if false && missing > 0`——即回到「挑不出候选就收口」 | 该测试在 `!tick.skipped.iter().any(… "archive_baseline_complete")` 处变红（「目录里还欠着详情，这份基线不算完成」）。修前形态正是这份代码：预算用尽的成员让候选集为空，根被标成完成，而欠着的那条详情永远不会再有下一次机会 |
| 没有执行地址的目录成员必须进不了候选（隔离 PostgreSQL `a_directory_member_without_a_signed_link_never_enters_a_doomed_work_order`） | 把候选里的 `/*executable*/` 换成常量 `TRUE` | 该测试在 `!tick.queued.contains(&target_ref)` 处变红：`queued: [目标]`——一张注定被停在成员展开处的工单被建了出来，下一轮还会再来一张 |

| 「这一篇此刻欠的是什么」在作品行上必须分得开（S3b 两条证据用例：`a_detail_that_spent_its_page_read_budget_keeps_the_baseline_open`、`a_new_signed_address_after_a_stop_opens_a_successor_eligibility_through_admission`） | 在 `read_material_execution_states` 里把三种落点全部折叠成 `Executable`（`state == "…"` 两个分支与 `pauses_new_work` 分支各加 `&& false`） | 两条用例都在各自的第一处新断言变红：`left: (Pending, Some(Executable)) != right: (Pending, Some(InputBlocked))`（关键词侧「缺输入停下的作品，行上说的是「输入不可执行」」）与 `left: (Pending, Some(Executable)) != right: (Pending, Some(RetryPending))`（建档侧「退避中的作品，行上说的是「延迟重试」」） |
| 「预算用尽」不能显示成「还会再来」（同上建档用例第三处断言） | 只把 `state == "budget_exhausted"` 那一支置为不可达（其余分支保持原样） | 用例在「预算用尽的作品，行上说的是「自动重试已停止」」处变红，且左边正是缺陷的形状：`left: (Pending, Some(RetryPending)) != right: (Pending, Some(BudgetExhausted))`——停止被说成「再等等就好」 |
| 作品行把「自动重试已停止」显示成「延迟重试」（页面层，`a_work_owed_detail_says_which_reason_it_is_waiting_on`） | 在 `target_drawer.rs` 的展示映射里把「自动重试已停止／`budget_exhausted`」那一支改成「延迟重试／`retry_pending`」（文字与 `data-state` 一起改，模拟真实的错映射） | 该用例在 `data-state="budget_exhausted">自动重试已停止` 处变红。它钉住的是**展示映射**这一层：领域判据再对，页面仍可以把停止说成重试 |
| 交付结论按**回执数**、不按通道行数（隔离 PostgreSQL `delivery_reconciliation_counts_receipts_not_the_session_marker`，T22） | 把 `DETAIL_DELIVERY_RECONCILIATION_SQL` 的 `count(receipt.receipt_ref) AS delivered_lanes` 换成 `count(lane.attempt_id)`——即「登记了通道」当成「通道送到」 | 该用例在刚导航完那一步变红：`left: Delivered != right: AwaitingDelivery`。刚提交导航、一条回执都还没有的会话被读成「已交付」，正是「没有 Receipt 不等于数据丢失」的反面 |
| 已终结的会话不再被任何晚到事实降级（同上，T27） | 把 `delivery_conclusion` 的终态分支从**第一条**挪到**最后一条**（先按通道数判 `Delivered`／`AwaitingDelivery`） | 该用例在 `Stopped{risk_stop}` 之后变红：`left: Delivered != right: Closed`。四通道回执齐全的已停止会话被改写成「已交付」——终结语义被通道计数盖过，而这正是读侧必须与写入侧（`SessionNotHeld`）一致的那条 |

十三次变异均已还原（前两次 S1a、四次 S1b、两次 S2、三次 S3、两次 S3c）。S3c 的两次变异都从 `crates/evidence/src/collection_task_read.rs` 还原：两处都是逐字对照原句反向替换（终态分支回到第一条、计数回到 `receipt.receipt_ref`），还原后 `grep -n "count(lane.attempt_id)"` 为 0 命中、匹配臂顺序与原句一致，两条 S3c 用例重新变绿。S3 的三次变异分别从 `execution_input_eligibility.rs`（两次）与 `target_drawer.rs`（一次）还原：还原后逐一 `grep` 变异标记（`false &&`／`if false`／写错的那两支文案）为 0 命中，`git diff` 回到变异前的内容，两条证据用例与页面用例重新变绿。S1b 的还原用还原前快照逐字节核对：`execution_input_eligibility.rs` sha256 `6408147952b274a9a7ae8f180fb53eaaf37362383177fa30355f31377fba43ab`、`0097` 最终 sha256 `83a8a99362df528217ac7473c102ca8c42f6f8beb3b76b9e195f95451ddac3b7`（已同步进 `material_fixture` 与 `full_schema_fixture` 两处账本）、`dispatch.rs` 变异前后同为 sha256 `5b54d0395180ea385150dff4b60fa608083ff334319a21ecb57d7b2dbab1c6b0`。T28① 的变异同样从 `dispatch.rs` 还原，之后该文件 sha256 仍为 `5b54d039…`（两次变异都是逐字节还原）。S2 的两次变异从 `acquisition_chain.rs` 还原：变异前快照存于 `/tmp/acquisition_chain_s2_pre_mutation.rs`，还原后 `grep -c MUTATION` = 0 且 sha256 与变异前同为 `3e4f14e56f7b5b2e08152dbb96e009bee48e701911d6f08a2fc770b4e8fedd43`（逐字节）。各阶段还原后：S1b 的 `keyword_archive_postgres` **16 passed / 0 failed**、`collection_dispatch_sequence_postgres` **27 passed / 0 failed**；S2 的 `collection_dispatch_sequence_postgres` **32 passed / 0 failed**、`observation_target_dossier_postgres` **23 passed / 0 failed**，`cargo check --workspace --all-targets --locked` 通过。源码冻结后的干净全套（`./scripts/test-local-001-discovery-postgres.sh`，20 目标）**231 passed / 0 failed**，容器/卷/库自建自清。

S3c 两次变异还原后：`collection_dispatch_sequence_postgres` **34 passed / 0 failed**（新增两条）、`content_reobservation_postgres` **4 passed / 0 failed**、`linggan-api` 二进制内 `--ignored` **23 passed / 0 failed**；`cargo check --workspace --all-targets --locked` 通过。S3c 源码冻结后的干净全套（20 目标）**234 passed / 0 failed**，容器/卷/库自建自清——这一跑同时补上了 S3b 记录里被宿主磁盘写满打断的那次重跑（当时第 20 个目标 7 例 `57P03 in recovery mode` 未计入结论）。

## 9. S3 变更清单（统一状态读取与页面动作）

> 依 `docs/design/templates/ui-change-manifest-form.md` 填写。清单写在实施计划里，不另建 `docs/design/` 下的零散文档。
> 状态: 活跃计划 · 实施前写入（本节先于 S3 代码存在）。

### 9.1 事项

- **Issue / SCOPE**：COLLECTION-UPGRADE-001 · S3（交付包 `02-实施步骤.md` §S3；验收行 T19–T23、T27，以及 `01-目标合同.md` §4.3 的 `LOST_AUTHORITY` 文案）。
- **Agent 与 worktree**：`fix/collection-upgrade-001` @ `.worktrees/collection-upgrade-001`。
- **目标**：让「详情已取得」「这一篇还能不能执行」「这一批包交付完了没有」三类事实各自只有一处领域判据，并让页面只说这三类事实各自能证明的话。不同页面可以用不同中文，但同一个原因语义在任何页面上都指向同一条事实。
- **用户可见结果**：
  1. 作品行不再把「详情已取到、只是标题为空」显示成还欠一篇详情；
  2. 欠详情的作品能说出它欠的原因（等新输入 / 自动重试已停止 / 还在退避），而不是一律「待采集」；
  3. 每个已授权详情页会话能说出它的交付结论（已交付 / 待交付 / 恢复待核实 / 已终结）与最后观察时间，而不是让 `delivery_pending` 直接冒充待交付；
  4. 失去执行权的晚到包显示「材料已保存，原执行权已失效」，不再显示英文机器码。
- **明确非目标**（本卡不做，也不得顺手做）：
  - 不新增页面、导航或状态分类卡片；四个新显示状态全部落在既有表面的既有列/既有小节里；
  - 不实现「恢复提交」「受控重新准入」「确认为已失效」的**动作按钮**：本卡只读，动作入口各自的合同分别归 S1a、S2 与 #158 的退役路径（见 9.6 停止条件）；
  - 不改接纳、权限、额度、调度与清理规则；不改跨行业评论准入；
  - 不改 `runtime-main`、共享库、插件版本或 :3000 运行态。

### 9.2 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / `docs/current-state.md` | 权威当前 | 现状里 `详情读取受阻` 已是既有显示状态（`COLLECTION-DOSSIER-RELIABILITY-002`）；五个采集子面已由 `COLLECTION-FIVE-PAGE-V4-UI-001` 合入 | 2026-09-21 读 |
| `docs/agents/ui-execution-contract.md` | 权威当前 | 四件套（表面地图/状态词典/依赖地图/验收矩阵）、变更分类、停止条件 | 2026-09-21 读 |
| `docs/design/README.md`、`docs/design/lids/README.md` | 权威当前 | LIDS 规则已采纳、运行时仍是 v2.0；不得以「和旁边一致」沿用旧口径 | 2026-09-21 读 |
| 产品页面文档 `docs/design/pages/collection-workspace-page.md` | 权威当前（PAGE-COLLECTION-001） | 五个子面的职责与「状态与诚实性」表；`UNKNOWN` 不得暗示为零 | 2026-09-21 读 |
| LIDS `language-policy.md`（LANG-05）、`data-boundaries.md`（LIDS-BOUND-001） | 权威当前 | 中文承载意义、机器码只作 Mono 旁注；字段缺失写明确中文，不留空、不编默认值 | 2026-09-21 读 |
| 数据/权限/行动合同 `01-目标合同.md` §4.3、§5、§6 | 交付包 | 九行展示状态与「最低事实条件」、`LOST_AUTHORITY` 文案、「详情已取得」的唯一判据、两域隔离 | 2026-09-21 读 |
| 当前代码/测试 | 真实运行事实 | 判据分叉点与两处终态守卫的位置（见 9.5）；`0097` 台账的两条键；`0004` 的 receipt/disposition 主键 | 2026-09-21 读 |

### 9.3 表面地图

| 表面 | 入口 | 本卡改动 |
|---|---|---|
| 观察目标 → 作品目录（抽屉） | `/collection/targets` 抽屉 · 作品列表 | 作品行「详情」列增加停止原因；标题占位区分「未收录」与「待取得」；四格摘要里「详情已取得」改由同一判据计数 |
| 观察目标 → 档案状态（列表） | `/collection/targets` 列表「详情进度」列 | 计数来源（`details_captured` / `pending_details`）改由同一判据产出；**列本身与写法不变** |
| 观察目标 → 待补齐 | 抽屉内「待补齐」区块（`read_blocked_materials`） | 判据统一（已取到详情的不再出现在待补齐） |
| 采集任务 | `/collection/tasks` | 任务行的状态词修复（`LOST_AUTHORITY`）；新增「交付对账」小节与读数 |
| 执行工位 | `/collection/runtime` | **不改**——工位、租约、心跳本就只属于这一面；交付结论不反向写进目标页 |
| 待处理 | `/collection/attention` | **不改**（读模型仍未接入，继续显示未知，不因本卡变成空） |

页头/导航/共享壳层：不改。

### 9.4 状态词典

| 展示含义 | 最低事实条件（来源） | 本卡之后的所在 | 本卡动作 |
|---|---|---|---|
| 等待执行 | 输入有效、未被阻断、预算允许、尚无执行权（`0097` 台账 + lease/task） | 采集任务 · 任务行「等待派发」 | 不变 |
| 延迟重试 | 明确可重试原因、未超限、有 `next_retry_at`（`0097` 台账） | 目标抽屉 · 作品行「延迟重试」 | **新增显示** |
| 已领取 / 正在采集 | 活租约 / 有效导航或执行进度证据 | 采集任务 · 任务行「执行中」 | 不变 |
| 待交付 | 有可对账的待交付依据、服务端尚无匹配 Receipt（会话 + 冻结通道） | 采集任务 · 交付对账「待交付」 | **新增显示** |
| 恢复待核实 | 有 `delivery_pending` 标记，但读不到该会话的冻结通道（缺 `lane_preparation`） | 采集任务 · 交付对账「恢复待核实」 | **新增显示** |
| 输入不可执行 | 必要输入缺失或该输入被停止（`0097` 台账 `input_blocked`） | 目标抽屉 · 作品行「输入不可执行」 | **新增显示** |
| 自动重试已停止 | 同一需求范围预算已用尽（`0097` 台账 `budget_exhausted`） | 目标抽屉 · 作品行「自动重试已停止」 | **新增显示** |
| 材料已保存 | 有 Receipt 且执行权仍在（`COMPLETED_LIVE_STEP`）/ 材料已保存但执行权已失效（`LOST_AUTHORITY`） | 采集任务 · 任务行与 Receipt 面板 | 文案修复（英文机器码 → 中文；机器码降为 Mono 旁注） |

**禁止互相替代的边界**（每一条都对应一个反例，写在 9.7）：

1. 「详情已取得」不得由 `title` 非空推断；标题缺失是**字段覆盖度**问题，不回写伪标题、不列为待采集（T19）。
2. 停止不是完成：预算用尽与输入缺失都不是「已补齐」。
3. 「从未开始」不是「试过没成功」：缺输入停下的成员不消耗页面失败预算，也不显示成读取失败。
4. 没有 Receipt 不等于数据丢失；`delivery_pending` 也不等于待交付——先按冻结通道回执归并（T22）。
5. `completed` 工单下的 `session.delivery_pending` 不直接计入待交付总数。
6. 租约失效不是正在执行；历史 `in_progress` 保持可查，但不计入当前执行（T21）。
7. 未知不等于 0：读不到就写「读不到 + 最后观察时间」，不写 0、不写「都补齐了」（LIDS-BOUND-001）。
8. 两域材料不互相顶替：跨行业样本不进入本领域资格读取，本领域细节也不计入跨行业计数（T20）。

### 9.5 依赖地图

| 依赖 | 归属 | 本卡的用法 |
|---|---|---|
| `collection_execution_input_eligibility`（`0097`） | S1b/S2 已交付 | **只读**当前非停止行（`state <> 'eligible'` 的部分）作为停止原因；不写、不新增列、不改索引 |
| `collection_detail_page_session` + `_lane_preparation`（`0090`/S1a） | 已交付 | **只读**会话状态、`last_progress_at`、冻结通道与其任务；不写终态（终态只由既有的完成/停止事件写） |
| `linggan_runtime_submission_receipt` / `_record_disposition`（`0004`） | 已交付 | 合格详情判据读它；`(package_ref, record_ordinal)` 是 disposition 主键、receipt 按 `attempt_id` 唯一 |
| `linggan_material_content_detail`（`0015`） | 已交付 | `title_state` 是字段覆盖度事实；本卡不新增列 |
| 冻结 Work / 控制面读取（`collection_control_surface_view`） | 既有 | 不合并、不改写；S5 的「两个 control view 的 SQL」候选项留到 S5 按证据决定 |
| 跨行业侧判据 `cross_industry_sample_facts` | 既有、已是单一定义 | 不合并两域：本卡只把**证据侧**的判据收成一处，跨行业侧继续用它自己那份 |
| 共享组件/样式 | LIDS Token | 只复用既有类；新增样式只用既有 Token，不写死颜色/字号/间距 |

**未纳入本卡的跨任务依赖**：作品退役动作（`collection_material_retirement` 的入口）仍在 #158 的路径上；本卡只读它的结论，不新增确认入口。

### 9.6 变更分类与影响边界

- **分类**：状态/语义（含展示）。四个新显示状态都由既有领域的既有事实直接读出，不新增权限、不改行动后果、不产生新的可点动作。
- **最高风险类别**：状态/语义——所以来源收据是「产品页面 + 数据合同同时说明状态来源、用户含义和禁止暗示」（§9.4 三条来源）。
- **是否存在 `DECISION_REQUIRED`**：
  - 「该不该有一个过期阈值把待交付归到恢复待核实」——**本卡不设阈值**。本卡只在读不到冻结通道时才说「恢复待核实」，并在两种状态下都显示最后观察时间；按时间本身推断「没采到」是被合同禁止的（§4.3）。阈值若要加，是一次产品决定。
  - 「受控重新准入的入口长什么样」——S2 已记为挂账，本卡只让「自动重试已停止」可见。
- **L1 / L2 / L3 与主 Pattern**：L2 工作台子面，沿用 Collection Control Pattern；不引入新 Pattern。
- **是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth**：触及 Data Truth（多列读同一判据），不触及 Token/Primitive/Scene/Motion；无新 CMP。
- **禁止修改的文件/能力**：接纳与 disposition 写入路径、`dispatch.rs` 的终态判定、`0097` 台账写入、插件源码、`runtime-main`、共享库。
- **停止条件**：若统一判据会在生产路径上改变既有结论（而非今天的读法不一致），停止并只保留「已核对」的读侧改动，把分歧报给 Mog。

### 9.7 验收矩阵

| 层级 | 验收方法 | 未证明边界 |
|---|---|---|
| 任务可用 | 隔离 PostgreSQL 集成证明：四类停止原因各自可读、交付对账四态各自可读 | 真实平台下的实际会话序列 |
| 状态诚实 | 反例（变异）验证：每一条新断言都要证明它能测出它声称守住的缺陷（T19/T20/T21/T22/T23/T27 各一条） | 未跑到的浏览器实拍 |
| 视觉一致 | 既有 HTML 断言 + LANG-05 文案专查（中文承载意义、机器码只作旁注） | **NOT VERIFIED**：本机 `:3000` 运行的是 `runtime-main`，不是本分支；未获授权切换运行态，因此没有本卡的浏览器实拍 |
| 真实后果 | 本卡不含动作按钮，不产生真实副作用；读路径不写库（隔离证明里以行数与状态比对确认） | 恢复提交、受控重新准入与退役动作仍待各自授权 |

### 9.8 交接

- **修改文件**：见各次提交的 `git show --stat`；本节的读法统一落在一个新模块上（`crates/evidence/src/` 下的单一判据模块），消费方为 `target_catalog`、`archive_ledger`、`archive_completeness`、`keyword_archive_detail`、`acquisition_chain`、`execution_input_eligibility`，页面侧为 `target_drawer`、`collection_tasks_view` 与其路由。
- **验证命令/走查**：`cargo check --workspace --all-targets --locked`；`./scripts/test-local-001-discovery-postgres.sh`（20 目标全套，容器/卷/库自建自清）。
- **规则或索引同步**：`docs/progress/2026-09.md` 记本次交付；`docs/design/pages/collection-workspace-page.md` 若与实现冲突则按治理流程裁定（本卡不新写页面规格）。
- **例外与替代**：无。写路径上两处「至少得有一行详情」的守卫（退役确认、材料补采）**有意保留**更宽的裸判据——它们问的是「有没有任何一行材料」，方向是把关更严，不并入展示判据（详见判据模块的模块注释）。
- **LIDS migration log / 预览同步**：不涉及 Token 或组件层变化，无需同步 migration log。
- **PR / reviewer / integration owner**：由 Mog 指定；本卡按交付包约定在整个交付包收尾时统一走一次独立复核。

### 9.9 实施状态（S3a / S3b / S3c 已实施）

**S3a · 判据收成一处（提交 `923341c`）**：新增 `crates/evidence/src/qualified_detail.rs`，把「这份内容此刻有没有一份合格详情材料」写成全库唯一一句（材料本体 + 已接纳来源 + 未被隔离）；目录行、覆盖统计、待补齐清单、关键词建档候选与缺口计数都从这一句读。`title_state` 退回**字段覆盖度**角色：详情已取到而平台没给标题时，作品行说「标题未收录」，不再说「标题待取得」。写路径上两处更宽的守卫（退役确认、材料补采）有意保持原判据——它们问的是「有没有任何一行材料」，方向是把关更严，不并入展示判据。

**S3b · 作品行说出欠的是什么（本步）**：`0097` 台账新增**只读**入口 `read_material_execution_states`（`execution_input_eligibility.rs`），给它自己的类型 `MaterialExecutionState{kind, reason_code, eligibility_ref, confirmed_at, retry_at}`——原因、来源引用、最后确认时间、下次重试时刻都原样带出，界面不自己拼第二套资格判据。`CatalogWork` 多一个字段 `execution_state`，由 `target_catalog.rs` 的三个读者贴上（本领域材料用 `own_domain`/`material_content`，跨行业样本用 `cross_industry`/`cross_industry_sample`，两侧取值在调用处明写）。作品行据此在既有的「详情」列里多说三句话：**延迟重试**（当前行还在退避里）、**输入不可执行**（只有停止行、没有更新的当前资格）、**自动重试已停止**（同一需求范围预算用尽）；没有台账行时仍旧「待采集」——**「从未开始」不编一句「试过没成」**。

三条判据边界（对应 §9.4 的 #2/#3/#8）：①「现在不排」只有一句话，写在 `current_row_pauses_work_sql` 上，看板与候选共用（此前是 `budget_blocks_new_work_predicate` 里内联的一份，字面相同但随时可以各自漂移）；②候选行压过停止行——平台后来给了新地址、准入另开了一条当前资格之后，这一篇读到的就是「可执行」，停止行只作历史（把停止行当永久封禁是另一种形态的同一缺陷）；③两域各读各的 `domain_scope`/`object_kind`，读取侧不加条件。

**证据**：`keyword_archive_postgres`（跨行业侧：缺输入停下 → 输入不可执行；新地址到达后 → 可执行）、`observation_target_dossier_postgres`（本领域侧：两次读失败后 → 延迟重试；第三次预算用尽后 → 自动重试已停止）、`target_inspector_performance_tests::a_work_owed_detail_says_which_reason_it_is_waiting_on`（页面层四条文案与 `data-state`）。三条断言都做了变异验证（§8）。**未证明**：`execution_state` 的另外三个字段（原因码、来源引用、最后确认时间、下次重试时刻）目前只被读出、尚未被任何界面消费——本卡只让「欠什么」可见，把「为什么、依据哪一行、什么时候确认的」留给需要它的下一个表面（见 §9.6 的 `DECISION_REQUIRED`：本卡不设过期阈值）。

**S3c · 已实施（本步）**：`/collection/tasks` 的两处改动（T22/T27，`01-目标合同.md` §4.3/§5）。

①**交付对账**读层新增 `crates/evidence/src/collection_task_read.rs::read_detail_delivery_reconciliation`，给它自己的类型 `DetailDeliveryReconciliation{session_ref, conclusion, state, stop_reason, platform, content_external_id, target_display_name, prepared_lanes, delivered_lanes, last_observed_at, closed_at}` 与四态枚举 `DeliveryConclusion{Delivered, AwaitingDelivery, RecoveryUnverified, Closed}`。**结论由冻结通道的回执数出来，不由会话自己写的 `state` 出来**：`finished`/`stopped` 一律 `Closed`（终结压过一切）；没有冻结通道 → `RecoveryUnverified`（读不到身份就是读不到，不推断「没采到」）；通道回执齐 → `Delivered`（所以 `delivery_pending` 不会永远挂在待交付里）；否则 `AwaitingDelivery`。进对账的只有**已消费导航的会话**，`authorized` 被排除——把还没打开页面的会话列进「待交付」，等于把「还没开始」说成「欠着几个包」。

②页面在既有「采集任务」表下方多一节 `<section class="c-task-delivery" data-delivery-reconciliation>`，行上带 `data-delivery-session` 与 `data-delivery-conclusion`；读数区（`.v7-kpi`）从 2 个变 4 个，新增「待交付」「恢复待核实」。三条诚实性边界落在这里：**读失败写 `UNKNOWN` 不写 `0`**（`delivery_section(None)` 明说「读不到不等于没有待交付，也不等于本地包丢了」）；**空集也要说清范围**（「最近 100 个已消费导航的详情页会话里没有可对账的行」，不触发任何平台访问）；**停止原因出中文、机器码只作 Mono 旁注**（`closed_reason_words` 五种原因各一句，未知码不编）。

③文案修复：`LOST_AUTHORITY` 从英文机器码改为「材料已保存，原执行权已失效」，「材料已保存，执行权仍在」对 `COMPLETED_LIVE_STEP`；任务行与 Receipt 面板各自新增 `data-task-effect-code`／`data-task-inspector-effect-code` 只承载机器码，且**没有码时留空**——不回落到那个假码 `UNKNOWN`（`collection_workspace.js` 用 `|| ""`）。

④**读层的一处设计更正（由证明证据逼出来的）**：第一版把 `cross_industry_sample` 写进 `LEFT JOIN`，隔离证明直接失败（`42P01 relation "cross_industry_sample" does not exist`）。这不是夹具问题——`0089` 为 `cross_industry_sample_ref` 写下的列注释点名「collection-control-only proof schema 不装这张来源表」，所以那一列故意不带外键，`dispatch.rs` 也早已用 `to_regclass` 探存在。读法据此改为：主查询只读会话自己的列与证据侧材料，跨行业身份**先探表存在、再按 `sample_ref = ANY($1)` 补齐**；表不在就留空——缺的是一张参照物表，不是这条会话，整页不该因此变成「读取失败」。只有当本页真的读到跨行业会话时才付这次往返。

**证据**：`collection_dispatch_sequence_postgres` 两条新用例——`delivery_reconciliation_counts_receipts_not_the_session_marker`（四态各自可读：只有导航 → `AwaitingDelivery` 4/0；逐个投递 → 4/3；`DeliveryPending` + 末通道 → `Delivered`（T22）；`Stopped{risk_stop}` → `Closed` 且带停止原因与终结时间；**晚到的 `DeliveryPending` 被写入侧拒绝为 `SessionNotHeld`，读侧结论仍是 `Closed`**（T27）；五种停止原因逐一回读）与 `a_session_without_its_cross_industry_table_still_reconciles`（来源表缺席时该行照常读出、平台与作品号留空）。`content_reobservation_postgres::detail_page_grant_replays_one_request_and_suppresses_a_new_request_for_the_same_work` 补了两段读层断言（`delivery_pending` 且无通道 → `RecoveryUnverified`；停止后 → `Closed` + `page_unavailable` + `closed_at`）。页面层五条单测覆盖四态渲染、`LOST_AUTHORITY` 不成全绿、读失败出 `UNKNOWN`、终结原因中文。两条断言做了变异验证（§8）。

**未证明**：跨行业会话**真的**走一遍授予→导航→回执（读层的补齐分支只有「表缺席」这一侧有隔离证明，跨行业侧需要跨行业来源表与授权夹具，本步没有建）；页面实拍仍是 `NOT VERIFIED`（§9.7 视觉一致一行的理由不变）。

**本卡不做**：不写台账、不新增列、不改索引、不新增可点动作、不改接纳/权限/额度/调度与清理规则、不动 `runtime-main`/共享库/插件版本/`:3000` 运行态。
