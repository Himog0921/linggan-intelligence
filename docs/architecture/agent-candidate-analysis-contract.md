# AGENT-CANDIDATE-ANALYSIS-001 · 单一 A1–A2 候选分析合同

> 状态: 草案
> 最后核对: 2026-08-31
> 适用范围: Issue #113，基于一个 exact Topic Material Pack 的受控读取与候选分析
> 事实来源: 用户本轮授权、Issue #113、Agent/Pi 架构基线、当前 migration/Rust/tests
> 冲突时以谁为准: 用户最新确认、AGENTS.md、领域不变量、当前代码与真实运行证据

## 1. 一句话合同

Linggan 冻结调用者、委托、目的、Profile、Topic Definition/Run/Pack、受控片段、工具与预算，随后允许一个执行适配器只读这一个材料包并返回带支持、反例、限制、可推翻条件、替代解释、未知和信息缺口的 `candidate_only` 输出。Invocation 主账、资格验证、取消和终态属于 Linggan；适配器不能写 Topic、Claim、采集任务或外部行动。

## 2. 深模块边界

```text
CandidateAnalysisInvocation
  actor + delegation + purpose + fixed profile
  exact Topic/Definition/ClassificationRun/MaterialPack
  exact members + controlled de-identified excerpts
  one read tool + bounded steps/calls/output
        ↓ submit / status / cancel
PostgresAgentRuntime
  authoritative invocation state + idempotency hash
        ↓ ModelToolLoopPort
  DeterministicCandidateAdapter | PiCandidateAdapter transport seam
        ↓
Output Validator
  exact citations + counterweight + alternatives + unknowns + gaps
        ↓
CandidateAnalysisOutput / candidate_only
```

这条 seam 不接受 SQL、shell、通用 HTTP、插件、动态检索或可扩张的观察范围。`controlled_excerpt` 是调用前已经明确脱敏、长度受限并被冻结的输入，不是让适配器自行读取原始评论的后门。

## 3. 权威输入与 Pack 核对

`submit` 在落主账前同时验证：

1. Profile key/version、调用目的、委托 revision 和各项文字边界；
2. 最多 8 步、恰好 1 次工具调用、最多 5 条候选的 A1–A2 预算；
3. Pack 至少两项，且同时有 support 和 challenge/boundary；
4. Topic、Definition version、Classification Run、Material Pack、source boundary 与 PostgreSQL 权威关系完全一致；
5. 成员顺序、`work_public_ref`、role 与人工 rationale 和冻结 Pack 完全一致；
6. 相同 idempotency key 只可重放相同请求 hash，异内容冲突。

这使页面当前筛选、临时搜索结果或适配器自行补充的引用不能偷偷进入一次已经获准的 Invocation。

## 4. 状态、取消与恢复

当前状态仅为 `accepted → running → succeeded | rejected | failed`，或在 `accepted/running` 时进入 `cancelled`：

- `succeeded` 必须同时有通过验证的 Candidate JSON；其它状态不能携带 Candidate；
- exact pack read 成功后保存一份 `read_topic_material_pack` Tool Receipt；
- 输出越界进入 `rejected`，provider/transport 失败进入 `failed`，不能伪装为部分成功；
- 已中断且仍为 `running` 的调用由 `recover_interrupted` 明确关闭为 `failed / interrupted_before_finalize`，不根据 Pi 临时 session 猜测成功；
- 取消与 finalize 竞争时，条件更新保证已取消调用不能随后变成 `succeeded`。

这不是完整的分布式 lease/重试系统。当前切片选择失败关闭，没有自动重试 provider，也没有用重放隐藏可能发生的外部成本。

## 5. 输出资格

每份成功输出必须：

- `claimCeiling = candidate_only`；
- 每条候选至少一条 support 和一条 challenge/boundary 引用；
- 所有 observed/support/challenge refs 都属于 frozen Pack；
- 包含非空的 limitation、falsification condition、alternative explanation、unknown、information gap 和用户摘要；
- 恰好记录一次已授予的 Pack read，且不超过候选与工具预算。

没有 Candidate Claim adopted、Topic Release、Proposal/A3、Action/A4 或自动采集入口。`succeeded` 只表示合同内候选已冻结，不表示候选正确、业务有用或已被人接受。

## 6. 两个适配器的真实状态

- `DeterministicCandidateAdapter` 已真实执行同一合同，用于预算、引用、反例、失败关闭和 PostgreSQL 终态证明。
- `PiCandidateAdapter` 已建立 Linggan-owned transport seam，并硬拒绝非 `@earendil-works/pi-agent-core@0.84.2` 的版本；固定版本 transport fixture 与 deterministic adapter 通过同一输出合同。
- **尚未安装或执行真实 Pi npm 包，也没有调用真实 provider。** 因而这只证明固定版本门与 Adapter seam，不证明 PI-1 的低层 Agent 构造、真实工具循环、成本、取消或上游事件映射。

真实 Pi 接入必须是本 Draft 的后续独立收口，不得把 transport fixture 写成“Pi 已接通”。第三方数据处理条件未确认前，真实材料入口保持 disabled。

## 7. 已证明与未证明

已证明：公开 Contract、deterministic/Pi transport 共用输出 validator、越界引用与超预算拒绝、fixed Pi version fail-closed、PostgreSQL exact Pack 核对、幂等、submit/status/cancel、工具回执、不合格输出拒绝、中断终态及隔离 proof 清理。

未证明：真实 Pi package/provider、真实模型质量与成本、真实材料权限/脱敏质量、长期并发负载、自动重试、分布式 worker lease、Agent API/UI、共享本机 migration、部署、Mog/业务验收、候选转正式知识的 Decision 链。
