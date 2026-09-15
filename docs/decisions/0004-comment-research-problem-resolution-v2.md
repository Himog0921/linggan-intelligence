# DEC-0004 · 评论研究的稳定 Problem 归并合同

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2 的 Atom 准入、候选比较、Problem 创建、membership、执行历史和读取表达
> 事实来源: Mog 2026-09-15 的明确授权、DEC-0003、当前 `main@dab0be3`、Issue #285
> 冲突时以谁为准: 用户最新决定、`AGENTS.md`、真实代码/migration/运行证据

## 决定

`Problem` 是跨评论、跨作品、跨 Run 累计的稳定研究对象，不是每个 Atom 的改写标题。一个 eligible Atom 以有出处的 `subject / goal / observed issue / context` 描述其所观察到的困难或未满足需求；未观察到的根因保持未知。

模型只能在服务端冻结的输入中做两类受限语义工作：提取有来源的 Problem Frame，以及逐个比较候选的 actor、goal、issue、context、contradiction。模型不得返回数据库 ID、`MERGE`、`CREATE` 或其它业务写入动作。服务端验证 Schema、候选完整性、证据引用、领域与版本后，按确定性规则决定业务结论。

```text
eligible Atom
→ bounded existing-Problem / deferred-Atom recall
→ fixed-dimension comparison
→ program-owned decision
   ├─ assigned_existing
   ├─ deferred_novel
   ├─ deferred_ambiguous
   ├─ deferred_context
   ├─ not_user_problem
   └─ out_of_scope
→ only an independent eligible pair may create a Problem
```

`assigned_existing` requires exactly one equivalent candidate and no material unknown candidate. A candidate is equivalent only when `issue=same`, no contradiction exists, and actor/goal/context are same or compatible. A missing, duplicate, invented, cross-snapshot or invalid evidence reference is a contract failure, never a no-match.

`deferred_novel` means the evaluated candidate set has no match; it completes the current execution and creates no membership. It remains visible and is re-evaluated only when its relevant input, candidates, definition revision, context, contract, or explicit future Run budget changes.

A new Problem needs two eligible, equivalent signals from independent comments/authors, both with valid fresh no-match decisions, plus a minimally supported shared definition. The current implementation rejects the same source, same author, and an exact duplicate comment body; it does **not** yet identify semantic repost groups, so no unverified repost claim can be used as an admission guard. Creation and both memberships commit atomically after a catalog revision check. A changed catalog invalidates a stale creation proposal rather than allowing a duplicate.

## Consequences

- Existing `Run`, `Atom`, embedding, invocation ledger, membership, cross-Run execution history and ResultRevision responsibilities remain. This is a replacement of the decision contract, not a second research pipeline.
- Business conclusion and execution checkpoint remain separate. Waiting is a completed decision, while provider/schema failures remain execution failures.
- Cumulative confirmed membership remains distinct from the statistical publication denominator. Deferred signals are readable evidence but cannot enter Problem statistics, shares, ranks or changes.
- Problem definitions gain identity boundaries (definition, includes and excludes) and a versioned catalog guard; definition changes remain explicit revisions.

## Explicit non-decisions

This decision does not authorize historical Problem cleanup, real comment replay, model calls, scheduler enablement, shared migration application, runtime refresh, merge, deployment or business acceptance. Semantic accuracy must be evaluated against a separately labelled, permitted sample; contract validity does not prove semantic truth.
