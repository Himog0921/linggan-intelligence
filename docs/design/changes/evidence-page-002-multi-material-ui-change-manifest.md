# EVIDENCE-PAGE-002 · 多材料证据库产品与原型变更清单

> 状态: 权威当前
> 最后核对: 2026-08-28
> 适用范围: `PAGE-EVIDENCE-001` 的多材料产品升级、技术呈现要求与合成静态原型
> 事实来源: Mog 批准的五卡计划、Issue #85 Claim、MEDIA-RECON-001、当前 PAGE、LIDS、UI execution contract 与当前运行时代码
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同；本清单不授权后端、运行时或真实平台动作

## 1. 事项

- Issue / SCOPE：Issue #85；五卡计划卡 2 `EVIDENCE-PAGE-002`。
- Agent / worktree：`/root/evidence_product_design_agent`；`codex/issue-85-evidence-page-product-design`；独立 worktree `evidence-page-product-design`。
- Stacked base：PR #84 exact head `2709d5dd8671d286b396e50ff4a1ed753d3c3bfe`。
- 目标：把“语料 → 证据库”从 Discovery-only 产品定义升级为可直接交付给卡 3/4 的多材料页面合同和完整合成高保真原型。
- 用户可见结果：以来源作品为主对象，分 lane 判断发现、详情、评论/回复、作者、媒体槽位/字节与 OCR/ASR 的状态，并在 Inspector 核验来源、Coverage、限制和敏感展示边界。
- 明确非目标：Rust/SQL/API/plugin/runtime HTML/CSS、global LIDS Token、真实平台、真实材料、媒体下载、OCR/ASR、部署、合并。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md`、`docs/README.md`、`docs/current-state.md` | 权威当前 | 文件治理、事实层级、卡 1 和当前运行边界 | 2026-08-28 |
| `docs/governance/*` | 权威当前 | 文件位置、索引、生成物、协作与收口 | 2026-08-28 |
| UI execution contract | 权威当前 | 表面/状态/依赖/验收矩阵和停止条件 | 2026-08-28 |
| `PAGE-EVIDENCE-001` 与现有 manifest/acceptance | 权威当前/历史一次性证据 | Discovery-only 当前能力、V7 几何、中文优先与 honest state | 2026-08-28 |
| `MEDIA-RECON-001` / PR #84 exact head | 权威当前 Draft input | 材料主对象、lane、媒体、派生、状态、read minimum | 2026-08-28 |
| `LIDS-SYS/TOK/PRI/PAT/AGENT/LANG` | 权威当前 | 唯一视觉语言、Pattern、状态、中文、响应式和 a11y | 2026-08-28 |
| `product-interface-architecture.md` / `page-map.md` / domain invariants | 草案 + 权威不变量 | Corpus 任务、敏感材料、页面非事实源、unknown/partial 边界 | 2026-08-28 |
| 当前 Evidence Library Rust/CSS/tests | 代码事实优先 | 当前 runtime 仍以 Discovery-only 语言/投影为主；本卡不修改 | 2026-08-28 |
| Issue #85 / Claim | 当前授权 | 文件所有权、stacked base、必交付、禁止项 | 2026-08-28 |

## 3. 方向锁的五项答案

方向已由项目权威来源回答，未重新向用户提问：

1. 用户/情境：Mog 及小团队在桌面 App Shell 内核验材料。
2. 美学：纯白台面上的精密情报基础设施；编辑式高密度 + 仪器式读数；粗野主义只作重音。
3. 记忆点：作品行中的材料 lane 带，不用总完整度压缩状态。
4. 约束：LIDS Token、中文主表达、敏感默认脱敏、键盘/Reduced Motion、静态合成、无 runtime 修改。
5. 签名交互：选择作品集合，同一工作面同步 Inspector；移动端顺序折叠。

- Visual thesis：高密度但克制的白色证据台面，以 lane 状态和来源血缘为唯一视觉核心。
- Content plan：方位/查询 → 作品集合 → 当前材料 → 来源/限制；无 Hero。
- Interaction thesis：选择行、切换 Inspector、小范围筛选；不伪造写动作。
- CSS strategy：原生 CSS only；静态 HTML 消费现有 `lids_tokens.css`。
- Radius：LIDS `0/2/4/8px`；不使用 Pill 或新圆角体系。

## 4. 变更分类

- 分类：展示 + 交互 + 状态/语义（静态原型）；不含权限/行动实现。
- 最高风险类别：状态/语义。
- 对应来源：`PAGE-EVIDENCE-001`、`LIDS-SYS-001`、`LIDS-PRI-001`、`LIDS-PAT-001`、`LIDS-LANG-001`、MEDIA-RECON §6–7。
- 风险控制：原型全局与材料区域持续标记 `SYNTHETIC / NOT LIVE`；所有数据人工合成；不出现运行成功回执。
- 是否存在 `DECISION_REQUIRED`：否。产品主对象、Pattern、lane、媒体合同、中文与视觉方向均已冻结。
- L1/L2/L3：L1 Corpus Explorer + embedded L2 Split Evidence Inspector；无 L3。
- 触及范围：更新 Page 层和 Data Truth 的静态表达；不触及 Token、Primitive、CMP、Scene 或 runtime Motion。

## 5. 表面地图

| 表面 | 结果 | 所有权 |
|---|---|---|
| 共享页头 / context / rail | 原型按现有职责复现，不修改 runtime shell | shared shell owner |
| 查询 / filter / receipt | 定义本地材料读语义与可恢复状态 | PAGE-EVIDENCE-001 / 卡 3 API |
| 来源作品材料集合 | 主对象与 lane 带 | 本卡 Page-local candidate |
| Provenance Inspector | 概览、讨论、媒体/派生、来源/限制 | 本卡 Page-local candidate |
| Empty / read failure / restricted | Inline feedback，不用 toast/modal | 本卡 Page spec + read contract |
| Mobile sequential flow | rail → query → results → inspector | 本卡 responsive spec |

## 6. 状态词典

本卡采用 MEDIA-RECON 的闭集：`NOT_REQUESTED / QUEUED / NOT_OBSERVED / OBSERVED / PARTIAL / ACQUIRED / PROCESSING / NOT_ENABLED / SEARCHABLE / FAILED / RISK_CONTROL / BYTES_CLEANED / WITHDRAWN_OR_RESTRICTED / UNKNOWN`。

原型另外展示页面级：`SYNTHETIC_REFERENCE / NO_MATCHING_MATERIAL / READ_PROJECTION_UNAVAILABLE / ACCESS_RESTRICTED`。不创造万能作品状态，不显示总完整度，不把 unknown 写 0。

## 7. 依赖地图

| 依赖 | 需要什么 | 本卡输出 | 未解决时影响 |
|---|---|---|---|
| 卡 1 / PR #84 | 媒体/材料语义 | 完整消费映射 | 已满足 exact head |
| 卡 3 / #86 | 作品聚合、lane、Inspector/read envelope | 最小字段和筛选合同已发送给实现 Agent | 后端未完成不影响静态原型，但阻止卡 4 运行接线 |
| 卡 4 | 运行时页面实现 | PAGE、HTML reference、acceptance matrix | 本卡不实现 |
| 卡 5 | 真实链证明 | 场景阶梯与未证明边界 | 本卡无真实动作 |

## 8. 文件与影响边界

- 修改：`docs/design/pages/evidence-library-page.md`。
- 新增：`docs/design/pages/evidence-library-multi-material-reference.html`、本清单、对应验收、reference verifier。
- 同步：`docs/README.md`、`docs/current-state.md`、`docs/design/README.md`、`docs/design/lids/migration-log.md`、`docs/progress/2026-08.md`。
- 禁止：`apps/`、`crates/`、`database/`、`plugins/`、`references/`、runtime CSS/HTML、global tokens。
- 停止条件：需要修改媒体/data identity、权限/隐私、真实动作、共享 token 或 backend contract。

## 9. 验收矩阵

| 层级 | 验收方法 | 预期/实际证据 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 原型选择作品 → Inspector | 浏览器走查与 DOM verifier | runtime API/交互 |
| 状态诚实 | 搜索禁词/必需状态；切换 empty/error | verifier + 人工走查 | 真实状态来源 |
| 视觉一致 | 实际桌面/窄屏截图与 overflow 测量 | ACC 记录 | Mog 最终审美验收 |
| 可访问性 | landmark、button、tab、focus、Reduced Motion | DOM/CSS 检查 + 键盘走查 | 完整辅助技术认证 |
| 真实后果 | diff 应无 runtime/backend；HTML 无外部调用 | diff path audit | 采集、媒体、OCR/ASR |

## 10. 交接

- 页面实现 Agent 必须消费卡 3 的版本化 read model；不得从 raw Package 或数据库表自行组装本页。
- 原型中的 lane 带、作品行和 Inspector section 不晋升 CMP；后续需要第二页面证明。
- 旧 Discovery-only runtime 继续保持事实有效，直到卡 3/4 真正接通；本卡不修改其文案或页面。
- stacked PR base 固定为 `codex/issue-14-media-lifecycle-reconciliation-v2`，`Refs #85`，Draft，不合并。
