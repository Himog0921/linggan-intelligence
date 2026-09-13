# 评论研究执行基础 V1 · 隔离证明

状态：PROVEN IN ISOLATION
日期：2026-09-13

范围：为未来、另行授权的评论研究执行保存最小的可追溯输入和结论边界。此包没有 HTTP 创建入口、worker、队列、provider、模型调用、Token、费用或真实研究结果。

## 不可混用的状态

`comment_research_run_v1` 与 `comment_research_run_item_v1` 只保存执行状态。V1 初始状态仅为：

- `prepared`：当前清洗表达为 `analyzable`，其 source-backed Context Pack 可冻结，未来执行器才可进一步入队；
- `blocked`：当前清洗表达为 `needs_context`。即使当前已采到一些上下文，也不能把预览文本擅自判为充分；冻结快照保留为审计材料，`initial_failure_code=needs_context_insufficient`，不能执行。

后续状态只能通过 append-only `comment_research_run_item_event_v1` 记录：`queued`、`running`、`completed`、`failed`、`timed_out`、`interrupted`、`awaiting_recovery`、`cancelled` 或再次 `blocked`。此包不写入任何事件。

`comment_analysis_v1` 与执行表分开，唯一允许的用户层结论是 `success` 或 `no_signal`。执行失败不是用户研究结论；`success` 也不是 RunItem state。`comment_analysis_v1(comment_derivation_id, research_fingerprint)` 的唯一约束为“同一有效语义输入可复用”提供可实现的存储边界，但本包没有执行器或查询选择器。

## 冻结与指纹

RunItem 固化：工作空间、创建时间、当前 Evidence locator、CommentObservation/Derivation identity、清洗表达与合同、研究合同、输出 schema、模型策略、fingerprint、Context Pack 版本、SHA-256、完整冻结文本、上下文充分性状态及初始可观察失败/输出校验状态。所有 Run、RunItem、事件和结论记录均由 PostgreSQL append-only trigger 保护；没有改动现有 User Voices DTO 或 API 投影。

`comment-research-fingerprint.v1` 使用 SHA-256，语义输入固定包括：

- 清洗后研究表达；
- 版本化、完整冻结的 Context Pack 文本；
- Cleaning Contract；
- Research Execution Contract V1；
- Comment Analysis Structured Output Schema V1；
- 模型策略 ID 与版本。

Observation、Derivation、Evidence 的 UUID 不进入 fingerprint 本身，因此它们不是唯一去重代理；它们仍随 RunItem 一起冻结，以便回溯来源。

## Structured Output Contract V1

`linggan-contracts` 的 `comment-analysis-structured-output.v1` 严格解析 JSON，拒绝未知字段。输出必须恰好匹配当前 RunItem 的 CommentObservation、CommentDerivation 和 fingerprint；`success` 至少有一个 finding，每个 finding 至少有一个 Unicode scalar 边界的 evidence span，且 span 的 substring 必须精确命中当前清洗研究表达。`no_signal` 是唯一另一种合法结论。合同不存在 discussion/work/context evidence 字段，因此回复与作品文字不能成为评论证据。

## 运行证明

```text
scripts/prove-comment-research-execution-foundation-v1.sh
```

脚本以随机容器、随机数据库和随机 loopback 端口运行 `postgres:16-alpine`。只向 ignored PostgreSQL integration test 提供临时 DSN；退出后删除容器，不读取共享 runtime 数据库。

已验证：

| 场景 | 结果 |
| --- | --- |
| 冻结输入 | 真实 Comment admission 后，RunItem 保存当前 locator、Observation/Derivation、fingerprint、snapshot version/hash/text 和 `prepared` 初始执行状态。 |
| 不可变 | 直接 `UPDATE frozen_context_pack_text` 被 PostgreSQL append-only trigger 拒绝。 |
| 指纹 | 同一有效输入相同；清洗研究表达或 Context Pack 内容改变时不同；Observation ID 不参与 hash。 |
| `needs_context` | 已采上下文不会自动升级语义；快照状态为 `insufficient_needs_context`，执行状态为 `blocked`。 |
| 输出证据 | 合法 Unicode direct span 通过；越界/不匹配的上下文文字、reference 不匹配与未知 `context_evidence` 字段被拒绝。 |
| 状态边界 | RunItem 为 `prepared` 时，可独立写入 `comment_analysis_v1.no_signal`；把 `success` 写成执行 event state 被数据库 CHECK 拒绝。 |

## 未证明、不得声称

- Run 创建 API、自动选择、跨 Run 重试、租约、调度、队列或 worker；
- provider/model 配置、Prompt/Skill、任何模型请求、Token、费用、超时或真实失败；
- 结构化输出的持久化 writer（未来 writer 必须先调用 V1 validator）、问题归并、向量、趋势或发布结果；
- 共享数据库 migration、main 合并、3000 runtime 或浏览器验收。
