# COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2 · 可验证的稳定问题归并

> 状态: 活跃计划
> 最后核对: 2026-09-15
> 适用范围: Issue #285 的 Comment Research Problem Resolution V2 源码、隔离测试、读取/API/UI 合同与文档同步
> 事实来源: Mog 当前交付授权、DEC-0003、DEC-0004、`main@dab0be3`、Issue #285
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、运行/数据库副作用、当前代码/migration/测试

## 用户结果与边界

用户在「用户问题」中看见的是可追溯的稳定 Problem，而不是逐条评论改写出的标题；合法的新信号不会消失，但不会因单条无匹配 Atom 自动污染 Problem 库。每一条已形成或尚未形成的结论都可回到原声、归一描述、候选比较和下一次重评估条件。

保留现有 Raw Comment → Derivation → Atom → local embedding → candidate recall → membership → ResultRevision 主线。替换 Atom 到 Problem 的决策合同、持久化和读取表达。不得改写原始评论或历史执行事实，也不新增外部服务、队列、自动排程或原始材料外发。

## 已确认事实

- `main@dab0be3` 已含父评论受控上下文、`semantic.v6` 输出合同和 #281 的跨 Run resolution execution history。
- 当前归并模型仍直接输出 `same_problem` 或 `new_problem`，而 `new_problem` 可单条直接写入 Problem；这与 DEC-0004 冲突。
- 本地 WeMM exact cosine 只能召回候选；它不能决定语义等价或写 membership。
- 下载附件的 schema/policy 是设计输入；其 28/28 Schema 样例不证明当前仓库集成、Rust、PostgreSQL、模型或 UI 正确。

## 领域与状态词典

| 术语 | 含义 | 不能替代 |
|---|---|---|
| Problem Frame | 有来源的 subject/goal/observed issue/context 描述；槽位可未知 | 根因诊断、Problem 名称、模型行动 |
| eligible | 当前领域中、由明确困难/未满足需求支撑且可进入归并判断的 Atom | 已关联 membership |
| Stable Problem | 由版本化定义和累计 membership 支撑的长期研究对象 | 单条 Atom、Topic、趋势或市场事实 |
| deferred_novel | 已完成候选判断但等待独立同类证据的信号 | failed、running、未发现或 membership |
| deferred_ambiguous | 多个匹配或关键未知，不能由向量最高分硬选 | no-match 或创建授权 |
| contract failure | Schema/候选/引用/快照验证失败 | 合法 deferred 业务结论 |

## 实施工作与可证伪验收

1. **合同与纯规则**：把 Frame 与候选比较变为封闭 Schema/Rust 类型；服务端验证引用和完整候选集合。测试漏答、重复、伪造候选/引用、unknown、empty catalog 和相互矛盾的正反例。
2. **持久化与并发**：追加一个 additive migration，保存 frame、identity definition、current decision、decision payload、retry trigger 与 catalog guard；不创建第二套执行历史。隔离 PostgreSQL 证明原子 membership、独立来源、过期 catalog proposal 与失败不降级。
3. **worker/召回/续办**：复用 WeMM embedding 与既有 worker/ledger；两路 bounded recall、definition/representative-evidence snapshot、固定维度模型调用和纯函数提案；不在模型等待中持锁。检查触发指纹防止无变化重复调用。
4. **读取与页面**：在既有「用户问题」页面中按已形成/待归并表达当前真相，并展示原声、frame 来源、候选边界、决定、重评估条件和历史；不新建一级页面，不引入前端写入或伪回执。
5. **历史与切换边界**：只提供有限的、可审计的候选重评估入口；不批量重写旧 Problem、不清库、不自动运行。真实样本重放、共享 migration、runtime、模型、排程、merge 和业务验收另行授权。

### 2026-09-15 实施检查点

- 已完成：ADHD policy scope 的冻结、V7 Atom/Problem Frame 合同、纯决策内核、V2 additive schema、候选 index 比较、`deferred_*` 决策与重评条件持久化、无候选时不调用模型，以及双独立信号的 catalog-guard 原子 admission 入口。
- 已完成：deferred-Atom 的第二路受限召回、pair-comparison worker 编排、受限 pair 输出合同、模型账本关联、120 秒 lease 与有界恢复；单条、同作者、同来源或原文完全重复的信号均不能建立 Problem。
- 已完成验证：规则内核、Atom 接纳、Problem admission、worker 合同单测；定向隔离 PostgreSQL proof 已证明两个独立 `deferred_novel` Atom 一次事务建立一个 Problem、两条 current membership 并递增 catalog revision，且同作者 pair 被拒绝。隔离建库过程中已暴露并修复 UUID 绑定与 Atom 写入参数序号两处缺陷。
- 已完成：V2 read DTO/API/UI；“用户问题”同一读取面可筛选 `全部 / 已形成问题 / 待归并信号`，只读侧栏按原声、frame 出处、候选快照与逐维比较、结论、重评条件、执行历史展开；打开详情不调用模型。
- 尚未完成：完整隔离回归的可复核终态、PR/合并/迁移应用/运行时与业务验收。`test-comment-research-postgres.sh` 已改用 Docker inspect 的结构化端口字段并可越过先前的端口 parser 失败；本次已启动完整 42 项 PostgreSQL suite 且随机容器/卷已清理，但当前交互没有保留其最终退出回执，因此不能将完整 suite 标为通过。上述定向 pair proof 是已确认的独立证据。

## UI 变更清单

| 面 | 常用状态 | 空/未知/失败边界 | 验收 |
|---|---|---|---|
| 概览 | 只读 confirmed membership 的累计事实 | 不把 deferred 数作 Problem 数或统计趋势 | API/read test |
| 用户问题 | `全部 / 已形成问题 / 待归并信号` 是同一读模型的筛选，不产生动作 | 0 是已知没有；未知/未评估不显示 0；contract/provider failure 不伪装 deferred | API + JS/render test |
| 详情抽屉 | 原声→frame及出处→候选比较→结论→唤醒条件→执行历史 | 不重新调用模型解释；引用不可读时明确限制 | read DTO test |
| 运行记录 | 区分已完成决策、等待、执行失败与续办 | 不把 failed 计作完成判断 | PostgreSQL/API test |

这是状态/语义变更，采用现有 L1 shell、文字 Tab、表格/空态和 LIDS data-boundary/LANG-05 规则；不新增 Token、全局组件、页面壳、权限或实际行动。完整读取回执在 [`../design/changes/comment-research-problem-resolution-v2-ui-change-manifest.md`](../../design/changes/comment-research-problem-resolution-v2-ui-change-manifest.md)。

## 文件与协作边界

Issue #285 Claim 中列出的 Comment Research code/migration/test/docs 是本事项 exclusive；`docs/progress/2026-09.md` 是 shared，写前必须重取 `origin/main` 并只追加本事项实际证据。`references/`、runtime scripts、plugins、collection/evidence 非评论研究代码、既有 migrations、真实数据库/配置/秘密为 forbidden。

## 停止点、回退与证明

停止受影响部分并报告 `DECISION_REQUIRED`：若领域范围无法映射到已有 policy、UI 需要未批准的状态/交互、需要新增原始材料传播、或 shared-file 冲突需要 integration owner。回退为不合并本分支；本事项不应用 migration，因此不会改变共享数据或运行时。

计划中的验证：规则单测、目标 crate/workspace 检查、`test-comment-research-postgres.sh`、API/UI 合同检查、治理检查、diff 审计。NOT VERIFIED：真实模型语义准确度、历史 27 Problem 的实际合并数量、真实数据库 migration、runtime/3000、排程、部署和 Mog 业务验收。
