# 评论理解合同（Comment Understanding V2）

> 状态: 代码事实优先
> 最后核对: 2026-11-19
> 适用范围: `comment-understanding.v1`、`comment-problem-consolidation.v1`、研究单元、需求身份与派生状态（迁移 `0230`）
> 事实来源: `database/migrations/0230_comment_understanding.sql`、`crates/intelligence/src/ai_foundation/comment_understanding/`、[实施计划](../plans/active/comment-understanding-v2-001.md)
> 冲突时以谁为准: 当前 migration/代码/隔离 PostgreSQL 测试；语义标准以计划第 3 节为准，提示词是它的实现

本合同描述 G1 已落地的结构。旧的评论三阶段（`comment-research-semantic.v1`、`comment-problem-stage.v1`）仍是运行中的流程，直到计划的切换步骤才退役；届时[研究自动化执行合同](research-automation.md)中评论部分同步改写。

## 1. 一次调用与一个研究单元

一个研究单元是同一作品的若干条评论，对应**一个 batch item、一次模型调用**。目标（评论）仍是各自独立的 `linggan_comment_research_request`；单元只固定共享的 L1 上下文和模型看到的需求目录。

| 表 | 含义 |
|---|---|
| `linggan_comment_research_unit` | `unit_ref`、`item_ref`、`domain_ref`、`work_ref`、`l1_request_ref` |
| `linggan_comment_research_unit_target` | 单元内的评论：`request_ref` 唯一，`position` 从 1 起 |
| `linggan_comment_research_unit_dispatch` | 派发时的 `invocation_ref` 与 `catalog_problem_refs`（有序，别名 `p<n>` 即第 n 个） |

数据库保证：L1 请求与单元同作品、同领域；目标是同作品、同领域、同批次的 `comment-understanding.v1` 请求。`request` 的批次、命令和 `result` 的调用不再唯一，一次调用的结果按评论各写一行。

## 2. `comment-understanding.v1`

模型端载荷（`payload()`）：`contract`、`workContext`、`catalog`（`ref` 为 `p1…`，含 `label/actor/goal/barrier/context`）、`targets`（`targetKey` 为 `t1…`，含 `researchText` 与 `parentContext`），任务指令放在 `instruction`。模型看不到任何 UUID。

响应（Schema 由 `response_schema()` 生成）：`newProblems[]`（`newKey/label/actor/goal/barrier/context`）与 `targets[]`（`targetKey/outcome/reason/signals[]`）；信号为 `kind/proposition/evidence/problem`，`problem` 为 null 或 `{ref,newKey}` 恰好一个非 null。

接纳（`accept_unit()`）按最小单位降级：

| 缺陷 | 后果 |
|---|---|
| 不是合法 JSON、合同名不符、顶层字段缺失或多余 | 整个单元失败：`unit_output_invalid` |
| 目标缺失 / 重复 / 字段不合法 / outcome 与 signals 不一致 | 该目标失败：`target_missing` / `target_invalid` |
| 所有信号都被丢弃 | 该目标失败：`target_all_signals_invalid` |
| 证据在**本条评论清洗后文本**里不是唯一连续片段；kind 非法；同 kind 同位置重复 | 丢弃该信号，计入 `droppedSignals` |
| 归属引用目录外的 `pN`、未定义或被定义两次的 `newKey` | 信号保留，去掉归属，`eligibilityState = deferred_context` |
| 单条目标超过 12 条信号 | 只检查前 12 条，其余计入 `droppedSignals` |

没有被信号引用的 `newKey` 不建需求。输出被截断（`unit_truncated`）由调用方依据 finish_reason 判定，不在接纳器内。

## 3. 存储的单条结果

写入 `linggan_comment_research_result.result`，合同名 `comment-study.semantic.v2`，沿用 v1 全部字段，Topic Map、评论目录等下游读取不变：

`outcome`、`reason`、`signals[]`（`kind`、`proposition`、`sourceRef`、`evidenceSpan{start,end}`、`problemFrame`（恒为 null）、`eligibilityState`、`eligibilityReason`、新增 `problemRef`）、`droppedSignals`。

信号序号是它在 `signals[]` 中的下标，对应 `linggan_comment_problem_membership.signal_ordinal`（0–11）。

## 4. 需求身份与派生状态

需求（Problem）是稳定身份：`linggan_comment_problem_definition.problem_ref` 唯一，V2 不修改定义。**候选/正式不存储**，由视图实时推导；合并只追加重定向，旧定义与支持保留。

| 视图 | 含义 |
|---|---|
| `linggan_comment_problem_active_redirect` | 未被撤销的重定向 |
| `linggan_comment_problem_resolution` | 每个需求 → 当前活的需求 |
| `linggan_comment_current_research` | 每条评论（领域、作品、评论 ID）当前算数的研究：最新一条有成功结果的 V2 请求；已被限制的评论不算；更新的失败请求不抹掉旧成功 |
| `linggan_comment_problem_support` | 经重定向解析后的有效支持 |
| `linggan_comment_problem_current` | 每个活的需求一行：`state`、`signal_count`、`comment_count`、`author_count`、`work_count`、`last_researched_at` |

状态：`author_count ≥ 2` 为 `formal`，否则 `candidate`。作者按（平台，作者 ID）去重；作者为空的评论计入评论数，不计入作者数。**正式化不代表市场规模、普遍性或商业价值。**

重定向写入由触发器校验：同领域；`from` 与 `to` 都不能已有生效的重定向；按领域咨询锁串行化。因此图中不会成环。撤销写入 `linggan_comment_problem_redirect_revocation`。

## 5. `comment-problem-consolidation.v1`

模型对程序提出的需求配对给出 `same` / `different` / `narrower`，只有 `same` 可成为重定向；存活方由程序按"正式优先、作者多优先、创建早优先"决定，模型不选。没被问到的、重复的、格式不对的配对不产生任何决定。整理任务和它的派发表随计划 G3 注册；本阶段只有合同和接纳函数 `accept_consolidation()`。

## 6. 任务与失败码

`comment_understanding` 已注册：生命周期 `active`，开关关闭，预算 `unconfigured`，未绑定模型，提示词 v1 为草案；批次只允许 `interactive`，外发材料类别为 `note_material` 与 `comment_material`。**G1 阶段它不能入队**，入队接线属于计划 G2。

失败码：`unit_output_invalid`、`unit_truncated`、`target_missing`、`target_invalid`、`target_all_signals_invalid`。
