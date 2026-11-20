# 评论理解合同（Comment Understanding V2）

> 状态: 代码事实优先
> 最后核对: 2026-11-19（G1–G3 与 G5）
> 适用范围: `comment-understanding.v1`、`comment-problem-consolidation.v1`、研究单元、需求身份与派生状态（迁移 `0230`、`0231`）
> 事实来源: `database/migrations/0230_comment_understanding.sql`、`crates/intelligence/src/ai_foundation/comment_understanding/`、[实施计划](../plans/active/comment-understanding-v2-001.md)
> 冲突时以谁为准: 当前 migration/代码/隔离 PostgreSQL 测试；语义标准以计划第 3 节为准，提示词是它的实现

本合同描述 G1–G3 与 G5 的结构和流程。旧的评论三阶段（`comment-research-semantic.v1`、`comment-problem-stage.v1`）仍登记在当前运行版本，按计划在切换阶段退役；届时[研究自动化执行合同](research-automation.md)中评论部分同步改写。

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

模型对程序提出的需求配对给出 `same` / `different` / `narrower`，只有 `same` 可成为重定向；存活方由程序按"正式优先、作者多优先、创建早优先"决定，模型不选。没被问到的、重复的、格式不对的配对不产生任何决定。`0231` 注册 `comment_problem_consolidation`，支持 `automatic` 与 `interactive`。它初始停用、未配置预算、未绑定模型。派发前校验 `comment_material` 许可。同领域的整理和研究单元串行领取。

自动整理以研究批次为单位。全部单元终止且本次新建过候选时，最多创建一个自动整理批次。派发时读取完整 V2 目录，上限 150 个候选、200 个需求。至少一方为候选的需求配对使用 `q<a>-<b>`（`a<b`），派发记录冻结每个别名对应的两个需求。没有可比较的需求或超过上限时跳过，且无模型调用。请求载荷为需求定义、状态和短别名。响应允许省略无结论的配对。

写重定向时重新检查两方仍是当前需求。`revoke_redirect` 校验领域，在同一事务追加撤销和命令回执。同一命令重放返回原回执，重复撤销不新增记录。

## 6. 任务与失败码

`comment_understanding` 已注册：生命周期 `active`，开关关闭，预算 `unconfigured`，未绑定模型，提示词 v1 为草案；批次只允许 `interactive`，外发材料类别为 `note_material` 与 `comment_material`，两类许可都要有才能派发。入队只通过 `comment_understanding::start`，不能走通用的批次创建。

失败码：`unit_output_invalid`、`unit_truncated`、`target_missing`、`target_invalid`、`target_all_signals_invalid`（已在迁移中注册）；单元在调用前被跳过时写在条目上的 `comment_l1_unavailable`、`comment_catalog_too_large`、`ai_source_unavailable`；目标在结算时评论已不可读的 `comment_publication_withheld`。

## 7. 规划与启动（T0）

`preview(domain, selections, rerun)` 与 `start(Start)` 共用一个规划函数。选择项是 `(workRef, sourceRef, observationRole)`，一次最多 1000 条。

- 资格：V1 来源门，差别是不读研究底稿、允许评论者作者未知；能确认来自创作者本人的评论排除，作品作者或评论者身份未知仍可研究。作品必须有可用的 L1 Core（`core_cards_in` 的第一张，其 `analysisRef` 即 L1 请求 ID），否则计入预览的 `waitingL1`。
- 不进计划：已有成功结果（除非 `rerun`）、已在排队或运行、上一次调用结果未知（`ai_dispatch_unknown`）。
- 切分：按作品分组，每单元最多 `UNIT_MAX_TARGETS`（20）条且不超过输出估算上限；之前因输出被截断失败的评论，每次失败容量减半，最小为 1；作品评论多的先执行。
- `planHash` 不含随机单元 ID；启动重算并比较，不一致为冲突。启动在同一事务里写一个批次、每单元一个条目和 `unit` 行、每条评论一行 `request` 与 `unit_target`，按领域加咨询锁避免两个启动规划同一批评论。命令幂等。

预览返回：`planHash`、`knowledgeCutoff`、`taskConfigRef`、`taskRevision`、`selectedCount`、`researchableCount`、`unitCount`、`expectedModelCalls`、`workCount`、`waitingL1`、`alreadyResearchedCount`、`alreadyQueuedCount`、`dispatchUnresolvedCount`、`excludedCounts`、`units`。

## 8. 执行（T1、T2）

同一领域同一时间只执行一个单元（领取条件，`executor/comment_understanding_order.sql`）。

准备（一个事务）：核对任务配置与外发许可、单元与请求属于本次批次；逐条重读评论，不可读的写入失败结果；读冻结的 L1 卡并投影为作品语境；读需求目录（超过 200 条则跳过整个单元）；组装载荷并创建调用；**在调用之前**写入 `unit_dispatch`（有序目录）。

结算（一个事务）：重读评论，调用期间被撤回的评论写 `comment_publication_withheld`；识别截断（适配器的 `output_limit` 失败或 `finish_reason=length`）为 `unit_truncated`；否则用 `accept_unit` 接纳，顶层不可用为 `unit_output_invalid`；成功则 `publish_in` 写入每条评论的结果、新需求（每个被引用的 `newKey` 一个）和支持关系，失败则给每条已发送的评论写失败结果。调用账本、批次条目状态在同一事务里结算。

## 9. 运行回执与取消

`read_run(domain, batch)`：每个单元的状态与失败码、目标按状态和失败码的计数、模型调用数与输入输出 Token（来自调用账本）。`cancel`：停止尚未开始的单元；已在调用中的单元正常结算。

## 10. 下游读取

`comment_research::current_source_in(request)` 是 Topic Map、Topic 评论来源和评论概览读取"某条研究对应的评论"的唯一入口：V1 研究按冻结输入复核；V2 研究只要求评论仍可读，L1 与清洗版本不作为有效性条件。三处 SQL 以"该请求有成功结果行"判断成功，不看同批次其他条目的状态。评论目录的"最近一次研究"状态对 V2 请求从单元条目与结果行推导。

Opportunity 从有效支持视图读取 V2 归属并保留原始支持键。当前指标和原声排除已失效或已移到另一需求的提及。研究单元和整理成功后请求刷新，撤销由现有周期刷新发现。机会卡仍按自身的分期、来源和作者资格计算指标。


## 11. 本机 V2 HTTP 入口（G5）

以下路径以 `/api/local/comment-research/v2` 为前缀，沿用本机同源访问与禁止缓存约束。所有写命令使用命令编号，回执表示提交或取消请求已受理。

| 方法与路径 | 事实和动作 |
|---|---|
| `POST /preview`、`POST /runs` | 批量选择预览与原计划启动。运营可从预览为缺 L1 的作品调用现有内容理解预览和提交。 |
| `GET /runs`、`GET /runs/{batch}`、`POST /cancel` | 领域内最近运行、单元和评论回执、失败重选项、用量和取消。 |
| `GET /latest`、`GET /overview` | 评论最近执行和当前可展示结果分开读取。概览展示当前 V2 信号、正式需求与候选数量。 |
| `GET /problems`、`GET /problems/{problem}` | 当前需求列表、作者与作品计数、最多三段代表原声、重定向和撤销历史。 |
| `POST /consolidation`、`POST /redirects/revoke` | 手动提交候选整理，在当前领域撤销已发生的合并。 |

列表和详情中的需求状态均由有效支持实时推导。展示代表原声前重检来源资格，再按已保存信号的字符范围取原文。V1 HTTP 保留到后续退役阶段。
