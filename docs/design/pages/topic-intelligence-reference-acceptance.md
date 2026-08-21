# ACC-TOPIC-001 · Topic Intelligence Reference Page 验收记录

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: DESIGN-002 静态 Topic Intelligence Reference Page 的设计、浏览器和边界验收
> 事实来源: PAGE-TOPIC-001、当前静态 HTML、Issue #7、实际脚本输出与本地 Chrome 151.0.7922.172 走查
> 冲突时以谁为准: 真实运行/代码/合同、用户最新确认与当前 SCOPE；截图、演示或本记录不覆盖这些来源

## 1. 验收对象

- Issue / 计划：Issue #7 / DESIGN-002
- 页面：`docs/design/pages/topic-intelligence-reference.html`
- 关联规格：PAGE-TOPIC-001、DS-001–DS-007、PAT-001–PAT-004
- 验收日期与环境：2026-08-21；本地 Google Chrome `151.0.7922.172`，以 `file://` 打开静态源文件
- 数据/权限前提：全部是 `SYNTHETIC / NOT LIVE` 示例；没有真实数据、用户身份、权限、网络请求、API 或持久化

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 桌面初始页 | 识别 Topic、页面任务、样本模式与限制 | `SYNTHETIC_REFERENCE` + `SOURCE_INCOMPLETE` | 1440px 截图首屏可见模式条、Topic、范围、候选和边界；中心样本区为主 | 不适用；不创建任何真实记录 | VERIFIED（静态呈现） |
| 窄视图 | 连续阅读参考页 | 仅承诺可读参考，不是移动工作台 | CDP 设为 375px CSS 视口时：`innerWidth=375`、`documentWidth=360`、`bodyWidth=360`、`shellWidth=336`，无横向溢出 | 不适用 | VERIFIED（阅读边界） |
| 切换样本窗口 | 查看 7D 合成示例 | 只改变演示文案，不是查询/趋势刷新 | 点击后标签变为 `07D · SYNTHETIC`，模式条和限制仍可见 | 无真实查询或回执 | VERIFIED（局部演示） |
| 展开原声限制 | 理解原声示例为何不能当 Evidence | 展开的是“合成示例，不是 Evidence”说明 | 展开区域原位出现，不挤压核心状态 | 无真实来源读取 | VERIFIED（状态诚实） |
| 继续探索 | 表达页面中的下一步意图 | `LOCAL_INTENT_ONLY` | 选择后局部回显，不出现成功 toast 或假进度 | 回显明确写“未创建真实记录、研究、决定、行动、通知或待办” | VERIFIED（无副作用演示） |
| 减少动效 | 偏好减少动效时仍可读 | 含义不依赖过渡或动画 | CSS 含 `prefers-reduced-motion: reduce`，取消 transition / animation | 不适用 | VERIFIED（代码规则） |

## 3. 视觉工作条件与证据

- 桌面工作区：使用 `1440 × 1200` 浏览器截图检查主层级；未将截图提交 Git。
- 窄视图：使用 Chrome DevTools Protocol 强制 `375 × 1300` CSS 视口，再读取实际宽度；未将截图提交 Git。
- 输入密度：一项 Topic、两种窗口、三条短原声示例、三类内容叙事、一项 Candidate、一项 Boundary、两种本地意图。
- 已检查规则：DS-001–DS-007、PAT-001–PAT-004、PAGE-TOPIC-001。
- 截图是临时走查生成物，不是长期证据；其展示只证明当次静态页面的渲染，不能证明产品数据、业务状态或部署。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（实现者自验） | PAGE-TOPIC-001 逐区引用 DS/PAT；`verify-topic-intelligence-reference.sh` 通过 | 仍待独立 reviewer 复核当前 PR head |
| 前端/组件实现 | VERIFIED（静态参考实现） | 自包含 HTML 在本地 Chrome 呈现；三种局部互动已实测 | 不是 React/Rust/Web 应用，也没有可复用 CMP 代码 |
| 自动检查 | VERIFIED（实施者自验） | `git diff --check`、`verify-ui-design-handbook.sh`、`verify-topic-intelligence-reference.sh`、`check-project-governance.sh origin/main` 均通过 | 自动检查不评价审美、真实合同或真实业务链 |
| 真实链路/回执 | N/A | 本事项明确禁止真实数据、网络请求、API、持久化与业务动作；静态 HTML 未发现 `fetch(`、`XMLHttpRequest` 或 `WebSocket` | 不得由 N/A 推断真实链已准备好 |
| 部署 | N/A | 未创建运行 Web 产品或部署任务 | 本地 `file://` 参考页不是部署 |
| Mog / 业务验收 | NOT VERIFIED | 尚未由 Mog 在浏览器中走查并确认视觉方向 | 需要 Mog 后续体验确认；其确认也不能取代真实数据/动作合同 |

## 5. 发现与后续

- 已修复的实现问题：首轮窄屏截图暴露等宽标签可能把画面向右撑开；收紧容器、断词与移动宽度后，实际 375px CSS 测量无横向溢出。
- 没有发现需要改写产品、数据、权限或行动合同的冲突。
- 仍需 `DECISION_REQUIRED` 的事项：完整 P0 入口/详情集合、真实脱敏样本、真实 Topic/Source/Observation/Coverage 合同、正式趋势资格、真实动作回执、Web 技术栈和首个用户可见业务 SCOPE。
- 不得因此推断：页面中的样本、候选、选择和视觉质量已经证明真实 Topic Intelligence 产品可用。
