# PLUGIN-POPUP-RECOVERY-001 UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: Issue #53 / Draft PR #54；`Linggan Intelligence Browser v0.4.2` 工具栏 popup 启动故障恢复
> 事实来源: Issue #53、`PAGE-PLUGIN-001`、当前 MV3 source、`PLUGIN-MIGRATION-001` 历史边界与 LIDS
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前插件合同与实际运行；本清单不扩大采集、权限或平台访问

本清单补充 `PLUGIN-MIGRATION-001`，不替代其完整 Producer Runtime 合同。

## 1. 事项

- **Issue / SCOPE:** GitHub Issue #53；Draft PR #54；不是新的采集、媒体或页面 SCOPE。
- **Agent 与 worktree:** `codex/issue-53-popup-recovery`，隔离 worktree；不接触用户已安装扩展、Chrome 或平台页面。
- **目标:** v0.4.0 初始 popup 渲染缺少 formatter 导入而空白。v0.4.2 必须保留正常 popup，并在 React 渲染错误时显示可读、可恢复而不虚构状态的 L1 提示。
- **用户可见结果:** 正常时仍显示既有 popup；渲染错误时显示「插件界面未能启动」、状态未知、该提示没有发起新的采集或传输，以及明确的重新加载版本动作。
- **明确非目标:** 不重画既有 popup 或页面浮条；不修改采集器、后台、TaskSpec、回传、权限、host permission、Cookie、账号、媒体、OCR/ASR、Linggan API、数据库或任何平台访问。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `current-state.md` | 已读 | 闭集范围、真实 Canary 暂停与完成声明边界 | 2026-08-26 |
| `docs/agents/ui-execution-contract.md` | 已读 | L1 状态文案、停止条件与 UI 交接 | 2026-08-26 |
| `docs/design/pages/plugin-producer-popup.md` | 已读并同步 | `PAGE-PLUGIN-001` 的 Package/host/Discovery 状态不能被 fallback 冒充 | 2026-08-26 |
| `docs/design/lids/{README,tokens,primitives,patterns}.md` | 已读并同步 | 新 fallback 只消费已批准 `--lgi-*` token，并使用 L1 Settings/Governance | 2026-08-26 |
| `docs/design/lids/migration-log.md` | 已读并同步 | 记录唯一 token 源被复制进 release 的局部实现 | 2026-08-26 |
| 数据、权限与行动合同 | 已读 | fallback 不触发新采集或传输；不能断言此前渲染没有读取页面信息 | 2026-08-26 |
| 当前 source、v0.4.0 故障证据与测试 | 已读并替换测试 | 用受控首次渲染复现缺失导入，而非只匹配源码字符串 | 2026-08-26 |

## 3. 变更分类

- **分类:** 状态语义 + 展示；无新增交互、权限或真实行动。
- **最高风险类别:** 状态诚实。Error Boundary 可能在既有 `App` 读取页面上下文后才捕获 rerender 错误，因此 fallback 只能说本机状态未知，不能绝对声称“本次没有读取任何数据”。
- **对应来源 ID:** `PAGE-PLUGIN-001`、`LIDS-PAT-001`、`PLUGIN-MIGRATION-001`、Issue #53。
- **为什么足以覆盖风险:** 正常路径保持原入口；失败路径不进入 Dashboard 或任何替代页面，不发起新 collector/write，只给出可恢复人工动作。
- **是否存在 `DECISION_REQUIRED`:** 否；不改变既有任务、数据或授权语义。
- **L1 / L2 / L3 与主 Pattern:** `L1 / Settings / Governance`；局部启动失败状态，不新增 Page、CMP 或 Scene。
- **Token / Primitive / CMP / Scene / Motion / Data Truth:** 构建时从唯一 runtime token 源复制 `themes/lids-tokens.css`；新 CSS 仅使用 `--lgi-*`。采用既有 `InstrumentSurface` 与 L1 可读错误反馈；2px 是 LIDS 已批准结构线。无 token 值变更、无新 Primitive/CMP/Scene/Motion，Data Truth 仅表达 `UNKNOWN`，不表达 health、接纳或 Evidence。

## 4. 影响边界

- **受影响页面:** `PAGE-PLUGIN-001` 的 toolbar popup，且仅新增其渲染失败路线。
- **受影响组件:** `src/popup/index.jsx`、`src/popup/startupRecovery.js`、popup HTML/CSS、build copy、release verification 与回归测试。
- **受影响状态:** `POPUP_STARTUP_FAILURE`（页面自身启动失败）；不等同于 `LOCAL_INGRESS_READY`、`ACCEPTED`、`REPLAY`、`NOT_ACCEPTED` 或 Evidence 入库。
- **数据、权限、敏感展示或真实行动:** 无；fallback 自身不触发新采集或传输。它不能证明 App 在失败前没有读取 tab/context/storage。
- **禁止修改的文件/能力:** Collector、content/background runtime、TaskSpec/receipt、manifest permission/host permission、Linggan API/数据库、页面浮条、真实浏览器配置与所有平台页面。
- **停止条件:** 需要新增 host permission、读取 Cookie/账号、调用小红书、修改采集/回传、接触真实材料或改变 Linggan API/数据库时，停止并另立事项。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 受控 React harness 删除 v0.4.0 import 后在首次 `App` render 抛出；当前 source 正常完成首次 render | 待 PR 新 head 的 `npm run verify` | Chrome 实际点击、Service Worker 生命周期与用户界面 |
| 状态诚实 | Boundary normal/fallback 行为测试；fallback 只说状态未知及“本提示”没有新采集/传输 | 待 PR 新 head 的 `npm run verify` | 失败前既有 App 是否已读取上下文；任何 health、任务或入库状态 |
| 视觉一致 | popup 加载 release 内复制的 canonical LIDS token；测试与 release verifier 检查该 CSS | 待 PR 新 head 的 build/release check | Chrome 像素截图、辅助技术走查与 Mog 视觉验收 |
| 真实后果 | 无真实 Chrome/平台/材料/写入动作；此事项故意不验证 | `NOT ATTEMPTED` | XHS、账号、Cookie、Discovery、receipt、Evidence、媒体、OCR/ASR 或研究能力 |

## 6. 交接

- **修改文件:** popup import/boundary/recovery/CSS/HTML、webpack copy、release verifier、popup startup test、package/lock/manifest、v0.4.2 ZIP 与 release manifest；本清单、PAGE、LIDS migration log、visual acceptance、current-state 与 2026-08 progress。
- **验证命令/走查:** `npm ci --ignore-scripts`、`npm run build`、`npm run package:release`、`npm run release:manifest`、`npm run verify`、`./scripts/check-project-governance.sh`、`git diff --check`。这些不是 Chrome、平台或真实采集证明。
- **规则或索引同步:** `docs/README.md` 已登记本清单与 visual acceptance；状态与 progress 在同一 PR 更新。
- **例外与替代:** 不使用 Dashboard 作为静默 fallback；不新增一个第二 token 文件，release CSS 只由 canonical `apps/api/src/local_web/lids_tokens.css` 复制。
- **LIDS migration log / 预览同步:** 同 PR 追加 2026-08-26 Issue #53 记录；visual acceptance 保留 `NOT VERIFIED` 的 Chrome/真实链路边界。
- **PR / reviewer / integration owner:** Draft PR #54，引用（不是关闭）Issue #53；修订 head 必须由原 Spec 与 Standards 审查者重新审查。实施者不自行 merge。
