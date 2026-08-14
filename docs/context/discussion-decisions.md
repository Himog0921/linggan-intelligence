# 讨论上下文与当前结论

本文件是给未来 Agent 的低分辨率入口。原始资料在 `references/`，不能只凭本摘要替代原件。

## 战略转向

旧产品以采集、内容管理和内容生产为中心；新项目以感知世界、理解变化、发现信号、形成情报和指导行动为中心。

情报的工作定义：

```text
情报 = 变化 + 证据 + 判断 + 行动价值
```

## 已确认决定

- 新建独立项目，不继续把所有新能力堆入旧内容工作台。
- 后端和 worker 使用 Rust。
- 使用全新 PostgreSQL 数据库，不带旧表。
- 现役插件不盲目重写；保留运行、租约、队列、重试、死信和页面采集能力。
- Evidence First、不可变 Evidence、版本化 Observation、无 fallback 继续有效。
- 历史数据不批量伪装为 V2；高价值数据重新采集。
- 当前所有 Linggan Intelligence 讨论稿与 V2 交付资料作为项目历史上下文归档。

## 讨论中提出、尚未物理确认

- Observation Plan 的最终字段和值域。
- Coverage 中 absent/unavailable/not_observed 的 producer 事实来源。
- Author、Comment、Metric Observation 的完整物理合同。
- 媒体 inventory、下载、对象存储、转码和分析的精确边界。
- Feature、Cluster、Signal 和 Intelligence Event 的评分与版本策略。
- Agent 模型供应商、成本门禁和人工修正流程。
- 第一批真实用户工作流和明确的 MVP 验收样本。

这些内容进入后续决策，不得由执行 Agent自行补齐。

## 已被实践证明的教训

- `success` 只能表示完整事实链完成，不能仅表示某个接口返回成功。
- TypeScript/Rust 类型不等于运行时合同。
- 测试必须主动攻击部分成功、并发、replay、跨 workspace、旧版本迟到和边界伪造。
- 同一个物理媒体可以对应多个业务 slot，不能因去重丢失业务语义。
- “采集成功”“合同接受”“Projection 可见”“现实 active”是不同事实。
- 上线证明系统不能无限扩张；在核心证据安全成立后，应尽早用真实小流量发现产品问题。
