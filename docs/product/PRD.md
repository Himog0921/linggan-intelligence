# Linggan Intelligence 产品需求文档

状态：Bootstrap，待用户逐阶段确认。

## 产品定义

Linggan Intelligence 是一个持续观察特定领域、保存可信证据、识别变化、形成可引用情报并记录行动结果的系统。

内容不是最终产品，而是观察现实世界的一种证据。

## 用户价值

用户最终需要回答：

- 最近发生了什么变化？
- 哪些变化值得关注？
- 证据是什么，有哪些反例和缺口？
- 这可能意味着什么？
- 下一步应观察、验证或行动什么？
- 之前的判断最终是否有效？

## 首个垂直切片（建议）

```text
XHS 内容/作者/评论/媒体
→ Evidence
→ Observation 时间线
→ Change
→ Signal
→ Intelligence Brief
→ 用户确认行动与结果
```

第一版必须包含：

1. 建立 Observation Target 与 Observation Plan。
2. 插件领取计划并形成版本化 CapturePackage。
3. 服务端保存不可变 Evidence，并记录 coverage 和采集终态。
4. 从 Evidence 形成 Content、Author、Comment、Media Observation。
5. 对同一对象的多次 Observation 计算明确变化。
6. 形成带证据、反例、缺失、不确定性和建议的 Intelligence Brief。
7. 用户可以接受、驳回、补证或发起下一次观察。
8. 行动结果可以回写，用于校准后续判断。

## 首期明确不做

- 全量历史数据迁移。
- 多平台同时首发；Douyin 在 XHS 闭环稳定后接入。
- 微服务、Kafka、图数据库。
- 全自动替用户做业务决定。
- 无证据的趋势预测。
- 旧内容生产、选题和飞书模块的整体复刻。

## 产品成功标准

- 每个 Intelligence Brief 的关键结论都能打开其 Evidence。
- 系统能明确区分“没观察到”“证明不存在”“无法访问”和“采集失败”。
- 同一对象跨时间的变化可以复算。
- 错误 Projection 可以从不可变 Evidence 重建，而无需重新抓取。
- Agent 结论包含反例、不确定性和规则/模型版本。
- 用户能从情报触发下一次观察或行动，并记录结果。
