# COLLECTION-SCHEDULER-SCALE-001：统一采集调度的规模化闭环

> 状态: 活跃计划
> 最后核对: 2026-09-05
> 适用范围: 仅隔离源码 worktree 内的采集控制、统一 WorkOrder 队列与 Runtime 只读运行态
> 事实来源: Mog 的当前授权、自动监测基线设计讨论、当前 Rust/PostgreSQL 源码与隔离测试
> 冲突时以谁为准: 用户最新确认、已接纳的产品合同、当前源码与真实运行事实；本计划不授权共享迁移、Runtime 切换、插件重载、创建规则、外部平台访问或真实采集

## 1. 要解决的事实

`COLLECTION-CONTROL-CLOSURE-001` 已经确立了“有效 Rule 到期后经由统一
WorkOrder 队列，由合资格工位领取 Lease”的主链路，也移除了历史建档完整度对持续
观察的门禁。

但当观察目标、工位和任务来源同时增加时，现有实现仍缺少四个必须由系统守住的边界：

1. 同一时间创建的大量规则会在同一时间到期，形成可避免的定时突发；
2. 批量深度建档只生成一小批后停止，无法按实际可执行容量持续蓄水，也不能按来源公平推进；
3. 站点与账号已有容量检查，但没有可串行判定的平台全局并发上限，失败重新入队也没有持久冷却；
4. Runtime 页面不能直接回答“哪个通道在等待、何时最老、平台还剩多少并发、规则是否已延迟”。

这不是新增一套“基线”“计划”或“模块队列”对象。它是对既有
`Rule → WorkOrder → Lease → RuntimeTask → Attempt → CapturePackage →
SubmissionReceipt` 执行链的规模化约束。

## 2. 冻结的产品规则

### 2.1 规则和排程

- 有效 Rule 是自动观察的唯一前置条件；历史档案的 `UNKNOWN` 或 `PARTIAL` 不阻塞
  Rule。
- 首次自动运行绝不绕过已选间隔。保存或恢复 Rule 时，系统为目标计算一个稳定、可见的
  间隔内相位；首次运行在“启用时间 + 一个完整间隔”之后的该相位，后续运行以已写入的
  `next_run_at` 为锚点推进。
- 同一 Rule 的一次计划时刻最多创建一个 WorkOrder；迟到时仅创建一张补偿单，并记录
  漏过的周期数，绝不补发 N 张历史单。
- 手动立即复采仍经过相同 WorkOrder/Lease 链路，但不改动 Rule 的下一次计划时间。

### 2.2 统一队列和公平性

- 所有浏览器执行工作仍在同一张 WorkOrder 队列中，按技术紧迫度划为 `immediate`、
  `scheduled`、`batch` 三个 lane；不按业务模块建队列。
- lane 之间使用一次加权公平选择；lane 内分别遵守即时 FIFO、定时最早到期、批量按
  `dispatch_group_key` 轮转的顺序。
- 批量工作按“合资格、可领取工位数 × 2”的上限滚动生成 ready WorkOrder，并受每轮生成
  上限保护；root 分页复用 Target 已持久的最近调度考虑时间，从最久未考虑的来源继续，不能
  让前 50 个 dossier 长期遮住后续来源。深度建档只是 batch lane 内的一类来源。

### 2.3 三层硬容量和恢复

- 一张 Lease 同时受工位、账号、平台三个硬上限约束；平台上限由可锁定的数据库策略行
  串行裁决，不能靠进程内计数。
- 浏览器在真正开始前遇到不可执行 locator，服务端必须释放刚取得的 Lease 并把工作单
  退回队列，不能留下只会等到超时的伪占用。
- 可重试的派发失败和过期 Lease 都保留原始 Attempt/Package/Receipt 历史，工作单通过
  持久 `retry_not_before_at` 延迟回队；退避按 60、120、240、480、900 秒封顶推进。

### 2.4 运行态可见性

Runtime 只读页面至少展示以下可由源数据证明的事实：

- 每个技术 lane 的等待、已租用、延迟重试、最老等待时刻、并发上限；
- 平台并发上限、当前活跃 Lease 和可用余量；
- 每个工位是否接活、当前 WorkOrder 的 lane/目标/开始时间/预计工作单元/Lease 到期；
- 有效 Rule 的上次计划、当前排队或租用状态、下次计划，以及到期和逾期汇总。

未被现有不可变账本精确定义的“成功率”不得用猜测数字填充；页面只展示可追溯的领取、
已接纳回执和工作单元事实。

## 3. 交付范围

| 层 | 本包修改 |
| --- | --- |
| PostgreSQL | `0037_collection_scheduler_scale.sql`：稳定相位、持久重试、批量组、lane 蓄水策略、平台并发策略及约束/索引 |
| Rust Evidence | 规则相位计算、批量滚动生成、派发公平/退避/locator 释放、平台并发再校验、Runtime 聚合读取 |
| 本地 API / UI | 将调度规模事实投影到既有 `/collection/runtime`，仅只读展示，不新增执行按钮 |
| 验证 | 更新全量 schema fixture；补 PostgreSQL 和 Rust 单元/集成测试，验证 slot、去重、batch 上限/轮转、平台上限、失败冷却与 Runtime 投影 |
| 文档 | 更新监控规则合同、当前状态、进度、设计变更清单和文档索引 |

## 4. 明确不做

- 不新建 baseline/readiness/recovery 生命周期或另一套业务对象；
- 不拆分 Kafka、RabbitMQ 或独立调度微服务；
- 不修改监控规则 Dialog 的用户配置表面：闭集间隔和固定 patrol 模板保持不变；
- 不迁移共享数据库，不切换本地 Runtime revision，不重新加载插件；
- 不创建任何观察规则、不领取真实 WorkOrder、不访问外部平台。

## 5. 验收条件

1. 执行同一时段保存的多个 Rule 时，数据库持久化其不同且确定的相位和下一次运行，
   任一 Rule 不会获得即时绕过；
2. scheduler 对同一 `rule_ref + scheduled_for` 仍只能创建一张 WorkOrder，迟到只产生一
   张并正确推进下次时刻；
3. batch 工作在可领取工位数变化时最多保持两倍 ready 容量，并跨 group 轮转，不造成
   无限排队；
4. 在平台 cap 下的并发 lease 尝试只有允许数量成功，额外尝试得到明确
   `platform_concurrency_reached`；
5. 派发失败和 locator 缺失不会留下 live Lease，重试在 `retry_not_before_at` 前不可领取；
6. `/collection/runtime` 的投影以数据库工作单、Lease、Rule、Receipt 和策略表为源，能
   表达上述排队和容量状态；
7. 所有新增行为通过格式化、编译和相应 PostgreSQL 隔离测试。共享库/Runtime/插件/业务
   结果另行授权和验收。
