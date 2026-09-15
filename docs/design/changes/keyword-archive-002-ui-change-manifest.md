# KEYWORD-ARCHIVE-002 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: `/collection/targets` 关键词行的「档案」列与主操作、抽屉决策区、右下角回执，以及关键词的规则命令、巡查准入与调度拒绝
> 事实来源: Mog 2026-09-15 的修复授权、Issue #286、PAGE-COLLECTION-001、UI execution contract、LIDS 与当前代码/测试/隔离 PostgreSQL 证明
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据合同与 Issue #286；本清单不授权新的采集口径、部署或运行时切换

## 事项与读取回执

- Issue / SCOPE: Issue #286 / KEYWORD-ARCHIVE-002；实际分支点 `origin/main@090ec80f72ac87a491d91309cfd54230ab238a36`（交付期间 main 数次前进，本分支随之变基：`dab0be3` → `090ec80`；合并前已核实与当时的 `origin/main@36efc4b` 无冲突）。
- Agent 与 worktree: Claude（harness 会话），`/Users/moglenny/proma/linggan-intelligence/.worktrees/keyword-archive-002`。
- 用户可见目标: 关键词没有完成搜索面建档或仍欠详情时，唯一主操作是「建立档案」或「补采缺口」；读取未知时只给「查看档案」，不提供设置/恢复巡查。两段完成后才显示「开始每周巡检」。这条顺序对列表与抽屉完全一致。
- 真实控制目标: 关键词 SaveRule/Resume、旧 target-level toggle、人工观察和定时调度共享同一完成判据。未完成或不可读写既有闭集原因 `baseline_not_ready`；定时调度不产生 Work Order。页面以当次可读建档态解释“未完成”或“不可读”。`UnknownTarget` 的建档请求回 `keyword_archive_target_missing`，不再谎称可稍后重试。
- 非目标: 不放宽深度建档准入、不改授权/工单字段/采样口径、不自动改写历史关键词规则、不改 creator 路径、不部署、不切 runtime、不访问平台。

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
- 状态词典: 「建过档没有」排在「是否在观察」之前（巡查只取每期增量，历史那一整段只有建档拿得回来）；**已暂停且未建档**同样先「建立档案」，只有 `Complete + paused` 才可恢复巡查；空的候选集不再被读成「已补齐」；建档态读不到时不发采集，也不把它压成任一已知状态；「当前状态不接受这次请求」与「请求没送出去」分成两条回执——前者重试一辈子都一样，后者过一会儿就好，**这条分类两段共用一处**（`keyword_archive_error_receipt`）：分写在两个分支里，就一定有一段会漏掉，而漏掉的那一段会用「稍后可以重试」去说一条永久的状态前置。
- 文件: `crates/evidence/src/{acquisition_chain,collection_control,keyword_archive_detail,patrol_scheduler}.rs`、`apps/api/src/local_web/{target_drawer,collection_targets_view}.rs`、`apps/api/src/local_web.rs`、本文、产品规则、验收记录、LIDS 记录与当月 progress。
- 停止条件: 不改授权/工单字段、巡查采样口径或 creator 行为。历史关键词的 `monitoring/paused` 状态仅在**缺少合格搜索面基线**时允许创建修复性 `deep_archive`；已有搜索面而欠详情只走作品作用域的补采，仍由关键词巡查入口的完成判据阻止其进入 patrol。

## 验收矩阵与交接

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | focused 判据测试（观察中/暂停/未观察的未建档词都得到「建立档案」）+ 隔离 PostgreSQL 走产品路由 | 见验收记录 | 浏览器点击 |
| 状态诚实 | 建档态读不到时不发采集并说明原因；状态前置的拒绝不再写成「稍后重试」（两段共用同一条分类）；已建档的观察中词不被建档按钮打断 | 见验收记录 | 真实数据库故障下的回执；**第二段那条分类只有判据级证明（含双向变异），没有端到端用例**，见验收记录 §5 与本清单的证明缺口说明 |
| 视觉一致 | 回执改用既有 `--lgi-z-toast`；真实样式表按产品加载顺序做层叠复核（含反向对照） | 见验收记录 | 产品页实拍 |
| 真实后果 | 隔离 PostgreSQL 上两条：未观察的未建档词与历史观察中的未建档词 → `archive_requested` + 各一张修复性 `deep_archive` 工单；暂停态由同一准入分支与列表/抽屉判据覆盖 | 见验收记录 | 平台采集是否成功、线上是否已部署 |

- 索引同步: `docs/README.md`、`docs/design/README.md`、`docs/progress/2026-09.md`。
- 例外: 上一包 `ACC-COLLECTION-ACTION-001` 的验收句「已开启巡查的词仍是『当前无需处理』」在本包被有意改变——那句话的前提（这个词已经建过档）对一个还没有底座的词不成立。
- PR / reviewer / integration owner: PR #287（`fix/keyword-archive-002` → `main`）；独立复审三轮（第三轮在提交前，审核员本机复跑编译、单测与隔离 PostgreSQL 全套）；**合并由 Mog 决定**。
