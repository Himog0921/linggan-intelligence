# 插件接入内容工作台前的代码治理顺序

> 文档类型：插件侧执行顺序清单  
> 更新日期：2026-04-02  
> 目的：把“协议草案”落成插件侧下一轮真实改造顺序  
> 当前状态：治理计划，尚未实施

---

## 1. 这份清单回答什么问题

前两份文档已经回答了两件事：

1. [协议边界清单](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/docs/plans/active/2026-04-02-plugin-workbench-protocol-boundaries.md)
   - 告诉我们，插件接入内容工作台前必须先收口哪些边界
2. [任务协议草案](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/docs/plans/active/2026-04-02-plugin-workbench-task-protocol-draft.md)
   - 告诉我们，对外协议对象大概长什么样

这份清单要解决的是第三个问题：

**插件代码到底该按什么顺序改，才能最小风险地接入内容工作台。**

---

## 2. 先给结论

插件下一轮改造，最正确的顺序不是：

1. 先去改 `popup.js`
2. 先去改 `platforms/douyin/index.js`
3. 先去把所有采集器都改成远程任务模式

而应该是：

1. 先加一层“外部协议适配层”
2. 先把页面能力检查升级成正式握手
3. 先把本地 `collectionRuns` 变成远程任务映射枢纽
4. 先把状态和结果结构化
5. 最后再把远程任务真正接到平台执行链路

也就是说，先改“边界层”，再改“执行层”。

---

## 3. 当前代码里的关键风险点

如果直接从现有代码往工作台联动上接，最容易踩的雷是这 4 个：

1. [popup.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/popup/popup.js)
   - 它是用户操作入口，不应该成为远程任务协议的宿主
2. [index.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/content/index.js)
   - 它是内容总入口，继续往里堆远程协议逻辑会重新变胖
3. [index.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/platforms/douyin/index.js)
   - 抖音适配器已经是高复杂度中心，不适合再塞调度协议
4. 现有 `MSG.*`
   - 它是内部消息总线，不该直接暴露给内容工作台

所以接入工作台时，第一原则应该是：

**尽量新增“协议边界层”文件，少直接把逻辑摊进现有胖文件。**

---

## 4. 必须先守住的规则

开始真正改代码前，插件侧建议守住 6 条规则：

1. 不让内容工作台直接调用内部 `MSG.*`
2. 不把远程任务逻辑直接塞进 `popup.js`
3. 不让 `content/index.js` 重新变成总调度中心
4. 不直接改坏现有单条采集和批量采集交互
5. 不破坏现有 `GET_PAGE_CONTEXT` 和 `collectionRuns` 的兼容性
6. 不在第一轮就去抽象所有平台公共基类

第 6 条很重要。  
现在最该做的是“把插件变成可接入执行端”，不是顺手做一轮过大的平台框架重构。

---

## 5. 推荐改造顺序

## Wave 0：冻结改造边界

### 目标

先划清哪些文件是“协议入口”，哪些文件暂时只当执行黑盒。

### 这轮主要动的地方

建议新增而不是硬改：

1. `src/workbench/`
2. `src/workbench/protocol/`
3. `src/workbench/runtime/`

### 这轮先不要动的地方

先尽量不碰：

1. `src/platforms/xhs/noteCollector.js`
2. `src/platforms/xhs/commentCollector.js`
3. `src/platforms/douyin/videoCollector.js`
4. `src/platforms/douyin/commentCollector.js`
5. `src/platforms/douyin/batchController.js`

### 原因

这些文件现在的主要价值是“真实执行稳定性”，不是协议表达。  
第一轮不应该一边接远程任务，一边重写采集器内部。

---

## Wave 1：建立外部协议适配层

### 目标

让插件第一次拥有“外部任务协议入口”，但内部执行链路先不大改。

### 建议新增文件

1. `src/workbench/protocol/schema.js`
   - 放协议对象枚举、阶段枚举、错误码枚举
2. `src/workbench/protocol/validator.js`
   - 校验 `task.envelope / task.control / capability.check`
3. `src/workbench/runtime/taskEnvelopeMapper.js`
   - 把外部 `task.envelope` 映射成内部执行参数
4. `src/workbench/runtime/taskControlMapper.js`
   - 把 `pause / resume / stop` 映射成内部动作

### 这轮主要接入点

第一落点建议接在 [messageHandlers.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/content/messageHandlers.js) 外围，而不是把新协议直接写进平台模块。

### 成功标准

1. 插件能识别一份正式 `task.envelope`
2. 插件能返回统一的校验失败
3. 现有 Popup 和页内按钮完全不受影响

### 为什么这轮优先

因为没有协议入口，后面所有改造都会变成“对着未来系统硬塞字段”。

---

## Wave 2：升级页面能力握手

### 目标

把现有 `GET_PAGE_CONTEXT` 升成正式 `capability.report`。

### 主要改的文件

1. [contentDataRuntime.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/content/contentDataRuntime.js)
2. [messageHandlers.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/content/messageHandlers.js)
3. [constants.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/shared/constants.js)

### 这轮要补的能力

1. `contextVersion`
2. `canRunTaskTypes`
3. `readiness.ready`
4. `reasonCode`
5. `reasonMessage`
6. `recommendedNextAction`

### 为什么不先去改 Popup

因为 Popup 只是这套能力的一个消费者。  
现在最重要的是把页面上下文从“UI 判断工具”提升成“正式对外握手协议”。

### 成功标准

1. Popup 还能继续工作
2. 新增消费者也能用正式能力报告派单前校验
3. 抖音搜索页和详情页的拒单原因可结构化返回

---

## Wave 3：升级运行记录，打通远程任务映射

### 目标

让本地 `collectionRuns` 从“本地运行记录”升级为“远程任务映射表”。

### 主要改的文件

1. [collectionRunStore.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/db/collectionRunStore.js)
2. [index.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/db/index.js)

### 这轮建议新增字段

1. `externalTaskId`
2. `externalTaskType`
3. `executorInstanceId`
4. `protocolVersion`
5. `resultUploadStatus`
6. `lastHeartbeatAt`

### 这轮的关键原则

尽量是“增量升级”，不是推翻 `collectionRuns`。

### 成功标准

1. 一次远程任务能映射到一个本地 `collectionRunId`
2. 本地记录里能看出这是工作台任务还是手工任务
3. 不破坏现有 Dashboard 和批量链路

---

## Wave 4：统一进度事件和错误包

### 目标

把当前“面向 UI 的状态”提升成“面向系统的进度事件”。

### 主要改的文件

1. `src/shared/messaging.js`
2. [taskUi.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/shared/taskUi.js)
3. [popup.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/popup/popup.js)
4. `src/workbench/runtime/progressReporter.js`
5. `src/workbench/runtime/errorMapper.js`

### 这轮建议做法

1. 保留当前 UI 文案能力
2. 但在底层统一成固定结构：
   - `status`
   - `stage`
   - `metrics`
   - `heartbeatAt`
   - `error.code`

### 这轮特别要注意

不要让 `popup.js` 成为协议生成中心。  
Popup 只应该消费统一事件，不应该负责定义协议。

### 成功标准

1. Popup 和页内任务栏继续正常显示
2. 工作台或执行节点能直接消费结构化进度
3. 字符串错误开始收口成错误码

---

## Wave 5：建立结果打包器

### 目标

能按一次 `collectionRunId` 打包本轮远程结果，而不是只靠全库导出。

### 主要改的文件

建议新增：

1. `src/workbench/runtime/resultPackager.js`
2. `src/workbench/runtime/resultSummaryBuilder.js`

会依赖：

1. [noteStore.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/db/noteStore.js)
2. [commentStore.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/db/commentStore.js)
3. [authorStore.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/db/authorStore.js)
4. [mediaAssetStore.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/db/mediaAssetStore.js)

### 这轮真正要解决的问题

今天插件的数据导出更偏“面板导出”视角。  
而远程任务结果需要的是：

1. 本轮摘要
2. 本轮结构化记录
3. 本轮媒体资产清单

### 成功标准

1. 给一个 `collectionRunId`，能拿到一份结果包
2. 结果包可直接给工作台用
3. 不需要扫描整库再人工筛选

---

## Wave 6：把第一批远程任务真正接起来

### 目标

只接最值钱、最稳定的 4 类任务，不贪大。

### 第一批建议接入的任务

1. `douyin.batchNotes`
2. `douyin.batchComments`
3. `xhs.batchNotes`
4. `xhs.collectAuthor`

### 为什么是这 4 个

1. 都已经有现成执行能力
2. 都代表“工作台派单”的典型远程任务
3. 风险比单条详情动作和评论图片区小

### 这轮主要改的文件

1. [index.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/content/index.js)
2. `src/workbench/runtime/taskExecutor.js`
3. `src/workbench/runtime/taskControlMapper.js`

### 成功标准

1. 外部任务能接入
2. 插件能正确接单、拒单、上报进度、返回结果
3. 手工按钮链路不回退

---

## 6. 哪些文件先不要碰

在正式开始协议接入的前 2 到 3 轮，建议明确先不做大改：

1. [popup.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/popup/popup.js)
   - 除非是消费新结构，否则不做大拆
2. [index.js](/Users/moglenny/proma/选题插件-打磨中/linggan-boom/src/platforms/douyin/index.js)
   - 先当执行黑盒，避免协议和抖音复杂页面状态纠缠
3. 小红书与抖音采集器内部
   - 第一轮不要顺手重写
4. BaseBatchController 抽象
   - 这是后续优化，不是接入工作台的首要前提

---

## 7. 文件层面的推荐职责

为了避免未来继续把逻辑堆进胖文件，建议插件侧新增一层明确职责：

### `src/workbench/protocol/`

负责：

1. 协议对象定义
2. 协议校验
3. 错误码与状态枚举

### `src/workbench/runtime/`

负责：

1. 外部任务 -> 内部动作映射
2. 页面能力握手
3. 进度事件构造
4. 结果打包
5. 远程任务控制

### 现有 `src/content/`

继续负责：

1. 真实页面执行宿主
2. Content Script 消息入口
3. 页面桥接与运行时装配

### 现有 `src/platforms/*`

继续负责：

1. 平台真实执行逻辑
2. 页面识别
3. DOM / API / bridge 级采集

---

## 8. 每一轮的验收重点

### Wave 1-2

重点看：

1. 旧按钮链路有没有回退
2. `GET_PAGE_CONTEXT` 是否仍兼容 Popup
3. 是否开始出现统一 `reasonCode`

### Wave 3-4

重点看：

1. `collectionRuns` 是否能稳定记录远程映射字段
2. 进度事件是否能被 Popup 和外部同时消费
3. 字符串错误是否开始收口

### Wave 5-6

重点看：

1. 按 `collectionRunId` 能否稳定打包本轮结果
2. 远程派单后是否能闭环到结果包
3. 手工按钮是否仍保持原体验

---

## 9. 这轮治理和现有技术债的关系

这份顺序清单，本质上是在主动消化当前几条高价值技术债：

1. `T1`：胖文件问题
   - 用新增边界层文件替代继续往胖文件里塞逻辑
2. `T2`：长任务语义不统一
   - 通过统一生命周期和进度事件解决
3. `T6`：消息协议 envelope 未统一
   - 通过外部任务协议和结果包解决
4. `T8`：`contentRouter` 抽象未完全落地
   - 暂不强推，把它放到协议接入完成后再看

---

## 10. 最重要的判断

对当前插件来说，真正划算的不是“大重构”，而是：

**新增一层轻量但明确的工作台协议边界，把远程任务能力接进来，同时尽量不破坏现有双平台执行链路。**

这条路的优点是：

1. 风险最小
2. 最贴合现有代码现实
3. 后面真的迁到云端时也最容易复用

---

## 11. 接下来最值得马上做的事

如果下一轮开始进入真正代码改造，建议第一步就做这 3 件：

1. 新建 `src/workbench/protocol/` 和 `src/workbench/runtime/`
2. 落第一版 `schema.js + validator.js + taskEnvelopeMapper.js`
3. 把 `GET_PAGE_CONTEXT` 升成正式 `capability.report`

这 3 件做完，插件才算正式从“本地可用插件”迈到“可接入工作台的执行端”。
