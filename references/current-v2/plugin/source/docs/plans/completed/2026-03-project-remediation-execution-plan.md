# 2026-03 项目治理与结构修复执行编排

> 来源：`2026-03-project-retrospective-audit.md` + `2026-03-project-remediation-checklist.md`  
> 目的：把修订清单转成按依赖排序、可连续执行的工作波次  
> 当前状态：已归档（2026-03 治理编排，内容已被后续工作覆盖，留作历史参考）

---

## 1. 执行原则

1. 先修“事实源”，再修“结构”，最后修“体验”。
2. 每一波结束都要同时更新：
   - 对应权威文档
   - `progress.txt`
   - 必要时的 `docs/decisions/index.md`
3. 不允许跨波次偷跑。
   - 上一波未闭环，不进入下一波
4. 每一波都必须有可观察结果，不做纯理论整理。
5. 从 2026-03-29 起，每一轮“实现 + 测试 + 用户反馈”都触发文档同步硬门禁：
   - 最低必须更新 `progress.txt`
   - 用户可见变化必须同步产品文档
   - 协议/字段/页面事实变化必须同步技术文档
   - 新策略/新规则必须同步决策日志
   - 未同步文档的回合，只能视为“部分完成”

---

## 2. 波次总览

| 波次 | 主题 | 目标 | 依赖 |
|------|------|------|------|
| Wave 0 | 权威关系收口 | 统一项目事实源与历史资料边界 | 无 |
| Wave 1 | 文档现实对齐 | 修正产品/架构/协议/数据层文档时效漂移 | Wave 0 |
| Wave 2 | 决策与时间线闭环 | 把近几轮关键架构决策与进展补回系统 | Wave 1 |
| Wave 3 | 结构阻塞点治理 | 处理主入口、抖音视频主链路、任务上下文接线 | Wave 2 |
| Wave 4 | UI/工作流治理 | 修正 prompt/confirm/alert、可访问性、统一反馈 | Wave 3 |
| Wave 5 | AI-ready 深化 | 补原始证据层与分析契约接线 | Wave 3 |

---

## 3. Wave 0：权威关系收口

### 目标

让任何后续开发者或智能体都能立刻知道：

- 哪些文档是当前事实源
- 哪些文档只是历史资料
- 哪些文档不该再作为实现依据

### 任务

1. 更新 `AGENTS.md`
   - 新增 Source of Truth 小节
   - 明确外层 `01/02/03` 的历史定位
   - 明确内层 `docs/**` 的当前权威关系
2. 为外层 `02_*.md`、`03_*.md` 增加历史说明
3. 明确 `BACKEND_STRUCTURE.md`、`IMPLEMENTATION_PLAN.md` 当前不是最高权威

### 完成标志

- 新成员只看 `AGENTS.md` 就不会走错文档入口
- 外层 PRD/架构文档不再被误认作现行事实源

---

## 4. Wave 1：文档现实对齐

### 目标

把当前已经跑通的产品能力和真实代码状态收进权威文档。

### 任务

1. 更新 `docs/product/PRD.md`
   - 抖音能力矩阵对齐当前现实
   - 区分已完成、部分完成、待验收
2. 更新 `docs/product/APP_FLOW.md`
   - 抖音从“单篇为主”修正为当前真实交互流
3. 更新 `docs/technical/MESSAGE_PROTOCOL.md`
   - 补齐 `commentLimit`
   - 补平台差异化 payload
   - 补 dashboard/content/background 真实边界
4. 更新 `BACKEND_STRUCTURE.md`
   - 同步 Dexie v6
   - 同步 `collectionRuns` 与 `mediaAssets`
5. 更新 `IMPLEMENTATION_PLAN.md`
   - 不再保留“抖音批量/评论待启动”的旧状态

### 完成标志

- 产品、架构、协议、数据层文档不再互相打架
- 抖音现实能力在权威文档中可追溯

---

## 5. Wave 2：决策与时间线闭环

### 目标

把“最近几轮靠聊天和记忆维持的事实”沉淀回正式系统。

### 任务

1. 更新 `docs/decisions/index.md`
   - 记录抖音视频上下文建模转向
   - 记录分享按钮触发采集
   - 记录批量视频改为作品列表 API 驱动
   - 记录批量评论改为作品列表 + 页面桥接
   - 记录 webRequest 拦截链路的替代/移除结论
2. 更新 `progress.txt`
   - 补 2026-03-25 到 2026-03-27 的缺口
   - 补本轮治理与文档修订动作

### 完成标志

- 关键架构转向不再只存在于聊天记录
- `progress.txt` 与当前现实重新对齐

---

## 6. Wave 3：结构阻塞点治理

### 目标

解决已经明确会拖慢后续开发的结构问题。

### 当前进展（2026-03-27）

- 已完成首刀：
  - 新增 `src/content/douyinBatchMessageHandlers.js`，将抖音批量视频/评论消息处理从 `src/content/index.js` 中抽离
  - `src/content/index.js` 规模从 1545 行下降到 1340 行，抖音批量链路不再直接堆在主入口里
  - `collectionRuns` 已真实接入抖音批量视频与批量评论
  - 抖音批量视频记录、评论记录、评论图片区媒体资产均开始带 `collectionRunId`
- 已完成第二刀：
  - 新增 `src/platforms/douyin/videoContext.js`，把“当前视频上下文解析、稳定判定、上下文级 API 命中”从 `videoCollector.js` 中抽出
  - `src/platforms/douyin/videoCollector.js` 从 1655 行进一步收敛到 1231 行
  - 抖音单条/批量视频现在共享独立的上下文解析层，而不是继续堆在采集器主文件里
- 已完成第三刀：
  - 新增 `src/content/commentImageTask.js`，将小红书评论图片区下载任务从 `src/content/index.js` 中抽离
  - 评论图片区下载开始真实接入 `collectionRuns`，任务现在具备创建、完成、失败、停止记录
  - `src/content/index.js` 从 1340 行进一步下降到 1061 行
- 已完成第四刀：
  - 新增 `src/content/mediaDownloadUtils.js` 与 `src/content/noteMediaDownload.js`
  - 将笔记媒体下载、媒体重试刷新、Blob 回退与下载摘要写回从 `src/content/index.js` 中抽离
  - `src/content/index.js` 从 1061 行进一步下降到 675 行，主入口职责收口到消息编排与 UI 桥接
- 已完成第五刀：
  - 新增 `src/content/messageHandlers.js` 与 `src/content/dashboardBridge.js`
  - 将 content 层消息处理表与 dashboard iframe bridge 从 `src/content/index.js` 中抽离
  - `src/content/index.js` 从 675 行进一步下降到 490 行，主入口开始接近 bootstrap + task state + XHS page actions 的职责边界
- 已完成第六刀：
  - 新增 `src/content/xhsPageController.js`，将 XHS 页内动作、任务状态协调、评论图片区任务控制整体从 `src/content/index.js` 中抽离
  - `src/content/index.js` 从 490 行进一步下降到 242 行，主入口现在主要承担 bootstrap、平台分发、消息接线
  - 结构拆分收益已明显超过“单文件可读性”阶段，下一步瓶颈转为 `content.js` 加载体积而非入口职责混杂
- 已完成第七刀：
  - 将 `jszip` 从抖音评论图片区下载链路中改为动态导入，抖音与小红书两侧的 ZIP 依赖都不再静态留在 content 首包
  - 首次验证真实异步 chunk 生效，`content.js` 从约 348 KiB 下降到约 256 KiB
  - 结论明确：下一阶段不能只继续拆文件，必须开始处理平台运行时的按需加载
- 已完成第八刀：
  - 新增 `src/content/douyinRuntime.js` 与 `src/content/douyinRuntimeModule.js`
  - 将抖音平台适配器、单条采集、批量采集、评论采集、媒体刷新统一移出 content 首包，改为“进入抖音页面或执行抖音动作时再加载”
  - `src/content/index.js` 保持在约 248 行，职责未回涨；但更关键的是加载边界已从“文件拆分”升级为“平台运行时拆分”
  - 构建结果显示 `content.js` 已进一步下降到约 188 KiB，首包体积告警消失，并产出独立异步 chunk
- 已完成第九刀：
  - 新增 `src/platforms/douyin/videoApiData.js`
  - 将抖音视频采集中的 API 缓存、render/router/detail 映射、标题/IP 规范化与 detail 刷新能力从 `videoCollector.js` 中抽离
  - `src/platforms/douyin/videoCollector.js` 从 1231 行下降到约 864 行，抖音视频主链路开始形成“上下文层 + API 数据层 + 采集/下载编排层”的分层
  - 构建验证通过，`content.js` 首包仍稳定在约 188 KiB，没有因为结构拆分而反弹
- 已完成第十刀：
  - 新增 `src/platforms/douyin/videoDom.js` 与 `src/platforms/douyin/videoDownload.js`
  - 将互动数读取、作者名提取、IP/封面/作者 DOM 辅助、等待逻辑、Blob 下载回退从 `videoCollector.js` 中抽离
  - 移除 `videoCollector.js` 中已不再参与主链路的旧等待逻辑，避免继续在单文件里保留历史分支
  - `src/platforms/douyin/videoCollector.js` 再次从约 864 行下降到约 642 行，抖音视频主链路进一步收口为“采集/下载编排层”
  - 构建验证通过，`content.js` 首包继续稳定在约 188 KiB
- 已完成第十一刀：
  - 新增 `src/content/contentDataRuntime.js`
  - 将 `noteStore / commentStore / authorStore`、dashboard bridge、内容消息处理、笔记媒体下载服务移出 content 首包，改为需要时再加载
  - `src/content/index.js` 保持在约 251 行，职责仍然收口在 bootstrap、平台分发与消息接线
  - 构建结果显示 `content.js` 首包进一步下降到约 179 KiB，首包内已不再包含 `jszip`
- 已完成第十二刀：
  - 新增 `src/platforms/douyin/commentApi.js` 与 `src/platforms/douyin/commentMedia.js`
  - 将抖音评论链路中的评论/回复接口请求、页面桥接 fetch、评论记录映射、评论图片区候选解析、图片下载辅助从 `commentCollector.js` 中抽离
  - `src/platforms/douyin/commentCollector.js` 收敛到约 318 行，职责开始接近“评论采集编排层 + ZIP 导出入口”
  - 构建验证通过，`content.js` 首包继续稳定在约 179 KiB
- 已完成第十三刀：
  - 新增 `src/platforms/douyin/batchDiscovery.js`
  - 将抖音批量链路中的博主页作品列表发现、点赞排序、任务建档与批量节流辅助从 `batchController.js` 中抽离
  - `src/platforms/douyin/batchController.js` 从约 486 行下降到约 315 行，职责更接近“批量视频/评论编排层”
  - 构建验证通过，`content.js` 首包继续稳定在约 179 KiB
- 已完成第十四刀：
  - 新增 `src/platforms/xhs/batchShared.js` 与 `src/platforms/xhs/batchCommentController.js`
  - 将小红书批量评论控制器与批量页面共享辅助从 `xhs/batchController.js` 中抽离
  - `src/platforms/xhs/batchController.js` 从约 972 行下降到约 595 行，XHS 批量链路不再把视频与评论控制器堆在同一文件
  - 构建验证通过，`content.js` 首包维持在约 179 KiB，说明结构收口未引起首包回涨
- 仍待完成：
  - 继续评估 Dexie 与 dashboard 相关依赖的加载边界，决定是否需要第二轮首包治理
  - 评估是否需要将 dashboard / 导出 / Dexie 相关链路进一步懒加载，避免后续功能继续把首包抬回去
  - 继续观察小红书批量笔记控制器是否也需要独立成单文件，进一步对齐抖音/小红书双平台的批量结构

### 任务

1. 拆分 `src/content/index.js`
   - bootstrap
   - message router
   - task orchestration
   - dashboard bridge
2. 模块化抖音 `videoCollector.js`
   - current video context resolver
   - API cache / alias registry
   - detail refresh / media resolution
3. 让 `collectionRuns` 真正接入：
   - 批量视频
   - 批量评论
   - 评论图片区下载
4. 补 `collectionRunId` 到评论采集结果

### 完成标志

- `src/content/index.js` 不再是 1500+ 行单点中心
- 抖音单条/批量视频共享同一上下文层
- 批量任务在本地数据中可按任务级别追溯
- 小红书评论图片区与笔记媒体下载不再继续堆在 content 主入口里
- content 层消息路由与 dashboard bridge 不再继续与 bootstrap 混写
- `src/content/index.js` 降到 250 行以内，主入口结构收口完成
- `content.js` 首包降到 200 KiB 以内，体积告警解除
- `commentCollector.js` 降到 350 行以内，并完成 API 层 / 媒体层分离
- `douyin/batchController.js` 降到 350 行以内，并完成发现层 / 编排层分离
- `xhs/batchController.js` 下降到 600 行以内，并把批量评论控制器独立成单文件

---

## 7. Wave 4：UI 与工作流治理

### 目标

把插件从“工程师可用”推进到“非技术用户也稳定可用”。

### 当前进展（2026-03-27）

- 已完成首刀：
  - Popup 设置弹层补齐 `aria-live / aria-atomic / aria-hidden / role="dialog" / aria-modal / :focus-visible`
  - Dashboard 新增统一通知区与自定义确认弹层，不再依赖浏览器原生阻塞弹窗
  - 抖音页内注入按钮补齐 `role / aria-live / aria-atomic`，并新增统一动作弹层 `showDouyinActionDialog`
- 已完成第二刀：
  - Popup、Dashboard、抖音页内注入三处的 `alert / confirm / prompt` 已全部清零
  - 抖音单条评论、批量视频、批量评论都改为自定义弹层采参，不再依赖 `window.prompt / confirm`
  - Dashboard 清空、导出、下载等操作统一改为通知区反馈 + 自定义确认
- 已完成第三刀：
  - Popup 已补齐抖音视频弹层页“💬 采集当前评论”与“🖼 评论图片区”双入口
  - 单条评论与评论图片区下载都改为统一的上限设置弹层，不再只依赖页内抖音按钮
  - 评论图片区下载现在也具备 Popup 侧的正式产品入口，便于后续验收与新手使用
- 当前结论：
  - Wave 4 的“去原生阻塞弹窗”和“无障碍基线补齐”主目标已达成
  - “抖音评论与评论图片区”已经进入统一弹层/统一反馈的产品化阶段
  - 剩余工作不再是清理阻塞 API，而是后续若扩充 UI，需要继续沿用这套通知区/弹层基线，避免回退

### 任务

1. Popup 去 `alert / confirm / prompt`
2. 页内注入按钮流程去 `window.prompt / confirm`
3. Dashboard 去原生阻塞弹窗
4. 统一任务反馈组件：
   - 设置弹层
   - 进行中反馈
   - 成功/失败反馈
5. 补无障碍基线：
   - `aria-live`
   - `role="dialog"`
   - `aria-modal`
   - `:focus-visible`

### 完成标志

- Popup、Dashboard、页内注入三处交互风格一致
- 不再依赖浏览器原生阻塞弹窗
- 可通过一次 Web Interface Guidelines 回归审查

---

## 8. Wave 5：AI-ready 深化

### 目标

把“schema 已声明”推进到“数据真正可分析”。

### 当前进展（2026-03-27）

- 已完成首刀：
  - 新增 `src/shared/collectorMetadata.js`
  - 统一提供 `collectorVersion / rawPayload / rawDomText / rawShareText / rawUrl / rawSource`
- 已完成第二刀：
  - 小红书内容、评论、作者采集器已接入最小原始证据层
  - 抖音视频、评论、作者采集器已接入最小原始证据层
  - 现有记录从“仅展示友好”提升为“最小可追溯、最小可回算”
- 已完成第三刀：
  - `handle / redId / douyinId` 的跨层语义已回写进 `DATA_MODEL.md` 与 `AI_READY_DATA_CONTRACT_V1.md`
  - 后续 UI、导出、分析层都以 `handle` 为统一展示字段，平台特有字段降级为兼容字段
- 已完成第四刀：
  - Dashboard 作者面板已改为优先展示统一 `handle`
  - 内容导出链路中的作者 CSV 已改为优先导出统一 `handle`，并补入 `collectorVersion / rawSource / rawUrl`
  - `authorStore.search()` 也开始按统一 `handle` 检索，避免治理只停留在文档层
- 已完成第五刀：
  - Dexie schema 升级到 v7，`mediaAssets` 新增 `collectionRunId` 索引
  - `mediaAssetStore` 已支持按 `collectionRunId` 读取，为评论图片区与后续内容媒体资产的任务级追溯做准备
- 已完成第六刀：
  - Dashboard 作者表已优先展示统一 `handle`
  - `messageHandlers` 的 notes/comments/authors CSV 导出已补齐 `collectionRunId / collectorVersion / rawSource / rawUrl / rawShareText / rawDomText / rawPayload`
  - 作者搜索也已优先按统一 `handle` 命中
- 已完成第七刀：
  - `src/content/noteMediaDownload.js` 已在笔记/视频媒体下载前后写入或回填 `mediaAssets`
  - 内容媒体现在可按 `contentId` / `collectionRunId` 追溯，不改变原有下载流程
- 已完成第八刀：
  - 新增 `src/db/recordNormalization.js`
  - `noteStore / commentStore / authorStore / mediaAssetStore` 在读写两端统一走标准化逻辑
  - 历史记录即使不重采，也会在展示、导出、搜索时尽量补齐 `contentId / platformContentId / handle / collectionRunId / collectorVersion / raw*`
- 已完成第九刀：
  - 新增 `src/db/legacyDataMaintenance.js`
  - 提供显式历史数据回填入口，可将 `notes / comments / authors / mediaAssets` 一次性批量持久化回写为新契约
  - 形成“读时标准化 + 显式批量回填”双路径，减少后续人工临时脚本修库
- 已完成第十刀：
  - Popup 已新增“🧹 数据维护”入口，维护动作可直接触发 `backfillLegacyAiReadyFields()`
  - 历史数据回填能力不再只存在于内部模块里，而是成为正式维护动作
  - 这一步让“历史数据治理”从文档与内部工具，真正进入可操作的产品维护层
- 已完成第十一刀：
  - Dashboard 评论表已开始直接展示 `platform / contentId / level / replyToUserName / collectionRunId`
  - 评论树关键字段不再只存在于导出与底层存储里，而是进入可见层，便于人工校验和后续分析
- 已完成第十二刀：
  - 抖音批量视频记录开始持久化 `batchSelectionMode / batchRank / batchLikesSnapshot`
  - “按点赞 Top N” 不再只在批量执行时生效，采集完成后仍可在 note 记录、导出与面板中追溯入选原因
  - notes CSV 与 Dashboard 抖音视频表已开始消费这组三段式批量选择字段
- 已完成第十三刀：
  - Popup“🧹 数据维护”已补齐结果格式化逻辑
  - 有实际回填时会显示总回填数与分表结果；无变更时明确提示“无需回填”
  - 数据维护从“可执行”进一步收口到“结果可理解”
- 当前结论：
  - Wave 5 的“原始证据层落地”主目标已完成第一阶段
  - 历史数据兼容策略已从“待设计”推进到“运行时标准化 + 显式回填工具”双路径
  - 历史数据回填现在已经具备正式入口；评论层级关键字段也已进入 Dashboard 可见层；抖音批量视频 Top N 的选择依据也已进入可见层与导出层；仍待推进的是是否默认执行回填，以及更大范围消费新语义；`mediaAssets` 对内容媒体的最小覆盖已就位

### 任务

1. 为内容/评论采集补原始证据层：
   - `collectorVersion`
   - `rawPayload`
   - `rawSource`
   - 必要时的 `rawDomText`
2. 统一跨平台评论树分析字段
3. 明确 `handle / redId / douyinId` 在 UI、导出、分析层的语义分工
4. 让 `mediaAssets` 更完整覆盖内容媒体与评论图片区

### 完成标志

- 数据不仅能展示，也能支撑后续大模型分析
- 历史回算与异常回溯具备证据基础
- 内容媒体和评论图片区都能按 `contentId` / `collectionRunId` 回溯到资产记录

---

## 9. 近期推荐顺序

当前 Wave 0 - Wave 5 已经完成首轮治理闭环。下一轮推荐顺序改为：

1. 评估是否执行一次历史数据显式回填，处理既有记录的 `handle / collectorVersion / raw*` 缺口
2. 将 AI-ready 字段进一步推入更多展示、导出与分析场景
3. 继续完成抖音评论图片区高清下载、单条评论体验与数据面板二次下载的长时效收口
4. 仅当首包重新失衡时，再重开第二轮加载边界治理

原因：

- 当前“事实统一 + 结构拆分 + UI 去阻塞 + 原始证据层”已经成型
- 第二轮 bundle 治理已完成评估：当前 `content.js` ≈ 185 KiB，且构建告警已消失，不值得继续为微小收益增加运行时复杂度
- 后续治理重点不再是大面积拆文件，而是把历史数据、分析可用性与产品体验补完整

---

## 10. 每波统一验收模板

每一波结束，统一按下面四项验收：

1. 对应文档是否已同步
2. `progress.txt` 是否已补记录
3. 若涉及架构转向，`docs/decisions/index.md` 是否已补
4. 是否明确了下一波可以开始的前置条件

只有四项都满足，当前波次才算真正完成。
