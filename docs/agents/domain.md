# Agent 领域文档读取规则

> 状态: 权威当前
> 最后核对: 2026-08-20
> 适用范围: 工程技能和 Agent 在 Linggan Intelligence 中读取、使用和更新领域语言与长期决定
> 事实来源: 当前文档结构、GOV-001、DISC-001、SCOPE-001 和用户确认
> 冲突时以谁为准: `AGENTS.md` 的事实优先级、真实代码与运行证据、ACCEPTED 决策和 `docs/README.md`

Linggan Intelligence 使用单一领域上下文，但不采用 Matt Pocock 模板默认的根目录 `CONTEXT.md` 和 `docs/adr/`。

本项目已经建立了更完整的领域与文档治理结构。Agent 必须使用现有权威文件，禁止创建第二套共同语言或决策目录。

## 领域文档布局

```text
AGENTS.md
最高约束、事实优先级、不可违反规则
        ↓
docs/README.md
文档总索引、状态与渐进式披露
        ↓
docs/current-state.md
当前阶段、唯一在办事项和授权边界
        ↓
docs/context/domain-language.md
正式领域共同语言
        ↓
docs/product/domain-invariants.md
跨模块长期不变量和对抗性验收
        ↓
docs/decisions/
长期有效的接受决定
        ↓
当前任务直接相关的计划、架构、合同、代码与测试
```

## 开始探索前的固定读取顺序

任何工程技能开始处理 Linggan Intelligence 前，至少读取：

1. `AGENTS.md`；
2. `docs/README.md`；
3. `docs/current-state.md`；
4. `docs/context/domain-language.md`；
5. 当前任务直接相关的 `docs/decisions/`、计划、架构或产品文档；
6. 如果任务涉及领域边界或 Claim 资格，再读取 `docs/product/domain-invariants.md`。

不要无差别加载全部文档。只有任务需要校验历史来源、旧系统、producer 或插件事实时，才进入 `references/`，并先读取 `references/README.md` 和对应来源说明。

## 单一领域上下文的含义

当前 Linggan Intelligence 是一个统一产品、多个平级一级研究 Domain、一个模块化 Rust 单体、一个 PostgreSQL 主库、API 和 worker 两个组合入口，以及一个受控插件执行边界。多个 Domain 不等于多个 bounded context、服务、数据库或插件；它们共享基础材料能力，并按各自的研究用途隔离读取和结论资格。

`apps/`、`crates/` 或逻辑模块的存在不自动产生新的领域上下文。文件数量、crate 数量、页面导航、数据库表、专门 Agent 工具或独立实现目录本身都不能成为拆分多上下文的理由。

只有当领域语言、业务规则、权限、生命周期和长期责任确实独立，并通过正式架构决定确认后，才考虑引入 `CONTEXT-MAP.md` 或分上下文文档。

## 使用正式共同语言

Agent 在 Issue、SCOPE、实施计划、Rust 类型/函数/模块、数据库表/字段/约束、测试、API/CLI、架构审查和 Agent 结构化输出中命名概念时，必须使用 `docs/context/domain-language.md` 的正式词汇。

如果需要的概念不在共同语言中：

1. 先检查是否只是已有概念的同义词；
2. 如果是同义词，使用现有正式名称；
3. 如果确实存在领域缺口，记录候选定义和使用场景；
4. 使用 `domain-modeling` 做压力测试；
5. 经确认后更新现有 `docs/context/domain-language.md`；
6. 同步相关决定、索引和当月变更记录。

不得为了适配某个技能，另建根目录 `CONTEXT.md`。

## 决策记录

本项目的 ADR 等价目录是 `docs/decisions/`。Agent 不得再创建平行的 `docs/adr/`。

处理任务前，应读取与当前范围有关的决定。当前决定与实现发生冲突时，按 `AGENTS.md` 的事实优先级和 `docs/governance/agent-collaboration.md` 的冲突流程处理。

如果新建议与已有决定冲突，必须明确指出冲突的决定、重新打开的原因、受影响的代码/数据/文档/历史解释和需要的确认者；不能静默覆盖，也不能只在 Issue 评论中改变长期决定。

## 领域不变量

`docs/product/domain-invariants.md` 不是普通产品说明，而是后续实现的对抗性验收基线。Evidence 接入、部分采集结果、Observation/Current、Topic/Definition Release、Corpus、Signal/Claim/Intelligence、Decision/Action/Outcome、外部 Agent、隐私传播和趋势可比性等工作必须检查相关不变量。

测试通过、编译通过或接口返回成功，不能替代不变量证明。

## 更新责任

领域文档发生变化时，必须同步：

1. 权威领域或决策文件；
2. 直接受影响的产品、架构、合同或 SCOPE；
3. `docs/README.md` 中的状态或索引；
4. `docs/progress/YYYY-MM.md`；
5. 必要时更新 `docs/current-state.md`；
6. 运行 `./scripts/check-project-governance.sh`。

旧定义不能静默删除。改名、修订、拆分、合并和导航变化应按各自规则处理，不能压成一次普通文本替换。

## 工程技能如何使用本文件

以下技能在探索或写入前应读取本文件：

- `grill-with-docs`；
- `domain-modeling`；
- `improve-codebase-architecture`；
- `to-spec`；
- `to-tickets`；
- `implement`；
- `code-review`；
- `research`；
- `wayfinder`。

如果技能的默认目录与本文件冲突，以本文件和 `AGENTS.md` 为准。
