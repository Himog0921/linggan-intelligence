# PostgreSQL 概念关系与约束

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 5 的候选持久关系、基数、引用完整性、发布切换和并发失败边界
> 事实来源: 已确认 Gate 1–3、Gate 4 已确认 Attempt/Package 关系、领域不变量与 `data-architecture.md`
> 冲突时以谁为准: 用户最新确认、`data-architecture.md`、权威共同语言、ACCEPTED ADR、真实 producer/fixture/测试与数据库副作用

本文是 [`data-architecture.md`](data-architecture.md) 的下一层渐进披露。父文档回答“为什么这样分”，本文回答“这些责任怎样相连”。它不是最终表名、DDL、migration、SQLx struct、crate 或接口合同；仍为 `SOURCE_INCOMPLETE` 的平台字段不会在这里被猜成列。

## 关系设计的四条硬规则

1. **稳定身份、不可变版本、一次运行、正式发布和读取投影分别建模。** 不使用一个 `status/version/type/payload` 万能表。
2. **有业务约束的关系必须能由 PostgreSQL 约束。** 核心引用不用 JSON、自由文本 ID 或无法校验的多态键。
3. **共享审计模式不等于共享领域父表。** 持续观察与主动研究、各种 Proposal/Decision、Author/Content/Comment 可以共享代码模式，但不因此共享万能生命周期。
4. **本文先定义关系责任，不冻结物理拆表数量。** 一组紧密事实可以在真实合同证明后合并；具有不同身份、时间、权限或失败边界的事实不能为了少表强行合并。

## 总体关系图

```text
Observation Objective ─┐
                       ├─→ Information Need ←─ Research Question
                       │          │
                       │          ├─ Need Assessment
                       │          └─ Acquisition Request Revision
                       │                       │
Collection Plan Revision ───────────────→ Authorization
                                               │
                                           Admission
                                               │
                                           Work Order
                                               │ 1:N
                                               ↓
                                            Attempt
                                               │ 0:1 terminal
                                               ↓
                                        Capture Package
                         ┌─────────────────────┼─────────────────────┐
                         ↓                     ↓                     ↓
                     Record               Artifact           Coverage Fact
                         │
                         ├─→ Discovery Entry ─→ Source Identity
                         └─→ Observation Evidence Use
                                          │
                                          ↓
                                 typed Source Observation
                                          │
                 ┌────────────────────────┼────────────────────────┐
                 ↓                        ↓                        ↓
          typed Current             Corpus Selection       Analysis Input Set
          Projection                      │                        │
                                         Material Pack        Analysis Run
                                                                  │
                                         ┌────────────────────────┼──────────┐
                                         ↓                        ↓          ↓
                                  Classification Result       Cluster     Claim Revision
                                         │                                  │
                                  Human Adjudication              Decision / Brief Release
                                                                            │
                                                                    Action → Outcome
```

箭头只表示可追溯关系。普通浏览、失败采集、一次简单字段读取或被证伪的 Candidate 不需要机械走完整链。

## 一、研究与采集控制关系

### Actor、Agent 与委托

第一阶段不建设 Tenant，但所有有后果的申请、授权、读取、决定和执行都必须引用可审计 Actor，而不是客户端提交的一段姓名：

- Human Actor 表达实际操作人；
- System Actor 表达有固定代码/服务身份的内部自动运行；
- Agent Identity 表达被调用的 Agent/版本，不自动拥有调用者权限；
- Delegation Grant 固定委托人、Agent、Domain、目的、允许读取/分析/申请的能力、预算、期限和传播范围；撤回与过期追加记录。

一次 Agent Invocation 同时引用调用者 Actor、Agent Identity 和适用 Delegation revision。未来多租户若成立，再通过独立 ADR 给这些身份增加隔离边界；首期不预建组织、套餐或跨租户共享。

### 持续观察目标与主动研究问题

- `Observation Objective` 是长期身份；语义改变时追加 revision，不与 `Research Question` 共用父对象。
- `Research Question` 固定一次问题的范围；从“最近 30 天表达”扩大到“三年市场机会”必须新建问题或明确的新 revision/派生关系，旧问题不漂移。
- `Collection Plan Revision` 可以服务一个或多个目标/问题，但计划说明“怎样观察”，不是目标、问题或实际运行。
- 同一 Observation 可以被多个目标、问题和 Need 使用，使用关系记录后来为何复用，不回写原采集动机。

### Information Need

只有需要跨任务追踪、决定资源或限制研究结论的缺口才获得稳定 Need 身份。关系为：

```text
Objective / Question 1:N Information Need
Information Need 1:N Need Assessment
Information Need N:M Evidence / Observation / Analysis Result（满足候选）
Information Need N:M Work Order（执行贡献）
```

Need Assessment 是一次有时间和规则版本的判断，不覆盖 Need。它分别说明材料取得、用途适用、充分性和是否继续；Work Order `completed` 不自动关闭 Need。

### Request、Authorization、Admission 与 Work Order

- Acquisition Request 使用不可变 revision；扩大对象、深度、Coverage、成本或传播范围产生新 revision。
- Authorization 固定确切 Request / Collection Plan revision、对象边界、期限、资源、风险和附加条件；撤回、到期和替代追加记录。
- Admission 是一次执行前判断，引用当时 Need Assessment、已有 Evidence 边界、等价在途工作、授权、时间价值、工位/账号能力和风险状态。
- 一个 Admission 可产生多个有界 Work Order；一个 Work Order 可以贡献给多个 Request/Need，但使用 join 关系保留各自用途和满足评估。
- Work Order 只能引用仍适用的 Authorization/Admission。领取与开始执行时使用条件更新或等价原子检查，不能先读后无条件占用工位。

不建立万能 `ResearchIntent`、`RequestItem` 或跨领域 `WorkflowStatus`。

## 二、Capture / Evidence 关系

### 确认基数

```text
Work Order 1:N Execution Attempt
Execution Attempt 0:1 terminal Capture Package
Capture Package 0:N Capture Record
Capture Package 0:N Artifact Reference
Capture Package 0:N Coverage Source Fact
Execution Attempt 0:N Ingress Delivery Occurrence
Ingress Delivery Occurrence 1:1 Receipt / Outcome
```

Attempt 使用服务端唯一 Capture Identity。一个终态 Package 冻结成员与 canonical hash；目标未全部完成仍可提交合格部分和真实 Coverage。剩余范围若继续，建立新的有界工作，而不是修改原 Package。

`Ingress Delivery Occurrence` 只记录一次实际到达服务端的提交/重传及安全元数据。第一次合格交付建立终态 Package；同身份同 hash 的后续 Delivery 是 replay，并返回同一业务接纳结果但保留必要重传审计；同身份不同 hash 是 conflict。不能用“一份 Package 只有一个 receipt”抹去真实重传，也不能让每次网络重传都创建新 Package。

### Package 接入

Package 的 envelope、Record/Artifact 元数据、Coverage/终态来源事实、canonical/hash、authority fence、接入结果和 durable work 同一事务写入。接入结果至少区分：

- 首次合格接纳；
- 同 Capture Identity、同 canonical 内容的 replay；
- 同身份、不同内容/hash 的冲突；
- authority/目标/最低 envelope 合同不合格；
- 完整载荷按隐私政策进入受限隔离或只保留安全失败元数据。

Package 原子接纳以后，Record 解析、来源身份归属和 Observation 生成按 Record / Source Object 隔离。Record 的结构/身份问题不会回滚其他已接纳来源材料，也不能因为 Package 接纳而自动获得 Observation 资格。

### Artifact

大型原件由受限对象存储保存，PostgreSQL 只保存内容地址、hash、大小、类型、加密/保留状态和主体关系。一个 Artifact 可以支持多个 Record，但必须通过明确关联表说明使用位置；URL 被观察到、字节下载、hash 验证和对象持久化是不同事实。

## 三、来源身份、发现和 Observation

### 最小 Source Identity Registry

候选唯一键：

```text
(source_system, identity_namespace, source_object_type, stable_external_id)
```

Registry 只解决跨 Evidence、Discovery、Corpus、Analysis 和隐私处置的稳定引用。它与类型化锚点形成受限一对一：

```text
Source Identity 1:0..1 Author Identity
Source Identity 1:0..1 Content Identity
Source Identity 1:0..1 Comment Identity
```

一个已解析 Registry identity 最终只能拥有与其 `source_object_type` 匹配的一个类型化锚点。并发解析依靠唯一约束收敛；身份合并/拆分通过显式修订和别名/血缘记录，不更新主键伪造历史。

这不是万能 Object：正文、昵称、指标、Topic、任意属性和任意关系均不得进入 Registry。

### Discovery

一次发现执行拥有零到多条 Discovery Entry。Entry 引用确切 Capture Record/Artifact 位置，保存入口、query/filter/作者、顺序或排名、时间、观察身份与 Coverage，并且：

- 可以解析到一个 Source Identity；
- 或保持 unresolved，保留原因和后续 Resolution；
- 同一 Source Identity 可以有任意多条 Entry；对象去重不删除 Entry。

接口返回、页面可见和详情访问使用不同 discovery/capture lane，不通过一个字段互相覆盖。

### 类型化 Observation 家族

不建立一张带 JSON 属性的万能 Observation。第一版候选按真实合同和变化速度分成有限家族，例如：

- Author Profile Observation；
- Content Detail Observation；
- Comment Observation；
- Metric Observation；
- Availability / Deletion Source Assertion。

具体字段分组等待 Gate 4 fixture。原则是：同一组字段必须共享可解释的来源位置、观察时间和修订方式；正文与高频指标、内容状态与隐私处置不能为了少表塞进同一生命周期。

每条类型化 Observation：

- 只指向匹配类型的 Source Identity；
- 引用一份或多份 Accepted Record/Artifact 使用关系；
- 保留实际 observed time、来源时间原文/精度和 Observation Context；
- 允许部分字段和 unknown；
- 不包含 Topic、角色推断、需求、趋势或机会。

评论者自述角色作为来源断言留原文；AI 推断角色属于 Analysis Result。缺少稳定 Comment ID 的文本保持 unresolved material，不制造 Comment Identity。

### 解析修订与来源差异

Interpretation Revision 引用同一 Evidence 和前一解释版本；parser 修复不生成新世界 Observation。API/页面或多个合格来源给出不同值时，各自 Observation/Assertion 并存，通过 discrepancy 关系进入 Current Resolution，不互相覆盖。

## 四、Current 关系

Current 不使用一个跨类型表，也不让最后写入成为当前。每类核心来源对象采用：

```text
typed immutable Observations
→ Current Resolution Run
→ typed Current Projection Revision
→ atomic active projection pointer
```

类型化 Projection 的每个可独立选择字段或字段组保留来源 Observation/Assertion 外键。例如正文、作者关系、来源发布时间和指标可以来自不同合格 Observation，但页面必须说明这是当前组合读取，不伪装成一次完整快照。

关系约束：

- Resolution Run 固定规则版本和输入水位；
- Adjudication 追加冲突选择与理由，不覆盖来源；
- Projection revision 完整构建后才原子切换；
- 每次 resolution 在选择值之前应用当前访问/隐私资格；已处置材料不能因为仍是历史最新值而继续进入 Current；
- 无合格来源时字段保持 unknown；
- 迟到 Observation 进入历史并触发重算候选，不按 `accepted_at` 自动倒退 Current；
- 投影可删除重建，任何独有业务事实都不能只存在 Projection。

第一版不使用 `field_key/value` EAV。字段/字段组的最终物理拆分由首个 Content/Comment fixture 和查询基准确认。

## 五、领域语言与发布关系

```text
Domain 1:N Topic Identity
Topic Identity 1:N Topic Definition Revision
Topic Definition Revision N:M Term（带语言、语境与 relation kind）
Topic Identity N:M Topic Identity（通过 Knowledge Relation Revision）
Domain Definition Release N:M exact Definition Revision
Domain Definition Release N:M exact Knowledge Relation Revision
Domain 1:0..1 active Definition Release pointer
```

- Release manifest 成员冻结；最后创建的 revision 不自动进入当前发布。
- broader/narrower 等层级关系在同一 Release 内必须通过数据库/发布前验证避免自环和循环；普通 related relation 不被误当层级。
- Topic 拆分/合并通过 typed lineage 连接旧、新身份；旧 Claim、分类和统计不自动继承。
- Map View Release 单独引用 Topic/Definition，不修改知识关系。
- 新 Definition Release 与分类/统计 ready 状态分开；读取端不能把新标题与旧结果拼接。

## 六、分析、分类和 Claim 关系

### 冻结输入与运行

```text
Analysis Input Set 1:N exact Input Member
Analysis Run N:1 Input Set
Analysis Run N:1 Definition Release 或 Experimental Vocabulary Snapshot
Analysis Run 1:N typed Result
```

Input Member 按分析目的使用类型化成员关系，可以引用不可变 Discovery Entry、Evidence/Coverage 来源事实、Observation、Coverage Evaluation、受版本控制的 Transformation/Fragment，或上述对象的内容寻址 manifest 成员；不能引用会变化的 Current 查询。大规模 manifest 必须可验证 hash 和成员计数；隐私处置后保留最小成员审计但阻断原文读取。不同成员类型不得靠一个无外键 `(type,id)` 混装，实际 Run 明确自己接受哪些输入合同。

Analysis Run 可以分批写入暂存结果，但只有输入边界、模型/规则/参数、实际结果、Coverage 和失败共同冻结后才能 finalize。Claim、Brief 或正式统计不能引用未 finalize 的 Run；失败 Run 和诊断结果仍保留。

### 分类和人工裁定

Classification Result 引用 exact input member、Topic Definition revision 和 Run。Human Adjudication 追加对哪一结果/材料采用什么结论、理由和依据；机器结果不被覆盖。新定义发布不自动搬迁旧裁定。

### Claim

```text
Claim Identity 1:N Claim Revision
Claim Revision N:M typed Evidence Record basis
Claim Revision N:M typed Observation basis
Claim Revision N:M typed Analysis Result basis
Claim Revision N:M Material Fragment basis
Claim Revision N:M other Claim Revision（supports/challenges/qualifies）
```

不用一个无法校验外键的万能 `claim_basis(type,id)`。不同 basis 使用类型化关联；关系保存用途、支持/挑战方向和限制。

Claim Revision 的主张、范围、时间、依据、反例和不能证明什么共同冻结。某个用途当前采用哪一版，由确切 Decision 或 Brief release 固定；Claim 本身没有全局 `is_true/current`。

## 七、Corpus、应用资产与 Agent

Corpus 不复制原文：

```text
long-lived Material Collection（只有真实长期用途时才存在）
→ Selection Revision
→ Member / exact Fragment
→ Transformation lineage

Retrieval Run
→ frozen Material Pack（产生后果时）
→ Material Usage
```

- Fragment 固定来源 Observation/Evidence version、范围和 hash；来源编辑产生新候选，不移动旧引用。
- Transformation 记录脱敏、翻译、转写或摘要的输入、输出和工具版本；输出不能提高 Evidence 强度。
- 曾被选择不产生永久读取许可；访问时重新做 purpose-bound access decision。
- Market Insight / Intelligence Brief release 固定 exact Claim revisions、Material Pack、反例、限制和正文，不复制 Claim 当前值。
- Content Idea 拥有独立 identity/revision；人工经验可以如实进入，不伪造 Signal。
- 有后果的 Agent Invocation 固定调用者委托、目的、授权、输入包、定义/Claim/模型版本、输出与真实使用；无后果浏览只留必要的受限访问审计。

## 八、Decision、Action、Outcome 与隐私

### Decision 不建立万能目标外键

第一版使用有边界的 typed decision relationship：Knowledge Change Decision、Acquisition Authorization/Decision、Content Action Decision 等分别指向自己的 exact proposal/request/plan revision。它们可以共享 Actor、时间、理由、选项和审计结构，但不通过无法约束的 `(subject_type, subject_id)` 假装数据库完整性。

### Action 与结果

```text
Content Idea Revision
→ Action Decision
→ Action Plan Revision + Expected Outcome
→ 0:N Action Attempt
→ 0..N qualified Action Occurrence
→ 0..N Outcome Observation
→ Evaluation Revision
```

Expected Outcome 必须在对应 Occurrence 前冻结。Action Attempt 失败也保留；只有平台回执或有来源边界的人工陈述能证明现实动作发生。未接入、未观察、窗口未结束和来源不合格保持 unknown，不创建数值零。

### Privacy Case 与传播

Privacy Disposition 使用一个 case identity 和类型化 subject link 指向 Evidence/Artifact、Source Identity 或 Material/Publication；不使用万能可读对象，也不把来源不可访问当隐私删除。

处置事务共同写入：Disposition Event、立即访问阻断和 durable propagation work。后续沿已登记 lineage 处理 Fragment、全文索引、embedding/feature、Transformation、缓存、Material Pack 和 Agent/Brief 引用；完成回执逐项保存。历史用途只保留非敏感最小 tombstone/hash、影响说明和重评估状态。

## 九、必须由数据库证明的并发约束

| 场景 | PostgreSQL 必须证明 | 不能只靠什么 |
|---|---|---|
| 并发解析同一来源对象 | 唯一来源业务键只产生一个 Source Identity | Rust 先查再插 |
| 同 Capture Identity 重传 | 相同 hash 幂等返回同一 receipt；不同 hash 冲突 | API 返回 `ok` |
| authority 在写入前失效 | authority fence 与 Package 接入原子失败关闭 | 事务外提前查询 |
| 部分 Package 接入失败 | envelope/成员/coverage/receipt 不出现半包成功 | worker 事后补偿 |
| Record 下游归属失败 | 只隔离相应 Record/Object，其他已接纳 Evidence 不丢失 | 整包回滚或整包成功状态 |
| 两次 Current resolution 同时发布 | 只有完整 revision 能成为 active，来源外键一致 | `updated_at` 最大值 |
| 两个 Definition Release 同时发布 | Decision 固定的完整 manifest 只有一个成为当前 | 最大 version |
| Claim/Brief 构建中途失败 | 未冻结版本不可被正式引用 | 页面隐藏按钮 |
| 隐私处置与读取并发 | 处置成功后新读取立即被阻断，传播任务不丢 | 异步缓存过期 |
| Action 结果晚到或重复 | 幂等来源身份保留真实时间，不回填预期 | UI 去重 |

## 十、拒绝的数据库捷径

- 万能 `Object / Asset / Entity / Relation / Property`；
- 所有对象共用一个 `status + version + payload JSONB`；
- `subject_type + subject_id` 但没有可验证外键的核心关系；
- 用 Domain、workspace 或 Topic 参与来源身份唯一键；
- 用 `latest row`、最大指标或最后写入作为 Current；
- 用一个 `confidence`、`evidence_score`、`coverage_percent` 或 `trend_ready` 代替用途资格；
- 把查询文本当可复现输入集合；
- 把 Corpus、Brief、Topic Map、Radar 或 CLI 缓存作为第二事实源；
- 为了未来规模提前分区、Kafka、微服务数据库或图数据库；
- 为了少表把不同时间、权限和失败边界压进巨型行。

## 十一、进入物理模型前的历史清单与当前分流

下列问题中的 Source Identity、Content Observation、title/body Current、L0–L3 承诺和合成 Package/Record 子集已由 `USER-DEC-03/04` 与正式 SCOPE-001 裁定；其余真实字段、隐私、分析输入、Decision/Claim 仍后置，不能由首切片合成模型外推。

1. 最小 Source Identity Registry + 类型化锚点是否采用；
2. 类型化 Observation 的首个真实字段组，由 Gate 4 哪份 fixture 证明；
3. 类型化 Current 混合模式是否采用，以及首切片哪些字段需要 Current；
4. 原始载荷保留、内部角色、第三方模型和删除传播参数；
5. 首期统计仍停在 L0–L3、正式 L4 趋势冻结的产品边界；
6. 分析输入逐行成员与内容寻址 manifest 的切换基准；
7. typed Decision/Claim basis/privacy subject 的首期实际种类，避免预建未用表。

首个候选物理表簇和 PostgreSQL 验收矩阵只存在于正式 SCOPE-001；完整业务模型仍需后续切片逐步证明，不从本文直接生成。
