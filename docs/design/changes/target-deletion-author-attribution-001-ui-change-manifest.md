# TARGET-DELETION-AUTHOR-001 · Targets 撤回动作 UI 变更清单

> 状态: 活跃计划
> 最后核对: 2026-09-09
> 适用范围: `/collection/targets` 行级停止/恢复观察、删除确认面板及其真实回执
> 事实来源: Issue #214、TARGET-DELETION-AUTHOR-001 实施计划、`PAGE-COLLECTION-001`、LIDS、后端删除/巡查合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、后端回执和真实运行；本清单不自行增加动作权限

## 1. 事项

- Issue / SCOPE：Issue #214；不改变 SCOPE-001 的采集 Evidence 语义。
- Agent 与 worktree：`/root`；`codex/target-deletion-author-attribution-001`。
- 目标：把“先暂停”和“彻底撤回目标”变成有界、可理解、可证明的行级操作。
- 用户可见结果：停止观察后可立即恢复；删除先展示真实控制面/保留材料数量，并对受保护材料明确拒绝。
- 明确非目标：不新增导航、页面、远程采集、评论研究动作、自动重试或“删除作品/评论”能力。

## 2. 读取回执

| 来源 | 本次解决的问题 |
|---|---|
| `AGENTS.md`、`current-state.md`、Issue #214 | 受保护交付、独立 worktree、PR/发布边界。 |
| UI execution contract、design governance | 这是权限/行动与状态语义混合变更，必须有真实回执与清单。 |
| `PAGE-COLLECTION-001` | Targets 是 L1 Collection Control；未知、读取失败和行动后果不能由前端猜测。 |
| LIDS Primitive/Pattern/Data Boundary/Language/Agent Guide | 主动作不被替代；停止/删除是 Danger + 可恢复/确认；数字与未知按真实 preview 表达，中文独立承担含义。 |
| `collection_target.rs`、0063 migration、isolated PostgreSQL tests | 删除范围、作者 attribution 来源、外键清理顺序和保留事实由后端裁定。 |

## 3. 变更分类

- 分类：交互 + 状态语义 + 权限/行动。
- 最高风险类别：权限/行动；删除必须有后端原子事务与确认名，不能靠前端提示。
- L1 / L2 / Pattern：Targets 为 L1 Collection Control；删除确认是当前工作面的受限面板，不建第二页或组件体系。
- Data Truth：会删数量、会保留数量、是否受保护都来自同一个 `TargetDeletionPreview`；未知 preview 不显示可执行确认。

## 4. 状态与表面地图

| 表面/状态 | 用户理解 | 禁止暗示 |
|---|---|---|
| 停止观察 / 恢复观察 | 只影响今后的自动巡查，可恢复；历史材料保留。 | 已删除目标、已取消历史采集、立刻停止在途任务。 |
| 删除确认 | 将撤回控制面；作品/详情/评论/采集包/回执保留。 | 删除作品、作者身份消失、未显示的数量为零。 |
| 删除受阻 | 关联跨行业样本或人工失效结论，未执行任何删除。 | “删除失败但已删一部分”、可安全强删。 |
| 名称不匹配 / 读取失败 | 没有执行删除。 | 目标不存在或当前数据为零。 |

停止条件：后端预览、确认、事务删除或保留材料合同存在冲突时，停止受影响 UI，不用本地状态补全。

## 5. 验收与证明边界

| 层级 | 验收方法 | 未证明边界 |
|---|---|---|
| 任务可用 | SSR/source tests 与 local API 路由编译。 | Mog 真实操作体验。 |
| 状态诚实 | isolated PostgreSQL assertions：成功删除、受保护阻断、作者归属保留、空名称回退。 | 共享历史所有数据形状。 |
| 视觉一致 | 1440px 本机页面的删除面板、两种行级控制与 focus/取消路径。 | 窄屏、完整视觉验收。 |
| 真实后果 | 本机 migration checksum、runtime health；不在共享数据上实际删除目标。 | 真实删除后的业务验收、外部平台/插件。 |

## 6. 交接

- 修改：Targets renderer/CSS/route，删除与 attribution read model，0063 migration、fixture、PostgreSQL proof、发布登记与项目治理检查。
- 例外：无新增 token/CMP；停止操作在成功后直接显示“恢复观察”，它是可见 undo，不创建额外弹窗。
- PR / reviewer / integration owner：由 Mog 对本包授权的独立 reviewer 与 integration 流程决定；实现者不自行合并。
