# 研究底稿 v2：统一阅读与唯一读取合同

> 状态: 权威当前
> 最后核对: 2026-10-26（统一当前读本、按需修正与 OCR 结果清理候选）
> 适用范围: RESEARCH-DOSSIER-V2 / STANDARD-DOSSIER-001；字段与行为合同，发布状态单独核对
> 事实来源: Mog 本轮确认的研究底稿标准、开发期直接替换要求与本合同验收矩阵
> 冲突时以谁为准: 用户最新确认、真实来源与验证；本稿替代 v1 及 note-material-input 的并行对外合同

本合同按 Mog 2026-10-26 的最终范围更新：保留最新版本地 Paddle `local-v4`、既有 `local-vl-v1` 与当前 DeepSeek-v4-flash 接口，统一发布到当前读本；删除旧版本兼容能力和历次机器 OCR 结果。本次修正仍是交付候选，尚未合并、部署或执行生产清理。主线、运行与迁移事实分别以实际发布回执为准。

## 用户结果与范围

STANDARD-DOSSIER-001 更新：平台非空标题优先，否则从明确封面（无封面时为已知第一页）的主要标题块整理 displayTitle，依据为 platform / cover / missing，并保留 titleBlockRefs。页序未知不猜封面。无正文时图片按页序成为主文；视频优先口播，尚无口播时已有画面文字可读；有可读口播/画面时封面归补充，只有封面文字可用时才作为主文。混合媒体中的正文图片仍按原图片规则。有任意非空平台正文时正文为主文，其余均为补充；短句、表情、标签正文同样保留。

`readingView.mainBlockRefs` 与 `supplementBlockRefs` 仅引用 `materials[].blocks`，由后端唯一组装。JSON、Markdown、前端与内部 AI 片段适配采用该顺序；显式 selection 后重建标题与编排，不泄漏未选图片内容。coverage 保留材料实际缺失、处理和失败；它不是整篇读取资格。没有文字时仍返回底稿及原件，文本消费者明确返回无可读文字。

分段读取复用同一 GET：`segmentSize`（1–200）与 `segmentOffset`（从 0 起），返回有序块的显式选择和 `segment` 范围。后续请求必须携带首段 `asOf` 与 `expectedRevision`，固定完整底稿 revision；来源受限仍按当前访问规则处理。分段只筛选完整底稿的主文/补充有序引用，不因本段缺少正文而提升补充或重排；显式用户 selection 仍重新编排。遍历到 nextOffset 为 null 即覆盖全部可用块，不按字符截断。

本地常规 OCR、本地 VL 与手动 DeepSeek 识别完成后，都经同一 canonical owner 自动发布完整读本。发布使用任务冻结的 expected current 作 CAS，同来源人工修正优先。GET 不准备或识别；原生材料与历史 ASR 的显式准备继续复用内部 dossier_input，旧图片 relayer 及其批量界面已从本次候选移除。

人打开一篇笔记，应能连续读到已取得的所有图片文字，保留数字、单位、否定、清单、表格、对话、引述及不确定处，并能回到原图。Agent 读取相同材料，按明确任务选择调用；主题图谱、评论研究、作者研究不各自重建另一份笔记事实。

默认读取整篇。没有字符、页数、词元预算截断、自动摘要或自动删除重复出现的原文。去冗余指去除运输层的同内容多份表示；原文在两页实际重复，仍是两个来源位置。模型容量不足时返回明确的容量错误，不静默改写或缩短输入。任务明确选择若干篇、评论、图片或区块不属于截断。

检查器的外部原文导航独立于底稿内容：作品详情 `item.inspector.sourceNote` 返回最新已接纳发现记录的 `url`、`state`（KNOWN/UNKNOWN）和 `sourceRef`。只允许实际链接与当前小红书笔记身份一致且包含非空签名；最新记录没有合格链接时保留UNKNOWN，不回用旧签名或按ID拼接地址。此只读字段用于用户主动打开原文，短期签名不进入本合同的JSON、Markdown、研究快照或AI材料；链接可访问性仍由平台当前状态决定。

开发期直接切换到唯一 `research-dossier.v2`。删除旧运行接口、旧消费者、兼容解码和字段级兜底；保留历史迁移与原始来源证据的审计价值。旧派生数据无需保活，重新生成必须是显式操作。现有领域权限、来源限制、模型出站授权和调度开关继续生效。

## 一份内容，按需表示

默认 JSON 只有作品身份 `note`、按阅读顺序排列的 `materials[].blocks`、覆盖说明 `coverage`、按需读取入口 `attachments` 和明确选择 `selection`。每份材料正文只出现一次，不同时序列化 fragments、raw OCR、media 文本、Markdown 与多个任务预览。

Markdown 通过 `format=markdown` 单独生成，与同一次读取的区块顺序和内容一致。评论、作者与互动元数据分别按需读取；评论页数不冒充平台评论总量。原图和识别诊断通过来源引用读取，不把原始布局/任务账本塞入每次 Agent 输入。

Markdown 顶层采用同一 displayTitle；为空时仅用界面占位“无标题”。标题来源块继续保留角色、时间、局部问题和精确引用，不同来源出现同文标题各自保留。显式选择后重建展示标题，未选图片文字不会带入导出。

顶层字段：`contract`、`workRef`、`revision`、`materialRevision`、`asOf`、`note`、`materials`、`readingView`、`coverage`、`attachments`、`selection`。`materialRevision` 绑定材料、来源和结构；`revision` 还绑定同截点的覆盖及辅助元数据；默认读取不附带另一份识别全文。时间戳本身不令未变材料失效。

`note` 保留平台、作品身份、平台标题及来源、展示标题及采用依据。平台标题与图片标题是不同事实。仅显式封面或已证明的第一页标题可补展示标题，不能伪造平台标题。titleBlockRefs 记录封面整理标题的实际块；不同来源的同文区块仍各自保留。

## 材料与结构化区块

材料可附 `purpose` 与 `processing:{state,reason}`（来自固定 asOf 的 job event；succeeded 映射 completed），不读取可变 work 状态冒充历史。材料字段：`ref`、`kind`、`slotKey`、`page`、`sourceState`、`sourceRef`、`sourceVersion`、`currentRevisionRef`、`originalUrl`、`reading`、`blocks`、`issues`；仅在字节证明建立页序关系时增加 `pageSource`。材料种类为 body、tags、image、audio、video_frame。未知页码为 null；不按数据库遍历序号猜平台页码。已知页序优先，未知页序沿真实 producer_ordinal 的稳定观察顺序，再以 slot_key 稳定打破同序关系；不按槽位字符串把第10次观察排到第2次之前。

`reading` 分开表达生成者 `generatedBy`、采用状态 `adoption`、核对状态 `verification`、规范化规则 `ruleVersion`，以及可空的 `review:{commandRef,actor,reason,action,reviewedAt}`。review 只来自形成该当前版本的人工修正命令；自动读本及原生标签为 null。既存真正人工内容的历史来源标记可继续读取，当前写命令仅支持 edit。生成、进入当前读本与人工核验分别表达。生成者取 platform、ordinary_ocr、local_vision、cloud_vision、asr、frame_ocr、human_edit。

区块拥有稳定 `ref` 与一种内容形态：

| kind | 内容 | 语义约束 |
|---|---|---|
| heading | text | 标题层级可显式标记；图片标题不等于平台标题 |
| paragraph | text | 不删除短句、否定、单位或正常重复文字 |
| list | items、ordered、可选 start | 条目顺序与编号保留 |
| table | columns、rows | 单元格边界与空格保留；不把表格摊平为无关联句子 |
| dialogue | turns | 每轮 speaker 与 text；他人发言不归为作者立场 |
| quote | text | 引用角色明确或标未知 |
| gap | text | 只标局部缺口，不补造原文 |

所有区块默认角色不明为 `unconfirmed`，确定角色为 `author` 或 `other`。角色未知不等于删去文字；必须随上下文进入研究任务。坐标仅在真实来源支持时输出，格式 `[x,y,width,height]`，范围 0..1；音视频位置为真实毫秒区间 `time:{startMs,endMs}`。帧只证明一个采样时点时，起止相等，不虚构持续时间。没有真实坐标或时间时省略，不编造。

OCR 低置信度局部成为 gap；可信的周围文字保留。gap 文字是识别诊断，不计作原文资格：全部为 gap 时公开底稿保留块和原件，文本消费者获得无可读文字；真实原文与 gap 混合时保留局部缺口及出处。原始识别行留作诊断，不通过白名单、UI 词表或“作者正文资格”把整张截图的可读内容过滤掉。无法可靠恢复列顺序、表格或角色时，明确记录结构问题；不能把估计说成原图事实。

## 唯一当前版本与不可变历史

数据库分责：

1. `linggan_material_unit` 是作品内材料位置，持有 sourceVersion、显式 currentReadingRef、单调 revision、采用与核对状态。
2. `linggan_material_reading` 保存不可变的完整区块、局部问题、生成者、规则、来源版本、依赖版本及 publication 类型。
3. `linggan_material_reading_selection` 追加每一次当前选择、来源变化与核对决定；历史读取按事件截点恢复，不读取今天的可变指针冒充昨天。
4. `linggan_material_reading_command` 以 commandRef 保存修正人、说明、完整请求指纹和回执，命令顺序保持确定。相同 ID、相同请求重试返回原回执；相同 ID 不同请求冲突。

媒体 sourceVersion 指向精确 materialization；正文 sourceVersion 指向不可变原生 material。媒体必须对应该槽位当时的最新来源 generation，不能从较旧 generation 借字节填缺。所有当前与历史读取仍检查当前受限、撤回与退役资格。

Live Photo 的静态图片与动态视频按实际已验证媒体类型分别选择来源；图像 OCR、音频与帧不能因另一组件后完成而失效或串用来源。已清理字节但未撤回文字的来源保留不可变文字，原图入口明确不可用，并拒绝新的人工决定；已完成命令可返回原回执，但仍检查当前来源与撤回资格。

平台标签沿用不可变 `content-hashtags.v1` 来源投影，保留原序与重复条目，不从正文猜标签。其 `currentRevisionRef` 为 null，sourceVersion 绑定 material 与投影规则，区块引用绑定 material；这属于同一底稿的原生材料，不另建可编辑标签读本或另一份正文。

日常本地 OCR、本地视觉任务、手动云端 OCR、平台正文、ASR 与当前帧处理均通过 Automatic publication 发布，以捕获的 expected current 作 CAS，并保护同来源人工内容。CAS 冲突或人工保护生效时，生成结果不替换当前；任务完成、结果被保存和当前仍指向该结果分别核实。

正文在来源接纳时规范化并持久化。已有平台原文但缺规范读本时，GET 只读投影原文块及不可变 material 来源，不伪造 currentRevisionRef；显式 native 准备可补持久读本。重复准备幂等，GET 不写库、不启动识别；已存在的人工作品优先。

图片未取得、已取得但尚无读本、已处理为空、处理失败与已有读本分别表达。明确 no_text/no_speech/no_audio/no_frame_text 原因优先于一般完成状态；缺少当前读本不能解释为原图没有文字。图片识别是否已进入底稿，以实际当前指针与来源资格为准。

历史 ASR 沿现有 `dossier_input` 冻结计划与逐作品事务显式准备，CLI 操作为 `historical_media`。只接入当前合格 video/live_photo 素材、真实成功 ASR 任务及非退休/受限的本地转写结果，保留 job、derivative、materialization、blob 与全文哈希；旧任务缺少 reading_source_version 时不伪补冻结字段或重跑识别。计划另冻结历史结果来源指纹，来源改变拒绝应用；同源兼容当前读本及人工读本优先，原计划重复应用只读幂等。全文取 text_content，不用截断 display_text；没有时间元数据的旧结果保留时间未知和未核实问题，不补造时码或作者身份。明确未检出语音以 canonical 空读本闭合 known_empty，无音轨为 not_applicable，不造文字块。无合格 ASR 结果逐项回执 unavailable，不阻止其他来源及已有正文。历史 frame OCR 不通过此准备操作重新接入；当前本地帧处理能力继续保留。

实际字节改变后，旧 reading 不再冒充新源当前，新读本不继承旧来源的人工核验。相同槽位经已验证相同字节重观察时，允许保留真实人工读本和已打开的编辑基底；sourceVersion 继续绑定实际 reading 的来源，不把新观察身份改写到旧 reading 上。迟到的旧源任务不能抢占新源当前指针，派发和精确引用回查继续核对来源限制。

## 按需人工修正

默认阅读直接使用当前读本。需要改字时，从该材料的原件入口打开同一个逐块编辑器；当前尚无读本但来源已取得且可读时，可以人工录入。空当前不是编辑失败，也不要求先生成或选择另一版本。

唯一写入口为 `materials/{unitRef}/review`，命令只支持 `action:"edit"`。请求字段如下：

| 字段 | 含义 |
|---|---|
| commandRef | 本次保存的稳定请求 UUID |
| unitRef | 字符串，与 URL 的原材料 locator 完全一致；可为持久化 UUID 或服务端可解析的当前材料 locator |
| sourceVersion | 当前编辑基底实际绑定的来源版本 |
| expectedCurrentRevisionRef | 当前读本 CAS 基底；尚无读本为 null |
| editedBlocks | 完整结构化修正版，允许空数组；正文没有字符数截断 |
| actor / reason | 非空修正人和说明，分别最多 120 / 300 个字符 |
| verified | 是否已对照原件核验；保存本身不自动核验 |
| resolvedIssueKeys | 本次明确解决的文字问题 key |

服务端从作品及当前底稿解析 locator、检查来源与访问资格，必要时确保空材料 unit，再复用同一个 edit/CAS owner。成功回执包含 commandRef、原请求 unitRef、action、非空 currentRevisionRef、adoption:"human_edited" 与 verification。底层仍记录真实 unit UUID；对外回执保留原 locator，确保空材料第一次录入也能核对同一请求。

相同 commandRef 与相同内容重试返回原回执；相同 UUID 携带不同内容冲突。网络中断、5xx 或无法解码的成功回执属于结果未知：保留原命令、基底与完整草稿，锁定修改，只核对同一请求。只有明确的拒绝回执才释放请求身份。当前读本或来源冲突不覆盖新版本，未提交草稿保留，使用最新读本须显式进行。

结构化编辑保留区块种类、表格单元格与对话角色。文字或结构变化且无法准确对应原位置时移除旧 bbox/time；沿用 blockRef 不等于位置已获证明。`reading_edit` 问题可由编辑并明确勾选解决；`source_refresh` 问题须补来源，文字修改或 verified=true 不能消除。未明确解决的问题继续存在。

已清理原始字节时，合法已得文字仍可阅读，禁止新的人工修正；已保存命令可在来源仍有资格时回放。撤回或受限后，旧命令回放也重新检查来源，不能带出已受限的文字。旧独立质量抽检系统及第二套采用流程已从本次候选移除。

## 图片、视频与覆盖

完整图片序列只从平台显式 imageList 获得。保留原始位置、缺失位置、相同 URL/字节在后页的真实重复；DOM 扫描只能给 observed_set，不声明平台图片总数。

封面与第一页只有在字节一致、位置关系明确时才合并为同一内容来源；第 3 页再次使用相同图仍保留。没有证明的“封面就是第 1 页”不可用于补页码。

合并优先保留实际可用读本与明确人工决定。若保留的读本来自封面，保留其真实 unit、reading、source 与区块引用，并以 `pageSource:{basis:"same_verified_bytes",sourceRef,sourceVersion,slotKey}` 标明页序证据；不能把文字改挂到另一条来源。不同已采用版本无法等价时同时保留并标明 `duplicate_image_readings`，直到来源核对完成。

最新完整平台图片清单使已移除的旧槽位失去当前资格；后续局部观察只增加自身取得的槽位，不能复活被移除的旧页。读取、任务领取、当前发布、修正、显式准备与检索共享同一成员判定。当前观察未提供完整清单时，平台总数继续标为未知。

coverage.imageInventory 分别给出 basis、expectedPages（未知为 null）、acquiredPages、processedPages、missingPages、unknownOrder。已知总页数时统计只覆盖对应真实页，不能把额外封面算成第 N+1 页；处理数 ≤ 取得数 ≤ 已知总数。

音频采用实际转写 segment 的时间。帧 OCR 保留所有实际采样帧及真实 PTS，不设固定 12 帧读入上限、不跨时间全局去重。采样视频明确标 incomplete；未采样位置未知。只有已证明无音轨才是 not_applicable；ASR 成功完成但未检出语音为 known_empty（不声称原视频无文字），失败为 processing.failed 并保留此前同源可读读本。其他识别器无输出不证明源内容为空，也不生成诊断占位正文。

来源状态：present、known_empty、not_acquired、not_applicable、restricted。取得来源但尚无 canonical reading 为 not_processed。部分内容可用仍可阅读，不用一个 completed 压平缺页、失败、待核对与处理进度。

## 读取、选择、引用与任务接入

现有 `/api/local/work-resources/{workRef}/research-dossier` 成为唯一笔记读取入口。支持 asOf 与 format。默认 full；kinds、unitRefs、blockRefs、imagePages 是显式选择，采用并集；空值、未知 ID 或非法枚举返回错误。selection 报实际选择、省略材料数与完整材料 revision，禁止把局部表示标成完整。

readings/{readingRef} 在同一只读快照返回 workRef、所请求版本 material，以及顶层 currentRevisionRef（当前仍有来源资格的 unit 指针，可为 null）。material.currentRevisionRef 保持请求的版本身份；旧成功回执不能替代最新 current，去重隐藏的 unit 也按该指针精确读取，不重复运输当前全文。

按需入口包括 readings/{readingRef}、materials/{unitRef}/review、metadata；评论沿用授权分页。旧 candidates 与 relayer 端点已移除。批量读取固定一个共同 asOf，为每篇返回独立结果，某篇缺失不伪装整批成功或抹去其他有效材料。

识读引用必须绑定作品、unit、reading、blockRef、sourceRef、sourceVersion 与 asOf；blockRef 采用 `workUUID.readingUUID.ordinal`。不可变原生标签使用 `workUUID.materialUUID.ordinal` 并绑定投影规则。相同文本不是相同来源。冻结引用精确匹配，不按字段名或相似文本找新内容顶替；旧引用缺少必要字段则明确不可解析。

内部任务 adapter 的 readingTitle 元数据复用统一展示标题、basis、titleBlockRefs及平台来源；封面标题不标为平台标题，不发送“无标题”占位。adapter 可以生成短引用编号和来源映射，但正文只发送一次，结构、角色、问题与真实位置必须保留。评论研究携带选定评论、必要父回复和完整作品语境；作者研究携带所选作品及独立作者材料；主题研究窗口不得截取一篇的前 N 字冒充整篇。

主题已保存来源按请求的统一 asOf 恢复正文、读本、当时的作者标记及已完成评论研究血缘；新增读本、Signal 或人工标记不得改写旧请求的来源身份。当前来源限制、研究域与定义资格仍单独核验。历史结果的可读性与当前材料覆盖率分责：只有仍匹配当前精确来源的分析区块计入当前覆盖，新增或更新材料不能因旧结果存在而被报告为已经完成分析。

任务快照只保存版本引用、选择与指纹，不建立第二份长期原文库。`dossier_input` 仅是内部准备工具，不提供第二个公开 note-material-input 合同。内部冻结计划为 `research-dossier-plan.v3`，回执为 `research-dossier-receipt.v3`，操作范围保留原生材料、标签、快照及历史 ASR；新版不含 relayerSourceHash 或 relayer 字段，旧 v2 计划须重新生成，不兼容解释。公开读本仍为 research-dossier.v2。默认读取不触发 OCR、模型、研究任务、补采或数据库写入。

## 作者、发布时间与互动元信息

`metadata?sections=author,interaction,publication&asOf=...` 按需读取与当前底稿相同的截止时间；不把打开详情时的实时时间混入已冻结的快照。

publication 返回 publishedAt、localDate、sourceText、sourceField、sourceKind、precision、referenceObservedAt、parserVersion、sourceRef、sourceObservedAt 与 observedAt。observedAt 是作品观察时间；sourceObservedAt 与 referenceObservedAt 分别保留来源观察和相对时间解析依据。未知字段为 null 或合同定义的未知状态。页面优先展示原始发布时间表达及精度，精确时间保留精度，不能用观察时间代替发布时间，也不能把相对表达解析结果伪装为平台提供的绝对时间。

interaction 包含 likes、comments、collects、shares。四项分别保留来源与值，已知零显示 0，未知显示未知。作者、发布时间、观察时间与四项互动集中在同一按需信息入口；原始解析与来源诊断折叠。

## 默认阅读与原件入口

作品文字与研究底稿都按 readingView 显示连续主文和补充材料，默认不加载编辑器或另一份 OCR 全文。重复材料状态、原件、版本与诊断按需展开。查看来源必须展开对应原件、滚动定位并提供键盘焦点；没有可读文字时仍提供直接可达的原件入口。人工保存后刷新同一底稿。

手动云端识别沿现有图组/原件附近的 OCR 入口确认当前作品、选定图片、模型和地址。每张一次，首个失败停止；未知回执沿原 UUID 核对，停止使用既有批次合同。每张回执以 readingRef 标识保存的读本，isCurrent 由真实当前指针及来源匹配计算。只有 isCurrent=true 才能提示已进入当前底稿；成功或百分比不代替此证明。canonical materialRevision 变化（包括人工修正）后只读刷新识别状态，及时反映 isCurrent，不创建新任务。

## 结果清理与验证边界

本次候选保留最新本地 Paddle local-v4、local-vl-v1、当前 DeepSeek-v4-flash 接口与本机现用安装。清理范围覆盖历次机器 OCR 结果、旧机器版本和关联下游副本，包括此前 DeepSeek 试跑；不为这些结果建立备份、归档或兼容读取。平台原文、原图、真正人工内容及共用连接/密钥继续按既有合同保留。历史 migration 源文件保持不可变，清理通过当前迁移办理。

必须验证结构与完整内容、来源位置、CAS、人工作品保护、原请求重试、字节/来源限制、冻结输入和多消费者读取。技术测试、实际供应商识别效果、生产数据清理、合并部署与业务签收分别报告；单元、合成库或 loopback 证明不能当成真实供应商质量或 Gold。新的模型外发另行确认，生产回填、不可逆清理和合并部署按已有授权及最终候选办理。
