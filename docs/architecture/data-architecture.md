# 数据架构与 PostgreSQL 概念模型

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 5 的数据分类、身份、关系、版本、Current、生命周期、隐私传播、统计资格与事务边界
> 事实来源: 已确认的 Gate 1–3 领域语义、Gate 4 已确认的 Attempt/Package 最小关系、固定 producer 审计和领域不变量
> 冲突时以谁为准: 用户最新确认、ACCEPTED ADR、真实 producer/fixture/测试与数据库副作用；本草案不得覆盖仍为 `SOURCE_INCOMPLETE` 的平台事实

本文是 Gate 5 的候选数据设计，不是 DDL、migration、SQLx model、Rust 模块清单或开工授权。它先回答“哪些历史必须保存、哪些解释可以重算、哪些当前值只是读取结果”，再决定 PostgreSQL 如何承载；不从页面字段、旧 Prisma schema 或 Bootstrap crate 倒推表。

需要继续查看候选基数、外键责任和类型化关系时，进入下一层 [`data-relations.md`](data-relations.md)；需要查看并发、重放、事务与 PostgreSQL 16 可证伪验收时，再进入 [`data-consistency.md`](data-consistency.md)。两份子文档同样是草案，不得绕过本文的待确认决定直接生成数据库表。

Gate 4 的七条逐 lane producer 约束和实验/冻结政策已随 `USER-DEC-01` 确认，部分数据保存与使用规则已随扩展 `USER-DEC-02` 确认。真实实验尚未执行，因此搜索可比性、完整 Coverage、正式趋势、7 天衰减、评论全量和安全访问频率继续保持 `SOURCE_INCOMPLETE`；确认设计纪律不等于平台事实已经被证明。

## 一句话结论

> **一个 PostgreSQL 主数据库保存少量有明确所有权的事实历史、版本历史和运行历史；Current、Topic Map、Radar、市场统计、Corpus 检索和 Agent 回答均从这些历史生成，不成为第二事实源。**

这不是全量 Event Sourcing，也不是把所有东西塞进万能事件表。只有需要追溯、纠错、并发保护或隐私传播的变化追加保存；稳定业务对象仍使用有约束的关系模型。

## 数据库中的五种身份形态

任何候选对象先判断它属于哪一种形态，不能都用一个 `status + version` 模板：

| 形态 | 回答什么 | 示例 | 允许怎样变化 |
|---|---|---|---|
| 长期身份 Identity | “长期指的是谁或什么” | Source Identity、Topic Identity、Claim Identity、Content Idea Identity | 身份本身稳定；改名、解释和状态不覆盖身份 |
| 不可变版本 Revision | “当时具体是什么内容” | Topic Definition Revision、Claim Revision、Idea Revision、Interpretation Revision | 新建后继版本，旧版本继续可引用 |
| 发布集合 Release | “某次正式采用了哪些版本” | Domain Definition Release、分析发布物版本 | 由明确 Decision 冻结成员；不由最大版本自动成为当前 |
| 运行 Run / Occurrence | “一次执行或现实观察实际发生了什么” | Execution Attempt、Analysis Run、Action Attempt、Outcome Observation | 每次运行独立追加；失败也保留 |
| 可重建投影 Projection | “现在为了读取采用什么” | Source Current、Topic Map、Radar、市场聚合 | 可以替换和重建，但每个值必须回到权威历史与选择规则 |

一条数据如果无法说明自己属于哪一种形态，就暂不创建物理模型。尤其禁止建立一个万能 `Asset`、`Object`、`VersionedEntity` 或 `WorkflowItem`，再用 JSON 和状态值容纳所有业务。

## 数据所有权区域

下面是逻辑所有权，不预设 PostgreSQL schema、Rust crate、微服务或部署单元。Gate 6 再决定物理模块。

| 区域 | 负责写入什么 | 明确不负责什么 |
|---|---|---|
| 研究与采集控制 | Information Need、Request、Authorization、Admission、Work Order、Attempt、lease/checkpoint 运行状态 | 不宣布 Evidence 成立，不写 Topic 或市场结论 |
| Capture / Evidence | Capture Occurrence、Package、Record、Artifact、Coverage 来源事实、接入回执、冲突与隔离 | 不决定材料对某个 Claim 是否适用，不把包成功当对象观察成功 |
| 来源身份与 Observation | 稳定来源身份、每次 Discovery、类型化 Observation、来源时间陈述、解析修订和来源差异 | 不写 AI Topic 分类、需求、趋势或机会 |
| 领域语言 | Domain、Topic Identity、Definition Revision、Term/Alias、Knowledge Relation、Definition Release | 不把共现、相似、搜索命中或市场变化写成正式知识关系 |
| 分析与判断 | 冻结输入集合、Analysis Run、分类/聚类/统计结果、Human Adjudication、Claim revision、支持与反例 | 不修改 Evidence、Topic 历史或采集计划，不因高置信自动正式发布 |
| 决策、行动与学习 | Proposal、Decision、Action Plan/Attempt/Occurrence、Expected Outcome、Outcome Observation、Evaluation | 不把批准当执行，不把结果当因果，不直接回写 Topic/Claim |
| 使用、隐私与传播 | Corpus 选择、转换血缘、调用/材料使用、访问裁定、处置事件和影响传播 | 不复制第二份原文，不以“曾允许”代替当前用途资格 |
| 读取投影 | Current、Topic Map、Radar、市场聚合、搜索索引、页面和 CLI 读取模型 | 不拥有权威事实，不接受业务方直接写入 |

第一阶段是 Mog 与小团队使用的单一产品，不为未来客户提前建设多租户 SaaS。数据库仍记录实际 Actor、Agent 调用者、委托和访问审计，但不因此创建 Tenant 套餐、跨租户共享、计费或复杂组织层级。若以后商业化，多租户是需要独立 ADR、隔离迁移和安全验证的新决定。

当前唯一正式 Domain 是 ADHD。Domain 表达长期研究边界，不参与来源对象身份，也不复制一套“ADHD 版 Note/Author/Comment”。同一 Source Identity 可被多个持续观察目标、主动研究问题、Topic、Corpus Selection 和分析运行引用；未来新增 Domain 时，先建立显式归属/使用关系，不修改来源身份键。

### 候选持久记录簇，不是最终表清单

下表只用于检查责任是否遗漏或重叠；一个记录簇未来可能拆成多张表，也可能由少量紧密关系共同表达。Gate 5 未确认前，禁止把它直接翻译为 migration。

| 记录簇 | 需要长期保存的最小责任 | 不能塞进去的内容 |
|---|---|---|
| Research / Control | Objective/Question revision、Information Need、Assessment、Request、Authorization、Admission、Work Order、Attempt 与运行租约 | Evidence、Topic 分类、市场判断 |
| Capture / Evidence | Package、Record、Artifact 引用、接入资格/回执、Coverage 来源事实、冲突与隔离 | Current、需求、趋势、AI 结论 |
| Source | Source Identity、Discovery、类型化 Observation、来源时间陈述、解析/身份修订 | Topic 定义、Claim、内容机会 |
| Knowledge | Domain、Topic Identity、Definition Revision、Term/Alias、Knowledge Relation、Definition Release | 共现强度、聚类、搜索命中 |
| Analysis | 冻结输入、Run、分类/聚类/统计结果、裁定、Claim Identity/Revision 与证据关系 | 原始载荷、正式 Topic 写入、采集授权 |
| Application | Corpus Selection/Fragment/Transformation、Material Pack/Usage、Brief/Insight 发布物、Content Idea revision、Agent Invocation | 第二份原文、无来源事实、现实动作结果 |
| Decision / Learning | Proposal、Decision、Action Plan/Attempt/Occurrence、Expected Outcome、Outcome Observation、Evaluation | 把批准伪装成执行、把结果伪装成因果 |
| Privacy / Access | purpose-bound access decision、Disposition、传播任务、最小 tombstone 与审计 | 业务内容副本、万能永久可用标签 |
| Projection | Source Current、Topic Map、Radar、统计和搜索读取模型 | 任何删除后无法恢复的唯一业务含义 |

## 核心关系图

```text
Observation Objective / Research Question
                    │
                    ↓
             Information Need
                    │
       Request → Authorization → Admission
                    │
                    ↓
                Work Order
                    │ 0..N
                    ↓
          Execution Attempt 0..1 Capture Package
                    │                    │
                    │                    ├─ Capture Record
                    │                    ├─ Artifact reference
                    │                    ├─ Coverage source fact
                    │                    └─ terminal / failure fact
                    │
                    ↓ accepted atomically
             Accepted Evidence
                    │
       ┌────────────┴─────────────┐
       ↓                          ↓
Source Identity / Discovery   unresolved material
       │
       ↓
typed Observation + source assertions
       │
       ├────────→ Current Resolution → rebuildable projection
       │
       ├────────→ Corpus selection / transformation
       │
       └────────→ frozen Analysis Input Set
                               │
                               ↓
                         Analysis Run
                               │
                  ┌────────────┴────────────┐
                  ↓                         ↓
          classification / cluster       Claim Revision
                                                │
                                 Decision → Action → Outcome
```

箭头表示可追溯关系，不表示每份材料必须走完全部流程。

## Capture 与 Evidence 的持久边界

### Work Order、Attempt 和 Package

- Work Order 是获准执行的有界工作，不把整个研究问题交给插件。
- 一个 Work Order 可以有零到多次 Attempt；每次 Attempt 使用独立服务端身份。
- 第一版一个 Attempt 最多一个终态 Package；复杂工作拆 Work Order，不让同一 Package 的成员不断变化。
- Attempt 对计划目标“不完整、中断或失败”，不等于本轮已经合格观察到的记录无效。目标 100 条、实际取得 50 条时，终态 Package 可以冻结这 50 条实际记录以及其余范围的失败/未尝试/unknown Coverage；合格记录按正常接入进入 Evidence，Package 不能在稍后继续追加到 100 条。
- “目标 100”必须能区分已知对象集合、最大配额、来源穷尽、时间预算、风险预算或探针目的。只有执行前已冻结 100 个具体目标身份，才可能把未访问成员记为已知 `not_attempted`；最大配额只说明上限，取得 50 条时剩余平台范围仍为 unknown，不能机械写成还有 50 个对象。
- lease 和 checkpoint 是可变运行状态；它们不进入 Evidence 身份，也不能证明 Coverage 或来源耗尽。
- 相同 Capture Identity 与相同内容是 replay；不同内容是冲突。网络重传不产生新 Observation。

### Package 原子性

一次 Package 接入事务至少共同保护：

1. authority 仍然有效；
2. Attempt、目标、合同版本与 Capture Identity 匹配；
3. canonical bytes 与 hash 一致；
4. replay 或 conflict 判定；
5. Package、Record、Artifact 元数据、Coverage/终态来源事实和接入回执；
6. 后续处理所需的 durable work / outbox。

任一接入硬门失败，不能只保存半包后返回成功。接入硬门应限制在 authority、Attempt/Capture Identity、目标/合同、canonical/hash、manifest/成员边界和最小可重放 Record/Artifact envelope；平台字段语义、Source Identity 归属和 Observation 资格尽量后置逐 Record 处理，避免一条平台字段异常连坐其他可保存原料。接纳以后，单条 Record 的来源身份解析、Source Object 归属、Observation 或派生失败按 Record / Source Object 隔离，避免一条坏记录阻塞整包的合格记录。

这里的“全包原子”指 producer 这次交付的冻结 Package、实际 Record、Coverage/终态事实与接入回执不能只写一半；它不要求 Work Order 达到原计划数量，也不承诺每条 Record 后续都能形成 Source Identity 或 Observation。执行完整度与单条记录资格是两条判断轴。

错误责任不能被一个 `identity_conflict` 吞并：Work Order/Attempt 目标错配、Package Capture Identity/hash 冲突、Record 内部来源关系冲突、Media/Artifact 主体错配和下游 Source Identity 归属失败分别记录，因为它们决定整包拒绝、幂等 replay、单 Record 隔离、重新采集还是修复 parser。具体错误码等待真实 fixture 与 Gate 6 合同确认，但责任边界现在固定。

### 原始材料与结构化记录

- PostgreSQL 保存 Package/Record 的身份、canonical 表达或可验证引用、hash、来源、时间、合同和访问元数据。
- 大型页面快照、完整响应和媒体字节进入受限对象存储；PostgreSQL 保存内容地址、hash、大小、类型、加密/保留状态和主体关系。
- JSONB 只用于保存 producer 原样稀疏字段、受控扩展和无法提前固定的原始结构，不用于核心身份、关系、状态或统计分母。
- 截断 JSON、拼接 DOM 文本和规范化 current row 可以作为诊断材料，不能代替可重放原件。
- 无效 Package 至少留下不含多余敏感内容的发生、失败、Coverage 与冲突审计。完整无效载荷是否进入隔离区由保留政策决定，不默认永久保存。

## 研究、需求、授权与执行控制

研究问题和持续观察目标不是采集任务。它们先形成有版本的 Objective / Question，再由 Information Need 说明“还需要知道什么”。Need Assessment 每次回答：现有 Evidence 是否已足够、缺什么、允许多旧、需要何种 Coverage、是否只是有限探索，以及继续获取的预计资源和风险。

只有 Assessment 证明需要真实访问后，才能依次形成 Request、Authorization 与 Admission；它们分别表达请求了什么、谁允许到什么边界、此刻在资源/风险/去重条件下是否值得执行。Work Order 只承载获准后的最小可执行单元，不能倒过来充当研究目的或授权凭据。

- 多个 Information Need 可以共享一次 Work Order 和同一份 Evidence，但每个 Need 的用途、充分性与停止结论分别保存，不能因“详情已采”让所有研究自动完成。
- Authorization 固定被批准的 Request / Collection Plan revision、对象范围、资源、风险、期限和附加条件；修改计划或扩大到新版本必须重新决定，不能让“当前计划”漂移后继续复用旧授权。
- Admission 在生成工作、领取和真正开始前都要验证所依据的 Need、Evidence 缺口、等价在途工作、授权和资源条件仍成立；若已满足、过期或风险变化，Work Order 明确取消/过期，不因曾入队就必然访问平台。
- 已有 Evidence、进行中的 Work Order、相同 Capture Identity replay 和真正有意复观测分别判断；对象相同不等于需求相同，需求不同也不等于必须再次访问平台。
- 探索驱动的深采只能使用明确、有限且可审计的探索预算；其结果不能进入稳定观察面的分母，也不能自动扩大下一轮探索预算。
- 暂停后的恢复先重新评估问题范围、新 Evidence、授权、时间价值和资源；运行 checkpoint 只辅助同一 Attempt 的技术恢复，不能跳过重新准入或把旧未接纳材料改挂到新 Attempt。
- 研究退出至少区分：问题已由现有材料回答、当前授权内无法再补证、资源价值不足、来源不可得、证据仍不足、判断被证伪或需要人工决定。`completed` 不能把这些语义压成一个成功状态。

## 来源身份：最小注册表，不是万能 Object

建议使用一个极小的 Source Identity Registry，统一回答：

> 在当前平台和来源对象类型下，这个稳定外部 ID 指向哪个内部身份？

其职责仅限：来源系统、identity namespace、来源对象类型、稳定 external ID、首次建立时间和身份资格状态。它不保存正文、昵称、指标、Topic、业务属性或任意关系，因此不会演变成 EAV/低配图数据库。不能只用一个宽泛 `platform=xhs` 假设所有 endpoint、地区或未来来源共享同一 ID 空间；namespace 必须由真实 producer 合同证明。

在注册表之上使用类型化关系与 Observation：

- Author Identity / Author Observation；
- Content Identity / Content Observation；
- Comment Identity / Comment Observation；
- 后续只有真实 producer 和业务需要证明时，才增加新的来源类型。

第一阶段约束：

- 同一来源系统、identity namespace、来源类型、稳定 external ID 只能解析到一个当前内部身份；并发解析由数据库唯一约束收敛到同一身份，不能由“先查再插”制造重复对象。身份合并和拆分必须留审计，不能改主键假装从未出错。
- Comment 缺少平台稳定 ID 时只进入 unresolved material，不用“作者 + 前 50 字”、正文 hash、数组位置或 AI 推断制造 Comment Identity。
- Media URL 先作为某次 Observation 中的来源断言；只有平台稳定媒体身份或已下载字节及 hash 能证明独立生命周期时，才考虑独立 Media Identity。
- Metric 是某对象、指标名、观察时刻和来源位置组成的时间性断言，不是 Source Object。
- Domain 不进入来源身份唯一键。同一真实 Note 可以被 ADHD 内多个 Topic、计划和研究复用。
- 评论者或作者“像家长、老师、机构、患者”等角色不是 Source Identity 属性。来源明确自述只保存为带原文和时间的来源断言；模型角色判断进入有版本的 Derived Analysis，unknown 不补成已知。

## Discovery、Observation 与解析修订

### Discovery 只记录如何看见

每次搜索、作者索引或关系扩展产生独立 Discovery 记录，至少连接：

- 发现执行和 Evidence；
- 入口/query/filter/作者/研究路径；
- 页面、cursor、位置或排名（来源能提供时）；
- 观察身份、时间和 Coverage；
- 已解析 Source Identity，或明确 unresolved。

Source Identity 去重不能删除 Discovery；同一 Note 被十个关键词发现仍是一个来源身份和十条发现历史。

### Observation 只追加并采用类型化结构

Observation 表达 Evidence 对一个来源对象在某次观察中直接支持的状态。推荐使用类型化 Observation，而不是统一 `object_properties`：

- 核心字段使用有类型的列和运行时合同；
- 未观察与观察到空值/零值分开；
- 每个采用字段能回到 Evidence Record/Artifact 和来源位置；
- 部分 Observation 合法，不强制补成完整快照；
- AI 分类、角色推断、需求、Topic、趋势和机会不进入 Observation。

Observation 可以引用一份或多份互相补充的 Accepted Evidence；一份 Package 也可以产生零个或多个 Observation。Evidence acceptance 优先作为 Package/Record/Artifact 上的接入资格与回执，不为了“Accepted Evidence”再复制一遍相同 payload。最终连接表和约束由首个真实合同证明，不使用一个 `package_id` 强行表达全部血缘。

unresolved material 后来取得稳定身份时，追加 Resolution 并连接原材料与新 Source Identity；不得重新伪造一次采集或覆盖当时“身份未知”的事实。此前依赖身份未知材料形成的用途限制也不会自动消失，必须按新的身份和隐私风险重新评估。

来源当前不可访问、平台陈述对象已删除、系统获准执行隐私删除，是三件不同的事：

- 页面打不开或本次未返回，只形成带 Observation Context 的 availability 观察，不能推断对象已删除；
- producer 明确观察到平台删除/不可用标记时，追加来源状态陈述，但旧 Evidence 和过去 Observation 仍是历史；对象以后重新出现时再追加新 Observation；
- 隐私处置由授权治理路径触发，即使来源仍公开，也可以立即停止系统读取与传播；它不伪装成平台删除。

Current 可以因此显示 unavailable、unknown 或 last-qualified-observation，但必须说明采用规则和时间；不得用来源消失覆盖旧 Observation，也不得把旧快照继续伪装成当前可见状态。

### Interpretation Revision 不制造世界事件

parser 修复、格式归一化更正或时间解析修正形成新的解释版本，连接同一 Evidence 和被替代版本。它不创建“世界在修复当天变化”的新 Observation。原始值、解析规则版本、参照时间、精度和修正理由必须可追踪。

## Current：历史是真相，当前是可重建选择

不采用“最新写入整行就是 Current”，也不要求每个领域对象都建立一套 Current 表。

推荐第一版采用更克制的混合模式：

1. 不可变 Observation 和来源断言保存历史。
2. Current Resolution Run 记录使用的规则版本、输入边界和运行时间。
3. 类型化 Current Projection 为每个核心字段保存值以及对应的 Observation/来源断言外键；字段由对象合同固定，不使用 `field_key/value` EAV 表。
4. 人工冲突裁定追加为独立 Adjudication，记录采用/排除理由并成为后续 Resolution 输入，不直接覆盖 Observation。
5. Projection 按一个完整 resolution revision 原子切换，允许事务性替换和完全重建；第一版不为每个无争议字段再永久追加一条 Resolution Entry。

这允许标题来自较早但完整的 Observation、指标来自较新的 Metric Observation，同时让每个值仍有来源。来源冲突或资格不足时 Current 可以是 unknown；不得按最大值、平均值或最后写入自动裁定。只有真实字段扩展和查询 benchmark 证明类型化投影不可维护时，才重新评估受控字段 Resolution 表，不先为“灵活”引入通用属性系统。

## 时间模型：不使用一个万能 created_at

每类记录只保存自己有资格表达的时间，至少分开：

| 时间责任 | 含义 | 不能替代什么 |
|---|---|---|
| execution started/ended | producer Attempt 实际运行区间 | 不能充当每条记录观察时间 |
| discovered at | 某次入口首次或再次发现对象的时间 | 不能充当发布时间或详情观察时间 |
| observed at | producer 实际读取对应来源材料的时间 | 不能由整批 `finishedAt` 统一回填 |
| received at | 服务端收到交付的时间 | 不能说明世界何时变化 |
| accepted at | Evidence Ingress 完成接纳的时间 | 不能推进 Source Current |
| source time assertion | 平台显示/返回的发布时间、评论时间等来源陈述 | 可能是绝对值、相对文本、范围、推算或 unknown |
| derived at | parser、分类、聚类、统计或 Agent 产生结果的时间 | 不能伪装成来源事件时间 |

PostgreSQL 中精确时刻使用带时区时间；来源只给日期、相对表达或模糊范围时，同时保留原始文本、参照观察时间、时区、解析器版本、精度以及可选上下界，不强制制造一个精确 timestamp。迟到材料按 observed/source time 进入历史，received/accepted time 只表达系统处理顺序。

本项目不建立一套所有表通用的 `valid_from/valid_to` 双时态框架。Topic Definition、Claim Revision、Source Observation 和权限/保留分别使用符合自身语义的版本与时间；只有具体查询需要时再组合现实时间和系统时间。

## Coverage：来源事实与用途评估分开

Capture Package 保存 producer 能直接报告的 Coverage Source Facts，例如：

- lane、subject 和计数单位；
- planned/requested、discovered、emitted、deduplicated、failed；
- page/cursor、`hasMore` 三态、回复根、位置范围；
- 达到上限、连续无新增、页面末尾、来源明确耗尽、风险中断等原始停止原因；
- 哪个 Attempt、账号/工位、producer 版本和时间产生这些事实。

`planned/requested`、`discovered`、`attempted`、`emitted`、`failed` 与 `not_attempted` 只有在对象范围和计数单位相同时才允许对账；producer 不知道的数量保持 unknown，不能用 `requested - emitted` 自动拆成失败或不存在。服务端接纳多少 Evidence、下游解析出多少 Source Identity/Observation 是后续处理结果，不伪装成 producer 的 Coverage 来源事实。

这些数字不能跨单位相减或合成一个全局完成百分比。目标笔记数、页面数、卡片数、唯一对象数、评论数和回复根数分别保留；unknown 不转为 0，启发式停止不转为 `source_exhausted`。

Coverage Evaluation 属于具体 Information Need、统计或 Claim 的派生判断：它说明现有来源事实是否足以支持“取得正文”“观察最多 40 条评论”“这两个窗口可比较”等明确用途。Evaluation 可随新证据和规则修订，不回写 Package。只有观察入口有资格、执行达到所需范围且分类能力适用时，零结果才能支持“本轮在该捕获面未观察到”；永远不能直接支持现实不存在。

部分执行中的合格 Evidence 可以支持“这批材料中存在什么”、内部检索或经用途/隐私评估后的 Corpus 选择；它能否支持代表性、占比、变化或市场判断，必须另过对应 Coverage 与统计资格，不能从“50 条已入库”推导“完成了一半市场观察”。

这里必须分开四项资格：

1. **保存资格：** 这次交付、原始引用、运行和失败是否按当前 producer/隐私合同保存；无效任意字节不因此获得永久保留权。
2. **Evidence 接入资格：** Package 与最小成员 envelope 是否满足 authority、身份、完整性和可重放合同；接入后不保证每个成员都能形成 Source Observation。
3. **用途适用性：** 当前调用者、目的、隐私状态和材料范围是否允许检索、Corpus 选择、模型处理、页面展示或外部传播；这是使用时判断，不是 Evidence 的永久标签。
4. **Claim 推断资格：** 当前固定输入、Coverage、单位、分母、时间、来源集中、定义/模型版本、反例和替代解释能支持哪一种具体主张。

因此同一批部分结果可以同时出现：原始 Record 已安全保存、Package 已接入、其中部分 Record 暂时 unresolved、部分材料可供内部检索、集合内描述成立、正式趋势仍不成立。任何一个布尔状态都不能替代这组边界。

后续补采形成新的 Attempt/Package，旧 Package 不追加。跨批分析使用已有 `Analysis Input Set` 固定准确输入，跨任务材料复用使用 `Material Pack`，真实进入 Agent/Decision/内容的成员由 `Material Usage` 记录；不再新增同义的 `Material Set`。合并前必须按 Source Identity、Observation 时间、Discovery 路径和分析单位区分重复发现、有意复观测与真正不同对象。

## 领域语言与双历史

领域知识至少保留五类责任：

1. Topic Identity：长期引用的概念身份；
2. Topic Definition Revision：某个时期的名称、定义、边界和说明；
3. Term 与语境关系：首选名称、Alias、多义和语言范围；
4. Knowledge Relation Revision：正式 broader/narrower 等定义关系；
5. Domain Definition Release：通过 Decision 冻结的一组定义和关系版本。

Map View 是导航版本，不修改 Topic 定义；实验聚类词表是 Analysis Vocabulary Snapshot，不进入正式 Release。Topic 拆分/合并产生新身份和血缘，不自动迁移旧分类、Claim 或统计。

因此系统同时能回答：

- 当时系统用什么定义理解材料；
- 现在使用新定义怎样重新解释同一批历史材料。

两者通过 Definition Release、Classification Run 和固定输入集合分开，不合并为一条伪造的历史趋势。

## 分析输入、分类、聚类与 Claim

### Analysis Run 必须冻结实际输入

每次有后果的分类、聚类、统计或 Agent 分析至少固定：

- Analysis Run 身份与目的；
- 实际输入集合版本和成员；
- 每个成员采用的 Observation/Corpus 转换版本；
- Domain Definition Release 或实验词表快照；
- 模型、embedding、规则、算法、参数和提示版本；
- 运行时间、代码/合同版本、成功/失败与已知缺口。

输入集合不能只保存一段查询条件。查询条件会随 Current、权限和数据增长而返回不同成员；需要复现、比较或支撑 Claim 时必须冻结成员或能够由不可变快照精确重建。成员数量较小时使用受外键约束的成员关系；规模和 benchmark 证明逐行成员成本过高时，可以使用内容寻址、不可变的成员 manifest 并在 PostgreSQL 保存 hash、计数、生成规则和验证回执，但不能只保留会漂移的查询文本。

### 分类与人工裁定

机器 Classification Result 只属于对应 Run。人工裁定追加对象、原结果、采用结论、理由和依据，不覆盖机器输出。正式定义发布后尚未完成重分类时，读取端显示未就绪或明确沿用旧分类，不能把新 Topic 名称和旧统计拼在一起。

### Claim 是长期追踪的最小判断单位

不是每个统计都需要 Claim。只有需要被引用、挑战、修订、用于 Decision 或进入分析发布物的重要判断，才建立 Claim Identity 与 Revision。Revision 至少固定：

- 明确主张；
- 范围、时间与适用对象；
- 支持、反例和无法排除的解释；
- 使用的 Analysis Run、Observation、Evidence 或其他 Claim revision；
- 当前能证明什么与不能证明什么；
- 提出者、修订理由和治理记录。

支持/挑战关系必须限定用途，不能用通用三元组或单一 confidence 取代。冲突 Claim 可以并存；“当前不采用”不等于已证明为假。

Claim 不设一个全局“当前真理”指针。某个范围和用途现在采用哪一条 Claim revision，由明确的 Decision、Brief release 或其他有责任的发布关系固定；撤回、取代或停止采用是治理历史，不覆盖原 Revision，也不自动把冲突 Claim 判假。

Signal、Market Insight 与 Intelligence Brief 不复制 Claim：Signal 是某次 Analysis Run 的有边界发现及注意力记录；后两者是固定当时 Claim revision、材料选择、反例、限制与正文的发布版本。

模型或 Agent 在分析正文中生成了输入材料没有直接支持的新事实性陈述时，不能因为文字流畅就进入 Observation 或现有 Claim。它只能：

1. 引用已经存在且适用于当前用途的 Claim revision；
2. 建立一个新的 Candidate Claim，明确它仍需什么支持或反证；
3. 保留为无事实资格的草稿表达，并禁止作为下游 Decision 的正式依据。

摘要、翻译和改写也要能回到具体输入版本；它们可以改善可读性，不能提升证据强度。

## Corpus 与材料使用

Corpus 不复制第二份评论正文。建议持久化：

- Selection：为何、为哪个用途、按什么规则选择；
- Member：引用哪个 Observation/Evidence 版本或精确片段；
- Fragment：起止位置、来源版本与 hash；
- Transformation：脱敏、翻译、转写、摘要等输入/输出与工具版本；
- Material Pack：一次研究或 Agent 调用实际冻结的材料集合；
- Material Usage：哪些材料最终被引用、采用、传播或用于 Decision。

“曾经入选 Corpus”不是永久访问许可。读取时仍按调用者、用途、受众、隐私处置和传播范围重新判断；零命中、被权限过滤、Coverage 不完整和现实不存在必须分开返回。

冻结的 Material Pack 或分析输入集合在隐私处置后仍保留不含敏感原文的成员身份/hash 和历史使用审计，但当前读取必须过滤或拒绝已失效材料，相关 Run、Claim 与发布物标记为受影响并进入重评估；“冻结可复现”不能成为继续返回已撤回内容的理由。

## 应用资产与 Agent 使用

- Market Insight / Intelligence Brief 是面向人的组织和发布物：固定采用的 Claim revisions、支持/反例材料、适用范围、限制和生成版本；它们不复制一套可独立漂移的“结论真相”。
- Content Idea 保存“准备尝试什么”的长期身份和修订，允许来自人工经验而不伪造一个 Signal；进入行动前由 Decision 固定所采用的 Idea revision 与依据。
- Publication 或其他现实行动与 Content Idea 分开。草稿完成、排期、发布尝试、平台实际发布和结果观察分别留痕，不能由页面状态推断现实动作发生。
- 每次有业务后果的 Agent Invocation 记录调用者/委托、目的、授权、实际 Material Pack、采用的定义/Claim/模型版本、输出和结果使用。临时浏览回答不必全部升级为长期资产；一旦被保存、共享、引用、用于 Decision 或触发资源动作，就冻结输入与输出版本。
- Agent 权限不能从“能读聚合结果”推导为“能读原文”，也不能从“能提出补证”推导为“能占用工位”。所有能力按身份、Domain、用途、数据级别、动作、预算、期限和传播范围分别授权。

## Decision、Action、Outcome 与 Evaluation

- Proposal 与 Revision 保存系统或人的候选方案；
- Decision 固定所决定的确切 revision、当时依据和批准/拒绝/缩小/附条件/延后结果；
- Action Plan 固定实际准备做什么和 Expected Outcome；
- Action Attempt 保存执行尝试和失败；
- Action Occurrence 只有外部来源或合格人工陈述能证明现实动作已发生；
- Outcome Observation 只保存实际取得的结果、来源、观察窗口和环境；
- Evaluation 解释结果如何支持或挑战旧 Claim，但不改写 Claim、Topic 或 Evidence。

Expected Outcome 必须在相应 Action Occurrence 之前由版本化计划冻结；行动发生后的新增解释只能进入 Evaluation，不能回填旧预期来制造预测准确。一个 Content Idea 被多个账号、形式或时间执行时，每次 Action 和 Outcome 独立保存，跨行动总结另建有范围的 Evaluation，不回写一个覆盖失败的总分。

未接入、未观察、窗口未结束和来源不合格是结果渠道可获得性，不创建数值为零的 Outcome。失败行动和没有结果的行动同样保留，避免幸存者偏差。

## 隐私、保留与处置传播

“Evidence 不可静默覆盖”不等于敏感原文永不删除。数据架构必须同时支持：

1. 业务代码不能无记录修改历史；
2. 当前读取可以立即禁止返回受限原文；
3. 经授权的删除、撤回、脱敏或访问限制形成 Privacy Disposition Event；
4. 同一事务记录处置、即时读取阻断和待传播影响任务；
5. Corpus、全文索引、embedding/feature、摘要、引用、缓存、Material Pack 和 Agent 输出按血缘失效、重建或停止返回；
6. 历史 Claim/Brief 不静默消失，而是标记来源资格变化并重新评估；
7. 物理载荷删除后只保留不含敏感原文的最小 tombstone、hash/身份审计和处置证明。

第一版先使用保留类别，不在本草案猜具体天数：

| 类别 | 内容 | 默认方向 |
|---|---|---|
| 受限原始载荷 | 页面、响应、原始评论、可能反向识别内容 | 最小访问、加密、可处置；具体期限 `DECISION_REQUIRED` |
| 接纳后的结构化来源记录 | 身份、原始表示、来源关系、时间、Coverage | 按用途长期保存但不承诺永久；敏感字段可分离处置 |
| 派生敏感内容 | 片段、embedding、摘要、翻译、缓存 | 必须能沿血缘失效或重建 |
| 非敏感审计元数据 | hash、合同、执行、失败、处置和最小回执 | 只保留证明责任所需的最小信息 |

第三方模型是否可以处理原文、具体保留期限、加密密钥生命周期、角色和法律适用性仍是 Gate 5–7 与专业审查的决定；未确认前默认不把原文发送到第三方模型。

## 统计与趋势资格

数据库不保存一个全局 `comparable=true` 或 `trend_ready=true`。每次统计或 Claim 的资格至少绑定：

- 实际输入集合；
- 观察对象与计数单位；
- 分母和去重规则；
- 事件时间与比较窗口；
- Observation Context / plan version；
- Coverage 来源事实与用途评估；
- 来源集中度、单一爆款/作者影响；
- Topic Definition、分类和分析版本；
- 已知观察镜头变化与替代解释。

建议将统计定义与 Analysis Run 结合，由冻结输入成员产生可重算结果。库存累计、时间窗口新增和可比观察面占比分开计算，但不把 `Stock/Flow/Share` 直接建成三种永久对象。

Gate 4 真实实验未完成时，系统仍可陈述“在这次、这个入口和捕获范围中看到了什么”，不能发布正式增长/下降趋势，也不能硬编码 7 天衰减。

## PostgreSQL 约束原则

1. 内部长期身份使用由服务端生成的 UUIDv7 候选；平台 external ID 始终作为受约束业务键单独保存，不能混作内部主键。最终 key 方案在 DDL 前用 SQLx/PostgreSQL 16 fixture 验证。
2. 核心关系使用真实 foreign key、unique、check 和条件更新，不依赖 Rust 类型或 JSON 约定替数据库守约。
3. append-only 历史禁止普通业务 UPDATE/DELETE；修订通过新行与 predecessor/revision 关系表达。隐私处置使用专门高权限路径和审计，不是普通更新例外。
4. 状态值按责任使用小而封闭的集合，不建立跨领域万能 `pending/approved/rejected/completed` 状态机。
5. 需要原子性的边界使用数据库事务、行锁/条件更新和唯一约束；“先查询、后无条件写”不是并发安全。
6. durable work / outbox 与产生它的事实同事务写入；worker 至少一次执行，业务副作用以幂等键和状态前置条件保护。
7. JSONB 不承载正式 Topic 关系、Source Identity、权限、成员集合或 Current 来源；这些必须可约束和可连接。
8. 不在第一版提前分区。只有真实写入规模、保留/删除模式和查询基准证明需要时，才按时间或责任拆分；避免在百万级以内先引入复杂分区治理。
9. 对象存储不是数据库成功的替代。涉及 blob 时必须明确“地址已观察、字节已下载、hash 已验证、对象已持久化”分别到哪一步。
10. 读取投影可以被删除重建；如果删除投影会丢失无法恢复的业务含义，说明该内容放错了层。

## 必须固定的事务边界

| 边界 | 必须共同成功 | 失败时允许什么 |
|---|---|---|
| Package 接入 | authority fence、identity/hash、Package/Record/Artifact/Coverage、receipt、durable work | 全包回滚或显式 replay/conflict；不能半包成功 |
| 单 Record 归属/Observation | identity resolution、类型化 Observation、血缘、处理回执 | 当前 Record/Object 隔离重试或隔离；不回滚其他已接纳 Evidence |
| Current 发布 | resolution run、类型化字段来源外键和整份 projection revision 切换 | 保留旧 Current 或显示未知；不能半新半旧无版本 |
| Domain Definition Release | Decision、release manifest、成员和当前发布指针 | 整次发布失败；不得最大版本自动接管 |
| Decision | 确切 proposal revision、依据包、决定结果和审计 | 不产生 Action 授权；不得决定漂移到新 proposal |
| Privacy Disposition | 处置事件、立即读取阻断、传播 work | 原文先停止返回，后续派生异步清理；不能处置记录成功但继续外传 |
| Action / Outcome | Action Attempt 或合格 Occurrence 与其来源；Outcome 单独接入 | 执行失败仍保留；不能由 Action 事务伪造未来结果 |

## 读取路径与性能方向

- 写模型按责任清晰、可追溯优先；页面不直接跨几十张权威表临时拼接。
- 高频页面和 CLI 使用类型化 read projection，投影带构建版本、输入水位和 freshness；不能只返回一个没有来源的缓存值。
- 原声搜索首先在允许读取的转换/片段与全文索引上工作，原始受限载荷不作为默认检索面。
- 向量检索、pgvector、聚类专用索引和 Python worker 都等待真实中文样本 benchmark；它们不是 Gate 5 的默认前提。
- 第一阶段不引入 Kafka、图数据库、通用事件总线、数据湖或微服务数据库拆分。

## 第一阶段不建什么

- 不建旧 Prisma 166 模型的映射版。
- 不建万能 Object、Property、Relation 或 Asset 表。
- 不为每个页面建一套真相表。
- 不把 Corpus 复制成第二份评论库。
- 不建全局 confidence、evidence score、coverage percent 或 trend-ready 开关。
- 不把所有变化塞进一个事件表，也不把整个项目变成 Event Sourcing。
- 不提前建 Topic 图数据库、微服务、Kafka 或复杂分区。
- 不建多租户套餐、客户组织和计费模型。
- 不为尚未接通的私信、咨询、销售和产品行为伪造 Outcome 表达。

## Gate 5 历史退出清单与 SCOPE 裁定范围

下列清单记录 Gate 5 当时的压力测试顺序。`USER-DEC-03/04` 已确认身份、Current 与首期统计方向；SCOPE-001 只物理冻结合成 Content 的 Source Identity、类型化 Observation、字段来源 Current、Package/Record/Coverage 和对应事务子集。Topic/Claim、真实原料隐私、正式趋势与生产权限仍按后续 SCOPE 和真实证据推进，不重新要求用户确认已经关闭的产品方向。

1. **真相分类与 Source Identity Registry：** 是否接受“最小统一身份注册表 + 类型化 Observation”，而不是万能 Object 或完全分裂的多套身份。
2. **Current 混合模式：** 是否接受“不可变历史 + 类型化字段来源外键 + 冲突裁定 + 可重建投影”，第一版不建通用字段 EAV。
3. **版本与发布：** Topic/Claim/分析输入的 Identity、Revision、Release、Run、Projection 是否无身份继承漏洞。
4. **隐私生命周期：** 原始载荷具体保留期限、受限角色、第三方处理和物理删除传播如何落地。
5. **统计资格：** 单位、分母、时间、Coverage、来源集中和定义/分析版本怎样成为具体 Claim 的资格，而非全局开关。
6. **事务与并发：** 用 INV-04、05、10–63 验证重放、并发、迟到、部分失败、目标语义、Record 分流、定义发布、隐私处置和行动结果。
7. **PostgreSQL 概念验收：** 关系与约束收口后再画候选表簇；未确认前不创建 DDL 或 migration。

## Gate 5 已完成的设计退出条件

只有同时满足以下条件，Gate 5 才能被用户确认完成：

1. 每个持久对象都能说明为何需要稳定身份，以及属于 Identity、Revision、Release、Run/Occurrence 或 Projection 中哪一种。
2. Evidence、Observation、Derived Analysis、Domain Definition、Claim、Decision、Action、Outcome 和视图之间不存在第二事实源。
3. Source Identity、Discovery、部分 Observation、来源差异、Interpretation Revision 和 Current 可以在跨时间场景中共存。
4. Topic 拆分/合并、新模型重算和 Definition Release 不静默改写历史。
5. 隐私处置能够立即阻断读取并可验证地传播到派生物，同时保留最小非敏感审计。
6. 统计资格不能由库内增长、扩采、模型变化或单个爆款伪造。
7. Package、Observation、Current、Definition Release、Decision 和 Privacy 的事务边界通过并发与失败压力测试。
8. 草案通过 `INV-01` 至 `INV-63`，未解决项明确标为 `SOURCE_INCOMPLETE` 或 `DECISION_REQUIRED`。
9. Gate 5 设计完成时未创建业务 DDL、migration 或 SQLx model；首个物理子集现在只能按正式 SCOPE-001 实施，不能从本草案扩大。
