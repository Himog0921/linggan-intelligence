# 设计变更清单：COLLECTION-SCHEDULER-SCALE-001 Runtime 调度规模投影

> 状态: 草案
> 最后核对: 2026-09-05
> 适用范围: `/collection/runtime` 的调度规模只读事实投影；不新增控制操作
> 事实来源: Mog 的当前授权、[COLLECTION-SCHEDULER-SCALE-001](../../plans/active/collection-scheduler-scale-001.md)、Collection Workspace、LIDS 与当前 Runtime 读模型源码
> 冲突时以谁为准: 用户最新确认、已接纳的采集控制合同、当前数据库/Runtime 事实；本清单不授权真实采集、共享 Runtime 变更或插件动作

## 1. 变更目的

在既有 `/collection/runtime` 中补足调度规模的只读事实，使操作者在不创建规则、不会触发
任何采集的情况下，能区分“Rule 已到期但等待容量”“WorkOrder 正在冷却重试”“平台并发
已满”“某工位正在执行什么”。

这不是新增 workspace、控制台、顶部导航或业务模块。`/collection/runtime` 仍是
Collection Workspace 的运行态子视图。

## 2. 信息和状态边界

| 区块 | 可展示的数据 | 不展示或不推断 |
| --- | --- | --- |
| 平台并发 | 平台名称、并发 cap、活跃 Lease、余量 | 平台风控原因、账号 Cookie、外部页面状态 |
| 通道队列 | lane、等待/租用/冷却计数、最老等待、lane cap | 任务正文、评论身份、采集结果内容 |
| 工位执行 | 工位名、是否接活、当前 lane/目标、开始/到期、预计工作单元 | 外部账号凭据、浏览器标签、用户行为 |
| Rule 排程 | 目标显示名、闭集间隔、上次计划、当前状态、下次计划 | 历史完整度对监控资格的错误推断 |

所有缺失字段必须显示为 `未知` / `暂无`，而不是 `0`、`正常` 或猜测的成功率。

## 3. 交互合同

- 该区块完全只读；没有“立即执行”“重试”“认领”“创建规则”按钮。
- 数字、时间和状态来自当前页面响应中的 Runtime read model，不直接从浏览器或外部平台
  拉取数据。
- 沿用现有桌面 Collection Workspace 的 surface、table、metric、badge 与空态模式；不为
  本次变更新建 token、字体、全局 CSS 或窄屏专用布局。
- 排序固定为：lane `immediate → scheduled → batch`；Rule 按下一次计划/最早等待排序；
  工位按当前执行优先、其后工位名排序。

## 4. 文案词典

| 机器事实 | 中文展示 |
| --- | --- |
| `platform_concurrency_reached` | 平台并发已满 |
| `retry_not_before_at > now` | 冷却重试中 |
| `queue_state = queued` | 等待领取 |
| `queue_state = leased` | 执行中 |
| `monitor_next_run_at <= now` | 已到期 |
| 无有效 Rule | 未设置观察规则 |
| 空队列 | 暂无等待工作 |

## 5. 验证

- 组件/路由测试确认 Runtime HTML 包含“平台并发”“调度通道”“规则排程”以及由 fixture
  产生的源数据；
- PostgreSQL 测试确认每个展示聚合均由持久 WorkOrder/Lease/Rule/Receipt/策略行推导；
- 不进行外部平台访问，不因 UI 验证创建 Rule 或触发真实 WorkOrder。
