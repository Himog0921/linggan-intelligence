# Creator Workbench G4/G5 — verification ledger

> 状态: 草案
> 最后核对: 2026-11-08
> 适用范围: 创作工作台开发态预览地址及浏览器冒烟，PR #549
> 事实来源: 参考稿 SHA-256 `c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e`；真实仓库代码及 CI
> 冲突时以谁为准: 真实代码、CI 和同条件浏览器证据；文档声明不可替代通过
> Authority: 用户批准的参考稿，SHA-256 `c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e`.
> Branch: `preview/creator-workbench-g4-g5-verify-20261107`; PR #549; production `/ops/creation` untouched.

## G4 closure evidence

- 预览地址返回冻结参考稿本身（`apps/web/design-reference/creator-workbench-goldmaster-2026-11-06-v2.html`），不经过任何改写或重搭。
- 共享外壳改动已全部还原为 main 版本；本 PR 不改动正式页面代码。
- 完整 typecheck / lint / Vitest / build / LIDS 以 CI 结果为准，本文件不作结论。

## G5 browser proof

`node apps/web/test/e2e/creator-workbench-g5.mjs` 由 `Delivery → web` 复用工作流执行。它启动本机 Vite 开发服务器，只允许本机 GET 请求并拒绝 `/api/`，然后记录：

- 预览地址响应体与仓库文件的 SHA-256 是否都等于冻结指纹 `c1e564b3…`；
- 六个视口（1440×1000、1280×720、1024×768、820×900、390×844、375×812）的渲染、页面错误与横向溢出；
- 每个视口的截图、1440 下资产库弹层打开与关闭的截图；
- 页面错误、越界网络请求与失败断言，即使失败也保留证据。

该脚本是冒烟验证，不是设计验收：它证明"给你看的就是那份参考稿"，不证明产品设计已通过。设计签收仍需用户逐屏人工确认。

## Gate decision

G4: 预览地址已原样提供参考稿，以 CI 为准。
G5: 冒烟可自动完成；设计验收待用户人工确认。
G6: 未开始；没有正式 API、合并或部署。
