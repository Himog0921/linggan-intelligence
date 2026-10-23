# 研究底稿 v2：完整材料与唯一读取合同

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: RESEARCH-DOSSIER-V2；实现候选，不代表已合并或部署
> 事实来源: Mog 本轮确认的研究底稿标准、开发期直接替换要求与本合同验收矩阵
> 冲突时以谁为准: 用户最新确认、真实来源与验证；本稿替代 v1 及 note-material-input 的并行对外合同

## 用户结果与范围

人打开一篇笔记，应能连续读到已取得的所有图片文字，保留数字、单位、否定、清单、表格、对话、引述及不确定处，并能回到原图。Agent 读取相同材料，按明确任务选择调用；主题图谱、评论研究、作者研究不各自重建另一份笔记事实。

默认读取整篇。没有字符、页数、词元预算截断、自动摘要或自动删除重复出现的原文。去冗余指去除运输层的同内容多份表示；原文在两页实际重复，仍是两个来源位置。模型容量不足时返回明确的容量错误，不静默改写或缩短输入。任务明确选择若干篇、评论、图片或区块不属于截断。

开发期直接切换到唯一 `research-dossier.v2`。删除旧运行接口、旧消费者、兼容解码和字段级兜底；保留历史迁移与原始来源证据的审计价值。旧派生数据无需保活，重新生成必须是显式操作。现有领域权限、来源限制、模型出站授权和调度开关继续生效。

## 一份内容，按需表示

默认 JSON 只有作品身份 `note`、按阅读顺序排列的 `materials[].blocks`、覆盖说明 `coverage`、按需读取入口 `attachments` 和明确选择 `selection`。每份材料正文只出现一次，不同时序列化 fragments、raw OCR、media 文本、Markdown 与多个任务预览。

Markdown 通过 `format=markdown` 单独生成，与同一次读取的区块顺序和内容一致。评论、作者与互动元数据分别按需读取；评论页数不冒充平台评论总量。原图和识别诊断通过来源引用读取，不把原始布局/任务账本塞入每次 Agent 输入。

Markdown 顶层标题只使用原生平台标题，缺失时使用“研究底稿”。图片派生展示标题仍在其真实图片 heading 区块中输出一次，角色、来源时间和局部问题随块保留；不同来源出现同文标题仍各自保留。显式选择未包含的图片块不会仅因它曾作为展示标题而被带入导出。

顶层字段：`contract`、`workRef`、`revision`、`materialRevision`、`asOf`、`note`、`materials`、`coverage`、`attachments`、`selection`。`materialRevision` 绑定材料、来源和结构；`revision` 还绑定覆盖、辅助元数据与同截点可审候选的身份；候选全文不进入默认读取。时间戳本身不令未变材料失效。

`note` 保留平台、作品身份、平台标题及来源、展示标题及采用依据。平台标题与图片标题是不同事实。仅显式封面或已证明的第一页标题可补展示标题，不能伪造平台标题。图片派生展示标题不另复制到 Markdown H1；不同来源的同文区块仍各自保留。

## 材料与结构化区块

材料字段：`ref`、`kind`、`slotKey`、`page`、`sourceState`、`sourceRef`、`sourceVersion`、`currentRevisionRef`、`originalUrl`、`reading`、`blocks`、`issues`；仅在字节证明建立页序关系时增加 `pageSource`。材料种类为 body、tags、image、audio、video_frame。未知页码为 null；不按数据库遍历序号猜平台页码。

`reading` 分开表达生成者 `generatedBy`、采用状态 `adoption`、核对状态 `verification`、规范化规则 `ruleVersion`，以及可空的 `review:{commandRef,actor,reason,action,reviewedAt}`。review 只来自该次 selection 的命令，后来的暂缓/拒绝不能改写已采用版本的审核人和依据；自动读本及原生标签为 null。机器生成、被采用、人工核对是三件事。生成者取 platform、ordinary_ocr、local_vision、cloud_vision、asr、frame_ocr、human_edit。

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

OCR 低置信度局部成为 gap；可信的周围文字保留。原始识别行留作诊断，不通过白名单、UI 词表或“作者正文资格”把整张截图的可读内容过滤掉。无法可靠恢复列顺序、表格或角色时，明确记录结构问题；不能把估计说成原图事实。

## 唯一当前版本与不可变历史

数据库分责：

1. `linggan_material_unit` 是作品内材料位置，持有 sourceVersion、显式 currentReadingRef、单调 revision、采用与核对状态。
2. `linggan_material_reading` 保存不可变的完整区块、局部问题、生成者、规则、来源版本、依赖版本及 publication 类型。
3. `linggan_material_reading_selection` 追加每一次当前选择、来源变化与核对决定；历史读取按事件截点恢复，不读取今天的可变指针冒充昨天。
4. `linggan_material_reading_command` 以 commandRef 保存审核人、依据、决定、请求指纹和回执；command_sequence 给同候选意见建立确定顺序。相同 ID、相同请求重试返回原回执；相同 ID 不同请求冲突。

媒体 sourceVersion 指向精确 materialization；正文 sourceVersion 指向不可变原生 material。媒体必须对应该槽位当时的最新来源 generation，不能从较旧 generation 借字节填缺。所有当前与历史读取仍检查当前受限、撤回与退役资格。

Live Photo 的静态图片与动态视频按实际已验证媒体类型分别选择来源；图像 OCR、音频与帧不能因另一组件后完成而失效或串用来源。已清理字节但未撤回文字的来源保留不可变文字，原图入口明确不可用，并拒绝新的人工决定；已完成命令可返回原回执，但仍检查当前来源与撤回资格。

平台标签沿用不可变 `content-hashtags.v1` 来源投影，保留原序与重复条目，不从正文猜标签。其 `currentRevisionRef` 为 null，sourceVersion 绑定 material 与投影规则，区块引用绑定 material；这属于同一底稿的原生材料，不另建可编辑标签读本或另一份正文。

普通 OCR、平台正文、ASR、帧 OCR 可自动发布，但须以捕获的 expected current 作 CAS，并保护同来源的人工作品。显式重新识别只形成 candidate，不按模型名、图片尺寸、文字长度或完成时间自动更换当前。

正文在来源接纳时规范化并持久化。GET 不生成、不升级、不重新解释正文。旧开发数据缺少 canonical reading 时显示 not_processed，并可由显式命令重建；读取不能退回原生 bodyText 来假装已处理。

来源改变后旧 reading 不再冒充当前；新 reading 不继承旧来源的人工核对。迟到的旧源任务可留下生成记录，但不能抢占新源当前指针。来源限制在任务发送和精确引用回查时再次验证。

## 独立校正、采用与核对

每页可查看候选、采用候选、结构化编辑后采用、保留当前，或记录暂缓与拒绝意见。已保存 OCR 的每张图片直接打开同一个底稿审阅面板；只含标题或空区块的候选也有入口。若封面/第一页去重隐藏了原 unit，按精确 reading 引用读取原 unit 与真实 current，不能把候选内容伪装成已采用内容。动作不依赖先加入抽检样本。

唯一请求包含 commandRef、expectedCurrentRevisionRef、candidateRevisionRef、action、actor、reason、可选 editedBlocks、verified、resolvedIssueKeys。actor 与 reason 为必填非空审核元数据，分别上限 120、300 个字符；这些限制不适用于正文或区块内容。请求指纹与未知回执重试冻结全部字段。

| action | 当前选择与核验状态 | 候选列表 |
|---|---|---|
| adopt | 选择完整候选，记录人工采用与显式 verification | 已采用读本退出 |
| edit | 新增不可变人工读本，绑定原 candidate，选择新读本 | 原候选退出 |
| keep | 保留确实存在且可读的 current，记录人工选择与显式 verification | 被保留决定处理的候选退出 |
| pending | 只追加审阅意见，不写 selection、不改 adoption/verification | 保持可审 |
| reject | 只追加审阅意见，不写 selection、不改 adoption/verification | 退出待审 |

pending/reject 允许 current=null，此时回执的 adoption 与 verification 也为 null；这两种意见不得携带 editedBlocks、已解决问题或 verified=true。未被采用过的候选按截至读取时间的最新命令判断：无意见或 pending 可审，reject/keep/adopt/edit 不可审。reject 后显式 pending 可以重新待审；已进入过 selection 的读本不会重新成为新候选。

服务器核对作品归属、候选来源、当前版本与请求幂等；冲突返回 409，不能覆盖他人的新决定。请求成功但回执丢失时，界面用原 commandRef 和原请求重试；未知状态不显示成功、不自动生成新命令。页面切换或候选变化后的迟到响应不得改写新页面。

对外 current 指针与锁内 CAS 共用来源资格：如果只撤回旧读本的衍生物，而同一原图的另一候选仍有效，current 为 null，新候选可以在 expected=null 下审阅。pending/reject 的回执与命令 resulting 同样为 null，既有 unit 指针和选择历史不因这类意见被清空；采用或编辑成功才追加新的选择。候选列表与精确版本入口遵循这一口径，历史计划继续按原 asOf 取选择和意见。

网络断开、5xx 或无法解码的成功回执都属于结果未知，必须保留原命令。只有服务器明确拒绝且确认未写入的错误才释放该命令身份。

结构化编辑保留区块种类、表格单元格与对话角色；来源位置仅在对应内容仍一致时保留。仅沿用 blockRef 不足以证明一段新转写位于旧 bbox/time；文字或结构改变且无法准确对应时移除位置，原读本与原始观察不变。问题带稳定 key 和解决责任：`reading_edit` 可通过编辑并明确勾选解决；`source_refresh`（缺页、来源变化、时间未知等）必须补来源，文字编辑或“已核对”不能消除。未明确解决的问题继续存在。

OCR 抽检仅记录质量判断 confirmed/dismissed/pending，可关联材料命令；它不再拥有第二套文字采用表、当前选择或修订接口。

## 图片、视频与覆盖

完整图片序列只从平台显式 imageList 获得。保留原始位置、缺失位置、相同 URL/字节在后页的真实重复；DOM 扫描只能给 observed_set，不声明平台图片总数。

封面与第一页只有在字节一致、位置关系明确时才合并为同一内容来源；第 3 页再次使用相同图仍保留。没有证明的“封面就是第 1 页”不可用于补页码。

合并优先保留实际可用读本与明确人工决定。若保留的读本来自封面，保留其真实 unit、reading、source 与区块引用，并以 `pageSource:{basis:"same_verified_bytes",sourceRef,sourceVersion,slotKey}` 标明页序证据；不能把文字改挂到另一条来源。不同已采用版本无法等价时同时保留并标明 `duplicate_image_readings`，直到来源核对完成。

最新完整平台图片清单使已移除的旧槽位失去当前资格；后续局部观察只增加自身取得的槽位，不能复活被移除的旧页。读取、候选、任务领取、采用、重建与检索共享同一成员判定。当前观察未提供完整清单时，平台总数继续标为未知。

coverage.imageInventory 分别给出 basis、expectedPages（未知为 null）、acquiredPages、processedPages、missingPages、unknownOrder。已知总页数时统计只覆盖对应真实页，不能把额外封面算成第 N+1 页；处理数 ≤ 取得数 ≤ 已知总数。

音频采用实际转写 segment 的时间。帧 OCR 保留所有实际采样帧及真实 PTS，不设固定 12 帧读入上限、不跨时间全局去重。采样视频明确标 incomplete；未采样位置未知。只有已证明无音轨才是 not_applicable；识别器无输出不证明源内容为空。

重建计划只枚举同源、同截点的普通本地 OCR layout。`pendingReviewCount` 统计有可审候选的图片槽位，仅云端候选存在时 `layoutCount=0`、`pendingLayouts=0`、`needsOcr=0` 并显示待核对；本地与云端并存时仍处理本地 target，保护人工 current。

封面与显式第一页经相同已验证字节证明去重时，计划保留内部来源别名，仅用于正确统计候选和已处理状态；候选仍绑定其原 unit、slot 和 sourceVersion，不搬到另一页，也不把后续相同图片的真实页面一并合并。

来源状态：present、known_empty、not_acquired、not_applicable、restricted。取得来源但尚无 canonical reading 为 not_processed。部分内容可用仍可阅读，不用一个 completed 压平缺页、失败、待核对与处理进度。

## 读取、选择、引用与任务接入

现有 `/api/local/work-resources/{workRef}/research-dossier` 成为唯一笔记读取入口。支持 asOf 与 format。默认 full；kinds、unitRefs、blockRefs、imagePages 是显式选择，采用并集；空值、未知 ID 或非法枚举返回错误。selection 报实际选择、省略材料数与完整材料 revision，禁止把局部表示标成完整。

readings/{readingRef} 在同一只读快照返回 workRef、所请求版本 material，以及顶层 currentRevisionRef（当前仍有来源资格的 unit 指针，可为 null）。material.currentRevisionRef 保持请求的版本身份；旧成功回执不能替代最新 current，去重隐藏的 unit 也按该指针精确读取，不重复运输当前全文。

独立入口包括 candidates、readings/{readingRef}、materials/{unitRef}/review、metadata；评论沿用授权分页。批量读取固定一个共同 asOf，为每篇返回独立结果，某篇缺失不伪装整批成功或抹去其他有效材料。

识读引用必须绑定作品、unit、reading、blockRef、sourceRef、sourceVersion 与 asOf；blockRef 采用 `workUUID.readingUUID.ordinal`。不可变原生标签使用 `workUUID.materialUUID.ordinal` 并绑定投影规则。相同文本不是相同来源。冻结引用精确匹配，不按字段名或相似文本找新内容顶替；旧引用缺少必要字段则明确不可解析。

内部任务 adapter 可以生成短引用编号和来源映射，但正文只发送一次，结构、角色、问题与真实位置必须保留。评论研究携带选定评论、必要父回复和完整作品语境；作者研究携带所选作品及独立作者材料；主题研究窗口不得截取一篇的前 N 字冒充整篇。

主题已保存来源按请求的统一 asOf 恢复正文、读本、当时的作者标记及已完成评论研究血缘；新增读本、Signal 或人工标记不得改写旧请求的来源身份。当前来源限制、研究域与定义资格仍单独核验。历史结果的可读性与当前材料覆盖率分责：只有仍匹配当前精确来源的分析区块计入当前覆盖，新增或更新材料不能因旧结果存在而被报告为已经完成分析。

任务快照只保存版本引用、选择与指纹，不建立第二份长期原文库。`dossier_input` 仅是内部准备/重建工具，不提供第二个公开 note-material-input 合同。默认读取不触发 OCR、模型、研究任务、补采或数据库写入。

## 验证与切换

实际验证与交付状态统一记录于 [RESEARCH-DOSSIER-V2](../plans/active/research-dossier-v2.md)。必须验证完整内容、结构、来源位置、并发 CAS、重试幂等、历史与当前限制、同源页面/API、副作用和多消费者输入。单元测试与合成数据库不替代真实识别质量；后者没有调用证据就不能声明通过。

迁移直接移除旧派生采用/任务输入表和运行分支，保留原始证据。历史数据库 migration 不回写。代码合并、共享库迁移、服务部署及业务签收分别报告，本轮实现授权不自动触发这些后续动作。
