# PLUGIN-POPUP-RECOVERY-001 UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: Issue #53；Linggan Intelligence Browser v0.4.1 工具栏 popup 启动恢复
> 事实来源: Issue #53、`PAGE-PLUGIN-001`、实际 MV3 source、`PLUGIN-MIGRATION-001` 历史边界与 LIDS
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前插件合同与实际运行；本清单不扩大采集、权限或平台访问

## 目标与非目标

- 目标：工具栏点击时，完整 Linggan Browser popup 能启动既有交互；若 React 启动期再次失败，显示一个诚实、可恢复的错误界面，而不是空白窗口。
- 根因：v0.4.0 popup 在初始渲染中调用运行时提示格式化函数，但缺少对应导入；打包不会静态拒绝这个未绑定名称，因此浏览器实际点击时出现空白。
- 非目标：不重做 popup 外观、不替换既有页面浮条、不修改采集器、后台、任务/回传合同、权限、Cookie、账号、媒体、OCR/ASR 或任何真实平台行为。

## 用户可见状态合同

| 状态 | 允许表达 | 禁止表达 | 用户下一步 |
|---|---|---|---|
| 正常启动 | 原有 popup 的现有状态与动作 | 真实平台采集已经成功，除非另有接纳回执 | 按已有页面与回执规则操作 |
| 启动失败 fallback | “插件界面未能启动”“本机状态目前未知”“本次没有读取、采集或传输任何数据” | 本机服务正常、任务成功、任何内容已读取/提交 | 在 Chrome 扩展程序页面重新加载明确版本后重试 |

fallback 不包含采集按钮，也不触发任何平台、页面、后台或本机 API 调用。版本仅用于帮助用户确认 reload 对象，不取代浏览器实际加载验证。

## 实施边界

- 必须：补齐 popup 的运行时格式化函数导入；以 React Error Boundary 包裹既有 `App`；新增回归测试；发布版本 `0.4.1` 并令 source、build、ZIP 与 release manifest 对齐。
- 必须保留：原有 Popup、Dashboard、内容页浮条与所有既有能力。
- 停止条件：若需要增加 host permission、读取浏览器账号/Cookie、变更采集动作、调用小红书或修改 Linggan API/数据库，停止并另行立项。

## 验收与未证明边界

| 层级 | 本卡需要证明 | 本卡明确不证明 |
|---|---|---|
| 代码/状态 | 缺失导入有测试保护；`App` 渲染错误会落入无副作用 fallback | Chrome 已加载新版本或真实 popup 截图 |
| 发布 | v0.4.1 source、dist、ZIP、manifest 与 SHA-256 一致且可复现 | 用户已重新加载 v0.4.1 |
| 真实后果 | fallback 明确不执行任何采集/交付动作 | XHS、账号、Cookie、真实 Discovery、Evidence Library 接纳、媒体或研究结果 |
