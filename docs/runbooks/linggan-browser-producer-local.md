# Linggan Browser Producer 本地构建与加载核对

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `PLUGIN-MIGRATION-001` 自有 MV3 Browser Producer 的本地构建、release 完整性核对与真实 Canary 前浏览器加载检查
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
cd plugins/linggan-browser-producer
npm run build
npm run check
```

预期：

- `dist/` 是可供浏览器“加载已解压的扩展程序”选择的临时目录，已忽略，不提交；
- `releases/linggan-browser-producer-0.2.0.zip` 是提交的可安装发布包；
- `releases/release-manifest.json` 给出版本、ZIP SHA-256、每个安装文件的 SHA-256 和 Discovery 合同兼容版本；
- `npm run check` 从 clean source 临时重建并对比 release manifest/ZIP，同时检查最小权限和禁止依赖。

本卡检查不打开浏览器、不发送 health 请求，也不访问平台。

## 浏览器加载与连接前检查（真实 Canary 前必须完成）

只有 Issue #37 合并、后续本机 runtime 卡通过、且 Mog 明确开始单次 Canary 后才可以执行：

1. 在 Chrome 扩展管理页选择 `dist/` 作为“加载已解压的扩展程序”；不要加载旧内容工作台插件或其 ZIP。
2. 在扩展详情页核对 manifest name、version 与 `releases/release-manifest.json` 一致。
3. 打开 popup；它只能显示 `localhost:3000`。`LOCAL_INGRESS_READY` 表示本机健康接口同时确认了 Discovery 接纳数据库和固定 ingress 路由；其他状态均不得开始。
4. 打开已经人工选择为 `ADHD` 且可见选中“综合”的小红书搜索结果页，再从 popup 明确点击 Discovery。浏览器只给当前 tab 一次 `activeTab` 页面读取权限；插件不持有 persistent 小红书 host permission。
5. 首次动作只读取当前 card surface 中最多 20 个稳定链接卡片。它不滚动、不打开详情、不读取 Cookie/账号、评论或媒体。少于 20 的实际可见合格卡片可以提交，但会带 `unknown` stopped reason；只有正好 20 张时才是 `quota_reached`。
6. popup 必须显示 `ACCEPTED`、`REPLAY`、`NOT_ACCEPTED` 或 `NOT_READY`。只有 `ACCEPTED`/`REPLAY` 是本机接纳回执；页面显示、HTTP 可达、按钮点击或卡片数量都不能替代该回执。

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
