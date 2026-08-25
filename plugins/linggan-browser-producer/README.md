# Linggan Browser Producer

> 状态: 受限实现
> 版本: `0.1.0`
> 适用范围: `PLUGIN-001` 的 Linggan 自有 Manifest V3 浏览器 Producer 包
> 事实来源: Issue #33、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1` 与 [`docs/runbooks/linggan-browser-producer-local.md`](../../docs/runbooks/linggan-browser-producer-local.md)
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、版本化 Discovery 合同和真实加载/运行证据

这是 Linggan Intelligence 仓库内唯一获准的未来运行时 Browser Producer。它不依赖、导入、调用或回退到内容工作台的插件、服务、发布包、数据库或队列。

当前版本只提供三件事：

1. 明确显示自身身份、版本和固定 Linggan loopback 目标；
2. 对 `http://localhost:3000/health` 做最小健康探测；
3. 在尚无 Discovery ingress、Work Order 或明确授权时，明确保持 Discovery 操作禁用。

它没有平台 host permission、content script、Cookie/账号访问、详情/评论/媒体/OCR/ASR、实际提交或本地持久 outbox。`DiscoveryPackage` helper 只固定 `xhs.discovery.visible-card.v1` 的 JSON 形状，不能自行执行平台访问或提交。

## Build

```bash
cd plugins/linggan-browser-producer
npm run build
npm run check
```

构建产物：

- 临时、未提交的 `dist/`：浏览器加载目录；
- 已提交的 `releases/linggan-browser-producer-0.1.0.zip`：可安装包；
- 已提交的 `releases/release-manifest.json`：版本、ZIP SHA-256 与文件 SHA-256。

`npm run check` 会从 clean source 重建临时包并比对当前 release manifest。它不加载浏览器、不调用 Linggan host，也不访问任何平台。
