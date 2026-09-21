# COLLECTION-UPGRADE-001 · 采集执行、交付恢复与材料完成收口

> 状态: 活跃计划
> 最后核对: 2026-09-21
> 适用范围: 采集链路的关键词详情缺口准入、派发资格、跨工单重试预算、交付恢复、统一状态读取与运行诊断
> 事实来源: Mog 派定的交付包 `linggan-collection-upgrade-handoff-2026-09-21`、`origin/main@1ef5830c` 的代码与迁移、隔离 PostgreSQL 基线运行结果
> 冲突时以谁为准: 交付包合同与 Mog 最新确认；实现事实以当前代码、迁移与真实验证输出为准

## 2026-09-22 真实数据候选查询继续核对

#330 已合并 720ca8c；recovery 一轮正常结束，关停阻断已修复。目标列表在 45 秒内未返回，故保持 recovery，继续同交付包的输入资格修复。EXPLAIN 显示签名地址解析子查询使用 `content` / `sample`，遮蔽调用方同名外层别名，把对象匹配变成内层自比较；这同时导致全量扫描与用别篇地址判断可执行。

- Claim 沿用；续修分支 `codex/collection-locator-correlation`，起点 720ca8c，原专属工作树。
- 修复解析片段内部别名的相关性，证明本领域/跨行业无签名作品不能借用另一作品地址；复跑关键词、资格派发和真实库只读执行计划。无新 schema、无历史回填。
- 临时 SQL 捕获只在隔离证明进程中写入 `/tmp`，已恢复诊断插桩；真实库仅 EXPLAIN/SELECT 与取消本任务超时只读查询，不执行 repair apply。
- 证明已完成：双侧错借地址红转绿；关键词 18/18、派发 37/37、档案 24/24，资源清理通过；真实库只读新查询 246.324ms，独立复审无阻断。运行治理阶段、PID/SHA/heartbeat 与前端完成度仍以最终发布回执为准。

## 2026-09-22 发布现场跟进：关停与详情查询

PR #328 已合入 main@d3ed766；0096–0101 已在备份后应用，0.8.55 已保持原安装身份重载。recovery 运行可交付旧包。切换 governance 时巡检 worker 未在 90 秒内确认退出，部署脚本按设计中止；API 仍为 recovery，巡检服务已停止。本节作为同一授权交付包的发布阻断修复，不引入新的业务权限。

- Claim 沿用本交付；branch `codex/collection-runtime-drain`，起点 d3ed766，沿用专属 `pr-328-review-20260921` 工作树。后续修复使用新 PR，不改写已合并的 #328。
- 现场账本证明 keyword_details 一步耗时 214.8 秒；关停信号只在整轮 tick 后处理。修复应取消采集面的等待，继续等待独立模型循环完成 drain；被打断轮次保持未完成，不伪造结束事实。
- recovery 在昂贵的详情候选查询前返回明确跳过；按目标 EXISTS 改写“是否还有候选”查询，保持既有资格条件并用 PostgreSQL 证明核对。
- 验证：真实子进程在数据库锁阻塞时 SIGTERM 的反例/修复证明、关键词与派发隔离集、独立 commit-reviewer、main 与运行 SHA/PID/readiness/heartbeat 回读。
- 实测：旧版持锁关停用例在 8 秒超时失败；修复后启动/关停隔离 3/3、关键词 16/16、派发 37/37；另以候选表独占锁证明 recovery 子进程不进入扫描，1/1。证明资源清理完成。最终部署与心跳证据写入本任务发布回执，仍不代表真实平台或 Mog 业务验收。

## 2026-09-21 PR #328 修复与发布接手

Mog 在本任务明确要求修复审查问题、提交合并 main 并部署刷新。该授权覆盖本交付包的代码、必要 schema 发布、插件候选、受控本机运行刷新；真实平台新增访问和任意历史数据重写仍不由发布推导。本节取代下文历史阶段中的“未授权 merge/push/部署”描述。

- Claim：`pr-328-completion`；执行者 Codex 当前任务；起点 `0aa8ac20ff68d5137352ec9cf6ad61061dab874e`，整合 `origin/main@3ff3982`。
- 专属 branch：`codex/pr-328-completion`；工作树：`/Users/moglenny/proma/linggan-intelligence/.worktrees/pr-328-review-20260921`。只更新既有 PR #328，不在共享 main 或原执行代理工作树改代码。
- 范围：审查 F1–F5、S6 默认只读 repair preview/隔离应用证明、分阶段启用、版本兼容与插件可复现候选；补齐对应失败路径后运行定向和整包验证，提交前按 AGENTS.md 执行只读 commit-reviewer。
- 通道准备合同补全原 TaskSpec。插件必须校验冻结通道全集和身份，再持久保存；已经取得的事实在后台按原 Task/Attempt 包装、冻结 envelope 并独立交付，不依赖后续执行 claim，不产生新导航。
- IndexedDB 读取失败保持明确失败；同身份同内容重放、异内容冲突；未通告受支持准备协议的服务端不授予新插件新导航。旧已入队包仍按原身份交付。
- 缓存恢复只处理实际持久化的 payload，未取得的数据保持未知。旧缓存缺完整原任务合同标记需处理，不猜造或改挂新 Task。
- 发布先开启恢复能力，再依据隔离与本机兼容证据开启资格治理；所有运行检查分别记录 schema、SHA、PID、readiness、插件构建及尚未验证的业务层。
- 以下 S0–S6、T 表与未授权说明保留为原提交的历史交付声明；最终修复结果以本节后续回执为准。

### 接手后的合同更正（当前执行口径）

本节覆盖下文交付代理在 `0aa8ac2` 及之前写下的完成、权限和发布判断；旧章节作为阶段记录保留，不能据其声明当前已上线。Mog 当前已授权修复、提交、合并到 main 与部署刷新。

- F1/F3：导航准备保存每个通道完整 TaskSpec、原 Attempt 和租约边界；IDB 读写失败不得生成替代身份。缓存恢复独立扫描，冻结包后入队，活租约未 claim 的通道等待 claim 或租约结束；服务端返回 `scheduled_lane_waiting_for_claim` 时继续保留。
- F2：`detail_page_session_lane_preparation_unavailable` 纳入未部署的 0097 约束及停止策略；不会再以 23514 代替停止回执。migration hash 以两处 fixture 与当前文件为准。
- F4：outbox 同身份不同内容明确冲突；同内容重放保留原 envelope。session 首写与排队标记用事务防止并发覆盖，正文提前交付与最终重试共用冻结包。
- F5：新插件遇旧 API 不发起新详情访问；旧 outbox 仍走原交付协议。
- S6：提供 `linggan-collection-repair` 默认只读预览；每批最多 100 个对象，支持 `--task-id UUID` 精确查看被默认窗口截断的历史对象。仅缺输入、全部相关通道未开始、无会话/Attempt 的活租约可停止；其余逐项排除。预算历史未知不追溯扣分，本地 outbox 明示 NOT_OBSERVED。应用需要原文件、batch-id、preview-hash；逐项 serializable 重核，锁等待 2 秒、语句 5 秒；新输入/状态变化/争用跳过，重复应用不追加重复事件。JSON 回执绑定批次与预览哈希。
- 分阶段发布：`LINGGAN_COLLECTION_UPGRADE_PHASE=recovery` 是默认值，暂停 claim、新 lease、新详情 grant 和关键词详情推进，但允许原包交付。仅验证后设 `governance`。回退使用同一新版二进制切回 recovery，保留新增 schema/状态/身份。旧二进制误读 input_blocked，**不批准回退旧二进制；禁止移动远端 main 指针**。
- 插件发布为 0.8.55；源码、manifest、lock、ZIP、release-manifest 同版本。从全新 npm ci 重建并核验 hash。`scripts/proof-session-mv3.mjs` 使用一次性 Chromium profile、合成缓存、真实 IndexedDB/worker 重启和模拟交付接口；死代理阻止真实平台/本机 API 访问。此证明不等于真实平台验收。

本次新增代码归属：领域修复入口 `crates/evidence/src/collection_repair.rs`，单次运维命令 `apps/worker/src/bin/collection_repair.rs`；两者不属于常驻 worker 自动任务。验证脚本与插件既有 scripts 同生命周期；运行输出仅进入任务证据目录，不进仓库。

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
| 故障无法按一条链路串起来 | 结构化事件字段 + 复用现有巡检账本的统一 tick 关联 | 采集/worker 入口、`collection_scheduler_run` | T29–T32 | **已实施（S4a–S4d）**：就绪分级与 worker 退出语义（S4a）、统一 tick／步骤结果／结构化事件（S4b，新表 `collection_scheduler_run_step` = 迁移 `0098`）、选择器诊断与陈旧版本文案（S4c，`0099`）、一个 tick 号串一条链与字段白名单（S4d）。隔离库 `scheduler_tick_postgres` **5 passed / 0 failed**、`linggan-evidence --lib` **86 passed / 0 failed**；诊断查询已写进部署手册 §7。**未证明**：样例是描出来的不是抓来的（一条真实故障的全链要真实平台）；`emit_step` 的调用点只有类型保证、没有取值断言 |
| 性能候选缺少证据 | 建 SQL 次数/耗时/计划基线；无稳定收益则明确「收益未证实，不改」 | 见交付包 S5 表 | T33–T34 | **已实施（S5）**：六条候选逐条给读数与结论（§11.4）——两条索引实施（`0100`）、一处 panic 型解码换成具名错误、另一条索引与其两条兄弟**明确不改**（收益未证实/不同对象）、零引用模块降级为历史材料、原因码词表补齐（`0101`）。读数取自 `pg_stat_user_tables` 的时间窗口差值 + EXPLAIN，**不写「提速 N%」**——收益量级如实说：真正理由是形状（随库增长线性上升 → 常数级），6.8× 只作复刻规模下的同参数对比出现。**未证明**：只在复刻规模上量过，真实并发下的计划选择未测 |

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

| 未就绪的 worker 必须留在原地等，不能以「干净退出」收场（T29，`unreachable_database_keeps_the_process_waiting_and_says_so`） | 把 `apps/worker/src/main.rs` 未就绪分支的退避等待（`tokio::select!` 睡眠 + `next_readiness_retry`）整个换成 `return ExitCode::SUCCESS` | 用例在 `alive` 断言处变红（`startup_contract.rs:107`）：15 秒观察窗内进程已经不在，日志停在开场行与 `linggan worker: not ready (database_unreachable: probe_timeout); no work is claimed until this clears`。修前形态正是这个：对 launchd 是一次正常退出，看日志的人只看到「不可达」然后什么都没有 |
| 连得上但台账不在，必须报 `migration_ledger_unreadable`，不能当成可以接活（同上，`reachable_but_unmigrated_database_is_classified_not_masked`） | 把 `crates/evidence/src/runtime_readiness.rs` 里 `!ledger_present` 那一支的 `MigrationLedgerUnreadable` 换成 `Ready` | 用例在分类断言处变红（`startup_contract.rs:185`）：日志里没有那一行分类，取而代之的是 `linggan worker: patrol tick every 60s` 和随后的逐步骤失败（`media acquisition projection failed: media acquisition schema is unavailable`、`progressive dossier tick failed: acquisition chain schema is not applied`）——**一台根本接不了活的机器被说成「在跑、只是步骤不顺」**，这正是把「缺 schema」与「步骤失败」压成一句话的后果 |
| 媒体 worker 连不上时必须在十秒内说出来（`unreachable_database_keeps_the_media_worker_retrying_instead_of_exiting`） | 去掉 `connect_when_reachable` 的 `tokio::time::timeout(CONNECT_ATTEMPT_BUDGET, …)`，直接 await 连接池（它自己会重试满 30 秒） | 用例在「连不上必须说出来」处变红（`startup_contract.rs:160`）：15 秒窗口内日志只有开场那一行（`linggan media worker: started; checking whether the local database is reachable`），`retrying with backoff instead of exiting` 一个字都没有——修前正是这个样子：整整半分钟与一台空闲机器无法区分 |

| 失败/跳过的步骤行必须能收尾，且**没有计数**（T30，隔离 PostgreSQL `one_failing_step_is_recorded_alone_while_the_other_three_finish` 与 `a_step_whose_own_tables_are_missing_is_skipped_rather_than_failed`） | 把 `0098` 的三个计数列还原成修前写法 `integer NOT NULL DEFAULT 0`（CHECK 一并复原为只判 `>= 0`） | 两条用例同时变红，且失败形状正是这版要消灭的那种：三步各打一行 `linggan runtime: step … finished but its ledger row could not be closed: … null value in column "considered_count" … violates not-null constraint`，随后在 `step rows are readable` 处 `ColumnDecode UnexpectedNullError`（`outcome` 读出来是 NULL）。**写不进去的行留下来的是「开始了没收尾」，与真的被杀掉长得一样**——一个「看起来什么都没发生」的步骤行，正是本卡存在的理由 |
| 一步失败必须染红整轮（T30，`one_failing_step_is_recorded_alone_while_the_other_three_finish`） | 从 `tick_outcome` 删掉「任一报告带 `error_class` 就返回 `failed`」那一段，让整轮只按巡查步的产出判 `idle` | 用例在收轮处变红（`scheduler_tick_postgres.rs:95`，`the tick closes in its ledger`）：`23514` —— `collection_scheduler_heartbeat_check1`（`0020` 的 `(last_outcome = 'failed') = (last_error IS NOT NULL)`）拒绝写入，失败行里正是 `last_outcome='idle'` 配 `last_error='media_acquisition:sqlstate_42703'`。**数据库是第二道闸**：把失败说成空闲的这一对内伤，账本根本不收 |
| 步骤账本不在时不开轮（T30，`an_absent_step_ledger_opens_nothing_rather_than_a_half_recorded_tick`） | 把 `TickLedger::begin` 的探针从「run 表 **且** 步骤表都在」改成只探 run 表 | 用例在 `opened.is_none()` 处变红（`scheduler_tick_postgres.rs:295`）：账本不全时照样开了一行 run，随后四步都记不上——「跑了却记不上」比不跑更坏，也正是这一条要挡的 |
| 结构化事件必须真的落在 stdout（T30，`unreachable_database_keeps_the_process_waiting_and_says_so` 末段） | 把 `RuntimeEvent::emit` 的 `println!` 换成 `eprintln!` | 用例在「未就绪时该有一行 readiness 事件」处变红（`startup_contract.rs:130`）：15 秒日志里一行可解析的事件都没有。白名单那条测试证明的是**定义**（字段闭集、每个字段都有生产者），这条证明它走到了 supervisor 看的那条流上——只改流向，白名单仍然全绿 |
| 本来就受限形状的长标识必须留前缀，而不是被判成「没分类」（T30，`a_long_restricted_identifier_keeps_its_prefix_instead_of_vanishing`） | 去掉 `bounded_code` 的 `.take(32)`（改成 `take(usize::MAX)`） | 用例在「超长的受限标识留前缀」处变红：`left: "0098_scheduler_tick_steps_and_readiness" != right: "0098_scheduler_tick_steps_and_re"`。截断不是美化——账本的 `error_class` 列写死 `≤32`，不截就是写不进去 |
| 数据库给的 SQLSTATE 大小写必须归一（同上用例第二处） | 去掉 `bounded_code` 的 `.map(\|c\| c.to_ascii_lowercase())` | 用例在「SQLSTATE 是大写，词表一律小写」处变红：`left: "42P01" != right: "42p01"`——同一件事在事件里是 `42P01`、在账本里是 `42p01`，两处读者会当成两个类别 |

| 收不下的一份报到**不能改写**这台安装已有的记录（T32，隔离 PostgreSQL `selector_health_is_stored_read_back_and_never_gates_a_check_in`） | 把 `execution_station.rs` 心跳写入里的 `selector_health = COALESCE($4::jsonb, selector_health)` 换成 `selector_health = $4::jsonb`——即把「收不下＝不写」改成「收不下＝写空」 | 用例在「收不下的一份报到不能改写这台安装已有的记录」处变红，左边正是缺陷的形状：`left: None != right: Some(Object {"xhs": Object {"checkedAt": …, "missingCategories": ["reply_expand"], …}})`——一份谁也没收下的报告把库里那份好记录抹掉了，而这次报到**什么都没证明** |
| 「没有自检记录」不得画成「一切正常」（T32，页面层 `a_station_that_never_reported_a_selector_check_is_not_drawn_as_healthy`） | 把 `station_view.rs` 无记录那一支的 `未上报自检` + `c-tg-neutral` 换成 `自检无缺失` + `c-tg-ok`（悬停说明一并换成「没有记录」） | 用例在第一处断言变红：`assertion failed: html.contains("未上报自检")`。它钉住的是**展示映射**这一层：领域判据再对，页面仍可以用一句没依据的「正常」盖住一台可能早就认不出页面的机器 |
| 弹窗说的必须是 Chrome 真正加载的那份版本（E10，插件 `the recovery surface names the version Chrome actually loaded, never a stale literal`） | 把 `popupPluginVersionLabel()` 的返回值写死成 `'v0.4.8'`——即把修前的那个字面值放回去 | 用例变红：`+ 'v0.4.8' - 'v0.8.54'`（`linggan-popup-startup.test.mjs:148`）。这正是 E10 指的那类文案：界面说着一个不是本机的版本，人在扩展页看到的会是两个不同的数字 |

| 失败的那一行必须说出受限类别（S4d 串证用例 `one_tick_ref_strings_the_failure_and_what_it_queued_without_reading_the_logs`） | 把 `StepReport::event` 里 `error_class` 那一段（`with_error_class`）整块去掉——库里的步骤行照旧带类别 | 用例在 `assert_eq!(line["errorClass"], "sqlstate_42703")` 处变红：事件那一行没有类别，而同一个号下账本写着 `sqlstate_42703`——**同一份报告的两个读者开始各说各的**，正是「值只在这里写一次」要防的那件事 |
| 快照里**只能**有白名单上的键（S4d `nothing_outside_the_field_whitelist_survives_into_the_snapshot`） | 让收口把上报里白名单之外的键原样带进快照（`url` 这一类）——即「收口漏了一类字段」 | 新用例在「快照只带字段表上的键」处变红（`url` 留在了快照里）；**同一条「只收受限形状」的旧断言仍全绿**——它的载荷里本来就没有那些键，够不到这处泄漏。这正是要补一条夹带载荷用例的理由 |
| 一步排出的决定必须挂在**同一轮**上（S4d 串证用例） | 让巡查步写目标级决定时用一个新号（`Uuid::new_v4()`）而不是 `ledger.run_ref()`——即「这一步排出的活成了另一轮的事」 | 用例在 `fetch_one` 处 `RowNotFound`（`从一个 run 号出发就能走到「哪一步失败了 + 这一步排出了什么」`）：从一个 run 号出发 JOIN 不到那一步排出的决定。串证要成立，三张表必须真的用同一个号 |
| 字段表本身就是判据（S4d 两条快照用例） | 从 `SELECTOR_HEALTH_SNAPSHOT_FIELDS` 里删掉 `failureCounts`（快照里这个键仍在） | 两条用例同时变红（「快照只带字段表上的键」与「只收受限形状」）：清单与实现分开写，多的、少的都会露出来 |

| 代码侧加了码、迁移没跟上（S5，单元用例 `both_receipt_check_constraints_spell_out_the_whole_closed_vocabulary`） | 往 `MONITOR_COMMAND_REASONS` 里插一个码 `MUTATION_A_pretend_new_code`——即「有人加了码却忘了开迁移」 | 用例变红并指名道姓：`最后一条写 collection_command_identity_reason_codes_ck 的迁移是 …/0101_collection_command_reason_vocabulary.sql，它的词表与本域词表不一致：多了 []，少了 ["MUTATION_A_pretend_new_code"]`；左边（库上真正生效的那份）是 55 个码。这正是本卡修的那个缺陷的复发形态：`0065` 之后代码侧陆续加了十几个码，两条 `CHECK` 一个都没跟上 |
| 迁移侧手抄漏了一个码（同上） | 从 `0101` 的身份表 `CHECK` 里删掉 `within_authorization`——即「词表的第三份副本自己写少了一个」 | 同一个用例变红：`少了 ["within_authorization"]`。方向相反、报法相同——它比的是**两份词表的差集**，谁少了谁都会露出来；这条也顺带证明它不会因为「两边都少同一个码」而放过 |
| 「已物化却没有下载尝试引用」必须得到具名错误，不是解码 panic（S5，隔离 PostgreSQL `full_runtime_accepts_each_capability_without_collapsing_partial_media_or_replay` 末段） | 把 `claim_media_upload_finalize` 的 `row.try_get::<Uuid,_>("download_attempt_ref").map_err(\|_\| MaterializedSessionWithoutDownloadAttempt)?` 换回 `row.get::<Uuid,_>(…)` | 用例变红，**红在解码处**：`panicked at crates/evidence/src/producer_runtime.rs:471:44: called Result::unwrap() on an Err value: ColumnDecode { index: "\"download_attempt_ref\"", source: UnexpectedNullError }`——一个正在处理请求的进程被一行坏数据打掉（正是 `try_get` 要换掉的那个后果），而修好的版本走的是具名错误分支 |

三十二次变异均已还原（两次 S1a、四次 S1b、两次 S2、三次 S3、两次 S3c、三次 S4a、六次 S4b、三次 S4c、四次 S4d、三次 S5）；S4b、S4c、S4d、S5 的十六次分别记在本节下面四段。**一处更正的计数**：这句话上一版写的是「十九次」，而表上当时已经是二十五条——S4b 的六条进了表却没有累进这句计数（十六应为二十二），S4c 又只按自己的三条把十六改成十九。以表上的行为准：二十五条 + S4d 四条 = 二十九条，+ S5 三条 = 三十二条。S3c 的两次变异都从 `crates/evidence/src/collection_task_read.rs` 还原：两处都是逐字对照原句反向替换（终态分支回到第一条、计数回到 `receipt.receipt_ref`），还原后 `grep -n "count(lane.attempt_id)"` 为 0 命中、匹配臂顺序与原句一致，两条 S3c 用例重新变绿。S3 的三次变异分别从 `execution_input_eligibility.rs`（两次）与 `target_drawer.rs`（一次）还原：还原后逐一 `grep` 变异标记（`false &&`／`if false`／写错的那两支文案）为 0 命中，`git diff` 回到变异前的内容，两条证据用例与页面用例重新变绿。S1b 的还原用还原前快照逐字节核对：`execution_input_eligibility.rs` sha256 `6408147952b274a9a7ae8f180fb53eaaf37362383177fa30355f31377fba43ab`、`0097` 最终 sha256 `83a8a99362df528217ac7473c102ca8c42f6f8beb3b76b9e195f95451ddac3b7`（已同步进 `material_fixture` 与 `full_schema_fixture` 两处账本）、`dispatch.rs` 变异前后同为 sha256 `5b54d0395180ea385150dff4b60fa608083ff334319a21ecb57d7b2dbab1c6b0`。T28① 的变异同样从 `dispatch.rs` 还原，之后该文件 sha256 仍为 `5b54d039…`（两次变异都是逐字节还原）。S2 的两次变异从 `acquisition_chain.rs` 还原：变异前快照存于 `/tmp/acquisition_chain_s2_pre_mutation.rs`，还原后 `grep -c MUTATION` = 0 且 sha256 与变异前同为 `3e4f14e56f7b5b2e08152dbb96e009bee48e701911d6f08a2fc770b4e8fedd43`（逐字节）。各阶段还原后：S1b 的 `keyword_archive_postgres` **16 passed / 0 failed**、`collection_dispatch_sequence_postgres` **27 passed / 0 failed**；S2 的 `collection_dispatch_sequence_postgres` **32 passed / 0 failed**、`observation_target_dossier_postgres` **23 passed / 0 failed**，`cargo check --workspace --all-targets --locked` 通过。源码冻结后的干净全套（`./scripts/test-local-001-discovery-postgres.sh`，20 目标）**231 passed / 0 failed**，容器/卷/库自建自清。

S3c 两次变异还原后：`collection_dispatch_sequence_postgres` **34 passed / 0 failed**（新增两条）、`content_reobservation_postgres` **4 passed / 0 failed**、`linggan-api` 二进制内 `--ignored` **23 passed / 0 failed**；`cargo check --workspace --all-targets --locked` 通过。S3c 源码冻结后的干净全套（20 目标）**234 passed / 0 failed**，容器/卷/库自建自清——这一跑同时补上了 S3b 记录里被宿主磁盘写满打断的那次重跑（当时第 20 个目标 7 例 `57P03 in recovery mode` 未计入结论）。

S4b 的六次变异分别落在 `database/migrations/0098_…sql`（一次，SQL 层）、`crates/evidence/src/scheduler_tick.rs`（两次）、`crates/evidence/src/runtime_event.rs`（三次），还原前快照存于 `/tmp/scheduler_tick.rs.orig` 与 `/tmp/runtime_event.rs.orig`（限码那两次另存 `/tmp/runtime_event.rs.pre_bounded`）。还原后：`0098` 的 sha256 回到 `1933b8978c73c094f8e41c04d119021c17a25483ba71c0d7751af7556e1ba310`（与 `material_fixture` / `full_schema_fixture` 两处登记逐字一致），两个 `.rs` 文件逐字节还原（按还原前快照 `cp` 回去；**注意此处的教训**：这两个文件在本步尚未提交，`git diff` 对未跟踪文件恒为空，因此不能用它当还原凭据，只能按快照逐条比对断言），`grep -rn "MUTATION" apps crates database scripts` 为 0 命中；`cargo test -p linggan-evidence --test scheduler_tick_postgres --locked -- --ignored --test-threads=1` 重新 **4 passed / 0 failed**，`cargo test -p linggan-worker --test startup_contract --locked` 重新 **4 passed / 1 ignored**，`cargo test -p linggan-evidence --lib --locked` 重新 **79 passed / 0 failed**。**两处如实记录的发现**：`0098` 的三个计数列在本步**曾经是错的**（`NOT NULL DEFAULT 0`），是 T30 的新用例在第一次跑时就抓出来的——失败与跳过的行写不进去，留下的 `outcome` 是 NULL，恰好把「这一步崩了」伪装成「这一步开始了没收尾」，也就是本卡要消灭的那个东西；`bounded_code` 原本把散文按字符挑成一个「码」（详见 §10.9 S4b），单测在第一次跑 `--lib` 时变红——这两处都记在该节的「两处如实记录的发现」里，不是我事后自查的结论。**另一处更正**：§10.4 原先举的跳过原因例子 `not_ready` 在实现里没有生产者（真实生产者只有 `schema_unavailable`，即这一步自己的表不在），已按实现改掉——计划里写一个没人生产的码，正是这张表要防的错。

S4c 的三次变异分别落在 `crates/evidence/src/execution_station.rs`（SQL 写入）、`apps/api/src/local_web/station_view.rs`（页面映射）、`plugins/linggan-intelligence-browser/src/popup/startupRecovery.js`（版本文案）。三处都按变异前快照 `cp` 还原并**用 sha256 逐字节核对**：`execution_station.rs` `7f6aa8c1b980e84327375ccb84b7ecc589fd8510d6673dc74ddb1792a5bf4599`（还原后 `COALESCE($4::jsonb, selector_health)` 回到第 298 行）、`station_view.rs` `a30fa8ebde7e53045aa15a5541d6c0d508c76327614af8fe41f1d38c984dd9db`、`startupRecovery.js` `fdbf5d14ee662e988f49a6f62ff6c274796df521aff38a556d34450caf00307a`。这三个文件都是**已跟踪、且有未提交改动**的文件，所以它们的 `git diff` 有内容、可以作为还原的旁证；但**还原凭据仍是快照比对**（逐字节 sha256），不是「diff 看起来对」。还原后：`grep -rn "MUTATION" apps crates database scripts plugins` 为 0 命中；`collection_control_runtime_postgres` 重新 **14 passed / 0 failed**、`linggan-api` 二进制重新 **259 passed / 0 failed / 25 ignored**、插件 `npm run test:linggan` 重新 **287 条 286 passed / 1 failed**（那一条是既有的 `linggan-current-surface-discovery-runtime`，见 §10.9 S4c 的未证明与如实记录）。

S4a 的三次变异分别落在 `apps/worker/src/main.rs`、`crates/evidence/src/runtime_readiness.rs`、`apps/worker/src/bin/media_worker.rs`（上表三条，逐字还原）。还原后 `grep -rn "MUTATION" apps crates --include="*.rs"` 为 0 命中，`cargo test -p linggan-worker --test startup_contract --locked` 重新 **4 passed / 1 ignored**（15.01s，`--ignored` 那条走证明库）。**一处如实记录的观察**：在「不可达端口」这一实测条件下，未就绪日志里的 `detail` 落在 `probe_timeout`（十秒没问到），不是 `connect_failed`——sqlx 连接池对被拒绝的连接会自己重试满 30 秒，所以「立刻失败」那条分支很少先到达。两者都归 `database_unreachable`，用例只钉分类码（`not ready (database_unreachable`），不钉它后面跟哪一个。

S4d 的四次变异分别落在 `crates/evidence/src/step_report.rs`（事件映射去掉 `error_class`）、`crates/evidence/src/selector_health.rs`（两次：收口漏一类字段；具名常量少一个键）、`crates/evidence/src/patrol_scheduler.rs`（决定用新号而不挂本轮）。四次都按**变异前快照** `cp` 还原并用 sha256 逐字节核对：`step_report.rs` `596713a3…`、`selector_health.rs` `a1e06afa…`、`patrol_scheduler.rs` `7112a33a…`，另两个本步碰过的文件 `lib.rs` `e84b6a21…`、`apps/worker/src/tick.rs` `63195bfa…` 同样与快照一致（快照存于 `/tmp/s4d-pre-mutation/`）。还原后 `grep -rn "MUTATION" apps crates database scripts plugins` 为 0 命中；三套证明重新全绿：`scheduler_tick_postgres` **5 passed / 0 failed**、`linggan-evidence --lib` **86 passed / 0 failed**、`linggan-worker --lib` **5 passed** 与 `startup_contract` **4 passed / 1 ignored**。**一处如实记录的教训（与还原凭据有关）**：本步对 `crates/evidence/src/lib.rs` 跑 `rustfmt` 时被连带重排了九个**没碰过**的模块（细节见 §10.9 S4d）。做法是**先整份快照到 `/tmp/s4d-rustfmt-collateral/`、再逐个还原、再与原快照逐字节核对**，还原后工作树只剩本步自己的七个文件。教训有两层：**模块根不是单文件**——`rustfmt` 会把它的 `mod` 子模块一起格式化，跑之前得先知道它会碰到谁；以及**动手之前先看清哪些文件本来就带着未提交改动**——`git checkout --` 是整文件覆盖，它不判断「这个文件里有没有我要留的东西」；本步这九个文件当时都在 HEAD 状态，所以覆盖是安全的，但同一个动作落在「改到一半的文件」上就会把在途改动一起丢掉。

S5 的三次变异分别落在 `crates/evidence/src/collection_control.rs`（代码侧加码，表上第 27 条）、`database/migrations/0101_collection_command_reason_vocabulary.sql`（迁移侧漏码，第 28 条）与 `crates/evidence/src/producer_runtime.rs`（`try_get` 回到 `get`，第 29 条）。三次都按**变异前快照** `cp` 还原并用 sha256 逐字节核对：`collection_control.rs` 回到 `0e68c49c14edc3f6e111c7ebcb8453251513bf5a56e6019abc3cd0c46d4997de`、`producer_runtime.rs` 回到 `b8266df21f0671e2f8d3f7ed049803cbf645d1d962a53fdb4f5faa4626f37681`、`0101` 回到 `fda3711ea7bb38af6bb5a6a28264a39ac0c04024aef3e6feeb931e07a9b074c1`（与两处夹具台账里登记的摘要逐字一致，等于同时核了两处登记）。还原后 `grep -rn "MUTATION" apps crates database scripts plugins` 为 0 命中，`cargo test -p linggan-evidence --lib --locked` 重新 **90 passed / 0 failed**（新增守卫一条后由 89 变 90）。**两处如实记录的顺序与返工**：①`try_get` 那条变异第一次复原用的是「按位置插入」的脚本，插进了声明中间（`expected item after doc comment`），源码被写坏——按快照整份复原、逐字节核对（当时 sha256 `1006a815…`）后，改成「先定位 `= &[`、再插在下一行」重做；②守卫用例的断言措辞在两条变异都跑完之后改过一次：原文只写了「代码侧加了码」这一种方向，而变异乙证明迁移侧漏码是同一个报法的另一个方向，留着会误导下一个读它的人；改完**重跑变异甲**确认新措辞同样变红，再跑 `--lib` 确认 90 条全绿，随后整套隔离库在那个最终状态上重跑一遍（结果见 §11.9）。

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

## 10. S4 变更清单（运行诊断与就绪）

> 依 `docs/design/templates/ui-change-manifest-form.md` 的可迁移骨架填写；本清单写在实施计划里，不另建文档。
> 状态: 活跃计划 · 实施前写入（本节先于 S4 代码存在）。

### 10.1 事项

- **Issue / SCOPE**：COLLECTION-UPGRADE-001 · S4（交付包 `02-实施步骤.md` §S4；验收行 T29–T32；事实行 E09「结构化日志不足、编译与运行日志混用、migration 台账查询失败放行、worker 连接失败以 SUCCESS 退出」、E10「有本地选择器 preflight，缺专门服务端选择器诊断；恢复提示 v0.4.8 陈旧，manifest/package 实为 0.8.54」）。
- **Agent 与 worktree**：`fix/collection-upgrade-001` @ `.worktrees/collection-upgrade-001`。
- **目标**：把「这台机器现在能不能接活、为什么不能」从日志里的自由文本，变成四类可判定的运行事实——**就绪分级**、**统一 tick 关联**、**结构化事件**、**受限选择器诊断**；并让插件与服务端两处陈旧版本文案如实说话。
- **用户可见结果**：
  1. 数据库不可达 / 迁移台账读不到 / 迁移未应用 / schema 不兼容时，页面与日志说的是这四件事之一，而不是「本轮 0 个」；此时不派新工作，恢复后自动接着干；
  2. 一个 tick 里哪一步跑了、哪一步失败了可以逐项查，且都挂到同一个 tick 上（不再靠累计日志猜「现在是不是每秒在失败」）；
  3. 插件启动失败弹窗说的是**本机真实版本**（不再写死 `v0.4.8`），服务端「版本过低」文案带上当前最低版本号；
  4. 选择器诊断分别说出「本次检查时间」与「上次验证日期」、缺哪一类检查、失败了几次，且不上报页面原文、DOM 或签名链接。
- **明确非目标**（本卡不做，也不得顺手做）：
  - **不建第二套巡检账本**：tick 与步骤复用 `collection_scheduler_run` / `_heartbeat` / `_target_decision`（§10.5）；
  - **不新增告警通道**（邮件/推送/值班监控）：「持续故障可告警」落在心跳与 `/health` 的持久状态 + 结构化事件上，是否外接告警是一次产品决定；
  - 不改接纳、权限、额度、调度与清理规则；**不改 `MINIMUM_PLUGIN_VERSION` 的取值**（`0.8.47` 保持原判），只改文案与诊断；
  - 不把服务端变成页面检查器：**服务端只据插件上报的受限快照作能力诊断**，不抓页面、不解析 DOM；
  - 不合并/推送/部署、不应用共享库迁移、不重载插件、不发发布包、不访问真实平台。

### 10.2 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| 交付包 `02-实施步骤.md` §S4、`03-验收与发布.md` T29–T32、`04-事实与取舍.md` E09/E10 | 交付包 | 四行验收的判据、退出条件（一条 submission/WorkOrder 串出故障、可脱敏诊断样例与字段白名单） | 2026-09-21 读 |
| `scripts/runtime/{sync,launch,install}.sh` | 真实运行事实 | 迁移台账比对写在 `sync.sh`（未应用→拒绝启动；**读不到→放行**）；`launch.sh` 只负责同步+exec；plist `KeepAlive=true`、无 `ThrottleInterval`、`StandardOutPath` 把编译输出与运行日志写进同一个文件 | 2026-09-21 读 |
| `apps/worker/src/main.rs` | 真实运行事实 | tick 四步（媒体投影/渐进档案/关键词建档/巡查）只有巡查写账本，其余只 `println!`；`ExitCode::SUCCESS` 出现在 DSN 缺失与数据库连不上两条路径 | 2026-09-21 读 |
| `crates/evidence/src/patrol_scheduler.rs`、`0034`/`0036` 迁移 | 真实运行事实 | `collection_scheduler_run` 一 tick 一行、`outcome ∈ (idle,queued,dispatched,partial,failed)`、`considered/dispatched_count` 由**巡查步**写；`device_heartbeat` 单行单键；`patrol_schema_is_ready` 为假时**不建 run 行**（未就绪被记成 idle） | 2026-09-21 读 |
| `crates/evidence/src/{producer_runtime,local_discovery}.rs` | 既有判据先例 | 「台账里必须有这些 migration id + 这些表在」的写法（`to_regclass` 先探物理面，再查 `linggan_local_schema_migration`） | 2026-09-21 读 |
| `apps/api/src/local_web.rs`（`/health`、`LocalDatabaseState`） | 真实运行事实 | 已有 NotConfigured/DatabaseUnavailable/SchemaUnavailable/Ready 四态与三层 schema 探针；插件据 `database.state === 'READY'` 决定是否报到 | 2026-09-21 读 |
| 插件 `src/shared/selectorHealth.js`、`src/platforms/*/selectorHealth.js` | 真实运行事实 | 已有本地 preflight（`finalizeSelectorPreflight` 带 `checkedAt`、`staleChecks`、`missingChecks`）与告警文案；**没有任何字段离开浏览器**；`SELECTOR_VERIFIED_AT`/`SEARCH_FEED_VERIFIED_AT` 是唯一的「验证日期」来源 | 2026-09-21 读 |
| 插件 `src/popup/startupRecovery.js` | 真实运行事实 | 弹窗写死 `version: 'v0.4.8'`，与 `manifest.json` 的 `0.8.54` 不一致（E10 所指的陈旧文案） | 2026-09-21 读 |
| `crates/evidence/src/execution_station.rs`、`collection_control.rs` | 真实运行事实 | 报到自述 `InstallationCheckIn` 已带 `plugin_version`/`capabilities`；`MINIMUM_PLUGIN_VERSION = "0.8.47"` 是判定所用常量 | 2026-09-21 读 |

### 10.3 表面地图

| 表面 | 入口 | 本卡改动 |
|---|---|---|
| 巡检 worker | launchd `patrol-worker` → `apps/worker` | tick 拥有一个 run（tick）标识；四步各记一行步骤结果；就绪不满足时不派活、按退避重试；启动与恢复写结构化事件。**这道闸只管采集面**：模型评论循环有自己的表，只要有连接就照常跑 |
| 媒体 worker | launchd `media-worker` | 仅启动标识与就绪事件（本卡不重排它的作业） |
| 本地 API | `:3000` `/health` | 新增 `readiness`（六分类 + 受限 detail + 检查时间）；已有 `database.state`/`scheduler` 语义不变 |
| 部署与日志 | `scripts/runtime/*.sh`、launchd plist | 编译输出与运行日志分开；启动生成 deployment/revision 标识；plist 补 `ThrottleInterval`（重启节流） |
| 执行工位页 | `/collection/runtime` | 「插件版本过低」文案带上当前最低版本号；**不新增区块** |
| 采集控制面 | `/collection/*` 的待处理/恢复文案 | `recovery_for("plugin_version_unsupported")` 文案带上最低版本号与「以本机实际版本为准」 |
| 插件 popup | 启动失败弹窗 | 版本号取自 manifest，不再写死 |
| 插件内容侧 | 采集页 preflight 与告警 | 快照补齐平台/页面类型/能力/插件版本/规则验证日期/缺失类别/失败计数；本地即时阻断行为不变 |
| 服务端诊断读口 | 报到接口 + 工位读取 | 受限快照经运行时**第二次收口**后按安装保存（收不下就不写，保持已有记录）；工位页在**既有「工位版本」格内**多一行短标记（`未上报自检` / `自检无缺失` / `自检缺 N 类` / `自检待重验`），逐项事实在悬停提示里——该列只有 84px，容不下一行长句，也不新增区块 |

页头/导航/共享壳层与 LIDS Token：不改。

### 10.4 状态词典

**就绪分级**（唯一判据模块 `crates/evidence/src/runtime_readiness.rs`，页面/日志/心跳共用）：

| 分类 | 最低事实条件 | 后果 |
|---|---|---|
| `not_configured` | 被要求接活，却连数据库地址都没给 | 非 READY；这是配置错误，等多久都不会好 |
| `ready` | 台账可读 + 本消费者要求的 migration id 全在 + 要求的表/列全在 | 正常跑 tick |
| `database_unreachable` | 连接或探针查询失败（含连接池超时） | 非 READY、不派活、退避重试 |
| `migration_ledger_unreadable` | 连得上，但 `linggan_local_schema_migration` 不存在或读不动 | 非 READY、不派活；**不再像 `sync.sh` 那样静默放行后照常派活** |
| `migrations_not_applied` | 台账可读，但本消费者要求的某个 migration id 不在 | 非 READY、不派活；detail 只写第一个缺失 id |
| `schema_incompatible` | 台账齐，但要求的表/列不在（旧库/半迁移库） | 非 READY、不派活；detail 只写第一个缺失对象 |

**tick 与步骤**（不新增词汇表，沿用既有 `outcome`）：

| 展示含义 | 最低事实条件 | 所在 |
|---|---|---|
| tick 跑了、这一步有产出 | 步骤行 `outcome='ok'` + 计数 | `collection_scheduler_run_step` |
| tick 跑了、这一步失败 | 步骤行 `outcome='failed'` + 受限 `error_class` | 同上；心跳 `last_outcome='failed'` |
| tick 跑了、这一步没轮到 | 步骤行 `outcome='skipped'` + 受限原因（今日的生产者只有一个：`schema_unavailable`——这一步自己的表不在） | 同上；**不再与「本轮 0 个」混同** |
| 开始了但没有收尾（进程被杀 / 崩溃） | 步骤行 `outcome IS NULL`（`completed_at` 也为空） | 同上；**「未知」必须查得出来**，不许写成 `ok`，也不许写成默认的 0 |
| 这台机器当前不能接活 | 心跳 `readiness_state <> 'ready'` | `collection_scheduler_heartbeat` + `/health.readiness` |

**选择器诊断**（插件产出、服务端受限接收）：

写的是实施后的**实际字段**（S4c 落地时按实现更正过一次，见 §10.9 S4c 的「与原计划的差异」）：

| 字段 | 语义 | 边界 |
|---|---|---|
| `checkedAt` | **本次**运行时检查时刻（每次 preflight 都变） | 不得被当作验证日期展示 |
| `verifiedAt` | 该平台/页面类型选择器**上次人工验证日期**（`SELECTOR_VERIFIED_AT` / `SEARCH_FEED_VERIFIED_AT`） | 过期 ≠ 坏了：只提示「回归验证」，不改判失败；页面上只显示到**日** |
| `platform` / `pageType` / `capability` | 哪儿的、哪类页面、哪项能力 | 受限词表（`[a-z0-9_]{1,32}`）；不是页面内容。`platform` 还是**分组键**：一份快照按平台各一条 |
| `checkedCategories` | 这次实际查过的检查项名 | 只写**检查项名**；名字形状不对也不丢，写 `unclassified`——丢掉它等于把「有一项没过」从快照里删掉 |
| `missingCategories` / `staleCategories` | 缺哪几类检查、哪几类验证日期陈旧 | 同上：只写检查项名，不写选择器串、不写 DOM 文本、不写任何 URL |
| `failureCounts` | 某个检查项**连续**几次检查都是缺的（中间查到过就从零重来） | 正整数才有条目；键同样是检查项名。它由本机存储层补上，不属于页面报来的快照 |

**不在快照里的两件东西**（原计划列过，实施后按实际去掉）：

| 原计划字段 | 为什么去掉 |
|---|---|
| `pluginVersion` | 报到自述里**本来就有** `pluginVersion`（`InstallationCheckIn.plugin_version`），服务端存进 `plugin_installation.plugin_version`。快照里再抄一份就是同一件事的第二个来源——两份迟早有一份过期，而「本机是哪个版本」只能有一个答案 |
| `ruleVersion` | 今日它**就是** `verifiedAt` 的别名（两个日期常量没有独立版本号）。上报一个与验证日期同值不同名的字段，只会让人以为存在一套独立的规则版本体系 |

### 10.5 依赖地图

| 依赖 | 归属 | 本卡的用法 |
|---|---|---|
| `collection_scheduler_run` / `_heartbeat` / `_target_decision`（`0034`/`0036`/`0020`） | 已交付 | **继续是唯一巡检账本**：tick 一行 run、目标级决定仍在 decision；新步骤行是 run 的子行，不另立账本 |
| `collection_scheduler_run_step` | 本卡新增（迁移） | run 的子表，一步骤一行：`step_key`、开始/结束、`outcome`、计数、受限 `error_class` |
| `collection_scheduler_heartbeat` | 已交付（本卡加列） | 加 `readiness_state`/`readiness_detail`/`readiness_checked_at`：持续故障有持久落点，`/health` 与工位页可读 |
| `linggan_local_schema_migration` | 已交付 | 就绪判据只读它；不新增台账、不改写入路径（`local-runtime.sh migrate` 仍是唯一应用者） |
| `plugin_installation` + 报到合同 | 已交付（本卡加一处受限列） | 选择器快照随报到上报，服务端运行时校验后保存**最新一份**；不改认领/授权语义 |
| runtime 标识 | 本卡新增（文件） | `sync.sh` 写 `<support>/runtime-identity.json`（revision/构建时刻/迁移头），`launch.sh` 导出路径；服务端启动读它作为 `revision` |
| `MINIMUM_PLUGIN_VERSION`（`0.8.47`） | 已交付 | 只用于文案，不用于改变判定 |

**未纳入本卡**：告警外发通道、插件发布包、`:3000` 运行态切换、共享库迁移应用与历史处置（S6 才准备）。

### 10.6 变更分类与影响边界

- **分类**：运行合同 + 诊断（含展示文案）。不改产品语义、不改权限与后果、不新增可点动作。
- **最高风险类别**：运行合同——就绪判据与 tick 归属一旦判错，会让**整台机器停止接活**（比「假成功」更贵）。因此：判据收在一个模块；「未就绪」必须能自愈（退避重试）；worker 退出语义与 supervisor 成对写进 runbook/脚本注释。
- **是否存在 `DECISION_REQUIRED`**：
  - **持续故障是否外发告警**——本卡只做持久可见（心跳 + `/health` + 事件），不外发；渠道与阈值属产品决定。
  - **`ruleVersion` 是否要独立版本号**——今日只有两个验证日期常量；本卡据实上报，不新造版本号。
  - **就绪不满足时 API 是否也拒绝派发**——`decide_dispatch` 已有自己的 schema 闸门（`dispatch_schema_is_ready`），本卡不叠加第二道闸；若 Mog 要求「任何未就绪都不许派发」，是一次策略决定。
- **L1 / L2 / L3**：L3 运行基础设施；页面侧只在既有行内改文案，不引入新 Pattern。
- **是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth**：不触及 Token/Primitive/CMP/Scene/Motion；触及 Data Truth（就绪与步骤结果各有唯一定义）。
- **禁止修改的文件/能力**：接纳/派发写路径（`dispatch.rs` 的判定与停止语义）、授权与额度规则、`MINIMUM_PLUGIN_VERSION` 取值、`0097` 台账、`local-runtime.sh migrate` 的应用顺序、`runtime-main`、共享库。
- **停止条件**：若就绪判据在真实运行时会把一台健康机器判成未就绪（假阴性）而无法在退避内自愈，停止扩展判据范围，保留已证实的五分类与最小消费者要求清单，把分歧报给 Mog。

### 10.7 验收矩阵

| 层级 | 验收方法 | 未证明边界 |
|---|---|---|
| 就绪分级（T29） | 隔离 PostgreSQL 注入四类故障：连接不可达（坏端口）、台账表缺失、台账在但缺 id、台账齐但缺表/列；逐类断言 `readiness` 分类与 detail；断言未就绪时不新增任何 run/decision/任务；断言 worker 在未就绪期**不退出**、退避期内不重复派发 | 真实 launchd 下的重启节流行为（本机 plist 未重装） |
| 统一 tick 与步骤（T30） | 隔离 PostgreSQL：一个 tick 内一步失败、其余成功 → 同一 `scheduler_run_ref` 下逐行结果独立可读、心跳如实；对照现有巡检账本，无第二账本 | 真实浏览器在途任务与 tick 的并发时序 |
| 选择器诊断（T32） | 插件侧 node 测试：成功/缺失/陈旧三态；`checkedAt ≠ verifiedAt` 不被混用；**快照键白名单**断言（无 DOM/URL/选择器串）；服务端：受限字段运行时校验、超长或非法载荷被拒、保存后可按安装读回 | 真实页面结构变化时的诊断取值 |
| 版本文案（E10） | 插件 popup 测试：弹窗版本 == `manifest.json` 版本；服务端两条文案含 `MINIMUM_PLUGIN_VERSION` | 真实 Chrome 的扩展页显示（未装包） |
| 端到端可串（S4 退出条件） | 一次 tick 的一步失败可只用引用（run/step/decision/target/work_order + 事件里的 `tick_ref`）串起来，不用累计日志推断 | 一条真实 submission 的全链（需真实平台，未授权） |
| 诊断样例与白名单（S4 退出条件） | 文档给出可脱敏样例与字段白名单（§10.8 落点），并用测试断言事件与快照**只能**含白名单字段 | 脱敏后的样例仍是样例，不是运行证据 |

### 10.8 交接

- **修改文件**：见各次提交的 `git show --stat`；核心新增为 `crates/evidence/src/runtime_readiness.rs`、`crates/evidence/src/runtime_event.rs`、`crates/evidence/src/step_report.rs`（值：三值结局、受限码、`StepFailure` 对照表；从 `scheduler_tick.rs` 拆出，使两者各自在边界上限内）、`collection_scheduler_run_step` 与会话/心跳迁移、`apps/worker/src/{tick,shutdown,keyword_details}.rs`（四步组合入口，`main.rs` 只按顺序装）、`scripts/runtime/{sync,launch,install}.sh`、`apps/api/src/local_web.rs` 的 `/health`；S4c 再加 `crates/evidence/src/selector_health.rs`（受限收口，插件侧同规则一份在插件里）、`0099_collection_selector_health.sql`（一列）、插件 `src/linggan/selectorHealthReport.js`（本机留存 + 报到时带上）、`src/shared/selectorHealth.js` 与 `src/popup/startupRecovery.js`、`apps/api/src/local_web/station_view.rs` 与 `collection_control_surface_view.rs` 的两处版本文案、以及工位版本格第二行的样式 `.c-stn-health`（`apps/api/src/local_web/collection_workspace.css`——只加这一条，语气沿用这张表既有的 `.c-tg-*` 三档，不新增颜色与 token）。
- **验证命令/走查**：`cargo check --workspace --all-targets --locked`；`./scripts/test-local-001-discovery-postgres.sh`（20 目标全套，容器/卷/库自建自清）；插件侧 `node --test tests/xhs-selector-health.test.mjs` 等由 `npm run test:linggan` 覆盖的用例。
- **诊断样例与字段白名单**：落在 `docs/runbooks/local-runtime-deployment.md` 的「运行诊断」一节（样例为**手工构造的脱敏样例**，非运行抓取）并登记到 `docs/README.md`；本计划 §10.4 的表即字段白名单。
- **规则或索引同步**：`docs/progress/2026-09.md` 记本次交付；迁移编号按 §3 的纪律在合并前重新 fetch 复核（本卡预期新增 `0098`/`0099`，撞号则整体顺延并同步六处登记）。
- **例外与替代**：`sync.sh` 开机时「读不到台账则放行启动」**有意保留**——开机时数据库常常还没起来，硬拦会让服务永远起不来；代价由本卡的就绪分级接住（worker 会保持未就绪、不派活），并把该日志行改成如实说明这一点。
- **PR / reviewer / integration owner**：由 Mog 指定；按交付包约定在整包收尾时统一走一次独立复核。

### 10.9 实施状态（S4a–S4d）

实施顺序：S4a 就绪分级与 worker 退出语义（T29）→ S4b 统一 tick、步骤结果与结构化事件（T30，含迁移与部署脚本）→ S4c 选择器诊断与陈旧版本文案（T32/E10）→ S4d 端到端串证与诊断样例白名单（S4 退出条件）。每一步落地后回填本节与 §8 的变异记录。

#### S4a（T29）— 已完成（2026-09-21；本地提交，未推送、未部署、未应用共享库迁移）

- **判据收在一个模块**：`crates/evidence/src/runtime_readiness.rs`——六分类、受限 `detail`（声明序里第一个缺的 migration id / 第一个缺的对象名 / SQLSTATE / `connect_failed` / `probe_timeout`）、判定时刻取自**数据库时钟**（UTC；问不到就是 `None`，不拿本地时间冒充）。只读：不写行、不应用迁移。
- **巡检 worker**：每轮先判就绪。未就绪 → 不跑任何 tick 步骤、按 1→2→4…封顶 60 秒退避重问、只在**状态变化**时说话；连不上数据库不再走上「干净退出（退出码 0）」那条路，也不空转热重启——进程留在原地等它回来。判定本身带 10 秒上限（sqlx 连接池默认 30 秒的耐心不该由操作员等；实测第一行「不可达」原来正好在第 30 秒出现）。
- **模型评论循环不受此闸约束**：它有自己的表，采集面的缺口不该把另一条通道连坐停摆；它的判据是「有没有连接」（连不上时起它只会制造一屋子错误日志）。
- **媒体 worker**：同一条退出语义的最小改动（连不上 → 留在原地按退避重试并说出这件事，不再以 SUCCESS 退出）。
- **`/health` 新增 `readiness`**：与 worker 同一判据（同一份分类码），`database.state`/`scheduler` 语义不变；`SchemaUnavailable` 保留连接，因此能报出确切原因（台账读不到 / 缺哪个迁移），而不是笼统的「schema 不行」。
- **plist 补 `ThrottleInterval=30`**：崩溃循环每半分钟最多一行日志；正常部署走 `bootout`/`bootstrap`，不受此限制。

**证据**（本机隔离 PostgreSQL 16 证明库；不代表 CI 或线上）：

| 命令 | 结果 |
|---|---|
| `cargo test -p linggan-evidence --test runtime_readiness_postgres --locked -- --ignored` | 5 passed（连接不可达 / 台账缺失 / 缺 migration id / 缺表四类注入 + 只读性与不编造时刻） |
| `cargo test -p linggan-worker --test startup_contract --locked` | 4 passed / 1 ignored（两个 worker 各两条：缺地址 → 退出码 1 + `refusing to start`；连不上 → 15 秒观察窗内不退出且说出来；媒体 worker 另断言日志里不出现连接串） |
| `cargo test -p linggan-worker --test startup_contract --locked -- --ignored` | 1 passed（可连、无台账 → `migration_ledger_unreadable`，不冒充空闲） |
| `cargo test -p linggan-api --bin linggan-api --locked` | 253 passed / 25 ignored |
| `cargo test -p linggan-api --bin linggan-api --locked -- --ignored` | 25 passed（含两条新增：`/health` 与 worker 同判、读取面判死时仍说得出确切原因） |
| `cargo check -p linggan-evidence -p linggan-worker -p linggan-api --all-targets --locked` | 通过（warning 均为既有 dead-code） |

**未证明边界**：真实 launchd 下的重启节流行为（本机 plist 未重装，§10.7 同注）；`./scripts/test-local-001-discovery-postgres.sh` 全套尚未整体重跑（新增的三个目标已登记进脚本，S6 串证时跑一次全量）。

**格式：本步交付的文件按当前 rustfmt 格式化，剩余的既有漂移不动**。本步的 9 个 `.rs` 文件逐个用 `rustfmt --edition 2024 --config skip_children=true` 跑过（`skip_children` 是为了只动这些文件本身，不把没碰过的子模块卷进来）。再跑 `cargo fmt --all -- --check`，剩余漂移是 **19 个本步没碰过的文件**（`crates/intelligence/*`、`apps/api/src/local_web/collection_dispatch.rs`、`crates/evidence/src/acquisition_chain.rs` 等），形态都是 `use` 块重排——仓库整体是按更早的 rustfmt 排的。本步**不做**这次全仓格式化（会把无关文件卷进提交）。它影响的是 `scripts/verify-development-environment.sh` 里的 fmt 一步：在本步之前那一步也不通过。要不要另开一次全仓格式化，是一次独立决定。

**挂账（报给 Mog，不在本卡自行处置）**：

1. **媒体 worker 没有自己的就绪要求清单与分级**——本卡只给了它退出语义。它现在「连上就跑」；若它自己的表不在，失败按原样逐次报错。要不要给它同一套分级，是一次范围决定。
2. **API 启动时连不上数据库会一直停在 `DatabaseUnavailable` 直到重启**（它不重连）；这种情况下 `/health.readiness` 报的是启动时的结论，`checkedAt` 为空即表示「问不到时钟」。要不要给 API 加同一条退避重连，是另一次范围决定。
3. **模型循环「不受采集面闸约束」这条决定目前只有代码注释与本记录**，没有测试锚定（进程日志里没有可观察的启动行）；S4b 给心跳加就绪列后它会有可见落点，届时补断言。

#### S4b（T30）— 已完成（2026-09-21；本地提交，未推送、未部署、未应用共享库迁移）

- **一轮 tick 一个 run，四步各一行**：`TickLedger::begin` 先写 run 行与心跳起点（run 行故意不写结局——被杀在半路的那一轮就是 `outcome IS NULL`，「有一轮没跑完」自己看得出来），四步各走 `run_step`：先记「开始了」，跑完补结果。表 `collection_scheduler_run_step`（`0098`）是 run 的**子表**，`UNIQUE (scheduler_run_ref, step_key)`——不是第二套巡检账本。
- **三种结局 + 未完成态**：`ok`（带计数）/ `failed`（受限 `error_class`）/ `skipped`（受限 `skipped_reason`，今日唯一生产者是 `schema_unavailable`：这一步自己的表不在）/ NULL（开始了没收尾）。计数可空，**只有 `ok` 带**：0 是「数过了，是零」，不能拿它冒充「没数过」；这条由数据库的 CHECK 判，不靠写代码的人记得。受限码的字符集写死在 `[a-z0-9_]{1,32}`：报文、连接串、选择器串、页面文本都没有能通过这道 CHECK 的形状。
- **失败隔离**：一步失败只染红那一步，其余三步照常收尾；整轮按「有一步失败就是失败」记账（`tick_outcome` 一个定义两个读者：收轮写行 + worker 发事件），心跳的 `last_error` 只记 `步骤:分类`（如 `media_acquisition:sqlstate_42703`）。
- **结构化事件**：新模块 `crates/evidence/src/runtime_event.rs`——字段闭集（`EVENT_FIELD_WHITELIST`，每条必须有生产者，测试断言集合精确相等）、`reason`/`error_class` 走全仓唯一的受限码转换、`ts` 是**进程时钟**（与就绪判定的数据库时钟分开标注，不互相冒充）、`emit` 写 stdout 一行 JSON。媒体 worker 补上 startup 事件后，白名单里不再有「登记了却没人生产」的常量。
- **worker 组合入口拆薄**：`main.rs` 只按顺序装（172 行，入口点硬上限 200；拆之前 237 行），tick 四步在 `tick.rs`、关停在 `shutdown.rs`、关键词建档在 `keyword_details.rs`。步骤名与受限码只有一处定义。
- **部署脚本**：`sync.sh` 把构建输出另落 `<support>/runtime-build/sync-build.log`——日志主体从这一版起是结构化事件，一行 cargo 警告混进去就要靠猜；失败时抄最后 20 行回服务日志。迁移头按 `ORDER BY migration_id` 取最后一行（写进身份文件）。身份文件**先写临时文件再原子改名**：三个服务会同时启动读它，读到半个 JSON 会报 `unknown`，看起来像「刚部署完却看不出 revision」。`launch.sh` 导出 `LINGGAN_RUNTIME_IDENTITY_PATH`。
- **迁移登记与就绪耦合**：`0098` 登记进 `scripts/local-runtime.sh migrate`（本仓唯一的应用入口）与两处夹具台账哈希，并同时进了 `COLLECTION_RUNTIME_REQUIREMENTS`（迁移 id + 新表名）。含义是**先应用、后部署**：只换二进制不应用迁移，worker 会报 `migration_missing`、保持未就绪、一个 tick 步骤都不跑——不会半写，也不会假装在跑。这条顺序是 S6 的部署与回滚说明必须带上的约束。
- **两个新模块的分工**：`scheduler_tick.rs`（账本：开轮 / 跑一步 / 收轮 / 就绪落点）与 `step_report.rs`（值：三值结局、受限码、`StepFailure` 对照表）。拆分的直接原因是边界检查——一个文件一度 535 行、撞上 500 行硬上限；拆完两个都在限内，`scheduler_tick.rs` 的公开面也回到告警线以下（`run_step` 是跑一步的唯一入口，`step_started`/`record_step` 收为私有：「开始」与「收尾」是配对的两笔，不该由调用方各写一半）。

**证据**（本机隔离 PostgreSQL 16 证明库；不代表 CI 或线上）：

| 命令 | 结果 |
|---|---|
| `cargo test -p linggan-evidence --test scheduler_tick_postgres --locked -- --ignored --test-threads=1` | 4 passed（一步失败其余三步照跑且各有结果 / 自己的表不在算「没轮到」不算失败 / 步骤账本不在就不开轮 / 就绪落心跳而不动 tick 列） |
| `cargo test -p linggan-worker --test startup_contract --locked` | 4 passed / 1 ignored（新增：未就绪时 stdout 上真的有一行可解析的 `readiness` 事件，字段全在白名单内） |
| `cargo test -p linggan-worker --test startup_contract --locked -- --ignored` | 1 passed（连得上、台账不在 → `migration_ledger_unreadable`） |
| `cargo test -p linggan-evidence --test runtime_readiness_postgres --locked -- --ignored` | 5 passed（S4a 的用例在 `bounded_code` 改动后重跑仍绿） |
| `cargo test -p linggan-evidence --lib --locked` | 79 passed（含受限码三条：散文不成码 / 超长标识留前缀 / SQLSTATE 大写折小写） |
| `cargo test --workspace --lib --locked` | 155 passed / 0 failed（19+1+79+56） |
| `cargo check --workspace --all-targets --locked` | 通过（warning 均为既有 dead-code） |
| `./scripts/check-rust-boundaries.sh` | 61 error(s) / 26 warning(s)：全部落在**修前就已超限**的既有文件上（含本步改过、但修前已超限的 4 个）；本步两个新文件均不触发 |

**未证明边界**：真实 launchd 下的重启节流行为（本机 plist 未重装，同 S4a）；`./scripts/test-local-001-discovery-postgres.sh` 全套尚未整体重跑（新增目标 `scheduler_tick_postgres` 已登记，S6 串证时跑一次全量）；真实浏览器在途任务与 tick 的并发时序（§10.7 同注）；**行量**——一 tick 一行 run + 四行步骤 ≈ 每分钟 5 行（约 7 200 行/天），保留与清理策略不在本卡（见挂账）。

**两处如实记录的发现**（都是本步第一次跑到就变红、按缺陷修掉的，不是事后自查）：

1. **`0098` 的三个计数列原本是 `NOT NULL DEFAULT 0`**，与 `TickLedger::record_step` 对失败/跳过写 NULL 直接冲突：失败与跳过的步骤行**根本写不进去**，留下的 `outcome` 是 NULL——也就是「这一步开始了没收尾」，恰好是本卡要消灭的那个东西。修正为三列可空 + 一条 CHECK（`outcome='ok' OR 三个都空`），重算 sha256 并重登记两处夹具（`0098` 当时未提交、只在一次性证明库上应用过，是更正而非改写历史）。
2. **`bounded_code` 原本把任意字符串按字符挑成「码」**：`failed to fetch https://x.test/a?token=secret` 会被过滤成 `failedtofetchhttpsxtestatoken`——一个谁也不曾说过的「原因码」，还把那段文字里的词带进了落盘的事实。单测 `reasons_are_bounded_codes_not_text` 在第一次跑 `--lib` 时变红（此前只跑过 `--test`，这条一直没被执行到）。按断言的意图修实现：只接受字母数字下划线（大写折小写、超长留前缀），其余一律 `unclassified`。

**更正 S4a 记录里的一处读数**：S4a 写的「62 error(s)，均为既有」当时把本步新文件的超限一并计了进去。把新文件的超限消掉之后读数是 **61 error(s) / 26 warning(s)**——那才是既有事实。

**挂账（报给 Mog，不在本卡自行处置）**：

1. **步骤表与 run 表的保留策略**：≈7 200 行/天，本卡不加清理。给 tick 加保留窗口，还是接受持续增长，是产品决定。
2. **S4a 挂账 3（模型循环不受采集面闸门约束缺测试锚定）仍未闭合**：就绪列已按预告落在心跳上，但断言要同时具备「迁移完整、可写心跳的库」与「真的 worker 进程」，今天两个夹具各占一半（worker 的进程夹具只有一个未迁移的库；迁移完整的夹具在 evidence crate 里、没有进程）。补它要新建一个两边都具备的夹具，属独立范围决定。
3. S4a 挂账 1 / 2（媒体 worker 没有自己的就绪分级、API 启动连不上不重连）本步未动，仍开着。

#### S4c（T32 / E10）— 已完成（2026-09-21；本地提交，未推送、未部署、未应用共享库迁移）

- **这条诊断要回答的问题（E10）**：页面结构变了，插件自己最先知道——每次采集前它都会查一遍关键选择器在不在，也知道这些选择器上一次人工重验是哪一天。但这份知识此前**只留在浏览器里**：服务端能看到的只有「这张工单失败了」。于是「页面结构缺了一类信号」与「这台机器网络断了」在页面上长得一模一样。
- **两侧各收一次口**：插件出门前收一次（`src/shared/selectorHealth.js` 的字段闭集 `SELECTOR_HEALTH_SNAPSHOT_FIELDS`，测试逐字断言），服务端进门再收一次（`crates/evidence/src/selector_health.rs`）。不是重复劳动：一侧防「本机把不该出门的东西发出去」（选择器串、DOM 文本、带签名的页面地址在出门前就消失），另一侧防「不管谁发来的都按受限形状收」——上报口在页面上，页面来的东西不可信。
- **收不下＝不写，且留一行日志**：形状不对（连对象都不是／超过 4096 字节／一条认得出的平台记录都没有）整条不收；安静丢掉会让「这台机器一直在报一份我们认不出的东西」在服务端完全隐形，而那正是这类诊断要回答的问题。收不下**不清空**这台安装已有的记录——那次报到什么都没证明，不能拿它去改写已有的结论；`COALESCE($4::jsonb, selector_health)` 就是这句话的写法。**报到本身照常成功**：诊断从来不是报到的前提。
- **词表外写 `unknown`／`unclassified`**：那是「说不出来」，本身是一条如实的事实，不是把垃圾当默认值存下来。检查项名形状不对也不丢——丢掉它等于把「这次有一项检查没过」从快照里删掉。
- **两个时刻分开**：`checkedAt` 是**这次**看的时刻，`verifiedAt` 是这些选择器**上一次人工重验**的日期。谁把它们合成一个，谁就把「刚看了一眼」说成「刚验证过」。
- **本机留存那一侧**：`src/linggan/selectorHealthReport.js` 把快照存进 `chrome.storage.local`——页面会随导航消失、background 会被 MV3 回收，本地存储活得比两者都长。它另维护一份 `failureCounts`：某个检查项**连续**几次检查都是缺的（不是累计：一次缺失说明不了什么，页面还在加载；每次检查都缺才是结构变了；中间查到过就从零重来）。`platform` 是 background 从**发送页面**的地址读出来的，页面自己说的对不上就拒收——免得一个页面替另一个平台写诊断。
- **服务端只加一列**：`plugin_installation.selector_health`（`0099`），`jsonb` + 一条「是对象」的 CHECK。存储层只保证「是个对象」，具体形状由运行时校验负责——**不在 SQL 里抄一份字段清单**：两份清单迟早有一份过期。
- **页面**：工位页「工位版本」格内多一行短标记——`未上报自检`（中性）／`自检无缺失`／`自检缺 N 类`／`自检待重验`（黄），逐项事实（这次检查、选择器验证日期、检查了哪几类、缺哪几类、连续缺失）在**悬停提示**里；两平台各有一条时标记前带平台名。该列只有 84px 宽，容不下一行长句，也不新增区块。**没有在岗安装时这一行不出现**——空缺由「连接」那一列说。`verifiedAt` 只显示到**日**：把时分写出来会让人以为那一刻真发生过一次验证。
- **版本文案（E10 的另一半）**：插件启动失败弹窗此前写死 `v0.4.8`（而 manifest 实为 `0.8.54`），改为**每次渲染时**从 `chrome.runtime.getManifest()` 取，取不到写 `v未知`；服务端两条「版本过低」文案（工位页的提示、采集控制面的恢复动作）改为从**判定用的那个常量** `MINIMUM_PLUGIN_VERSION` 现拼——取值本身（`0.8.47`）没有动，改的只是「说的数字与判的数字必须是同一个」。
- **与原计划的差异**（实施时按实际更正，§10.4 已同步）：去掉 `ruleVersion`（今日它就是 `verifiedAt` 的别名，上报一个同值不同名的字段只会让人以为存在一套独立的规则版本体系）与快照内的 `pluginVersion`（报到自述里本来就有，快照再抄一份就是同一件事的第二个来源）；补上 `checkedCategories`——「这次查过哪几类」与「哪几类缺」是两件事：只查过两类、两类都在，与全查过且都在，不是一个结论。
- **一处边界信号（如实记录，不做处置）**：新模块 `selector_health.rs` 367 行，越过 350 行的**评审线**（硬线 500 未破），其中约 127 行是 6 条受限形状单测。不拆的理由：本卡的证明入口是 `cargo test -p linggan-evidence --lib`（受限收口那 6 条就在这条命令里），把单测挪进 `tests/` 会变成一个新的集成测试目标——而本仓的证明脚本是按目标名逐个点跑的，没登记的目标不会有人跑，反而更容易烂掉。这是一个 warning，不是失败；要不要把「模块 + 同文件单测」一起计入评审线的口径改掉，是仓库规则层面的决定。
- **一处实现取舍**：快照里的两个时刻在 SQL 读取时用 `linggan_human_moment` 渲染成 `YYYY-MM-DD HH:MM`（会话时区）。仓库里没有日期库，而 `0062` 已定下「时刻只有一种人读的写法」；读取侧只重写这两个时刻，其余字段原样带出，**不做二次解释**（快照里存的是什么，读出来就是什么）。
- **收尾时跑了一遍 rustfmt，只动排版、不动语义**：本步碰过的 14 个文件先整份快照到 `/private/tmp/s4c-pre-fmt/`，再就地跑 `rustfmt --edition 2024 --config skip_children=true`，其中 **8 个文件**有改动（`crates/evidence/src/selector_health.rs`、`crates/evidence/src/lib.rs`、`crates/evidence/tests/{collection_control_postgres,collection_control_runtime_postgres,collection_dispatch_sequence_postgres}.rs`、`apps/api/src/local_web/{station_view,collection_control_surface_view,full_schema_fixture}.rs`）。「只动排版」不是靠眼看：先对每个文件取**标识符多重集**（`grep -oE '[A-Za-z_][A-Za-z0-9_]*' | sort | uniq -c`）前后比对，8 个全部相等（换行位置变、词没增没减）；再跑 `cargo check -p linggan-evidence -p linggan-api --all-targets --locked` 通过；最后三套证明原样重跑，结果与重排前逐条一致（见下表）。**顺带收敛了几处早于本步的漂移**：`crates/evidence/src/lib.rs` 里 `pub use step_report` 与 `pub use station_read` 的先后顺序在 HEAD 上就是反的、`0098` 的 `include_str!` 折行在 HEAD 上也是折的——这三处不在本步的编辑范围里，是 rustfmt 顺手排齐的。本仓没有 fmt 门禁（`check-project-governance.sh` 不跑它），所以这条不是「修好了检查」，只是**让本步碰过的文件排版回到工具的标准形**；若不需要这类顺带收敛，撤销它是机械操作。

**证据**（本机隔离 PostgreSQL 16 证明库 + 插件 node 测试；不代表 CI 或线上）：

| 命令 | 结果 |
|---|---|
| `cargo test -p linggan-evidence --lib --locked` | **85 passed / 0 failed**（含受限收口 6 条：只收受限形状且多余字段不跟着走／非对象与超长与「一条都认不出」被拒／词表外写 unknown／两个时刻分开且缺席不冒充「就是现在」） |
| `cargo test -p linggan-evidence --test collection_control_runtime_postgres --locked -- --ignored --test-threads=1` | **14 passed / 0 failed**（新增 `selector_health_is_stored_read_back_and_never_gates_a_check_in`：两个时刻各自按会话时区渲染且互不相等、检查项与连续次数原样读回、第二次上报覆盖第一次、**收不下的与超长的两份报到让这一列逐字节不变**、库里仍只有这一条记录） |
| `cargo test -p linggan-api --bin linggan-api --locked` | **259 passed / 0 failed / 25 ignored**（S4a 时 253；本步新增 6 条：无记录不画成正常／空缺工位不提自检／缺失、陈旧、健康三态互不替代／两个时刻不合并／两处版本文案各一条） |
| `node --test tests/{xhs,douyin}-selector-health* tests/selector-health-notice tests/linggan-selector-health-report tests/linggan-station-check-in tests/linggan-popup-startup` | **36 passed / 0 failed**（含「快照只带白名单字段，绝不带选择器、DOM 文本或签名地址」「连续缺失按检查项计数并被一次查到重置」「页面只能替自己那个平台上报」「报到载荷＝快照＋连续次数，不凭空造平台」） |
| `npm run test:linggan`（插件全套） | **287 条：286 passed / 1 failed**——失败的是**既有**的 `linggan-current-surface-discovery-runtime`（见下） |
| `./scripts/check-rust-boundaries.sh` | **61 error(s) / 27 warning(s)**：61 个 error 全部落在修前就已超限的既有文件上（本步改动过的 `execution_station.rs`、`station_view.rs`、`station_read.rs` 等都在其中，但都不是本步才超的）；27 个 warning 里**有 1 个是本步新增的**——`crates/evidence/src/selector_health.rs` 367 行（评审线 350、硬线 500），主体是 6 条受限形状单测（约 127 行） |
| `cargo check -p linggan-evidence -p linggan-api --all-targets --locked` + 上表三套重跑（rustfmt 之后） | **通过**；三套结果与重排前逐条相同（85 / 259·25 ignored / 14，全 0 failed）——排版改动没有改掉任何一条结论 |

**未证明边界**：真实页面结构变化时的诊断取值（§10.7 同注，需真实平台，未授权）；真实 Chrome 扩展页上看到的版本号（本机未装包，§10.7 同注）；`./scripts/test-local-001-discovery-postgres.sh` 全套尚未整体重跑（`0099` 已登记进脚本与三处测试 `MIGRATIONS` 常量，S6 串证时跑一次全量）。

**如实记录的既有失败（报给 Mog，不由本卡处置）**：插件全套里 `tests/linggan-current-surface-discovery-runtime.test.mjs` 变红，形态是「测试结束后仍有异步活动」引发的 `unhandledRejection`：`TypeError: Cannot read properties of undefined (reading 'offscreen')`（该用例没有 mock `chrome.offscreen`，而 background 的媒体 worker 那条路会去读它）。它与本步改动的文件无关（本步在 `background.js` 里只加了一个消息分支与一个报到字段，都不经过 offscreen），且**在本步之前就存在**；本卡不改它。

**挂账（报给 Mog，不在本卡自行处置）**：

1. **`0099` 没有进 `COLLECTION_RUNTIME_REQUIREMENTS`，因此它是一条「部署顺序约束」而不是就绪判据**——理由：那份清单是**巡检循环**能干活的最小要求（`0034`/`0036`/`0097`/`0098`），而这一列只被「插件报到」这条写路径用到；把它加进去，会让一个诊断列的缺席把三个与它无关的 tick 步骤（媒体投影、渐进档案、关键词建档）一起停掉，正是该模块开头警告的「一个缺口连坐其它还能跑的步骤」。**代价必须写明**：新二进制若跑在没应用 `0099` 的库上，**每一次插件报到都会撞 `42703 undefined_column`**——报到接口把它归到 `422 check_in_rejected`（插件侧只显示「Linggan 本机服务返回 422。」，看不出是哪一列缺了；确切原因只在服务端日志里），页面上工位随之陆续显示失联，而 `/health` 仍会说 READY。因此 S6 的部署与回滚说明必须把 `0098` 与 `0099` 一起写成「先应用、后部署」。要不要给 API 单独一份要求清单（把「报到面」与「巡检面」分开），是一次范围决定。
2. **页面上的验证日期只显示到「日」**：选择器验证日期是人类约定的日期（`2026-04-28`），显示时分没有意义；但若有别的读者需要精确时刻，那是一次产品决定。
3. **「持续缺失」只在本机计数**（连续次数由插件维护、随快照上报）：服务端不重算、不留历史，因此「这个检查项已经连续缺了三天」查不到——要不要在服务端留一份诊断历史，是范围决定。

#### S4d（S4 退出条件）— 已完成（2026-09-21；本地提交，未推送、未部署、未应用共享库迁移）

- **这一步收的是两颗出口条件**（§10.7 最后两行）：「一次 tick 的一步失败可只用引用（run／step／decision／target／work_order + 事件里的 `tickRef`）串起来，不用累计日志推断」，以及「文档给出可脱敏样例与字段白名单（§10.8 落点），并用测试断言事件与快照**只能**含白名单字段」。本步没有 migration、没有插件改动、没有用户可见界面改动。
- **串证用例**：`crates/evidence/tests/scheduler_tick_postgres.rs` 新增 `one_tick_ref_strings_the_failure_and_what_it_queued_without_reading_the_logs`——先**真的坏一处**（把媒体投影要读的那一列改名，这一步拿到的是真实 SQLSTATE，不是构造的错误码），跑一轮 tick，然后**从一个 run 号出发走一条 JOIN**：步骤行（哪一步失败／哪个受限类别）→ 同一轮的目标级决定（排了谁／什么理由）→ 那张工单此刻的队列状态。再断言工单真的挂在这个目标上——否则只是「两条各自成立的记录被摆在一起」。最后取同一份报告发出去的那一行，断言 `tickRef` 与库里那把号是同一个、`stepKey`／`outcome`／`errorClass` 同值、失败行不带计数，且**每一个键都在事件白名单上**——这是运行链上的最后一道口，任何一处「顺手把详情塞进日志」都会在这里露出来。
- **seed 补领域（一处如实记录的根因）**：新用例第一次跑是 `RowNotFound`，根因不在新代码：`0041` 之后**领域是采集的准入前提**，没有归属领域的目标在申请工位之前就被挡下（`target_domain_unassigned`）——这条闸门本身是对的。此前没撞上它，是因为两个夹具的迁移清单不一样：`scheduler_tick_postgres` 用的清单含 `0041`，而 `collection_dispatch_sequence_postgres` 的清单从 `0038` 直跳 `0045`（没有 `0041`，那条闸门在那个库里等于不存在）。seed 改为像建档路径（`store_pending_target`）一样显式写领域，且**领域号从表里读**（`observation_domain` 里 `is_own_domain` 那条），不抄 `0041` 里那串字面量。
- **白名单从「一张清单」变成判据**：`crates/evidence/src/selector_health.rs` 新增具名常量 `SELECTOR_HEALTH_SNAPSHOT_FIELDS`（九个键，从 `lib.rs` 再导出，有生产者、不是死代码）；「只收受限形状」那条用例改为与这张表逐个相等——此前测试里另抄了一份键表，同一件事有两个答案。新增 `nothing_outside_the_field_whitelist_survives_into_the_snapshot`：喂一份夹带选择器串、DOM 文本、带签名的页面地址、`pluginVersion`、`ruleVersion` 与嵌套对象的载荷，断言收下来的键**逐字等于**白名单、那些键一个不剩。它防的是「换一个人发同一样东西」：服务端不因为发送方是自家插件就少做一次收口。
- **worker 那一侧少一个号**：`apps/worker/src/tick.rs` 里的局部 `tick_ref` 变量删掉，`emit_step` 改为**当场问账本要号**（`report.event(ledger.run_ref())`）。原来那个变量有两个去处（发事件与收轮），两处各写各的号时会各自都对、合起来对不上——那正是这条链最要命的错法；手里没有第二个号，就没有传错的机会。
- **文档落点**：`docs/runbooks/local-runtime-deployment.md` 新增 §7「运行诊断：一个 tick 号串一条链」（三个可直接粘的查询 + 三行 JSON，明确标注**手工构造的脱敏样例、不是从运行日志里抓的**）、§7.1 字段白名单（13 个事件字段逐个给含义与边界、9 个快照键、指名两份常量为绑定、点名快照里有意没有 `pluginVersion` 与 `ruleVersion`）、§7.2 这一节不证明什么；原 §7／§8 顺延为 §8／§9（仓库内没有指向旧 §7／§8 的引用；引用 `§3`／`§4` 的两处未动）。`docs/README.md` 的手册行补上这一节。

**证据**（本机隔离 PostgreSQL 16 证明库；不代表 CI 或线上）：

| 命令 | 结果 |
|---|---|
| `cargo test -p linggan-evidence --test scheduler_tick_postgres --locked -- --ignored --test-threads=1` | **5 passed / 0 failed**（S4b 四条 + 新增串证一条） |
| `cargo test -p linggan-evidence --lib --locked` | **86 passed / 0 failed**（85 + 越界载荷一条；「只收受限形状」改为与具名常量比对） |
| `cargo test -p linggan-worker --lib --locked` | 5 passed（媒体 worker 单测；本步未动它，作回归） |
| `cargo test -p linggan-worker --test startup_contract --locked` | 4 passed / 1 ignored |
| `./scripts/check-rust-boundaries.sh` | **61 error(s) / 27 warning(s)**（与 S4c 同数；本步相关的仍是 `selector_health.rs` 一条——428 行，其中 265 行实现 + 163 行同文件单测，硬线 500 未破） |
| `./scripts/check-project-governance.sh` | 通过 |

**未证明边界**：①**样例是描出来的，不是抓来的**——它证明字段表长得对、按它查得到，不证明任何一次真实故障被这样串起来过；一条真实 submission 的全链要真实平台，未授权（§10.7 同注）。②**被串起来的那一轮不是 worker 自己跑的**——用例里的 `run_one_tick` 按与 `apps/worker/src/tick.rs` 相同的形状驱动账本（开轮 → 四步 → 收轮），但 worker 自己那个组合函数没有被它执行到：进程级夹具与迁移完整的夹具今天各占一半（同 S4b 挂账 2），串证用例落在后者。③`emit_step` 的**调用点只有类型保证**（它只能拿到 `&TickLedger` 与 `&StepReport`），没有取值断言；本步删掉局部号变量消除了「手上有两个号」的可能，但「调用点在值上没测」这条边界留着。④`./scripts/test-local-001-discovery-postgres.sh` 全套尚未整体重跑（S6 串证时跑一次全量）。

**如实记录的一处返工**：收尾时对 `crates/evidence/src/lib.rs` 跑了 `rustfmt --edition 2024`——**模块根会把它的 `mod` 子模块一起格式化**，于是本步没碰过的九个模块（`acquisition_chain.rs`、`collection_task_read.rs`、`dispatch.rs`、`execution_input_eligibility.rs`、`runtime_event.rs`、`scheduler_tick.rs`、`work_order_lease.rs`、`work_resource_current.rs`、`patrol_scheduler.rs`）被顺手重排。判定「只动排版」用的是标识符多重集前后相等（`grep -oE '[A-Za-z_][A-Za-z0-9_]*' | sort | uniq -c`），九个模块全部相等；但它们**不是这一步该动的**，因此整份快照到 `/tmp/s4d-rustfmt-collateral/` 后用 `git checkout --` 逐个还原（这九个文件里没有本步的任何改动，不会丢东西），工作树回到本步自己的七个文件。**教训**：本仓没有 fmt 门禁，也不要对模块根跑 rustfmt——那九个模块的既有漂移不是本步的账。

**挂账（报给 Mog，不在本卡自行处置）**：

1. **S4b 挂账 2 的同一条在本步又挡了一次**：串证用例证明的是「账本这条链从一个号出发串得起来」，证不了 worker 的进程真的那样跑。补它要一个同时具备「迁移完整、可写心跳的库」与「真的 worker 进程」的夹具，属独立范围决定。
2. S4c 挂账 1（`0099` 的就绪归属）、S4b 挂账 1（步骤表与 run 表的保留策略）本步未动，仍开着。

**S4 收束**：S4a–S4d 四步全部落地，§10.7 的两行出口条件都有了可复现的证明与如实写明的未证明边界。下一步是 S5（T33–T34，条件 D），再之后是 S6（集成证据表、历史处置预览、兼容矩阵、回滚步骤、PR 候选）。

## 11. S5 变更清单（查询性能基线与热路径改善）

> 依 `docs/design/templates/ui-change-manifest-form.md` 的可迁移骨架填写；本清单写在实施计划里，不另建文档。
> 状态: 活跃计划 · **实施前写入**（本节先于 S5 代码存在；§11.9 在每步落地后回填）。

### 11.1 事项

- **Issue / SCOPE**：COLLECTION-UPGRADE-001 · S5（交付包 `02-实施步骤.md` §S5；验收行 T33 与 T34；退出条件「T33–T34；每项有稳定收益或不实施的证据结论。样本太小则报告未证明收益，不宣传固定百分比提速」）。
- **Agent 与 worktree**：`fix/collection-upgrade-001` @ `.worktrees/collection-upgrade-001`。
- **目标**：为四类热路径（采集页渲染、claim/renew/recovery、detail-gap、媒体 finalize）建立**可比的 SQL 次数／耗时／执行计划基线**，并只对**被测量证明确有稳定收益**的语句动手；量不出来的候选，结论写成「不实施」并留下判据，而不是靠感觉下手。
- **用户可见结果**：采集页刷新与工单认领这两条最常走的路，不再逐行全表扫描租约表与媒体物化表；「媒体已物化」这类事实在查询里被索引直接命中。收益量级如实说明（见 §11.4 的读数），**不承诺任何固定百分比提速**。
- **明确非目标**（本卡不做，也不得顺手做）：
  - **不合并三套 Dexie**、**不合并两个域的材料表**；
  - **不为 `materialization_ref` 主键查询额外造 blob 索引**（主键查询本来就不缺索引）；
  - **不把 `include!` 改语法当作修复**；
  - **不新增统计扩展或全量日志**：不 `pg_stat_reset`、不默认开启全量 SQL 日志、不装 `pg_stat_statements`；统计一律用**带时间窗口的差值**；
  - **不重排其它语句**：候选 ③ 判为「不改」（§11.4）；
  - 不改接纳、权限、额度、调度与清理规则；不改用户可见状态词与文案语义；
  - 不合并/推送/部署、不应用共享库迁移、不重载插件、不发发布包、不访问真实平台。

### 11.2 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| 交付包 `02-实施步骤.md` §S5、`03-验收与发布.md` T33/T34 | 交付包 | 六候选的**实施条件**（不是「都要做」）、禁止清单（不合并 Dexie／材料表、不造 blob 索引、不用 `include!` 语法凑数）、统计纪律（时间窗口差值、不 `pg_stat_reset`、不默认开日志） | 2026-09-21 读 |
| `crates/evidence/src/producer_runtime.rs`（`claim_media_upload_finalize` / 采集页库查询） | 真实代码 | 候选 ①（`download_attempt_ref` 查找）与候选 ④（`row.get`）在同一文件；`materialized` 分支是唯一读该索引的路径，也是该列**唯一**的写入点 | 2026-09-21 读 |
| `crates/evidence/src/work_order_lease.rs`、`crates/evidence/src/dispatch.rs` | 真实代码 | 候选 ② 的调用点：`expire_lapsed_leases_in_transaction` 与 `recover_released_orphaned_work_orders_in_transaction` 的**真实调用者**是 dispatch 的两处 tick 步骤，不是页面 | 2026-09-21 读 |
| `apps/api/src/local_web/collection_control_surface_view.rs`、`collection_control_rule_view.rs` | 真实代码 | 候选 ③（两个 control view）与**候选 ② 真正的量**：`read_frozen_works` 的 LATERAL 子查询逐行扫租约表 | 2026-09-21 读 |
| `crates/evidence/src/collection_control.rs`、`acquisition_chain.rs` | 真实代码 | 候选 ⑥（重复原因表达）的两处：`manual_observe_error_reason` 与 `closed_monitor_reason` 各有一张表，中间夹着一个通配臂 | 2026-09-21 读 |
| `plugins/linggan-intelligence-browser/src/content/douyinBatchMessageHandlers.js` + `tests/` + `package.json` | 真实代码 | 候选 ⑤：模块 684 行，**零** `src/` 引用，不在 webpack content 图内，只有 4 个测试文件 import 它 | 2026-09-21 读 |
| `plugins/linggan-intelligence-browser/MIGRATION-MAP.md` | 责任文档 | 该模块的既有归属判定就是「旧 source 仅作历史保留」——本卡是**执行**这条既有判定，不是新判 | 2026-09-21 读 |
| 运行时库 `linggan_intelligence_dev`（只读窗口差值 + EXPLAIN） | 真实运行事实 | §11.4 的全部读数；本次核查**自己的探针也是计数器污染源**，已按交付包要求排除 | 2026-09-21 量 |
| 一次性证明库 `linggan_cu_proof`（容器 `linggan-cu-proof-pg`） | 隔离证明面 | 代表规模阶梯与前后对比；可丢弃，不影响运行时库 | 2026-09-21 量 |

### 11.3 表面地图

| 表面 | 入口 | 本卡改动 |
|---|---|---|
| 媒体 finalize 写路径 | `/api/local/producer/media-uploads/{session_ref}/finalize` | 候选 ④：把 `row.get` 换成 `try_get` + 一个具名错误，并给路由加一条如实分支；**不改原子写语句** |
| 采集页库查询（封面） | `/collection` 库列表的读取面 | 候选 ①：给 `linggan_media_materialization.download_attempt_ref` 加索引（与其排序键组成复合索引）；**查询语句本身不改** |
| 采集控制面（冻结物） | `/collection` 的待处理/恢复读取面 | 候选 ②：给 `collection_work_order_lease(work_order_ref, issued_at DESC)` 加索引，让逐行的 LATERAL 判断走索引；**不改视图结构与文案** |
| 认领 / 回收 / 过期 | `dispatch` 的两处 tick 步骤 | 候选 ② 的同一索引顺带覆盖回收语句的扫描；**过期语句已有的有效索引保留**（不动） |
| 两个 control view | 同上读取面 | 候选 ③：**不改**（判据见 §11.4） |
| 原因码词典 | 页面回执、恢复动作、巡查关闭原因 | 候选 ⑥：两张表补齐显式分支、去掉通配臂、兜底改成如实的中性词，并加一条往返守卫测试 |
| 插件内容侧 | 打包入口 | 候选 ⑤：把 `douyinBatchMessageHandlers.js` 移入 `tests/historical/` 并写明责任；**打包入口与产物不变** |

页头/导航/共享壳层与 LIDS Token：不改。分页、状态词表与页面区块：不改。

### 11.4 候选与判据（六条，逐条给读数与结论）

测量纪律：统计取自 `pg_stat_user_tables` / `pg_stat_user_indexes` 的**时间窗口差值**（本机 PG 16）；一次核查自己的 EXPLAIN 探针也是计数器污染源，已从归因里排除（交付包「排除核查本身」）。规模阶梯与前后对比在一次性证明库上做，用**与生产相同的语句与参数形状**。

#### ① 媒体物化按 `download_attempt_ref` 查找 —— **实施**

- **现状**：该列只有主键之外的**零个**索引。运行时库上 finalize 的那条语句是 `Seq Scan on linggan_media_materialization`（3071 行被过滤、139 buffers、0.461 ms）。
- **调用频率**（窗口差值折算）：今日 126 次、昨日 174 次、09-15 542 次——约 5～600 次/天，随采集量增长（约 +200/天）。
- **代表规模阶梯**（同参数、各 3 次）：3 千行 0.11–0.40 ms → 30 千行 1.37–3.19 ms → 30 万行 9.6–10.0 ms（转并行顺序扫描）；加索引后同一阶梯为 0.004–0.03 / 0.006–0.03 / 0.005–0.03 ms。
- **同一条列还被采集页的封面子查询用**：运行时库上 3.612 ms、453 buffers，**每渲染一行执行一次**（一页最多 51 行）。忠实复刻表上的归因（每次 / 每页 51 次）：现状 0.624 ms / 20.0 ms；只加被点名的物化索引 0.444 / 15.1；只加两个兄弟索引 0.264 / 6.6；三个都加 0.102 / 2.2 ms。
- **结论**：**实施**——加 `linggan_media_materialization (download_attempt_ref, verified_at DESC)`（第二个键就是语句里的排序键，让 `ORDER BY … LIMIT 1` 直接走索引首行）。
- **如实说明收益量级**：今天的绝对节省是**亚毫秒到几毫秒**（当前 3 千行）；真正的理由是**形状**——这条语句的成本随库增长**线性上升**，而索引版几乎不随规模变。所以本卡**不写**「提速 N%」。

#### ② 租约高频扫描 —— **实施**

- **现状**：运行时库上 `collection_work_order_lease` 累计 `seq_scan = 166,694`、`seq_tup_read = 148,405,174`——是全库顺序扫描量最大的表。
- **窗口差值**（4 分 23 秒）：租约表 `seq_scan +4796`、`idx_scan +94`。脉冲式：某一分钟 +958，某 20 秒样本 0。
- **归因**：候选表把它记为「lease 高频扫描」，但真正的语句**不在** `work_order_lease.rs`，而在采集控制面的 `read_frozen_works`——它的 LATERAL 子查询对每行取最近一条租约。运行时 EXPLAIN 实测：`Seq Scan on collection_work_order_lease candidate_1 (actual rows=1 loops=100)`，即**每次页面渲染扫 100 次**；4 796 ÷ 100 ≈ 48 次渲染/窗口 ≈ 11 次/分钟。
- **前后对比**（证明库复刻表，同一语句同参数）：改前每次查询 100 次顺序扫描 / 1000 buffers / 3.403 ms，11 次重复 29.224 ms；加索引后 283 buffers / 0.637 ms，11 次 4.313 ms（**6.8×**）。
- **这个 6.8× 的适用范围（同 ① 的纪律，不当作线上提速宣传）**：它是**复刻表在复刻规模下**的同参数前后对比，不是生产提速承诺。运行时库上这条语句的绝对耗时本来就小；收益的形状和 ① 一样——**每次渲染扫 100 次、且随表增长线性上升**，索引版把它压成常数级。样本与规模都不足以支撑一个百分比结论，所以本卡只报「形状改变 + 缓冲页数下降（1000 → 283）」，不报「线上快 6.8 倍」。
- **为什么既有的两条索引都服务不了它**（这是本条成立的**关键**，复核必问）：租约表原有两条索引，都是**部分索引**——`_live_idx`（唯一，`ON (work_order_ref) WHERE released_at IS NULL`）与 `_station_idx`（`ON (station_ref, expires_at DESC) WHERE released_at IS NULL`）。部分索引只能服务**谓词蕴含它**的查询，而 `read_frozen_works` 的 LATERAL 里**没有** `released_at IS NULL`——它要的是「最近一份租约，不论释放与否」，因为同一行还要据 `released_at`／`expires_at` 把状态分成 `not_issued`／`released`／`expired`／`live` 四档。所以它只能退化成顺序扫描。**同表另一条 LATERAL 则相反**：`live_patrol_execution_in`（`collection_control.rs:2263`）的谓词里**有** `released_at IS NULL AND expires_at>scope_001_now()`，它正好被 `_live_idx` 服务——那一条**不需要**新索引，本卡也不动它。两条 LATERAL 长得像、判据不同，一处差别就是这条索引存在的全部理由。
- **同一索引的顺带效果**：回收语句（`recover_released_orphaned_work_orders_in_transaction`）的顺序扫描一并消失（0.085 → 0.042 ms）；过期语句**继续用它已有的** `collection_work_order_lease_station_idx`（有效索引保留，不动）。
- **结论**：**实施**——加 `collection_work_order_lease (work_order_ref, issued_at DESC)`。
- **一条被量掉的想法（不实施）**：给 `collection_work_order(queue_state='leased')` 建部分索引，实测 0.024 ms，放大 100 倍后收益约等于 0——**证据不支持，不做**。

#### ③ 两个 control view 中的 SQL —— **不改**

- **判据**：交付包给的条件是「与统一状态读取重复，或使错误边界不清」。
- **核对的结论**：S3 建立的统一读取（`read_material_execution_states`、`read_detail_delivery_reconciliation`、`read_collection_task_timeline`）服务的**对象不同**——它们答的是「材料执行到哪一步、详情交付对没对上、任务时间线」，而 control view 答的是「这一轮调度决定了什么、哪些工单被冻结、运行时资源能不能接活、当前生效规则与回执」。二者没有同一条事实被写两遍。
- **确实重叠的那一处，已经在用统一读取**：容量（`read_capacity`）本来就是 `acquisition_chain` 的读口，control view 调用它，没有第二份实现。
- **错误边界**：视图内读取统一用 `try_get`（失败即报错，不静默取默认值），与本次要求一致。
- **结论**：**不改**，并把理由留在本节（供下次复核，不必重查）。
- **一处需要写清楚的交叉**：候选 ② 要改的那条语句**住在**这两个 view 之一的里面。这不矛盾——② 改的是**一条语句的索引命中**（交付包条件是「真实调用频率与代表规模证明稳定收益」），③ 判的是**要不要把这些 SQL 搬进已有读接口**（条件是「重复或边界不清」）。两条各自按各自的条件判。

#### ④ `row.get` —— **实施**

- **风险面**：`linggan_media_upload_session.download_attempt_ref` 可空，且**没有** CHECK 把它与 `state='materialized'` 绑在一起；`materialized` 分支用**会 panic 的** `row.get::<Uuid, _>` 读它。
- **为什么是「潜伏」而不是「正在发生」**：该列全仓只有一个写入点（同文件的原子 `UPDATE … SET state='materialized', download_attempt_ref=$2`），它保证了两者同时成立。所以今天读到的都是非空——但这条保证**只存在于代码约定里**，任何一次越界写入就会把 panic 带给正在处理请求的进程。
- **结论**：**实施**——该处改 `try_get`，新增一个具名错误分支，并在 `finalize_media_upload_route` 加一条显式分支把「物化会话缺下载尝试引用」如实报出去。**不做全仓机械替换**（交付包明确禁止）：只改本次触及的热路径。

#### ⑤ `douyinBatchMessageHandlers` —— **实施迁移（不删除）**

- **生产引用的证明**：模块内零个 `src/` 引用；不在 webpack 的 content 入口图里；`scripts/verify-linggan-isolation.mjs` 早已断言 content 入口**不得**提到它；`MIGRATION-MAP.md` 对它的既有判定就是「旧 source 仅作历史保留」。
- **谁还在用**：4 个测试文件（`douyin-batch-remote-startup`、`douyin-batch-ui-routing`、`manual-execution-lock-release`、`douyin-batch-summary`）与 `task-state-constants` 里的两条路径条目。
- **结论**：**实施**——移入 `tests/historical/` 并附一份说明它为什么在那里的 README（治理规则：默认保留并降级旧材料，不静默删除），同步改 4 处 import 与 2 处路径条目，模块自己的 4 条相对 import 随目录深度调整。`package.json` 的测试脚本不变（用例仍留在 `tests/`）。
- **T34 的加强证明**：不止 grep `src/`，还要**构建产物**里搜不到这个名字（§11.7）。

#### ⑥ 重复原因表达 —— **实施**

- **分歧的确切集合**：`acquisition_chain` 会产出的下面 7 个码，在 `closed_monitor_reason` 的白名单里**没有**，于是全部被兜底臂渲染成「数据库不可用」：`available`、`in_flight_work_covers_it`、`need_already_satisfied`、`question_unanswerable`、`queueable`、`target_domain_unassigned`、`within_authorization`。
- **第二处分歧**：`manual_observe_error_reason` 的通配臂把 7 个 `AcquisitionChainError` 变体一并藏进「数据库不可用」。
- **一处具体的自相矛盾**：`target_domain_unassigned` 由一个函数如实产出，重放时被另一个函数改写成「数据库不可用」——同一条事实在两条路径上说法不同。
- **结论**：**实施**——两张表补成显式分支、去掉通配臂，把兜底从「数据库不可用」改成**如实的中性词**（说不出原因就说「说不出原因」，不猜一个具体故障），并加一条守卫测试：产出侧会发出的每个码，穿过关闭词表后必须逐字不变。**分领域**集中（交付包禁止混成全项目万能枚举），不合并成一张大表。
- **实施时发现的第三份副本（本卡最重要的发现，也是本卡范围扩大的唯一原因）**：这份词表在本仓库里有**三个**写处，不是两个。除了代码侧 `MONITOR_COMMAND_REASONS` 与两个渲染函数，`0065` 还给两张回执表各建了一条 `CHECK` 约束，把同一份词表**手抄进了数据库**。手抄会分头长大：`0065` 之后代码侧陆续加的十几个码，两条 `CHECK` 一个都没跟上。后果不是「显示得不好看」——`finish_monitor_command` 把原因码**原样**绑进两条 INSERT，码一旦不在 `CHECK` 里，Postgres 直接 `23514`，**这条拒绝连一张回执都留不下**。比写一个错的说法更坏。
- **它已经能在线触发**：`manual_observe_error_reason` 有一个显式分支专门产出 `target_domain_unassigned`（注释就写在那条分支上方，说的是「缺领域必须单独报」，因为以前它被说成「数据库不可用」），而这个码**不在**两条 `CHECK` 里。对一个还没指定领域的目标点「立即观察」，就是在写回执这一步整笔失败。**如实划界**：这条路径能触发是**读代码读出来的**（该码的产生由显式分支保证），「码不在 CHECK 里就写不进去」是**实测的**（本地看到 `23514`），**未证明**的是它在生产上被触发过——今天没有任何用例走过这条路径，这正是它能一路发布出来的原因。
- **修法与为什么是新开一条迁移**：migrations 是 append-only 的，`0065` 已在共享库与所有证明库上应用过，改它对已应用的环境不生效，只会让「仓库里的 0065」与「库里的 0065」变成两个东西。所以新开 `0101`，只把两条 `CHECK` 补成代码侧的完整词表。两份新 `CHECK` 都是旧的**超集**，因此不可能与任何既有行冲突。
- **防复发的那道闸不在 SQL 里**：`collection_control.rs` 的单元用例扫全部迁移，取这两条约束的**最终**定义逐码比对（两侧各变异验证过一次，见 §8）。它跑在 `--lib` 里，不用等一次真实的拒绝写不进去才发现：代码侧加一个码而没有配套迁移，那条用例直接变红。**它挡不住的**是「某条迁移把约束单独 DROP 掉而不重建」——那种改动会让这道闸和数据库那道闸一起消失，只能靠复核看 diff；这条限制写在用例自己的注释里，不靠本节记。
- **一处有意的不对称**：身份表不收 `identity_conflict`（身份行记的是一条命令的**首条**结果，身份冲突按定义不可能发生在第一条，写入点用 `unreachable!` 表达了同一件事）；回执表收。`reason_not_recognized` 两张都收——它的用途是**回滚**：新二进制按新词表写下的码，回滚后旧二进制不认识，读出时收敛成这个中性词；要能写回去，`CHECK` 就必须收它。
- **本次范围扩大的一处决定**：交付包原本写的「本卡新增 `0100`」，实施后是 `0100` + `0101`。这不是顺手多做——⑥ 的验收（「产出侧的码不再被改写」）在 `0101` 应用之前**根本不成立**：改了代码反而让以前能写进去的路径变成写不进去。所以两条迁移与 ⑥ 是一个不可分的单位，要撤就一起撤。已在 §11.6 记为 `DECISION_REQUIRED`，报 Mog。

#### 未纳入本卡的相邻发现（报给 Mog，不自行实施）

采集页封面子查询里还有两个**兄弟缺索引**：`linggan_media_observation(slot_key)` 与 `linggan_media_download_attempt(media_observation_ref)`。按 §11.4 ① 的归因读数，它们各自省下的时间**比被点名的那个索引更多**（只加两个兄弟索引 → 0.264 ms，只加被点名的 → 0.444 ms，三个都加 → 0.102 ms）。它们**不在**交付包的六候选里，因此本卡**不做**，只把读数与建议报给 Mog 决定是否另开一卡。

同一族、本卡同样**不做**的三条（都报给 Mog，不自行实施）：

- **`complete_media_upload` 会静默什么也不做**：它只在会话处于 `finalizing` 态时落 `materialized`（`producer_runtime.rs:523` 的 `WHERE … AND state='finalizing'`），其它状态一律**返回 `Ok(())` 且不改变任何东西**。调用方因此分不出「已经物化了」和「状态不对，什么都没发生」。核查时正是先漏了「先领终结权」这一步，才看到它悄悄留在 `ready_to_finalize`。**未证明**这造成过线上后果——本卡只是把调用顺序写进用例注释，不改这个函数（它被插件链路依赖，改语义超出本卡）。
- **原因码会原样出现在页面上**：`collection_control_rule_view.rs:607` 把 `reason_code` 直接交给界面，而这份词表里有相当一批是英文机器码（`target_domain_unassigned`、`in_flight_work_covers_it` …）。这是**用户可见文案**问题，不是 ⑥ 要修的「同一个事实两种说法」问题，且已经超出本卡「不改用户可见状态词与文案语义」的边界。
- **同一份词表还有两份手维护的部分映射**：`receipt_meaning` 与 `error_summary` 各自对 55 个码做截断式翻译（覆盖不全是设计使然，缺的部分走兜底）。它们与 ⑥ 是同族风险（手抄会分头长大），但交付包只点名了那两个函数。

### 11.5 依赖地图

| 依赖 | 归属 | 本卡的用法 |
|---|---|---|
| `linggan_media_materialization` | 已交付 | 只**加一条索引**；列、约束、写入语句都不动 |
| `linggan_media_observation` / `linggan_media_download_attempt` | 已交付 | 本卡**不动**（兄弟缺索引，见 §11.4 末） |
| `collection_work_order_lease` | 已交付 | 只**加一条索引**；过期语句既有的 `_station_idx` 保留不动 |
| `collection_work_order` | 已交付 | 本卡不动（被量掉的部分索引想法不实施） |
| 两个 control view | 已交付 | 语句不改；只让其中的 LATERAL 走索引 |
| `AcquisitionChainError` / 原因码 | 已交付 | 不新增码、不改语义；只让两张呈现表**不漏**已有码 |
| `collection_monitor_rule_command_identity.first_reason_code` 与 `collection_monitor_rule_command_receipt.reason_code` 的 `CHECK` | 已交付（`0065` 建） | **本卡修正**：两条 `CHECK` 是同一份词表的第三份手抄副本，`0101` 把它们补成代码侧的完整词表（只放宽、不收紧）。列与写入语句不动 |
| 插件打包入口与产物 | 已交付 | 不变（候选 ⑤ 只挪测试侧文件） |
| `linggan_local_schema_migration` | 已交付 | **本卡新增两条**迁移（`0100` 索引、`0101` 词表），各按既有六处登记 + 两处夹具摘要，先应用后部署 |

**未纳入本卡**：兄弟缺索引（§11.4 末）、三套 Dexie 合并、两域材料表合并、统计扩展安装、共享库迁移应用与历史处置（S6 才准备）。

### 11.6 变更分类与影响边界

- **分类**：查询性能（索引）+ 错误边界（一处 `try_get`）+ 呈现词典（原因码）+ 测试侧文件归属。不改产品语义、不改权限与后果、不新增可点动作。
- **最高风险类别**：**migration**（本卡新增**两条**：`0100` 索引、`0101` 词表）。两条的风险形状不同：
  - `0100` 只影响执行计划，不改变任何判定。风险在①写入成本——索引会给媒体物化表与租约表各加一笔写开销，租约表写频率高，需如实观察；②部署顺序——新二进制不认识没应用的 `0100` 也能跑（索引只影响计划，不产生 `42703`），所以 `0100` 与 `0098`/`0099` **不同**：它不在就绪判据里，但 S6 的部署说明仍把它与其它待应用迁移**一起**写成「先应用、后部署」，避免同一批出现两种顺序。
  - `0101` **是**部署顺序约束，且方向与 `0100` 相反：它放宽的是**新二进制才会写出的码**的落库门槛。新二进制先上线、`0101` 后应用，则「按新词表说出名字的拒绝」仍然写不进回执。所以 `0101` 必须与代码**同一批先应用**；反过来，旧二进制不认识的新码在回滚后读出时收敛成 `reason_not_recognized`，该码两张 `CHECK` 都收，回滚方向是通的。
  - 两条迁移都只放宽/新增，**不收紧**：`0100` 加索引不改任何行的合法性，`0101` 的两份新 `CHECK` 都是旧的超集，不可能让已写下的行变成非法。
- **是否存在 `DECISION_REQUIRED`**：
  - **本卡范围扩大：`0101` 是计划外的第二条迁移**——交付包写的是「本卡新增 `0100`」。`0101` 不是顺手多做，而是 ⑥ 成立的**前提**（理由见 §11.4 ⑥ 末）。它与 ⑥ 是一个不可分的单位，要撤就一起撤。需要 Mog 确认这条扩大是否接受；不接受就是**整个 ⑥ 撤回**并保留 `0100`，不能只留代码不留迁移——那只留会把拒绝变成 `23514` 的坏处。
  - **兄弟缺索引是否要一并做**——本卡不做，报 Mog（§11.4 末）；
  - **租约表的写开销与保留策略**——索引给写路径加成本，而租约行只增不减（S4b 挂账 1 同类的保留问题），是否设保留窗口是产品决定；
  - **`closed_monitor_reason` 兜底改成中性词后页面上少一个具体说法**——这是**如实**（原来那个说法是错的），但会让某些历史回执的表述变化，属呈现变化，本卡按实修改并记录。
- **L1 / L2 / L3**：L1 查询与索引；页面侧**零**结构变化，只在既有文案槽内换词。
- **是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth**：都不触及。
- **禁止修改的文件/能力**：接纳/派发写路径的判定与停止语义、授权与额度规则、`0097`/`0098`/`0099` 台账、`local-runtime.sh migrate` 的应用顺序、`runtime-main`、共享库、三套 Dexie、两域材料表、`materialization_ref` 的主键查询、`0065`（已应用，不得改写，见 §11.4 ⑥ 末）。
- **停止条件**：若新索引在真实运行时被计划器**绕过**（例如统计未更新导致仍选顺序扫描），不追加更多的索引或强制 `enable_seqscan`；保留已证实的计划对比，把分歧报给 Mog。

### 11.7 验收矩阵

| 层级 | 验收方法 | 未证明边界 |
|---|---|---|
| 基线可复现（T33） | 证明库上以**同参数**跑规模阶梯（3 千 / 3 万 / 30 万）与两条语句的前后计划，断言 `EXPLAIN` 计划从 `Seq Scan` 变为索引命中、且缓冲页数下降；结论按**计划与规模曲线的形状**给出，不按单次扫描累计差值 | 真实运行时库在真实并发下的计划选择（本卡只在只读窗口内取样） |
| 时间窗口差值（T33） | 运行时库上取窗口快照（`pg_stat_user_tables` / `pg_stat_user_indexes`，只含 `public`），记录租约表与物化表的 `seq_scan`/`idx_scan` 差值；**排除核查自身的探针** | 窗口内如果一次流量都没有，差值就是 0；样本小则如实写「未证明收益」 |
| 候选 ① / ② 索引（迁移 `0100`） | 隔离 PostgreSQL 全套（`./scripts/test-local-001-discovery-postgres.sh`）在应用 `0100` 后全绿；夹具台账含 `0100` 摘要 | 线上规模（本机库只有 3 千行量级） |
| 候选 ④ 错误边界 | 用例构造「`state='materialized'` 但引用为空」的行（**故意绕过写入点**），断言得到**具名错误**而不是解码 panic；变异验证：把 `try_get` 改回 `get`，用例以 `ColumnDecode … UnexpectedNullError` panic 变红（证明它钉住的正是那个差别，不是一条恒绿的断言） | **证据层已证，HTTP 层未证**：`finalize` 路由的 4xx 分支没有用例——那条路由今天一条 HTTP 级用例都没有（相邻的 `media_upload_session_not_found` 等分支同样没有）。新分支按相邻分支的同一形状写，属**读代码验证**，如实记在 §11.9.2 |
| 候选 ⑤ 无生产引用（T34） | 三证：`src/` 零引用；webpack content 图不含该模块；**构建产物**（`dist/`）里搜不到该名字 | 真实 Chrome 的扩展加载（本卡不装包，未授权） |
| 候选 ⑥ 词典不漏 | 单测：产出侧每个码穿过关闭词表后逐字不变；两个函数的通配臂被去掉后仍有兜底（中性词），且不再是「数据库不可用」；**隔离库实证**：一条 `schema_unavailable` 的拒绝真的留下了回执（`0101` 之前这一步是 `23514`，一条都留不下） | 历史回执文本会随之变化（已在 §11.6 记为呈现变化） |
| 词表第三份副本不再分头长大（`0101`） | 单测扫**全部**迁移、取两条 `CHECK` 的**最终**定义，与代码侧词表逐码比对（不是只查创建它的那个文件）；两侧各变异验证一次：代码侧加码 → 红，迁移侧漏码 → 红 | **挡不住**「某条迁移把约束 DROP 掉而不重建」——那种改动会让两道闸一起消失，只能靠复核看 diff（限制写在用例自己的注释里） |
| 全模块集成与构建（T34） | `cargo check --workspace --all-targets --locked` + 插件 `npm run build` + 产物 grep + 既有 verify 脚本；断言「活跃插件入口、真实 Rust 查询、API 与视图覆盖；无仅测试调用的假链路」 | 真实平台与线上运行（未授权） |

### 11.8 交接

- **修改文件**：见各次提交的 `git show --stat`；核心是 `database/migrations/0100_*.sql`（两条索引）、`database/migrations/0101_*.sql`（两条 `CHECK` 补成完整词表）、`crates/evidence/src/producer_runtime.rs`（`try_get` + 具名错误）、`apps/api/src/local_web.rs`（finalize 路由一条如实分支）、`crates/evidence/src/collection_control.rs`（两张原因码表 + 两条守卫测试）、`crates/evidence/tests/local_producer_postgres.rs`（候选 ④ 的攻击性负例）、插件 `src/content/douyinBatchMessageHandlers.js` → `tests/historical/`（含 README）与 4 个测试文件的 import、`MIGRATION-MAP.md` 里 2 处路径条目。
- **迁移 `0100` 的登记点（六个，逐一核过）**：新迁移文件本身之外，`scripts/local-runtime.sh:274` 的 `apply_migration_once`；`crates/evidence/tests/support/material_fixture.rs:215` 与 `apps/api/src/local_web/full_schema_fixture.rs:194` 的 `include_str!`，这两处**另各带一行台账摘要**（`…:250` / `…:255`，同为 `a26c7006…`）；`crates/evidence/tests/{collection_control_postgres:156, collection_control_runtime_postgres:146, collection_dispatch_sequence_postgres:153}.rs` 的 `include_str!`（**不带**台账摘要）。核对方式：`grep -rln "0100"` 全仓扫过，`crates/contracts/src/collection.rs` 的命中是一个含 `0100` 的测试用平台 ID，不是登记点；**上一版这一行列的四处是错的**（把 `runtime_readiness.rs` 与 `scripts/test-local-001-discovery-postgres.sh` 当成了登记点）——前者只登记 tick 需要的 `0034/0036/0097/0098` 四个 id，后者按目录扫迁移、不逐条登记。列错登记点的代价是下次改迁移时按这行去改，改了不生效还以为登记过了。
- **迁移 `0101` 的登记点（六个，逐一核过）**：与 `0100` 同构、同一批核对，行号各差一行：`scripts/local-runtime.sh:275` 的 `apply_migration_once`；`crates/evidence/tests/support/material_fixture.rs:216` 的 `include_str!` 与 `:252` 的台账摘要；`apps/api/src/local_web/full_schema_fixture.rs:195` 的 `include_str!` 与 `:257` 的台账摘要（两处摘要同为 `fda3711ea7bb38af6bb5a6a28264a39ac0c04024aef3e6feeb931e07a9b074c1`，与文件实际 sha256 一致，已核）；`crates/evidence/tests/{collection_control_postgres:157, collection_control_runtime_postgres:147, collection_dispatch_sequence_postgres:154}.rs` 的 `include_str!`（**不带**台账摘要）。核对方式同 `0100`：全仓 `grep -rn "0101"` 逐条看过，命中的都是登记点或说明性注释。
- **验证命令/走查**：`cargo check --workspace --all-targets --locked`；`cargo test -p linggan-evidence --lib --locked`；`cargo test -p linggan-api --bin linggan-api --locked`；`./scripts/test-local-001-discovery-postgres.sh`（全套，**读退出码与 log 的 `test result` 行，不看管道尾部**）；插件 `npm run build` + 受影响的五个用例文件 + 产物 grep；`./scripts/check-rust-boundaries.sh`；`./scripts/check-project-governance.sh`。
- **规则或索引同步**：`docs/progress/2026-09.md` 记本次交付；迁移编号按 §3 的纪律在合并前重新 fetch 复核（本卡实际新增 `0100` + `0101` 两条，撞号则整体顺延并同步全部登记点）。
- **例外与替代**：候选 ③ **有意不改**（判据在 §11.4），兄弟缺索引**有意不做**（报 Mog）。两处都写明了理由，不是遗漏。
- **PR / reviewer / integration owner**：由 Mog 指定；按交付包约定在整包收尾时统一走一次独立复核（不再为 S5 单独发起）。

### 11.9 实施状态

**已完成（2026-09-21；全部是本 worktree 内的本地状态）**。实际顺序：迁移 `0100` 与六处登记 → 候选 ④ → 候选 ⑥（**在实施中扩出第二条迁移 `0101`**，理由见 §11.4 ⑥ 末与 §11.6 的 `DECISION_REQUIRED`）→ 候选 ⑤ → 集成与构建证据 → 回填本节。**除 `0101` 外没有偏离计划的其他改动**；候选 ③ 与兄弟缺索引仍是有意不做。

#### 11.9.1 证据表（命令 + 真实结果）

| 证据 | 命令 | 结果 |
|---|---|---|
| 基线可复现（T33） | 证明库规模阶梯（3 千／3 万／30 万，同参数各 3 次）与运行时库**时间窗口差值**，读数在 §11.4 ①② | 计划对比：①`Seq Scan` → 索引首行（3 千 0.11–0.40 ms → 0.004–0.03 ms）、②每次渲染 100 次顺序扫描 → 0.637 ms／283 buffers；**只报形状改变与缓冲页数下降，不报线上百分比** |
| 隔离库全套（T34 集成面） | `./scripts/test-local-001-discovery-postgres.sh` | **24 次 cargo 运行：252 passed / 0 failed / 1 ignored，退出码 0**，容器/卷/库自建自清（日志末尾 `LOCAL-001 PostgreSQL proof cleanup verified`）。关键目标：`collection_control_runtime_postgres` **14 passed / 0 failed**（`0101` 的证明——`schema_unavailable` 那条拒绝此前是 `23514`，一条回执都留不下）、`collection_control_postgres` 22、`collection_dispatch_sequence_postgres` 34、`local_producer_postgres` **6 passed**（候选 ④ 的攻击性负例）、`scheduler_tick_postgres` 5、`runtime_readiness_postgres` 5、`linggan-api` 二进制 25 passed / 259 filtered out |
| 单元面 | `cargo test -p linggan-evidence --lib --locked` | **90 passed / 0 failed**（新增守卫一条，89 → 90） |
| API 二进制（单独跑一次） | `cargo test -p linggan-api --bin linggan-api --locked` | **259 passed / 0 failed / 25 ignored** |
| 全仓编译 | `cargo check --workspace --all-targets --locked` | 退出码 0 |
| 插件构建（T34 产物证） | `npm run build` | 退出码 0（3 条既有体积警告）；**产物三证**：`src/` 零引用、webpack content 图不含它（`verify-linggan-isolation.mjs:67` 本来就断言这件事）、新建的 `dist/` 里搜不到模块名（命中 **0** 个文件） |
| 插件用例与校验 | `npm run test:douyin`；两条**没有 npm 脚本入口**的用例直接跑；`check:contracts`；`verify:linggan-isolation`；`verify:content-runtime` | 69 passed / 0 failed；3 passed / 0 failed；三者退出码 0 |
| 边界与治理 | `./scripts/check-rust-boundaries.sh`；`./scripts/check-project-governance.sh` | 前者 **61 error(s) / 27 warning(s)**（与 S3c/S4c/S4d 逐项同数：本卡碰过的 `collection_control.rs` 与 `producer_runtime.rs` 本来就在那 61 个里，该检查按文件计、只报一次，所以往已超限文件里加的行不会再新增计数——这意味着**这份检查证明不了「本卡没有新增超限行」**，只证明没有新增超限**文件**）；后者**通过** |
| 变异验证 | 见 §8 末尾三条 | 三次全红、三次逐字节还原（`collection_control.rs` `0e68c49c…`、`producer_runtime.rs` `b8266df2…`、`0101` `fda3711e…`） |

#### 11.9.2 未证明边界（如实）

- **T33 的收益只在复刻规模上量过**：证明库的规模阶梯与运行时库的窗口差值都不是生产并发，运行时库上这条语句的绝对耗时本来就小；样本与规模都不足以支撑一个百分比结论，所以本卡**不宣传任何固定提速**。这正是交付包「样本太小则报告未证明收益」那条。
- **`0101` 只在隔离库与单元面上证明**：共享库未应用（未授权），因此「线上那两条 `CHECK` 的确切定义」这一步没有实测——本卡读的是 `0065` 的文本与 `0101` 的最终态。
- **候选 ⑤ 的「没有生产引用」止于构建产物与入口图**：真实 Chrome 加载扩展未做（未授权）。
- **候选 ④ 的新 4xx 分支没有 HTTP 级用例**：`/api/local/producer/media-uploads/{session_ref}/finalize` 这条路由今天**一条** HTTP 级用例都没有（相邻几条错误分支也一样），隔离库那条用例证到的是**证据层**的具名错误。分支本身按相邻分支的同一形状写（同样的 `local_producer_error` + 状态码），属读代码验证。
- **全套隔离库跑在本机 PostgreSQL 16 镜像上**：与部署机版本的差异未测。
- **一件与本卡无关但会让 `npm run verify` 整体变红的事**：`release:verify` 失败（`releases/linggan-intelligence-browser-v0.8.54.zip` 的 `background.js`／`content.js` 与新建的 `dist/` 不一致）。已实证**与本卡无关**——把候选 ⑤ 迁移出去的那个模块临时放回 `src/` 重新构建，产物**逐字节相同**（`background.js` `26cd650f52a9`、`content.js` `3d369914321c`），即该模块进出构建图对产物零影响；建完即刻删除并重建，工作树回到本卡状态。**未证明**它到底是「发布包本身过期」还是「本机 webpack 与打包时版本不同」；本卡不重建发布包（未授权），只报这一条。

#### 11.9.3 如实记录的两处返工（候选 ④ 的用例）

- 第一版用了**随手编的**观察对象 UUID（`eeeeeeee-…`，它在包里是 `packageRef` 而不是 `observationRef`），`begin_media_upload` 第一步查存在性就以 `MediaObservationNotFound` 死在开头——用例变成在测别的东西。改用包里真的建出来的那个（`11111111-…`）是本条的修法，注释里写明了为什么不能用编的。
- 改对之后又暴露第二处：漏了「先领终结权」这一步，而 `complete_media_upload` 在**非 `finalizing` 态上返回 `Ok(())` 却什么也不做**，于是前提断言停在 `ready_to_finalize` 上。两处都写进了用例注释，并把「静默无操作」这条同族发现报给 Mog（§11.4 末）。

#### 11.9.4 挂账与报 Mog（不自行处置）

①`0101` 是否接受这次范围扩大（不接受即整个 ⑥ 撤回，见 §11.6）；②`target_domain_unassigned` 这条**改动之前就在**的雷（同一个领域里另有别的码也可能撞上那两条 `CHECK`）；③`reason_code` 会原样出现在页面上；④`receipt_meaning`／`error_summary` 是同一份词表的另两份手维护映射；⑤采集页两个兄弟缺索引；⑥`complete_media_upload` 的静默无操作；⑦`manual-execution-lock-release.test.mjs` 与 `task-state-constants.test.mjs` 没有 npm 脚本入口（本步直接跑过，3 passed）、`release:verify` 的既有红灯、以及既有的插件 `linggan-current-surface-discovery-runtime` 失败与 `crates/contracts/src/producer_runtime.rs:428` 的既有 clippy `too_many_lines` error；⑧是否为这次受保护交付开 GitHub Issue（**上一窗口问过，Mog 未答**）。

**未完成（不在本卡）**：S6（集成证据表、历史处置预览、兼容矩阵、回滚步骤、PR 候选）；共享库迁移应用与历史数据处置；merge／push／部署／runtime 切换／插件重载／发布包生成／真实平台访问——**全部需要另行授权**。本交付包收尾后按 Mog 指定走**一次**独立复核（不逐步复核）。

## 12. S6 集成交付与历史处置准备

> 状态: 活跃计划 · 实施状态回填（本节在 S5 之后写入，记录整包的集成证据、兼容与回滚口径、历史处置的只读预览与最终交付表）。

### 12.1 范围与基线

| 项 | 值 |
|---|---|
| 交付包 | `~/Downloads/linggan-collection-upgrade-handoff-2026-09-21 2/`（README + 01 目标合同 + 02 实施步骤 + 03 验收与发布 + 04 事实与取舍） |
| 分支 / worktree | `fix/collection-upgrade-001` @ `.worktrees/collection-upgrade-001`（仓库唯一允许的 worktree 位置） |
| 基线（分支点） | `1ef5830` —— 已确认是 `origin/main` 的祖先（S0 已更正交付包对基线的描述：包内称 `1511725c → 1ef5830` 只含评论研究，实际中间 PR #322 直接改了采集模块） |
| 本包提交 | **15 个**（`a1e8a9c` → 本记录提交，全部**未推送**）：12 个实现提交（`a1e8a9c` → `de1e8be`）+ 2 个 S6 文档提交（`72763ff`、`e743ed9`）+ 1 个复核裁定与修复（**本记录提交**，HEAD 以 `git log -1` 为准）。复核结论逐条见 §12.9 |
| 差异规模 | 相对分支点 **97 个文件、+15341 / −1199**（含 S6 与本次复核修复；`Cargo.lock` 因 worker 加一条 **dev** 依赖而 +1 行）。**这是写本记录那一刻的读数**：本记录自己就在这批改动里，写完它数字还会再长几条——要引用就现跑 `git diff 1ef5830 --shortstat`，别抄这里的字面值 |
| 当前 `origin/main` | `3ff3982`（已晚于分支点 **8 个提交**，全部是 comment-study 系列、**不含 migration**，因此编号不撞；试合并结果与冲突面见 §12.7） |
| 共享库 | 迁移台账最高 `0095`；本包的 `0096`–`0101` **未应用** |
| 本轮**未发生**的动作 | 共享库迁移应用、历史数据处置、merge、push、部署、runtime 切换、插件重载、发布包生成、真实平台访问 |

### 12.2 合同冲突裁定

| 冲突 | 裁定 | 落点 |
|---|---|---|
| `docs/product/collection-monitoring-rules.md` §0.4「缺失签名执行 locator…也必须走同样的释放与冷却」与交付包 `01` 的缺输入止转相抵触 | **旧条被取代**：原文保留并标明为历史对照，改述为停止语义（缺输入是停止，不是重试；只停这一篇） | S1b 提交 `de7f6f4` |
| 交付包 `01` 的三条同时约束（01:50 身份键含指纹 / 01:52 预算键不含指纹 / 01:54 地址变化不得增加 `retry_epoch`）与初版单键设计互斥 | 改为**两条键分工**（身份键含指纹 + `NULLS NOT DISTINCT`；当前键不含指纹、只覆盖非停止行） | 计划 §5.1；`0097` 迁移注释 |
| `docs/plans/active/detail-page-session-replay-safety-001.md` 只写了「授权 → 执行 → 接纳」的既有 `LOST_AUTHORITY`，未区分「授权登记的准备」与「实际执行」 | 该计划新增第 11 条：三者是三个不同关系；准备记录不构成 Attempt/执行/交付；实施面登记 `0096` | S1a 提交 `edd1e56` |
| 交付包对基线的描述与 `origin/main` 实际不符（包内所有插件与 `dispatch.rs` 行号因此失效） | **改按符号名定位**，并在计划 §3 更正基线 | S0 提交 `a1e8a9c` 前 |

页面文档 `docs/design/pages/collection-workspace-page.md` 未与实现冲突（它不枚举本包新增的作品行文案与「交付对账」小节）；LIDS 手册未改。

### 12.3 验收矩阵结果（T01–T34）

层级口径：**单元** = `linggan-evidence --lib`；**隔离 PG** = 容器/卷/库自建自清的 PostgreSQL 16 证明库；**插件** = `node --test`（Dexie/fake-indexeddb 与路由桩）；**页面** = `linggan-api` 二进制内的 HTML 断言；**浏览器生命周期** = 真实 Chrome + 真实 MV3 worker 终止/重启——**本包一次都没跑**（未获授权安装/重载插件）。

| ID | 场景 | 本包的证明（层级 · 用例/命令） | 结论 |
|---|---|---|---|
| T01 | 页面读完后 API 断开、跨租约到期再恢复 | 隔离 PG `navigation_time_lane_identities_outlive_a_closed_lease_without_impersonating_execution`（身份跨关闭租约存活、按原身份开始并记 `LOST_AUTHORITY`+`ACCEPTED`）；插件 `prepared lane identities are persisted with the navigation grant and cannot be re-pointed`、`lane delivery uses the server-minted identity and stops without one` | **证据层 + 插件单测已证**；「再次导航数为 0」需浏览器生命周期，**NOT VERIFIED** |
| T02 | 服务端已提交 Receipt、HTTP 响应丢失 | 隔离 PG `a_lost_submission_response_still_replays_after_its_lease_closed`（**修前 1 failed / 修后通过**）；插件 `a closed claim still asks the submission route and keeps the Receipt it already earned` | **已证** |
| T03 | outbox 写入后 MV3 worker 终止再启动 | 插件 outbox 持久化用例（既有 + 本包相关用例）只覆盖 Dexie 层 | **NOT VERIFIED**（真实 worker 生命周期未跑） |
| T04 | flush 标记 in_flight 后终止、唤醒 | `markInFlight` 只在路由桩里被断言（调用顺序），没有「终止后超时行恢复」的用例 | **NOT VERIFIED** |
| T05 | 准备 Attempt 响应丢失 | 隔离 PG：同一申请 ID 重放身份一致、准备阶段 attempt 计数为零；插件 `an announced handshake is requested, and a missing receipt never authorizes a page open`（拿不到合规回执即不开页） | **已证** |
| T06 | 同 submission/Attempt 内容变化 | 既有覆盖（`collection_control_postgres`／`material_media_postgres` 的身份冲突用例）+ 本包插件 `an Attempt identity conflict stays terminal and never reaches the submission route` | **既有覆盖（复用）+ 插件侧已证** |
| T07 | 本地相同幂等键、不同内容并发 enqueue | 既有覆盖：`outbox reuses one envelope for the same detail-session lane idempotency key` 与同文件的并发 enqueue 用例 | **既有覆盖（复用）** |
| T08 | 第一个通道包无效、后续通道合格 | 隔离 PG：四通道身份互不相同、错配通道任务被拒、未领取通道仍可各自交付 | **已证（证据层）** |
| T09 | 一页多通道，只有部分已登记/已持久化/已送达 | 隔离 PG `delivery_reconciliation_counts_receipts_not_the_session_marker`（4/0 → 4/3 → 已交付）；页面层四态渲染 | **已证** |
| T10 | API 断开时尚未登记 Attempt 的旧版历史包 | 插件 `prepared lane identities … cannot be re-pointed` 守住「不自动改挂」；**没有**「旧包保留并可人工处理」的运行级用例 | **部分**；运行级 **NOT VERIFIED** |
| T11 | 空 locator 连续调度/claim 100 次 | 隔离 PG `a_lease_whose_members_all_lack_execution_input_ends_as_a_stop_not_a_completion`（停 `input_blocked`、零 Attempt/Package、其后连续 100 轮不再新建租约） | **已证** |
| T12 | 缺地址后获得新有效地址 | 隔离 PG `a_new_signed_address_after_a_stop_opens_a_successor_eligibility_through_admission`（旧停止事实保留、新资格可执行） | **已证** |
| T13 | 重复发现同一地址/只改时间戳或刷新 token | 隔离 PG：同一条地址再准入不多出当前资格；指纹不同时不增加 `retry_epoch`、不清零预算 | **已证（结构面）**；身份键的 `NULLS NOT DISTINCT` 那条**如实降级为守卫**（去掉它没有变红，因为成员展开已挡住） |
| T14 | 每次新 WorkOrder 都收到相同 `page_read_failed` | 隔离 PG `detail_read_failure_budget_follows_the_requirement_scope_across_new_work_orders`（三张新工单、第三次停止） | **已证** |
| T15 | 批量工单一个对象无地址、其余有效 | 隔离 PG `a_missing_locator_stops_only_its_own_material_without_requeueing_forever`（**修前红**） | **已证** |
| T16 | 同一 content 同属目标 A/B | 隔离 PG `the_same_content_under_two_targets_keeps_two_independent_detail_budgets` | **已证** |
| T17 | 两 worker 同时建同缺口工单/重复上报 failure_id | 隔离 PG `duplicate_detail_failure_reports_count_once_and_keep_one_current_row`（`tokio::join!` 真并发）+ 唯一性约束 | **已证（证据层）** |
| T18 | `in_progress` 重放时 locator 不可用但有旧 Attempt/包 | 隔离 PG `an_in_progress_replay_with_a_dead_locator_keeps_the_attempt_and_never_renavigates`（100 轮、现场全量比对；变异验证见 §8） | **已证**（守的是**修前就已正确**的路，记为回归守卫） |
| T19 | 已接纳详情 `title=NULL` | 隔离 PG `a_detail_accepted_without_a_title_still_counts_as_captured`；页面 `a_captured_work_without_a_title_says_the_field_is_missing_not_the_detail` | **已证** |
| T20 | own/cross 各含空标题、无详情、隔离记录 | 隔离 PG `keyword_archive_postgres`（跨行业侧）+ `observation_target_dossier_postgres`（本领域侧）各自读取口断言 | **已证** |
| T21 | released lease + 历史 `in_progress` | **既有覆盖（复用）**：隔离 PG `released_orphaned_work_order_is_recovered_without_rewriting_old_lease_history`（旧租约历史不被改写、仍可查）；S3c 的读层结论由回执数出来、不由会话标记数出来（同一件事的读侧一半） | **既有覆盖（复用）+ 读层已证**；本包未新增专项目例 |
| T22 | `delivery_pending` 会话已有 Receipt 或只是未知本地状态 | 隔离 PG `delivery_reconciliation_counts_receipts_not_the_session_marker`（`DeliveryPending` + 末通道 → 已交付；无通道 → 恢复待核实） | **已证** |
| T23 | 成功 Receipt 与旧失败/预算更新并发 | 隔离 PG `a_late_detail_read_failure_never_spends_the_budget_of_an_already_accepted_material` | **已证** |
| T24 | 租约到期/撤销后原 Attempt 的合格晚到包 | 隔离 PG（`LOST_AUTHORITY` + `material_admission='ACCEPTED'`）；页面 `a_receipt_with_lost_authority_is_not_rendered_as_a_plain_success` | **已证** |
| T25 | 伪造新 Attempt、变更 owner/target、安装替换 | 本包新增（隔离 PG `navigation_time_lane_identities_outlive_a_closed_lease_without_impersonating_execution` 内）：陌生安装与错配通道任务被拒、安装被取代后身份失效；**既有覆盖（复用）**：`creator_lease_claims_and_completes_two_scheduled_tasks_in_order`（陌生安装的新 Attempt 仍被拒）、`replacement_installation_releases_stale_work_instead_of_adopting_it` | **已证（证据层）** |
| T26 | session cache / IDB 配额不足、序列化失败 | 未见任何用例 | **NOT VERIFIED** |
| T27 | 完成/停止事件后到达旧 progress | 隔离 PG：晚到 `DeliveryPending` 被写入侧拒绝为 `SessionNotHeld`、读侧结论仍 `Closed`；页面层五种终结原因 | **已证** |
| T28 | 明确页面不可用 / 暂时超时 / 选择器失配各一例 | 隔离 PG 三例三码：`an_explicitly_unavailable_page_stops_only_its_lanes_without_fabricating_deletion`（新增）、`failed_browser_start_is_audited_then_returns_work_order_to_shared_queue`（复用）、`unavailable_detail_is_audited_without_blocking_later_materials` + `repeated_detail_read_failure_becomes_blocked_without_stalling_later_materials`（复用）；变异验证见 §8 | **已证** |
| T29 | 数据库不可达 / 台账读失败 / 未迁移 | 隔离 PG `runtime_readiness_postgres` 5 例（四类注入 + 只读性）；`startup_contract` 4+1（不退出、不空转、日志不含连接串）；`/health` 两例同判 | **已证（隔离）**；真实 launchd 重启节流 **NOT VERIFIED**（plist 未重装） |
| T30 | worker 一步失败、其余成功 | 隔离 PG `one_failing_step_is_recorded_alone_while_the_other_three_finish`、`an_absent_step_ledger_opens_nothing_rather_than_a_half_recorded_tick`、`a_step_whose_own_tables_are_missing_is_skipped_rather_than_failed`、`ready_requires_the_ledger_row_and_the_required_tables`、`readiness_is_a_heartbeat_column_that_does_not_touch_the_tick_columns` | **已证（隔离）** |
| T31 | 新旧插件/服务端混合版本及旧 outbox 升级 | 插件三例（未通告握手 → 请求与 v1 完全一致；通告 → 请求且缺回执不开页；回执必须回答确切契约/会话/冻结通道集）；服务端 `lane_preparation_contract` 未知版本 422 而不是静默降级 | **已证（契约层）**；「旧 outbox 升级」无运行级用例，**NOT VERIFIED** |
| T32 | 选择器检查成功/缺失/日期陈旧 | 插件 `linggan-selector-health-report` 9 例（三态、两个时刻分离、白名单字段、按平台归一、取最早验证日）；服务端 `selector_health_is_stored_read_back_and_never_gates_a_check_in` | **已证（隔离 + 插件）**；真实页面结构变化下的取值 **NOT VERIFIED** |
| T33 | 优化前后同参数、代表规模的重复测量 | 证明库规模阶梯（3 千/3 万/30 万，同参数各 3 次）+ 运行时库**只读时间窗口差值** + `EXPLAIN` 计划对比；见 §11.4 ①②、§11.9.1 | **已证**；收益只在复刻规模上量过，**不宣传固定百分比** |
| T34 | 全模块集成与生产构建 | 隔离库全套 24 次 cargo 运行 **252 passed / 0 failed / 1 ignored**（退出码 0）；`cargo check --workspace --all-targets --locked` 退出码 0；插件 `npm run build` 退出码 0 + 产物三证（`src/` 零引用、webpack 入口图不含、新建 `dist/` 搜不到该模块名）；`npm run test:douyin` 69/69 | **已证** |

### 12.4 兼容矩阵（schema / API / 插件 / 迁移顺序）

**schema 面（六条新迁移，全部只增不改）**

| 迁移 | 加什么 | 旧二进制读新库 | 新二进制读旧库 | 属于哪一类闸门 |
|---|---|---|---|---|
| `0096_detail_page_session_lane_delivery_identities` | 新表 `collection_detail_page_session_lane_preparation` | 无影响（旧代码不读该表） | **不应用就不通告**：`dispatch_schema_is_ready` 要求该表存在，缺表时 `/health` 不通告 `detailPageSessionLanePreparationContract`，插件也就不请求它——退化成旧路径（**不报错，但 S1a 的交付恢复不生效**） | 部署顺序约束（API 侧） |
| `0097_collection_execution_input_eligibility` | 新表 + `collection_work_order.execution_input_frozen_at` + `collection_work_order_lease_task.execution_state` 增 `input_blocked` + 失败账本两个取值 + 租约释放原因 | 旧代码把 `execution_state` 读成 String，未知值落进 `else → "QUEUED"`——**不崩，但会把「已停止」读成「排队中」**；`release_reason IN ('expired','station_unavailable','revoked')` 那类统计会少算一条 | **是就绪判据**：`COLLECTION_RUNTIME_REQUIREMENTS` 含 `0097`，缺它 worker 判未就绪、一个 tick 步骤都不跑 | 就绪判据 |
| `0098_scheduler_tick_steps_and_readiness` | 新表 `collection_scheduler_run_step` + 心跳/就绪列 | 无影响 | 同上，**是就绪判据** | 就绪判据 |
| `0099_collection_selector_health` | 一列（选择器诊断） | 无影响 | **是部署顺序约束**：新二进制每次「插件报到」都会写这一列，缺列 → `42703` → 报到被判 `422 check_in_rejected`，而 `/health` 仍是 READY（详见 §10.8 的代价说明） | 部署顺序约束（API 侧） |
| `0100_collection_hot_path_indexes` | 两条索引 | 无影响 | **不构成约束**（索引只影响计划，不产生 `42703`） | 无闸门 |
| `0101_collection_command_reason_vocabulary` | 两条 `CHECK` 放宽到代码侧全量词表 | 无影响（旧码是新集合的子集） | **是部署顺序约束**：新二进制写下的新原因码在未应用的库上撞 `23514`，**那笔拒绝连回执都留不下** | 部署顺序约束（API 侧） |

读法：`0097`/`0098` 是**就绪判据**（不应用就不干活、不会半写）；`0096`/`0099`/`0101` 是**部署顺序约束**（新二进制能用，但会在某一条路径上退化成旧行为或整笔失败）。因此部署口径只有一句：**六条一起先应用、后部署**——不给同一批迁移留两种顺序。

**API 面**

| 变化 | 旧插件 → 新 API | 新插件 → 旧 API |
|---|---|---|
| 授权请求新增可选字段 `lanePreparationContract` | 不出现该字段 → 走旧路径（已证：`a service that never announced the handshake receives the unchanged v1 grant request`） | 插件只在 `/health` 通告该握手版本时才带字段；旧服务端不通报 → 不带（已证） |
| 未知握手版本 | —— | 服务端**显式 422** `detail_page_session_lane_preparation_contract_unknown`，**不静默降级成 v1**（已证） |
| `/health` 新增 `readiness` 与 `detailPageSessionLanePreparationContract` | 旧插件只读它认识的键 | 新插件在缺通告时按「没有该能力」处理，不猜 |
| 新拒绝码 `work_order_only_stopped_members_remain`（S1b 的 `…input_blocked_members_remain` 因从未发布直接改名） | —— | 旧插件对该码按未知 4xx 处置（它只在这张工单**被终结**时出现，不是交付路径） |
| finalize 路由新增一个 422 分支（候选 ④） | 触不到（旧插件不会产生 `state='materialized'` 且引用为空的行） | —— |

**插件面**

- 插件源码是本包唯一权威源码（`plugins/linggan-intelligence-browser/`），版本号**未变**（`0.8.54`），`releases/` 未重新打包（未授权）。
- 因此「新插件」目前**只存在于源码里**：真实 Chrome 里跑的还是 0.8.54 的旧构建。安装/重载插件在本包未授权，浏览器生命周期层因此整列 `NOT VERIFIED`（§12.3）。

「假链路」核对（T34 的最后半句）：本包新增的每条 Rust 读取入口都有真实调用方——`qualified_detail`（6 个消费者）、`execution_input_eligibility`（4 个入口 + 3 个读取面）、`runtime_readiness`（worker + `/health`）、`step_report`/`scheduler_tick`（worker 四步）、`selector_health`（插件报到 → 服务端列 → 工位页）、`read_detail_delivery_reconciliation`（`/collection/tasks`）。候选 ⑤ 移出的模块是**反向**的例子：它今天没有任何生产调用方，因此被降级为历史材料而不是留着冒充入口（§11.4 ⑤）。

### 12.5 历史处置只读预览（**只读，未处置任何数据**）

本节回答一个问题：**`0096`–`0101` 应用之后，哪些存量数据会第一次被新语义覆盖**。它**不是处置**——没有改任何状态、没有清任何队列、没有重挂任何 Attempt、没有刷新任何失败预算，全部是 `SELECT`。

数据来源：本机运行时库（`linggan_intelligence_dev`，迁移台账最高 `0095`，容器内只读查询，2026-09-21）。**口径边界**：这是**本机这台运行实例**的快照，不是共享环境的普查；既然是快照，计数会随运行变化；它也不证明外部环境有同样形状的数据。

| 预览对象 | 判据（代码里那一条） | 今天读到什么 | `0096`–`0101` 应用后会变成什么 |
|---|---|---|---|
| **旧缺输入任务**（没有带签名执行地址的小红书作品） | `signed_locator_predicate` = 地址 `LIKE 'https://www.xiaohongshu.com/%'` 且含 `xsec_token=`；解析取「最新已接纳的发现地址」，跨行业样本兜底（`candidate_locator_sql`／`execution_source_url_for_task` 两处**同形**） | 1220 篇小红书详情类作品里 **1137 篇有地址、83 篇没有**；但那 83 篇对应的 86 个任务**一行租约任务行都没有**——它们今天不在派发链上，所以新停止语义**不会**作用到它们 | 真正每天在被反复重试的是另一批：失败账本里 `execution_locator_unavailable` **96 条，全部发生在今天（00:35 → 11:11），只落在 2 篇作品上**（63 次 + 33 次）。它们今天按「有界退避重试」处理，本包之后第一次改判为**停止**（`input_blocked`）并只停这两篇自己 |
| **预算超限范围** | 详情读失败按内容对象累计，预算 3（`DETAIL_PAGE_READ_BUDGET`）；旧规则按**单张工单**数（`page_read_failure_count_for_detail_in_transaction`），新预算按**需求范围跨工单**累计（`0097` 台账 + `unverified_prior_failures_sql`） | 按「作品 × 工单」数：26 对 1 次、1 对 2 次、**23 对正好 3 次**、1 对 5 次、1 对 7 次、**1 对 40 次** | 台账建立前的旧失败（`execution_input_frozen_at IS NULL`）**只记「待核实」、不进预算**——这是代码口径（`unverified_prior_failures_sql` 的 `order_row.execution_input_frozen_at IS NULL` 一条），因此存量工单一条都不会被追溯扣分。**待 Mog 决定**的是：那对 40 次的组合（旧规则下每次换工单就重新开始）今天会被新预算第一次拦住 |
| **遗留 `delivery_pending`** | 会话状态 + 交付对账：冻结通道数 vs 按 `attempt_id` 对上的回执数（`collection_task_read.rs` 的 `LEFT JOIN … ON receipt.attempt_id = lane.attempt_id`） | **57 个会话全部未结束**（`finished_at IS NULL`、无一有 `stop_reason`），涉及 32 张工单、448 个任务；其中 **40 个至少有一条回执、17 个一条都没有** | **今天算不出通道级结论**：`collection_detail_page_session_lane_preparation`（`0096`）**在本机库不存在**——这正是 S1a 补的那张表。`0096` 应用后，对账第一次可读，但**存量会话的冻结通道没有准备记录**，所以它们的结论只能是「无法核验」（`RecoveryUnverified`），不会是「已交付」——这是**如实未知**，不是失败 |
| **捕获包与 Attempt 身份** | `linggan_runtime_capture_package` 的外键 `(attempt_id, task_id)` + `producer_instance_id` 相等（`assert_attempt_owner` 的同一条判据） | **11509 个包、0 个生产者不一致、0 个找不到 Attempt**（这条判据今天本来就成立） | 不改写任何包；这条留在预览里是为了说明「新代码的守卫对着的是 0 行」——它的价值在**防止未来**出现不符，不是修存量 |
| **租约与工单状态词表** | `release_reason`／`queue_state`／`execution_state` 的取值集合 | 释放原因：`completed` 448、`partial` 164、`dispatch_start_failed` 127、`expired` 104、`execution_locator_unavailable` 96、`station_unavailable` 9、`revoked` 2，另有 1 条是**仍然活着**的租约（`released_at IS NULL`，所以原因为空——这不违反那条 `(released_at IS NULL) = (release_reason IS NULL)` 的约束）。工单队列状态：`completed` 596、`legacy` 43、`cancelled` 21、`queued` 3、`leased` 1 | `input_blocked` 在三张表里**各 0 行**——而且**在 `0097` 应用前根本写不进去**（`release_reason`／`failure_disposition` 的 `CHECK` 还不认这个值）。这就是 §12.6 把六条迁移定为「一起先应用」的直接理由 |
| **插件本地 terminal outbox** | 插件侧持久化的终态投递队列 | **读不到**：它存在浏览器侧，服务端既不持有也不该持有这份数据 | `NOT VERIFIED`——本包**没有**读取、没有清空、也不打算清空任何工位存储（红线）。要在浏览器里看它，需要真实 Chrome 与插件重载，两者都未授权 |

**与交付包 `03` §3 的预览合同对账（如实说差在哪）**：本节的预览按「对象 / 判据 / 今天读到什么 / 迁移应用后会变成什么」四列给出，覆盖了 `03` 要求的**对象与当前状态**；**没有**给的是 `03` §3 里属于**受控应用侧**的那几项——「拟追加的动作、影响条数、并发前提、排除理由」的成批清单，以及一个**可重跑**的 repair preview 命令（显式 `batch_id`／预览 hash、事务重验原状态、遇新 Receipt／新租约／新输入跳过并报告、每批最大处理量与锁等待界限、审计记录、重复应用无额外影响）。原因不是遗漏而是边界：**应用**存量处置在本包未获授权，本包也**没有**实现那个命令。是否要另开一卡把它做出来（连同「同输入预算已超限」「旧 released + in_progress」「缺合法历史 Attempt 的包」三类同样只做了只读读数的对象），**报 Mog 决定**。

**预览的结论（一句话）**：这个包里会**改变存量数据行为**的地方只有两处——**停止语义首次作用到 2 篇作品**、**预算首次跨工单累计**；其余四类要么今天就是成立的（包身份一致）、要么要等迁移应用后才第一次可读（通道对账）、要么明确读不到（插件本地 outbox）。因此「历史处置」这件事在本轮的结论是 **`DECISION_REQUIRED`**：本包**不主张**对存量会话、存量失败、存量租约做任何回填或重写，是否需要在部署后另开一张「存量收口」的卡，请 Mog 决定。

### 12.6 部署与回滚步骤（**待执行方案，本包未执行其中任何一步**）

本节是**步骤准备**，不是操作记录。本包没有应用任何迁移、没有部署、没有切换 runtime、没有重载插件。

**部署顺序（一条链，不给两种顺序）**

1. **迁移 —— 六条一起应用**：`./scripts/local-runtime.sh migrate`（开发目录）。当前台账 `public.linggan_local_schema_migration` 最高 **`0095`**（只读实测，2026-09-21），本包六条都还没有名字进过任何库。为什么不拆开应用：§12.4 的读法——`0097`/`0098` 缺一条 worker 就不就绪，`0096`/`0099`/`0101` 缺一条会在某条路径上退化成旧行为或整笔失败，「先应用三条看情况」只会制造一批看起来成功、实际半生效的状态。
2. **二进制 —— 走受控入口**：`~/Library/Application Support/Linggan Intelligence/runtime-main/scripts/runtime/install.sh`（或与 `origin/main` 同步的那份入口；2026-09-17 实测过用落后副本当入口会在 drain 之后失败——巡检 worker 被停掉却没装回去）。`sync.sh` 自己会核对迁移台账：**有未应用的迁移就拒绝启动**——所以顺序上第 1 步必须在第 2 步之前完成，且这一步的失败是可读的（日志列出缺哪几条），不是一堆 SQL 报错。
   - **已知边界（手册已记）**：开机时若 PostgreSQL 比 API 后起来，读不到台账会**跳过检查继续启动**。所以「服务起来了」不证明迁移检查跑过；部署后手工重启一次，或在运行日志里确认那一行不是「读不到迁移台账」。
3. **插件 —— 本步不需要做，也做不到**：版本号仍是 `0.8.54`、`releases/` 没有重新打包，所以真实 Chrome 里那份旧构建**不重载也一致**（不会因为服务端升级而坏，§12.4 的「旧插件 → 新 API」列）。代价是 S1a 的「断网包按原身份交付」在真实浏览器里**不会生效**——它要新插件。要拿到它必须重新构建发布包并重载插件：**未授权、未做**。

**部署后核对（用已有入口，不新造指标）**

- `/health` 的 `readiness` 与 `detailPageSessionLanePreparationContract` 是否出现（对应 §12.4 的 0097/0098 与 0096）。
- 一个 tick 号串一条链：`collection_scheduler_run.scheduler_run_ref` → 步骤行 → 目标级决定 → 工单（三个查询见 `docs/runbooks/local-runtime-deployment.md` §7）。
- 部署后第一次真实插件报到如果返回 `422 check_in_rejected`，先查 `0099` 是否已应用（`42703` 会被收成 422，而 `/health` 仍是 READY）。
- 按交付包 `03` §5，运行观察至少覆盖**一次长于有效租约的交付中断、两轮到期调度、一次 worker 生命周期恢复**，并记录六个数：无输入产生的新可执行工单数、相同输入超预算派发数、重复导航数、待交付年龄分布、交付恢复成功/需处理数、状态不一致数——**前三类在受测样本中应为 0**，且必须写明分母与观察窗口。这六个数本包**一个都还没有**（未部署）。

**回滚**

| 面 | 回滚动作 | 已验证的兼容性 | 未证明 |
|---|---|---|---|
| 迁移 | **不回滚**（仓库口径：append-only，没有 down 迁移）。回滚 = 代码回退，库里多出来的表/列/索引留着 | 多出来的东西对旧二进制不可见（`0096`/`0098`/`0100`）；`0101` 是**放宽** CHECK，旧码是新集合的子集，不可能因此写失败 | —— |
| 二进制 | **已被接手合同否决**：不得移动远端 main。当前回退为新版本切回 recovery；旧二进制回退未获兼容性放行 | §12.4 逐条判过：`0097` 的 `execution_state = 'input_blocked'` 会被**旧二进制读成 `QUEUED`**——不崩、查询也不报错，但那条工单在旧二进制眼里变成「排队中」，而旧二进制不会再推进它（它不是旧代码认识的状态）。**回滚前必须确认没有工单还停在该状态**，或者明确接受它们显示成「排队中」 | 「旧二进制读到 `input_blocked` 之后到底会不会再动它」只按代码读出来（读成 QUEUED → 走排队路径），**没有真的用旧二进制跑过新库** |
| 插件 | 不需要（版本号没变、产物没动） | 旧插件对新 API 的全部路径按 §12.4 逐条判过 | 真实浏览器未做 |

**与交付包 `03` §4／§5 的发布与回退合同对账（一处真冲突，报 Mog）**

| `03` 的阶段 | 本包的对照 | 结论 |
|---|---|---|
| §4.1 候选准备（锁 SHA、S0–S6、独立复审、migration 用执行时下一个空闲编号） | 分支与 SHA 已锁（§12.1）；六条编号在**执行时**实测空闲（`origin/main` 停在 `0095`）；独立复审按 Mog 指定在整包完成后一次 | 一致 |
| §4.2 服务端兼容阶段（新旧插件/服务端三个方向分别验证；未知版本不降级） | 三个方向都按 §12.4 逐条判过：旧插件→新 API 走旧路径（已证）、新插件→旧 API 不带字段（已证）、未知握手版本**显式 422**（已证） | 一致 |
| §4.3 插件切换阶段（按 release-manifest 校验 ZIP 与实际加载的构建） | 未打包、未重载（未授权）；`release:verify` 是既有红灯 | `NOT VERIFIED` |
| **§4.4 资格治理启用（交付恢复已可用后，再启用缺输入止转与跨工单预算）** | **本包是一条链：同一个二进制里交付恢复（S1a）与资格治理（S1b/S2）同时生效，没有开关、也没有分两次部署的实现**。`0096`–`0101` 六条迁移必须一起先应用，就绪判据（`0097`/`0098`）也不再允许「只上一半」 | **冲突，未解决**：按 `03` 的口径应当先让交付恢复跑起来、观察过，再启用止转与预算；本包做不到分段。要分段就得**改代码加开关**（当前没有），或者接受「一次到位」。**报 Mog 决定** |
| §4.5 历史处置（逐批应用、核对影响、并发变化重跑预览） | 未执行（未授权）；只做了只读预览（§12.5） | `NOT VERIFIED` |
| §4.6 运行与业务验证（PID/cwd/revision、readiness、结构化日志、真实平台逐条串联） | 未执行（未部署） | `NOT VERIFIED` |
| **§5 版本回退的前置**：版本回退必须先通过「旧代码读取新增状态／包」的兼容测试 | 本包**没有**这条测试。§12.4 只按代码读出「旧二进制把 `input_blocked` 读成 `QUEUED`」这一个方向 | **按 `03` 的口径，本包当前不满足版本回退的前置条件**；「六条一起先应用、后部署」是部署口径，不等于「可以随时切回旧二进制」 |

**一条既有风险提醒（不是本包的验证结论）**：部署这个动作本身会重启三个常驻服务，**在途作业会被掐断**；2026-09-17 的现场记录显示，被掐断的那一类作业会把租约堵住一直到过期（当时是 asr 类）。因此「挑一个没有在途采集/媒体作业的时间部署」属于操作纪律，不是代码能兜住的。这条来自既有运行记录，本包没有复验。

### 12.7 PR 候选与发布产物校验

**分支与差异（实测）**

| 项 | 值 |
|---|---|
| branch | `fix/collection-upgrade-001`（worktree `.worktrees/collection-upgrade-001`） |
| HEAD | 本记录提交（复核裁定与修复；PR 应以**提交时的实际 HEAD** 为准，用 `git log -1` 读）。分支已于 2026-09-21 推到 `origin`，当时 HEAD = `5212232` |
| base（分支点） | `1ef5830` |
| 提交数 | 15（`a1e8a9c` → 本记录提交；12 个实现 + 2 个 S6 文档 + 1 个复核裁定与修复） |
| 差异 | 97 files changed, +15341 / −1199（写本记录时的读数；本记录也在改动里，见 §12.1 的说明） |
| origin/main | `3ff3982`（**分支点之后又进了 8 个提交**，全部是 comment-study 系列，不含任何 migration——编号不撞） |
| 与 origin/main 的试合并 | `apps/api/src/local_web/comment_study.rs` 自动合并；`docs/progress/2026-09.md` **冲突**（双方都往同一个月度记录里追加，冲突是文本位置，不是语义） |
| 远端 | **已推送**到 `origin/fix/collection-upgrade-001`（首次推送 HEAD = `5212232`）；**PR #328 已建，状态 Draft、未合并** |

**PR 正文（已作为 PR #328 提交，按 `.github/pull_request_template.md` 的骨架填；下面保留成文时的全文以备离线查阅）**

- **Related Issue and authority**：Refs #（**待 Mog 决定是否开 Issue**——本包是受保护交付，但至今没有对应的 GitHub Issue）。Active SCOPE：COLLECTION-UPGRADE-001（handoff 包 `linggan-collection-upgrade-handoff-2026-09-21`）。Base commit：`1ef5830`。Execution Agent/task-id：见 `docs/progress/2026-09.md` 同日各条。
- **Changed scope**：采集模块的交付/恢复与状态表达（S1a/S1b/S2/S3a–S3c）、巡检 worker 的退出语义与可观测（S4a–S4d）、热路径两条索引与原因码词表补齐（S5），六条新迁移（`0096`–`0101`，**均未应用**），插件侧历史材料归位与一个零引用模块降级。
- **Explicitly out of scope**：共享库迁移应用、历史数据处置、merge/push/deploy/runtime 切换、插件重载与发布包生成、真实平台访问、以及所有 `NOT VERIFIED` 行对应的验收（§12.3/§12.8）。
- **Validation and evidence**：照 §12.3 的 T01–T34 表逐行填；自动化侧的分子分母以各步进度记录为准（隔离库全套 24 次 cargo / 252 passed / 0 failed / 1 ignored；`linggan-evidence --lib` **92**（复核修复 +2）；`linggan-api` 259 / 25 ignored；`apps/worker --test startup_contract` ignored 集 **2**（复核修复 +1）；`node --test apps/api/src/local_web/evidence_observation.test.mjs` 3；插件 `test:douyin` 69）。
- **Database and external side effects**：**没有**——本包未对任何共享库执行迁移或写入；隔离库的容器/卷/库由脚本自建自清。
- **Proved / Not proved**：照 §12.3 的「未证明」段与 §12.8 的 `NOT VERIFIED` 行。

**发布产物校验**

- 本包**没有**也无法产出插件发布产物（未授权）；`releases/` 未被改动，版本号未变。
- `npm run release:verify` 在**改动之前就是红的**（`releases/…v0.8.54.zip` 与新建 `dist/` 不一致），已按「把移出的模块临时放回重建、产物逐字节相同」证明与本包无关（§11）；本包不重建发布包，因此「插件发布产物」这一行是 `NOT VERIFIED`，不是「通过」。
- 合并授权：Mog 决定；exact-head 合并授权**未给出**。分支落后 origin/main 8 个提交，合并前需先决定 rebase 还是直接合并（试合并只有一处文本冲突，见上表）。

### 12.8 最终交付表（逐行结论，**不合成总完成度**）

| 交付面 | 结论 | 依据 |
|---|---|---|
| **代码分支 / PR** | 分支已推送、**PR #328 已建**（**Draft、未合并**） | `fix/collection-upgrade-001` → `origin/fix/collection-upgrade-001`；PR #328 以 `main` 为 base，正文按 `.github/pull_request_template.md` 骨架逐段填（§12.7）。**是否开 Issue 仍未决**，因此 PR 的 `Refs #` 空着、Merge gate 两条未勾 |
| **origin/main** | **未被本包改动** | 本包一次 `push` 都没有发生；`origin/main` 仍是 `3ff3982` |
| **自动化与隔离 PostgreSQL** | **通过（S6 全量终态 + 复核后分层复跑）** | S6 串证时跑过全套 `./scripts/test-local-001-discovery-postgres.sh`：**24 次 cargo 运行、252 passed / 0 failed / 1 ignored、退出码 0**，末尾输出 `…cleanup verified; isolated database, container, and volume were removed`（容器/卷/库自建自清）。复核修复**之后**没有重跑这整套 24 次，只重跑了**受影响的层**：`cargo build --locked --workspace`、`linggan-evidence --lib` 92、`linggan-api` 259/25 ignored、`evidence_observation.test.mjs` 3、`apps/worker --test startup_contract` ignored 集 2（在一次性 pgvector 容器上）。**这是本表的已知缺口**：未受影响的层按修复前的读数记账，不冒充「整套在终态上又跑过一遍」。口径：本包**没有**在分支点上跑过一次全量，所以「净值 = 终态 − 分支点基线」这个减法**不写**——只报终态。分层读数见 §12.3 各行 |
| **插件发布产物** | **`NOT VERIFIED`** | 未重新打包、未发布（`releases/` 未动、版本号仍 `0.8.54`）；`npm run release:verify` 在**改动之前就是红的**（§11） |
| **live runtime（本机常驻服务）** | **`NOT VERIFIED`** | 未部署、未切换 revision；3000 端口跑的仍是 `origin/main` 的构建。部署与回滚步骤是**待执行方案**（§12.6），不是操作记录 |
| **浏览器插件链路** | **`NOT VERIFIED`** | 真实 Chrome 未安装/未重载插件、未做 MV3 worker 终止-重启、未跑真实页面。T03/T04/T26 与 T31 的真实浏览器一半都落在这里（§12.3） |
| **共享迁移 / 历史修复** | **`NOT VERIFIED`** | 六条迁移（`0096`–`0101`）**未应用**到任何共享库；没有执行任何历史数据处置（§12.5 只做只读预览）。本机运行时库今天仍是 `0095` |
| **真实平台样本** | **`NOT VERIFIED`** | 未访问小红书/抖音；T29 的 launchd 样例、T32 的真实页面、T34 的真实平台链路都因此只能给结构证据 |
| **Mog 前端 / 业务验收** | **未进行** | 未部署即不可验收；页面侧证据止于 `linggan-api` 二进制内的 HTML 断言与页面单测 |
| **独立复核（commit-reviewer）** | **已跑（交付收尾一次），3 条意见：2 条成立已修、1 条部分成立（改注释、驳回取值改动并写明理由）** | 按 Mog 的指定：整个交付包完成后跑**一次**（不逐步复核）。逐条裁定与复现证据见 **§12.9**；同一条也记入 `docs/progress/2026-09.md` |

**不合成百分比**：本表逐行给结论，不折算成一个「完成度」数字——不同行的分母不同（有的按测试项、有的按真实环境），合成出来的数字只会掩盖哪一行没跑。

### 12.9 独立复核结论与逐条裁定

**什么时候跑的**：整个交付包（S0–S6）完成之后、收尾提交之前，跑**一次** `commit-reviewer`（Mog 指定的节奏：不逐步复核）。它只读代码、不改代码、不跑测试、不连数据库。

**裁定口径**：复核意见不是圣旨。**每条都要判断成不成立，成立的修，不成立的驳回并写明理由**，两种处理都留痕（本表即留痕）。「整轮照单全收」和「整轮无视」一样是没做复核。

| # | 复核意见（概述） | 核实结果 | 裁定 | 处理 |
|---|---|---|---|---|
| F1 | 新增的 `execution_state='input_blocked'` 在复观测读模型里没有对应状态，会落到兜底分支，被显示成「还在排队」——而这批工作永远不会有下一次派发 | **成立** | **修** | `content_reobservation.rs` 新增 `INPUT_BLOCKED`；前端词表与终态集合同步（见 F1 详情） |
| F2 | 未就绪时 worker 只打日志、不落心跳，`readiness_state` 的几个取值永远不会出现在库里，运维分不清「还接不了活」和「进程已死」 | **成立** | **修** | 未就绪分支每轮写心跳（见 F2 详情），并补一条隔离库集成测试 |
| F3 | `keyword_archive_detail.rs` 里 `cross_industry_ready` 传 `true` 的注释理由是错的：外层查询提到表名 ≠ 表存在，真正的保护是入口闸 | **部分成立**（注释确实错；由它推出的取值改动不成立） | **只改注释，驳回取值改动并写明理由** | 见 F3 详情 |

#### F1 —— 读模型把「不会再执行」的工作报成「还在排队」

**意见**：`0097` 引入 `input_blocked` 表示「这个成员在做出任何一次尝试之前就因为缺执行地址被停下，不留 Attempt / Package / Evidence」。而复观测读模型的 `task_state` 按 `execution_state` 分支，没有这一档，于是落进最后的兜底分支。

**核实**：属实，且比意见说的更严重一层。兜底分支的两个出口——`EXPIRED_WITHOUT_RECEIPT`（租约已释放）与 `QUEUED`——**都是「还会再来」的意思**。对一批永远不会被派发的工作显示这两个词，不是文案不准，是把「停在这里等人补输入」说成了「稍后会自动重试」，操作者会一直等一个不会发生的事。

**处理**：
- `crates/evidence/src/content_reobservation.rs`：`task_state` 增加 `input_blocked → "INPUT_BLOCKED"` 分支，位置在 `blocked` 之后、`lease_released` 之前（它比租约状态更具体）。
- `apps/api/src/local_web/evidence_library.js`：`stateLabels` 增 `INPUT_BLOCKED: ['输入不可执行，已停止', 'warning']`，措辞与 `target_drawer.rs` 的「输入不可执行」对齐。
- `apps/api/src/local_web/evidence_observation.js`：`INPUT_BLOCKED` 进 `TERMINAL_TASK_STATES`。它是终态而非进行中——补输入后会形成**新的**资格行、**新的**派发，不是这一条租约继续走；不这样归类，页面会对着一条已经结束的租约每 2.5 秒轮询一次。
- `docs/design/pages/evidence-library-page.md` §7.3：词表增行，并把这张表声明为**闭集**。

**留痕（变异验证，两次都验）**：把 Rust 的 `input_blocked` 分支临时删掉 → 新断言变红，实测差值 `left: "EXPIRED_WITHOUT_RECEIPT", right: "INPUT_BLOCKED"`；把 JS 的 `'INPUT_BLOCKED',` 临时删掉 → 页面单测变红（既没进终态集合，也排了轮询）。两次都**恢复到原状**后再跑绿（已用 `grep` 与逐行读回确认不是变异残留）。

**顺带发现、本包不扩展**：`PAGE_UNAVAILABLE` / `DETAIL_READ_BLOCKED` 在同一个词表里**本来就没有**对应文案，会掉到 `['来源未完整表达','unknown']`。这是本包之前就存在的，不是本包引入，已记入未决清单，**不作为本次扩展**。

#### F2 —— 未就绪的机器不落心跳，五个取值里四个永远读不到

**意见**：`0098` 给 `collection_scheduler_heartbeat` 加了 `readiness_state` / `readiness_detail` / `readiness_checked_at`，列注释写明「任何非 ready 都表示这台机器不得认领工作」。但 `record_readiness` 只在 **ready** 路径被调用，未就绪的分支只 `println!` 一句就 `continue`。

**核实**：属实。后果有两层：库里这一列永远是 `ready` 或 `unknown`，中间四档**从来不会出现**；而「最后一次判定」这个语义恰恰在故障时最有用——现在故障发生时心跳停在故障前那一刻，读它的人分不清「现在还接不了活」与「这台机器说完那句话就死了」。

**处理**（`apps/worker/src/main.rs` 未就绪分支）：每轮落一次心跳，并把「为什么是每轮而不是只写第一次」写进注释（日志只在状态**变化**时说话是为了不制造噪音；心跳那一行是「最后一次判定」，只写第一次就会停在故障开始的那一刻）。数据库连不上时 `database` 仍是 `None`，没有可写之处——那种情况由 `/health` 说，不拿本地时间冒充。

**留痕（新增集成测试）**：`apps/worker/tests/startup_contract.rs` 增 `an_unready_worker_leaves_its_classification_in_the_heartbeat`（`#[ignore]`，用一次性隔离 schema）：
- 夹具不手抄 DDL（手抄会随迁移漂移），而是**运行时读真的 `database/migrations/*.sql`**，建出「能连上、但台账里独独缺 `0034_collection_control_closure`」的状态——这是真实的「可达但未就绪」，且心跳表列齐全。
- 子进程用 `?options=-csearch_path%3D<schema>` 指到该 schema（已先验证 sqlx 接受这种写法）。
- 起来跑 3 秒，断言日志出现 `not ready (migrations_not_applied`，再**读回**心跳行：`readiness_state = 'migrations_not_applied'`、`readiness_detail = Some("0034_collection_control_closure")`、`readiness_checked_at` 非空。
- **变异验证**：把写心跳那 5 行临时去掉 → 心跳行停在 `unknown`，测试变红；恢复后绿。同一次运行里既有的那条 ignored 测试也仍是绿的（我的 schema 不碰 `public`）。

**依赖变动**：worker 的产品依赖里没有数据库客户端（它自己不拼 SQL，SQL 都在 `linggan-evidence`），因此为这个测试加了一条 **dev** 依赖 `sqlx.workspace`；`Cargo.lock` 随之 **+1 行**。证明脚本用 `--locked` 跑，所以 `Cargo.lock` 必须与 `Cargo.toml` 一起提交，否则脚本会在原地拒绝。

#### F3 —— 注释把「为什么安全」说错了；由它推出的取值改动不成立

**意见**：`crates/evidence/src/keyword_archive_detail.rs` 中 `evidence_side_executable(false)` 一类的注释称「传 `true` 是安全的，因为外层查询提到了 `cross_industry_sample` 表」。意见指出这个理由站不住：**提到表名不等于表存在**，真正挡住 `42P01` 的是别的东西。

**核实**：注释确实错。`next_evidence_detail_batch` 的外层查询是**证据侧**的，它并不会去碰样本表；把「外层查询提到了这张表」当成保护，是把「语句里出现过这个名字」误当成「这张表一定在」。

**裁定：部分成立。** 注释改；**取值改动驳回**，两条理由：

1. **派发侧的先后顺序是证据侧在前、样本侧在后**（`execution_source_url_for_task`）。传 `false` 会让候选集只看证据侧，而停转判定看两侧——**可执行的作品会被静默丢掉**。候选集与派发必须同构，这条对称性是这一版设计的核心。
2. **`unchanged_input_block_predicate` 传 `false` 会只查证据侧**，而停转判定查两侧，于是重新制造「重复入队 → 再停」的循环——正是这一版要消灭的东西。

真正承担安全性的是**入口处的闸**：`sample_facts_schema_ready_sql!()`（`to_regclass` 三张样本表）+ `collection_work_order_cross_industry_target`，例如 `advance_keyword_archive_detail` / `keyword_targets_pending_detail` 进门前先查，`keyword_archive_baseline_in` 走 `keyword_detail_schema_is_ready_in`。把这个闸撤掉，才会 42P01——但那是入口的问题，不是这个布尔的职责。

**处理**：两处注释改写成上面这两条真实理由，并写明「安全性由入口闸承担，不由这个布尔承担」。

#### 这一轮复核**没有**覆盖的

- 复核是**读代码**：不跑测试、不连数据库、不碰共享库。上面每条结论都由我自己复现验证过（变异两次 + 一次隔离库集成测试），不采信「委员会说它对」。
- 本次修复**之后**没有重跑整脚本 `./scripts/test-local-001-discovery-postgres.sh` 的 24 次全量，只跑受影响的层（读数见 §12.8 自动化行）。这是本表记录的**已知缺口**，不用「应该没影响」把它抹平。
- §12.7 记的既有红灯（`release:verify`、clippy `too_many_lines`、插件 `current-surface-discovery-runtime`）**不是**复核发现的，是 S6 串证时就存在的，与本包无关（§11 已逐字节证明）。



### PR #328 接手终态验证（提交前，2026-09-21）

- 原代理交付不能判为全部完成；本次已补齐 F1–F5、并发缓存、repair 工具、阶段开关和候选发布。
- `test:linggan` 290/290、`test:douyin` 69/69；新 npm ci/build/package 可重复，0.8.55 ZIP SHA-256 `888d2b387e22de56df591f485932e57c149c721393e998db33157ee8a72cabbf`。
- 全量隔离 PostgreSQL：24 次 cargo 运行，256 passed / 0 failed；启动套件中最初 ignored 的 2 项在末尾独立运行通过，容器/库/卷清理通过。定向 dispatch 37/37；`linggan-evidence --lib` 92/92。
- 真实 Chromium 合成 MV3 生命周期：worker 执行上下文确已更换；原四个 Submission/Attempt/Package 不变，4/4 模拟回执接纳；真实平台导航 0。证据边界为独立 profile/真实 IDB 与 worker/模拟 API，不是正式材料。
- commit-reviewer 三轮只读复审收口；最后独立 Node 29/29，加测并发四通道标记完整，未发现新增阻断。治理检查通过。
- 提交前发现 main 新合入 PR #329（4f4dada，评论研究范围，无 migration 冲突）；在推送前继续集成该提交并完成受影响验证。部署回执以实际 main SHA、共享迁移台账、运行身份和浏览器加载结果为准。
