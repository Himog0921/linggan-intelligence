# 评论研究运行准备 V1 · 隔离 HTTP / PostgreSQL 与页面确认合同证明

状态：PROVEN IN ISOLATION
日期：2026-09-13

范围：在一个明确的本地 `POST` 确认后，重新读取当前评论范围，冻结本轮输入并创建 append-only `Run`、`RunItem` 与初始事件，供未来另行授权的执行器读取。它不配置或调用模型，也不创建队列、worker、自动调度、Token、费用、向量或问题结果。

## 创建合同

```text
POST /api/v0/comment-research/runs
```

请求对象只接受 `workspace_id`、`scope`、`limit` 与可选 `preview` 摘要。`limit` 必须在 1–100；`preview` 只含已在浏览器预览显示的来源分布、当前接入时间和来源轮次，用来告知用户预览是否已过期。它不携带 Evidence ID、评论 ID、Derivation ID 或候选授权列表。未知字段（包括 `evidence_ids`）一律以 `400 invalid_request` 拒绝。用户原声页的确认流更严格，只发送前三个范围参数，不发送 `preview` 或任何候选清单。

创建事务内重新计算 `available` / `ready` / `needs_context` 当前范围，并以来源作品轮换排序：每个来源的第 1 条在其第 2 条前被考虑。浏览器预览永远不能覆盖这次查询。若摘要与这次查询不一致，响应 `scope_refreshed=true`，并仅返回最终的来源分布和聚合数量。

每一个重新选择的候选都在同一事务中组装 Context Pack V1、冻结文本和 SHA-256、计算 V1 fingerprint，并写入 RunItem。`analyzable` 写成 `prepared` RunItem 与 `prepared` 事件；这里的 prepared 只代表输入已经冻结，绝不代表已经入队、开始或完成研究。`needs_context` 同样冻结以保持阻断理由可追溯，但固定写成 `insufficient_needs_context` / `blocked` 和 `needs_context_insufficient`，不能交给未来执行器。

`0005_comment_research_run_preparation_v1.sql` 是前向 migration，只把 RunItem event 的允许初始状态补为 `prepared`，避免把没有队列的冻结输入伪称为 `queued`。它不修改历史 `0004`。

运行准备依赖已按顺序应用的 `0001` Comment Fact、`0002` Context Storage、`0003` Cleaning Derivation、`0004` Execution Foundation 和 `0005` Preparation 扩展。该依赖在隔离证明中显式执行；没有在共享数据库执行 migration。

## 重用和未完成策略

有效结论的判断是 `comment_derivation_id + current research_fingerprint`：当前 fingerprint 下已有 `success` 或 `no_signal` 时，创建器明确计入 `excluded_existing_conclusion_total`，不再冻结该条。已存在但没有有效结论的 RunItem 不是完成标志：`prepared`、`cancelled`、`interrupted`、暂态失败和未来的 `awaiting_recovery` 都不会被这个创建器静默排除。

同一新 Run 内出现完全相同的语义输入时，只冻结第一个按轮换顺序选中的项目，并报告 `deduplicated_semantic_input_total`。这消除重复输入，但不把历史未完成项目误判为已有结论。跨 Run 的暂态失败次数、合同型失败重开、实际租约和重试执行仍属于未来 worker 卡，尚未实现。

响应不会返回 Context Pack 文本、fingerprint、Derivation ID、Evidence ID、模型/Provider、Prompt、Token 或费用。可见状态只有 `prepared_for_execution` 或 `blocked_by_context`，不会称为「已研究」或「运行完成」。

## 隔离运行证明

```text
scripts/prove-comment-research-run-preparation-v1.sh
```

脚本创建随机命名的 `postgres:16-alpine` 容器、随机数据库和随机 loopback 端口，再通过真实 Axum Router 调用 HTTP。结束时移除容器。它不读取共享 runtime DSN、不占用 3000 端口，也不启动浏览器、worker、queue、provider、模型、向量服务或外部来源。

已验证：

| 场景 | 结果 |
| --- | --- |
| 冻结与初始状态 | 新 Run 持久化 2 个 `prepared` 输入、1 个 `blocked` 输入；每个都有完整 V1 snapshot、完整性 SHA-256、fingerprint 和 append-only 初始 event。 |
| 轮换与同义去重 | 两个来源 A/B 的首条先被选中；同一来源 A 的第二条与 A 首条语义相同，只冻结一次。 |
| `needs_context` | 「同问」被冻结为 `blocked`，初始 failure code 是 `needs_context_insufficient`；它不成为可执行项。 |
| 有效结论排除 | 测试直接写入一个已验证 `no_signal` 结论后，后续 POST 返回 `excluded_existing_conclusion_total=1`。这只验证选择器边界，不代表模型调用。 |
| 正文推进 | 当前评论正文更新后，旧浏览器摘要在 POST 时被刷新并返回 `scope_refreshed=true`；旧 Evidence locator 不再属于 Current。 |
| 客户端边界 | 带 `evidence_ids` 的请求为 400，所有 Run/Item/Event/Analysis 行数不变。 |
| 现有只读路径 | User Voices 和 plan preview 在创建后仍无 Run 表写入，且 DTO 没有 fingerprint、冻结 Context Pack 或模型策略字段。 |
| 页面确认合同 | 测试通过真实 Router 读取用户原声 HTML，静态断言抽屉含“准备本次研究”“确认并冻结输入”“待执行，尚未开始分析”与无执行器说明；提交函数只序列化 `workspace_id`、`scope`、`limit`，不含 `preview`、`evidence_id` 或 `evidence_ids`。 |
| 页面回执可显示 | 同一隔离 HTTP 流以这三个参数创建 Run，响应含 public `run_ref`、最终冻结数、待执行/阻断数和来源分布；页面合同要求以这些字段呈现“待执行，尚未开始分析”，不把它写回原声研究状态。 |
| 无模型执行 | 证明脚本没有 Provider、模型、queue 或 worker 依赖；页面文案与 API `execution_note` 均明确当前没有执行器、未调用模型。 |

## 未证明、不得声称

- 有头浏览器的视觉验收、3000 runtime 或业务验收；本证明只在真实 Axum 页面响应上验证静态页面合同，并以同一隔离 HTTP 请求验证确认体和回执。它不启动本地 Web 服务；
- 任何 Provider / 模型 / Prompt / Token / 费用、模型输出、真实研究结论或重试 worker；
- 队列、租约、自动调度、连续研究、向量、问题归并、共享数据库 migration、main 合并、3000 runtime 或业务验收。
