# Linggan Browser Producer 本地构建与加载核对

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `PLUGIN-001` 自有 MV3 Browser Producer 的本地构建、release 完整性核对与未来浏览器加载前检查
> 事实来源: Issue #33、插件 source/build scripts、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1` 与 `PLUGIN-001` 计划
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
- `releases/linggan-browser-producer-0.1.0.zip` 是提交的可安装发布包；
- `releases/release-manifest.json` 给出版本、ZIP SHA-256、每个安装文件的 SHA-256 和 Discovery 合同兼容版本；
- `npm run check` 从 clean source 临时重建并对比 release manifest/ZIP，同时检查最小权限和禁止依赖。

本卡检查不打开浏览器、不发送 health 请求，也不访问平台。

## 未来浏览器加载前检查（本卡不执行）

只有后续明确授权的真实 Canary 卡才可以执行：

1. 在 Chrome 扩展管理页选择 `dist/` 作为“加载已解压的扩展程序”；不要加载旧内容工作台插件或其 ZIP。
2. 在扩展详情页核对 manifest name、version 与 `releases/release-manifest.json` 一致。
3. 打开 popup；先只确认它显示 `localhost:3000`、当前 host state、`NOT AUTHORIZED` 和 disabled Discovery。
4. 本机 `http://localhost:3000/health` 可达只证明 local host；仍必须看到 ingress/authorization 的独立回执，才可能进入下一步。
5. 未取得新的 Work Order、localhost ingress 和明确用户授权前，禁止为测试而访问小红书、登录账号、提取卡片、详情、评论或媒体。

## 错误处理

| 现象 | 影响 | 下一步 |
|---|---|---|
| `npm run check` 失败 | source、ZIP 或 release manifest 不一致 | 停止加载；先重建并审查差异，不手工改 ZIP |
| popup 显示 `LOOPBACK_UNREACHABLE` | 不能证明本地 host 可达；Discovery 仍禁用 | 只检查 Linggan local host 的启动/health，不改插件权限 |
| popup 显示 `LOCAL_HOST_REACHABLE` | 只证明 `/health` 可达 | 仍等待独立 ingress/authorization 卡，不可采集 |
| version 与 release manifest 不同 | 浏览器加载了非当前 package | 停止；删除/重新加载明确的 Linggan `dist/`，不引用旧内容工作台版本 |

## 明确不证明

这份 runbook 和构建检查不证明 plugin 已加载、host ingress、数据库接纳、Evidence Library 读投影、平台 Discovery、封面本地副本、OCR/ASR、部署或业务验收。
