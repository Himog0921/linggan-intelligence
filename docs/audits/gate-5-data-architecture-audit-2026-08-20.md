# Gate 5 数据架构第一轮对抗审计

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 5 平台无关概念数据模型第一轮压力测试
> 事实来源: 已确认 Gate 1–3、Gate 4 已确认 Attempt/Package 关系、`INV-01`–`INV-60`、当前数据架构草案
> 冲突时以谁为准: 用户最新确认、权威共同语言、ACCEPTED ADR、真实 producer/fixture/测试与数据库副作用；本报告不取代 `architecture/data-architecture.md`

> 当前结论: 等待 Gate 5 逐项确认；本报告不表示数据库设计已经获批。

## 审计结论

当前候选已经能用一套较少的事实源承载观察、知识、判断、行动和结果，没有重新退化成“每个产品模块一套真相表”。第一轮对抗审查发现并修正了一个实际漂移：PRD 与数据草案曾残留统一 `Research Intent / Observation Intent` 表述，与 Gate 3 已确认的“持续观察目标和主动研究问题保持不同生命周期，不建立万能父对象”冲突。

修正以后，当前设计可以继续 Gate 5，但**尚不能确认 Gate 5 完成，也不能创建业务 DDL、migration 或 SQLx model**。剩余阻塞不是表名，而是四组需要明确采用的设计决定，以及三组必须保持来源不完整的现实合同。

## 本轮实际攻击的方法

本轮没有按文档标题检查“是否提到了某个名词”，而是把 `INV-01`–`INV-60` 分成八条失败传播链：

```text
采集失败 / 身份冲突
→ 是否污染 Evidence 与 Source Identity

迟到 / 部分 Observation / 来源差异
→ 是否污染 Current

定义、模型和导航变化
→ 是否重写知识与市场历史

研究扩围 / 授权过期 / checkpoint 恢复
→ 是否越权占用工位

Corpus、Agent 与发布物复用
→ 是否扩大原文权限或生成无来源事实

来源删除 / 隐私处置
→ 是否继续通过索引、向量、摘要和缓存传播

行动与结果
→ 是否把批准当执行、把表现当因果、把事后解释写成事前预期

重放 / 并发 / authority 失效
→ 是否产生重复、半包或越权 Evidence
```

## `INV-01`–`INV-60` 分组结果

“设计通过”只表示当前概念关系没有发现穿透，不表示代码、数据库或真实平台已经证明。`条件通过` 表示平台无关边界成立，但最终字段或资格仍依赖真实 producer/fixture。

| 案例组 | 当前结果 | 已覆盖的核心边界 | 仍需后续证明 |
|---|---|---|---|
| INV-01–03 评论、身份、聚类候选 | 设计通过 | Capture/Evidence/Observation/Analysis 分层；自述角色与模型角色分开；实验标签不进正式 Topic | 用脱敏评论 fixture 验证 unresolved identity 和候选分类 |
| INV-04–06 定义拆分、扩采假趋势、支持与反例 | 条件通过 | Topic Identity/Definition/Release 双历史；调查面不进稳定分母；Claim 保存反例和范围 | 真实可比性仍为 `SOURCE_INCOMPLETE`，首期不开放正式趋势 |
| INV-07–09 Agent 越权、结果缺失、身份错误 | 条件通过 | 委托/申请/授权/准入/执行分责；未接通结果保持 unknown；身份错误按责任分流 | producer 错误分类和 Agent 技术权限留 Gate 4/6 fixture 证明 |
| INV-10–18 隐私、模型版本、复用与 Outcome | 部分通过 | 隐私处置传播、分析版本、用途适用性、Definition 与 Map 分责、Outcome/Evaluation 分开 | 保留期限、角色、第三方处理、加密和法律边界仍 `DECISION_REQUIRED` |
| INV-19–27 发现、接入、迟到、Current 与时间 | 条件通过 | Discovery 不等于详情；Package 原子接入与 Record 隔离；replay/conflict；来源差异；字段来源 Current；时间精度 | 搜索页/API、Comment/Media 身份、真实 Coverage 和来源时钟仍需 Gate 4 实验 |
| INV-28–34 定义就绪、实验词表、裁定和 Claim | 设计通过 | 发布版本不自动切统计；Map View 独立；裁定追加；Claim 无全局真理指针 | PostgreSQL FK、唯一约束、发布事务需下一轮关系图与集成测试设计 |
| INV-35–43 研究扩围、复用、准入、探索与行动 | 设计通过 | Objective/Question 分责；Need 多对多；授权固定 revision；领取前重评估；探索预算隔离；暂停恢复重评估；Expected Outcome 事前冻结 | 资源预算单位和工位风险值等待 Gate 6 调度合同，不在 Gate 5 猜公式 |
| INV-44–55 Corpus、Signal、Agent、发布与应用 | 部分通过 | Corpus 不复制原文；片段固定版本/hash；Brief 固定 Claim revision；Content Idea 可来自人工；Agent 有后果时冻结调用 | 原文撤回后的向量/模型供应商删除回执、外部 Agent 身份与传播角色仍待收口 |
| INV-56–60 Attempt、Package、checkpoint 与并发 | 设计通过 | Work Order 0..N Attempt、Attempt 0..1 终态 Package、独立 Capture Identity、replay/conflict、checkpoint 非 Evidence、authority 原子 fence | Gate 5 下一轮给出候选关系约束；实现期用 PostgreSQL 并发/失败集成测试证明 |

## 本轮发现并修正的逻辑问题

### 0. “任务失败”曾可能连带否定合格部分结果

新增讨论中的核心修订成立：目标 100 条、因风险或页面异常只取得 50 条时，对计划目标而言执行不完整，但 50 条合格记录不应被丢弃。它们应随终态 Package、实际 Coverage 和停止原因一起接入，并按各自身份/来源资格形成 Evidence 和可能的 Observation。

需要拒绝两个过度修订：

- 不把 Evidence 描述成“页面真实，所以现实一定真实”；系统只能证明在当时条件下收到了合格来源材料；
- 第一版仍不改成一个 Attempt 多个可变 Package。部分结果通过一个冻结的终态 Package 交付，剩余范围如仍值得继续，由新的有界 Work Order / Attempt 执行。这样既保留 50 条，也避免成员、hash、authority 和 Coverage 随多次追加漂移。

这是一条“执行完整度与记录资格分离”的澄清，不改变已确认的 Work Order 0..N Attempt、Attempt 0..1 终态 Package 关系。

### 1. 统一 Research Intent 漂移

错误方向是把持续观察目标和主动研究问题做成同一父对象，再共享一套状态机。这样会让长期计划版本、一次性问题范围、完成含义和授权边界互相污染。

当前修正：

- 两者都可以说明“为什么关注”，但保持不同身份和生命周期；
- Observation 不拥有任何研究目的；目的通过多对多使用关系复用 Observation；
- Information Need 只在缺口需要跨任务追踪、影响资源或停止条件时持久化，不让简单查询制造工作流对象。

### 2. “有版本”仍可能偷偷扩大旧授权

只保存 Authorization 版本还不够。如果它引用“当前计划”，计划从 20 篇改成 200 篇以后，旧授权仍会被系统错误复用。

当前修正：Authorization 固定确切 Request / Collection Plan revision、对象范围、资源、风险、期限和条件；生成、领取和开始执行前都重新检查。暂停恢复也不能仅凭 checkpoint 绕过准入。

### 3. 冻结输入与隐私撤回的冲突

如果为了可复现永久保留完整 Material Pack，隐私撤回会失效；如果无痕删除成员，历史分析又无法解释。

当前修正：保留非敏感成员身份/hash 与历史使用审计，立即阻断当前读取；受影响 Run、Claim 和发布物标记并重评估。冻结不等于继续返回原文。

### 4. Claim 的“当前”不能成为真理开关

一个全局 `current_claim=true` 会把有范围的判断变成系统真理，也无法表达两个限定范围不同的冲突判断。

当前修正：Claim Revision 自身不拥有全局真理指针。某个用途当前采用哪一版，由 Decision 或发布关系固定；停止采用、撤回和取代保留治理历史，不自动证明它为假。

### 5. Agent 有 Evidence 输入不代表整篇输出有证据

Agent 可以在合格材料之间补写诊断、效果或因果承诺。只保存输入血缘仍会让整篇回答被错误标成“有证据”。

当前修正：新增事实性陈述必须引用已有适用 Claim、形成新的 Candidate Claim，或明确保留为不可作为正式依据的草稿。摘要、翻译与改写不能提升证据强度。

## 仍需收口的四个设计决定

### DEC-G5-01 最小 Source Identity Registry

推荐采用：一个只负责 `source system + identity namespace + object type + stable external id` 的最小注册表，外加 Author/Content/Comment 类型化身份与 Observation。它解决跨类型 Evidence、Discovery 和隐私传播的统一引用，但禁止保存任意业务属性或万能关系。

不推荐完全分裂的三套来源主键，因为 Package Record、Discovery、Corpus、Analysis Input 和隐私传播都会重复建立多态关系；也不推荐万能 Object/EAV，因为它会吞掉类型约束。

需要确认的问题：是否接受这个“极小统一身份 + 类型化业务表”的中间方案。

### DEC-G5-02 类型化 Current 混合模式

推荐采用：不可变 Observation 历史 + 有版本的 Current Resolution Run + 类型化 Current Projection；每个当前字段保存来源 Observation/Assertion 外键，冲突由追加裁定进入下一次 resolution，整份 projection revision 原子切换。

第一版不建立通用 `field_key/value` Current 表。它会让约束、SQLx 类型、查询和隐私传播都变弱。只有真实字段扩展和 benchmark 证明类型化投影无法维护时再重审。

需要确认的问题：是否接受“历史不可变、Current 可重建且字段有来源”的混合方案。

### DEC-G5-03 隐私与原始载荷参数

原则已确认，但以下参数不能由架构自行决定：

- 原始页面/响应/评论的默认保留期限；
- 到期删除还是转为长期加密保存；
- 哪些内部角色可以读取未脱敏原文；
- 是否允许任何第三方模型处理原文；
- embedding、供应商日志和缓存需要什么删除证明；
- 适用的隐私/法律审查。

在决定前，默认最保守策略仍是：内部最小访问、工作台默认脱敏、外部 Agent 不批量读取原文、不发送第三方模型。

### DEC-G5-04 统计资格的首期边界

推荐采用已经确认的产品承诺：首期只做到 L0–L3 和有捕获面边界的候选变化；不创建全局 `trend_ready`，也不因数据量增长开放 L4 趋势。统计运行仍必须冻结输入、单位、分母、时间、Coverage、来源集中、定义和模型版本。

真实 Gate 4 实验完成后，只对实验实际证明的入口、账号、producer、时间与 Coverage 范围开放比较资格，其余保持单次观察或 unknown。

## 仍不能由 Gate 5 文档解决的现实缺口

1. 搜索页、接口与作者页真实返回哪些字段、ID、顺序、终止和时间；
2. Comment/Media 身份、评论回复结构、页面/API 差异和 Coverage 是否能被 producer 证明；
3. “发布后 7 天价值衰减”的真实分布、安全访问频率和不同账号可比性。

这些继续标记 `SOURCE_INCOMPLETE`。它们不阻塞平台无关关系设计，但阻塞真实插件升级、正式比较统计和声称平台完整 Coverage。

## 下一轮 Gate 5 工作

在四个决定确认后，下一轮只做三件事：

1. 把候选记录簇收敛为关系图，明确主键、外键、唯一性和追加/投影责任，但仍不写 DDL；
2. 对 Package、Source Identity、Observation、Current、Definition Release、Decision 和 Privacy 画事务/并发失败表；
3. 用少量脱敏 fixture 设计 PostgreSQL 16 可证伪验收，不开始业务实现。

Gate 5 通过以后才进入 Gate 6：Rust 模块、AI Agent 内核、调度/插件边界、错误传播、可观测性、文件大小和依赖方向。无巨型代码文件的约束属于 Gate 6 的模块与门禁设计，不能靠提前猜表名解决。

## 第二轮关系与一致性审查追加

第一轮以后新增了两层渐进披露：

- [`../architecture/data-relations.md`](../architecture/data-relations.md)：候选基数、外键责任和类型化关系；
- [`../architecture/data-consistency.md`](../architecture/data-consistency.md)：事务、并发、重放、隐私传播和 DB-P01–P18 PostgreSQL 16 验收目录。

第二轮没有把候选对象直接变成表，而是重点攻击“为了统一而使用无法约束的多态关系”。结论如下：

1. Source Identity Registry 仍可以保留，但只能作为来源身份注册表；Author/Content/Comment 使用匹配的 typed anchor 和 typed Observation，正文/指标/Topic 不进入 Registry；
2. Claim basis、Analysis input、Privacy subject 和 Decision target 不使用核心 `(type,id)` 自由多态键，首期按真实种类使用类型化关联；
3. Observation 不做 JSON 属性大表，按对象类型与变化速度形成有限 typed family；具体字段组仍等待 Gate 4 fixture，不能由本报告猜定；
4. 网络 replay 需要独立 Ingress Delivery Occurrence/receipt 审计，但不因此产生多个 Package 或 Observation；
5. Actor、Agent Identity 与 Delegation 必须可审计；第一阶段不建设 Tenant，但也不允许调用者以自由文本冒充授权主体；
6. Current、Definition Release、Analysis Run 和 Claim Revision 均采用“先完整构建、再原子发布/冻结”，未完成版本不可被正式读取；
7. 隐私阻断必须位于所有当前读取的权威 gate，不能等待 projection、向量和缓存异步清理后才停止暴露；
8. Package authority fence 的精确时间语义仍是 Gate 6 前必须解决的真实合同：既不能接受过期越权提交，也不能把获准时间内完成但网络稍晚到达的合格材料无依据丢弃。

第二轮当前没有发现需要推翻 Gate 1–4 的结构性漏洞，但它仍不能证明 Gate 5 完成。DEC-G5-01 至 04 尚未得到用户确认，部分结果资格澄清也尚未升级为权威不变量；真实 platform identity/Coverage/time 仍为 `SOURCE_INCOMPLETE`。
