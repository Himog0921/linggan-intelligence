# 现役 V2 架构全景

> 状态: 代码事实优先
> 最后核对: 2026-08-15
> 适用范围: 固定 V2 源码快照的参考架构
> 事实来源: 固定点源码、fixture、测试与 provenance
> 冲突时以谁为准: 固定点真实源码和可复现测试；不代表新 Rust 实现状态

本文件描述固定点 `content-workbench@2dca6cb9` 与 `linggan-boom@60a896c1`。它是新项目的参考基线，不代表 Rust 版本已经实现。

## 1. 插件执行面

插件 v2.0.95 是 Chrome MV3 扩展。现役运行能力包括：

- 工位注册、插件授权和心跳；
- 服务端任务领取与 lease；
- IndexedDB 本地 run/outbox/capture journal；
- 重试、恢复导出、死信和幂等回执；
- XHS 页面识别、详情、列表、作者、评论采集；
- producer 终态、slotReports 和 captureCounters；
- CaptureSubmissionV2 映射与服务端签名提交。

六个 XHS CollectionProfile：`list_scan`、`note_detail`、`note_full`、`comment_probe`、`author_profile`、`author_links`。

关键参考：

- `references/current-v2/plugin/source/src/workbench/runtime/`
- `references/current-v2/plugin/source/src/workbench/protocol/v2/`
- `references/current-v2/plugin/source/src/platforms/xhs/`
- `references/current-v2/plugin/source/src/db/`

## 2. 服务端入口

现役三类入口：execution、manual import、recovery import。边界负责认证/授权、请求身份、运行时校验和调用统一 EvidenceIngress。

关键参考：

- `workbench/source/src/app/api/v2/evidence/execution/`
- `workbench/source/src/app/api/execution-tasks/manual-import/`
- `workbench/source/src/app/api/execution-tasks/recovery-import/`
- `workbench/source/src/lib/evidence/adapters/`

## 3. Evidence 层

核心数据对象：CapturePackage、CaptureArtifact、EvidenceIngressReceipt、RawSnapshot、RawRecord。入口执行 canonical JSON、内容 hash、包级幂等、身份冲突拒绝和 Evidence 追加写入。

CapturePackage 是边界包；RawSnapshot/RawRecord 是旧系统中承载 V2 Evidence 的现役物理对象。新 Rust 数据库不必保留这些旧物理名称，但必须忠实保留已证明的不变量。

关键参考：`workbench/source/src/lib/evidence/ingress/`。

## 4. 数据库安全

现役设计分离 evidence writer、contract reader、canonical writer、default app、preflight reader 等数据库身份；原始 Evidence 通过 SECURITY DEFINER 受控读取并写访问审计。

核心对象：EvidenceAccessAudit、EvidenceReaderWorkspaceGrant。跨 workspace、撤权、敏感列直读、直接 UPDATE/DELETE 应由 PostgreSQL 拒绝。

关键参考：

- `workbench/source/src/lib/evidence/security/`
- `workbench/source/prisma/migrations/20260812170000_v2_evidence_security_hard_cut/`

## 5. Durable Work

Evidence 提交后生成持久 work。worker 负责 claim、lease、重试、接管和故障分类；B2/B3 提交需要数据库 fencing，避免失去 lease 的旧 worker 继续写事实。

关键参考：`workbench/source/src/lib/evidence/worker/`。

## 6. B2 Derived / Contract Evaluation

B2 从受审计 Evidence bytes 读取事实，执行来源校验和 XHS normalization，形成 NormalizationRun、ContractEvaluation、ContractEvaluationInput 和 current evaluation。

- accepted 表示合同候选可用；
- rejected 可以推进 evaluation current；
- rejected 不得推进新的 Content observation/current pointer，并应隔离旧可见 Projection。

关键参考：`workbench/source/src/lib/evidence/derived/`。

## 7. B3 Canonical / Projection

B3 将 accepted evaluation 形成 CanonicalObservation、ContentObservation 和 ContentCurrentProjection。它处理 replay、迟到 observation、同时间歧义、accepted A→B 先隔离、rejected 撤销、跨 snapshot 并发与直接 SQL 绕过。

Current Projection 是产品可读视图，不是 Evidence 本身；原文 URL 只能来自当前 observation 已验证字段。

关键参考：`workbench/source/src/lib/evidence/projection/`。

## 8. Media

现役合同已经区分 CanonicalMediaSlot、MediaItem、MediaOrigin、ContentMediaUsage 和 OutboxEvent。同一个物理资源可对应封面与正文等多个 slot。

当前最重要缺口：真实 producer 的 media inventory、coverage 与 absent/unavailable 事实尚未形成稳定全量生产闭环。新项目不能仅因 candidate 缺失就声称媒体不存在。

关键参考：`workbench/source/src/lib/evidence/media/`。

## 9. 当前产品读取

Content-only Material 读取已经有 V2 Projection provider 参考实现。旧系统其它 Author、Comment、Metric、Topic、Monitor 页面仍属于历史产品世界，不能因 Content slice 跑通就宣称全域 V2 完成。

## 10. 能力状态

| 能力 | 当前状态 | Rust 项目处理 |
|---|---|---|
| 插件运行、租约、outbox | 生产验证 | 保留行为，后续适配新合同 |
| XHS Content Evidence | 已运行 | 首个移植对象 |
| B2 Content evaluation | 已运行 | fixture 对照重写 |
| B3 Content Projection | 已运行 | 数据库不变量对照重写 |
| 数据库角色与审计 | 已实现 | 用新 baseline 重新设计并攻击测试 |
| Observed Media | 合同较完整、生产覆盖不足 | P0 补齐 producer |
| Author Observation | 未完成 | 新领域设计 |
| Comment Observation | 未完成 | 新领域设计 |
| Metric Observation | 未完成 | 新领域设计 |
| Observation Plan | 未实现 | 新核心能力 |
| Coverage/缺失事实 | 部分来源 | 先审计再建模 |
| Feature/Cluster/Signal | 未实现 | 新情报内核 |
| Intelligence/Outcome | 未实现 | 新产品核心 |
