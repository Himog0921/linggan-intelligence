# AGENT-CANDIDATE-ANALYSIS-001 · 实施卡

> 状态: 活跃计划
> 最后核对: 2026-08-31
> 适用范围: Issue #113 的单一受控 A1–A2 候选分析切片
> 事实来源: 用户本轮授权、Issue #113、Topic PR #115、Agent/Pi 架构基线、当前分支代码与验证
> 冲突时以谁为准: 用户最新确认、AGENTS.md、正式领域不变量、当前代码与真实运行证据
> Issue: #113
> 分支: `codex/agent-candidate-analysis-001`
> 堆叠基线: `df51269` / Draft PR #115

## 目标

证明一个最小、可替换的 Linggan Agent Kernel：只对 exact Topic Material Pack 做 A1 读取和 A2 候选解释，保存权威 Invocation/Receipt/候选终态；任何正式知识、扩采或现实动作都在范围外。

## 范围冻结

- additive `0028_agent_candidate_analysis.sql`；不改历史 migration，不应用共享本机库。
- `AgentRuntime` 的 submit/status/cancel 与显式 execute/recover seam。
- 窄 `ModelToolLoopPort`、deterministic adapter、Pi fixed-version transport adapter。
- exact Pack validator、输出 validator、隔离 PostgreSQL proof 与攻击性 contract tests。
- 只用合成、明确脱敏 fixture；无真实原文、真实 provider 或网络模型调用。

## 非目标

- API/UI、聊天、Profile 管理台、Control Plane、Runtime Router、多 Agent 编排或长期记忆。
- SQL/shell/HTTP 通用工具、插件、采集、scheduler、WorkOrder、Topic 修改。
- A3 Proposal、A4 Action、Claim adopted、Knowledge Release、Brief、选题或外部内容。
- 安装/执行真实 Pi npm 包、发送真实材料给第三方、部署、合并。

## TDD 记录

1. RED：公开合同测试先引用不存在的 Invocation、Budget、Port、Adapter 与 Validator，Rust 编译失败。
2. GREEN：deterministic exact-pack 分析、candidate-only 输出、反例/未知/gap 和越界引用拒绝通过。
3. PROOF RED：首轮 PostgreSQL proof 的成员核对错误使用不存在的 `material_pack_ref` 列，3/4 用例失败。
4. PROOF GREEN：改为按权威 `classification_run_ref` 核对 exact members 后，幂等、取消/恢复、Pack mismatch、不合格输出和成功工具回执全部通过；隔离 container/volume 清理。

## 当前退出门

| 层 | 当前状态 | 说明 |
|---|---|---|
| 设计/合同 | VERIFIED（branch） | A1–A2、主账、输出和非目标已冻结 |
| Code | IMPLEMENTED（branch） | PostgreSQL Runtime、deterministic、Pi transport seam |
| 自动检查 | VERIFIED（branch） | workspace tests、聚焦 clippy、治理、fmt、diff check 与 PostgreSQL proof 已绿 |
| Offline contract spike | VERIFIED | 合成 exact Pack 与攻击性负例已运行 |
| Real Pi/provider | NOT STARTED | transport fixture 不是 Pi npm/runtime 证明 |
| Real material | DISABLED | 第三方数据处理与脱敏资格未确认 |
| Deploy | NOT STARTED | 本卡不自动迁移/部署 |
| Mog/business acceptance | NOT VERIFIED | 等待 Draft 审查和后续实际任务验收 |

## 停止条件

- 需要动态扩大 Pack、通用 shell/HTTP/SQL、插件或 WorkOrder 时停止。
- 需要把 Candidate 升级为正式 Claim/Topic/Brief/Action 时停止。
- 需要真实原文或第三方 provider 但数据处理条件未明确时停止。
- Pi 实包版本/完整性/事件语义不能锁定时，保持 transport seam，不伪装真实 Pi 已接通。
