# GOV-002 Agent 工程技能配置

> 状态: 已完成计划
> 最后核对: 2026-08-20
> 适用范围: Matt Pocock 工程技能在 Linggan Intelligence 中的任务追踪、triage 和领域文档适配
> 事实来源: 用户逐项确认、当前 GitHub 仓库配置、项目文件树和实际治理检查
> 冲突时以谁为准: `AGENTS.md`、真实 GitHub 状态、GOV-001 权威文档和用户最新确认

## 目标

让 `to-spec`、`to-tickets`、`triage`、`wayfinder`、`implement`、`code-review`、`domain-modeling` 等工程技能能够读取统一的项目级配置，同时保持 Linggan Intelligence 已确认的文件治理、领域语言、决策和授权边界。

## 用户可见结果

- 任务统一进入私有仓库 GitHub Issues；
- 五个 triage 角色拥有固定标签和含义；
- 工程技能读取既有领域语言、决策和不变量；
- GitHub Issue、标签或关闭状态不成为第二事实源或自动授权；
- 后续 Agent 不会为了适配通用技能创建平行 `CONTEXT.md`、`docs/adr/`、产品真相或任务体系。

## 明确非目标

- 不创建 SCOPE-001 业务 Issue；
- 不启动 fixture、migration 或 Rust 业务代码；
- 不访问真实平台或真实敏感 Evidence；
- 不修改插件、Agent Runtime、生产数据库或部署；
- 不把 GitHub Issues 变成正式产品、架构、领域或进度文档的替代品。

## 已确认决定

1. Section A：使用 `Himog0921/linggan-intelligence` 私有仓库的 GitHub Issues。
2. Section B：采用 `needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix` 默认角色词表。
3. Section C：采用单一领域上下文，但映射到 `docs/context/domain-language.md`、`docs/product/domain-invariants.md` 和 `docs/decisions/`；不创建根目录 `CONTEXT.md` 或 `docs/adr/`。
4. 用户已审阅并确认 `AGENTS.md` 与三份 `docs/agents/` 文件草案，以及创建四个缺失 GitHub 标签的动作。

## 实际交付

- `AGENTS.md` 增加 `Agent skills` 入口；
- 新增 `docs/agents/issue-tracker.md`；
- 新增 `docs/agents/triage-labels.md`；
- 新增 `docs/agents/domain.md`；
- 文件放置规则、总索引、当前状态和月度变更记录同步更新；
- GitHub 保留既有 `wontfix`，补齐另外四个 triage 标签。

## 冲突处理

通用技能默认建议根目录 `CONTEXT.md` 和 `docs/adr/`，与 Linggan 已有权威结构重复。GOV-001 的“一个事实一个权威入口”优先，因此本项目通过 `docs/agents/domain.md` 做适配，不复制目录，也不降低现有领域文档和决定的权威性。

## 验收

- 三份配置文档有完整状态头并进入 `docs/README.md`；
- `AGENTS.md` 只有一个 `## Agent skills` 区块；
- GitHub 五个 triage 标签均可读取；
- `git diff --check` 通过；
- `./scripts/check-project-governance.sh` 通过；
- 工作区只包含 GOV-002 相关文件，没有业务代码、migration、fixture 或真实数据。

## 后续边界

本事项完成后，工程技能可读取这些配置。只有切换任务追踪器、改变 triage 词表或重构领域文档布局时才需要重新运行 setup；普通标签或文案调整可直接更新 `docs/agents/*.md` 并遵守 GOV-001。
