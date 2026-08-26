# ACC-PLUGIN-POPUP-RECOVERY-001 · Popup 启动恢复验收

> 状态: 一次性报告
> 最后核对: 2026-08-26
> 适用范围: Issue #53 / v0.4.1 source、build、release 与静态启动保护
> 事实来源: `PLUGIN-POPUP-RECOVERY-001` UI Change Manifest、实际测试/构建/发布检查
> 冲突时以谁为准: 浏览器实际加载与用户可见结果；本记录不把静态检查说成真实浏览器或真实采集验证

| 场景 | 预期状态含义 | 本次验收 | 未证明边界 |
|---|---|---|---|
| 既有 popup 正常启动 | 运行时提示格式化函数已可解析，原有 `App` 保持入口 | `linggan-popup-startup.test.mjs` 断言导入与首次调用；production build 通过 | 用户 Chrome 点击后的实际画面 |
| `App` 启动异常 | popup 不得空白；本机状态未知，未读取/采集/传输数据 | 静态回归测试断言 Error Boundary、恢复文案与无 action import；构建包包含 popup entry | 真实故障注入下 Chrome 的像素级截图 |
| v0.4.1 安装包 | source、dist、ZIP、release manifest 同版本且 SHA-256 对齐 | `npm run verify` 中的 release/reproducibility 检查 | 用户重新加载该 release 后的版本页 |

本报告不证明小红书、账号、Cookie、真实 Discovery、接纳回执、媒体、OCR/ASR、Evidence Library 数据或研究能力。真实 Canary 仍保持暂停，直至该 release 经独立审查、合并并由用户重新加载验证。
