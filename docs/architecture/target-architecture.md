# 目标架构

> 状态: 权威当前
> 最后核对: 2026-08-15
> 适用范围: Linggan Intelligence 目标模块与数据边界
> 事实来源: ACCEPTED ADR 与已确认架构原则
> 冲突时以谁为准: ACCEPTED ADR；实现状态以真实代码和运行结果为准

## 结构

```text
Browser Extension
  → Capture Contract
  → Evidence Ingress
  → Immutable Evidence Store
  → Observation Builder
  → Temporal Change Detector
  → Feature / Cluster
  → Signal
  → Intelligence Event
  → Action / Outcome
```

采用模块化单体：一个 Rust workspace、一个 PostgreSQL 主数据库、独立 API/worker 进程。对象存储只保存媒体二进制，数据库保存身份、来源、checksum 和状态。

## 领域层

### Capture

Observation Target、Observation Plan、Station、Run、CapturePackage、Coverage。

### Evidence

外部世界在特定时间被观察到的原始事实。追加、版本化、不可静默覆盖。

### Observation

由合同验证后可以确认的领域事实。Observation 不能把采集缺失解释为现实不存在。

### Intelligence

Feature、Cluster、Signal、Intelligence Event、Hypothesis、Counter-evidence、Outcome。

## 关键边界

- Capture 成功不等于 Observation 成功。
- Observation accepted 不等于现实对象 active。
- 同一物理媒体可以承担多个业务 slot。
- Evidence、Observation 与产品 Current Projection 分离。
- Agent 判断不是事实；它是带证据、版本和置信度的可撤销解释。

## Rust 移植策略

TypeScript 现役实现不是逐行翻译对象。先固定跨仓 fixture、canonical bytes、hash、SQL 不变量和攻击用例，再在 Rust 中重建边界。每完成一段，以相同 fixture 对照旧实现和 Rust 输出。

## 数据库设计时序

在完成插件字段审计、情报需求映射、Coverage 语义和媒体 producer 审计前，不创建业务 baseline。数据库从领域事实推导，不从页面列表倒推。
