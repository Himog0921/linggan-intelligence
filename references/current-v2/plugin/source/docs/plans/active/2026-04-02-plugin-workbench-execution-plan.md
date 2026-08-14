# 插件接入内容工作台执行总计划

> 文档类型：插件侧总执行计划  
> 更新日期：2026-04-03  
> 目标：在不破坏现有双平台采集能力的前提下，把插件推进成可被内容工作台调度的重执行端  
> 执行原则：先边界层，后执行层；先协议化，后联调；先最小闭环，后扩任务面

---

## 1. 当前计划要解决什么

当前插件已经具备：

1. 小红书与抖音双平台采集能力
2. Popup / Content / Background / Dashboard 四入口结构
3. 本地数据层与 `collectionRuns`
4. 页面上下文判断与长任务 UI

但还不具备：

1. 正式外部任务协议
2. 正式能力握手协议
3. 远程任务与本地 run 的稳定映射
4. 面向工作台的结果包

所以接下来不该再零散修边角，而要按明确波次推进。

---

## 2. 总体路线

### Phase A：边界层成型

目标：

1. 建立 `src/workbench` 目录
2. 定义协议对象
3. 建立任务信封与控制映射
4. 升级页面能力握手

### Phase B：执行记录与事件标准化

目标：

1. 升级 `collectionRuns`
2. 统一进度事件
3. 统一错误包
4. 形成结果包

### Phase C：第一批远程任务接入

目标：

1. 只接第一批高价值任务
2. 建立最小联调闭环
3. 确保手工入口不回退

---

## 3. 波次拆解

## Wave 1：协议骨架

### 目标

新增协议边界层文件，不改平台执行器内部逻辑。

### 产出

1. `src/workbench/protocol/schema.js`
2. `src/workbench/protocol/validator.js`
3. `src/workbench/runtime/taskEnvelopeMapper.js`
4. `src/workbench/runtime/taskControlMapper.js`

### 完成标准

1. 可识别外部任务对象
2. 可校验基本字段
3. 可把远程任务类型映射到内部任务语义

---

## Wave 2：能力握手

### 目标

把 `GET_PAGE_CONTEXT` 提升成正式 `capability.report` 基线。

### 产出

1. `contextVersion`
2. `canRunTaskTypes`
3. `readiness.reasonCode`
4. `recommendedNextAction`

### 完成标准

1. Popup 仍兼容
2. 外部调用方可正式做派单前校验

---

## Wave 3：远程任务映射

### 目标

让 `collectionRuns` 支持远程任务映射。

### 产出

1. `externalTaskId`
2. `externalTaskType`
3. `executorInstanceId`
4. `protocolVersion`
5. `resultUploadStatus`
6. `lastHeartbeatAt`

### 完成标准

1. 一次远程任务可绑定一次本地 run
2. Dashboard 和现有链路不回退

---

## Wave 4：进度与错误协议化

### 目标

把现有状态更新从“UI 内部语义”升级成“系统可消费事件”。

### 产出

1. `progressReporter`
2. `errorMapper`
3. 统一 `status / stage / metrics / heartbeatAt`

### 完成标准

1. Popup 和页内任务栏继续工作
2. 外部调用方可消费进度和错误事件

---

## Wave 5：结果包

### 目标

按 `collectionRunId` 打包本轮结果。

### 产出

1. `resultPackager`
2. `resultSummaryBuilder`

### 完成标准

1. 结果不再依赖整库导出
2. 能直接为工作台原始数据层提供输入

---

## Wave 6：第一批任务落地

### 第一批任务

1. `douyin.batchNotes`
2. `douyin.batchComments`
3. `xhs.batchNotes`
4. `xhs.collectAuthor`

### 完成标准

1. 能接单
2. 能拒单
3. 能上报进度
4. 能返回结果包

---

## 4. 暂不推进的内容

当前阶段先不做：

1. BaseBatchController 抽象
2. `contentRouter` 全量重构
3. 评论图片区远程任务首批接入
4. 所有单条动作的远程化

原因很简单：这些都不是最小闭环必需项。

---

## 5. 任务优先级

### P0

1. Wave 1
2. Wave 2
3. Wave 3
4. Wave 4

### P1

1. Wave 5
2. Wave 6 第一批任务

### P2

1. 第二批任务扩容
2. 平台公共抽象再治理

---

## 6. 每轮执行要求

每一轮都必须同时做 4 件事：

1. 代码变更
2. 构建验证
3. 文档同步
4. `progress.txt` 回填

如果只改代码不回填文档，这轮不算闭环。

---

## 7. 当前启动点

本轮开始执行：

1. 先完成 Wave 1
2. 完成后立即进入 Wave 2
3. 每一轮结束后再决定是否继续往下推进

这意味着现在已经正式进入代码落地阶段，不再停留在纯梳理阶段。

---

## 8. 2026-04-02 当前执行快照

### 已完成

1. Wave 1：协议骨架
2. Wave 2：能力握手
3. Wave 3：远程任务映射
4. Wave 4：进度与错误协议化
5. Wave 5：结果包
6. Wave 6：第一轮长任务心跳统一

### 进行中

1. Wave 6：剩余长尾链路与实机回归

### 已落地的 Wave 6 子链路

1. `douyin.batchNotes`
   - 已支持 `externalTaskId -> collectionRun -> result package`
2. `douyin.batchComments`
   - 已支持 `externalTaskId -> collectionRun -> result package`
3. `xhs.collectAuthor`
   - 已完成远程派单、run 映射与结果包回取
4. `xhs.batchNotes`
   - 已完成远程派单、run 映射与结果包回取
   - 2026-04-03 已修复“第一篇目标 note 未稳定就提前关闭”的实机问题
5. `xhs.batchComments`
   - 已完成远程派单、run 映射与结果包回取
   - 2026-04-03 已完成心跳实机验证与运行中摘要对齐

### 当前 Wave 6 真正剩余的长尾项

1. Dashboard 对远程结果的二次消费体验
2. 长任务暂停 / 继续 / 停止 语义进一步统一

### 真实页面验证状态

截至 2026-04-02，本计划的第一批核心远程任务已经完成以下真实浏览器验证：

1. `xhs.collectAuthor`
2. `xhs.batchNotes`
3. `xhs.batchComments`
4. `douyin.batchNotes`
5. `douyin.batchComments`
6. `douyin.collectAuthor`
7. `douyin.singleComments`
8. `douyin.commentImageDownload`

统一验证结论：

1. 远程任务协议入口已经能稳定映射到插件内部动作
2. Background -> Content -> 页面侧 Dexie -> 结果包回传 的主链路已被双平台验证成立
3. 当前剩余工作重点已从“证明可行性”转向“补长尾链路、收口语义、继续治理体积与复杂度”

### 2026-04-02 第二轮代码收口

本日新增一轮针对长任务运行记录的统一治理：

1. 新增 `src/workbench/runtime/heartbeat.js`
2. 已把 `collectionRunStore.markHeartbeat()` 接入以下长任务进度链路：
   - `xhs.batchNotes`
   - `xhs.batchComments`
   - `xhs.singleComments`
   - `xhs.commentImages`
   - `douyin.batchNotes`
   - `douyin.batchComments`
   - `douyin.singleComments`
   - `douyin.commentImageDownload`
3. 当前结论：
   - `lastHeartbeatAt` 已不再停留在 run 创建瞬间
   - 但这轮仍属于“代码已落地 + 本地测试通过”，后续还需补一轮真实页面长时任务验证
   - 已支持把 `collectionRunId` 传入作者采集器
   - 已支持完成/失败后回写 run 状态
   - 已支持结果包通过作者记录关联 run
   - 已改为“先接单、再异步执行”，避免远程派单阻塞在作者采集完成前
   - Background 对这类 `asyncDispatch` 任务不再等待 content 即时回包，接单语义与执行语义正式拆开
4. `xhs.batchNotes`
   - 已支持远程派单时创建 `collectionRun`
   - 已支持把 `collectionRunId` 传入 note 采集器
   - 已支持结束时回写 `markDone / markStopped / markFailed`
5. `xhs.batchComments`
   - 已支持远程派单时创建 `collectionRun`
   - 已支持把 `collectionRunId` 传入 comment 采集器
   - 已支持结束时回写 `markDone / markStopped / markFailed`
6. 远程 run 生命周期
   - 远程任务创建 run 时会进入 `pending_upload`
   - 结果包生成后会推进到 `packaged`
   - `lastHeartbeatAt` 已在远程 run 创建时初始化
7. `douyin.singleComments`
   - 2026-04-09 已完成远程任务类型映射到 `collectSingleComment`
   - 已支持 `asyncDispatch -> collectionRun -> result package` 的最小代码闭环
   - 已完成真实详情页验证：`accepted: true`，`status: done`，结果包成功回取 20 条评论
8. `douyin.commentImageDownload`
   - 2026-04-09 已完成远程任务异步接单与页面侧 `collectionRun` 绑定
   - 已修复此前“本地下载成功但 externalTaskId 无法回查结果包”的映射缺口
   - 已完成真实详情页验证：可正确进入 `done` 并回取结果包；本次样本页无评论图片区素材，因此返回空结果

### 2026-04-03 回归补记

1. `xhs.batchNotes`
   - 第一篇过早关闭问题已完成真实页面复测并修复
   - 当前已切换为“目标 note 数据连续稳定确认后再采集”的保守语义
2. 当前工作重点
   - `xhs.batchNotes` 不再是未决主风险
   - 下一轮重点回到剩余长尾链路、抖音侧未完全收口项与治理同步

### 当前剩余缺口

1. 实机联调
   - 代码闭环已补齐，但还没完成真实页面验收
2. 结果包实测
   - 当前结果包依赖 run 关联写入已补齐，但仍需真实页面数据验证
3. 体积治理
   - 当前 `content.js` 已回升到约 `215 KiB`
   - `contentDataRuntime` 已因真实页面 chunk 丢失改回主包静态加载，后续体积优化应优先从 `background`、抖音运行时和长尾依赖入手
4. 心跳续写
   - `lastHeartbeatAt` 已初始化，但长任务过程中的持续刷新还未系统接入
5. 真实页面加载稳定性
   - `GET_PAGE_CONTEXT` 曾在小红书真实页面因 `contentDataRuntime` 的异步 chunk 丢失而失效
   - 当前已改回主包静态加载，仍需继续完成实机回归确认
