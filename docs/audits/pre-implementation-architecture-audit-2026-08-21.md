# 代码前独立对抗性架构审计

> 状态: 一次性报告
> 最后核对: 2026-08-21
> 适用范围: `AGENTS.md` 权威冲突治理、SCOPE-001 物理约束、首页 Agent 候选、Bootstrap 命名与第一阶段规模校准
> 事实来源: 本地工作区 `AGENTS.md`、领域不变量、当前 SCOPE、migration、架构/页面草案、实现就绪审计与点时 `git status`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、ACCEPTED ADR、当前活跃 SCOPE 与可复现代码/数据库事实；本报告只保存审查证据与最终处置，不是产品、架构或实现的第二真相源

> 稳定事项编号: `ADV-AUDIT-001`

## 最终结论

原始对抗审查成功暴露了三类值得保留的风险：不同层级事实发生冲突时可能静默漂移、合成事实内核与真实 producer 之间存在接缝风险、丰富的页面想象可能被误读成已获准的 Agent/产品架构。但独立复核确认，原报告把部分“未来设计问题”误判成“当前物理缺陷”，并给出两项危险动作：过早解除 SCOPE-001 的 1:1 证明约束，以及把混合工作区一次性提交。

本报告最终处置如下：

| 编号 | 最终裁定 | 当前处置 |
|---|---|---|
| `ADV-M-01` | **部分成立 / P1 治理歧义** | 需要 Mog 决定最高权威规则；不能采用“`INV-*` 永远绝对高于代码/测试”的例外，也不能允许低层实物静默改变产品含义 |
| `ADV-P0-01` | **不成立** | 保留 `capture_attempt.work_order_id UNIQUE`；SCOPE-001 明确只证明 Work 1:1 Attempt，未来 multi-attempt SCOPE 开始前再迁移并补齐 provenance/Oracle |
| `ADV-P0-02` | **部分成立，但不是当前阻断** | F01 可完成 synthetic fact-kernel tracer；完成后 hard stop。真实 producer audit 必须先于真实合同、真实插件和真实平台实现 |
| `ADV-P1-01` | **未来产品决定** | “不做复杂预算系统”与“每次 Invocation/批次有硬上限、触顶停止”可以并存；首页/编队未获实现授权，不在当前 F01 修改 |
| `ADV-P1-02` | **不成立** | Capture Satisfaction 是 Work 级聚合，不应在当前 1:1 proof 中伪装成 Attempt 事件；未来跨 Attempt 采用 revision + `sourceAttemptRefs`/`sourcePackageRefs` |
| `ADV-P1-03` | **部分成立** | 五个名称当前先是页面中的产品职责，不等于五个长期运行 Agent；P0 产品原型保留该机制后，才设计运行编队、输入、上限、失败与评测 |
| `ADV-P1-04` | **风险成立，原处置错误** | 不把点时 45 项改动一次性提交；先冻结新增写入、盘点来源/责任、拆成独立可审查提交，再逐个验证提交和推送 |
| `ADV-P2-01` | **已由显式临时映射关闭** | `crates/evidence` 是当前 Bootstrap/SCOPE-001 放置名，不改名；目标模块地图不是现行 crate 清单 |
| `ADV-P2-02` | **有价值的规模提醒，不是缺陷** | 保留八个责任表达，但只为当前切片建立真实结构；概念责任多不等于现在预建同数量表、crate、服务或状态机 |

本轮不改变当前代码门，不修改 migration、合同或 SCOPE-001，也不授权真实 producer、插件、媒体、Agent、Web 或生产。

## 证明范围

### VERIFIED

- 读取并交叉核对本报告列出的当前治理、领域、SCOPE、migration、架构与页面材料；
- 核对 SCOPE-001 明确把 Work 1:1 Attempt、synthetic contract 与未来 multi-attempt 迁移写成有意的证明范围；
- 核对页面文档把首页和五个职责标为产品形态草案而非实现授权；
- 核对 `module-architecture.md` 已把当前 Bootstrap crate 名称声明为临时放置，不具有目标模块优先权；
- 在本次修订开始前，点时工作区为 19 个 modified + 26 个 untracked，共 45 项；`HEAD` 与本地 `refs/remotes/origin/main` 均为 `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d`。

### NOT VERIFIED

- 本轮没有执行 `git fetch`，因此上述 `origin/main` 只是本地 remote ref，不证明 GitHub 此刻仍无新提交；
- 未核实真实 XHS producer 字段、分页、终止语义、账号镜头、媒体或风控；
- 未运行生产、容量、并发、部署或业务验收；
- 未证明首页常驻职责最终应当实现为多个 Agent；
- 未证明未来 multi-attempt 的物理合同、聚合规则或恢复路径。

审查材料的数量、轮次和字符数不是架构真假的证据，因此不再把“第几次审查”“读取多少份文档”或估算字符量作为权威结论。

## `ADV-M-01`：权威冲突治理存在歧义

原审查正确指出：如果 Agent 机械地把“真实代码/migration 顺位更高”理解为“实现可以改变产品含义”，低层错误可能被静默合法化。但原建议把所有 `INV-*` 移出事实优先级并设为绝对例外，也会制造反向错误：过时或表述错误的不变量可以否定已经复现的现实事实。

需要 Mog 决定的是**最高权威冲突规则**，不是某条 SQL：

- 可复现代码、数据库副作用和真实运行结果决定“系统当前实际上做了什么”；
- 用户确认、ACCEPTED ADR、当前 SCOPE 与领域不变量决定“系统获准做什么、产品含义是什么”；
- 两者冲突时不得平均、静默覆盖或按顺位自动修改产品语义，必须停止受影响部分并形成 `DECISION_REQUIRED`；
- 事实错误由实现修正；若要改变产品含义或不变量，必须经用户明确确认并留下版本记录。

最终裁定：**部分成立 / P1 治理歧义**。ARC-001 只记录这一决策，不在本报告直接修改 `AGENTS.md`。

## `ADV-P0-01`：当前 1:1 Attempt 不是物理缺陷

原审查把长期的 Work 1:N Attempt 与 SCOPE-001 的 Work 1:1 proof 当成同一时间必须成立的约束，因此误判 `UNIQUE` 使 `INV-56` 不可满足。

当前 SCOPE 已明确：

- 本切片只预置一个 synthetic Work 和一个 Attempt；
- 不实现真实插件休眠、lease 接管、换账号、补采或跨 Attempt Satisfaction；
- `work_order_id UNIQUE` 是 proof database 对当前 1:1 输入集合的物理保护；
- 未来 multi-attempt 是新的 SCOPE、migration、contract、route、fixture 和 Oracle，不是本切片的隐形能力。

现在移除 `UNIQUE`，但又不实现 1:N 的聚合、不变量和测试，只会让数据库允许当前应用没有资格解释的状态。最终裁定：**不成立；当前不改 migration**。

未来打开 multi-attempt 前必须同步证明：新 Attempt/Capture Identity、旧 Attempt 围栏、跨 Attempt Coverage/Satisfaction revision、来源 refs、迟到 Package、reconcile、恢复与 Current/CLI 展示。

## `ADV-P0-02`：合成先行存在返工风险，但 F01 可继续

原审查成立的部分是：synthetic 合同不能证明真实页面字段、终止、分页、账号镜头或恢复；如果合成层自然扩张为真实合同，会造成返工。

不成立的部分是把这一风险升级为当前 F01 的阻断。F01 的目的只是证明平台无关的事实内核：

```text
synthetic Package
→ PostgreSQL atomic ingress
→ two independent Records
→ Observation / Current
→ loopback API
→ minimal CLI
```

该链完成后立即 hard stop。ARC-001 未收口前，不继续 F02–F10，不访问真实 producer，不升级插件，不引入媒体、Agent、Web、生产或旧系统迁移。

真实 producer audit 必须在以下工作前完成：冻结真实 Capture Contract、真实插件实现、真实媒体 Lane、真实账号/工位调度。最终裁定：**部分成立，但不是当前 F01 阻断**。

## `ADV-P1-01`：预算简化不等于允许无限运行

`HOME-15` 的合理意图是第一阶段不预建复杂预算分配、自动降级和优化系统。它不能被解释为夜间批次可以没有任何停止边界。

未来若保留自动 Agent/批次，最低安全合同应同时成立：

- 每次 Invocation 或批次具有单一、可配置的金额/调用/步骤硬上限；
- 触顶即停止，输出 `partial + gap + stop reason`；
- 不静默续费、自动申请更多预算或把部分结果显示为完整；
- 是否建设成员级预算分配、优先级与降级顺序，等待真实花销后再决定。

当前首页、编队和夜间批次都未获实现授权。本项进入 ARC-001 的产品/运行决策，不修改 F01。

## `ADV-P1-02`：Capture Satisfaction 是 Work 聚合

原审查把 Capture Satisfaction 当成一个由单次 Attempt 直接拥有的事实，因此要求当前 entry 增加 `attemptRef`。该类比不成立。

Capture Satisfaction 回答“这个 Work 的目标目前是否满足”，它可以在未来由多次 Attempt、多个 Package 和不同 Coverage 共同形成；把一个 `attemptRef` 塞入当前标量会把未来聚合重新压成单一来源。

当前 1:1 proof 使用 `{workOrderRef, state}` 是被 SCOPE 明确限制的完整输入。未来打开 multi-attempt 时，应建立追加式 Satisfaction revision，并冻结 `sourceAttemptRefs`、`sourcePackageRefs`、评估时间、规则版本和未解决范围。最终裁定：**不成立；当前合同不改**。

## `ADV-P1-03`：五个产品职责不等于五个 Agent 实例

首页草案中的“判断守卫、缺口巡视、新说法侦测、选题猎手、战绩回收”可以先理解为五种用户可见职责或问题，不证明第一阶段需要五个常驻进程、五个独立 Agent、一个总编 Agent 或固定多 Agent 编排。

原审查正确指出：如果未来要把这些职责实现为自动夜间运行，就必须补输入材料、用途授权、Claim Ceiling、硬停止、批次失败/平静、总编规则和评测。但当前最先要验证的是产品任务和 P0 页面，而不是补写一套尚未确认的 Agent 编队架构。

最终裁定：**部分成立；先产品原型，后运行架构**。

## `ADV-P1-04`：单一工作区风险成立，不能整批提交

点时快照（本次修订开始前，仅本地、未 fetch）：

```text
HEAD:                            9801fdf5deb55e1b3fc5b8ac2c43234be295a42d
local refs/remotes/origin/main:  9801fdf5deb55e1b3fc5b8ac2c43234be295a42d
modified:                        19
untracked:                       26
total dirty entries:             45
```

风险成立：未提交的代码、最高约束、当前状态、SCOPE、审计和生成物混在一个工作区，其他 checkout 无法得到同一事实。

原处置“立即提交 19 个文件”错误，因为：

- untracked 也是交付的一部分，不能只看 modified；
- 这些改动来自多个责任和阶段，未证明应进入同一个 commit；
- 部分 HTML 属生成物，必须先核对登记和来源；
- 未 fetch，不能把本地 remote ref 宣称为 GitHub 最新状态。

正确动作是冻结新的无关写入，盘点每项来源、授权、验证和归属，拆成独立可审查提交；每个提交分别运行治理/测试并核对 push。该处置是工程治理，不需要 Mog 决定提交命令，但提交/推送仍按既有授权执行。

## `ADV-P2-01`：Bootstrap 名称已有显式映射

`module-architecture.md` 已明确：当前 `crates/evidence` 等 Bootstrap 名称不是永久 bounded context 或目标模块地图，SCOPE-001 又冻结了本切片允许的 crate 与文件位置。目标图中的 Capture 是长期责任名，不要求现在创建同名 crate。

因此现在把 `evidence` 改成 `capture` 会违反当前文件白名单并制造无业务收益的 churn。最终裁定：**已通过显式临时映射关闭；当前不改 crate**。后续只有出现真实第二调用者、事务边界或接口深度证据时，才在新 SCOPE 中决定拆分/改名。

## `ADV-P2-02`：责任完整与物理实现节奏分开

保留八个责任表达是为了防止状态坍缩，不等于第一阶段建立八个服务、八个 crate、八组表或八套状态机。当前物理实现继续遵守：

1. 当前切片有真实调用者才建；
2. 当前数据库必须证明的不变量才落约束；
3. 页面、Agent 和未来资产不提前建表；
4. Claim、Decision、Action、Outcome、隐私传播等责任保留在逻辑架构，直到对应用户切片出现；
5. 不因团队只有 5–15 人而削弱 unknown、不可变、来源追踪、幂等与围栏。

最终裁定：**规模提醒成立，物理过度设计尚未发生；不删责任，不预建结构**。

## 后续动作

1. F01 只按当前窄范围完成 synthetic fact-kernel technical tracer；
2. 新建并逐票推进 `ARC-001`，先确认产品壳、每日主任务和 P0 页面；
3. 在真实 producer/插件前完成 producer Canary 边界与 Capture Control Contract；
4. 在任何媒体 Lane 前完成 Media Lifecycle Contract；
5. 架构收口后再由 Mog 批准首个用户可见 SCOPE；
6. dirty worktree 单独盘点和拆分提交，不与本报告处置混成一次“大提交”。

## 本报告的地位

本报告是 `ADV-AUDIT-001` 的一次性证据和最终处置记录。它不取代 `AGENTS.md`、领域不变量、当前 SCOPE、ARC-001 决策图、架构基线或真实实现。后续 Agent 不得从原始风险描述中恢复已被本次最终处置否定的动作。

本轮文档修订没有修改代码、migration、合同、fixture、数据库、插件、Agent 或生产资源，没有提交或推送，也没有打开任何新实现范围。
