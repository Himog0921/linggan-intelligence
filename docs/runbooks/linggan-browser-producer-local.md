# Linggan Browser Producer 本地构建与加载核对

> 状态: 权威当前
> 最后核对: 2026-09-04
> 适用范围: `plugins/linggan-intelligence-browser` 当前 MV3 Browser Producer 的本地构建、release 完整性核对与真实链加载检查
> 事实来源: Issue #37 / #128 / #149、PR #132 / #153、插件 source/build/release scripts、工位/运行时/真实 Package 与 Evidence UI 回执
> 冲突时以谁为准: 用户最新确认、实际 manifest/release hash、浏览器实际加载状态和 Linggan local host receipt

## 这份 runbook 解决什么

它让 Linggan 自有插件的四种事实分别可核对：源码版本、可安装 ZIP、浏览器已加载版本、Linggan local host 实际响应。它们不能互相替代。

```text
source build verified
    ≠ release ZIP verified
    ≠ browser loaded/check-in
    ≠ station claimed
    ≠ Package accepted/materialized
    ≠ Evidence UI consumed
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
- 当前可安装发布包只能由 `releases/release-manifest.json` 中与当前 manifest version 对齐的 ZIP 决定；不得把历史 ZIP 名称或 hash 当作当前版本；
- `releases/release-manifest.json` 给出版本、ZIP SHA-256、每个安装文件的 SHA-256 和 Discovery 合同兼容版本；
- `npm run check` 从 clean source 临时重建并对比 release manifest/ZIP，同时检查最小权限和禁止依赖。

上述构建命令本身不打开浏览器、不发送 health 请求，也不访问平台；不得用其结果替代后续加载、工位、接纳和 UI 回执。

## 浏览器加载与连接检查

当次真实操作仍必须有 Mog 明确授权。`0.8.28` 曾完成一次受控标准详情验收，但它是历史证据，不证明后续版本、重装、换工位、新样本或扩大采集；这些情形仍按本节逐层核对：

1. 在 Chrome 扩展管理页选择 `dist/` 作为“加载已解压的扩展程序”；不要加载旧内容工作台插件或其 ZIP。
2. 在扩展详情页核对 manifest name、version 与 `releases/release-manifest.json` 一致。
3. 打开 popup；它只能显示 `localhost:3000`。健康接口必须公布 `LINGGAN_BROWSER_PRODUCER_RUNTIME / PLUGIN_RUNTIME_002_SCHEMA_READY` 及完整 runtime/media routes。
4. 重载或启动插件后，它会自动签到并通过 alarm 领取服务端已批准任务；正常路径不要求点击 Popup 的“领取”。任务会自行打开明确的搜索、博主页或作品详情页。
5. 固定作品深化只能来自服务端冻结的作品集合；一个作品的 detail、media slots、comments、replies 分 lane 提交，任一 lane 失败不能抹掉已接纳 sibling。标准详情评论窗口最多 30；全量深采使用独立任务范围与回执，不与详情窗口混用。媒体字节服务端重试最多 3。
6. 验收分别核对 Task/Attempt/Receipt、媒体 Blob/Materialization、处理 Job/Derivative 和 Evidence Library 页面；浏览器打开页面或出现本地文件都不能单独替代完整链回执。

### `2026-09-01 · 0.8.28` 历史对齐回执

- `main` / `origin/main` / detached runtime 均为 `d7e722018f4f4cfa217c9cf5c0cac6fbcdcaacb3`。
- 工位 `1` active installation 为 `9179e6cf-3316-493a-abee-2e7eb162f824` / `0.8.28`。
- `/health` 为 `LINGGAN_BROWSER_PRODUCER_RUNTIME`、`PLUGIN_RUNTIME_002_SCHEMA_READY / READY`、scheduler running。
- 真实标准详情为详情 1、媒体槽位 6、顶层评论 15、回复 15，全部 Receipt accepted；详情窗口 `30/30`。
- 封面、3 张正文图、Live Photo still/motion 和作者头像共 7 个字节组件均已物化，Evidence UI 已读取本地封面/头像。
- 非空评论图片未在该真实样本观察；音频/ASR 保持真实 `FAILED`。

## 2026-09-04 当前诊断与 `0.8.36` 发布候选

- `origin/main@84fd9498e18164011621ffabc81a2a421a52f7a1` 的本机运行时、Chrome 已加载的 `0.8.34` 和已认领工位都已逐层核对；该安装凭证有效，但运行时仍以 `account_unbound` fail closed。它不是“插件没加载”的同义词，而是尚无可供人工绑定的当前登录账号 Observation。
- 根因在 `0.8.34` 的被动账号探针：博主页的 injected `user` 状态描述的是正在查看的创作者，不能作为执行账号；当前实际已登录页面也没有由该路径形成账号 Observation。`0.8.36` 候选只接受已验证 XHS 全局导航容器内明确标为「我」或「我的」的 profile link；该容器还必须同时含首页、通知和消息路由，并验证同站 HTTPS `/user/profile/<hex-id>` 形状。它对 SPA hydration 最多等待 8 次、每次 500ms；找不到或不合法一律提交 `signal_incomplete`，绝不猜测、回退至博主、读取 Cookie 或持久化原始账号标识。
- 候选 ZIP 为 `releases/linggan-intelligence-browser-v0.8.36.zip`，SHA-256 `1b0d2832b7483d4efb3c7d13f94bef37018fb1455edfac9434575ca89ee3313c`。它仅代表本分支 source/build/release 候选；不代表已经合入 `main`、替换本机 runtime、被 Chrome 加载、成功产生账号 Observation、完成账号绑定或取得任务。
- 合入后仍需一次由 Mog 明确授权的受控运行验证：精确更新 runtime 与 Chrome 已解压扩展，刷新已登录页面以形成资格 Observation，在 Collection Runtime 人工绑定账号，并确认 `account_eligibility` 与 station capacity 后，才可对一项明确授权的手动复采或已配置规则验证 `WorkOrder → Lease → Attempt → Package → Receipt`。这一步不能由构建或 ZIP 校验替代。

## 2026-09-04 · `0.8.37` 认领默认接活与工位名候选

- 候选 ZIP 为 `releases/linggan-intelligence-browser-v0.8.37.zip`，SHA-256 `1492f8d9fd454ca52aa5b7e324f83fb791696897fe36604ff97f1d89f91ad3fd`。它包含 `0035_claimed_station_auto_acceptance` 对应的 Producer 回显协议：claim 或 heartbeat 会返回服务端确认的 `stationDisplayName` 和 `stationAccepting`。
- 正常语义是：成功认领把 station 默认置为自动接活；但仍须通过心跳、凭证、版本、能力、账号、风险、预算、并发和待执行队列的全部 server-side 门禁。Runtime 的人类暂停是持久覆盖，不能由新安装或心跳自动重开。
- 名称在 Collection Runtime 人工维护，新建工位预填“本机 Chrome”；Popup 只镜像服务器名称和自动接活/已暂停/待认领，不把名称作为安装身份或账号绑定。
- 本候选尚不是当前现场状态：未合入 `main`、未切换 runtime/共享迁移、未 reload Chrome，且不授权实际采集。安装/重载后若要让真实工位接活，仍须在 Runtime 看到正确名称和账号资格，并由 Mog 明确决定是否把当前人类暂停改为“恢复自动接活”。

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

构建检查本身不证明插件已加载、工位已认领、Package 已接纳或 Evidence UI 已读取。历史 `0.8.28` 有一笔标准详情真实链，但仍不证明小红书全平台总量/趋势、所有筛选搜索、所有作者页、真实非空评论图片、长期稳定性或未实测媒体/处理组合；也不证明 `0.8.36` 候选已经在真实 Chrome 或平台生效。
