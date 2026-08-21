# SCOPE-001 Agent 执行合同

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: 任何 Agent 对 SCOPE-001 的 fixture、migration、Rust、PostgreSQL、API、worker、CLI、测试、审查和完成声明
> 事实来源: 用户确认的代码前语义冻结、SCOPE-001、领域不变量与 Agent 协作规则
> 冲突时以谁为准: `AGENTS.md`、SCOPE-001 的当前正文、可复现测试与真实 PostgreSQL 副作用；一次性审计和 `references/` 不得与其平均折中

本文件只规定 Agent 如何执行已经确认的 SCOPE-001，不复制领域模型、状态真值或数据合同。具体业务状态、fixture 正负 Oracle、允许文件和完成标准仍以 [`../plans/active/scope-001-content-evidence-vertical-slice.md`](../plans/active/scope-001-content-evidence-vertical-slice.md) 为唯一来源。

任何 Agent 在修改 SCOPE-001 范围内文件前，必须先读当前 SCOPE，再按本合同执行。目标不是消灭未知，而是消灭未经授权的合理推断：已知问题只有一个答案，未知问题有明确停止方式。

## 五条执行规则

### 1. Closed World

当前 SCOPE 明列的能力、状态、字段、文件、依赖和副作用构成封闭世界。未列出的业务能力默认是 `NOT AUTHORIZED`，不是扩展点。

当实现必须引入未列出的状态、表、migration、业务文件、crate、route、后台任务、兼容路径或外部能力时，停止受影响部分：

- 真实 producer/字段/合同不足时记录 `SOURCE_INCOMPLETE`；
- 需要产品或授权选择时记录 `DECISION_REQUIRED`；
- 纯规格遗漏先在当前 SCOPE 中给出具体场景、唯一语义和正负验收，未收口前不得用 fallback、自由字符串、万能 JSON、默认值或“先做以后改”越过。

不受影响的已授权工作可以继续，停止条件不自动扩大为全项目阻塞。

### 2. No Semantic Compression

以下责任必须分别建模、返回、测试和报告：

```text
Ingress Delivery Outcome
Package Acceptance
Attempt Terminal Reason
Capture Satisfaction
Record Processing Runtime
Record Processing Business Outcome
Observation Formation
Current Field Resolution
```

任何单一 `status`、`completed`、`ok` 或总成功布尔值都无权替代它们。某一层成功不得推导下一层成功；某一层出现缺口也不得连坐其他层已经成立的事实。

API/CLI 不得只保留一个空 envelope 再由实现者决定字段。必须使用当前 SCOPE “API/CLI envelope 与八责任外部表示”的八个必有 key、封闭 entry、合法组合、route 权限映射和 CLI 固定词。任何额外 state、reason、fallback 或字段缺省都属于合同变更，不是 adapter 自由。

### 3. Unknown Preservation

`unknown` 必须沿数据库、模块、API 和 CLI 保持 `unknown`，直到新的合格 Evidence 或明确规则改变它。

任何 adapter、默认值、序列化或展示都无权把未知转换成：

```text
0
false
[]
not_found
does_not_exist
completed
100%
```

“尚未评估”“已评估但未知”“明确为空集合”“对象不存在”和“无权读取”是不同结果。每个字段必须使用当前合同明确的表示，不能用字段缺省让调用者猜。

`not_evaluated`、`not_applicable`、Current `unknown`、Current `unresolved` 和 HTTP 404/403 不得共用 null 或空数组。无 Record 不产生 Observation Formation 实例；未 finalize 的 Record 显式为 `not_evaluated`；没有 Current revision 为 `not_applicable`；只有已发布 revision 中没有合格字段值才是 Current `unknown`。

### 4. Claim Ceiling

每层只表达本层有资格承担的主张：

| 层 | 语义天花板 |
|---|---|
| Package / Record | 系统接纳了什么合成交付原料 |
| Observation | 合格来源材料直接支持的合成对象状态 |
| Current | 当前规则从哪些 Observation 采用、保留未知或判为 unresolved |
| Analysis | 对固定输入可能意味着什么；本 SCOPE 不实现 |
| Intelligence | 为什么值得行动；本 SCOPE 不实现 |

低层字段、表名、API 文案、测试名和完成报告都不得包含 Topic、趋势、需求、机会、代表性、市场或现实平台结论。合成文本即使出现这些词，也只作为不带业务含义的 fixture 字符串。

### 5. Proof Scope Travels

任何 `verified`、`passed`、`done` 或“端到端”声明必须同时携带证明范围和未证明范围。最小格式为：

```text
VERIFIED
- contract: content-detail.synthetic.v1
- runtime: 实际验证到的 Rust/PostgreSQL/Node 版本
- scenarios: 实际通过的 F 编号与负向 Oracle
- effects: 实际核对的数据库/API/CLI 副作用
- verified_at: 实际时间

NOT VERIFIED
- real XHS producer or source exhaustion
- search/comment/media coverage
- plugin station/recovery
- Topic/Corpus/AI/Signal/Intelligence
- production deployment or market claims
```

缺少范围的“通过”视为未完成报告。后续摘要必须复制证明边界，不能只复制成功词。

## Authority 冲突处理

Agent 不得把多个冲突来源综合成平均答案。先按 `AGENTS.md` 的事实优先级裁定：

1. 可复现测试和真实数据库副作用；
2. 当前代码、migration、版本化合同和 fixture；
3. ACCEPTED 决策；
4. 权威当前文档；
5. 当前 SCOPE、一次性审计和完成计划；
6. `references/` 历史证据。

当前代码尚未实现某项语义时，不能用“代码不存在”推翻已经获准的 SCOPE；实现后代码与测试也不能静默改变 SCOPE 的产品含义。无法按顺序裁定时，停止受影响部分并使用项目既有的 `SOURCE_INCOMPLETE` 或 `DECISION_REQUIRED`，不得发明第三种模糊状态。

## 语义 Oracle

每个 fixture、模块测试和 PostgreSQL 集成场景必须同时拥有：

- **正向 Oracle**：必须出现的状态、行、关系、receipt、API/CLI 字段和来源；
- **负向 Oracle**：绝对不能出现的状态、行、推断、完成比例、默认值、额外对象或越权文案。

测试不得只断言 HTTP 200、`Ok(())`、行数大于零或 snapshot 存在。F01 主链及后续 F02–F10 硬化场景的真值与禁止副作用必须由各自的手工 fixture manifest 固定，预期值不得由被测 Rust 实现自行生成。

“数据库行数”必须覆盖当前 SCOPE 全部正式表，并按 fresh seed、ordered step delta 和 final total 固定；不得用主表计数代替 Work target、Coverage 或 processing attempt 历史。多故障 ingress 必须使用 SCOPE 的唯一判定优先级，不允许测试随便接受任一 rejectionCode。

## 代码门

只有下面五项全部为 YES，SCOPE-001 才能从语义冻结进入业务 TDD：

| Gate | YES 的可检查条件 |
|---|---|
| G1 语义唯一性 | 当前实施的 F01 主链每一层状态和数据库副作用均有唯一预期；后续 F02–F10 各自在开始时补齐唯一预期 |
| G2 禁止推断 | 未列能力、状态和字段都有停止规则，不需要 Agent 自行补全 |
| G3 负向约束 | 每个场景同时固定不能出现的错误结论和副作用 |
| G4 证据边界 | 每个读取值和业务结果都能回到其 Package/Record/Observation/规则责任 |
| G5 证明边界 | 验证报告同时列出 VERIFIED 与 NOT VERIFIED，不能膨胀为真实产品证明 |

任一项为 NO 时，代码门保持关闭；可以继续补文档、fixture 预期和测试设计，但不能创建业务 migration 或 Rust 业务实现。五项通过只打开当前合成切片，不授权任何后续能力。

### 2026-08-21 用户确认的 F01 主链优先裁定

用户于 2026-08-21 明确确认：本裁定不是 Agent 推断出的放宽。F01 先以一条可运行、可查询、合成的主链进入并完成实现；F02–F10 保留原有产品语义、正负 Oracle 和未来价值，但改为 F01 主链之后按风险逐项开启的硬化/回归场景库，不再是开始或完成 F01 主链的共同前置门。

F01 主链的唯一目标是让一份有效合成 Package 实际经过：

```text
Package ingress
→ PostgreSQL 的 Package / Record / Coverage / processing work
→ 两条 Record 独立处理
→ Source / Content / Observation / Current
→ loopback API 读取
→ CLI 经 API 读取
```

F01 主链必须同时保留以下不可协商保护，且均须在真实 PostgreSQL proof 中证明：

1. accepted ingress 只建立接入事实和 processing work，不能提前形成 Observation 或 Current；
2. Package 接入事务任一点失败不得留下半写的 delivery、Package、Record、Coverage 或 processing work；
3. 同一 Package 中坏 Record 的处理结果不得撤销合格 Record 的 Observation/Current；
4. Package accepted、Record processing、Observation 与 Current 继续分别表达；unknown 不得变成 `0`、空值或 completed；
5. 不可变 hash、replay 与 conflict 不得静默覆盖既有事实。

这只改变 F01 的实施顺序和 F01 主链完成口径，不删除 F02–F10，不降低其未来回归价值，也不授权真实 producer、Raw Artifact、插件、AI、Topic/Corpus/Intelligence 或生产。SCOPE-001 整体不得因 F01 主链通过而宣告完成。

### SCOPE-001 的当前开工裁定

2026-08-20，四轮独立只读审查已经把实现歧义逐层压缩；第四轮的 4 个 P1 已按当前 SCOPE 最小修订。用户随后明确要求停止重复的文档复核循环并尽快进入代码阶段。因此当前门为 **CONTROLLED OPEN FOR TDD**，不再把第五轮独立文档审查作为前置条件。

这不是把历史 FAIL 改写为 PASS，也不降低五条执行规则。实施必须从失败的 F01 合同测试开始；代码、PostgreSQL 约束、API/CLI snapshot 和数据库副作用会成为下一层可复现证据。新发现若只是已确认语义的技术表达缺口，Agent做最小修订并继续；若会改变产品含义、权限、F01 主链边界、后续 F02–F10 的既定预期、真实数据/平台/插件/AI/生产范围，则必须停止受影响部分并使用 `DECISION_REQUIRED`。
