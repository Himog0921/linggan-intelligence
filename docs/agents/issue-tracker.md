# Agent 任务追踪：GitHub Issues

> 状态: 权威当前
> 最后核对: 2026-08-28
> 适用范围: Matt Pocock 工程技能与其他 Agent 对 Linggan Intelligence 任务、问题、阻塞和工作单的读写
> 事实来源: GitHub 私有仓库配置、当前 `origin`、`gh` 实际读取结果和用户确认
> 冲突时以谁为准: `AGENTS.md`、真实代码与运行证据、ACCEPTED 决策、`docs/README.md` 中的权威文档和用户最新确认

Linggan Intelligence 使用私有仓库 `Himog0921/linggan-intelligence` 的 GitHub Issues 作为任务追踪器。

Agent 在仓库克隆目录中使用 `gh` 命令进行操作。仓库默认从 `git remote -v` 推断；需要消除歧义时显式使用 `--repo Himog0921/linggan-intelligence`。

## Issues 的职责

GitHub Issues 用于：

- 记录待讨论问题、缺陷、实施任务和外部阻塞；
- 保存任务当前责任、状态、标签和依赖；
- 把已经确认的规格拆成可以独立验证的小任务；
- 为跨多个会话或 Agent 的工作提供统一工作单；
- 连接相关计划、决策、提交、测试和验证证据。

GitHub Issues 不用于：

- 取代 `AGENTS.md` 的最高约束；
- 取代 `docs/context/domain-language.md` 的正式领域语言；
- 取代 `docs/decisions/` 的长期决定；
- 取代 `docs/plans/active/` 的正式实施计划；
- 取代 `docs/progress/` 的实际变更记录；
- 仅凭标签或关闭状态证明代码、数据库、插件、部署或业务链路成功；
- 通过一个 Issue 自动扩大真实数据、生产系统、插件工位、外部 Agent 或现实行动的授权范围。

Issue 是工作入口，不是第二事实源，也不是自动授权书。

## 先分级，再决定是否需要 Issue

Issue/worktree/PR 是高风险与并行开发的隔离工具，不是所有文字改动的入场券。

| 级别 | 典型改动 | 最低流程 |
|---|---|---|
| 受保护交付 | 代码、migration/schema、运行合同、插件/release、部署/生产、真实外部动作、敏感数据/权限、不可逆处置、多个执行者或可能冲突的权威文件改写 | Mog 派定 Issue/交付包 → Claim → 独立 branch/worktree → PR → 按要求审查/集成 |
| 低风险直接维护 | 文档勘误、索引/链接、已确认决定的状态同步、进度记录、关闭/标记已被取代卡片，以及不改变产品/领域/运行/权限/数据语义的小型治理整理 | Mog 当前对话明确授权 → 核对干净工作区与无并行冲突 → 有界修改 → diff/治理检查 → 留痕并分层报告 |
| 只读调查 | 代码、运行、数据库或历史证据核对，不写回仓库和外部系统 | 无需 Issue/worktree/PR；如要写回，重新按前两级分类 |

低风险直接维护只免除不必要的流程，不免除事实核对、文件治理、变更记录和完成边界。执行中一旦需要改代码或运行合同、改变产品/领域/数据含义、扩大真实权限/敏感数据/成本/外部副作用、删除或重写重要历史、处理不干净或重叠工作区，或者无法用现有权威事实裁定，就必须升级为受保护交付。

## 受保护交付的执行闭环

本项目采用 **Mog 直接派单**，不允许 Agent 自由抢单、自动分派或自行增加并行。Mog 决定谁执行、允许多少并行、何时审查及何时合并；只有 Mog 在当前事项中明确委托时，指定协调者才可代行某一项协作决定。执行 Agent 只能处理 Mog 明确分配给自己的 Issue。

受保护交付的标准链路分成四个阶段，不能在 Issue 创建时提前填写未来事实：

```text
Issue request
→ Mog（或其明确指定协调者）核对、ready/assignment
→ Claim 评论
→ 独立 branch + worktree
→ 有界修改与验证
→ draft PR
→ Mog 指定时才进行 review
→ Mog 对 exact head 授权后才集成
→ 重新核验 main 与正式文档
→ 分层记录完成证据
→ 手工关闭 Issue
```

### 阶段 1：Issue request

Issue Form 在创建时只要求：SCOPE/authority、goal、in/out、dependencies、候选文件边界、acceptance、validation plan、stop/escalation 和 completion requirements。它不要求填写尚未发生的 Agent、exact base、branch、worktree、实际验证结果或完成证据。

本仓库是 private repository，Issue Form 中的 `validations.required` 只作为表单提示，不能当成可靠执行门。Mog（或其明确指定协调者）必须人工检查字段完整、权威有效、依赖已解、文件范围可分配、停止条件清楚，才能赋 assignee 和 `ready-for-agent`；不得用占位符骗过表单后直接 Claim。

### 阶段 2：Mog ready 与 assignment

Mog 决定是否可以派单，并在 Issue 正文或评论中明确执行 Agent、并发许可、exclusive/shared/forbidden 文件、依赖、是否需要 reviewer、谁可集成及 exact-head 合并授权方式。涉及产品、领域、架构、真实权限或多个 Issue/PR 的事项必须先完成对应决定或 active plan，不能靠标签放行。Agent 不得把“协调者”理解成默认获得派单、并发或合并权。

### 阶段 3：Claim 协议

任何受保护交付开始前，执行 Agent 必须在 Issue 评论中留下 Claim，至少包含：

- Agent 标识和稳定 task-id；
- exact base commit；
- feature branch 和独立 worktree 绝对路径；
- exclusive files、shared files 与 forbidden files；
- 开始时间、依赖和停止条件。

同一个 Issue 同一时间只能有 Mog 明确允许的执行 Agent。协作者、reviewer 和 integration owner 只有在 Mog 指定时才可存在，且不能形成第二个未声明的执行分支。使用相同 GitHub 账号的所有 Agent 必须靠稳定 task-id、Claim 和 exact commit/PR 证据区分责任，不能把共同账号当成执行身份。实现者不得自行担任 reviewer、integrator 或 merge 授权者。

本项目没有额外的 `in-progress` triage 标签。执行中由 Claim 评论和 assignee 表达；`ready-for-agent` 可以保留，因为它说明任务仍是机器可执行的有界工作，不代表尚未被领取。

### 分支、worktree 与文件所有权

所有受保护交付都必须发生在 Issue 专属 feature branch 和独立 worktree。禁止编码 Agent 在共享 root checkout 或其 `main` 分支直接实施这类改动；root checkout 只用于核对和集成，不是并行开发工作区。

纯只读调查或审查不需要 worktree/PR。调查结果若只形成已经明确授权的低风险状态同步，可以走上文直接维护通道；若会改变代码、合同、语义、权限或高风险权威面，则必须升级为 Issue/Claim + 独立 branch/worktree + PR。

Issue 必须把文件分为：

- **exclusive**：只由本 Issue 执行 Agent 修改；
- **shared**：可能被多个事项触碰，只能由指定 integration owner 统一整合；
- **forbidden**：本 Issue 不得修改；未列出的文件默认 forbidden。

执行 Agent 不得通过扩大 glob、顺手重构或修改相邻文档绕过所有权。共享文件出现并行变化时，执行 Agent 保留自己的有界补丁和证据，报告 Mog；由 Mog 决定是否指定 integration owner、改变并发或安排独立集成。禁止在任一执行 worktree 中擅自吸收其他任务。

### 阶段 4：Pull Request、handoff、审查和集成

受保护交付通过 PR 交付；是否先保持 Draft 由当前交付包决定。PR 默认使用 `Refs #<issue>`，不能用 `Closes` 跳过合并后核验。PR 必须关联主 Issue/SCOPE，使用模板报告修改范围、非目标、实际验证、数据库/外部副作用、proved/not proved、共享文件和分层完成证据。

是否安排 reviewer、由谁审查、是否需要新 head 复核，以及何时集成，都由 Mog 对当前交付包明确决定。若 Mog 指定 reviewer，则 reviewer 必须使用稳定 task-id、审查 PR 当前 exact head commit，并给出 `PASS` 或带可执行发现的 `FAIL`。代码可合并性、模板勾选或自动检查通过不能自动取得 Mog 的合并授权。

只有 Mog 或 Mog 对当前 exact head 明确指定的 integration owner 可以合并 PR。执行 Agent 不自行 merge；同一批并行 PR 不通过“最后一起解决冲突”压缩来源或责任。合并后由 Mog 或其指定 integration owner 重新核验 main、正式文档与实际副作用，追加完成层级记录，再按 Mog 指示关闭 Issue。

### 小型工作单与 active plan

小型、低风险工作可以不建立独立 `docs/plans/active/` 计划。若它已经属于受保护交付，则还必须同时满足：

1. 已有 SCOPE、ACCEPTED 治理规则或用户明确授权；
2. 不改变产品含义、领域含义、系统架构、真实权限、生产或敏感数据范围；
3. 不跨多个 Issue 或 PR；
4. 对应 Issue/交付包已完整写明验收、验证计划、文件边界、停止/升级条件和完成报告要求。

任一条件不满足，就必须先建立 active plan。低风险直接维护不要求为了形式再建一个空 Issue；但必须符合本页分级条件并保留进度/提交或对话回执。受保护交付则不能借“小型”省略 Claim、worktree、PR 和必要的集成证明。

### 没有 GitHub 自动保护时的人工门禁

当前未把 GitHub Actions、branch protection 或 CODEOWNERS 作为本流程前提。对受保护交付，在这些自动门不存在时，以下人工证据缺一不可：

1. Issue Claim 与 assignee；
2. 独立 worktree/branch；
3. draft PR 和模板完整报告；
4. 与实现者不同的 reviewer 结论；
5. 带稳定 task-id 的 integration owner 合并次序确认；
6. 合并后对目标 branch 和正式文档状态的重新核验；
7. 分层完成记录与手工 Issue close。

任一证据缺失都不得用“GitHub 允许 Merge”替代。

### Issue 关闭与完成层级

Issue close 不得把不同完成层压成一个 `done`。受保护交付的关闭评论和正式进度记录必须分别说明：

- Design；
- Code；
- Automated checks；
- Real-chain proof；
- Deploy；
- Mog / business acceptance；
- Proved；
- Not proved。

不适用的层标 `N/A` 并说明理由；未发生的层标 `NOT VERIFIED`。PR merge 不能自动证明部署、真实链路或业务验收。实现型 Issue 只能在 merge 后核验和分层记录完成后手工关闭。纯决策/治理 Issue 可以在其权威决定已由当前主线或更高权威文件吸收、且关闭评论明确列出未证明层时关闭；close 不能改写这些边界。

## 基本操作

### 创建

```bash
gh issue create \
  --repo Himog0921/linggan-intelligence \
  --title "..." \
  --body "..."
```

多行正文使用 heredoc 或正文文件，避免命令转义破坏内容。创建前必须检查：

1. 是否已有同主题 Issue；
2. 是否已有权威文档覆盖该问题；
3. 这是待讨论、需要决定、已确认实施还是外部阻塞；
4. 是否需要关联事项编号、SCOPE、ADR、测试或来源；
5. 创建 Issue 是否会产生超出当前授权的新工作。

### 读取

```bash
gh issue view <number> \
  --repo Himog0921/linggan-intelligence \
  --comments
```

读取时同时检查 Issue 正文、当前标签、评论中的新事实或用户确认、关联计划/决定/提交/验证、是否已被后续决定取代，以及是否仍存在未解决的 `DECISION_REQUIRED`、`SOURCE_INCOMPLETE` 或 `BLOCKED`。

### 列表

```bash
gh issue list \
  --repo Himog0921/linggan-intelligence \
  --state open \
  --json number,title,body,labels,assignees,comments
```

Agent 应按当前任务使用标签和状态过滤，不能把所有 Issue 一次性加载为上下文。

### 评论

```bash
gh issue comment <number> \
  --repo Himog0921/linggan-intelligence \
  --body "..."
```

重要评论应说明新发现、对当前任务的影响、已经验证和仍不能下结论的边界，以及是否需要更新正式项目文档。长期有效的决定不得只留在 Issue 评论中；必须同步进入相应的权威文档或 `docs/decisions/`。

### 标签

```bash
gh issue edit <number> \
  --repo Himog0921/linggan-intelligence \
  --add-label "..."
```

```bash
gh issue edit <number> \
  --repo Himog0921/linggan-intelligence \
  --remove-label "..."
```

Triage 角色标签以 [`triage-labels.md`](triage-labels.md) 为准。`bug`、`enhancement`、`documentation` 等标签描述任务类型；`ready-for-agent` 等 triage 标签描述下一步责任。两类标签可以同时存在，但不能相互替代。

### 关闭

```bash
gh issue close <number> \
  --repo Himog0921/linggan-intelligence \
  --comment "..."
```

关闭前必须说明实际完成内容、验证、数据库或外部系统副作用、关联 commit/PR/计划/文档、未完成边界，以及是否需要更新 `docs/current-state.md`、相关计划和 `docs/progress/YYYY-MM.md`。

关闭 Issue 不自动表示代码已经推送、migration 已执行、生产已部署、插件已发布、真实 Evidence 链路已成立或用户已完成业务验收。

## 工程技能约定

当技能说“发布到任务追踪器”时，创建 GitHub Issue；当技能说“读取相关任务”时，运行 `gh issue view <number> --repo Himog0921/linggan-intelligence --comments`，并同时读取 Issue 指向的权威文档。

以下技能会读取本文件：

- `to-spec`；
- `to-tickets`；
- `triage`；
- `wayfinder`；
- `implement`；
- `code-review`。

创建或处理 Issue 前仍需遵守 `AGENTS.md`、当前 SCOPE 和用户授权边界。

## Pull Request 是否进入 triage

**PRs as a request surface: no.**

Pull Request 不是默认需求入口。PR 可以实现或讨论已有任务，但不自动进入 Issue triage 流程。如果未来改变这一规则，应先更新本文件和当月变更记录，不能由某个 Agent 临时改变。

## Wayfinder 大型事项

`wayfinder` 使用一个总 Issue 保存已知事实、已确认决定、当前不确定区域、子任务关系和后续推进边界。子任务优先使用 GitHub 子 Issue；原生子 Issue 或依赖关系不可用时，退回到总 Issue 中的任务清单和子 Issue 正文中的关系说明。

子 Issue 应明确：

- `Part of #<map-number>`；
- 任务类型；
- 阻塞项；
- 当前责任；
- 可证伪退出条件。

领取任务时才分配给当前执行者。只有所有阻塞项关闭后，任务才可以进入执行。

## 授权边界

以下行为不能仅凭 Issue 或标签自动获得授权：

- 改变正式 Topic、Claim、领域语言或观察基线；
- 访问真实敏感 Evidence；
- 扩大真实平台采集；
- 占用插件工位或账号资源；
- 修改生产数据库；
- 发布外部内容；
- 执行隐私删除、撤回或传播处置；
- 向外部 Agent 批量提供原文；
- 部署到生产；
- 删除或覆盖历史资料。

`ready-for-agent` 只表示任务描述足以由 Agent 执行，不表示上述高风险权限已经成立。

## 渐进式披露

处理一个 Issue 时，默认读取顺序是：

1. `AGENTS.md`；
2. `docs/current-state.md`；
3. 本 Issue；
4. Issue 直接引用的计划、决策、合同或测试；
5. 当前任务需要的最少代码；
6. 只有来源核对确有需要时才进入 `references/`。

禁止为了“全面了解”一次性加载整个 `references/` 或全部项目文档。
