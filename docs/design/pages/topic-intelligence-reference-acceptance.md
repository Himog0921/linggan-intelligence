# ACC-TOPIC-001 · Topic Intelligence Reference Page 验收记录

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: DESIGN-002 L2 静态 Topic Intelligence Reference Page 的 LIDS、浏览器、互动和事实边界验收
> 事实来源: PAGE-TOPIC-001、DS-REF-TOPIC-001、LIDS-SYS-001 / LIDS-PAT-001 / LIDS-AGENT-001、Issue #7、实际脚本输出与本地 Chrome 走查
> 冲突时以谁为准: 真实运行/代码/合同、用户最新确认与当前 SCOPE；截图、演示或本记录不覆盖这些来源

## 1. 验收对象

- Issue / 计划：Issue #7 / DESIGN-002（LIDS v2.0 scope correction 后的最新工作树）。
- 页面：`docs/design/pages/topic-intelligence-reference.html`。
- 关联规格：PAGE-TOPIC-001、DS-REF-TOPIC-001、PAT-001–PAT-004、LIDS-SYS-001、LIDS-TOK-001、LIDS-PRI-001、LIDS-PAT-001、LIDS-AGENT-001。
- 环境：2026-08-21；本机 Google Chrome `151.0.7922.172` headless，`file://` 读取静态源文件；临时截图仅存系统临时目录，未提交 Git。
- 数据/权限前提：全部是 `SYNTHETIC / NOT LIVE`；无真实数据、身份、权限、网络请求、API、持久化或业务动作。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 检查结果 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 桌面初始页 | 看出 Topic、样本模式、限制与本地意图 | `SYNTHETIC / NOT LIVE` + `SOURCE_INCOMPLETE`；L2 参考，不是当前运行态 | 1440×1200 静态渲染可见暖灰画布、煤黑结构、小面积 Signal、L2 三栏信息序；没有 L3 场景/假运行 | N/A；不创建记录 | VERIFIED（实施者自验） |
| 合成原声 | 在点击前判断其是否可当 Evidence | 每条均为 `SYNTHETIC / NOT EVIDENCE` | 三条 quote 边界持续显示；展开后分别补充其不能证明什么 | N/A；不读取来源 | VERIFIED（实施者自验） |
| 切换样本窗口 | 查看另一组合成阅读文案 | 仅本地文本变化，不是查询/趋势刷新 | 点击 7D 后：`07D · SYNTHETIC`、`7D`、`aria-pressed=true` | 无请求、无查询回执 | VERIFIED（实施者自验） |
| 展开限制 | 读取一条样本的限制 | disclosure，不新增 Evidence 资格 | 点击第一条后 `hidden=false`、`aria-expanded=true`；三条边界数为 3 | 无来源读取 | VERIFIED（实施者自验） |
| 本地意图 | 表达继续探索 | `LOCAL_INTENT_ONLY`，没有真实 Decision/Action | 选择后 `aria-pressed=true`，回显“未创建真实记录、研究、决定、行动、通知或待办” | N/A；无写入 | VERIFIED（实施者自验） |
| Reduced Motion | 关闭动效后仍可理解 | 状态/含义不依赖动画 | CDP 模拟 `prefers-reduced-motion: reduce` 后 `.window-button` 的 transition duration 为 `0s` | N/A | VERIFIED（实施者自验） |
| 窄屏阅读 | 连续阅读、使用键盘，而不是移动工作台 | 只承诺参考阅读 | CDP 强制 375px CSS viewport：`innerWidth=375`、`documentWidth=360`、`bodyWidth=360`、`shellWidth=328`，无越界元素 | N/A | VERIFIED（实施者自验） |

## 3. 自动检查

下列命令在 LIDS scope correction 的当前工作树中通过：

```text
git diff --check
./scripts/verify-ui-design-handbook.sh
./scripts/verify-topic-intelligence-reference.sh
./scripts/check-project-governance.sh origin/main
```

Topic 专项检查现额外验证：LIDS 主题/关键 Token/L2 marker、每条原声的持续 `SYNTHETIC / NOT EVIDENCE` 边界、PAGE 中的 LIDS 采用记录、已完成 UI Change Manifest，以及无 `fetch(`/`XMLHttpRequest`。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| LIDS / 设计规格一致 | VERIFIED（实施者自验） | PAGE、LIDS 标准、Agent 合同、模板、迁移记录与专项检查相互链接；旧暗色局部规则已替代 | 独立 reviewer 尚未审查本次 scope correction 的最新 head |
| 静态参考实现 | VERIFIED（实施者自验） | 自包含 HTML 在本机 Chrome 呈现；窗口、三条独立 disclosure、本地意图和 Reduced Motion 已实测 | 不是 React/Rust/Web 应用，也没有正式 CMP 代码 |
| 自动检查 | VERIFIED（实施者自验） | 四项命令均通过 | 自动检查不评价真实合同、完整审美或真实业务链 |
| 真实链路/回执 | N/A | 本事项明确禁止真实数据、网络、API、持久化、权限和业务动作 | 不得由 N/A 推断真实链已经准备好 |
| 部署 | N/A | 未创建运行 Web 产品或部署任务 | 本地 `file://` 参考页不是部署 |
| 独立审查 / 集成 | NOT VERIFIED | 旧 review 已因 scope correction 失效；需要新的 reviewer 和不同 integration owner | draft PR 尚未可合并 |
| Mog / 业务验收 | NOT VERIFIED | Mog 尚未确认更新后的 LIDS 视觉参考 | 即使确认，也不替代真实数据/动作合同 |

## 5. 发现与后续

- 已修正独立预审指出的两项 P1：原声卡在点击前已持续显示合成/非 Evidence 边界；DESIGN-002 active plan 现包含完整 UI Change Manifest。
- 已修正与 LIDS 冲突的首版视觉方向：暗色酸绿、全局粗野主义和“ASCII 可自由扩张”不再保留；当前参考页为 L2 暖灰 Precision Canvas，ASCII 只承担阅读协议。
- 仍需 `DECISION_REQUIRED`：完整 P0 入口/详情集合、真实脱敏材料、真实 Topic/Source/Observation/Coverage/Trend 合同、真实动作回执、运行时前端技术、Token 落库、正式组件、L3 场景资产/性能和部署。
- 不得因此推断：LIDS 已转 Stable、真实 Topic Intelligence 产品可用、样本文案/候选成立，或用户已经执行真实行动。
