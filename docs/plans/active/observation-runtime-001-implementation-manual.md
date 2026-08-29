# OBSERVATION-RUNTIME-001 实施手册

> 状态: 活跃计划
> 最后核对: 2026-08-29
> 适用范围: Issue #94；观察目标与规则、Rust API/worker、PostgreSQL、Browser Producer、材料投影、Evidence Library、本机发布
> 事实来源: 当前 `main`、PR #84/#87/#88/#91 的已实现代码、内容工作台已验证的执行经验、`collection-monitoring-rules.md`、`capture-control-contract.md`、`media-lifecycle-contract.md`
> 冲突时以谁为准: 用户最新授权、`AGENTS.md`、领域不变量、当前代码与运行回执；本文不允许实现方自行扩大真实平台访问

## 1. 交付结果

用户完成一次有意动作——设立观察目标并启用规则——之后，不再需要去插件弹窗点击“领取任务”。系统自行完成：

```text
观察目标 + 已启用规则
        ↓
Rust worker 判断到期并完成准入
        ↓
有界 Work Order 与顺序 Step
        ↓
Browser Producer 自动签到、领取、执行
        ↓
不可变 Package + Receipt
        ↓
类型化 Material Projection
        ↓
Evidence Library 展示标题、封面、作者、时间、数据状态和来源
```

完成不是“代码存在”或“页面能打开”。完成必须同时满足：实现进入 `origin/main`；新插件发布包可复现；本机 API 与巡检 worker 常驻；一个真实目标无需点击领取即可形成新 Receipt；对应作品在 Evidence Library 可见。

## 2. 不重造的部分

以下能力已经存在，直接整合，不建立第二套模型：

- `collection_observation_target`：长期观察身份；
- `request → authorization → admission → work_order`：新增平台访问的控制链；
- `execution_station / plugin_installation`：稳定工位与可替换安装；
- `collection_work_order_lease / lease_task`：有限执行权和顺序步骤；
- `linggan_runtime_task / attempt / capture_package / submission_receipt`：插件执行与不可变回传；
- `linggan_material_*`：作品、作者、评论、媒体的类型化材料；
- `/api/local/evidence-library` 与 `/corpus/evidence`：新材料的唯一默认读取入口；
- 内容工作台中已经验证的浏览器页面读取器、窗口关闭、断点与 Outbox 行为。

不引入 `Control Plane`、`Observation Engine`、`Information Need` 等新的物理服务或表。它们若只是在解释责任，就停留在产品语言；本次执行链只使用已存在的目标、规则、工单、步骤、租约、尝试、包、回执和材料。

## 3. 唯一对象层级

| 对象 | 唯一责任 | 明确不负责 |
|---|---|---|
| 观察目标 `ObservationTarget` | 长期说明观察谁或什么关键词 | 不表示已授权、已派发或已采集 |
| 观察规则 | 是否启用、多久观察、单轮上限与停止边界 | 不保存某次执行状态 |
| 采集申请与准入 | 说明为什么需要新访问，并检查授权、重复、产能与风险 | 不访问平台 |
| 工作单 `WorkOrder` | 冻结一轮有限范围 | 不随插件执行动态扩大 |
| 步骤 `Step` | 一个插件可以独立完成的一项能力 | 不代表执行成功 |
| 租约 `Lease` | 某台工位在有限时间内拥有执行权 | 不等于 Attempt 或 Package |
| 尝试 `Attempt` | 一次真实执行 | 不覆盖之前的失败尝试 |
| 数据包 `Package` | 一次尝试取得的不可变结果与 Coverage | 不因任务部分失败而丢弃合格成员 |
| 回执 `Receipt` | 服务端是否接纳这一个包 | 不自动证明所有规则已完成 |
| 材料 `Material` | 可追溯、可检索、按类型读取的研究材料 | 不自动升级为洞察或现实总体结论 |

稳定工位和插件安装必须分开。插件升级或 Service Worker 重启不创造新工位；旧安装失效也不得让它推进新安装已经接管的控制状态。

## 4. 目标与规则执行

### 4.1 创作者目标

启用规则后：

1. `pending_decision` 且没有在途基线：worker 申请一次 `deep_archive`；
2. 基线顺序执行 `author_profile → profile_discovery`；
3. 两步 Package 均接纳后，目标进入 `monitoring`（规则仍开）或 `archived`（规则已关）；
4. 后续按 `patrol_interval_seconds` 产生 `profile_discovery` 巡检；
5. 新发现作品直接形成作品材料。发现卡上的标题、作者、发布时间文本、互动字段和封面都属于本轮实际观察，不要求为了显示封面再访问详情页。

`maximumQuota=200` 是访问上限，不是平台作品总数。只看到 32 条时保存 32 条，并保留“当前表面是否耗尽/未知”的 Coverage。

### 4.2 关键词目标

启用规则后，worker 按同一套到期、准入、工位和风险边界生成 `discovery_search`。插件自动打开带规范化查询词的搜索页、读取有限表面、回传 Package。关键词与创作者共享执行内核，但不共享目标身份或基线含义。

### 4.3 不自动扩张的边界

本次不让发现结果无限生成详情、评论与媒体字节任务。第一阶段自动闭环所需的“标题、封面、作者、互动和来源”已经存在于搜索/博主页发现记录中；它们应被正确类型化和展示。

详情、评论、媒体槽和媒体字节能力继续保留，供明确批准的深采 Work Order 使用。自动扩张到逐篇详情、评论或原件必须由后续规则明确给出数量、用途和停止条件，不能由插件看见一篇作品后自行决定。

## 5. 调度、领取与恢复

### 5.1 服务端

- API 不运行平台采集；worker 每 60 秒扫描一次规则；
- worker 每轮写入可读取的心跳和结果摘要；“没有到期目标”是成功空转，不是未接通；
- 同一目标、同一 lane 同时最多一个活租约；数据库约束和事务负责防重；
- 派发前重新检查授权、风险暂停、工位能力和当日额度；
- 深度建档完成后由 Package/Receipt 同一事务推动目标生命周期，不靠页面猜测；
- 过期租约被明确收回。尚未完成的基线最多形成 3 张执行工作单；超过后进入人工关注，不继续自动重试；
- 有多个合格工位时，恢复优先避开上一张过期工作单使用的工位；没有替代工位时才允许同工位下一次尝试。

### 5.2 插件

- MV3 只使用 `chrome.alarms` 和浏览器事件唤醒，不依赖 `setInterval` 常驻；
- 安装、启动、Service Worker 被唤醒后先签到，再立即领取一次；
- 每次服务端回答给出下一次轮询时间，插件只执行 `mayExecute=true` 的任务；
- 领取到任务后在不抢焦点窗口执行，完成或失败都关闭窗口；
- 成功 Package 进入本地 durable Outbox，网络重投使用同一 submission identity，不重新访问平台；
- 插件重载后，稳定 `installKey` 继续领取服务端返回的同一在途任务；新安装不能冒充旧安装的执行权；
- popup 的“领取任务”只保留为诊断按钮，不是正常运行前置条件。

### 5.3 重试域

“三次”只约束平台执行尝试，不把所有重投混成一个计数。

| 失败域 | 动作 | 是否消耗三次平台尝试 |
|---|---|---|
| Receipt 响应丢失、网络未知 | 同一个 Outbox envelope 重投 | 否 |
| Service Worker 休眠、页面窗口意外关闭 | 同一步骤恢复或产生下一次 Attempt | 是 |
| 工位/浏览器故障 | 有替代工位时换工位；否则同工位有限重试 | 是 |
| 平台风控或账号冷却 | 停止当前访问，记录风险暂停/冷却，不立即撞击 | 是 |
| 内容不存在、权限明确拒绝 | 终态失败，不重复 | 是，且立即停止该步骤 |
| Package 部分成功 | 接纳合格成员，缺口另记；不重采已取得成员 | 已发生的一次照常计数 |

租约失效后的迟到提交必须拆成两维回执：`execution_effect=LOST_AUTHORITY`，因此不能完成 Step、释放新租约或覆盖新执行者；`material_admission=ACCEPTED`，因此已经真实观察且通过合同校验的数据仍进入 Package、逐条处置与材料投影。控制权失效不能被误写成数据不存在。

## 6. PostgreSQL 规格

只做 expand-only migration：

1. 增加 `collection_scheduler_heartbeat` 当前运行投影，保存 worker 实例、启动时间、最近 tick 开始/结束、结果、派发数、跳过数和错误；
2. `linggan_material_discovery_finding` 增加封面来源 URL 与 `KNOWN/UNKNOWN` 状态。旧行保持 `UNKNOWN`，不扫描历史 payload 回填；
3. `linggan_runtime_submission_receipt` 增加不可变的 `execution_effect` 与 `material_admission` 两轴；旧回执保持 `UNKNOWN`，不猜测历史执行权；
4. 基线重试次数只从 `collection_work_order` 与 lease 历史计算，不给不可变 Package/Receipt 增加可变计数；
5. migration 进入 `scripts/local-runtime.sh` 的顺序账本，重复运行按 hash 拒绝漂移；
6. 不删除旧表、不更新历史 Package、不把旧 90 张卡迁入新 Material Projection。

封面 URL 是“来源当时观察到的远程候选”，不是已取得字节。它不得写入 `local_asset_url`，也不得被 UI 标成“本地副本可用”。

## 7. API 与读取合同

### `/health`

除现有数据库、station、dispatch、producer 路由外，增加：

```json
{
  "scheduler": {
    "state": "running | stale | unknown",
    "lastTickCompletedAt": "timestamp | null",
    "lastOutcome": "idle | dispatched | partial | failed | unknown"
  }
}
```

超过 3 个扫描周期没有完成心跳才是 `stale`。没有目标或没有任务仍是 `running + idle`。

### Evidence Library

列表项必须提供：

- 稳定作品引用、平台作品 ID；
- 标题与状态；
- 作者显示名与状态；
- 发布时间原始文本与资格状态；
- 最近观察时间；
- discovery/detail/comments/media 等 lane 状态；
- 封面：`localAssetUrl`（已验证本地字节）优先，否则 `observedSourceUrl`（受限平台来源候选）；
- Package/Record/Coverage 来源引用。

页面只有在 `localAssetUrl` 存在时才说“本地副本可用”；只有 `observedSourceUrl` 时显示真实封面并标“来源封面，未物化”。没有任何 URL 时显示未知状态，不能画占位图暗示已取得。

## 8. Browser Producer 版本与发布

本交付升级次版本到 `0.6.0`，原因是正常运行语义从“可手动领取”变为“安装归位后自动领取”，并新增搜索目标自动执行和封面材料对齐。发布必须按固定顺序：

1. `npm test`；
2. `npm run build`；
3. `npm run package:release`；
4. `npm run release:manifest`；
5. `npm run release:verify`；
6. `npm run release:reproducibility`。

源码、`manifest.json`、`package.json`、release manifest 与 ZIP 必须同版；测试通过但 ZIP 仍是旧代码，视为未发布。

## 9. 实现顺序

1. 整合媒体合同、Material Projection、Evidence Runtime 和当前 `main`；
2. migration 与 scheduler heartbeat；
3. worker 的基线/巡检状态推进与最多三次工作单恢复；
4. 插件签到后立即自动领取，并补关键词目标执行；
5. discovery 封面类型化与 Evidence 图片呈现；
6. 插件版本、发布包和运维入口；
7. 一次冻结验证；
8. 合并 exact head 到 `main`，切换本机 runtime，做一次真实链验收。

## 10. 唯一验证清单

自动验证只运行这一套：

- Rust：`cargo fmt --check`、`cargo test --workspace --locked`、PostgreSQL 集成测试；
- 并发/恢复：同一任务并发领取最多一个；同安装重取返回同一任务；过期基线最多 3 张工作单；完成后生命周期正确；
- 插件：单元测试、构建、release verify、reproducibility；
- UI：Material JSON 的标题/封面状态正确；DOM 真正渲染 `<img>`，远程来源与本地副本文案不混淆；
- 治理：`./scripts/check-project-governance.sh` 与 `git diff --check`；
- 运行：API/worker 常驻、`/health.scheduler=running`；插件不点领取也取得任务；新 Receipt 和 Evidence item 可回查。

失败时只集中修复这份清单中已经确认的失败项，再重跑对应项；不增加新的架构评审卡、压力测试门槛或与本交付无关的重构。

## 11. 停止与非目标

出现授权过期、账号未登录、平台安全验证、无合格工位、数据库 migration hash 冲突或真实目标身份不足时停止真实访问并保留已有事实，不换账号撞击、不删除记录。

本次不实现 OCR/ASR provider、通用工作流引擎、微服务、Kafka/Temporal、历史卡片回填、云生产部署、自动扩大研究问题、自动逐篇深采全部详情评论或旧内容工作台修复。
