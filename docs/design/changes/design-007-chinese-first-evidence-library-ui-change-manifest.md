# DESIGN-007 · Evidence Library 中文优先 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: Issue #65 对 `PAGE-EVIDENCE-001 /corpus/evidence` 的用户可见语言表达修订
> 事实来源: Mog 的中文优先确认、LIDS-LANG-001、PAGE-EVIDENCE-001、LOCAL-001C0、LIDS 与当前运行时页面
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同；本清单不扩大数据、行动或页面授权

## 1. 事项

- Issue / Scope: Issue #65；`DESIGN-007 / LIDS-LANG-001`
- 目标: 让 Evidence Library 的用户可见含义由中文独立承担；英文只保留为紧邻中文的技术旁注、品牌/固有名或短代码。
- 用户可见结果: 页面在已有 20 张已接纳 discovery 卡片、默认最新已接纳视角、严格发布时间窗口和空态中，均不再要求用户通过英文理解状态或按钮。
- 明确非目标: API/DB/数据合同、采集/插件、媒体生命周期、OCR/ASR、查询条件/排序、按钮行为、路由、共享 shell、其它页面、Token 和视觉结构。

## 2. 读取回执

| 来源 | 本次解决的问题 | 已核对 |
|---|---|---|
| `AGENTS.md`、`docs/README.md`、`docs/current-state.md` | 闭集执行、事实边界、Issue/worktree/PR 规则 | 2026-08-26 |
| `docs/agents/ui-execution-contract.md` | UI 变更分类、状态诚实性、验收与收口 | 2026-08-26 |
| `PAGE-EVIDENCE-001` 与 `LOCAL-001C0` 合同 | 当前页面任务、未知发布时间、Discovery-only、媒体未取得边界 | 2026-08-26 |
| `LIDS-SYS-001`、`LIDS-TOK-001`、`LIDS-PRI-001`、`LIDS-PAT-001`、`LIDS-AGENT-001` | L1/L2 组合、中文 Sans/技术 Mono、状态与页面语言 | 2026-08-26 |
| 用户截图与明确确认 | 英文技术键不应单独承担用户含义 | 2026-08-26 |

## 3. 变更分类与规则映射

- 分类: 展示 + 状态/语义。
- 最高风险: 状态/语义。文案改动不能把未知、未取得、仅发现面或部分 Coverage 变成不同状态。
- LIDS 强度 / 主 Pattern: 既有 L1 `Corpus Explorer` + 受限 L2 `Split Evidence Inspector`；不变。
- 实施规则: `LIDS-LANG-001 / LANG-01`–`LANG-04`、`LIDS-PRI-001` 排版/状态/禁用态、`PAGE-EVIDENCE-001`。
- 技术表达: 使用当前页局部 `.v7-tech-key`；不新增 Token、CMP、Shell、Scene 或 Motion。

## 4. 影响边界

- 页面: 只限 `/corpus/evidence`。
- 模板与测试: 可修改页面模板所在 `apps/api/src/local_web.rs` 和其 focused tests；仅替换页面文案/断言，不改 route、API、DB、查询或状态判定。
- 页面局部样式: 可为中英文主次关系添加 `.v7-tech-key`；不得改变当前三栏结构、尺寸、按钮可用性或数据来源。
- 禁止: `shell.rs` / `shell.css`、其它页面、合同行为、Ingress、Producer、媒体、迁移。
- 停止条件: 任何共享外壳、数据含义、行动、查询、状态来源或新视觉原则需求均标记 `DECISION_REQUIRED`。

### 4.1 审查收口修订（同一事项内的最小补正）

- 独立审查发现 Discovery 卡片的原始 `stopped_reason` 曾只显示英文代码；本次只在页面 renderer 中补上中文主语义，原始代码仍紧邻保留为技术旁注。未知或未来代码明确显示「停止原因未知／未归类」，不猜测为成功、完成或无更多结果。
- `.v7-tech-key` 与 Discovery 卡片内的动态技术键改为相对关联中文主文案的较小字号；技术键保持可见，但不能大于中文主表达。
- 共享 shell 中仍存在的孤立英文（例如页面共用的 `CORPUS · UNKNOWN` 或时区标记）影响其它页面，**明确不在 Issue #65 / DESIGN-007 改动范围内**；需另立事项确认其跨页语言与视觉合同。本事项不修改 `shell.rs` 或 `shell.css`。
- Standards 复核仅余的三处页面内英文整句，均在本事项的页面模板／renderer 中收口：结果表头改为「已接纳的发现卡片（`ACCEPTED DISCOVERY`）」；未接通时的本机呈现与未读材料分为两条中文说明（`LOCAL PRESENTATION`、`NO MATERIAL READ`）；默认视角以中文完整说明「当前显示最新已接纳的发现卡片；其中部分卡片的发布时间仍可能未知」，英文只保留 `LATEST ACCEPTED DISCOVERY · PUBLISHED_AT UNKNOWN` 技术旁注。此修订不改共享 shell、查询、数据或状态来源。
- 最终动态路径审查：成功读投影的查询行采用「只读取 Linggan 已接纳的发现卡片」作为中文主表达，`DISCOVERY ONLY` 仅为紧邻旁注；无效查询与读投影暂不可用分别呈现其准确中文状态，并通过页面局部槽位写入原始代码 `LOCAL_QUERY_INVALID` / `READ_PROJECTION_UNAVAILABLE`。两条路径都明确「未读取材料」，不再通过全局替换把它们误写为 `SOURCE_INCOMPLETE`、空库或已完成读取。

## 5. 证明计划与边界

| 层级 | 验收方式 | 本次不能证明 |
|---|---|---|
| 用户语言可读性 | DOM 文案审查：主按钮、导航、状态、空态、Inspector 均有中文主表达 | 翻译质量在其它页面已成立 |
| 状态诚实 | 保留 `PUBLISHED_AT UNKNOWN`、`MEDIA NOT ACQUIRED`、严格窗口排除和 Discovery-only 既有 tests | 平台发布时间、媒体、详情/评论已取得 |
| 视觉一致 | 既有 LIDS Token/CSS test + local DOM/浏览器检查 | 数据真实性或用户业务验收 |
| 自动检查 | focused Rust tests、fmt、clippy、governance check | 真实平台/插件/媒体/OCR/ASR/部署 |

## 6. 交接

- 文档同步: LIDS 索引、项目文档索引、PAGE-EVIDENCE-001、DESIGN-007 acceptance 和 2026-08 progress。
- PR: Draft PR，关联 `Refs #65`；由独立 Spec 与 Standards reviewer 审查 exact head。
- 禁止事项: 实现者不 merge、不关闭 Issue、不部署 local runtime；验证报告分开写已证明与未证明。
