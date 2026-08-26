# ACC-PLUGIN-POPUP-RECOVERY-001 · Popup 启动恢复验收

> 状态: 一次性报告
> 最后核对: 2026-08-26
> 适用范围: Issue #53 / v0.4.2 source、build、release 与可执行启动保护
> 事实来源: `PLUGIN-POPUP-RECOVERY-001` UI Change Manifest、实际测试/构建/发布检查
> 冲突时以谁为准: 浏览器实际加载与用户可见结果；本记录不把静态检查说成真实浏览器或真实采集验证

| 场景 | 预期状态含义 | 本次验收 | 未证明边界 |
|---|---|---|---|
| 既有 popup 正常启动 | formatter 可解析，原有 `App` 保持入口 | 受控 React harness 删除 v0.4.0 import 后在首次 render 复现 `ReferenceError`；当前 source 完成对应首次 render | 用户 Chrome 点击后的实际画面 |
| `App` 启动异常 | popup 不得空白；本机状态未知；**该提示**没有发起新的采集或传输 | 行为测试覆盖 Error Boundary normal/fallback；fallback 不再绝对断言失败前未发生页面读取 | 真实故障注入下 Chrome 的像素级截图；失败前 App 是否已读取 tab/context/storage |
| v0.4.2 安装包 | source、dist、ZIP、release manifest 同版本且 SHA-256 对齐；release 内含 copied LIDS token CSS | `npm run verify` 中 build、release/reproducibility 与 `themes/lids-tokens.css` 包验证 | 用户重新加载该 release 后的版本页 |

本报告不证明小红书、账号、Cookie、真实 Discovery、接纳回执、媒体、OCR/ASR、Evidence Library 数据或研究能力。真实 Canary 仍保持暂停，直至该 release 经独立审查、合并并由用户重新加载验证。
