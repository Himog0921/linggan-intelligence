# Topic Map HTTP 合同

> 状态: 代码事实优先
> 最后核对: 2026-10-10
> 适用范围: TOPIC-MAP-V41-001 既有接口及 TOPIC-MAP-CORE-001 研究内核候选
> 事实来源: 手册 v1.1、当前 TopicMap DTO、0120 migration、Rust HTTP composition 与定向验证
> 冲突时以谁为准: 用户最新确认、当前代码和来源资格；本合同不授权运行库迁移/外发

## 页面与读取

`/topics` 为完整概览；`/topics/{canonical_key}` 旧暂定工作区保留。`/api/local/topic-map` 查询采用 camelCase：domainRef/topicRef/platform/windowDays/referenceWindowDays/overlay/path/includeReference。referenceWindowDays=0表示全部历史；概览windowDays缺省全时间。我方发布历史始终独立于这两个窗口。includeReference必须显式为true才把reference用途纳入概览统计。没有领域选择时先返回领域入口；领域材料集合来自 material_domain_usage，不由前端昵称推断我方或用户诊断。

读取包含 domains/scope/topics/works/statistics/journey/sources/candidates/changes/alternatives/ownCreators/methodVersion/sourceBoundary。Topic附bindingVersion用于导航归属版本竞争；definitionVersion用于内容定义引用。Topic identity/definition 与成员引用复用 TopicWorkspace；作品display/media由共享WorkResource Current读取。statistics由后端基于完整明确范围计算，各平台分列，样本rule未设为null；未知互动不进入中位/P90/规则分母。父主题作品是子集union去重。scope说明总量/读取覆盖/时间口径；作品分页不得替代完整统计。

旅程五主阶段固定 discover_understand/seek_assessment/choose_support/begin_practice/long_term_manage；其他保留跨阶段综合/背景/不明/待分析。主阶段分布每作品单值，涉及率允许跨阶段，matrix以本行主题去重作品为分母。格子引用精确workRefs；零不表示市场空缺。

## 明确写入

`POST /api/local/topic-map/commands` 使用带action的typed enum，unknown字段拒绝。action使用ownCreator/breakout/saveAlternative/bindTopic/performanceRule/viewed，字段idempotencyKey与domainRef；viewed含实际打开的workRefs，不以打开主题树代替阅读。全部成功必须真实提交后给receiptRef/subjectRef/revision/persistedAt；幂等键相同但payload不同冲突，版本冲突不覆盖。

我方身份为显式domain+platform+authorExternalId用途；爆款为canonical work手动标记。备选保留title/angle/说明、topic+definition、方法与作品引用，保存不启动模型/采集/跟踪、不保存第二份原文。读取/导出依据始终复核当前资格。

研究备选给researchResultRef，并且仅指定researchAngleIndex或researchOpportunityIndex之一。前者保存角度，后者保存可编辑的产品研究说明；kind由后端推导为angle/product_research。引用必须来自该接纳结果的相应条目，不能由界面拼造引用。

`GET /api/local/topic-map/alternative?domainRef={uuid}&alternativeRef={uuid}` 返回保存时的定义、说明与不可变来源引用对应的正文/OCR/ASR/评论片段。sourceState为available/version_changed_available/source_unavailable；当前版本变化时仍可读取合格的保存时原版本，并明确显示历史版本。当前Work/Domain/Comment访问限制继续复核，受限时说明/定义与片段隐藏，只保留最小引用及状态。导出每次重新读取此接口核验资格，再在浏览器本地生成有引用的Markdown，不写外部应用。

## 研究

`GET /api/local/topic-map/research?domainRef={uuid}` 返回设置、用量、run/task/result真实进度；`POST /api/local/topic-map/research/commands` 采用action configure/start/pause/resume/stop。configure明确modelConfigRef/dailyTokenLimit/runTokenLimit/automaticEnabled/collectionEnabled；没有具体用量配置不外发。start区分historical/incremental/on_demand。当前prototype按钮/页面读取/筛选/保存不能代替开启授权。

`runs[]` 最多返回最近 30 个运行；`queuedCount/succeededCount/failedCount` 和 `phases` 从当前任务状态聚合。`failedCount` 保留既有口径，包含 `failed` 与 `unknown_dispatch`，不把派发未知算作已知失败或成功。`taskIssues` 按任务 `updated_at`、`task_ref` 降序返回最多 20 项当前处于这两种状态的任务，每项固定 `taskRef/phase/state/reason`；`phase` 为 `extract/resolve/compare`，`reason` 保留数据库原因码或明确的 `null`。明细上限不截断总数；界面说明实际显示项数与完整计数，缺失或未识别原因显示未知，不能暗示自动重试。

`lastReason` 和 `inputScope` 保留兼容及审计含义；其中 `comparison/workComparisons` 是当时的调度检查点，不是实时任务状态。运行已经结束且任务失败时，界面使用 `taskIssues` 解释失败，不能继续用历史 `comparison_queued` 解释当前结果；无失败的已结束运行也不以旧排队码表示实时状态。`phases` 的 `extractQueued/extracting`、`resolveQueued/resolving`、`compareQueued/comparing` 分别显示提炼、归属、比较的待处理与进行中数量。

运行操作按真实状态显示：`queued/running` 提供暂停、明确停止；`paused` 提供恢复、明确停止；`daily_budget_paused` 表达额度等待并提供暂停、明确停止。`completed/stopped/run_budget_exhausted/failed` 不提供无效的运行恢复操作。明确新 Start 仍经过原输入复用、来源、未知派发及预算守门，读取和显示不改变重试或授权规则。

研究输入是 canonical source refs、不可变来源版本与可重建 fragment；不冻结敏感原文到新账本。所有断言引用允许片段内的 Unicode scalar start/end，区间使用原字段的绝对位置。越界、错误来源角色、外域或失效来源拒绝。采集、分析、结果接纳状态分责，已发 unknown 保留预算且不盲重发。日额度与 run 预算在实际 dispatch 之前原子核验；费用未知不计 0。

发送给模型的 coverage 不重复携带服务器使用的 `currentSources`、`sourceHashes`、`fragmentOrigins` 审计映射；这些字段在完整冻结 manifest 和来源恢复中保留。实际原文、引用 ID/绝对坐标、片段所属作品、讨论及选择范围完整传递。输入上限仍检查完整 system 与实际序列化 prompt，run/day 预算还计入输出预留；正文字符数达标不等于请求 token 达标，超限保持明确失败且不发送。

### 主题内核与证据归属

候选采用 `topic-map.research.v2` 提炼及 `topic-map.resolve.v1` 归属两个严格结构化合同，内核方法为 `topic-map.core.v1`。第一阶段从来源窗口提炼独立讨论，持久化草稿和进度；第二阶段逐条比较精确主题定义和原文。`matched/new/uncertain/out_of_scope` 分开，允许多主题归属。名称、词频和向量相似度都不是成员资格。新主题包含定义、纳入/排除条件及与召回候选的关系；完整边界等价时复用身份，同名但不同边界保留独立身份。机器只产生候选，正式定义及既有合并/拆分仍需明确操作。

`topics[].core` 补充定义来源、纳入/排除条件、当前范围的讨论量、支持/反例/背景计数与有依据的主题关系。`sourceState=source_unavailable` 时隐藏依赖受限来源的机器定义与条件；保留最小身份及受限状态。计数在 `windowDays/path/overlay/platform` 等最终作品范围确定后重算；讨论数、去重作品数和需求人数不同，不能互相替代。父节点继续采用作品并集去重，不把树导航关系冒充概念关系。

`works[].research.core.units[]` 给出稳定讨论身份、陈述、来源角色、证据角色、接受的 resolution、精确定义版本、归属依据、关系及原文引用。`comparedDefinitionRefs` 保留参与决策的定义依赖，未归属的近邻也不能在撤回后继续泄漏。`author/commenter/quoted/unknown` 与 `support/challenge/context` 是两个独立维度。父评论只能作为 `parent_comment_context`，不单独增加人数或当成子评论自己的观点。

### 来源窗口、覆盖与复用

正文、合格 OCR/ASR 与全部已取得且合格的评论进入主题专用读取，未取得部分仍未知。新一轮评论采集的 30 个身份上限不限制既有评论研究或保存引用重开。来源按至多 1200 个 Unicode scalar 的片段、至多 3000 个输入字符的窗口处理；每个窗口只含一个主来源及必要父评论上下文。输入还须通过实际模型配置的 token 估算上限和独立字节安全上限，超限明确失败，不以截掉尾部伪装完成。

`coverage` 记录当前来源分母、实际片段和窗口范围、已覆盖字符及限制；重叠区间去重；不同配置或同文重新观察不会增加讨论、覆盖字符或完成窗口。窗口去重包含父评论上下文范围，不因重复子评论而把未处理的父文分页算作完成。每个窗口的文本范围参与语义身份，物理 observation/job/derivative 仍冻结在审计 manifest。同一作品字段、同一评论逻辑身份或同一媒体资产位置取得相同文本，不因观察时间或处理版本变化自动重跑。重绑定必须重新核验资格、逻辑归属、绝对范围及文本 hash，不能跨作品、评论或媒体位置借用相似文本。原文、域/方法/配置、角色或实际 Signal 语义改变才构成相关输入变化。

同一个语义来源窗口只投影最新已接纳的研究版本，包括已接纳的部分结果。换模型后对同一段话的不同表述不会和旧版本一起增加讨论量；新版本仅完成一部分时如实显示部分，不借旧版本填成完成。旧结果与已保存条目仍保留各自的不可变历史引用。

主题定义变化与原文提炼分责：不因无关主题新增使全部正文重新提炼。定义事件对已接受讨论做有界召回，优先重新检查旧定义的实际依赖，并复用持久化讨论。每次记录 240 个讨论、12 个来源窗口的扫描上限及 partial 状态；后续归属仍经过同样的来源、模型、预算和显式研究授权守门。历史判断保持原 definitionRef，不转填新定义。自动关闭后，过去已完成的手动 run 不获得持续重评权限；新的显式 Start 冻结至多 10 篇作品和当时精确定义集合，最多授权 12 个原 run，继续消耗原预算并复用已有提炼。后续新定义不继承这份授权。原 run 的获准重评及比较尚未排空时，不提前标记完成。

### 部分结果、比较与保存

任务阶段为 `extract/resolve/compare`，进度的 `phases` 分别报告排队与运行。窗口还未全部完成、预算暂停或任务停止时，已经提交的有效讨论仍可读取；未接受草稿不进入归属和计数。部分结果 `resultRef=null`，没有可保存的草稿角度；`resultRefs` 只列真实完成结果。作者正文不可用但评论仍合格时，`work.readable` 与 `work.research.readable` 分开；`authorSourceState/commentSourceState` 明确说明各自来源状态。

显式 on-demand 范围可包含 1–10 篇作品。单篇需同时具有作者与评论证据才生成回应比较；多篇从当前讨论选择共享主题、反例和来源多样性代表，每篇最多两条，合计仍受 3000 字符窗口约束。比较使用原 run 和预算，冻结作品、角色、讨论、精确定义依赖及实际选择覆盖，不将未入选内容或互动相关性写成因果。自动研究仅在明确启用时排队相应工作内的作者/评论比较。重复打开同一范围或选择顺序变化不新开同一比较。

同一比较只保存一份研究结果，读取在实际 `selectedWorkRefs` 的每篇作品上可见；数据库的物理主作品不决定用户从哪篇打开或保存。请求范围中未入选的作品不获得该结果；主作品被当前列表筛掉时，其余已入选作品仍在完整来源复核后读取。比较条目的 `comparisonScope` 保留原 `resultRef/scopeWorkRefs/selectedWorkRefs/state/boundary`，不能把跨作品判断算作单篇新增讨论、旅程或覆盖量。`research.fragments[].workRef` 标明实际原文归属；跨作品引用不得冒充当前作者正文。角度和产品研究说明的 `evidenceWorkRefs` 固定实际所选作品，前端按原结果及原条目索引去重并传给保存接口，不改为当前页面作品。

每条角度/产品研究说明保留自身 `researchResultRef`、`researchMethodVersion` 及原结果数组索引，不能用合并展示数组的位置或最新窗口的方法替代；旧 v1 与当前 v2 窗口同时可读时，保存仍使用各自原版本。保存和导出按冻结 physical source、原字段绝对 Unicode 区间与当前资格重取正文/完整媒体文本/评论/父评论上下文；超过第 30 条评论或正文第 10000 字的合格片段仍可定位。来源撤回后隐藏相关原文及衍生说明；机器定义及其参与的新定义、比较和保存条目的明确依赖传递复核。暂停或来源变化不能抹去已发送但结果未知的事实，各配置的已排队请求在最后发送前再次检查相关原文范围的 unknown 记录。历史 v1 结果使用显式旧合同读取，不套用 v2 字段或当前新版定义。

旧版 `machine_proposed` 定义若没有可重建创建来源的 concept rule，不从材料成员记录猜造来源；隐藏其机器定义文本、排除后续召回，并限制依赖它的旧研究输出及保存条目。人工 `human_adjudicated` 定义保持自身裁定资格。v1 输出按完整冻结 prompt 中的精确定义依赖复核，不能因为某个主题最后未被归属就省略该依赖。

## 有界采集与结构变更

`POST /api/local/topic-map/collection/commands` 的 action 为start_collection/deepen_comments/stop_collection，字段requestRef/domainRef，明确topicRef/targetRef/作品及purpose。详情按一轮最多10个新canonical身份冻结；已有合格详情不额外占新身份，失败尝试仍占已预留身份。评论追加使用knownCommentIds+newUniqueLimit=30，根评论、回复和需要补齐的父评论共同计数，重复ID不重复占额度；服务器复核包与本轮预算。关闭弹窗继续原轮；stop显式终止后续准入，已取得材料沿既有资格接纳。

`GET /api/local/topic-map/search?domainRef={uuid}` 返回现有可用keyword/deep_archive授权与轮次；`POST .../search/commands` 使用start_search/freeze_search_details/stop_search。start_search显式给authorizationRef、1–3个keywords与purpose，最多200候选配额按词分配；TaskSpec把工单冻结的临时预算传到实际浏览器执行，单词最多3轮/180秒。freeze_search_details给roundRef与1–10个本轮已接纳候选workRefs，停止搜索并冻结后续详情身份。搜索完成的partial阶段仍可选择详情；旧轮重开复用，newRound=true是明确新开意图，不由窗口重开自动触发。

`POST /api/local/topic-map/structure/preview` 接收requestRef/domainRef/kind(merge|split)/sources(topicRef+definitionRef)/destinations(新名称定义、parentTopicRef、显式members)/rationale。预览给previewHash与影响集合、未分配作品、旧备选保留策略。`POST .../apply` 给plan+previewHash，重新核验版本与当前来源，原子创建新定义、分类、绑定及回执。旧定义/引用保留，旧判断不继承。含子主题的旧节点先用bindTopic调整导航归属，避免把子主题留在隐藏旧节点下。

## 安全与错误

沿用现有local HTTP Host/Origin/Sec-Fetch-Site guard：只允许localhost/127.0.0.1指定端口、同Origin，无CORS开放；所有响应no-store/nosniff。非法参数/JSON 400、不存在404、幂等与版本409、来源/数据库暂不可用503，失败不返回成功回执或伪造空库。guard拒绝403。

已有 `/api/local/work-resources/{ref}?domain={uuid}` 与 `/comments` 是正文/评论安全读取面；主题页只组合引用，不维护第二原文存储。

## 验证边界

隔离PostgreSQL fixture证明写入/幂等/引用/统计/预算；fake provider证明执行状态，不证明真实模型语义质量。源码与HTTP/browser可用、main合并、运行库迁移/3000部署、付费外发、真实采集和Mog业务验收分别证明。
