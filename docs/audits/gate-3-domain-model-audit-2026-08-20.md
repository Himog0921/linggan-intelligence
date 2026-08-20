# Gate 3 领域模型收口审计

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 3 当前完成度、已确认设计 A/B/C 与后续 Gate 分流
> 事实来源: 当前权威文档、DISC-001 活跃计划、固定 V2 Prisma schema 与固定源码目录
> 冲突时以谁为准: 用户最新确认、真实 producer/fixture/运行结果、ACCEPTED ADR 与权威当前文档

> 后续状态: 2026-08-20 用户已整体接受本报告的全链复核结论，Gate 3 正式退出并进入 Gate 4。以下“等待用户整体确认”保留为审计完成当时的历史状态，不再代表当前项目阶段；本确认不授权数据库、业务代码、AI Agent 或插件升级。

## 审计目的

本报告不新增领域真相，也不把讨论中的推荐升级为已确认合同。它回答三个问题：

1. Gate 3 当前哪些边界已经由用户确认；
2. 哪些设计已经完成压力测试，其中哪些已确认、哪些仍等待确认；
3. 哪些问题看似属于 Gate 3，实际上必须等待真实 producer、PostgreSQL 或系统架构关口，不能靠领域名词提前猜定。

审计同时检查 Gate 3 是否出现以下退化：

- 为每个页面名创建一个领域对象；
- 为了减少对象数量建立万能对象；
- 把旧 V2 schema 当成新项目合同；
- 把待确认讨论写入权威共同语言；
- 用字段、状态枚举或技术模块代替业务语义；
- 为了继续推进而提前开始数据库、AI Agent 或插件实现。

## 当前结论

Gate 3 的 A/B/C 三组责任设计均已得到用户明确确认，但全链复核尚未完成，因此本报告仍不能单独证明 Gate 3 已退出。当前完成度应理解为：

```text
已确认
├── 发现与详情采集分责
├── Capture Package、Evidence 接纳与 Observation 分责
├── Observation 历史、来源差异、Current 与时间语义
├── Domain Definition、Topic、Term、Alias、关系、分类与 Claim 边界
└── 研究问题、Information Need、采集申请/授权/准入、
    Decision、Action、Expected Outcome、Outcome Observation 与 Evaluation

已确认应用资产责任
└── Corpus Selection、语义派生物、候选变化、Market Insight、
    Intelligence Brief 与 Content Idea 的长期责任和应用视图边界

随后
└── 基于全部确认版本完成 Gate 3 全链复核，
    再由用户整体确认 Gate 3 是否退出并进入 Gate 4
```

因此，Gate 3 仍不能标记完成，也不能开始业务表设计。用户已在 2026-08-20 分别明确确认设计 A、B 与 C；A/B/C 的权威表达进入共同语言、讨论决定、产品合同与不变量后，继续推进的最短真实路径是用全部确认版本重跑全链审查。

## 已确认责任矩阵

| 责任 | 当前已确认含义 | 明确不是 | 后续仍需证明 |
|---|---|---|---|
| Domain | 当前一级长期研究边界为 ADHD | 关键词、Workspace、多租户容器 | Gate 5 的权限与数据归属；未来第二 Domain 不进入首期 |
| Discovery | 某次搜索、作者页或其他入口实际怎样发现对象 | Topic 分类、完整对象状态、市场普查 | Gate 4 的 API/页面对账、顺序、终止和 Coverage |
| Source Identity Resolution | 判断来源记录能否指向稳定来源对象 | 相关性、优先级、Topic 分类 | Gate 4 的真实平台 ID、链接和冲突合同 |
| Source Object | 外部平台中可重复观察的稳定来源身份 | 研究问题版本、页面行、一次采集结果 | Author、Comment、Media 的最终身份合同等待 Gate 4–5 |
| Capture Occurrence | 记录系统实际尝试了什么、怎样结束和哪里失败 | Evidence 成立、对象被完整观察 | Attempt、重试、分块、checkpoint 与 Package 基数等待 Gate 4 |
| Capture Package | producer 对有界工作的完整交付单元 | 自动成立的 Evidence 或一条 Observation | Record、Artifact、Coverage、终态和隐私隔离等待 Gate 4–5 |
| Evidence Ingress / Acceptance | 判断来源、权限、身份关系、包一致性和最低合同是否合格 | 证明来源陈述真实、完整、有代表性或适合所有用途 | 真实 producer 合同与事务实现等待 Gate 4–5 |
| Observation | Evidence 直接支持的来源对象在一次条件下的版本化状态 | Topic 分类、医学解释、趋势、市场意义 | 不同对象的字段、来源优先和物理关系等待 Gate 4–5 |
| Interpretation Revision | 修正系统怎样忠实表达同一来源材料 | 新世界 Observation、AI 语义判断 | parser 版本与传播事务等待 Gate 5 |
| Current Resolution | 带来源、规则和理由的可重算当前读取责任，允许未知 | 最后写入、最大值、无来源拼接 | 字段级/对象级选择和事务等待 Gate 5 |
| Derived Analysis | 对明确输入集合执行的分类、聚类、统计、embedding 或 Agent 分析 | Evidence、Observation、正式知识或永久事实 | 输入集合、版本与算法合同等待 Gate 5–6 |
| Domain Definition | 由明确发布决定采用的 Topic 定义、首选名称、Alias 与知识关系版本 | 聚类标签、市场事实、分类统计或页面导航 | 持久关系、发布事务与重算传播等待 Gate 5–6 |
| Classification Run / Human Adjudication | 固定输入、词表与模型/规则版本的分类输出，以及有理由的追加人工裁定 | 覆盖旧模型输出、自动改变正式 Topic | 结果存储、裁定优先和重算策略等待 Gate 5–6 |
| Claim | 有主张、范围、时间和依据的重要可争论判断，允许支持、挑战和冲突并存 | 万能三元组、每个统计或一个可覆盖 confidence | 物理身份、关系和资格传播等待 Gate 5 |
| Research / Information Need | 持续目标、主动问题、计划和运行分责；信息缺口可由多种来源满足 | 万能 Research Intent、插件命令、任务完成即 Need 满足 | 最终身份、关系和退出状态等待 Gate 5–7 |
| Acquisition Request / Authorization / Admission / Work Order | 申请、人的许可边界、执行前重评估和 producer 有界工作分责 | 插件令牌授权业务采集、排队即永久执行、发现自动深采 | producer 合同、关系、事务和调度等待 Gate 4–6 |
| Decision / Action / Expected Outcome / Outcome Observation / Evaluation | 决定固定版本；行动计划、尝试、现实成立、行动前预期、结果事实和结果解释分别保存 | 一个 Outcome 字段、批准即执行、事后补写预测、结果自动证明原因 | 最终关系与结果来源合同等待 Gate 5–7 |

以上责任已经由当前权威共同语言、不变量和讨论决策支持。后续可以改变最终英文名称或物理表达，但不能重新把这些责任压回一个状态或一张表。

## 已确认设计 A：领域正式语言

本组设计已于 2026-08-20 得到用户明确确认，并已写入权威共同语言、领域不变量、讨论决定与产品/架构合同。以下内容保留为该次确认的审计快照；冲突时以更新后的权威文件为准：

1. Topic 拆分为长期 `Topic Identity` 与不可静默覆盖的定义版本。长期身份回答“历史引用指向哪个概念”，定义版本回答“某个时期怎样描述它的含义、边界、包含与排除”；
2. 一套可复现的 `Domain Definition Release` 说明某时期正式采用了哪些 Topic 定义、首选名称、Alias 和知识关系。它是逻辑清单，Gate 5 可以使用共享版本引用，不要求每次发布物理复制全部词表；
3. “当前正式版本”必须由明确发布决定产生，不等于最后创建或更新时间最大；允许已经发布新定义、但重分类和统计仍未准备完成；
4. Candidate Topic 本质是待治理的知识变更提案，不是半正式 Topic。聚类产生的临时标签先属于 Analysis Run；只有需要提交、比较、决定或长期追踪时，才形成持久提案；
5. 正式词表与实验词表分开。模型可以使用候选 Topic 或临时 label set 做探索，但分类运行必须固定自己使用的分析词表快照，不能把实验标签偷偷写入正式 Domain Definition；
6. Term、Alias、用户表达和 Collection Seed 分责。Term 是语言表达；某个 Term 是哪个 Topic 的首选名称或别名，是带语言、语境和定义版本的关系；同一个词可以在不同语境指向不同 Topic，不能使用全局同义词等价；
7. 定义性知识关系、数据共现/相似关系和 Map View 导航关系分开。知识层的 broader/narrower 等层级关系在同一发布版本内不能形成自我引用或循环；普通 related-to 可以形成网络；页面导航只负责可理解路径；
8. 分类运行引用明确、不可变的输入集合，使用的正式发布或实验词表快照，以及模型、规则、提示和运行版本；人工裁定追加保存并说明依据，不覆盖模型当时输出；
9. 新定义发布不自动重分类历史，也不自动切换当前统计。查询和页面必须说明当前使用哪个定义和哪次分类；定义已发布但重算未完成时，显示未准备完成，而不是混用新旧分类；
10. Topic 改名、边界修订、拆分、合并和导航移动不是同一种变化，采用下表规则；拆分或合并以后，旧 Topic 上的 Claim、统计和人工分类不自动继承给新 Topic；
11. Claim 是可单独支持、挑战、修订和引用的重要判断，但不设计成万能三元组，也不要求普通可重算统计全部持久化。一个需要长期保存的 Claim 至少必须说清主张内容、适用范围、时间边界和当前依据；
12. 支持材料增加、反例出现和评估改变不能通过一个可覆盖 confidence 字段代替。相互冲突的 Claim 可以并存；“被取代、被撤回、当前不再采用”是治理结果，不等于系统已经证明它为假；
13. 正式 Topic 定义和知识关系本身属于当前采用的领域语言，不需要再复制成 Claim；Claim 可以支持或挑战一次知识变更提案，但不能与正式定义形成两份互相竞争的当前真相；
14. 语义性正式语言变化由人确认并形成决策包；决策包要说明系统想改什么、为什么、影响哪些分类/统计/页面、是否需要重算、是否可撤回。纯拼写和展示修正留编辑记录，不制造重审批，也不伪装成概念变化。

### Topic 变化的身份连续性规则

| 变化 | 推荐处理 | 不能发生 |
|---|---|---|
| 拼写、排版或不改变含义的展示修正 | 保持 Topic Identity 和语义版本；保留编辑记录 | 为每次错别字创建新 Topic 或触发全量重分类 |
| 名称改变但含义和边界不变 | 保持 Topic Identity；发布新的名称/定义版本 | 把历史引用断开，或让旧名称无痕消失 |
| 边界发生可解释修订，但仍是同一核心概念 | 保持 Topic Identity；发布新定义版本并说明差异 | 用新定义重写当时分类和 Claim |
| 一个 Topic 拆成多个新的概念 | 新 Topic Identity；旧 Topic 保留历史身份，并记录拆分血缘 | 指定一个新 Topic 自动继承全部旧历史、Claim 和统计 |
| 多个 Topic 合并成新概念 | 通常创建新 Topic Identity并记录合并血缘；只有一个 Topic 明确吸收别名且自身含义不变时才保留该身份 | 因名称相近就无损合并，或把冲突历史压成一条 |
| 只改变 Topic Map 中的展示路径 | 只发布新的 Map View；不改变 Topic 定义或分类 | 因页面移动就声称知识定义改变或重跑全部分类 |

这里的判断标准不是“名字像不像”，而是：历史引用继续指向同一身份时，会不会误导用户对旧材料和旧判断的理解。如果会，就必须新建身份并保留血缘，不能为了数据库简单强行保持同一个 ID。

这组设计解决旧 V2 `Topic` 混合内容素材、指标、AI 摘要和工作流的问题，也解决 `TaxonomyNode.parentId + level + aliasKeywords` 将知识、导航和关键词压在一起的问题。它还防止“发布新词表但统计尚未重算”时混用新旧解释，以及候选聚类通过模型实验反向污染正式语言。确认只授权领域语义收口，不授权创建数据库表、知识图谱、统一 Claim 对象、分类作业或页面实现。

## 已确认设计 B：研究、申请、决定与行动

本组设计已于 2026-08-20 得到用户明确确认，并已写入权威共同语言、领域不变量、讨论决定与产品/架构合同。以下内容保留为该次确认的审计快照；冲突时以更新后的权威文件为准：

### 观察目标与研究问题

1. 持续观察目标和主动研究问题保持不同产品责任，不建立万能 `Research Intent`。前者回答“长期希望注意什么”，后者回答“这次希望弄清什么”；
2. 观察目标、观察计划和一次实际执行分开。目标说明为什么，计划说明采用什么观察方法，一次运行说明实际做成了多少；修改计划可能影响可比性，但不自动改变长期目标；
3. 研究问题的原始范围需要保留。AI 或人如果把问题从“任务启动表达是否存在”扩大成“是否形成商业机会”，这不是润色，而是新的范围和新的决定；
4. “为什么采集”和“后来为什么复用”分别留痕。新增研究可以复用旧 Evidence，但不能把新的使用理由倒写成当初的采集动机；
5. `Research Project` 暂为产品组织范围，不提前成为数据库超级对象或通用工作流。只有一次研究需要跨多轮协调、保存问题版本、判断、缺口和决定时，才需要稳定研究案例身份；一次简单查询不必制造项目。

### Information Need 与 Evidence 复用

6. Information Need 说明“为了当前判断还缺什么信息”，不是“调用哪个插件接口”。它可以由已有 Evidence、新的 Analysis Run、人工提供的合格材料或新的平台采集满足；
7. 一份 Evidence 可以满足多个 Need，一个 Need 也可以依赖多份 Evidence；满足关系是多对多，并按具体用途重新判断新鲜度、Coverage 和适用性；
8. 简单取一个已知字段不必全部建立长期 Need 对象。只有缺口会跨任务追踪、影响研究结论、资源决定或停止条件时，才需要持久身份；
9. “已经拿到材料”“材料适用于当前问题”“信息已经足够”“研究决定停止”是四种不同结论。Need 可以部分满足、被替代、因不值得继续而关闭，或保持未知；不能由 Work Order `completed` 自动关闭；
10. 假设可以作为 Candidate Claim 在研究中接受支持或挑战，不额外创建一套与 Claim 重复的永久 Hypothesis 真相表。

### 采集请求、授权、准入与执行

11. Acquisition Request、Authorization、Acquisition Admission 和 Work Order 分别表示申请、许可边界、执行前准入判断和插件有界工作；
12. Authorization 是“允许在什么范围、期限、资源和风险条件内尝试”的运行许可，不是一次成功事实。长期观察可以拥有范围明确的持续授权，不要求用户逐个批准机械 Work Order；超出计划版本、来源、成本或风险边界才重新授权；
13. Admission 必须在需求变成真实平台访问前检查 Evidence 复用、等价在途工作、有意复观测、时间价值、Coverage 缺口、授权、工位/账号能力和风险；排队已久的工作在领取前重新评估，创建时值得做不代表现在仍值得做；
14. 一个 Work Order 可以部分满足多个 Request，一个 Request 也可以拆成多个 Work Order；失败、暂停或部分 Coverage 只影响相应范围，不用固定一对一外键或统一 `completed` 隐藏真实关系；
15. 申请、批准、准入、租约领取、producer 执行、Package 提交、Evidence 接纳、Need 满足、Claim 形成和业务成功分别有自己的事实来源；任意前一步不能伪装后一步成功。

### Proposal、Decision、Authorization 与 Action

16. Knowledge Change Proposal、Acquisition Request 和 Action Proposal 按后果分开，不先创建万能 Proposal；可以共享展示和审计模式，但不能用一个状态机一次批准不同权力；
17. Decision 保存当时“选择了什么、不选择什么、基于哪些版本化材料、由谁在什么权限内作出以及理由是什么”。后续 Evidence 或 Claim 变化不重写旧 Decision，只能产生受影响提示或新 Decision；
18. Authorization 是某些 Decision 产生的有期限操作许可；不是所有 Decision 都产生 Authorization。Authorization 可以过期、撤回或被取代，但对应历史 Decision 仍然存在；AI 和被授权 Agent 不能给自己扩大授权；
19. 计划、建议、预约和 Action Occurrence 分开。Action 只记录现实中实际发生的业务动作，例如内容真正发布、访谈真正举行或产品实验真正启动；插件采集继续使用自己的 Work Order/producer 执行责任链，不冒充业务 Action；
20. 不建立装入所有现实行为的巨大 `Action + payload JSON`。Gate 5 先确定共同审计责任，再按首期真实业务动作决定是否需要各自类型和关系。

### Expected Outcome、Outcome Observation 与 Evaluation

21. 行动前预期、实际 Outcome Observation 与 Evaluation 分开。Expected Outcome 必须在行动结果出现前保存目标、观察窗口和预期方向，不能事后补写成“早就预料到”；
22. 一个 Action 可以在多个时间窗口、多个渠道产生多次 Outcome Observation；一个结果也可能受多个 Action 和外部条件共同影响。系统可以保存关联和不确定性，不能强迫单因果归属；
23. 未接入、未观察、窗口未结束、来源不合格和真实为零分开；失败行动、未执行行动和没有结果数据也要保留，不能只记录成功案例；
24. Evaluation 回答“这些实际结果对原 Claim、Decision 或行动假设提供什么有限支持或挑战”，不把表现数字直接变成因果结论；外部条件和替代解释作为评价依据，不是一个自由填写的万能 excuse 字段；
25. Outcome 通过新的 Claim、Evaluation 和后续 Decision 形成学习，不直接调整 Topic、Claim 分数、观察计划或采集范围。

### 研究怎样诚实结束

主动研究至少允许以下不同出口，不能压成一个 `completed`：

| 研究出口 | 真实含义 | 不能偷换成 |
|---|---|---|
| 当前问题已有足够依据 | 在声明范围和时间内可以形成有限回答 | 永久真理或所有相关问题均已解决 |
| 原假设未获支持或受到反证 | 当前材料不支持原方向，或者反例足以限制它 | 现实中绝对不存在 |
| Evidence 不足 | 当前无法得到有资格的结论 | 研究失败、结论为否或数据库计数为零 |
| 继续研究不再值得 | 资源成本、时间价值或行动影响不足以继续 | Need 已经满足或世界没有变化 |
| 问题范围发生实质变化 | 关闭或保留原问题，并建立新的研究范围 | 直接改写原问题，使历史看似一直在研究新目标 |

这组设计解决“AI 提出缺口就产生插件任务”“批准即执行成功”“任务完成自动关闭 Need”“选题获批但未发布仍有 Outcome”“事后补写预期”“公开表现好就证明情报正确”等状态污染。本次确认仍不授权创建审批引擎、工作流表、Research 超级对象或业务 Action 超级类型。

### 第二轮对抗修正：授权、探索、合并与现实动作

在正式确认前，本报告曾继续用“授权已过期但任务仍排队”“已有新 Evidence 到达但旧任务仍准备开工”“插件设备凭证被误当业务许可”“发布按钮已点击但平台内容未成功出现”等场景攻击候选 B。以下补充修正一并进入本次确认：

26. **不同授权不能共用同一语义。** Acquisition Authorization 表示人允许在明确目标、来源、计划版本、期限、资源和风险边界内尝试新的平台访问；Evidence Access/Use Authorization 表示谁能为哪个用途读取、处理或传播材料；Plugin Authorization 只是设备/插件身份和接入凭证。拥有后两者不自动拥有前者，批准采集也不自动授权原文展示、第三方模型处理或外部传播；
27. **授权必须固定被批准的版本与边界。** 修改计划、请求、来源、字段深度、成本、风险或期限不会原地扩大旧授权。长期观察可以复用有边界的 standing authorization，但 AI、Agent、计划编辑者和插件都不能通过修改所引用的“当前计划”给自己扩大权限；
28. **授权失效与停止执行需要真实过渡。** 排队或已预留工作在真正开始前必须重新检查授权未过期、未撤回且仍覆盖当前执行计划。执行中撤回后，是立即安全停止、完成当前不可分割步骤还是仅禁止下一步，必须由后续按工作类型确认的安全停止策略决定；无论怎样，已经发生的访问、局部结果、失败和 Coverage 都不能消失；
29. **准入判断本身会过期。** 创建队列项时“值得采”不能永久有效。在领取或开始前，至少重新检查新到 Evidence、等价在途工作、内容生命周期/时间价值、Need/探索目的是否仍有效、授权、工位/账号能力与风险；若已有材料足够或价值已消失，应停止不必要访问并保留理由。该重评估不是让插件自行做研究判断，而是服务端使用当前可追溯状态重新裁定；
30. **允许受控探索，但不允许无目的扩采。** 新平台访问可以由明确 Information Need 驱动，也可以来自已授权的有限探索预算。探索不必先知道正式 Claim，但必须说明想降低哪类未知、允许的来源/成本/时间、停止条件以及为什么不能由已有 Evidence 回答；探索材料不得进入稳定比较分母自证增长；
31. **合并执行不合并原始目的和用途资格。** 多个 Request 可以共享一次实际平台访问，但每个 Request 的原始动机、所需字段、时间/新鲜度、Coverage、访问/传播权限、保留要求和满足评估分别保留。相同对象或字段不代表所有用途等价；一次采集满足请求 A 不能自动关闭 Need B；
32. **Decision 固定当时提案和依据版本。** Decision 可以批准、拒绝、缩小、附条件、延期或要求补证；批准后实质修改 Proposal、Request 或 Action Plan 必须形成新决定或明确修订，不能让旧批准随“当前版本”漂移。Decision 的历史不因后来依据变化而重写；
33. **业务动作必须区分计划、尝试与现实成立。** Action Proposal/Plan 说明准备做什么；Action Occurrence 记录确实发生的尝试和过程；只有合格来源证明内容已经发布、访谈已经举行或实验已经启动时，才能声称相应外部动作成立。点击发布但失败、计划通过但未执行、采集 Work Order 完成分别是不同事实；后续 Gate 5 不得用一个巨大 `Action + payload JSON` 混装；
34. **Expected Outcome 固定在行动前版本。** 它可以在行动前有记录地修订，但必须固定对应 Action Plan 版本、观察窗口、预期方向和依据；行动实质改变后，旧预期不能继续冒充对新动作的预测。行动发生后新增的解释只能是 Evaluation 输入；
35. **研究暂停与主动停止分开。** 授权撤回、资源暂缺、平台风险或 producer 故障可能使研究暂时无法继续，但这不等于 Information Need 已满足、不值得继续或原假设为否。恢复时必须重新检查范围、时效、授权和已有新材料；
36. **研究、Claim 与 Evidence 不是一对一所有权。** 一个研究问题可以形成多个互相冲突或不同强度的 Claim，也可以因 Evidence 不足而不形成 Claim；同一 Claim 可以被多个研究或发布物引用，同一 Evidence 可以在不同研究中按用途重新评估。研究会话不拥有和复制 Evidence 真相。

这些修正解决的是权力与历史语义，不要求现在建立九种状态机、四套授权服务、通用工作流引擎或每个简单查询都持久化研究对象。具体哪些责任需要独立表、值对象、事务或 Rust 模块，分别等待 Gate 5–6。

### V2 对候选 B 能证明和不能证明的边界

- 旧 schema 的 `TaskDemand`、`TaskDemandLink` 和 `ExecutionJob` 表达了多个业务需求共享一个实际作业、过期时间、新鲜度和幂等键等有价值的设计意图；`ExecutionQueueEntry` 与 claim/start 代码证明了队列过期、工位能力、账号健康、预留和租约并发保护的局部运行机制。
- 但是固定源码缺少 `task-demand-service.ts`、`execution-planner-service.ts`、`monitor-checkpoint-demand-service.ts`、`execution-station-mailbox-service.ts`、`execution-platform-account-eligibility.ts` 和 `execution-identity.ts` 等被现役文件导入的实现，不能把 schema 与部分 claim 代码陈述为完整 Demand → Admission → Job 运行闭环。
- 当前可见 claim 查询只按 queued、available/expires、workspace、platform、lane、能力和账号条件领取；start 再检查 reservation、计划版本、工位和账号健康。固定源码没有证明领取/开始前重新检查已有 Evidence 是否足够、等价需求是否已被其他结果满足、研究/探索目的是否仍有效，或人的 Acquisition Authorization 是否仍覆盖该计划。
- 旧 `PluginAuthorization` 及签名校验绑定插件设备、workspace、状态、令牌和期限，证明接入凭证可以失效和 fail closed；它不是用户对某次研究范围、采集深度、资源预算或平台访问目的作出的 Acquisition Authorization，不能直接移植为业务授权。
- `ResearchSession`、`ResearchQueryRun`、`ResearchEvidencePin`、`DecisionEvent` 和 `OperationPublication` 仍只证明 schema 设计意图。固定 `src/` 未找到前四类研究/决定模型的实际业务调用；`OperationPublication` 将发布身份、同步状态、指标、线索和事后分析放在同一对象，也没有 Expected Outcome、Action Attempt/Occurrence 与 Evaluation 的已证明分责。因此新项目只能吸收版本和血缘思想，不能照搬状态或宣称反馈闭环已运行。

## 已确认设计 C：应用资产与发布视图

本组设计已于 2026-08-20 得到用户明确确认，并写入权威共同语言、领域不变量、讨论决定与产品/架构合同。以下内容保留为该次确认的审计快照；确认只冻结长期责任与权力边界，不授权据此创建业务表、页面状态机或 AI 工作流。

### Corpus 是用途化选择，不是第二份评论数据库

1. 原始来源材料只保留一份；Corpus 记录“某份材料或精确片段因什么用途、按什么规则、以什么角色被选择”，不复制一份新的原文真相。
2. 来源原文、精确 Source Span、脱敏/安全表达和 AI 语义标注具有不同资格。片段必须固定到某个来源内容版本、范围和 hash；来源后来编辑时，旧片段不能悄悄指向新文字。脱敏文本不能冒充原文，AI 摘要不能冒充可引用片段。
3. `Semantic Unit` 不成为所有 AI 输出必经的万能对象。只有某个派生片段需要被稳定引用、重复选择或形成长期血缘时，才获得可寻址身份；普通分类与摘要继续属于可重算的 Analysis Run 输出。
4. Corpus Selection 是持久的用途关系，至少要能说明选择目的、选择理由、材料角色、选择规则/版本和当时访问边界；移除或替换选择也要保留历史。它不表示材料内容真实，也不表示适合其他用途。
5. 一次检索运行、一次保存下来的选择集合和材料最终被怎样使用是三件事。普通无后果查询不要求全部长期保存；当结果被交给 Agent、形成决策包、被引用或进入内容生产时，才需要保存足够的输入、选择和使用血缘。
6. “代表性”只能相对于明确捕获面、输入集合、语义簇或选择目的表达。典型表达、高表现内容、反例和多样性样本采用不同选择规则，不能合并成一个全局 `featured=true`。
7. 使用资格在调用时按调用者、目的、受众、当前隐私状态和传播范围判断，并记录当时采用的策略版本和结果；不把 `allowedUse` 作为材料永远不变的单值属性。策略变化或隐私处置可以使后续使用失效，但不能重写当时确实发生过的使用记录。
8. 缺少稳定 Source Object 身份的材料，如仍有合格 Capture Package、来源位置和不可变定位，可以保持为资格受限的 Evidence/未解析材料并参与受限 Corpus 选择；不能为了收入 Corpus 伪造 Comment、Expression 或 Observation。
9. 自动聚类、内部研究、工作台脱敏展示和外部 Agent 引用具有不同后果，不共享一个“正式 Corpus”审批门。机器可以在合格的内部材料上自动分析；会长期引用、公开展示或外部传播的选择才进入相应治理。

### Signal 保留分析发现，注意力入口不改变其资格

1. Detector Event、统计异常或聚类输出先属于 Analysis Run。不是每个模型输出都值得成为 Signal；只有结果能够说明输入范围、方法版本、实际异常/结构和限制，并值得后续关注或调查时，才形成可引用的 Signal Candidate。
2. `Signal` 推荐定义为：一个有来源、有范围、有分析版本的派生发现，说明某种变化、结构、异常或风险“可能值得注意”。它可以支持或挑战一个 Candidate Claim，但不直接承担更强的市场解释、因果或行动价值。
3. 多个 Signal 可以共同支持一个 Claim；同一个 Signal 也可能被不同 Claim 以不同方式使用。Signal 不能复制 Claim 正文和证据关系，也不能因被人工看到就升级为正式趋势。
4. Signal 内容和注意力处理历史分开。某个 Signal 是否进入每日入口、何时继续观察、何时降级或关闭，属于 attention/investigation 处理；关闭只表示当前不再投入注意力，不表示原分析从未发生或现实不存在。
5. 每日入口还可以展示 Coverage 异常、证据缺口、待授权申请和已有 Claim 的状态变化，它们不是 Signal。Radar 项、Signal、Candidate Claim 和正式 Claim 不共享一个状态机。
6. 第一阶段不承诺正式趋势，因此 Signal 可以描述限定捕获面中的异常或结构，但不能越过 DEC-05，把跨期差异包装成已经成立的市场增长/下降。
7. 是否需要独立 Signal 持久对象、Signal 是否以某类 Claim 或 Derived Finding 实现，由 Gate 5 根据多对多引用、历史追踪和查询需求决定；Gate 3 只冻结“保留分析发现但不复制判断、不因展示升级资格”的语义。

### Market Insight 与 Intelligence Brief 是版本化发布物，不是第二套 Claim

1. Market Insight 是面向市场结构、用户表达、内容供给或变化解释组织的一组 Claim、分析结果、反例、限制和适用范围。页面即时生成的小结不必自动成为长期 Insight。
2. 当一份市场解释需要被保存、比较、订阅、引用或作为后续决策依据时，才形成版本化发布物；发布物必须固定引用当时的 Claim revision、Analysis Run、输入边界和文字版本。
3. Intelligence Brief 是面向具体目标或决策组织的版本化发布物，除 Claim 外还包含证据缺口、不确定性、备选解释、行动选择和行动价值。发布 Intelligence 不等于批准其中建议，更不等于授权现实行动。
4. Claim 是可单独支持、挑战、取代或撤回的判断最小单位；发布物保存当时组织方式。后来某条 Claim 被推翻时，历史发布物不被静默重写，当前阅读界面应提示其引用的判断已经修订。
5. Market Insight 与 Intelligence 在产品语义上不同，但 Gate 5 不预设两张相似的大表。它们可以共享一种“版本化发布能力”，再由类型化结构表达不同职责。
6. 自然语言正文是发布版本的一部分；结构化 Claim 和证据关系是可审计依据；页面摘要是可重建表达。三者不得互相冒充唯一真相。

### Content Idea 是内容工作资产，不是 Domain Topic

1. 使用 `Content Idea / 内容选题候选`，避免 `Topic Idea` 与领域 Topic 再次混淆。
2. Content Idea 可以来自人工经验、原声、Topic、候选变化、Market Insight 或 Intelligence；没有系统 Evidence 时必须如实记录“人工来源”，不能伪造 Signal 或 Intelligence 作为上游。
3. Content Idea 是可编辑、可比较、可采用或放弃的工作资产；当它被 Decision、制作 Brief 或 Action 引用时，要固定当时内容版本，不能让后续编辑改变已经批准的选题。决定采用、实际发布 Action 和后续 Outcome Observation 分开。
4. “内容角度”不是一个单值标签，而至少可能包含用户问题、目标人群、叙事立场、内容承诺、表达形式、情绪入口、内容目的和解决方案类型。第一阶段先作为有版本的派生分析维度，不自动成为 Domain Topic。
5. 如果某套叙事分类以后用于长期检索或跨时间统计，它需要自己的定义和分析版本；不能把一次聚类标签直接固化成正式词表。
6. 一个 Content Idea 可以被执行多次，用于不同账号、形式和时间；Outcome 首先关联实际 Publication/Action，而不是直接汇总回 Idea。多个结果能否共同评价该 Idea，需要另一次有范围的 Evaluation。
7. 自有内容进入平台后仍是外部可观察 Source Object，但必须标记行动来源，并从稳定市场基线中隔离；其公开表现也不能自动证明原 Intelligence 正确。

### 本组设计的关键防线

- 不建立“原始语料池 + 精选语料池”两份物理真相；先用同一材料上的选择关系表达不同书架。
- 不给每条评论、每次聚类和每个页面小结都创建长期资产；持久身份由引用、复用、治理和后果决定。
- 不让 `Signal → Insight → Intelligence → Content Idea` 变成强制单向审批流水线；任一环节可以合理结束，人工选题也可以没有机器上游。
- 不让正式发布物通过引用“当前 Claim”而持续变脸；历史版本固定当时 revision，当前界面另行显示后续修订。
- 不让内容角度、用户问题、Domain Topic、采集词和页面导航再次合并成“关键词”。
- 不让全局允许用途、全局代表性、全局置信度或统一 `approved` 状态替代具体用途下的资格判断。

### 第二轮对抗修正：选择、复现、发布与生成内容

在用户确认前，本报告继续用“已选材料后来失去使用资格”“同一异常在不同运行中反复出现”“历史报告引用的原声被隐私处置”“AI 基于合格材料生成了材料中不存在的新事实”等场景攻击候选 C。补充修正如下：

1. **选择历史与当前可用性分开。** `Corpus Selection` 只证明某份材料曾在明确目的、规则和访问边界下被选择；它不能兼任当前访问许可。来源撤回、隐私处置、用途变化或授权过期后，历史选择与真实使用记录继续存在，但新的检索、展示、模型调用和外部传播必须重新检查当前资格；不能通过删除选择历史伪装成从未使用，也不能因为历史上选过就永久可用。
2. **来源、片段、转换和语义标注各自留血缘。** 精确片段固定来源版本、范围与 hash；脱敏、翻译、转写、摘要和改写属于从来源派生的不同表达，必须说明转换方法和版本。它们可以服务检索或展示，但不能反向替换原始来源，也不能让翻译/摘要中的新增含义冒充来源原话。
3. **检索、固定材料包和实际使用分开。** 检索运行回答“当时用什么条件得到哪些候选”；固定材料包回答“当时为某个目的保存了哪些引用”；使用记录回答“哪些材料真实进入了 Agent 上下文、报告、选题或成稿”。零命中、未保存、未采用和因权限被拦截不是同一种结果；普通浏览不必全部持久化，产生决策或传播后果时才保存足够的复现链。
4. **代表性角色不能变成总体声明。** “典型、反例、多样性、高表现、边界样本”是相对于指定输入集合和选择规则的角色。系统必须同时保留来源集中度、跨帖子/作者覆盖和已知缺口，不能把聚类中心、爆款评论或 AI 精选写成“小红书用户代表”或“ADHD 家庭代表”。具体抽样算法和数量留给 Gate 6–7 的真实数据实验。
5. **同一分析现象重复出现不会原地增强为事实。** 每次 detector/cluster/statistical run 都保留自己的输入和版本；跨运行的相似结果可以由新的分析关联为复现、持续、合并或分裂候选，但不能用标题相似、hash 冲突消解或累计出现次数去覆盖旧 Signal。Signal 的“再次出现”本身仍需要可比输入和来源分散检查，不能自动提升 Claim 强度。
6. **Signal 没有“批准后变成正式趋势”的通用晋级。** Signal 是有边界的派生发现；是否值得关注、是否需要调查和是否支持某个 Claim 分别判断。人工查看或保留 Signal 只改变注意力处理，不制造市场事实；正式趋势、需求、机会或行动价值仍必须形成各自有资格的 Claim。
7. **发布物固定当时依据，也要能响应资格变化。** Market Insight/Intelligence Brief 的历史版本固定当时 Claim revision、Analysis Run、选择集合与正文，不随“当前 Claim”变脸。后来 Claim 修订、来源失效或隐私处置时，系统追加影响评估和当前阅读警示；若原文依法不得继续保留或展示，可以撤销其访问、生成受限/脱敏后继版本并保留不含敏感内容的最小审计痕迹，不能拿“版本不可变”阻止合规处置。
8. **发布物中的重要新断言不能藏在散文里。** 会改变结论、资源选择、对外承诺或行动建议的主张，应引用已有 Claim revision，或先明确成为待审查 Claim；背景说明和连接文字可以作为发布正文，但不能在自然语言中悄悄扩大人群、时间、平台、因果或确定性。正式发布前需要能检查“文字说到哪里”是否超出结构化依据，具体自动/人工校验机制留给 Gate 6。
9. **发布资格按后果决定，不按页面名称决定。** 页面即时摘要、个人草稿和一次查询回答可以自动生成并明确标为可重算表达；当内容将被订阅、共享、外部传播、引用为 Decision 依据或长期比较时，才需要冻结版本和相应发布决定。`Market Insight` 页面不是事实源，`Intelligence Brief` 的生成完成也不是正式发布或行动授权。
10. **AI 输出血缘不等于输出正确。** Content Idea、标题、提纲、文案或 Agent 回答即使引用合格 Corpus/Claim，也可能新增材料中不存在的事实、诊断、承诺或因果。输入血缘只证明“参考过什么”，不能替新输出背书；任何对外事实性主张仍需逐项满足当前证据、隐私和用途边界，不能把生成文本整体标记为 `evidence_based=true`。
11. **Content Idea 的建议、保存版本、采用与实际成稿分开。** 未保存的机器建议可以保持短暂输出；需要编辑、比较、决策或多次执行时才形成可寻址的 Idea 与版本。Decision 固定被采用的版本，后续修改形成新版本或派生方案；实际文案对 Corpus 片段的引用、改写或未采用另记使用血缘，不能因 Idea 有来源就假定成稿仍忠实于来源。
12. **外部 Agent 回答不是自动形成 Intelligence。** 普通受控查询可以返回带范围、时间、来源和限制的临时答案；只有答案被保存、共享、作为 Decision 依据或需要后续追踪时，才冻结请求、委托、输入、输出与所引用的 Claim/材料版本。Agent 回答进入长期资产并不扩大其读取、传播或行动权限。

这一轮没有确认 `CorpusSelection`、`Signal`、`Publication`、`ContentIdea` 等物理表名，也没有引入统一 `Artifact`、`Publication` 或 `GeneratedOutput` 超级对象。Gate 5 必须先根据真实引用、修订、隐私传播和查询需要决定哪些责任需要稳定身份；Gate 6 再决定生成校验、检索复现和模块边界。

## 已分流到后续 Gate、不得在 Gate 3 猜定的问题

### Gate 4：真实 producer 与证据链

- 搜索 API 返回与页面实际可见逐字段对账；
- Comment、Author、Metric、Media 的实际来源和身份；
- Attempt、重试、分块、checkpoint 与 Package 基数；
- Record、Artifact、Coverage、cursor/`has_more` 与终态；
- 原始时间表达、客户端时钟、服务端接收时间和解析精度；
- 四类身份失败及其不同后果；
- 风控停止、账号/工位资源、批量和恢复；
- 跨账号、跨时间、跨入口的真实可比性。

### Gate 5：PostgreSQL 概念数据设计

- 最终持久身份、关系基数和唯一性；
- Current 的字段级或对象级来源选择；
- 历史定义、分类运行、Claim 修订和人工裁定的版本关系；
- Evidence 对具体 Claim 的适用性与支持/挑战关系；
- 统计单位、分母、Coverage 资格和输入集合快照；
- 包级接入事务、Record/Object 下游隔离和并发重放；
- 隐私保留、隔离、脱敏、删除传播、索引/向量/缓存失效；
- Outcome 渠道可获得性与人工陈述资格。

### Gate 6：Rust、AI Agent 与插件架构

- crate、API、worker 和部署边界；
- 避免巨型文件与巨型模块的依赖方向和大小门禁；
- AI 模型、提示、工具权限、成本、停止、评测和降级；
- 是否需要 pgvector、Python 分析进程或具体聚类算法；
- 服务端调度、插件 lease/恢复、兼容性 fixture 与版本门槛；
- 错误传播、Outbox、可观测性和安全。

### Gate 7：页面、CLI 与用户验收

- 每日入口、Topic 探索和决策包的最终信息结构；
- Corpus、Topic Map、Market Insight、Intelligence 和选题是否独立成页；
- 代表样本、时间窗口、判断资格和未知状态怎样显示；
- 不同 Agent 委托下的查询、建议、申请、回执和传播；
- 第一条垂直切片最终用户结果和真实样本。

## V2 证据边界复核

固定 V2 schema 提供了若干可参考思想，但不能证明新领域模型已经存在：

- `Topic` 同时保存来源、正文、作者、平台指标、AI 摘要、选题价值和审核状态，是旧内容工作台对象，不是新 Domain Topic；
- `TaxonomyNode` 保存 proposed/active/merged/retired、父子和合并关系，但缺少定义版本、完整发布版本和知识/导航分责；
- `ContentTopicTag` 有 `taggerVersion`，但同一 subject/topic 的唯一约束无法同时表达多次定义/模型分类和人工裁定历史；
- `TopicPerformanceRollup` 有版本字段，但该结构本身没有输入集合、观察协议、Coverage 和定义/分类版本，不能证明趋势；
- `ResearchSession`、`ResearchQueryRun` 和 `ResearchEvidencePin` 表达“研究草稿不自动成为正式结论”的好方向，但查询快照缺少完整输入、Coverage、定义和分析版本；
- `DecisionEvent` 是通用状态变更审计雏形，不足以证明有范围授权、实际执行、Outcome 和新旧决定关系；
- `WritebackDelivery` 区分请求、重试、应用和失败，证明技术执行状态需要分开，但它不是业务 Decision/Action/Outcome；
- `VoiceSnippet` 的精确范围、hash、来源内容 hash、脱敏文本和 extractor 版本是可复用的血缘思想；但它把 `allowedUse`、安全和审核压成当前单值，不能直接成为新项目的用途资格模型；
- `CorpusPack`、`CorpusRetrievalRun` 和 `CorpusMaterialUsage` 已表达“选择集合不复制原文、检索留范围/版本、实际使用另记账”的好方向；但固定 `src/` 没有运行时引用，不能陈述已经形成真实产品闭环；
- `CommentSignalType` 同时混入共鸣、冲突、需求、金句、故事、FAQ、俚语和解决方案等不同维度；`CommentSignal` 容易把单条评论过早升级成 Signal，不能移植；
- `DemandCard` 的 Evidence role、review snapshot 和 topic suggestion 可作为治理参考，但其 counts/quotes/status 不能自动证明需求，且唯一源码调用点引用了仓库中不存在的 `demand-card-merge-service`；
- `VoiceAsset` 复制 quote、保存全局 `allowedUse` 并用 `CommentSignalType` 作为资产类型，存在第二份原文、永久用途标签和维度混合风险；
- `MarketSignalCard` 以不透明 JSON、统一 confidence 和卡片 status 保存分析，缺少原子 Claim、输入资格和历史修订边界；
- 固定 `src/` 中未找到 `VoiceAsset`、`CommentSignal`、`RecommendationTrace`、`TopicSuggestion`、`MarketSignalCard`、`MarketSample`、`CorpusPack`、`CorpusRetrievalRun` 或 `CorpusMaterialUsage` 的运行时引用；因此只能陈述 schema 设计意图，不能陈述已运行的语料、市场或选题闭环。

可忠实保留的是不覆盖历史、保留 actor/版本、查询不复制原文、Evidence 选择保留理由、写回失败不触发重新采集等原则。具体模型必须重新设计。

## Gate 3 收口路径

### Step 1：用户确认领域正式语言设计（已完成）

可证伪退出标准：改名、定义修订、拆分、合并、歧义 Term、多路径导航、模型重分类和单条 Claim 被推翻都不会静默重写历史。

### Step 2：用户确认研究、申请、决定与行动设计

可证伪退出标准：批准但未执行、部分采集、研究证据不足、行动未发生、结果渠道未接入和好结果无法证明因果均能分别表达。

### Step 3：用户确认应用资产边界（已完成）

可证伪退出标准：Corpus、Signal、Market Insight、Intelligence Brief 和 Content Idea 不复制事实、不强制单向升级，也不退化成只有页面名称、没有长期责任的空壳对象；历史发布物不会因当前 Claim 修订而静默变脸。

### Step 4：Gate 3 全链对抗审查

以下矩阵已使用当前已确认结论和已确认设计 A/B/C 完成首轮压力测试。它不是 Gate 3 已通过的证明；当前必须按全部确认版本复跑并处理矩阵中的待确认标记。

| 案例 | 允许发生 | 必须禁止 | 仍需后续证明 |
|---|---|---|---|
| 身份未知评论被两个研究复用 | 同一份有来源位置的受限材料分别建立两个用途关系；各研究独立记录选择理由和适用边界 | 为两个研究复制两条 Comment/Observation，或因文本像家长而制造用户身份 | Gate 4 的评论定位与身份合同；Gate 5 的受限 Evidence/selection 关系 |
| 一条已选 Corpus 片段的来源评论后来被编辑 | 旧片段继续固定原来源版本、范围和 hash；新文字形成新的来源观察/片段候选，当前使用资格重新评估 | 让旧引用自动指向新文字，或不留历史地覆盖被报告/内容使用过的原声 | Gate 3-C 已确认；Gate 4 编辑来源能力；Gate 5 span/version/usage 血缘 |
| 搜索返回 50 条、页面可见 35 条、只批准 5 条详情 | 分开保存两种来源陈述、全部合格 Discovery 与 5 个有界详情需求 | 把 50 条都建成完整 Note Observation，或自动打开全部详情/评论 | Gate 4 真实 producer 对账、Coverage、风险和终止实验 |
| 包接纳后 15 条 Record 合格、2 条对象归属冲突 | Package 接纳结果保持原子；15 条继续；2 条下游隔离并影响相应 Coverage | 因 2 条坏记录回滚 15 条合格记录，或整包成功掩盖对象错误 | Gate 4 fixture；Gate 5 事务与隔离关系 |
| 迟到材料描述更早状态 | 追加历史并保留 observed/received/accepted；Current 按规则重算并允许不变/未知 | 按写入时间把世界倒退，或覆盖原 Observation | Gate 5 Current resolution 与并发事务 |
| Topic “作业拖延”拆成多个概念 | 新定义版本和血缘发布；旧分类继续可读；按新定义启动独立重分类运行 | 改写旧 Topic/分类，使系统看起来过去就知道新结构 | Gate 3-4 已确认；Gate 5 版本和历史查询 |
| 旧 Topic 上有“最近增加”的 Claim，随后 Topic 被拆分 | 原 Claim 继续指向旧定义；新 Topic 需要各自重新评估和形成新 Claim | 按拆分比例或名字相似度自动继承旧 Claim、证据适用性和趋势 | Gate 3-4 已确认；Gate 5 Claim revision、applicability 与血缘 |
| 同一个词在不同语境指向不同 Topic | 保存同一 Term 在具体定义版本和语境下的多个 label/alias 关系；分类结合上下文 | 建立全局 `aliasOf` 后把所有历史表达强制映射到一个 Topic | Gate 3-4 已确认；Gate 5 term-label 关系；Gate 6 分类评测 |
| 只把 Topic 从一个导航父节点移动到另一个入口 | 发布新的 Map View；Topic 定义、分类和 Claim 保持不变 | 因页面目录变化触发知识修订、重分类或历史统计变化 | Gate 3-4 已确认；Gate 7 导航投影 |
| 新 Domain Definition 已发布，但新分类运行尚未完成 | 显示定义版本已发布、分析尚未准备；继续明确展示旧版本统计或暂时未知 | 在同一图表混合新旧分类，或因发布定义自动宣称全部历史已重算 | Gate 3-4 已确认；Gate 5 发布/分类准备关系；Gate 7 展示 |
| 聚类使用候选 Topic 做实验 | Analysis Run 固定实验 label set；结果保持候选并可与正式定义比较 | 把实验标签写入正式词表、Current 分类或长期统计 | Gate 3-4 已确认；Gate 5 analysis vocabulary snapshot |
| 新模型识别出更多“任务启动困难” | 固定历史输入比较解释差异；产生新 Analysis Run 和候选 Knowledge Revision Claim | 把模型升级显示成市场增长或自动切换正式统计 | Gate 3-4 已确认；Gate 5 输入快照、模型和统计资格 |
| 一份旧 Evidence 同时满足两个新 Information Need | 两个 Need 分别保存适用性/新鲜度/Coverage 评估，共享同一 Evidence | 复制 Evidence，或因满足第一个 Need 就自动关闭第二个 | Gate 3-B 已确认；Gate 5 need-satisfaction 多对多关系 |
| 长期观察已经有范围授权，计划只产生机械分批 | 在授权范围内自动准入和生成有界 Work Order；每批仍保留真实结果 | 要求用户逐条批准，或把一次授权解释成无限来源、成本和期限 | Gate 3-B 已确认；Gate 4–6 授权、计划版本和执行合同 |
| 插件设备令牌有效，但当前采集没有人的业务授权 | 插件可以证明设备身份，但服务端拒绝把请求准入为新平台访问；保留申请和拒绝理由 | 把 Plugin Authorization 当成扩大采集、读取原文或第三方处理的业务许可 | Gate 3-B 已确认；Gate 5 授权关系；Gate 6 接入/权限边界 |
| 人批准 Plan v1 后，AI 将当前计划改成更深评论采集 v2 | v1 授权继续固定原边界；v2 形成新请求/决定，未批准前不执行新增深度 | 旧授权通过“读取当前计划”自动覆盖 v2，形成自我扩权 | Gate 3-B 已确认；Gate 5 plan/request/authorization 版本关系 |
| 两个请求共享一次详情访问，但只有一个用途允许原文进入第三方模型 | 共享实际访问和 Evidence；分别保留请求目的、满足关系和数据使用资格，禁止未获准用途传播 | 因采集结果相同就合并授权、保留和传播边界 | Gate 3-B 与 DEC-01 已确认；Gate 5–6 enforcement |
| 排队任务创建时很有价值，两天后内容时间窗口已过 | 领取前重新评估并过期、延后或改为低时效用途；保留原因 | 因已经进入队列就必然访问平台，浪费有限工位 | Gate 3-B 已确认；Gate 4 时间价值实验；Gate 5 admission history |
| 任务仍在排队时，另一个作业已经带回足够 Evidence | 开始前重评估，复用新 Evidence 并停止重复访问；原 Request 仍保留由哪份 Evidence 满足 | 队列一旦创建就必然执行，或取消任务后丢失原始需求和复用关系 | Gate 3-B 已确认；Gate 5 admission/satisfaction；Gate 6 原子领取 |
| 研究问题从“表达是否存在”扩大为“是否值得投入产品” | 保留原问题和已有回答，建立新范围、额外信息缺口与新授权 | 直接改写问题文本，使旧 Evidence 看似足以证明商业机会 | Gate 3-B 已确认；Gate 5 research scope/revision |
| 一个 Decision 依赖的 Claim 后来被修订 | 历史 Decision 保留当时依据；当前界面标记依据变化并允许新 Decision | 静默改写旧 Decision，或继续把已修订 Claim 当未变化依据 | Gate 3-B/C 已确认；Gate 5 versioned decision basis；Gate 7 提示 |
| Agent 同时建议修改 Topic 和扩大采集 | 生成两个不同后果的申请；知识治理和采集授权/准入分别决定 | 用万能 Proposal 一次批准全部，或改词表后扩采并自证 | Gate 3-4 与 Gate 3-B 已确认；Gate 6 工具权限和结构化回执 |
| 一个 Signal Candidate 被提升到每日入口后自动消退 | 保留原 Analysis Run、Signal/关联 Claim 与注意力处理历史；结束表示“不再值得关注” | 删除原分析、把进入首页视为正式趋势，或把自动消退写成现实不存在 | Gate 3-C 已确认；Gate 5 Signal/Derived Finding 与 attention case 表达 |
| 同一异常由两次不同模型运行识别，标题和解释很相似 | 两次运行和输入分别保留；新的关联分析说明可能复现/持续，同时检查输入可比性、来源集中和模型差异 | 按标题或 hash 合并后累加 confidence，或把重复模型输出当成现实持续增长 | Gate 3-C 已确认；Gate 5 finding lineage；Gate 6 复现/去重评测 |
| 一份 Market Insight 中一条 Claim 后来被推翻 | 历史发布版本固定旧 Claim revision；当前界面提示受影响并可发布修订版 | 整份报告静默更新、整体删除，或继续把旧 Claim 当当前依据 | Gate 3-C 已确认；Gate 5 publication/revision 关系；Gate 7 展示 |
| 一份历史 Brief 引用的原声后来必须撤回原文 | 撤销当前原文访问并传播失效；历史发布记录追加影响说明，必要时生成脱敏/受限后继版本，只保留合规最小审计 | 以版本不可变为由继续展示原文，或静默改写历史 Brief 使其看似从未引用 | DEC-01 与 Gate 3-C 已确认；Gate 5 处置传播；Gate 7 安全呈现 |
| AI 使用合格 Corpus 写出原材料没有提到的诊断或效果承诺 | 保留输入和生成版本；将新增事实性主张单独验证、删除或明确标为无依据草稿 | 因为输入有 Evidence 血缘就把整篇输出标成有证据，直接对外发布 | Gate 3-C 已确认；Gate 6 生成校验与 Agent 权限；Gate 7 发布验收 |
| Content Idea 来自 Mog 的经验而非系统 Evidence | 记录 human-origin、假设和预期；允许进入内容工作区 | 伪造一个 Signal/Intelligence 作为上游，或把人工经验包装成系统事实 | Gate 3-C 已确认；Gate 7 内容工作流 |
| 某次 Corpus 检索零命中，但执行 Coverage 不完整或材料被权限拦截 | 分别显示零候选、Coverage 缺口和权限过滤；只在输入与资格明确时解释本次检索结果 | 从 resultCount=0 推断用户没有这种表达，或把权限过滤当作现实不存在 | Gate 3-C 已确认；Gate 5 retrieval/qualification；Gate 7 未知态 |
| Content Idea 被采用但没有实际发布 | 保存采用 Decision；Action 保持未发生；不存在 Outcome Observation | 因“approved”产生发布事实、平台对象或结果 | Gate 3-B/C 已确认；Gate 5–7 Action 关联 |
| 发布操作被真实尝试，但平台拒绝且内容没有出现 | 保存 Action Occurrence/attempt、失败和影响；Publication/外部动作保持未成立，不产生平台 Outcome | 只因用户点击或本地任务 completed 就创建已发布内容和表现结果 | Gate 3-B 已确认；Gate 4 行动/平台身份来源；Gate 5 Action 关系 |
| 同一个 Content Idea 在三个账号、两种形式被执行六次 | 六次 Publication/Action 分别拥有实际环境和结果；另行形成有范围的 Idea Evaluation | 把结果直接写回 Idea 的一个 score，覆盖失败、账号差异和多次执行历史 | Gate 3-C 已确认；Gate 5 idea/action/evaluation 多对多关系 |
| 内容发布后才补写“预期会高收藏” | 保存实际发布和结果；晚补内容只能标记为事后解释，不能伪装成 Expected Outcome | 用结果反向生成看似准确的行动前预测 | Gate 3-B 已确认；Gate 5 expected/evaluation 时间关系 |
| 内容发布但私信/咨询/销售未接入 | 保存实际发布 Action；各结果渠道保持 unavailable/unobserved/open-window 等真实未知 | 把未知写成 0、失败或用公开互动猜测转化 | DEC-04 已确认；Gate 4–7 结果 producer 和页面未知态 |
| 研究因账号风控或授权撤回暂停 | 保留未满足 Need、已经发生的局部工作和暂停原因；恢复前重评估时效、范围和授权 | 将暂停写成研究完成、问题不存在或继续研究不值得 | Gate 3-B 已确认；Gate 4 风险终态；Gate 5 research/admission history |
| 公开表现很好但原因不明确 | 保存来源合格的表现 Observation；Evaluation 只说明它对原判断提供有限支持或挑战 | 自动提升 Claim、Topic、观察范围，或断言 Intelligence 已被证明 | Gate 3-B 已确认；Gate 5 Evaluation 关系 |
| 一条原声已进入 Corpus、Insight 和 Brief 后发生隐私处置 | 限制/删除传播到索引、向量、选择、引用、缓存和输出；历史发布物保留“来源资格已变化”的安全痕迹 | 外部继续返回原文，或无痕删除使历史判断看似从未依赖它 | Gate 5 隐私传播；Gate 6 缓存/索引；Gate 7 展示 |
| 自有内容被平台搜索再次发现 | 作为 Source Object/Discovery 正常保留，同时关联原 Action；可作为行动反馈分析输入 | 混入稳定市场基线并使系统用自身内容证明市场变化 | Gate 3-B/C 已确认；Gate 4 身份；Gate 5 样本资格 |
| 无合格事项且本轮观察未完成 | 显示观察不完整、Coverage 与未知；不产生 Signal/Insight 填满页面 | 从 count=0 推出现实不存在、需求下降或“今日无变化” | Gate 4 Coverage；Gate 5 absence claim 资格；Gate 7 文案 |

首轮矩阵暴露并修正了五个容易落地成灾难的隐含假设：

1. `Corpus Selection` 不能只引用已解析 Source Object，否则最需要审查的身份不完整材料会被迫伪造对象或无痕丢弃；它必须能引用有稳定来源位置但资格受限的材料。
2. 历史 Insight/Intelligence 不能引用一个会原地变化的“当前 Claim”；必须固定当时 revision，并把后续修订作为阅读警示或新版本。
3. Signal 保留可引用的分析发现，attention/investigation 另记“曾经被注意和怎样处理”；两者都不复制 Claim 真相。
4. Content Idea 不是 Claim 的下一级状态，也不是 Domain Topic；人工来源是合法来源类型，系统不得为了血缘完整而编造机器依据。
5. 任何用于跨期统计的“内容角度/叙事分类”也会发生定义与模型漂移；它不能因为属于应用层就逃避版本、输入和可比性约束。

全部案例在确认后的共同语言中仍能指出允许结论、禁止结论和后续证明来源。此时仍只允许由用户整体确认 Gate 3，再进入 Gate 4；不授权数据库表或业务代码。

### 全部确认版本的第二轮复核结果

在 A/B/C 均确认并回写权威共同语言后，本轮重新从一条评论、一次搜索发现、一次失败采集、一条模型异常、一份情报简报、一个人工选题和一次失败发布贯穿检查全部责任。结果为：**语义责任复核通过，可以提交用户进行 Gate 3 整体确认；这不是 producer、数据库或运行链路通过。**

| 检查面 | 复核结果 | 防止的灾难 |
|---|---|---|
| 事实方向 | Capture Occurrence、Package、Evidence Acceptance、Source Identity、Observation、解析修订与 Current 保持单向可追溯；下游不能反写上游 | AI 解释或页面当前值改写原始历史 |
| 身份与目的 | Source Object 身份不包含 Research Question、Information Need、Topic 或 Corpus 用途；同一 Evidence/Observation 可以被多个目的复用 | 按研究问题复制评论、笔记和 Observation |
| 知识与判断 | Domain Definition、实验词表、Classification Run、Human Adjudication、Signal、Claim 与发布物分责 | 聚类标签变正式 Topic，或 Signal/报告成为第二真相 |
| 研究与资源 | Need、Request、Authorization、Admission、Work Order、Package 和 Need 满足分别留来源；执行可合并，权限与用途不能合并 | AI 自己扩大采集、重复占用有限工位并自证增长 |
| 应用资产 | Corpus Selection、Retrieval Run、Material Pack、Material Usage、Market Insight/Brief、Content Idea 和 Agent Answer 按后果持久化，不构成强制流水线 | 每个页面名变成一套大表和同步真相 |
| 决定与现实行动 | Proposal、Decision、Authorization、Action Plan、Attempt/Occurrence、外部动作成立、Expected Outcome、Outcome Observation 与 Evaluation 分责 | “批准/点击/任务完成”伪装成现实发布和结果，表现好伪装成因果 |
| 历史与隐私 | 来源、定义、分析、Claim、分析发布物和 Content Idea 各自固定版本；隐私处置撤销当前访问并传播影响，不以不可变为由保留敏感原文 | 新模型静默重写历史，或合规处置与证据血缘互相破坏 |
| Agent 与审计 | 临时回答不自动成为 Intelligence；保存/共享才冻结研究复现链，但受限访问与外部调用无论是否保存正文都留安全审计 | “不保存回答”绕过审计，或为了审计复制全部敏感上下文 |
| 命名与超级对象 | 分析发布物与外部内容发布行动已分开；没有确认万能 Object、万能 Proposal、万能 Publication、统一 confidence 或统一 candidate 状态机 | 一张巨大表/一个巨大服务承载互不相干的权力和生命周期 |

第二轮复核发现并已修正两处跨模块歧义：

1. 普通查询不冻结完整候选/回答，不能被解释为外部 Agent 或受限原文访问无需安全审计；安全审计与研究复现是两种不同保留责任。
2. Market Insight/Intelligence Brief 的“分析发布”与小红书等外部平台的“内容发布行动”不能共用 `Publication` 含义或状态机；前者固定判断表达，后者必须由现实来源证明动作发生。

复核没有发现第三个仍需在 Gate 3 由用户作产品语义选择的问题。仍未决定的内容均需要真实来源或物理设计证据，已经正确分流：

- Gate 4：搜索/API/页面字段对账，Author/Comment/Metric/Media 身份，Coverage/终态，时间与风控恢复；
- Gate 5：稳定身份、关系基数、版本/Current、事务、用途资格、分析发布和隐私传播；
- Gate 6：Rust 模块、AI Agent、调度/插件合同、依赖方向与无巨型文件门禁；
- Gate 7：页面、CLI、抽样呈现、权限反馈和第一条垂直切片验收。

在本报告完成时，Gate 3 只剩一个用户决定：是否接受本次全部确认版本的全链复核结论，并允许 DISC-001 进入 Gate 4。用户随后已于 2026-08-20 整体确认；该确认只开放真实 producer 与证据合同设计，不开放数据库表、业务代码、AI Agent 内核或插件升级。

## 审计结论

Gate 3 的逐项设计和全部确认版本全链复核已经完成，没有发现仍需在本关补造对象或状态的逻辑漏洞。审计完成时达到“等待用户整体确认”的退出前状态；用户随后已整体确认，因此 Gate 3 现已完成。最主要风险已经从概念混淆转为后续 Gate 把语义责任过早翻译成表、服务或统一状态机，所以 Gate 4 只能先核对真实 producer 与证据合同，仍不能开始物理数据库或业务实现。

本报告只作为一次性进度审计。确认后的术语必须回写 `docs/context/domain-language.md`、`docs/context/discussion-decisions.md`、`docs/product/domain-invariants.md` 和 DISC-001 活跃计划；本报告本身不成为第二份权威共同语言。
