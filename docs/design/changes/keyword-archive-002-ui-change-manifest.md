# KEYWORD-ARCHIVE-002 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: `/collection/targets` 关键词行的「档案」列与主操作、抽屉决策区、右下角回执，以及 `POST /collection/targets/archive` 的关键词分支
> 事实来源: Mog 2026-09-15 的修复授权、Issue #286、PAGE-COLLECTION-001、UI execution contract、LIDS 与当前代码/测试/隔离 PostgreSQL 证明
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据合同与 Issue #286；本清单不授权新的采集口径、部署或运行时切换

## 事项与读取回执

- Issue / SCOPE: Issue #286 / KEYWORD-ARCHIVE-002；实际分支点 `origin/main@090ec80f72ac87a491d91309cfd54230ab238a36`（交付期间 main 数次前进，本分支随之变基：`dab0be3` → `090ec80`；合并前已核实与当时的 `origin/main@36efc4b` 无冲突）。
- Agent 与 worktree: Claude（harness 会话），`/Users/moglenny/proma/linggan-intelligence/.worktrees/keyword-archive-002`。
- 用户可见目标: 一个还没建过档的关键词，**只要它还在观察里或还没开始观察，都拿得到「建立档案」这个入口**（本包修的正是这个：此前正在观察的词只给「查看结果」，行上写着「尚未建立」却没有任何入口）；按下之后走哪一段由建档态决定，回执说的是已经发生的事；抽屉打开时这条回执也看得见。**已暂停的词仍以「恢复巡查」为主操作**（#283 的既有决定，本包不动）；被停掉的词给「查看结果」——这两类不在这句话的范围内。
- 用户可见目标**未达成的那一半（如实记）**: 入口对观察中的词已经出现，但按下去仍发不出采集——采集准入只在开始观察之前接受关键词的历史建档请求（见「停止条件」与验收记录 §5）。本包把这一下的回执从「可以稍后重试」（假）改成「这一条是状态前置，重试不会改变结果」（真），但**没有**让观察中的词真的建起档。**Mog 2026-09-15 已决定这一半怎么收口**：不是放宽准入——准入本身是对的，它就是「先建档后巡查」在采集口的表达——而是在**开观察那一端**补上同一道门槛（「必须全部补完，才能添加观察（巡查）规则」，只管关键词，不覆盖 creator），属另一交付包，本包不改；且它要排在判据层 `expectedCount`（验收记录 §5「DECISION_REQUIRED 的处置」第 2 条）修好之后，因为那道门槛读的正是这份判据。**在判据修好之前，不要在 adhd 上点这颗按钮。**
- 非目标: 不改采集准入、授权、工单字段与调度口径（**这正是挡住上面那一半的东西，本包有意不碰**）；不改巡查规则；不改 creator 路径；不改 `keyword_baselines_qualified` 的判据本体（`surface_scan_complete_sql!`）；不改 `linggan-evidence` 的读语义（含 schema 未就绪时返回空集的那一处，见验收记录 §5）；不部署、不切 runtime、不访问平台。

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `docs/current-state.md` | 权威当前 | 受保护交付、事实优先与分层完成边界 | 是 |
| UI execution contract | 权威当前 | 表面、状态、依赖、验收矩阵与含真实行动的 UI 边界 | 是 |
| `PAGE-COLLECTION-001` | 权威当前 | 关键词行的「档案」列语义、唯一主操作、抽屉决策区与回执位置 | 是 |
| LIDS data boundaries / language policy / tokens | 权威当前 | 中文独立承担状态与动作含义；层级用既有 `--lgi-z-toast`，不写死数字 | 是 |
| 数据/权限/行动合同 | 权威当前 | 建档两段（第一段 `deep_archive` 翻搜索面、第二段补详情）；受控 endpoint 与 durable receipt，前端不宣称成功 | 是 |
| 当前代码/测试 | 当前交付依据 | `target_primary_action` 的判据次序、处理函数「先第二段」的分叉、回执表两处缺口 | 是 |

## 变更分类与边界

- 分类: 状态语义 + 交互 + 展示的混合修复；**最高风险为真实行动**——按下去会真的发起一次平台采集请求。
- 依据: Issue #286；LIDS token 阶梯；既有受控 endpoint 与 durable receipt。
- Pattern: L1 Collection Control 内的目标抽屉与列表行共用同一判据函数；不新增 Token、Primitive、CMP、Scene 或 Motion。
- 表面与状态: `/collection/targets` 的关键词行与 `?drawer=<keyword>`；`KeywordArchiveRead::{NotArchived,DetailPending,Complete,Unavailable}` 与回执码。
- 状态词典: 「建过档没有」排在「是否在观察」之前（巡查只取每期增量，历史那一整段只有建档拿得回来）；**已暂停**仍是「恢复巡查」优先（#283 的既有决定，本包不动，其后果记在验收记录 §5）；空的候选集不再被读成「已补齐」；建档态读不到时不发采集，也不把它压成任一已知状态；「当前状态不接受这次请求」与「请求没送出去」分成两条回执——前者重试一辈子都一样，后者过一会儿就好，**这条分类两段共用一处**（`keyword_archive_error_receipt`）：分写在两个分支里，就一定有一段会漏掉，而漏掉的那一段会用「稍后可以重试」去说一条永久的状态前置。
- 文件: `apps/api/src/local_web/target_drawer.css`、`apps/api/src/local_web/target_drawer.rs`、`apps/api/src/local_web/collection_targets_view.rs`、`apps/api/src/local_web.rs`、`apps/api/src/local_web/tests.rs`、本文、验收记录、两个索引与当月 progress。
- 停止条件: 若需改变采集准入/授权/工单字段、巡查口径、`surface_scan_complete_sql!` 判据或 creator 行为，停止并上报。**本包已在这条线上停住一次**：观察中的关键词按建档会被准入拒绝，放宽它属于这类改动，因此只改回执、不改准入，并把决定交给 Mog。

## 验收矩阵与交接

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | focused 判据测试（观察中/未观察的未建档词都得到「建立档案」）+ 隔离 PostgreSQL 走产品路由 | 见验收记录 | 浏览器点击；**观察中的词按下去仍发不出采集**（准入闸，待 Mog 决定） |
| 状态诚实 | 建档态读不到时不发采集并说明原因；状态前置的拒绝不再写成「稍后重试」（两段共用同一条分类）；已建档的观察中词不被建档按钮打断 | 见验收记录 | 真实数据库故障下的回执；**第二段那条分类只有判据级证明（含双向变异），没有端到端用例**，见验收记录 §5 与本清单的证明缺口说明 |
| 视觉一致 | 回执改用既有 `--lgi-z-toast`；真实样式表按产品加载顺序做层叠复核（含反向对照） | 见验收记录 | 产品页实拍 |
| 真实后果 | 隔离 PostgreSQL 上两条：未观察的未建档词 → `archive_requested` + 一张 `deep_archive` 工单；观察中的未建档词 → `keyword_archive_not_requestable` + 工单数为 0 | 见验收记录 | 平台采集是否成功、线上是否已部署 |

- 索引同步: `docs/README.md`、`docs/design/README.md`、`docs/progress/2026-09.md`。
- 例外: 上一包 `ACC-COLLECTION-ACTION-001` 的验收句「已开启巡查的词仍是『当前无需处理』」在本包被有意改变——那句话的前提（这个词已经建过档）对一个还没有底座的词不成立。
- PR / reviewer / integration owner: PR #287（`fix/keyword-archive-002` → `main`）；独立复审三轮（第三轮在提交前，审核员本机复跑编译、单测与隔离 PostgreSQL 全套）；**合并由 Mog 决定**。
