# CONTENT-WORKBENCH-001 · 作品与材料使用合同

> 状态: 权威当前
> 最后核对: 2026-10-15
> 适用范围: Issue #425 后端候选，供 WEB-REACT-001 阶段 C 消费
> 事实来源: Mog 实施授权、已认可 V3、main@0a90fd4a React 底座与阶段 C 候选源码
> 冲突时以谁为准: 用户最新确认、真实来源资格；运行事实以实际代码与验证回执为准

## 责任与身份

作品属于本机个人创作空间，actor 为 local_machine；不冒充登录用户，不写运营发布台账，不把导出写成已发布。作品由 domainRef 限定，accountRef 可暂空但导出前须选定真实所属账号。选题方案使用标题、角度、依据、提纲和素材；TopicDefinition 只作引用，不复用为作品版本。

JSON 统一 camelCase、UUID 稳定身份；范围为 Unicode scalar / code-point [start,end)，不归一化空白、换行或 Unicode。正文段落、图页、图页文字块、画布块、使用关系各有稳定 UUID；排序仅改变数组位置，不改身份。

## 文档与修订

`WorkDocument` contract 为 `content-workbench.v1`，含 title/tags/accountRef/topicRef/angle/outline/paragraphs/excerpts/canvas/pages/uses。具体 Rust 类型唯一放 `content_workbench/types.rs`，对外所有写入 deny_unknown_fields 并验证类型、长度、数量、UUID唯一、引用完整及资产所属作品。文档 JSON 是有闭集类型的不可变修订，不是任意 JSON 兼容层。

- paragraphs：id/text；作者分段保留身份，无法可靠还原定位的关联进入 needs_review。
- excerpts：id/citation/selectedText/textHash；source 原文与自己的表达分开。
- canvas：id/excerptId/group/note/expression/expressionRevision；group 限 inbox/opening/argument/example/ending。
- pages：id/kind(cover/opinion/voice/steps)/templateVersion/title/subtitle/blocks{id,text}/assetId/crop/style。宽1080高1440；crop归一化且finite，模板版本 creation-v1 / creation-v1-editorial，palette cream/sage/ink；alignment start/center/end。自有图 PNG/JPEG/WebP 解码验真，禁止任意远程URL和SVG。
- uses：id/canvasId/expressionRevision/excerptId/target/placedTextHash/contentOrigin/state/sourceSpan；target 闭集为标题、角度、提纲项、标签项、画布笔记、画布表达、正文段落、图页标题、副题或文字块；提纲与标签按 index，画布、段落和图页按稳定 UUID 定位。sourceSpan 固定实际使用原文在目标中的 Unicode scalar 范围和 hash；state current/modified/needs_review 由服务端核定。contentOrigin(excerpt/expression) 由服务端核定，原文与自主表达不混同。手工文字不会被后续表达或模型自动覆盖。

服务器保存为权威。React 客户端串行保存、内存中的未确认编辑与中文输入法 composition 保护；含原声的查询或文档不落 localStorage/IndexedDB/sessionStorage。持久恢复以服务器版本与回执为准；未发送编辑恢复策略在阶段 C 明确，不能沿用原生整篇浏览器快照。409 保留双方供选择。expectedRevision 使用 revision UUID；同 requestId+同命令重播返回原回执，异命令409。一次放入正文和内页同事务；重复目标不重复追加。恢复历史追加新修订。保存不触发模型/采集。

## 材料与精确引用

Citation 为 sourceKind(note/comment/ocr/ops_publication)、sourceRef、workRef、field(title/body/ocr/cover_headline/comment_body)、sourceVersion、start/end、textHash、location。OCR location 绑定 derivativeRef/layoutRef/layeringRef/blobSha256/slotKey 与 lineSpans(lineRef/start/end/bbox)，坐标为归一化 [x,y,width,height]，从原所有者 [left,top,right,bottom] 显式转换。

服务端从原读所有者重读并校验 exact range/hash，客户端选文不自证来源。来源模块提供同事务 validate_citation/qualify_citations；保存、放入、AI准备/发出、导出均检查当前来源资格。Readable/Changed/Restricted/Unavailable 不混同，受限原文不在旧修订读取中绕过。来源版本变更标记需核验，仍可读的历史摘录与自主改写保留；受限/不可用原文仅遮断实际引用。保存新自由文本不让引用失效整篇丢失；保留失效引用身份和可解释状态，阻止把受限材料新增使用/外发。

复用 ResearchDossier、完整匿名评论及父语境；缺失/受限父评论显式状态。Ops 自有历史内容沿镜像所有者只读，merged/missing 保持语义。全文查询数量仅代表已取得且符合当前范围的资料，分页显示当前页与更多，不伪造全库语义相似数。纸条、分布索引、书页高亮共用同一 fragment ID 与筛选集合。

目录分页先限定每页最多30个来源对象；totalBasis=source_directory，returnedSourceCount/returnedFragmentCount 区分来源与片段。searchScope 如实标明原生标题正文历史目录、当前匿名评论或我方导出内容范围；不把它称为全库语义检索。

## HTTP 组合

路径统一 `/ops/api/creation`；guard 沿现有 local/same-origin，所有写命令有限大小且无秘密回显。

- GET/POST `/works`；GET `/works/{id}`；POST `/works/{id}/save` `{requestId,expectedRevision,document}`。
- GET `/works/{id}/revisions`；POST `/works/{id}/restore` `{requestId,expectedRevision,restoreRevision}`；POST `/works/{id}/place` `{requestId,expectedRevision,canvasId,targets}`。
- work envelope：workId/domainRef/savedBy/revision{id,number,savedAt,document}/exports；receipt 含 requestId/outcome/replayed/revision。stale/conflict 使用409，无假成功。
- POST `/works/{id}/assets` raw 图片和 request ID；GET 注册资产路径。资产只允许作品owner生成storage key，不接受路径。
- GET `/sources`；GET `/sources/{kind}/{ref}`；POST `/sources/validate`。query/返回 DTO 由sources模块统一，前端不得猜字段。
- AI readiness/preview/explicit request/status/cancel 独立路由；suggestion 只读，不自动采用。采用作为新作品修订，校验底稿、目标和建议版本，并记录 adopted linkage。
- POST `/works/{id}/exports` 冻结expectedRevision；PUT `/exports/{exportId}/pages/{pageId}`上传实际PNG；POST `/exports/{id}/finalize`；GET状态、download及单页。所有资产/导出路径均由服务端固定。

## AI

动作仅 draft/rewrite_selection/page_copy；methodVersion `content-workbench-writing.v1`。只发用户选定材料、底稿与指令，材料按数据编码，不作为系统指令。configuration、transportCallable、writingQualified 分别报告；不得以评论研究资格证明创作。可用既有immutable model config，但资格/建议/请求/采用记录属于创作模块，统一 model_invocation只承担传输与计量。

预览不出站；显式执行须预算与输入确认，单动作一调用、不自动重试收费。cancel与late result不覆写正文。建议按段/整篇采用，底稿已变则标stale并阻止直接覆盖；失败可继续手写。真实模型未调用时A06保持NOT VERIFIED。

## 导出

冻结已保存修订、账号、模板/渲染器/字体配置及资产hash；同一正式 renderer 产生预览和导出。字体/图片就绪后测量排版，溢出给出解决入口并阻止完整导出。正式 React 绘图按需加载仓库内的 NotoSansSC WOFF2，fontVersion 固定文件 SHA256，来源和 OFL 许可见生成物登记。后端冻结此标识，但不把客户端声明当成服务端绘制证明。

每张PNG须完整解码、CRC及1080×1440核对，页ID必须属于冻结清单；成功文件不可覆盖。数量/顺序/尺寸/sha256与冻结manifest一致才产出ZIP。ZIP用维护中的库，默认只包含正文.txt（含作者确认的 tags）、01-封面.png 与有序内页 PNG，不含内部原声/出处/模型数据。内部manifest独立保存。原子写文件后登记数据库，失败不会生成可下载完整包；重复finalize/download返回同一成功成品。后续编辑提示有新版，旧导出不可变。

正式运行资产目录由 LINGGAN_CREATION_ASSET_ROOT 显式指定，launch 脚本的候选默认是 Application Support/Linggan Intelligence/creation；不得回退到 checkout。隔离验证使用随机 /tmp/linggan-content-workbench-assets.* 并精确清理。GET /storage 返回配置就绪状态，未配置时不能上传/导出，但允许创作文本。迁移入口登记0127/0128仅是候选代码，尚未对共享数据库执行。

## 证明

验收仍为专项计划A01–A13，覆盖真实PostgreSQL正负例、来源变化/受限、保存冲突恢复、稳定引用、多目标原子性、模型stale/取消、实际PNG/ZIP。独立候选与共享运行发布分开；隔离合成测试不代替真实选题和Mog成品验收。

2026-10-15，撤下原生页面后执行完整 `scripts/test-content-workbench-postgres.sh`：作品/导出8项、材料来源1项、AI合成适配器1项、HTTP2项全部通过；临时数据库、容器与卷清理通过。日志留在私有 `implementation/postgres-react-handoff-2026-10-15.log`。这些用例未覆盖下列已知缺口，独立审查和真实选题验收仍未完成。

## WEB-REACT-001 阶段 C 候选接入

基线为已合并的 main@0a90fd4a。以下实现位于阶段 C 专属工作树，尚未合并或部署：

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
