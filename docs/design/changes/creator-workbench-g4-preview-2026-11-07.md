# CREATOR-WORKBENCH-G4 · 冻结 Demo 原样预览

> 状态: 草案
> 最后核对: 2026-11-08
> 适用范围: 开发态预览地址 `/ui-assets/__preview/creator-workbench`；不代表正式创作页面重构、数据接入或生产发布
> 事实来源: 用户批准的参考页面 `apps/web/design-reference/creator-workbench-goldmaster-2026-11-06-v2.html`（SHA-256 `c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e`）；Intelligence main 基线 `a18a439ee69763e6dcfa8b7b3047aaaa489a61aa`
> 冲突时以谁为准: 用户最新确认、真实代码/测试与冻结文件指纹；文档声明不可替代通过

## 本轮结果

预览地址**原样提供**冻结的 Gold Master Demo，不做任何改写：

- 参考文件位于 `apps/web/design-reference/creator-workbench-goldmaster-2026-11-06-v2.html`，与用户验收的参考稿字节一致；指纹由 `apps/web/test/e2e/creator-workbench-g5.mjs` 双向核对（仓库文件与 HTTP 响应）。
- `apps/web/vite.config.ts` 用 `apply: 'serve'` 的中间件在开发服务器上返回该文件；正式构建不含该文件，也不注册路由。
- 原先用 React 重搭的隔离预览（`CreatorWorkbenchPreviewPage`、fixtures、弹层组件及其测试）已删除，不保留第二套实现。
- 为该预览改动过的共享外壳（`AppShell` / `OpsFrame` / `ContextBar` / `Sidebar` / `shell.css`）已还原为 main 版本；本分支不再改动正式页面代码。

## 边界

- 预览是独立页面：不经 React 应用外壳、不读正式数据、不调用正式 API、不写入业务数据。
- 页面内的示例内容与交互来自参考稿本身，属于设计演示，不构成业务事实。
- 该文件进入公开仓库是用户明确同意的决定。
- 把设计落进正式创作页面（路由、权限、真实数据）尚未开始，需另行确认范围。
