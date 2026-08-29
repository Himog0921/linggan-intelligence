# Linggan Browser Producer 本地构建与加载核对

> 状态: 权威当前
> 最后核对: 2026-08-30
> 适用范围: `plugins/linggan-intelligence-browser` 当前 MV3 Browser Producer 的本地构建、release 完整性核对与真实链加载检查
> 事实来源: Issue #37、插件 source/build scripts、`PLUGIN-MIGRATION-001` 与 `LOCAL-001C0-DISCOVERY-BOUNDARY-V1`
> 冲突时以谁为准: 用户最新确认、实际 manifest/release hash、浏览器实际加载状态和 Linggan local host receipt

## 这份 runbook 解决什么

它让 Linggan 自有插件的四种事实分别可核对：源码版本、可安装 ZIP、浏览器已加载版本、Linggan local host 实际响应。它们不能互相替代。

```text
source build verified
    ≠ browser loaded
    ≠ localhost ingress connected
    ≠ real Discovery accepted
```

## 本卡允许的本地构建与静态核对

在 Linggan 仓库根目录执行：

```bash
cd plugins/linggan-intelligence-browser
npm ci --ignore-scripts
npm run build
npm run package:release
npm run release:manifest
npm run release:verify
npm run release:reproducibility
npm run verify:linggan-isolation
```

预期：

- `dist/` 是可供浏览器“加载已解压的扩展程序”选择的临时目录，已忽略，不提交；
- `releases/linggan-intelligence-browser-v0.8.0.zip` 是当前可安装发布包；
- `releases/release-manifest.json` 给出版本、ZIP SHA-256、每个安装文件的 SHA-256 和 Discovery 合同兼容版本；
- `npm run check` 从 clean source 临时重建并对比 release manifest/ZIP，同时检查最小权限和禁止依赖。

本卡检查不打开浏览器、不发送 health 请求，也不访问平台。

## 浏览器加载与连接前检查（真实 Canary 前必须完成）

只有 Issue #37 合并、后续本机 runtime 卡通过、且 Mog 明确开始单次 Canary 后才可以执行：

1. 在 Chrome 扩展管理页选择 `dist/` 作为“加载已解压的扩展程序”；不要加载旧内容工作台插件或其 ZIP。
2. 在扩展详情页核对 manifest name、version 与 `releases/release-manifest.json` 一致。
3. 打开 popup；它只能显示 `localhost:3000`。健康接口必须公布 `LINGGAN_BROWSER_PRODUCER_RUNTIME / PLUGIN_RUNTIME_002_SCHEMA_READY` 及完整 runtime/media routes。
4. 重载或启动插件后，它会自动签到并通过 alarm 领取服务端已批准任务；正常路径不要求点击 Popup 的“领取”。任务会自行打开明确的搜索、博主页或作品详情页。
5. 固定作品深化只能来自服务端冻结的作品集合；一个作品依次提交 detail、media slots、comments、replies，任一 lane 失败不能抹掉已接纳 sibling。评论最多 30、回复展开最多 2，媒体/处理重试最多 3。
6. 验收分别核对 Task/Attempt/Receipt、媒体 Blob/Materialization、处理 Job/Derivative 和 Evidence Library 页面；浏览器打开页面或出现本地文件都不能单独替代完整链回执。

## 错误处理

| 现象 | 影响 | 下一步 |
|---|---|---|
| `npm run check` 失败 | source、ZIP 或 release manifest 不一致 | 停止加载；先重建并审查差异，不手工改 ZIP |
| popup 显示 `LOOPBACK_UNREACHABLE` | 不能证明本地 host 可达；Discovery 仍禁用 | 只检查 Linggan local host 的启动/health，不改插件权限 |
| popup 显示 `LOCAL_HOST_REACHABLE` | 只证明 `/health` 可达，但本机数据库接纳未就绪 | 不可采集；完成独立 runtime/database 卡，不改插件或权限 |
| popup 显示 `LOCAL_INGRESS_READY` | 本机 entrypoint 可尝试接纳 Discovery | 仍需在正确的 XHS `ADHD / 综合` 搜索页，由用户明确点击 |
| popup 显示 `NOT_READY` | 当前 tab、关键词、综合排序或可审计卡片面不符合合同，且没有提交 | 修正当前页面后再试；不扩大 host permission 或改写字段 |
| popup 显示 `NOT_ACCEPTED` | 结果已送到本机，但本机拒绝或未能确认接纳 | 不把卡片当作已入库；检查本机 receipt code，不回退旧工作台 |
| version 与 release manifest 不同 | 浏览器加载了非当前 package | 停止；删除/重新加载明确的 Linggan `dist/`，不引用旧内容工作台版本 |

## 明确不证明

这份 runbook 和构建检查不证明 plugin 已加载、数据库接纳、真实平台 Discovery、Evidence Library 读投影、封面本地副本、OCR/ASR、部署或业务验收。真实浏览器步骤即使发生，也只证明一笔有 Coverage 边界的搜索页发现，不证明小红书总量、趋势、详情、评论、媒体或用户需求。
