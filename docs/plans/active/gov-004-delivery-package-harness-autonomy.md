# GOV-004 · 交付包优先与 Harness 自主执行治理

> 状态: 活跃计划
> 最后核对: 2026-08-26
> 适用范围: Linggan Intelligence 的 Agent 协作、UI 执行和任务追踪治理
> 事实来源: `AGENTS.md`、`docs/governance/agent-collaboration.md`、`docs/agents/ui-execution-contract.md`、Issue #70
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实代码/运行证据、ACCEPTED 决定

## 目标与用户可见结果

将项目协作方式从“由 Mog 逐张任务卡指挥实现”改为“由 Mog 定义结果与不可接受后果，Harness 自主完成交付包”。未来一个可见需求不会因为最初的文件边界、审查顺序或 Agent 分工，被拆成需要 Mog 反复介入的修补循环。

## 范围与非目标

**范围**：更新最高 Agent 合同、协作协议与 UI 执行合同，明确交付包、Harness 自主、升级边界、完整盘点和批量化审查。

**非目标**：不更改任何产品功能、页面、设计 Token、插件、真实采集、数据库、SCOPE-001、权限、隐私或 Evidence 规则；不新建重复的 `CODEX.md`。

## 已确认事实与决策

- 现有治理正确保护了事实、未知、权限和独立审查，但“一个事项一个事项推进”“仅修改 Issue 授权文件”等措辞容易被误用为文件小卡优先。
- 用户确认的协作权力边界：Mog 不应负责告诉 Harness 怎样做；他应说明期望结果、禁止/不可接受结果和需要他裁定的业务后果。
- 现有 `AGENTS.md` 和治理文档已是权威入口；新增同级 `CODEX.md` 会形成重复权威源，因此不创建。

## 执行与可证伪验收

1. 将 `AGENTS.md` 明确为“用户结果优先、Harness 自主执行”。
   - 验收：文件列明用户与 Harness 各自的决定范围，且不削弱事实/权限硬约束。
2. 将协作协议改为“交付包优先”，添加表面/状态/依赖/验收矩阵与类别级自审规则。
   - 验收：协议不再把 Issue/文件边界描述为用户结果单位；共享依赖有明确集成路径。
3. 将 UI 合同改为交付包内 Work Package 的执行合同。
   - 验收：闭集保护语义而非偶然文件名单，且没有授权新增功能或状态。
4. 运行治理与文本一致性检查。
   - 验收：`./scripts/check-project-governance.sh`、`git diff --check` 通过；关键禁止项能在权威文档中检索到。

## 文件、风险、停止与回退

**允许修改**：`AGENTS.md`、`docs/governance/agent-collaboration.md`、`docs/agents/ui-execution-contract.md`、本计划、`docs/README.md`、`docs/progress/2026-08.md`。

**风险**：文字可能被理解为放松 Scope 或文件所有权。缓解：明确“语义闭集不变”“共享依赖仍需声明与集成”“新增语义仍要升级”。

**停止条件**：若任何条款必须改动 Evidence、隐私、权限、Unknown 或 SCOPE 的硬规则，停止并另提决策。

**回退**：该事项只改治理文本；未合并前可丢弃分支。若合并后发现措辞与权威约束冲突，以 `AGENTS.md` 事实/权限规则优先，并通过新的 GOV Issue 修订。
