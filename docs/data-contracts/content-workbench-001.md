# CONTENT-WORKBENCH-001 · 作品与材料使用合同

> 状态: 权威当前
> 最后核对: 2026-10-28
> 适用范围: Issue #425 后端候选，供 WEB-REACT-001 阶段 C 消费
> 事实来源: Mog 实施授权、已认可 V3、main@0a90fd4a React 底座与阶段 C 候选源码
> 冲突时以谁为准: 用户最新确认、真实来源资格；运行事实以实际代码与验证回执为准

## 责任与身份

作品属于本机个人创作空间，actor 为 local_machine；不冒充登录用户，不写运营发布台账，不把导出写成已发布。作品由 domainRef 限定，accountRef 可暂空但导出前须选定真实所属账号。选题方案使用标题、角度、依据、提纲和素材；TopicDefinition 只作引用，不复用为作品版本。

JSON 统一 camelCase、UUID 稳定身份；范围为 Unicode scalar / code-point [start,end)，不归一化空白、换行或 Unicode。正文段落、图页、图页文字块、画布块、使用关系各有稳定 UUID；排序仅改变数组位置，不改身份。

## 文档与修订

`WorkDocument` contract 为 `content-workbench.v1`，含 title/tags/accountRef/topicRef/angle/outline/paragraphs/excerpts/canvas/pages/uses/materials/materialPreparation，以及两个可选字段 bridge/coverAi（CREATION-ASSETS-001，见下）。具体 Rust 类型唯一放 `content_workbench/types.rs`，对外所有写入 deny_unknown_fields 并验证类型、长度、数量、UUID唯一、引用完整及资产所属作品。文档 JSON 是有闭集类型的不可变修订，不是任意 JSON 兼容层。

- paragraphs：id/text；作者分段保留身份，无法可靠还原定位的关联进入 needs_review。
- bridge（可选，缺省即未决定）：`{skip, sentence?, afterParagraph?}`。`skip` 是“这篇不植入”；`sentence` 是本稿使用的衔接句 `{sentenceCode, sentenceRevision, group, text, edited}`，`text` 是本稿实际所说，改写过则 `edited` 为真、不再跟库同步；`afterParagraph` 指向某个段落 id，缺失或该段已删除时落在默认位置（最后一段之前）。写入时校验编码 `XJ-` 加数字、修订号为正、文本为一行且不超过 500 字。衔接句库、规则与复制格式见 [CREATION-ASSETS-001](creation-assets-001.md)。
- coverAi（可选）：封面是否 AI 作图；缺省表示未记录，不等于“否”。
- excerpts：id/citation/selectedText/textHash；source 原文与自己的表达分开。
- canvas：id/excerptId/group/note/expression/expressionRevision；group 限 inbox/opening/argument/example/ending。
- pages：id/kind(cover/opinion/voice/steps)/templateVersion/title/subtitle/blocks{id,text}/assetId/crop/style。宽1080高1440；crop归一化且finite，V3 模板版本 creation-v3-type（留白大字）/ creation-v3-split（双色分栏）/ creation-v3-list（清单笔记）；creation-v1 / creation-v1-editorial 保留旧版原始绘图含义，palette cream/sage/ink；alignment start/center/end。自有图 PNG/JPEG/WebP 解码验真，禁止任意远程URL和SVG。
- uses：id/canvasId/expressionRevision/excerptId/target/placedTextHash/contentOrigin/state/sourceSpan；target 闭集为标题、角度、提纲项、标签项、画布笔记、画布表达、正文段落、图页标题、副题或文字块；提纲与标签按 index，画布、段落和图页按稳定 UUID 定位。sourceSpan 固定实际使用原文在目标中的 Unicode scalar 范围和 hash；state current/modified/needs_review 由服务端核定。contentOrigin(excerpt/expression) 由服务端核定，原文与自主表达不混同。手工文字不会被后续表达或模型自动覆盖。

服务器保存为权威。React 客户端串行保存、内存中的未确认编辑与中文输入法 composition 保护；含原声的查询或文档不落 localStorage/IndexedDB/sessionStorage。持久恢复以服务器版本与回执为准；未发送编辑恢复策略在阶段 C 明确，不能沿用原生整篇浏览器快照。409 保留双方供选择。expectedRevision 使用 revision UUID；同 requestId+同命令重播返回原回执，异命令409。一次放入正文和内页同事务；重复目标不重复追加。恢复历史追加新修订。保存不触发模型/采集。

## 材料与精确引用

Citation 为 sourceKind(note/comment/ocr/ops_publication)、sourceRef、workRef、field(title/body/ocr/cover_headline/comment_body)、sourceVersion、start/end、textHash、location。OCR location 绑定 derivativeRef/layoutRef/layeringRef/blobSha256/slotKey 与 lineSpans(lineRef/start/end/bbox)，坐标为归一化 [x,y,width,height]，从原所有者 [left,top,right,bottom] 显式转换。

服务端从原读所有者重读并校验 exact range/hash，客户端选文不自证来源。来源模块提供同事务 validate_citation/qualify_citations；保存、放入、AI准备/发出、导出均检查当前来源资格。Readable/Changed/Restricted/Unavailable 不混同，受限原文不在旧修订读取中绕过。来源版本变更标记需核验，仍可读的历史摘录与自主改写保留；受限/不可用原文仅遮断实际引用。保存新自由文本不让引用失效整篇丢失；保留失效引用身份和可解释状态，阻止把受限材料新增使用/外发。

复用 ResearchDossier、完整匿名评论及父语境；缺失/受限父评论显式状态。Ops 自有历史内容沿镜像所有者只读，merged/missing 保持语义。全文查询数量仅代表已取得且符合当前范围的资料，分页显示当前页与更多，不伪造全库语义相似数。纸条、分布索引、书页高亮共用同一 fragment ID 与筛选集合。

目录分页先限定每页最多30个来源对象；totalBasis=source_directory，returnedSourceCount/returnedFragmentCount 区分来源与片段。searchScope 如实标明原生标题正文历史目录、当前匿名评论或我方导出内容范围；不把它称为全库语义检索。

## 本篇阅读材料集合（V3 重新落地候选）

`materials` 与摘录、画布独立：保存 documentId/sourceKind/sourceRef/workRef/sourceVersion/addedBy/matchReasons，原文由现有来源所有者每次重读，不把整篇正文复制进作品 JSON。`materialPreparation` 保存 methodVersion/query/terms/state/gaps。进入研读会准备尚未准备的选题；作者可修改检索词重新准备，异步返回基于最新文档合并，保留手工加入材料。准备不调用模型、不生成摘录，移除阅读材料不删除已建立的引用。

本轮 `workbench.lexical-context.v1` 是可见字词规则检索：对作业启动标题/角度/提纲识别场景与问题词，原笔记需同篇当前原文中“作业”与启动问题在邻近范围共同出现；评论需来自匹配父笔记且自身表达具体相关问题。每个检索词从原生标题/正文目录读取最多 30 个候选，笔记去重最多 120，再批量读取当前完整 dossier 核对；默认最多 12 篇笔记与 12 条原声。数量不足显式返回不足，不能用全域最新内容填满。此规则不等于通用语义检索或每日自动生成十个选题；topicRef 保留关联，不据此声称进行主题语义扩展。OCR 可随整篇读出，但未宣称全库 OCR 检索召回。

准备先按当前标题/场景匹配固定最多 12 篇父笔记，再只从这些父篇读取评论候选；每父篇以轮转顺序共享 120 条有界窗口，最终最多 12 条原声保留父篇轮转顺序。未入选的第 13 篇及其大量评论不能挤占本篇父材料的评论窗口。已选合格笔记与相关原声均存在时，正常候选截断不单独造成 insufficient；实际缺少笔记或原声时仍标不足。该公平窗口仍是库内有界选择，ready 不代表平台总体或全面召回。

`materialPreparation.query/terms/gaps` 及历史 `materials.matchReasons` 同样受已知 Citation 的来源限制约束：完整已知引文从标题/画布移入这些字段时仍保留引用身份；受限后不得新增复制，每次作品读取、历史和请求回执重播均精确遮断引文，并保留正常检索词、说明和周边作者文字。元数据删节使用不增长字段长度的单字省略号；删节后不足两字的单个检索词移出 terms，其余正常词与说明保留，500/40/200 字限制仍按原规则校验，不为客户端占位符提供豁免。元数据不成为正文/图页放置关系；保存的新材料理由由服务端规范化，读取理由只按当前合格原文重新计算。

手工搜索支持 `query/kind/cursor`；kind 在分页前筛选，整篇页长 24。候选范围有界，前端说明搜索范围和不足；不把有界候选数量写成全库总量。选择结果先预览、再加入本篇，之后选片段进入画布。笔记包括完整标题/正文/现有 OCR；评论包括完整原声与其自身父语境，不拼接其他笔记作依据。

完整阅读文档版本与精准引用版本分开。评论整篇版本包含评论版本、父笔记 revision、父评论状态及正文 hash；父语境变化使阅读材料标为 changed，需要核对后采用当前版本。精确 Citation 仍由实际引用片段的原版本和范围确定。受限/失效材料遮断原文，保留身份与解释；重新采用须经相同来源资格检查。

## HTTP 组合

路径统一 `/ops/api/creation`；guard 沿现有 local/same-origin，所有写命令有限大小且无秘密回显。

- GET/POST `/works`；GET `/works/{id}`；POST `/works/{id}/save` `{requestId,expectedRevision,document}`。
- GET `/works/{id}/revisions`；POST `/works/{id}/restore` `{requestId,expectedRevision,restoreRevision}`；POST `/works/{id}/place` `{requestId,expectedRevision,canvasId,targets}`。
- work envelope：workId/domainRef/savedBy/revision{id,number,savedAt,document}/exports；receipt 含 requestId/outcome/replayed/revision。stale/conflict 使用409，无假成功。
- POST `/works/{id}/assets` raw 图片和 request ID；GET 注册资产路径。资产只允许作品owner生成storage key，不接受路径。
- GET `/sources`；GET `/sources/{kind}/{ref}`；POST `/sources/validate`。GET `/sources/documents`、GET `/sources/document`、POST `/sources/prepare` 和 POST `/sources/resolve` 为只读材料集合接口；query/返回 DTO 由 sources 模块统一，前端不得猜字段。
- AI readiness/preview/explicit request/status/cancel 独立路由；suggestion 只读，不自动采用。采用作为新作品修订，校验底稿、目标和建议版本，并记录 adopted linkage。
- POST `/works/{id}/exports` 冻结expectedRevision；PUT `/exports/{exportId}/pages/{pageId}`上传实际PNG；POST `/exports/{id}/finalize`；GET状态、download及单页。所有资产/导出路径均由服务端固定。

## AI

2026-10-20 / #454 PR-1B候选补充：writing_legacy使用[统一AI底座](ai-foundation.md)配置与队列，预览固定task_config_ref，确认返回queued且不内联派发。预留按冻结输入/输出额度，estimatedInputTokens是估算；无调用账时invocationRef/用量为NULL且usage.state=not_reserved，和已建账的未知/实测分开。实际调用、在途取消、晚到用量由统一执行器结算；正式建议与采用仍属于本合同。0140拒绝未核实的旧在途；历史已完成结果可读，不变更旧回执。该行为在独立候选/一次性PG证明，尚未合并部署或取得真实供应商/A06验收。

动作仅 draft/rewrite_selection/page_copy；methodVersion `content-workbench-writing.v1`。只发用户选定材料、底稿与指令，材料按数据编码，不作为系统指令。configuration、transportCallable、writingQualified 分别报告；不得以评论研究资格证明创作。可用既有immutable model config，但资格/建议/请求/采用记录属于创作模块，统一 model_invocation只承担传输与计量。

预览不出站；显式执行须预算与输入确认，单动作一调用、不自动重试收费。cancel与late result不覆写正文。建议按段/整篇采用，底稿已变则标stale并阻止直接覆盖；失败可继续手写。真实模型未调用时A06保持NOT VERIFIED。

## 导出

冻结已保存修订、账号、模板/渲染器/字体配置及资产hash；同一正式 renderer 产生预览和导出。当前绘图版本为 creation-renderer.v3，按模板版本分派；旧模板仍按其原始算法绘制。已完成旧导出始终下载原文件，未完成导出若 renderer/font 与当前实现不一致，在任何续绘和重试变更前要求生成新版，不以 V3 重绘冒充旧版本。字体/图片就绪后测量排版，溢出给出解决入口并阻止完整导出。正式 React 绘图按需加载仓库内的 NotoSansSC WOFF2，fontVersion 固定文件 SHA256，来源和 OFL 许可见生成物登记。后端冻结此标识，但不把客户端声明当成服务端绘制证明。

每张PNG须完整解码、CRC及1080×1440核对，页ID必须属于冻结清单；成功文件不可覆盖。数量/顺序/尺寸/sha256与冻结manifest一致才产出ZIP。ZIP用维护中的库，默认只包含正文.txt（含作者确认的 tags）、01-封面.png 与有序内页 PNG，不含内部原声/出处/模型数据。内部manifest独立保存。原子写文件后登记数据库，失败不会生成可下载完整包；重复finalize/download返回同一成功成品。后续编辑提示有新版，旧导出不可变。

正式运行资产目录由 LINGGAN_CREATION_ASSET_ROOT 显式指定，launch 脚本的候选默认是 Application Support/Linggan Intelligence/creation；不得回退到 checkout。隔离验证使用随机 /tmp/linggan-content-workbench-assets.* 并精确清理。GET /storage 返回配置就绪状态，未配置时不能上传/导出，但允许创作文本。迁移入口登记0127/0128仅是候选代码，尚未对共享数据库执行。

## 证明

验收仍为专项计划A01–A13，覆盖真实PostgreSQL正负例、来源变化/受限、保存冲突恢复、稳定引用、多目标原子性、模型stale/取消、实际PNG/ZIP。独立候选与共享运行发布分开；隔离合成测试不代替真实选题和Mog成品验收。

2026-10-15，撤下原生页面后执行完整 `scripts/test-content-workbench-postgres.sh`：作品/导出8项、材料来源1项、AI合成适配器1项、HTTP2项全部通过；临时数据库、容器与卷清理通过。日志留在私有 `implementation/postgres-react-handoff-2026-10-15.log`。这些用例未覆盖下列已知缺口，独立审查和真实选题验收仍未完成。

## WEB-REACT-001 阶段 C 候选接入

基线为已合并的 main@0a90fd4a。以下六项阶段 C 基础能力已随 PR #432 合并；本轮 V3 重新落地以 main@36f3fdf9 为基线。新增 V3 界面与阅读集合仍在独立候选中，等待 Mog 验收后再合并部署：

1. 唯一 `shared/api.ts` 支持同源 JSON 与 raw POST/PUT 图片、JSON 内容类型、闭集 DTO、409 保存回执；不自动重试。
2. 前端 pages.ts / Router 与 Axum REACT_PAGES 同时登记 `/ops/creation`；React.lazy 隔离工作台包，旧运营页继续按原路由提供。没有原生工作台或新旧切换开关。
3. `GET /ops/api/creation/requests/{requestId}` 返回 `{requestId,kind,receipt}`，kind 为 work/asset/export。export 的查询回执统一 `{export,revision}`。跨写入所有者共用请求 UUID 锁，同一 UUID 不得改命令；未确认写入由作者显式核实原请求，再决定重发。
4. 正式前端仅在内存保留未确认编辑及原请求；跨站内作品切换保留原编辑器，原声不写浏览器持久存储。关闭或刷新前提示未保存内容；服务器修订支持持久恢复。
5. 图页预览、单页 PNG 和整包使用同一 TypeScript 绘图模块。图页顺序依稳定 UUID；正文变化经已持久化的使用关系提示关联图页更新，手动改过图页不得自动覆盖。新增 npm 依赖为零。
6. AI 选文改写包含 paragraphId/start/end/textHash，按精确 Unicode scalar 范围替换。预览不出站；执行只能来自确认输入与额度后的点击。请求冻结完整命令 hash，StrictMode 不触发模型调用。

## 来源传播边界

AI 采用从冻结 Citation 建立摘录、画布和实际目标的使用关系，包括只在本次预览选择、此前未加入作品的原声。保存重新计算精确来源范围；已知完整原文移动到新的段落、角度、提纲、标签或图页，或客户端删掉引用关系，不能解除原有来源身份。来源受限后所有作者可编辑字段、历史回执、列表和导出均重新核验，并保留周围的作者自主表达；AI 输入也核验实际将发出的文字。

该传播匹配的是**完整选定 Citation 原文**的精确出现，包含重复及重叠范围。若模型仅复制一段较长 Citation 中未独立声明的更小子片段，当前没有结构化子引用传播证明，不能宣称覆盖任意局部引用；不以模糊或通用词匹配替代来源身份。此限制须保留在真实 AI 验收记录中。

## 当前验收范围

一键入口为 `scripts/test-content-workbench-postgres.sh`（隔离 PostgreSQL 合同）和同脚本 `--browser`（真实 Axum + 正式 React + 合成资料 + 实际成品）。两者接入 Web CI；浏览器 Rust 夹具使用 multi_thread，禁止真实凭据和真实模型出站。真实选题、模型质量与 Mog 审美验收仍需单独记录，测试通过不等于上线完成。

最新通过数量与证据以阶段 C 验收记录为准；上文 2026-10-15 首轮历史结果不覆盖后续修复。候选仍须独立审核及最终主线迁移编号核对。

## WEB-REACT-001 收尾：导出可读元数据

导出记录新增可选createdAt，来自已有export.created_at实际时间，旧不可变回执缺失时保持未记录，不重写历史包/回执或生成新时间。UI按生成时间、页数和当前已保存版本/历史冻结版本区分，完整技术版本号放详情；作品修改后顶部明确当前版本尚未导出。资产、冻结快照、PNG/ZIP及来源资格合同不变，无schema迁移。

### 2026-10-19 材料读批量实现（口径不变）

匿名原声批次经`crates/evidence/src/material_creation_comments.rs`在同一个冻结只读事务内取得评论/父评论，并一次读取全部父笔记底稿。完整字段、parentWork/parentComment、资格、版本和匿名身份合同与逐条读一致；缺失不回填旧版本、受限不回原文、不跨请求缓存。搜索/准备与已选材料resolve复用本事务批量结果；默认READ COMMITTED的保存事务仍逐条即时核验新增材料资格和版本，不复用先前批量上下文；异常来源沿原逐条错误优先级处理。未更改候选窗口、匹配词规则、排序、数量、引用码点或来源哈希。用同事务逐条资源接口与批次完整Value对照，覆盖重复/不存在身份、根评论、回复、缺父语境、父评论受限及自身受限；跨新事务确认限制即时生效。


创作资格提示只说明writing_output_contract_only输出格式是否曾通过当前方法/模型/连接版本的严格验证；不是质量、事实或审美签收。UI分别使用“创作输出格式已验证/未通过验证/尚未验证”；连接true、false、unknown分别显示“模型可连接/模型连接测试尚未通过/连接情况尚未确认”。API和账本保留原始资格状态，不修改账本；重新打开弹窗先读取当前配置，旧读取结果不覆盖较新的配置，不自动准备输入或调用模型。

连接false包含最新探测进行中及未成功的情况，不据此宣称探测已经失败；配置读取中/失败时不显示格式资格结论。
