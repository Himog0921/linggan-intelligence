# LOCAL-001A · Evidence Library UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: Issue #25 的首次本地 Evidence Library 页面实现，以及 Issue #29 的受限 V7 精确视觉/骨架修订
> 事实来源: Issue #25、Issue #29、LOCAL-001、PAGE-EVIDENCE-001、LIDS 与当前实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、UI 执行合同、真实代码/合同；本清单不扩大数据或行动授权

## 1. 事项

- Issue / Scope: Issue #25（host）与 Issue #29（V7 fidelity）；`LOCAL-001 / 001A`
- Agent 与 worktree: Issue #25 的 host 交付保持原记录；Issue #29 的 V7 修订由 `codex:/root/implement_v7_fidelity` 在 `/Users/moglenny/.proma/agent-workspaces/linggan-intelligence/issue-29-v7-evidence-library-fidelity` 完成。
- 目标: 建立 loopback Rust host 与真实空态 Evidence Library 路由
- 用户可见结果: 访问 `http://localhost:3000` 会临时进入 `/corpus/evidence`，看到 V7 的精确页头、三栏、FACT LAYER 与 Inspector 骨架；所有未接通内容保持诚实空态并禁用控件
- 明确非目标: 数据库、插件、真实平台、媒体、OCR/ASR、Agent、旧内容工作台、部署和任何写动作

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `current-state.md` | 已读 | 授权层级、Unknown/Coverage、不接旧系统和当前实现边界 | 2026-08-24 |
| UI execution contract | 已读 | UI 前置、状态诚实、页面规格与收口要求 | 2026-08-24 |
| Local-001 计划 / Issue #25 | 已读 | 001A 用户任务、禁止项、停止条件 | 2026-08-24 |
| `REF-V7-001` / reference register | 已读 | 本页精确视觉/骨架 Gold Master 与不能继承的模拟数据/运行态边界 | 2026-08-25 |
| LIDS Token / Primitive / Pattern / Agent guide | 已读 | token 映射、L1+Inspector、状态和交互约束 | 2026-08-24 |
| 数据/权限/行动合同 | 已读，来源不足 | 001A 只能表达未接通，不可读取/展示材料 | 2026-08-24 |
| 当前 Rust 代码/测试 | 已读 | `apps/api` 是 bootstrap，可在最小 host 内实施 | 2026-08-24 |

## 3. 变更分类

- 分类: 混合（展示 + 状态语义）
- 最高风险类别: 状态语义
- 对应来源 ID: `PAGE-EVIDENCE-001`、`LIDS-PRI-001`、`LIDS-PAT-001`、`LOCAL-001-UI-EX-01 / Issue #29 修订`
- 为什么该类别足以覆盖本次风险: 页面唯一可见信息是“当前知道/不知道什么”；因此不把未接通状态压为无数据、成功或失败比视觉复杂度优先。
- 是否存在 `DECISION_REQUIRED`: 否。真实材料读取留给 001B；这不是本卡未解决后暗中实现的内容。
- L1 / L2 / L3 与主 Pattern: L1 `Corpus Explorer`，嵌入右侧 L2 `Split Evidence Inspector`；没有 L3。
- 是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth: 不修改 LIDS 全局 token 真源；页面只新增 `--v7-*` 局部变量以承载本页已授权的精确视觉值。没有新 CMP、Scene 或 Motion，Data Truth 边界不变。

## 4. 影响边界

- 受影响的页面: 仅 `/corpus/evidence`
- 受影响的组件: 无跨页面组件；页面内结构由 HTML/CSS 局部实现
- 受影响的状态: `SOURCE_INCOMPLETE`、`NOT_CONNECTED`、`UNKNOWN`、页面级无可展示材料
- 是否影响数据口径、权限、敏感展示或真实行动: 否；没有 data read/write，且不显示材料
- 禁止修改的文件/能力: Issue #25 Claim 的所有 forbidden 范围，尤其是数据/迁移/插件/旧工作台/平台
- 停止条件: 如果需要真实 read contract、状态来源、材料/权限或外部网络，停止在当前空态并创建下一卡

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | local host + `GET /corpus/evidence` | 见 `ACC-EVIDENCE-001` | 页面不读取真实材料 |
| 状态诚实 | route/content tests + 浏览器文案走查 | 见 `ACC-EVIDENCE-001` | 没有真实 partial/failed/stale 合同 |
| 视觉一致 | V7 精确结构/几何/颜色检查 + 四视口 local walk-through | 见 `ACC-EVIDENCE-001` | 不继承 V7 的模拟内容、运行状态或动作结果 |
| 真实后果 | loopback listener and HTTP response only | 见 runbook | 不证明 DB、插件、平台、部署或业务结果 |

## 6. 交接

- 修改文件: Claim 内 Rust、CSS、Page Spec、Manifest、Acceptance、Runbook、LIDS migration log、docs index/progress
- 验证命令/走查: `cargo fmt --all -- --check`、`cargo clippy --workspace -- -D warnings`、`cargo test --workspace`、loopback `curl`、独立 Chrome 四视口截图/几何检查、`./scripts/check-project-governance.sh origin/main`
- 规则或索引同步: `docs/README.md`、`docs/design/lids/migration-log.md`、`docs/progress/2026-08.md`
- 例外与替代: `LOCAL-001-UI-EX-01 / Issue #29 修订` 仅授权本页的完整 V7 视觉/骨架：128px 页头、216px rail、440px inspector 和 V7 色值/硬线/断点。它不替代 LIDS，也不建立第二套全局 token。`lids_tokens.css` 的 107 个 token 继续是本地运行时唯一全局值编辑源；`evidence_library.css` 不声明 `--lgi-*` token。
- LIDS migration log / 预览同步: runtime page is the preview; no generated screenshot is committed
- PR / reviewer / integration owner: draft PR after checks; reviewer and `/root` integration owner remain non-implementers
