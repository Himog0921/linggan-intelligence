# 当前系统资产盘点

盘点日期：2026-08-14（Asia/Shanghai）。

## 现役来源

| 系统 | 固定点 | 用途 |
|---|---|---|
| 内容工作台 | `2dca6cb9b08cd217d851aef845ea462c19289ef7` | V2 内核、数据库合同和集成证明的参考来源 |
| 浏览器插件 | `60a896c1def5062dbb8098e05b030c5a0871203b` | v2.0.95 真实运行能力与采集事实来源 |

## 已验证、应保留的能力

插件侧：工位注册、授权、心跳、任务领取、租约、本地队列、重试、死信、幂等提交、终态报告、XHS 页面采集与生产合同。

服务端：CapturePackage、EvidenceIngressReceipt、RawSnapshot/RawRecord、运行时校验、canonical JSON/hash、受控 Evidence reader、访问审计、durable work、B2 evaluation、CanonicalObservation、B3 Content Projection、并发 fencing、原文链接和 observed media 合同。

## 不能继承为“已完成”的能力

- Author、Comment、Metric 的完整新领域模型。
- Observation Plan 与多周期观察。
- Coverage/缺失事实闭环。
- 真实 observed media 的稳定端到端生产覆盖。
- Feature、Cluster、Signal、Intelligence Event、Outcome。
- Agent 推理审计与结果反馈闭环。

## 旧系统规模与迁移结论

- 旧工作台目录约 75GB，其中 `storage/local-blob` 约 68GB。
- 旧 PostgreSQL 数据库约 5.5GB；现有压缩 dump 约 658MB。
- 旧 schema 约 166 个模型，累计约 153 个 migration。
- 历史数据不能反向伪造 CapturePackage、采集上下文、coverage 或 producer 状态。

因此，新项目只保留源代码参考、合同、测试和数据清单。旧数据库与媒体另行只读归档。
