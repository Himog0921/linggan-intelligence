# 切片 A：单篇分析结果规范与精确接口设计

> 状态: 草案
> 最后核对: 2026-11-04
> 适用范围: 单篇分析手动发起、保存、展示及下游读取投影；本地切片 A 实现与隔离验证，未部署
> 事实来源: 用户两层合并决策、N02设计样例、源码f34dc08a3e1e7e1c57ffe365bc0d47e670d4f368
> 冲突时以谁为准: 用户当前要求与真实源码；实现状态见实施计划第 11 节；真实模型质量尚未验收

## 1. 本次决定

一篇笔记生成并保存一份结构化结果，前端直接渲染，下游按用途取字段。无需三次模型调用、三份独立摘要或全局知识图谱。

切片A提供4个HTTP接口、2张业务表、1个正式任务handler、1个结果组件及2种内部读取投影。复用现有任务设置、队列、调用账、来源权限和预算机制。HTTP预览是系统组装步骤，不新增确认弹窗；同一“开始分析”动作完成预览和提交。

切片B才修改图谱/评论研究消费者。本次把读取合同设计清楚，切片A可做投影函数与测试，但不宣称消费者已经接通。

新结果合同为note-analysis.result.v1；方法为note-analysis.method.v0.3，代表两层合并方向。原note_interpretation_offline与candidate.v0.1不作为本结果的存储格式；候选Skill已更新为v0.3。

## 2. N02：同一结果的三个视角

完整私有样例位于本工作树`artifacts/private/ai-midterm-quality-002/slice-a-design/`：

- N02.saved-example.json：请求记录模板、结果记录与完整业务内容。
- N02.frontend-projection.json：由同一结果机械提取的前端分组，无第二次改写。
- N02.downstream-projections.json：主题与评论用途的精简读取结果。
- checks.json：引句与内部引用检查边界。

示例不是正式数据库记录：work/domain/request/invocation等正式身份保留null，不伪造UUID；source坐标以exampleShareRef说明分享出处。正式接口不接受这些null身份或exampleShareRef，实施时用原始canonical来源绑定。实际评论角色、引用文字和数量来自N02分享包。结果是Agent编写，未作真实模型调用或用户质量验收。

| 视角 | N02具体内容 | 不额外生成什么 |
|---|---|---|
| 保存 | 启动作业的讨论、三个动作、家庭叙事与解释比喻、尝试意愿、介入边界追问、一条吸引力解释、9个候选标签、一条方法候选及来源 | 不生成正式Topic或评论problem对象 |
| 默认展示 | 概括；“讲什么、给什么”；关键做法与可能作用；已有反馈 | 不把9个标签全部排在页首 |
| 展开查看 | 标签及用途、方法条件、原文依据、材料限制 | 不生成另一个版本的长报告 |
| 图谱读取 | 讨论、面向对象、情境、回答、提供价值及其原文引用 | 不读取“恢复掌控感”等猜测来决定归属，不重复传评论概览 |
| 评论研究读取 | 讨论、情境、回答、明确互动指令及原文引用 | 不读取该目标评论的上游分类、心理解释或方法候选 |

N02默认概括：“以三个具体动作回应孩子坐下后迟迟不开始的问题；已有评论表达尝试意愿，也提出家长介入与自主性的相邻追问。”

核心解释：“熟悉的催促困境经由解释转折连接到明确动作，可能让有相近处境的家长觉得还有办法可试。”其支持说明为“有评论支持情境相近和尝试意愿；各表达要素的作用尚未被单独验证”。不写成已缓解焦虑或已改善作业。

## 3. 输入与模型输出职责

scope只有note和note_with_comments。note使用当前配置允许的标准底稿文字列；note_with_comments额外读取选定时点全部合格可读评论，包含作者标识、父级关系。平台总评论数仅作覆盖说明。首版不提供自由抽样器；超出实际模型输入额度时返回input_too_large，用户可明确改为note，不能静默截取。

预览冻结knowledgeCutoff；开始时重组同一时点输入并校验inputHash，事务中保存inputManifest。manifest保存实际发送的标题/fragment/comment材料和来源身份，复用底稿结构，避免另建snapshot系统。任务执行直接使用已保存输入，不重新捡取更晚的评论。当前来源资格按已有执行框架复核。

模型收到：标题、可读材料、选定评论、材料范围、Skill/Prompt和输出结构。不把平台指标喂给模型解释因果；页面仍展示既有指标。原图/视频不在本次文本条件中；没有看到的版式不要求模型解释。

模型生成业务内容与短引句。程序负责正式身份、范围计数、版本、生成时间、调用费用和引句位置。模型不能提交自称正确的UUID或伪造调用回执。

## 4. 正式结果结构

result为以下对象，所有集合以[]表示没有条目，不填“无”占位项。未知单值用null；summary、discussion、answer在有可读材料且成功时必有。没有可读材料不发起模型调用。

| 字段 | 类型／具体要求 |
|---|---|
| contract | 固定note-analysis.result.v1 |
| summary | string；压缩summaryBasisIds中的判断，保留归属与未知 |
| summaryBasisIds | string[]；指向本结果understanding/features/feedback中的判断 |
| understanding | 对象；discussion、answer为Statement；audience、situation、offeredValue为Statement或null |
| features | Feature[]；仅本篇有区分度的做法 |
| feedback | Statement[]；评论回应与追问，作者回复身份明示；不强制逐条评论输出 |
| explanations | Explanation[]；可以为空 |
| tags | Tag[]；少量，维度不要求填满 |
| methodCandidates | Method[]；可以为空 |
| limitations | string[]；只写材料缺口对本次判断的具体影响 |
| evidence | Evidence[]；程序将模型引句解析成正式来源后保存 |

Statement = {id:string, text:string, kind:observation|author_claim|interpretation, evidenceRefs:string[]}。

Feature = Statement + {label:string}。为了精确筛选互动背景，Feature额外有category:answer_form|expression|participation；其他内容归入understanding，不另造所有特征类型。

Explanation = {id, text, kind:"interpretation", featureIds:string[], feedbackIds:string[], evidenceRefs:string[], support:string, uncertainty:string|null}。feedbackIds可以为空，不强编评论印证；uncertainty写影响核心解释的未决点即可。

Tag = {label:string, dimension:discussion|audience|situation|offered_value|answer_form|expression|participation|response, judgmentIds:string[], retrievalUse:string}。judgmentIds只引用understanding/features/feedback；来源范围从判断及evidence推导，不再复制一套scope与证据。标签是候选，没有全局tagId和正式Topic ID。

Method = {id, text, kind:"reuse_idea", basisIds:string[], conditions:string, openQuestion:string|null}。候选方法关联本结果判断，不跨篇自动合并。

Evidence = {id:string, kind:material|comment, sourceRef:UUID, sourceVersion:string, citationId:string, start:int, end:int, quote:string}。start/end为对应来源文本的Unicode标量位置、左闭右开，与Rust字符计数一致。底稿证据沿用fragment坐标；评论以冻结原文为坐标。读取时可依据来源身份跳到原文，无需新建证据对象。若相同引句出现多次，模型需提供足够上下文或明确fragment，程序不能随意采用第一处。

id只在一次结果内部有效，唯一即可。程序检查引用关系、短引句与角色；不以格式通过宣称解释正确。正文里“作者说有效”必须保留作者主张性质，即使引用完全匹配也不能提升为独立事实。

模型线格式使用同一业务字段，但evidence只含{id, inputCitationId, quote}，由程序解析成上述Evidence；正式身份与位置不交给模型生成。所有模型inputCitationId来自manifest。正式result只存解析后的证据，原始响应沿用已有调用记录策略。

### 4.1 字段职责与证据传递（2026-11-04）

本节替换历次追加的单篇纠错说明，沿用现有两层结果和字段形状。最终冻结编号为note-analysis-baseline-20261104，边界与已知缺口见[frozen-baseline.md](frozen-baseline.md)。

| 位置 | 职责与依据 |
|---|---|
| understanding / features / feedback | 说明材料包含什么、谁说了什么；有依据的归纳保留interpretation，观察条目不混写读者作用 |
| audience | 内容面向谁；实际评论者姿态归反馈；有依据的推断保留interpretation，未知可null |
| offeredValue | 内容提供物，如规则、步骤、清单、例子或参与入口 |
| explanations.text | 具体表达特征如何可能帮助读者理解、产生兴趣或参与 |
| support / uncertainty | support写已确认的前提及联系理由，uncertainty写同一作用假设尚缺的支持 |
| summary / label / retrievalUse | 压缩或索引已有判断，保留主体、对象、程度、说话者及未知；用途只描述筛选目的 |
| methodCandidates | 内容表达或研究方法与迁移条件，仍可依据解释假设 |

“评论者把数字解释为完成个数”只确认解读行为。它在摘要、标签或提供价值中不能变成已核实成绩；“可能帮助读者看懂这些数字是成绩”也已经偷带了成绩这一事实前提。解释仍允许产生新理解，但所用前提和推测部分须分清。

summaryBasisIds与tags[].judgmentIds现在均只指向understanding、features、feedback；程序通过已有base集合校验标签，不直接接收指向explanations或methodCandidates的标签。基础判断仍可为interpretation，这不是事实白名单。首版的取舍是标签暂不检索解释假设；数字用途未知时按可见文本筛选，已观察回应归response、明确互动设计归participation。评论身份仅写输入确认范围，不从不同评论ID增补不同人。假设继续在解释和方法候选中供研究。方法basisIds范围不变。

所有判断evidenceRefs、解释featureIds、标签judgmentIds、方法basisIds及summaryBasisIds非空；解释整体及feedbackIds可空。quote逐字复制输入的唯一匹配短句，包括错字、拼音替字和标点，释义另写。来源类型和角色依输入，限制说明影响本篇的实际缺口。

程序检查结构、来源与引用范围；人工沿证据→判断→解释→摘要/标签检查含义是否保持。合法ID不能证明自然语言得到支持，不新增评分器、分析层或自动引句修补。

结果仍为result.v1，方法入口method.v0.3；内容修订由Skill/Prompt版本及哈希追踪。此次只修改未部署候选的提示词种子，没有执行迁移或更新正式配置。已初始化的隔离库不会自动更新提示词。历史试跑结果不回写；历史标签若直接引用解释，在当前校验下会返回tag_basis，不静默转换。

N03正反例及新接收器验证在私有 `slice-a-design/field-responsibility-implementation/`。人工稿可承载此语义不代表模型已遵守新职责。

## 5. 保存设计：两张表，状态复用队列

拟议表linggan_note_analysis_request：

| 列 | 类型／意义 |
|---|---|
| request_ref | UUID主键；同时作为analysisRef，不再创造第二个结果ID |
| command_ref | UUID唯一，引用现有命令账；同一提交重放返回同一request |
| domain_ref、work_ref | UUID；作品与分析领域 |
| scope | text：note或note_with_comments |
| knowledge_cutoff | timestamptz，输入观察截止 |
| input_hash | text，冻结输入manifest的规范JSON哈希 |
| input_manifest | jsonb，实际输入、来源、范围与materialSnapshotIdentity |
| content_fingerprint | text，用于判断源材料是否已更新；只哈希正文/识读/所选评论语义及来源版本，不含指标、观察时钟和任务配置 |
| task_config_ref、task_revision | UUID、bigint，开始时采用的配置及修订 |
| method_version | text，首版note-analysis.method.v0.3 |
| batch_ref、item_ref | UUID唯一，关联现有单项interactive批次及执行项 |
| created_at | timestamptz，程序生成 |

拟议表linggan_note_analysis_result：request_ref主键且外键；invocation_ref关联已有调用；result jsonb可空；failure_code text可空；created_at。一次终态写入，成功result非空且failure_code空，调用失败则相反。未进入调用即被阻止的状态只从现有batch item读取，无需伪造invocation/result行。

输入manifest持有冻结材料；result持有分析与必要短引句，不再存第三份全文。模型/Skill/Prompt版本通过冻结task config及调用记录关联获取，不维护重复版本主账。新增迁移号实施时按当时仓库分配，不预占号码。

索引：request(domain_ref,work_ref,scope,created_at DESC,request_ref DESC)用于latest；其余使用主键/唯一约束。当前没有全站标签查询需求，不先建标签索引和独立标签表。

新增InputRef::NoteAnalysis { request_ref }供正式handler使用，保留旧Note { snapshot_ref }历史输入，不改它的既有含义。允许关系、schema与admission一起扩展；handler只处理新输入类型。

## 6. HTTP接口：四个

所有接口沿用local API的鉴权、领域来源校验和错误响应形式。下面JSON中的尖括号是说明占位，不是实际请求。

### 6.1 GET /api/local/note-analysis/preview

查询：domainRef=UUID&workRef=UUID&scope=note|note_with_comments。无写入、无模型调用。

200响应：

```json
{
  "contract": "note-analysis.preview.v1",
  "domainRef": "<uuid>",
  "workRef": "<uuid>",
  "scope": "note_with_comments",
  "knowledgeCutoff": "<timestamp>",
  "inputHash": "<sha256>",
  "taskConfigRef": "<uuid>",
  "taskRevision": 1,
  "methodVersion": "note-analysis.method.v0.3",
  "coverage": {
    "materialColumns": ["title", "body", "ocr"],
    "includedCommentCount": 6,
    "authorCommentCount": 3,
    "platformCommentCount": 10,
    "issues": ["原封面图未纳入，本次使用校对文字"]
  },
  "canStart": true,
  "reasons": []
}
```

coverage计数来自程序，platformCommentCount可null；scope=note时includedCommentCount为0，说明未选评论而非没有评论。配置缺失、材料额度超限或调用未获准时canStart=false及原因，不能以preview成功代表允许调用。canStart的核对不触发探测。没有可读材料时同样给出no_readable_material，不填假输入hash，inputHash为null且不能提交。

### 6.2 POST /api/local/note-analysis

请求：{commandRef:UUID,domainRef:UUID,workRef:UUID,scope,knowledgeCutoff,inputHash,taskConfigRef:UUID,taskRevision:int}。客户端不提交正文、标签或模型输出。

成功接受返回202：{requestRef:UUID,batchRef:UUID,itemRef:UUID}。事务内验证预览输入与配置、保存冻结manifest和单项批次。相同commandRef及相同请求返回同一回执；不同内容沿用现有命令冲突处理。

预览失效或配置变化返回409并要求重新预览；明确409后才生成新的提交。网络结果未知时保留原commandRef和请求重试，沿用现有命令语义，不新增重试编排。

### 6.3 GET /api/local/note-analysis/{requestRef}?domainRef=UUID

返回：{contract:"note-analysis.read.v1",requestRef,workRef,scope,run,result,coverage,freshness,versions}。

- run={state,batchRef,itemRef,invocationRef,failureCode}，state投影既有pending/running/succeeded/failed/skipped/cancelled；不新增平行状态机。
- result：第4节对象或null；运行中/失败没有新结果，不能返回半份成功内容。
- coverage：本次冻结范围。
- freshness：current|source_changed|not_checked；由当前可读材料与contentFingerprint比较得出，指标变化忽略。若来源本身不可读，沿用来源访问错误，不返缓存正文；not_checked只用于当前版本检查暂不可得，不等于权限允许。
- versions：methodVersion、taskConfigRef、skillVersionRef、promptVersionRef、modelConfigRef、createdAt；来自冻结记录和调用，不由模型编造。

### 6.4 GET /api/local/note-analysis/latest

查询：domainRef、workRef、scope。返回{contract:"note-analysis.latest.v1",latestRun:Read|null,latestSuccessful:Read|null}。Read为6.3对象。

latestRun按created_at、request_ref倒序取最新请求；latestSuccessful取最新成功且仍可读的结果，同scope内选择。两者可能指同一次记录；客户端按requestRef识别。新一次失败时保留上一份成功结果并显示其日期/是否已更新，不让错误面板覆盖可读分析。两者皆null表示尚未分析。无需新增列表历史接口，具体历史requestRef仍可直读。

400：参数不合法；404：领域内找不到作品/请求；403：按现有访问或调用约束拒绝；409：输入或配置冲突；503：基础服务暂不可用。具体已有failureCode优先复用，只有没有对应语义时才新增。模型输出不合格属于任务failed，而非启动接口伪返回成功分析。

## 7. 前端展示与操作

在现有证据库详情Inspector的阅读区域增加NoteAnalysisPanel，保持底稿可直接阅读；不重做整个详情页，也不新增主导航。

进入时只调用latest，不自动发起模型请求。无结果显示“分析这篇”；材料范围默认note，用户可选择“包含已有评论”，选择后记住当前页面状态。点击开始后自动preview→POST→轮询具体request，终态刷新latest；UI防止重复点击，服务端使用commandRef。调用设置不具备时显示可行动原因及现有设置入口。

N02默认展开：

1. 一句概括。
2. 内容理解：对象／情境、核心回答与提供价值，用自然语言分组，不展示字段名。
3. 关键做法与可能作用：features和explanations按关联连接显示；有些feature没有解释时保留事实，不凑机制。
4. 已有反馈：尝试意愿与介入边界追问。

展开项：候选标签及其检索用途、方法候选与条件、证据原声和本次材料范围。标签首版作为可读特征，不做假链接；尚未实施全库筛选时不暗示可以点击检索。

点击判断的“依据”显示quote及原文位置，能回到当前作品底稿/评论；历史来源版本不能对应当前阅读时显示冻结引句与版本，不错跳新正文。来源访问规则沿用现有证据库，不新建权限系统。

重新分析仅在用户操作时发生。旧结果标示材料已更新即可，不弹多层确认，不要求用户批准每条解释或标签。桌面验证，不增加移动端工作。

## 8. 下游读取合同：切片A可测，切片B接入

采用Rust内部读取函数，不新增公开HTTP接口：

read_note_analysis_context(db, domainRef, workRef, materialSnapshotIdentity, purpose) -> Option<NoteAnalysisContext>。

purpose为topic或comment。选择最新成功、该领域作品相符、正文/识读身份与调用方一致且仍可读的分析。两个scope均可，按时间选；投影只包含正文支持的判断，因此新增评论不会迫使下游丢弃仍有效的正文背景。调用方使用的asOf与来源资格继续由自身冻结机制处理。没有适合结果返回None，沿用当前原文路径。

NoteAnalysisContext={contract:"note-analysis.context.v1",analysisRef,materialSnapshotIdentity,purpose,understanding,interactionFeatures,evidence}。

- topic：understanding保留5个字段；interactionFeatures=[]；evidence仅取这些判断所用证据。
- comment：understanding只取discussion/situation/answer；interactionFeatures仅取category=participation且证据全为material的features。
- 两种投影均只包含evidence.kind=material支持的Statement；若判断混入评论依据则该字段不投影（必需的discussion/answer缺失则返回None）。因此内容理解应尽量由作者材料支撑，评论增量留在feedback。
- 不含summary（它可能综合评论）、feedback、explanations、tags、methodCandidates；不隐含向下游导入读者心理或目标评论结论。
- 不给投影再调用模型、不单独存一张上下文表。下游在切片B冻结实际采用的analysisRef与投影，并纳入自己的输入身份。

N02给图谱的主体是“作业启动＋孩子坐下未开始＋三个动作”，给评论研究额外提供“结尾征集启动经历”。介入边界追问仍由原始评论及评论研究处理，不伪装成正文主题。具体投影见私有样例。

## 9. 具体改动范围（切片 A 本地候选已实现）

| 文件／区域 | 计划变化 |
|---|---|
| 新增crates/intelligence/src/note_analysis.rs及其小模块 | 结果类型、验证、引用解析与两种确定性投影；只按职责拆分，不预建框架 |
| 新增crates/intelligence/src/ai_foundation/note_analysis.rs | preview/start/read/latest、冻结输入、结果存储与来源读取 |
| 新增executor/service/note_analysis.rs；修改executor/service.rs | 单篇一次调用handler及任务分发，复用pi_adapter和调用回执 |
| ai_foundation.rs、lib.rs | 模块导出 |
| ai_foundation/batch_store.rs、admission相关分支 | 新NoteAnalysis输入引用、材料范围和队列资格；保持旧Note引用语义 |
| ai_foundation/provider_store.rs、task配置/Prompt/Skill绑定 | 允许该任务使用comment_material；固定本任务输入与结果合同 |
| 新增一份database/migrations迁移 | 两张表、必要索引、任务handler与方法配置、精确材料类别约束扩展；不自动开启调用或授予外发 |
| 新增apps/api/src/local_web/note_analysis_api.rs；local_web.rs及路由组合处 | 4个接口，复用local鉴权和错误转换 |
| apps/web/src/pages/corpus/evidence/新增NoteAnalysisPanel.tsx、noteAnalysisApi.ts、noteAnalysisTypes.ts及局部样式；ReadingPanel.tsx | 阅读区域入口、分析结果、依据展开、scope选择与请求状态 |
| settings/AiSettingsExperience.tsx、AiModelSection.tsx | note_interpretation的comment_material设置映射，保持前后端一致 |
| 当前候选Skill与输出指南 | 依据已确认两层结果更新为v0.3；首轮质量验证后再定稿 |
| schema readiness、迁移fixture、受影响测试 | 跟随新增迁移/handler登记更新现存测试基础；不另建测试平台 |

切片A不改topic_map_research/source.rs、comment_study_selection/input.rs的消费逻辑，不改图谱resolve、评论problem配对。它们是切片B的具体接入点；仅内部投影存在不能算实际接通。

## 10. 验收与未决项

A的最低验收：N02能发起、生成、保存、刷新后重读同一结果；默认页面能读懂三步回答和吸引力解释；9个标签各有判断依据；方法候选有条件；评论尝试意愿不会变成效果；图谱/评论投影不含反馈结论和吸引力解释。

程序验证重点：引用准确；同commandRef重放不重复提交；配置/输入明确变化返回409；无评论可正常分析；新调用失败不抹掉旧成功；指标变化不误报语义更新。必要持久化流程用隔离数据库验证，桌面核对结果与来源交互。纯结构与mock不替代真实模型内容验收。

本轮N02设计示例使用11条精确引句，内部判断引用和JSON可读性已检查；正式canonical身份尚未绑定，不能作为端到端执行证据。

仍需明确：真实模型与实际输入容量、预算；N02模型输出的业务内容是否达到用户要求。迁移号0191与底稿/评论来源映射已实现并在隔离库验证。数据结构、四个接口、两个存储记录、展示分组与下游取用范围已经可供实现评审。真实模型调用另提交方案和预算，不在本轮执行。
