# ACC-KEYWORD-ARCHIVE-002 · 关键词「建立档案」的入口可达性与回执真实性验收

> 状态: 一次性报告
> 最后核对: 2026-09-15
> 适用范围: Issue #286 的 `/collection/targets` 关键词分行、抽屉决策区与右下角回执，以及 `POST /collection/targets/archive` 的关键词分支
> 事实来源: 当前交付分支、真实样式表层叠复核、focused 渲染测试、隔离 PostgreSQL 走产品路由的证明与两轮独立复审
> 冲突时以谁为准: 真实代码/测试/运行结果与用户最新确认

## 0. 2026-09-15 流程门槛补充（当前合同）

Mog 已决定关键词必须完成建档才允许进入巡查。本补充取代本文件中任何“未建档关键词可先保存/恢复巡查规则”的历史表述，但不改变 creator 的独立观察合同。完成是“合格搜索面 + 无待补详情”，不是已提交任务、空候选或 `monitoring` 生命周期。

- `SaveRule` 与 `Resume` 在同一目标锁事务中拒绝未完成/未知关键词，写入既有闭集的 `baseline_not_ready` command receipt，且不创建规则 revision。
- `patrol` 的统一 Request → Admission → WorkOrder 链再次检查；人工观察和 scheduler 都不能绕过。scheduler 记录 `baseline_not_ready` 并且不生成 WorkOrder。
- 旧 target-level 开关在逐规则操作前预检，避免部分翻转；未知/删除均给明确回执，不再落“稍后重试”。
- 列表与抽屉对 `Unavailable` 只提供查看建档状态；不显示“设置巡查”。
- 历史任务缺失 `expectedCount` 时，搜索面 `target_reached` 只回退到该任务冻结的 `maximumQuota`；新任务仍优先使用 `expectedCount`，不会把关键词巡查的 20 条口径误读成 200。

本补充不自动暂停或重写历史规则；这会是独立的数据处置。验证结果与未证明边界以本次提交后的实际测试记录为准。

## 1. 验收对象

- 页面/组件/状态: 关键词行的「档案」列与主操作按钮、抽屉决策区、成功回执；处理函数 `collection_target_deep_archive` 的关键词分支。
- 关联 PAGE / PAT / CMP / DS / ACC ID: `PAGE-COLLECTION-001`；上一包 `ACC-COLLECTION-ACTION-001`（Issue #283）；LIDS token `--lgi-z-toast` / `--lgi-z-drawer`。
- 验收日期与环境: 2026-09-15；`.worktrees/keyword-archive-002`（分支点 `origin/main@090ec80`，由 `dab0be3` 变基而来）；`./scripts/test-local-001-discovery-postgres.sh` 的隔离 PostgreSQL 16；层叠复核使用本机合成页，不接共享数据库。
- 适用数据/权限前提: 关键词目标已归属领域、已有覆盖 `deep_archive` 的采集授权。隔离库里的目标由页面入口现场创建，不复制线上数据。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 未建档 + 未观察（`pending_decision`） | 建起这个词的底座 | 「尚未建立」，主操作是「建立档案」 | 档案列与主操作同屏不矛盾 | `archive_requested` + 一张 `deep_archive` 工单 | VERIFIED：隔离 PG 走产品路由（新增证明用例） |
| 未建档 + **正在观察**（`monitoring`，线上 adhd 的实际状态；a 娃已于 2026-09-15 按产品删除路径删除，见 §5） | 同上 | 同上——按钮会出现（这是本包修的） | 同上 | **`keyword_archive_not_requestable`，且不产生任何工单**：采集准入只在开始观察之前接受关键词的历史建档请求 | VERIFIED：隔离 PG 走产品路由（新增用例）。**这一格点下去仍然发不出采集**，原因不在本包范围，见 §5 DECISION_REQUIRED |
| 未建档 + 已暂停（`paused`） | 同上 | 主操作是「恢复巡查」 | 该行不提供建档动作 | 无采集动作 | SOURCE VERIFIED：`a_paused_keyword_still_resumes_its_patrol`（#283 的既有决定，有测试钉住）。与上一格叠加后：恢复 → 变成 `monitoring` → 仍被准入拒绝 |
| 建档中（第二段欠详情） | 继续补详情 | 「建档中」，主操作是「补采缺口」 | 不被巡查状态顶掉 | `keyword_detail_requested` / `archive_in_progress` | VERIFIED：既有分支保留，focused test |
| 建档中 + **已被停掉**（`dismissed`）：补详情同样不被准入接受 | 不替人做决定 | 新渲染给「查看结果」；只有**旧渲染**上那颗按钮还可能被按到 | — | `keyword_archive_not_requestable`——两段共用同一条分类，不再说「稍后可以重试」 | SOURCE VERIFIED：focused 判据测试 + 代码事实。**没有端到端用例**（造出这一格需要八张表的重型夹具），理由与本轮为何接受这一取舍写在 §5 第二轮① |
| 已建档 + 正在观察 | 不被打断 | 「档案已建立」，主操作回到查看 | 不出现建档按钮 | 无采集动作 | VERIFIED：focused test |
| 已建档（两段都完成）后按建档 | 确认无需重复 | 说明已完成 | — | `keyword_detail_complete` | SOURCE VERIFIED：路由分支 |
| 建档态读不到 | 不替人做决定 | 建档态未知 | 说明原因 | 不发起任何采集，回 `keyword_archive_unreadable` | SOURCE VERIFIED：路由分支 + 回执表 |
| 回执与抽屉同屏 | 看见刚才那一下的结果 | 回执在抽屉之上 | 右下角角标不被遮盖 | 3 秒自动退场 | VERIFIED：真实样式表层叠复核（含反向对照） |

## 3. 视觉工作条件

- 声明的桌面工作区/视口: 层叠复核在 1920×1080 的本地合成页上完成；合成页按产品页面的同一顺序内联 `lids_tokens.css`、`shell.css`、`collection_workspace.css`、`target_drawer.css`，并复现「回执在前、抽屉在后」的真实 DOM 次序。**合成页不是产品页**，它只用于复核层叠与计算值。
- 输入内容长度与数据密度: 回执文案取最长的成功回执（含排队位置）。
- 已批准的设计规则: LIDS 层级阶梯 `--lgi-z-toast`(180) > `--lgi-z-popover`(140) > `--lgi-z-overlay`(100) > `--lgi-z-drawer`(80)。
- LIDS 强度 / Pattern / Token 依据: Token 层修正，不改表达强度与布局；新增回执文案沿用既有回执形态，无新组件。
- LIDS 状态五轴或合成边界依据: 无新增状态；本次没有把未知写成已完成（读不到就不动作），也没有把永久的状态前置写成「稍后重试」。
- Reduced Motion / 移动或静态 Poster 降级: 回执退场沿用既有 CSS 动画与 `prefers-reduced-motion` 分支，本包未改。
- 已检查的响应式/可访问性条件: 回执 `role="status"` 未变；层叠改动不移动任何控件位置。
- 截图或录屏证据位置及生成物登记状态: 未生成截图；层叠结论来自 `elementFromPoint` 计算值，不产生需登记的生成物。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | SOURCE VERIFIED | LIDS Token 阶梯 + 修正为使用既有 `--lgi-z-toast` | 实页视觉待验 |
| 前端/组件实现 | VERIFIED | focused 渲染测试 + 真实样式表层叠复核（toast z=180 命中回执；反向写死 80 时命中抽屉） | 合成页不证明真实回执文案与真实页面 |
| 自动检查 | VERIFIED | `cargo test -p linggan-api --bin linggan-api --locked`（交付修订版上 `225 passed; 0 failed; 22 ignored`；第一轮独立复审复跑 `223` 项同结果）+ `./scripts/check-project-governance.sh` 通过 | 见 §6 本次运行记录；不覆盖浏览器与产品页实拍 |
| 真实链路/回执 | VERIFIED | 隔离 PostgreSQL 走产品路由两条：①未观察的未建档词 → 回执 `archive_requested` 且库中新增一张 `deep_archive` 工单；②观察中的未建档词 → 回执 `keyword_archive_not_requestable` 且工单数为 0 | 不证明平台采集成功；不证明线上已部署 |
| 部署 | NOT VERIFIED | 本包未切 runtime。当场核对的部署版本是 `e2598d4`（`origin/main` 当时最新修订），**上面没有本包的代码**：adhd 那一行同时写着「尚未建立」与「242 / 242」详情进度，主操作是「查看结果」，没有建档入口 | main 合并不等于部署；本记录不证明修复已在线上生效 |
| Mog / 业务验收 | NOT VERIFIED | 待前端验收 | 不由自动测试替代 |

本记录不证明：线上已运行修复后的代码、adhd 现在能一键建好档（见 §5 的两条：采集口那条准入与判据层 `expectedCount`）、关键词档案已经完整、平台采集成功、或 Mog 已完成业务验收。

## 5. 发现与后续

- **发现的规格冲突**: 上一包 `ACC-COLLECTION-ACTION-001` 的验收句「已开启巡查的词仍是『当前无需处理』」在本包被**有意改变**。那句话成立的前提是「这个词已经建过档」——对一个还没建过档的词，巡查按规则口径只取每期增量，把建档让位给「查看结果」等于让这个词永远没有底座，且行上写着「尚未建立」却没有任何入口。本包把「建过档没有」排在巡查状态之前，两行/抽屉仍共用同一个判据函数（`target_primary_action`），一致性没有回退。
- **DECISION_REQUIRED 的处置（Mog 2026-09-15 已决定）**：原提两条——Ⅰ 是否放宽采集准入闸、Ⅱ 判据层 `expectedCount` 必须先修。Mog 的决定是把门槛设在**流程**上：「必须全部补完，才能添加观察（巡查）规则」。由此 **Ⅰ 不是放宽准入**：准入写的是「历史建档只在开始观察之前接受」，在「先建档后巡查」之下这条规则本来是对的；缺的是流程没有在**开观察那一端**把关，那是一个新交付包，**本包不改**（`#286` 已列为非目标）。**Ⅱ 仍成立，且因为新门槛成为前置**——新门槛读的是同一份判据：判据不修，一个真正建好档的词会被读成「尚未建立」，从而永远开不了观察。两条的原始事实依据照原样保留如下。
  1. **采集准入闸**：`request_and_admit` 的 `deep_archive` 分支对**不携带具体作品**的关键词请求只接受 `pending_decision | archiving`（`crates/evidence/src/acquisition_chain.rs` 中的 `"deep_archive" => matches!(lifecycle_state, "pending_decision" | "archiving")`），而迁移 0042 禁止关键词进入 `archiving`/`archived`，于是对关键词它实际等价于「只在开始观察之前」。已在隔离 PostgreSQL 上实测：`lifecycle_state='monitoring'` 的未建档关键词按建档 → 回执 `archive_unavailable`、工单数为 0（本包已把这条回执改成诚实的 `keyword_archive_not_requestable`）。**「先关掉观察再去建档」这条绕路也不通**：`Stop` 会把目标打成 `dismissed`（`collection_control.rs` 中的 `MonitorCommandKind::Stop if lifecycle_state != "dismissed" => Some("dismissed")`），而 `dismissed` 同样不在准入允许之列。
  2. **判据层的 `expectedCount`**：`keyword_baselines_qualified` 读的是派发时冻进 `task_spec` 的 `expectedCount`，早于该字段的轮次永远没有它，`COALESCE(...,2147483647)` 使判据对历史行恒不成立。后果：adhd（本机库中已有 **80 张 `completed` 的 `deep_archive` 工单**，全部由建档取得）被读成「尚未建立」。判据本体涉及 6 个调用点，是另一件事。**在它修好之前，不要在 adhd 上点这颗按钮。**
- **流程那一端缺的到底是什么（已在本机与 `origin/main` 上核实，供新交付包使用）**：`toggle_target_patrol` 只在「一条活规则都没有」时拒绝（`MonitorRuleCommandError::UnknownTarget`，意思是「先去设一条规则」），**从不读建档态**；`monitor_lifecycle_next_state` 里有两条路可以在完全没有建档的情况下走到 `monitoring`——`MonitorCommandKind::SaveRule if target_automatic && lifecycle_state == "pending_decision"` 与 `MonitorCommandKind::Resume if matches!(lifecycle_state, "paused" | "archived" | "pending_decision")`。**另有一条既有设计决定必须在动工前处理**：同一个函数里写着「Observing a known target and historically archiving it are separate capabilities. A creator need not first prove a complete archive in order to enter ordinary automatic observation.」——**creator 的观察被有意设计成不需要先建档**。**Mog 2026-09-15 已裁定：这道新门槛只管关键词**——creator 那条既有设计保持不变，新交付包不得顺手把门槛套到 creator 上。
- **同一决定带出的一次数据处置（已执行、已核实）**：Mog 2026-09-15 决定删掉 a 娃（「这两个关键词监控可以删了……当前我们在项目开发期，数据可以接受直接删除」），并在「adhd 修判据」之下保留 adhd。a 娃走的是产品自己的受控删除路径（`POST /collection/targets/delete` → `delete_observation_target`，含显示名复核），不直连数据库改表；删前按该路径涉及的每一张表逐表导出到 `/tmp/a-wa-deletion-backup-20260915/`（共 7 行：1 条规则、1 条规则修订、1 条命令身份、1 条命令回执、2 条状态迁移、1 条目标行，其余表为空）。删后核实：目标行与全部 6 张控制面子表为 0，12 张带 `target_ref` 的表（含采集请求、准入判定、工单、`cross_industry_sample`）全部为 0——**a 娃名下从未有过采集请求，因此没有它采来的材料会被这次删除留下**；证据侧三条计数与删除前完全一致（**737 材料 / 619 详情 / 4567 采集包**），这与 `delete_observation_target` 的设计一致——它只清控制面，append-only 证据保留（见 `crates/evidence/src/collection_target.rs`）。adhd 未动，仍为 `monitoring`（它那 80 张 `completed` 的 `deep_archive` 工单正是上面第 2 条的证据）。
- **独立复审发现的存量缺陷（本包不修，如实登记）**: `keyword_targets_pending_detail` 在 schema 未就绪时返回 `Ok(空集)`（`crates/evidence/src/keyword_archive_detail.rs:236-245`），而空集在四态读里表示「都补齐了」——于是一个**已建档但详情未补齐**的词可能被读成 `Complete`，点建档回「详情已补齐」且不发采集。这与本 Issue 要消灭的「空候选集读成已补齐」是同一形状，发生在部分迁移的角落；旧代码在同一角落也回同一句，不是本包引入的回归。修它要动 `linggan-evidence` 的读语义（超出本包文件边界），报给 Mog 决定。
- **独立复审结论的核实与处置**: 本轮共 5 条发现 + 3 条低注记。①准入闸（上面第 1 条）——**成立**，已自行复现并改掉被它误导的那句回执；②治理检查红（状态头不在允许词表）——**成立**，已改为 `一次性报告`，检查现通过；③ paused 词无建档入口——**成立但不是缺陷**：`a_paused_keyword_still_resumes_its_patrol` 是 #283 有意钉住的行为，本包只更正文档措辞（此前写成「不论是否正在观察」是不准确的），其后果并入上面第 1 条决定；④新证明用例首跑 422——**部分驳回**：审核归因为「容器冷启动瞬态」，实测根因是夹具把授权 body 写成 snake_case 而 `GrantBody` 是 `#[serde(rename_all = "camelCase")]`，服务端解析失败即回 422，与数据库无关（改为 camelCase 后连跑通过）；其附带的「路由把 `grant_authorization` 的错误吞成笼统 422 且不落日志」属既有代码的可诊断性问题，不在本包范围；⑤详情查询在 schema 未就绪时返回空集——**成立**，同上条登记。
- **第二轮独立复审（对 ① 反驳的裁定 + 本轮新增三处改动）的核实与处置**: 复审员已撤回其第一轮的 ④（「容器冷启动瞬态」），确认根因是夹具字段名与 `GrantBody` 的 camelCase 不符。本轮它另报三条，逐条核实：
  1. **第二段的同一个分裂（成立，已修——但不是按它建议的「一行」）**：`0042` 的 CHECK 让关键词进不了 `archiving`/`archived`，而第二段那条准入守卫接受 `pending_decision | archiving | archived | monitoring | paused`，所以这一臂**唯一会被拒的状态就是 `dismissed`**；而这一臂的 `Err(_)` 仍旧落 `archive_unavailable`「可以稍后重试」。触发窗口真实存在——渲染之后这个词被停掉，再按旧页面上的那颗按钮，这正是本卡说「点击必须按点击当下的状态走」的那一格。**修法不是再加一条并列分支**（那正是本卡要消灭的补丁式修法），而是把两段的错误阶梯并成一处 `keyword_archive_error_receipt`，调用方不再自己 match，也就不再有「哪一段漏了」的地方。连带把回执文案改成两句规则都说——沿用第一段那句「只在开始观察之前接受」**会在第二段说谎**，因为第二段被拒的原因不是它。
  2. **兜底仍覆盖两个永久条件（部分驳回，登记）**：`SchemaUnavailable` 写「可以稍后重试」是**对的**——迁移补上之后确实可以重试，复审这条不成立；只有 `UnknownTarget`（目标在读取与动作之间被删掉）重试确实无用，但它是并发删除的竞态，与本卡要修的状态前置不同类，要修它得新加一句「目标已经不在了」的回执。本包不动，如实登记给 Mog。
  3. **manifest 范围句对已暂停的词不成立（成立，已修）**：与第一轮 ③ 是同一处措辞，已收紧为「只要它还在观察里或还没开始观察」，并把「已暂停仍以恢复巡查为主操作」「被停掉的给查看结果」写进同一句。
- **提交前第三轮独立复审（3 条低严重度；结论是未发现拦路级缺陷，已逐条裁定）**: 审核员本机实测了编译、单测全套与隔离 PostgreSQL 全套（脚本 exit 0、两条新增用例均 ok、`cleanup verified`），其余为逐行读代码。
  1. **运行记录把基线数字写大了（成立，已改）**：记录把本包中间态 `223` 说成「与包内既有基线一致」，当时（分支点 `dab0be3`）的真实基线是 **220 passed / 20 ignored**（2026-09-14 条目独立写着「`linggan-api` 单元 220/220」）。已改成可逐项对账的说法：本包净增 4 个非忽略用例 + 2 个忽略用例。**变基到 `090ec80` 后这个数字又动了一次**，原因不在本包：main 在 `dab0be3..090ec80` 之间新增了一个非忽略用例（`evidence_cover_layout_keeps_a_stationary_data_plate_and_one_shared_flip_stage`，属 EVIDENCE-COVER-FLIP-CARD-001），包内基线随之变成 **221 / 20**，于是交付修订版上的终态是 **225 / 22**——净增仍是 +4 / +2，逐项对得上（见 §6）。纯文档，不影响运行。
  2. **层叠断言只钉住了回执一侧（成立，已补 + 变异验证）**：「回执盖过抽屉」是**两侧一起**成立的不变量，原断言只核对回执是否用 token、以及 token 阶梯的先后——**抽屉那一侧没人守**。将来把抽屉写死成 `999` 或改挂别的 token，回执会重新被盖住，而这条为守护该不变量而写的用例照样绿。已补上抽屉侧：`.c-dw` 必须由 `--lgi-z-drawer` 定层（本页抽屉就是它——`target_drawer.rs` 渲染 `<aside id="c-drawer" class="c-dw">`，已用线上页核实）。**变异**：把 `.c-dw` 写死成 `999`，新断言变红且只有它变红；回退后 CSS 与改动前逐字节一致。
  3. **回执里「被停掉」有歧义（成立，已改）**：产品自己的词表里 `dismissed` 是**「已停止观察」**（`target_drawer.rs:939`），而 `paused` 是「已暂停」，且**暂停期间详情补采是被准入接受的**（`a_paused_keyword_can_still_finish_its_details` 钉住，守卫见 `acquisition_chain.rs:828`）。把「被停掉」读成「已暂停」的用户会以为暂停期间连详情也补不了——那是一条不存在的规则。改用词表原词：「详情补采在这个词**停止观察**之后就不再接受」。触发窗口窄（paused 的主操作是「恢复巡查」，只有旧渲染可达），但它落在本包主题最敏感的那一点上。
- **本轮接受的一处证明缺口（如实记）**: 第二段那条新分类**没有端到端用例**。要造出这一格（已建档、欠详情、且被停掉的关键词）需要向工单、租约、运行时任务、采集包、回执、发现、处置等八张表灌入一致夹具，代价与这条 LOW 级缺陷不相称，且重型夹具本身会成为新的负债。代之以：一条 focused 判据测试（两条断言都做过变异验证，撤掉分类或改掉兜底各自变红）+ 第一段那条隔离 PostgreSQL 用例（钉住两段共用的那一处真的接在真实链路上）。**因此本记录不声称第二段的拒绝已经端到端证明过。**
- **是否需要设计例外或长期决定**: 无。本次使用既有 token 与既有回执形态，未新增例外。
- **不得因此推断的结论**: 本包不证明抽屉覆盖时列表行按钮可点（调查结论是设计使然，见 Issue #286「丙」）；不证明任何一次真实平台采集的结果；不证明线上部署与业务验收；不证明观察中的关键词已经能建成档。

## 6. 本次运行记录

| 运行 | 命令 | 结果 |
|---|---|---|
| focused（变基前，分支点 `dab0be3`） | `cargo test -p linggan-api --bin linggan-api --locked` | `test result: ok. 223 passed; 0 failed; 22 ignored`。**223 不是包内既有基线**：这时本包已经加入「回执层叠」断言、把 #283 那两条判据测试各拆成两格（2 → 4）以及两条 `#[ignore]` 隔离证明。在该分支点上包内基线是 220 passed / 20 ignored（`linggan-api` 单元 220/220 见 `docs/progress/2026-09.md` 的 2026-09-14 条目），所以这一次是 **+3 非忽略 / +2 忽略**——第四条的判据分类用例在第二轮才加入（见下一行）；本包终态净值是 +4 / +2 |
| focused · 第二轮修后重跑（变基前） | 同上 | `test result: ok. 224 passed; 0 failed; 22 ignored`（+1 为本轮新增的判据测试） |
| 变异 · 第二轮（状态拒绝分类） | 撤掉 `TargetNotRequestable` 的分类，让它落回兜底 | 新判据测试变红（tests.rs:1104，`left: "archive_unavailable" / right: "keyword_archive_not_requestable"`） |
| 变异 · 第二轮（兜底被当成状态前置） | 把 `_` 兜底也改成返回 `keyword_archive_not_requestable` | 同一测试变红（tests.rs:1116，`left: "keyword_archive_not_requestable" / right: "archive_unavailable"`）——两个方向都钉住了 |
| 隔离 PostgreSQL 全套 | `./scripts/test-local-001-discovery-postgres.sh` | 18 个 `test result: ok`、0 失败；末行 `LOCAL-001 discovery PostgreSQL proof passed`，收尾打印 `cleanup verified; isolated database, container, and volume were removed` |
| 治理检查 | `./scripts/check-project-governance.sh` | `project governance check passed` |
| 变异 · 甲 | 把 `.c-tg-toast` 层级改回写死 80 | 只有该断言变红 |
| 变异 · 乙（两段次序） | 处理函数改回「先试第二段」 | 只有 `a_never_archived_keyword_starts_its_first_stage_when_asked_to_archive` 变红，报错为 `error=keyword_detail_complete` |
| 变异 · 乙（回执分类） | 撤掉 `TargetNotRequestable` 的单独映射 | 只有 `a_patrolling_keyword_says_its_archive_request_is_refused_by_state_not_by_chance` 变红，报错显示回执落回 `archive_unavailable` |
| focused · 第三轮修后重跑（变基前） | `cargo test -p linggan-api --bin linggan-api --locked` | `test result: ok. 224 passed; 0 failed; 22 ignored` |
| 变异 · 第三轮（抽屉侧层叠） | 把 `.c-dw` 的 `z-index` 从 `var(--lgi-z-drawer)` 写死成 `999` | 只有 `the_receipt_is_layered_above_the_drawer_it_must_be_read_over` 变红（`z-index:999;max-width:100%;…`）；回退后 CSS 与改动前逐字节一致 |
| focused · 交付修订版重跑（变基后） | `cargo test -p linggan-api --bin linggan-api --locked` | `test result: ok. 225 passed; 0 failed; 22 ignored`——基线 221 / 20 加本包净值 +4 / +2，逐项对得上（口径见下） |
| 隔离 PostgreSQL 全套 · 交付修订版重跑（变基后） | `./scripts/test-local-001-discovery-postgres.sh` | 18 个套件全部 `test result: ok`（合计 **195 条用例、0 失败、0 panic**）；本包两条新用例均 `ok`；末两行为 `LOCAL-001 discovery PostgreSQL proof passed` 与 `LOCAL-001 PostgreSQL proof cleanup verified; isolated database, container, and volume were removed`；逐套件日志 `/tmp/local001-proof-keyword-archive-002.log`（一次性文件，不入库） |
| 治理检查 · 交付修订版重跑（变基后） | `./scripts/check-project-governance.sh` | `project governance check passed` |

本记录不覆盖：浏览器里的真实点击、产品页实拍、平台侧采集结果、线上部署。

**关于上面两次数值的口径**：表内写着 `223` / `224` 的那三行（`focused` 与两次`focused` 重跑）是**变基前**在分支点 `dab0be3` 上跑出来的记录，原样保留。变基到 `090ec80` 后 main 带进来一个非忽略用例（`evidence_cover_layout_keeps_a_stationary_data_plate_and_one_shared_flip_stage`，EVIDENCE-COVER-FLIP-CARD-001），包内基线随之从 220 / 20 变成 **221 / 20**，所以交付修订版上的终态是 **225 / 22**。终态由 224 变成 225 **不是本包少算或多算了一个用例**，是分支点变了：本包净值始终是 4 个非忽略用例（回执层叠 1、#283 的两条判据测试各拆成两格共 +2、判据分类 1）+ 2 个 `#[ignore]` 隔离证明。这个数字可以用 `git diff` 逐条对：`090ec80..<交付修订版>` 在 `apps/api/src` 下新增 4 条 `#[test]` 与 2 条带 `#[ignore]` 的 `#[tokio::test]`，无删除。
