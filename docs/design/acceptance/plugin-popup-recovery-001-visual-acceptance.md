# ACC-PLUGIN-POPUP-RECOVERY-001 · Popup 启动恢复验收

> 状态: 一次性报告
> 最后核对: 2026-08-26
> 适用范围: Issue #53 / Draft PR #54；`Linggan Intelligence Browser v0.4.2` source、build、release 与启动失败表达
> 事实来源: `PLUGIN-POPUP-RECOVERY-001`、`PAGE-PLUGIN-001`、LIDS、实际 source、测试、build 与 release 检查
> 冲突时以谁为准: 实际浏览器可见结果、当前代码/合同与用户最新确认；本记录不把 build、静态检查或 ZIP 说成 Chrome、平台或业务验证

## 1. 验收对象

- **Issue / SCOPE:** Issue #53 / Draft PR #54；这是 P0 popup 启动恢复，不是新采集或媒体 SCOPE。
- **页面/组件/状态:** `PAGE-PLUGIN-001` toolbar popup；`POPUP_STARTUP_FAILURE` 与既有 normal popup 入口。
- **关联 PAGE / PAT / CMP / DS / ACC ID:** `PAGE-PLUGIN-001`、`LIDS-PAT-001`、`InstrumentSurface`、`ACC-PLUGIN-POPUP-RECOVERY-001`；无新增 CMP、DS、Scene 或 Motion。
- **验收日期与环境:** 2026-08-26，隔离 worktree 的 Node build/test/release 环境；没有启动或控制 Chrome。
- **适用数据/权限前提:** 没有真实平台材料、tab、Cookie、账号或本机实际 Task/receipt。失败提示自身不能发起新的采集或传输。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 正常 popup 首次渲染 | 打开既有 popup | formatter 可解析，既有 `App` 入口仍存在；不等于 host、任务或 Evidence 已成功 | 正常 popup 不应被空白错误路由替代 | 本卡不要求产生 action 或 receipt | **自动检查 VERIFIED；Chrome 视觉 NOT VERIFIED**。受控 React harness 将当前 source 执行到首次 render |
| `POPUP_STARTUP_FAILURE` | 识别故障并知道下一步 | 本机状态未知；此提示没有发起新的采集或传输；不代表失败前未读取页面上下文 | L1 单列 failure surface：版本 readout、失败标题、未知状态和重新加载指引；只消费 `--lgi-*` | 无按钮 action、无 receipt | **自动检查 VERIFIED；Chrome 视觉 NOT VERIFIED**。Error Boundary 的 normal/fallback 行为与文本经测试 |
| local host / Discovery / receipt | 用户原本可在正常 popup 内判断本机准备度或有限 Discovery | 本卡不改变既有语义；failure 不得冒充 `LOCAL_INGRESS_READY`、`ACCEPTED`、`REPLAY` 或 Evidence 入库 | N/A：本卡没有实际 host 或 package 画面走查 | N/A：不发起健康检查或 Discovery | **N/A / NOT ATTEMPTED**，没有虚构数据或回执 |
| 平台、权限、媒体与研究 | 任何真实采集/处理 | 不属于此修复 | N/A | N/A | **N/A / NOT ATTEMPTED**；无 XHS、账号、Cookie、媒体、OCR/ASR 或研究调用 |

## 3. 视觉工作条件

- **声明的桌面工作区/视口:** `popup.css` 的 source 声明宽度为 368px；本轮没有实际 Chrome popup 视口，因此不把该源码值写成视觉已验收尺寸。
- **输入内容长度与数据密度:** fallback 为固定短文案，不显示平台卡片、标题、作者、正文、评论、媒体或 receipt；真实内容密度为 N/A。
- **已批准的设计规则:** `PAGE-PLUGIN-001`、`PLUGIN-POPUP-RECOVERY-001`、`docs/agents/ui-execution-contract.md`。
- **LIDS 强度 / Pattern / Token 依据:** `L1 / Settings / Governance`；既有 `InstrumentSurface` 与 L1 可读错误反馈；新 failure CSS 只使用构建副本中的 `--lgi-*`，2px 为已批准结构线。
- **LIDS 状态五轴或合成边界依据:** failure 只表达 `UNKNOWN`，明确不提升为 host、任务、receipt、Evidence 或平台状态；无合成数据。
- **Reduced Motion / 移动或静态 Poster 降级:** failure route 不新增动画或 Motion；移动/缩放实际视觉为 **NOT VERIFIED**，因为未启动 Chrome。
- **已检查的响应式/可访问性条件:** source 有 `aria-live="polite"`；实际屏幕阅读器、键盘、缩放与窄宽度行为均为 **NOT VERIFIED**。
- **截图或录屏证据位置及生成物登记状态:** 无截图、录屏或生成视觉资产；**N/A**。本报告不以 source/build 替代视觉截图。

未登记的截图和录屏不得提交 Git。视觉证据只能证明所述情形下的呈现，不能证明数据正确、后端已执行或业务结果已发生。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | **VERIFIED** | `PAGE-PLUGIN-001`、UI change manifest、LIDS migration log 与 source/release 闭集对齐；failure 只定义未知状态与 reload 指引 | 没有真实 Chrome 视觉走查，不能将此项读作像素或可用性验收 |
| 前端/组件实现 | **VERIFIED** | `startupRecovery.js`、popup entry、HTML token load、CSS、release verifier 与 production build 都存在并通过 | 浏览器实际加载、Error Boundary 在真实扩展内捕获的表现未验证 |
| 自动检查 | **VERIFIED** | `npm run verify`：contracts、27 tests、production build、release package、reproducibility、runtime isolation；项目治理检查和 diff check 通过 | controlled React harness 不等于 Chrome、Service Worker 或用户设备 |
| 真实 Chrome / 浏览器链 | **NOT VERIFIED** | 本事项故意未启动、控制或重新加载 Chrome | 未确认 v0.4.2 是否被浏览器实际加载、popup 是否可见、辅助技术或视口表现 |
| 真实平台链 | **NOT VERIFIED** | 没有平台访问、tab、Cookie、账号、XHS/Douyin 或真实材料操作 | 未证明 Discovery、receipt、Evidence、媒体、OCR/ASR 或研究链路 |
| Deploy | **N/A** | 本地私有插件 release 的 Draft PR；没有部署授权或部署动作 | PR 未合并，用户未重新加载 v0.4.2 |
| Mog / 业务验收 | **NOT VERIFIED** | Mog 尚未在实际 Chrome 中确认 popup 恢复 | 不能恢复 REAL-CANARY-001，直至独立审查、合并与用户 reload/可见确认完成 |

## 5. 发现与后续

- **发现的规格冲突:** 原 fallback 曾绝对表示“本次没有读取、采集或传输任何数据”，但 Error Boundary 也可能捕获既有 `App` 后续渲染错误；已改为仅对提示自身做无新动作陈述。
- **是否需要 `DECISION_REQUIRED`:** 否。本卡不改变任务、数据、权限或平台范围。
- **是否需要设计例外或长期决定:** 否。failure 样式使用现有 LIDS token 和 Primitive；不产生第二 token 系统或 CMP。
- **不得因此推断的结论:** build、ZIP、自动测试、规格一致都不能证明 Chrome 已加载、popup 用户可见、平台可访问、真实采集接纳、Evidence 页面更新或业务结果。真实 Canary 继续暂停，直到独立审查、合并和用户重新加载 v0.4.2 后再单独验证。
