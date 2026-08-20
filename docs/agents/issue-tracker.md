# Agent 任务追踪：GitHub Issues

> 状态: 权威当前
> 最后核对: 2026-08-20
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
