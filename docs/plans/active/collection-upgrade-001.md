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
| `LOST_AUTHORITY` 回执在页面上显示英文机器码 | 按 S3 状态词典给出「材料已保存，原执行权已失效」 | `collection_tasks_view.rs` | T24, T27 | 待实施（UI 文案，须先提交变更清单） |
| 采集完成才登记 Attempt，断网后新包无服务端身份 | 导航前在授权事务内登记全部冻结通道的稳定 Attempt 身份，按 capability 协商启用 | `0096`、`dispatch.rs::grant_detail_page_session_with_lane_deliveries`、`producer_runtime.rs::prepared_lane_delivery_exists`、`apps/api/src/local_web.rs`、`adapter.js`/`detailPageSessionStore.js`/`background.js` | T05, T09, T31 | **已实施；T31 与 T05 的隔离 PostgreSQL + 插件测试通过**（T09 逐通道如实显示待 S3；变异验证见 §8） |
| E1 缺地址反复入队 | 输入资格前移；新工单冻结执行输入引用；原范围事务化停止 | `keyword_archive_detail.rs` 候选/批量判据、`dispatch.rs` pending 分支、`work_order_lease.rs` 剩余工作、新 `execution_input_eligibility.rs` | T11–T15, T18 | **已实施；隔离 PostgreSQL 证明通过**（S2 补齐了渐进建档这一侧的候选判据：「此刻解析得出地址」与「停过且输入未变」两条入口判据现在是两个面共用的同一句，见 §5.1；T11/T12/T15 有测试；T13 的两处语义见 §5.1；**T18 单独立了测试** `an_in_progress_replay_with_a_dead_locator_keeps_the_attempt_and_never_renavigates`——它守的是**修前就已正确**的那条路（`in_progress` 重放不释放、不销毁），本次改造没有碰它，因此这是回归守卫而不是修复本身；变异验证见 §8；**T28 的三个例子已齐**：①「明确页面不可用」新增 `an_explicitly_unavailable_page_stops_only_its_lanes_without_fabricating_deletion`（原先没有任何用例驱动 `page_unavailable` 这个码），②「暂时超时」复用 `failed_browser_start_is_audited_then_returns_work_order_to_shared_queue`，③「选择器失配」复用 `unavailable_detail_is_audited_without_blocking_later_materials`（详情通道落在 `detail_page_session_recovery_required`）与非详情通道的 `repeated_detail_read_failure_becomes_blocked_without_stalling_later_materials`；三例是三个不同的失败码（处置：①③停止、②有界退避；详情通道的失配也停止，因为导航许可已消费而重开比停下更危险，但原因分开记），都不伪造删除/耗尽，变异验证见 §8。T14 由 S2 的 `detail_read_failure_budget_follows_the_requirement_scope_across_new_work_orders` 覆盖，见 §5.3） |
| E2 失败预算随新工单清零 | 预算落在资格台账的**当前行**上：按需求范围跨工单累计、按事件去重、退避阶梯 60/120/240/480/900，第三次用尽即停止自动重试；建单时冻结输入（`execution_input_frozen_at`），台账建立前的旧失败只记待核实不进预算 | `execution_input_eligibility.rs`（`record_detail_page_read_failure_in_transaction`／`budget_blocks_new_work_predicate`／`budget_exhausted_object_refs_in_transaction`）、`dispatch.rs`、`work_order_lease.rs`、`acquisition_chain.rs`（建单冻时刻 + 渐进候选）、`keyword_archive_detail.rs`（四个入口） | T12–T18, T23 | **已实施；隔离 PostgreSQL 证明通过**（口径与测试见 §5.3；`0097` 在 S2 就地追加两列——它从未应用到任何共享库、分支未合并，实施前仍须按 §3 复核编号） |
| E5 title 非空被当作「详情已取得」 | 统一材料完成判据 = 合格详情材料及其来源；标题缺失是字段覆盖度 | `target_catalog.rs`、`archive_completeness.rs`、`content_reobservation.rs`、`collection_targets_view.rs` | T19–T21, T23 | 待实施 |
| E6 session 终结与 Receipt 不对账 | 统一当前状态 DTO（原因/来源/最后确认时间/允许动作）+ 会话终结归并 | `collection_tasks_view.rs`、`runtime_capacity.rs`、`queue_position.rs` | T22, T27 | 待实施 |
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
S3 统一状态读取（消费 S2 的资格事实）
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

八次变异均已还原（前两次 S1a、四次 S1b、两次 S2）。S1b 的还原用还原前快照逐字节核对：`execution_input_eligibility.rs` sha256 `6408147952b274a9a7ae8f180fb53eaaf37362383177fa30355f31377fba43ab`、`0097` 最终 sha256 `83a8a99362df528217ac7473c102ca8c42f6f8beb3b76b9e195f95451ddac3b7`（已同步进 `material_fixture` 与 `full_schema_fixture` 两处账本）、`dispatch.rs` 变异前后同为 sha256 `5b54d0395180ea385150dff4b60fa608083ff334319a21ecb57d7b2dbab1c6b0`。T28① 的变异同样从 `dispatch.rs` 还原，之后该文件 sha256 仍为 `5b54d039…`（两次变异都是逐字节还原）。S2 的两次变异从 `acquisition_chain.rs` 还原：变异前快照存于 `/tmp/acquisition_chain_s2_pre_mutation.rs`，还原后 `grep -c MUTATION` = 0 且 sha256 与变异前同为 `3e4f14e56f7b5b2e08152dbb96e009bee48e701911d6f08a2fc770b4e8fedd43`（逐字节）。各阶段还原后：S1b 的 `keyword_archive_postgres` **16 passed / 0 failed**、`collection_dispatch_sequence_postgres` **27 passed / 0 failed**；S2 的 `collection_dispatch_sequence_postgres` **32 passed / 0 failed**、`observation_target_dossier_postgres` **23 passed / 0 failed**，`cargo check --workspace --all-targets --locked` 通过。源码冻结后的干净全套（`./scripts/test-local-001-discovery-postgres.sh`，20 目标）**231 passed / 0 failed**，容器/卷/库自建自清。
