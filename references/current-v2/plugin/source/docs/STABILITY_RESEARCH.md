# 平台数据采集稳定性调研

> **版本**: v1.1 | **日期**: 2026-04-20 | **状态**: 调研基线 + 当前代码对照版

本文档是 linggan-boom 插件的核心运维参考，系统性记录两个平台（小红书、抖音）的数据采集稳定性现状、风险评估、技术对标和改进路线。

---

## 目录

0. [2026-04-20 当前代码对照](#0-2026-04-20-当前代码对照)
1. [数据源可靠性分级](#1-数据源可靠性分级)
2. [风险热力图](#2-风险热力图)
3. [MediaCrawler 技术对标](#3-mediacrawler-技术对标)
4. [反检测能力评估](#4-反检测能力评估)
5. [选择器过期监控](#5-选择器过期监控)
6. [已知问题与根因](#6-已知问题与根因)
7. [改进路线图](#7-改进路线图)
8. [探查验证记录](#8-探查验证记录)

---

## 0. 2026-04-20 当前代码对照

> 本节用于把 2026-04-18 的调研结论和 2026-04-20 的实际代码状态对齐。  
> 下文原有章节仍保留“调研基线”价值，但是否已经落地，以本节为准。

### 已落地

| 项目 | 当前代码状态 |
|------|-------------|
| XHS 博主页 Vue ref 拆包 | 已落地：`src/injected/user.js` 对 `userPageData / userInfo` 做 `._rawValue` 拆包 |
| XHS `noteDetailMap` 幽灵 key 基础防护 | 已完成基础回写：`src/platforms/xhs/noteCollector.js` 已过滤 `undefined / 空 key` 并优先按 URL / expectedId 精确匹配，原“完全无防护”判断已不再成立 |
| 抖音综合搜索页 API 发现骨架 | 已落地：`src/platforms/douyin/batchDiscovery.js` 已按 tab 选择 `aweme_video / aweme_general` |
| 抖音评论 API 主链路 | 已落地：`src/platforms/douyin/commentApi.js` 已直接走评论 / 回复 API |
| XHS 评论 API 主链路 + 主动分页 | 已落地：`src/injected/xhsApiCapture.js` + `src/platforms/xhs/commentApi.js` 已接入页面评论 API、主评论/子回复主动分页，且 2026-04-20 已完成真实浏览器验收 |
| 抖音评论跳失第一轮修复 | 已落地：三级 reply 递归、空 ID fallback、连续空页容忍、hot/latest 双轮补漏均已进入 `src/platforms/douyin/commentCollector.js` |
| 抖音评论图片区高清候选 | 已落地：`src/platforms/douyin/commentMedia.js` 优先吃 `origin_url`，并保留多级 URL 候选 |
| 抖音评论图片区 MIME 校验 | 已落地：`fetchImageBlob()` 已跳过非 `image/*` blob |
| 抖音视频上下文竞态补救 | 已完成基础回写：`src/platforms/douyin/videoContext.js` 在缓存不足时主动补调 `fetchDetailApiData()`，原“完全无兜底”判断已不再成立 |
| 抖音博主页串号防护 | 已落地：`src/platforms/douyin/authorCollector.js` 会比对 `renderSecUid` 与 URL `userId`，不匹配就丢弃 render 用户数据 |
| Dashboard 评论分页 | 已落地：当前 Dashboard 已有分页，不再是一口气渲染全部评论 |
| Dashboard 同步反馈 | 已落地：同步到工作台已能展示导入数 / 跳过数 / 失败原因 |
| Workbench 运行时心跳与结果包 | 已落地：`collectionRun` 心跳、`pending_upload -> packaged`、`taskPoller + lease + alarms` 都已在代码中存在 |

### 已部分落地，但仍未收口

| 项目 | 当前状态 |
|------|---------|
| 抖音综合搜索页批量发现 | 已从 DOM 策略切到 API 发现，并新增页面 `search stream` 捕获与优先消费；当前仍有“页面顺位未完全对齐”问题，但已按用户判断降级为下一阶段增强项 |
| XHS `noteDetailMap` 非法 key 过滤 | 当前基础防护已足以否定“完全无防护”这个旧结论，但还没升级成 24 位 noteId 正则；剩余应视为稳定性硬化，而不是活跃功能 bug |
| 抖音 `status_code != 0` 错误处理 | 第三阶段已落地：单条评论、评论图片区、批量评论、搜索批量与批量视频的 discovery / detail 补数链路都会把 `status_code + DOM` 信号归因为安全验证并自动暂停；剩余缺口主要是更完整的验证码选择器与页面信号覆盖 |
| Dashboard 评论大数据可用性 | 当前已有分页与列宽整理；2026-04-20 用户实机反馈确认评论页优化成功，剩余无虚拟滚动仅保留为后续体验债 |

### 仍未完全收口

| 项目 | 当前状态 |
|------|---------|
| 降级质量标记体系 | 第二阶段已落地：Dashboard 手动同步到 Workbench、Popup/Flywheel 同步与 `syncToFlywheel()` 批量请求体现在都会透传 `dataQuality / qualityReason / sourceTier / collectionRunId`，monitor surface seed 路径也已统一补 `seed` 质量语义；剩余主要是服务端消费侧联调观察 |
| 自适应降速 | 已部分落地：抖音批量 discovery 与逐条采集链路已有错误率滑窗退避；评论详情翻页等子链路与 XHS 侧仍未接入 |
| 选择器健康监控 | 第三阶段已落地：XHS / Douyin 已在启动前 preflight 之外补齐初始化轻探针、SPA 路由复检与 selector 缺失 / `staleChecks` 的可见告警，且告警会去重写入同一份 `window.__lgboomSelectorHealth` 快照；剩余主要是更长期的定时巡检与聚合观测 |

### 当前最该盯住的 5 项

1. 抖音验证码选择器与页面信号覆盖仍未收口
2. 固定节流和自适应降速仍未全量收口
3. 降级质量标记体系的插件侧已收口，剩余服务端消费口径仍需联调观察
4. 关键选择器已具备启动前预检、初始化轻探针、SPA 复检与可见告警，剩余只差更长期的定时巡检与聚合观测
5. 抖音综合搜索页顺位一致性仍待后续复验

---

## 1. 数据源可靠性分级

每个数据采集点按数据来源分为三个可靠性等级：

| 等级 | 含义 | 说明 |
|------|------|------|
| **Tier 1** | 稳定 API | 直接调用平台版本化 API，字段有一定稳定性保证 |
| **Tier 1.5** | API 拦截 | 监听页面自身的 API 请求，签名由平台 JS 维护，但存在时序风险 |
| **Tier 2** | 页面状态对象 | 读取 `__INITIAL_STATE__` / `RENDER_DATA` 等 SSR 注入数据 |
| **Tier 3** | DOM 选择器 | 依赖 CSS class / 标签结构，最脆弱 |

### 小红书

| 功能 | 主数据源 | 等级 | 备用链路 | 风险 |
|------|---------|------|---------|------|
| 笔记内容/统计 | `__INITIAL_STATE__.note.noteDetailMap` | **T2** | 无 | State 路径变更 |
| 笔记发现（列表） | DOM: `section a.cover`, `.footer span` | **T3** | 无 | 选择器失效 |
| 评论全字段 | DOM: `.parent-comment`, `span:not([class])` 等 | **T3** | 无 | **最高风险** |
| 博主统计 | `__INITIAL_STATE__.user.userPageData.interactions` | **T2** | DOM 兜底（仅 warn） | Vue ref 拆包变化 |
| 博主基本信息 | DOM: `.user-name`, `.user-redId` 等 | **T3** | `basicInfo` 字段 | 选择器失效 |

### 抖音

| 功能 | 主数据源 | 等级 | 备用链路 | 风险 |
|------|---------|------|---------|------|
| 视频内容/统计 | API 拦截缓存 `__lgboom_dy_video_data` | **T1.5** | `RENDER_DATA` → DOM | 注入时序竞态 |
| 视频下载地址 | API 拦截: `download_addr.url_list` | **T1.5** | DOM `<video>.currentSrc`（可能带水印） | 字段路径变更 |
| 评论全字段 | `/aweme/v1/web/comment/list/` API | **T1** | 无 | API 签名参数硬编码 |
| 博主详情 | `RENDER_DATA.app.user.info` | **T2** | Profile API → DOM | 字段名变更 |
| 博主抖音号/IP | `/aweme/v1/web/user/profile/other/` API | **T1** | DOM 文本 → RENDER_DATA | API 签名参数 |
| 批量发现（博主页） | API 拦截 aweme/post 列表 | **T1.5** | DOM 扫描 | 较稳定 |
| 批量发现（搜索页） | DOM 文字解析 | **T3** | 无 | **高风险** |

---

## 2. 风险热力图

按失效概率和影响范围排序：

### 极高风险（红色）

**1. 小红书评论采集——全链路纯 DOM，无 API 兜底**

这是整个插件最脆弱的功能模块。所有评论字段（ID、作者、正文、时间、点赞、IP、头像）全部依赖 CSS class 选择器。

特别危险的点：
- **评论正文**使用 `span:not([class])` 启发式提取——小红书给任何 span 加上 class 就会静默失效
- **commentId** 在缺少 `data-id` 时退化为 `${author}_${text}` 合成键，可能导致重复数据
- **无任何 API 降级路径**——小红书评论 API 需要签名，而插件架构（浏览器扩展）的优势恰好可以利用页面上下文请求来解决

**2. 小红书笔记列表发现——选择器组合无冗余**

`section a.cover` + `.footer span` + `.like-wrapper .count` 都是语义 class，一旦小红书前端重构，整个批量发现链路断裂。

### 高风险（橙色）

**3. 抖音搜索页批量发现——已转向 API + 页面搜索流优先，但仍属高风险**

综合搜索页（无 `type=video`）的结果不使用 `<a href="/video/...">` 结构，原始 DOM 发现策略完全不可用。当前代码已经转向 API 发现，并新增“页面 `general/search/stream` 捕获 + 优先消费”的补救路径，用来减少手搓请求与页面真实结果脱节的问题；但这条链路还缺综合搜索页实机复验，因此仍属于高风险。

**4. 小红书博主 DOM 选择器——无持续监控**

`.user-name`、`.user-redId` 等选择器最后验证于 2026-04-18，但无自动化监控。一旦改版，静默返回空字符串而不抛异常。

### 中风险（黄色）

**5. 抖音 API 拦截时序竞态**

如果用户在插件注入前页面 API 已完成，缓存为空。当前代码已经在关键下载/上下文路径补充 `fetchDetailApiData` 调用，原“完全无兜底”判断已不再成立；剩余风险主要是补调覆盖面仍未完全统一。

**6. 抖音评论 API 签名参数硬编码**

`device_platform=webapp&aid=6383&channel=channel_pc_web` 硬编码在请求中。长期稳定但无版本保证。

### 低风险（绿色）

**7. `__INITIAL_STATE__` / `RENDER_DATA` 路径**

SSR 注入数据是两个平台目前最稳定的数据源。MediaCrawler 也使用相同路径作为降级策略。

---

## 3. MediaCrawler 技术对标

> 参考项目：[NanmiCoder/MediaCrawler](https://github.com/NanmiCoder/MediaCrawler)

### 架构差异

| 维度 | MediaCrawler | linggan-boom |
|------|-------------|-------------|
| 运行环境 | 外部 Python 进程 + Playwright | 浏览器扩展（真实浏览器内） |
| Cookie 获取 | QR 扫码/手机号/字符串注入 | **天然拥有**（用户已登录） |
| 请求签名 | 必须逆向 a_bogus/x-s/x-t | **无需签名**（页面上下文 fetch） |
| 反检测 | stealth.min.js + CDP 真实浏览器 | **天然隐匿**（就是真实用户） |
| 规模化 | 代理 IP 池 + 多账号 | 单浏览器单账号 |
| 数据覆盖 | 可爬任意公开内容 | 仅限用户实际访问的页面 |

### 核心发现

**linggan-boom 的架构优势远大于劣势。** MediaCrawler 最新版已转向 CDP 模式（连接真实 Chrome），本质上在向浏览器扩展的天然优势靠近。

**值得借鉴的点：**

1. **XHS 评论 API 路径**：MediaCrawler 直接调用 `/api/sns/web/v2/comment/page`，linggan-boom 可以在页面上下文中同样调用这个 API（自动带签名），摆脱 DOM 依赖
2. **XHS 笔记 Feed API**：`/api/sns/web/v1/feed`，可作为 `__INITIAL_STATE__` 的补充数据源
3. **降级设计**：MediaCrawler 的 XHS 部分在 API 被频控时降级到读 HTML 的 `__INITIAL_STATE__`——与 linggan-boom 的主路径一致，说明这条路径足够可靠
4. **字段提取器模式**：MediaCrawler 有独立的 `extractor.py` 做字段映射和清洗，可借鉴做字段规范化层

**不适用的点：**

- a_bogus / x-s 签名算法：linggan-boom 不需要，浏览器上下文自动处理
- 代理 IP 池：单用户场景不适用
- Playwright 自动化：扩展架构天然解决了这个问题

### MediaCrawler 使用的关键 API 端点（可在页面上下文复用）

| 平台 | 用途 | 端点 |
|------|------|------|
| 抖音 | 视频详情 | `GET /aweme/v1/web/aweme/detail/?aweme_id=xxx` |
| 抖音 | 评论列表 | `GET /aweme/v1/web/comment/list/?aweme_id=xxx&cursor=N` |
| 抖音 | 评论回复 | `GET /aweme/v1/web/comment/list/reply/?comment_id=xxx` |
| 抖音 | 用户信息 | `GET /aweme/v1/web/user/profile/other/?sec_user_id=xxx` |
| 抖音 | 用户作品 | `GET /aweme/v1/web/aweme/post/?sec_user_id=xxx&max_cursor=N` |
| 小红书 | 笔记详情 | `POST /api/sns/web/v1/feed` body: `{source_note_id, xsec_token}` |
| 小红书 | 评论列表 | `GET /api/sns/web/v2/comment/page?note_id=xxx&cursor=N` |
| 小红书 | 评论子回复 | `GET /api/sns/web/v2/comment/sub/page?note_id=xxx&root_comment_id=xxx` |
| 小红书 | 用户信息 | `GET /api/sns/web/v1/user/otherinfo?target_user_id=xxx` |
| 小红书 | 用户笔记 | `GET /api/sns/web/v1/user_posted?user_id=xxx&cursor=N` |

> **重点关注**：小红书评论 API `/api/sns/web/v2/comment/page` 是解决当前 P0 风险（评论全链路 DOM 依赖）的关键。linggan-boom 可以在页面 MAIN world 中直接 fetch 这个 API，浏览器会自动附加 x-s/x-t 签名。

---

## 4. 反检测能力评估

### 小红书

| 能力 | 状态 | 评估 |
|------|------|------|
| 随机延迟 | ✅ 已实现 | `randomDelay(min, max)`，分级节流 |
| 拟人滚动 | ✅ 已实现 | 100-300px 步长，200-500ms 间隔 |
| 分级限速 | ✅ 已实现 | 按采集量三档递增：1.2-2.8s / 1.8-4.2s / 2.4-5.6s |
| 验证码检测 | ✅ 已实现 | 6 个选择器 + 轮询，暂停 + 人工介入 UI |
| 风控页检测 | ✅ 已实现 | 300017 错误码识别 |
| **指纹规避** | ❌ 未实现 | 不修改 webdriver 等自动化标记（扩展架构天然无此问题） |
| **鼠标轨迹** | ❌ 未实现 | 无随机化模拟（滚动和点击是脚本直接触发） |

### 抖音

| 能力 | 状态 | 评估 |
|------|------|------|
| 请求伪装 | ✅ 天然 | MAIN world fetch 带完整 Cookie/签名 |
| 批量间隔 | ⚠️ 部分实现 | 抖音批量 discovery 与逐条采集已改为随机区间，并会随近期失败率抬高等待区间；部分子链路仍保留固定延迟 |
| 评论翻页延迟 | ✅ 已实现 | `randomDelay(220, 360)` |
| **验证码检测** | ⚠️ 部分实现 | 单条评论 / 评论图片区 / 批量评论 / 搜索批量，以及批量视频的 discovery / detail 补数链路已支持 `status_code + DOM` 安全验证识别与自动暂停；更完整的滑块 / 旋转验证码选择器仍待补齐 |
| **频率监控** | ❌ 未实现 | 无 API 返回 `status_code` 异常的自动降速 |

### 关键缺口

1. **抖音验证码识别仍未完全收口**：当前已覆盖单条评论 / 评论图片区 / 批量评论 / 搜索批量，以及批量视频的 discovery / detail 补数链路，但滑块 / 旋转等更多验证页形态仍未系统覆盖
2. **抖音批量节流仍未全量随机化**：discovery 与逐条采集已脱离固定常量，但评论详情翻页等子链路仍有固定延迟
3. **自适应降速仍未扩到双平台全量**：当前只有抖音批量 discovery / 逐条采集具备错误率驱动的退避

---

## 5. 选择器过期监控

> 规则：验证日期超过 30 天标记 ⚠️，需重新验证。

### 当前过期/待验证选择器（截至 2026-04-18）

| 平台 | 选择器 | 用途 | 上次验证 | 状态 |
|------|--------|------|---------|------|
| XHS | `.footer span` | 卡片标题 | 2026-04-18 | ⚠️ 可用但过于宽泛（140 匹配），需限定作用域 |
| XHS | `.comment-item img` | 评论区图片 | 2026-04-18 | ⚠️ 22 个匹配全是头像，未见评论附图样本 |
| XHS | SPA 路由跳转到 `/explore/{noteId}` | 批量打开笔记 | 未验证 | ❓ |
| XHS | `history.back()` 返回列表 | 批量返回 | 未验证 | ❓ |
| XHS | `.close-circle, .close-btn` | 关闭弹窗 | 未验证 | ❓ |

### 本次验证已确认正常的选择器

> 2026-04-18 全部通过验证，下次过期日期：2026-05-18

- XHS 评论区：`.comments-container`, `.parent-comment`, `.comment-item`, `a.name`, `.like-wrapper .count`, `.date .location`, `.avatar img.avatar-item`, `.avatar a[data-user-id]`, `div.show-more`, `span:not([class])` 正文提取
- XHS 博主页：`div.user-name`, `span.user-redId`, `span.user-IP`, `div.user-desc`, `img.user-image`, `__INITIAL_STATE__` Vue ref 拆包
- XHS 卡片：`section`, `a.cover`, `.like-wrapper .count`, `.play-icon`
- XHS 数据：`__INITIAL_STATE__.note.noteDetailMap` 路径

---

## 6. 已知问题与根因

### P0 - 影响核心功能

| # | 问题 | 根因 | 修复方向 |
|---|------|------|---------|
| 1 | XHS 评论正文提取脆弱 | `span:not([class])` 启发式，无稳定标识 | 迁移到评论 API，或优先使用 `span.note-text` 定位 |
| 2 | XHS 评论 ID 依赖 `el.id` | 实测 17/17 走 `el.id`，目前可用但无 `data-id` | 迁移到评论 API 获取天然唯一 `id` |
| 3 | 抖音综合搜索页批量采集结果与页面不符 | 原始 DOM 发现已失效；当前已新增页面 `search stream` 捕获与优先消费，减少 `aweme_general` 手搓请求与页面脱节的问题，但仍缺实机复验 | 在综合搜索页验证首屏、翻页、Top N 三类路径，并继续观察是否仍需细调搜索参数 |
| 4 | `noteDetailMap` 含幽灵 key `"undefined"` 和 `""` | SPA 路由切换时写入的残留（Vue 状态管理） | 当前代码已做基础过滤，后续升级为 `/^[a-f0-9]{24}$/` 正则更稳 |

### P1 - 影响数据质量

| # | 问题 | 根因 | 修复方向 |
|---|------|------|---------|
| 4 | 降级质量标记需继续联调观察 | 采集器、归一化层、Dashboard、Workbench 手动同步、Popup/Flywheel 同步与 monitor surface seed 路径现已统一质量字段；剩余主要是服务端消费侧观察 | 保持字段枚举稳定，并在服务端侧确认消费口径 |
| 5 | 抖音注入时序竞态 | 页面 API 先于插件注入完成 | 当前已在部分关键链路主动补调 detail API，后续继续扩大覆盖范围 |
| 6 | 抖音批量节流仍未全量随机化 | discovery 与逐条采集已改为随机区间，但评论详情翻页等局部链路仍保留固定延迟 | 将随机区间 pacing 扩到剩余高频子链路 |

### P2 - 影响稳定性

| # | 问题 | 根因 | 修复方向 |
|---|------|------|---------|
| 7 | 抖音验证码识别覆盖仍不完整 | 当前已覆盖单条评论 / 评论图片区 / 批量评论 / 搜索批量，以及批量视频的 discovery / detail 补数链路，但更完整的验证页选择器与页面信号仍缺系统整理 | 补更多滑块 / 旋转 / 文本验证选择器，并统一页面信号识别 |
| 8 | 自适应降速未全量落地 | 当前只覆盖抖音批量 discovery / 逐条采集，XHS 与更多子链路仍未接入 | 扩展滑动窗口错误率监控，并统一失败分类后接入更多链路 |
| 9 | 选择器健康监控仍缺长期巡检 | 当前已具备任务启动前 preflight、初始化轻探针、SPA 复检与可见告警；剩余缺口主要是定时巡检、历史聚合和更长期观测 | 在现有 selector health 快照基础上补轻量定时巡检与历史聚合 |

---

## 7. 改进路线图

### 短期（1-2周）—— 消除 P0 风险

| 优先级 | 任务 | 预期效果 |
|--------|------|---------|
| **P0-1** | XHS 评论迁移到 API | 评论采集从 T3 提升到 T1.5，消除最大风险 |
| **P0-2** | 抖音搜索 API 拦截 | 综合搜索页批量采集恢复 |
| **P0-3** | 全量选择器重新验证 | 确认当前所有选择器仍然有效 |

### 中期（2-4周）—— 提升稳定性

| 优先级 | 任务 | 预期效果 |
|--------|------|---------|
| **P1-1** | 数据质量标记体系 | 降级采集可追溯，下游同步能识别 |
| **P1-2** | 抖音验证码识别规则增强 | 补齐更多验证页形态，让自动暂停从“主链路可用”提升到“页形态更稳” |
| **P1-3** | 自适应限速机制 | 根据错误率自动调整采集速度 |

### 长期（1-2月）—— 架构优化

| 优先级 | 任务 | 预期效果 |
|--------|------|---------|
| **P2-1** | 选择器健康定期巡检 | 在现有 preflight + 初始化轻探针 + SPA 复检 + 可见告警基础上补轻量定时巡检与历史聚合 |
| **P2-2** | API 拦截覆盖扩展 | XHS 评论/笔记双路径（State + API） |
| **P2-3** | 字段版本化追踪 | 记录每次平台字段变更，便于快速适配 |

---

## 8. 探查验证记录

> 本节记录每次探查脚本执行的结果。运行脚本后将 JSON 输出粘贴到此处，并更新上方的选择器状态。

#### 2026-04-18 — 抖音视频页探查

**执行页面**: `douyin.com/video/7628996252075592970`（影视飓风视频）

**数据源状态**:
- ✅ `RENDER_DATA` 存在（`hasRenderData: true`）
- ❌ `__INITIAL_STATE__` 不存在（抖音视频页不使用）
- ⚠️ `app.videoDetail` 路径为空（`renderKeyScan: []`）——RENDER_DATA 实际结构路径与探查脚本假设不符，但**不影响插件**（插件走 API 拦截缓存，不依赖此路径）

**DOM 选择器**:
- ✅ `[data-e2e="detail-video-info"]` 正常，含完整视频描述、标签、统计数据
- ✅ `[data-e2e="user-info"]` 正常，含博主名称、粉丝数、认证信息
- ❌ `[data-e2e="video-info"]` 不存在（老版选择器，已废弃）
- ❌ `[data-e2e="feed-active-video"]` 不存在（视频详情页不使用此属性）

**视频 ID 解析**:
- ✅ URL 路径解析正常：`pathVideoId = "7628996252075592970"`
- ✅ `resolvedVideoId` 一致性通过

**下载地址**:
- ⚠️ 探查脚本只发现 blob URL（无 http(s) 候选）——这是**预期行为**，探查脚本无插件注入，无法访问 API 拦截缓存 `__lgboom_dy_video_data`；插件实际通过 API 拦截获取真实下载地址，不受影响

**结论**：视频页 DOM 选择器正常，插件采集链路（API 拦截 → 缓存 → 采集）工作正常。`app.videoDetail` RENDER_DATA 路径已废弃，可从探查脚本中移除。

---

#### 2026-04-18 — 抖音博主页探查

**执行页面**: `douyin.com/user/MS4wLjABAAAAaCcBHb3Rhc4zxF8YkBOfHfLh6k-IWEK2l3Ne9xOXPnQ`（影视飓风）

**串号风险验证**:
- ⚠️ `RENDER_DATA app.user.info` 返回的是**登录账号**（nickname: "Hi Mog"，secUid 不同），不是当前主页博主
- ✅ 插件代码已有保护：`authorCollector.js` 中 `isRenderMatchedUser` 检查 secUid 是否与 URL 一致，不一致时 `safeRenderUser = null`，不会串号
- ❌ `__INITIAL_STATE__` 不存在（抖音博主页不使用）

**DOM 选择器**:
- ✅ `[data-e2e="user-detail"]` 正常，含完整博主信息（名称、粉丝数、抖音号、IP属地、简介）
- ✅ `[data-e2e="badge-role-name"]` 正常（"优质科技创作者"）
- ✅ `[data-e2e="user-info-follow"]` / `user-info-fans` / `user-info-like` / `user-tab-count` 均正常
- ❌ `[data-e2e="user-signature"]` 不存在（简介在 user-detail 内）
- ❌ `[data-e2e="user-avatar"]` 不存在（img 无此属性）
- ❌ `[data-e2e="user-ip"]` 不存在（IP 属地在 user-detail 文本中，需文本解析）

**IP 属地**:
- DOM 文本中存在 "IP属地：浙江"，但无专用选择器
- 需用文本正则 `/IP属地[：:]\s*(.+)/` 从 `user-detail` 文本中提取

**结论**：串号风险已被代码正确处理。IP 属地无专用选择器，依赖文本解析，存在轻微脆弱性。

---

### 探查脚本清单

| 脚本 | 执行页面 | 用途 |
|------|---------|------|
| `scripts/probe-xhs-note-full.js` | XHS 笔记详情页 | 验证 `__INITIAL_STATE__` + 评论 DOM + 笔记字段 |
| `scripts/probe-xhs-profile-full.js` | XHS 博主主页 | 验证用户数据 + Vue ref + 批量发现选择器 |
| `scripts/probe-douyin-search-full.js` | 抖音搜索结果页 | 验证搜索页 DOM 结构 + API 缓存 |
| `scripts/probe-douyin-video-fields.js` | 抖音视频页 | 验证视频数据多源一致性 |
| `scripts/probe-douyin-author-fields.js` | 抖音博主页 | 验证博主字段 + 串号风险 |

### 验证记录

#### 2026-04-18 — XHS 笔记页探查

**执行页面**: `xiaohongshu.com/explore/69e08600000000001a02fa1b`

**`__INITIAL_STATE__` 结构**:
- ✅ `__INITIAL_STATE__` 存在，顶层 20 个 key
- ✅ `note.noteDetailMap` 存在
- ⚠️ detailMap 含 2 个幽灵 key：`"undefined"` 和 `""`（空字符串），真实 noteId 需用正则 `/^[a-f0-9]{24}$/` 过滤
- ✅ 笔记字段完整性已确认（补充探查）：14 个字段全部正常
- ✅ `noteDetailMap[noteId]` 非 Vue ref，是普通对象（含 `note`, `comments`, `widgets`, `currentTime`）
- ✅ 笔记数据在 `entry.note` 下，包含：xsecToken, desc, imageList, tagList, lastUpdateTime, ipLocation, noteId, type, title, shareInfo, time, user, interactInfo, atUserList

**评论 DOM 选择器**:
- ✅ 全部 14 个评论选择器存活，均正常工作
- ✅ `span:not([class])` 正文提取仍然有效（2/2 通过，时间正则正确过滤）
- 🔍 发现评论正文父级有 `span.note-text` class，可作为更稳定的替代选择器
- ✅ 评论 ID 全部走 `el.id` 路径（17/17），无合成 ID

**卡片选择器**:
- ✅ `section` 28 张卡片、`a.cover` 28 个链接均正常
- ⚠️ `.footer span` 匹配 140 个（过于宽泛，混入评论区）
- ✅ `.play-icon` 正常（1 个视频笔记）

#### 2026-04-18 — XHS 博主页探查

**执行页面**: `xiaohongshu.com/user/profile/5edeed9d00000000010079fc`

**`__INITIAL_STATE__` 用户数据**:
- ✅ `userPageData` 是 Vue ref，`_rawValue` 拆包正常
- ✅ 拆包后包含 `basicInfo`, `interactions`, `tags`, `tabPublic`, `extraInfo`, `result`
- ✅ 粉丝/关注/互动数据完整：关注 184, 粉丝 124,930, 获赞与收藏 669,762
- ✅ tags: ["狮子座", "广东深圳", "撰稿人", "情感博主"]
- ⚠️ `basicInfo.userId` 为空（代码实际从 URL 提取，不影响功能）

**DOM 选择器**:
- ✅ 全部 5 个博主 DOM 选择器正常
- ✅ DOM vs State 交叉验证：名称/小红书号/IP 全部一致

**批量发现选择器**:
- ✅ `.feeds-container` / `#userPostedFeeds` / `section` / `a.cover` 全部正常
- 🔍 **卡片 href 格式变化**：从 `/explore/{noteId}` 变为 `/user/profile/{userId}/{noteId}?xsec_token=...`
- ✅ `extractNoteId()` 兜底逻辑 `url.split('/').pop()` 仍可正确提取 noteId
- ⚠️ 建议给 `extractNoteId` 增加显式正则 `/\/user\/profile\/[a-z0-9]+\/([a-z0-9]+)/i`

#### 2026-04-18 — 抖音搜索页探查

**综合搜索页** (`/search/xxx?type=general`):
- ✅ `RENDER_DATA` 存在（key: `app`）
- ❌ `__INITIAL_STATE__` 不存在
- ❌ 视频链接 `a[href*="/video/"]` = 0 个——**DOM 发现策略完全不可用**
- ⚠️ API 缓存存在但为空（插件未注入时）
- `data-e2e` 分布：仅导航/UI 元素，无搜索结果专用属性

**视频搜索页** (`/search/xxx?type=video`):
- ✅ 视频链接 16 个，格式 `//www.douyin.com/video/{videoId}`
- ✅ `li a[href*="/video/"]` = 16 个——**DOM 发现策略可用**
- ✅ `li:has(a[href*="/video/"])` = 16 个（卡片级选择器）
- ⚠️ 搜索结果卡片无任何 `data-e2e` 属性，无法用 data-e2e 定位
- `data-e2e` 分布与综合搜索页完全相同

**结论**：综合搜索页必须走 API 拦截（`aweme_general` 频道），视频搜索页 DOM 发现可用。
