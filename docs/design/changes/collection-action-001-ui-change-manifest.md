# COLLECTION-ACTION-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: `/collection/targets` 的 keyword Drawer 概览主操作与检查器读取失败状态
> 事实来源: Mog 2026-09-15 对 PR #284 的修复并合并授权、Issue #283、PAGE-COLLECTION-001、UI execution contract、LIDS 与当前代码/测试
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据合同与 Issue #283；本清单不授权新采集、部署或运行时切换

## 事项与读取回执

- Issue / SCOPE: Issue #283 / COLLECTION-ACTION-001；PR #284 exact base `6215619`。
- Agent 与 worktree: Codex `/root`，`/Users/moglenny/proma/linggan-intelligence/.worktrees/collection-action-001`。
- 用户可见目标: inspector 投影读失败时，keyword Drawer 仍以与列表行相同的 `KeywordArchiveRead → target_primary_action` 呈现建立档案或补采缺口；同时保留 inspector 事实不可读的提示。
- 非目标: 不改 archive / patrol 读模型、数据表、写入协议、权限或动作 endpoint；不改变 creator 失败行为；不部署、不切 runtime、不访问平台。

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `docs/current-state.md` | 权威当前 | 受保护交付、事实优先和分层完成边界 | 是 |
| UI execution contract | 权威当前 | 表面、状态、依赖、验收矩阵与混合 UI 的行动边界 | 是 |
| `PAGE-COLLECTION-001` | 权威当前 | keyword Drawer 的任务、唯一主动作和 `READ_UNAVAILABLE` 诚实表达 | 是 |
| TARGET-INSPECTOR-PERFORMANCE manifest / acceptance | 权威当前 / 历史验收 | 继承 L1 Collection Control + 受限 L2 Drawer，不另造页面骨架 | 是 |
| LIDS data boundaries / language policy / patterns | 权威当前 | 未知不等于失败或零；中文独立承担状态与动作含义 | 是 |
| Issue #283 与当前代码/测试 | 当前交付依据 | 已独立读 `KeywordArchiveRead` 却在 inspector 失败分支被丢弃的残余分叉 | 是 |

## 变更分类与边界

- 分类: 状态语义 + 交互 + 权限行动的混合修复；最高风险为权限行动。
- 依据: Issue #283 B、PAGE-COLLECTION-001 §3/§5；可见按钮仍重用既有受控 endpoint 与 durable receipt，不在前端宣称成功。
- Pattern: L1 Collection Control 内的受限 L2 Split Inspector；不新增 Token、Primitive、CMP、Scene 或 Motion。
- 表面与状态: `/collection/targets?drawer=<keyword>` 概览 Tab；`KeywordArchiveRead::{NotArchived,DetailPending,Complete,Unavailable}` 与 `TargetInspectorView::ReadUnavailable`。
- 状态词典: inspector 失败只意味着巡查与最近结果未知；关键词建档态由独立读取得出，不能被失败读压成“无需处理”。建档态自身 `Unavailable` 时，沿用既有无 archive 写操作的分支。
- 文件: `apps/api/src/local_web/target_drawer.rs`、其单元测试、本文、验收记录、两个索引与当月 progress。
- 停止条件: 若需改变 `target_primary_action`、archive endpoint、权限判断、真实采集范围或 creator 行为，停止并上报。

## 验收矩阵与交接

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 抽屉 render test 覆盖 inspector `ReadUnavailable` + `NotArchived` / `DetailPending` | focused 与 API 220 项单元测试通过 | 浏览器点击与真实后端 receipt |
| 状态诚实 | 同一 render 断言不可读提示和唯一主操作 | focused 与 API 220 项单元测试通过 | 真实 inspector 数据库故障 |
| 视觉一致 | 复用既有 Drawer section / decision markup、LIDS token 与中文文案 | SOURCE VERIFIED；未新增视觉 token 或布局 | 1440 浏览器视觉走查 |
| 真实后果 | 不改 endpoint；既有 handler 在 POST 时重算、返回 durable receipt | SOURCE VERIFIED | 未触发真实写入或采集 |

- 索引同步: `docs/README.md`、`docs/design/README.md`、`docs/progress/2026-09.md`。
- 例外: inspector 失败时不复制“健康”概览；keyword 的独立建档态在同屏作为另一项事实保留，creator 继续保持原有未知态。
- PR / reviewer / integration owner: PR #284；Codex `/root` 在 Mog 明确授权下修复、复审并集成新 exact head。
