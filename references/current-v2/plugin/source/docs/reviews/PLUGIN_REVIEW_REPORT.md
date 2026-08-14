# 灵感爆爆爆插件代码审查报告

> 历史审查存档：下文“稳定封面上传”和 `flywheel-cover-asset-upload` 描述的是当时实现，已于 2026-08-01 被唯一媒体账本路径替代；不作为当前运行规则或验收依据。

> 审查日期：2026-05-07  
> 审查范围：当前本地工作区，只审查不修复  
> 审查人：Cindy + 执行策略  
> 结论口径：风险导向，不做泛泛打分

## 1. 总结结论

这个插件的核心价值不是“页面上能点采集”，而是作为内容工作台的浏览器执行端，在真实登录状态下完成小红书/抖音采集、远程接单、任务回写和异常恢复。

当前结论（2026-05-08 P0-1 复核后）：

| 结论项 | 判断 |
|---|---|
| 本地基线 | 通过，288 个测试全过，构建通过 |
| 核心功能骨架 | 成立，小红书、抖音、远程工位、回写队列都有对应实现和测试 |
| P0-1 回写队列 | 代码层已修复并复核通过；仍需真实断网/重启场景验收 |
| P0-2 消息桥 | 代码层已修复并复核为 nonce 校验；仍需真实 dashboard 场景验收 |
| 企业级稳定验收 | 仍需真实浏览器验收后再判定 |

一句话判断：插件已经具备采集执行端的基础能力，P0-1 数据丢失风险已在代码层收口；下一步重点是跑真实浏览器验收，确认断网、重启和远程任务回写在实际环境中不丢数据。

## 2. P0 阻塞项

### P0-1 数据回写队列存在卡死风险

| 项 | 内容 |
|---|---|
| 位置 | `src/db/workbenchOutboxStore.js`, `src/workbench/runtime/deltaOutbox.js` |
| 证据 | 回写前会把记录标记为 `in_flight`；待发送查询只会重新捞 `pending` 和 `failed` |
| 影响 | 如果浏览器、Service Worker 或电脑在发送中断掉，这批记录可能长期停在 `in_flight`，后续不再自动重试 |
| 产品后果 | 监控任务看起来完成了，但部分采集结果没有回到内容工作台 |
| 修复状态 | 已修复并通过代码层复核 |
| 修复方式 | `in_flight` 记录写入 5 分钟过期时间，过期后自动回到可重试队列 |
| 复核结果 | `workbenchOutboxStore` 会回收过期发送中记录；`deltaOutbox.flush()` 会先做回收再发送；测试覆盖“发送中断后自动补发” |
| 验收 | 代码层已通过；真实浏览器仍需模拟断网/重启，确认工作台最终收到数据 |

这是本轮最重要的问题，因为它直接影响“采集结果不能丢”。当前代码层风险已收口，剩余是实际浏览器环境验收。

### P0-2 Dashboard 消息桥只认字符串来源，缺少一次性校验

| 项 | 内容 |
|---|---|
| 位置 | `src/content/index.js`, `src/content/dashboardBridge.js`, `src/dashboard/utils.js` |
| 原始证据 | Content script 监听页面 `message` 后，只判断 `event.data.source === 'lgboom-dashboard'` |
| 影响 | 平台页面里的脚本如果伪造同样的 source，就可能调用本地数据读取、删除、同步等 dashboard 动作 |
| 产品后果 | 有机会造成本地插件数据被清空，或向工作台写入伪造数据 |
| 修复状态 | 已修复并通过代码层复核 |
| 修复方式 | Dashboard bridge 生成随机 nonce 并写入 `chrome.storage.session`；Dashboard 发送消息前读取 nonce 并随消息带上；Content script 只处理 nonce 匹配的消息 |
| 复核结果 | `src/content/dashboardBridge.js` 已校验 `event.data.nonce === currentNonce`；`src/dashboard/utils.js` 已在 `postMessage` 中附带 nonce；288 个测试通过 |
| 验收 | 页面脚本伪造不带 nonce 或 nonce 错误的 `lgboom-dashboard` 消息时，插件应拒绝处理 |

这个问题比普通 postMessage 通配符更关键，因为它连接到了本地数据和工作台同步能力。

## 3. P1 高优先级问题

### P1-1 工作台服务地址允许任意配置

| 项 | 内容 |
|---|---|
| 位置 | `src/workbench/runtime/pluginAuthorization.js`, `src/workbench/runtime/executionStationClient.js`, `src/workbench/runtime/taskLeaseClient.js`, `src/sync/flywheelSync.js` |
| 影响 | 授权码、设备信息、工位 token、任务请求都跟随配置地址发送 |
| 产品判断 | 本地调试需要 `localhost`，但正式用户不应轻易把服务地址切到未知域名 |
| 建议 | 正式模式只允许 `https://lingganboom.fun`；开发模式才允许 `http://localhost:*`，并给用户明确提示 |

### P1-2 抖音远程任务缺少账号池自动切换

| 项 | 内容 |
|---|---|
| 位置 | `src/background/index.js`, `src/workbench/runtime/cookieManager.js`, `src/popup/App.jsx`, `src/popup/components/AddAccountModal.jsx` |
| 证据 | 远程接单前只对 `xhs` 注入账号 Cookie；添加采集账号也固定写入 `platform: 'xhs'` |
| 影响 | 抖音远程任务依赖当前浏览器已经登录且账号状态正确，无法像小红书一样自动换号、控配额、进冷却 |
| 产品后果 | 抖音任务在少量人工操作场景可用，但不适合作为多工位稳定调度资源 |
| 建议 | 明确产品口径：抖音先作为“当前登录账号执行”，或补齐抖音账号池能力 |

### P1-3 监控任务的页面连接中断仍会记为失败

| 项 | 内容 |
|---|---|
| 位置 | `src/workbench/runtime/taskPoller.js` |
| 证据 | 监控任务遇到页面连接中断时会映射为 `failed` |
| 影响 | 电脑休眠、标签页断开、浏览器重启这类资源问题，会污染真实失败数据 |
| 产品后果 | 运营看到失败日志增多，难以区分“插件暂时不可用”和“平台采集真的失败” |
| 建议 | 与监控中心的状态口径对齐：连接中断优先进入可重试/等待资源口径，不直接算业务失败 |

### P1-4 页面注入脚本会覆盖全局 `fetch` / `XMLHttpRequest`

| 项 | 内容 |
|---|---|
| 位置 | `src/injected/xhsApiCapture.js`, `src/injected/douyinApiCapture.js` |
| 影响 | 这是捕获接口数据的必要手段，但会增加与平台页面脚本冲突、被平台识别的概率 |
| 建议 | 保留能力，但增加原始函数保护、重复安装保护、异常恢复和可观测日志 |

### P1-5 插件本地敏感信息明文存储

| 项 | 内容 |
|---|---|
| 位置 | `chrome.storage.local`, IndexedDB 账号数据 |
| 内容 | 授权 token、工位 token、账号 Cookie |
| 影响 | 普通插件场景可接受，但企业级交付需要明确风险边界 |
| 建议 | 短期增加清除入口和风险提示；长期评估 session storage 或加密存储 |

## 4. P2 中优先级问题

| 编号 | 问题 | 影响 | 建议 |
|---|---|---|---|
| P2-1 | 构建链有 5 个 dev 漏洞 | 发布链路供应链风险 | 单独开依赖升级任务，不在本轮审查中直接 fix |
| P2-2 | 没有 `npm test` 脚本 | 验收命令不统一 | 增加标准测试入口 |
| P2-3 | 没有 ESLint 配置 | 低级问题依赖人工发现 | 建立轻量 lint 门禁 |
| P2-4 | `content.js` 体积偏大 | 页面加载和插件注入可能变慢 | 后续拆分或延迟加载 |
| P2-5 | `chrome.alarms` 注册分散 | 维护成本高 | 集中为一个初始化函数 |
| P2-6 | `postMessage(..., '*')` 和 CustomEvent 暴露较多 | 平台可检测性上升 | 用 nonce 或更窄通信通道 |
| P2-7 | 注入脚本下载文件名未统一过滤 | 可能出现异常文件名 | 复用后台下载文件名过滤规则 |

说明：`cookies` 权限当前不是冗余权限，代码实际使用了 `chrome.cookies.getAll` 和 `chrome.cookies.set/remove`。

## 5. 核心功能矩阵

### 5.1 小红书采集

| 项 | 结论 |
|---|---|
| 功能目标 | 单篇、批量、作者页、搜索页、评论、媒体字段、封面兜底 |
| 代码覆盖 | `src/platforms/xhs/*`, `src/content/xhsPageController.js`, `src/workbench/runtime/monitorTask.js` |
| 已有测试 | `xhs-note-collector`, `xhs-note-discovery`, `xhs-comment-*`, `xhs-selector-health`, `monitor-author-surface-scan` |
| 审查结论 | 有条件通过 |
| 主要风险 | 平台页面变化会影响采集准确性，需要真实浏览器定期验收 |

### 5.2 抖音采集

| 项 | 结论 |
|---|---|
| 功能目标 | 视频采集、搜索/作者页批量、评论、评论图片、封面/视频字段 |
| 代码覆盖 | `src/platforms/douyin/*`, `src/injected/douyinApiCapture.js` |
| 已有测试 | `douyin-search-*`, `douyin-batch-*`, `douyin-comment-*`, `douyin-security-challenge`, `douyin-selector-health` |
| 审查结论 | 有条件通过 |
| 主要风险 | 远程执行依赖当前浏览器登录；账号池和配额治理不如小红书完整 |

### 5.3 远程工位

| 项 | 结论 |
|---|---|
| 功能目标 | 注册工位、心跳、接单、续租、停止、释放、页面启动失败恢复 |
| 代码覆盖 | `src/workbench/runtime/*`, `src/background/index.js` |
| 已有测试 | `workbench-task-lease`, `workbench-task-poller`, `workbench-heartbeat`, `workbench-control-sync`, `navigation-orchestrator` |
| 审查结论 | 有条件通过 |
| 主要风险 | 连接中断和资源不可用的失败口径还要与服务端统一 |

### 5.4 数据回写

| 项 | 结论 |
|---|---|
| 功能目标 | 本地 outbox、断点续传、去重、稳定封面上传、工作台归属 token |
| 代码覆盖 | `src/sync/flywheelSync.js`, `src/workbench/runtime/deltaOutbox.js`, `src/db/workbenchOutboxStore.js` |
| 已有测试 | `workbench-delta-outbox`, `flywheel-cover-asset-upload`, `flywheel-sync-quality`, `dashboard-workbench-sync` |
| 审查结论 | 代码层有条件通过，待真实断网/重启验收 |
| 主要风险 | 已补 `in_flight` 超时恢复；实际浏览器中仍需验证恢复时机和重复回写去重 |

### 5.5 用户界面

| 项 | 结论 |
|---|---|
| 功能目标 | popup、页内按钮、dashboard、进度、错误提示、同步、下载 |
| 代码覆盖 | `src/popup/*`, `src/content/components/*`, `src/dashboard/*` |
| 已有测试 | `popup-*`, `dashboard-*`, `inject-button-group-ui`, `ux-feedback-system` |
| 审查结论 | 有条件通过 |
| 主要风险 | Dashboard 消息桥需要补来源校验 |

## 6. 值得保留的设计

| 设计 | 为什么值得保留 |
|---|---|
| 任务账本 + 租约 | 能支撑多工位接单，避免重复执行 |
| 本地 outbox | 方向正确，是离线回写的基础 |
| 稳定封面上传 | 不再只依赖平台临时图片链接 |
| 45 秒页面未启动恢复 | 能处理“接单了但页面没跑起来”的常见问题 |
| selector health 测试 | 能提前发现平台页面变化 |
| 协议验证器 | 已经能阻断不合法任务信封 |

## 7. 建议执行顺序

1. 完成 P0 复核：P0-1、P0-2 均已通过代码层复核。
2. 跑 `PLUGIN_ACCEPTANCE_CHECKLIST.md` 的真实浏览器验收，重点验证断网、重启、远程回写和 dashboard 伪造消息。
3. 修 P1：服务地址白名单、抖音账号口径、监控失败分类、注入脚本稳定性。
4. 再决定是否进入依赖升级、lint、体积优化等 P2 改造。

## 8. 最终判断

当前插件可以继续作为内部测试和小规模执行端使用。P0-1、P0-2 已通过代码层复核，但还不建议跳过真实浏览器验收直接按“企业级稳定采集执行端”通过。

通过 P0 收口和真实浏览器验收后，再决定是否进入 P1 修复和正式验收更稳妥。
