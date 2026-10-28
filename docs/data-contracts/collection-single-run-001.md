# COLLECTION-SINGLE-RUN-001 · 一次采集的持久范围与读取合同

> 状态: 代码事实优先
> 最后核对: 2026-10-28（0180 增量：主页浅看、精选研究、none 评论模式；此前 2026-10-25 up_to 增量、控制探针锁争用修复候选）
> 适用范围: Issue #398 / Draft PR #406 的单次采集候选；0125 migration、Rust、正式插件 0.8.61
> 事实来源: Mog 完整交接授权；本分支 collection_run、原 Request/Admission/WorkOrder/Lease/Package/Receipt 与共享材料代码
> 冲突时以谁为准: 用户最新授权、可执行合同和真实运行结果；本合同不代表合并、业务库迁移或发布

## 写入和所有权

`POST /api/collection/runs` 接受 camelCase JSON：`idempotencyKey`、`domainRef`、`source`、`ranking`、`workLimit`、`commentMode`，以及仅 `up_to` 必带的 `commentTarget`。source 为 temporary_keyword/query、observation_target/targetRef、selected_works/workRefs。只接受本次明确选择的 active 领域、XHS 的有效长期目标或该领域的精确材料 ID；作品上限 1–200，临时词 1–80 字符；创作者与精确作品无搜索排名，关键词排名使用现有五种枚举。

相同幂等键和同一规范请求返回原 run；相同键不同意图返回 409。客户端未知写结果先读 `/api/collection/runs/requests/{idempotencyKey}`，再使用原身份重试，不自动创建第二次采集。当前 API 属于现有本地工作台的访问边界，未新增外部超级用户接口。

`collection_run` 冻结用户意图、领域、参数、授权及控制版本；`collection_run_work` 保存精确 Content ID、1-based 顺位、发现包、明确复用详情包、检查点和缺口；`collection_run_order` 关联现有 Request/WorkOrder、作品、阶段及段号；`collection_run_command` 保留不可变命令回执。它们不保存第二份作品、作者或评论原文，不建立另一执行引擎。

每次需求使用 `purpose_kind=single_run` 的执行目标和独立身份 `single-run:<runRef>`，真正查询词独立传到 TaskSpec。长期目标的 identity、领域关系、monitor rule/revision、周期均不修改；名单、计数、选择器、领域目标统计和自动巡查排除 single_run。数据库禁止临时目标启动长期监控、关联监控规则或改变用途身份。原长期目标被删除时停止关联 run，保留历史 source UUID；与推进竞争时删除事务可重试，不逆向等待锁。

## 固定集合、阶段与媒体

首次合格发现按接纳顺序及平台 resultPosition/recordOrdinal 取前 N 个唯一 Content。已在库对象仍占位置，入选后不补第 N+1 篇；37/50 保存 37 和真实停止原因。精确勾选直接冻结 ID 顺序，不再搜索。已冻结成员的身份、顺位、发现血缘不可修改或删除，数据库拒绝追加和越界。

普通详情与作者基础身份先按原材料投影交付。all_public 的未取详情作品先发 detail 阶段；已有合格详情保存复用包依据。标准采样与基础详情复用同一页面链。媒体槽位、下载权限、OCR/ASR 沿用原材料工单与统一媒体层，基础阶段保留既有授权，后续评论段不重复扩大媒体范围；文字成果不等待媒体处理。未知作者简介/粉丝数不变成 0，不隐含作者深建档或新研究模型请求。

每次 worker 推进只处理有界数量的 run/作品；原队列、工位并发、账号限制、授权、节奏和资源准入继续有效。暂未准入保留真实请求/决定，最多每分钟重试缺少工单的相同阶段；已准入阶段不重复创建。

## 评论模式

| 模式 | 用户范围 | 分段/恢复 |
| --- | --- | --- |
| sample_30 | 每篇最多 30 条唯一总记录，含回复；每主评论最多 2 回复 | 首段为原标准窗口；部分交付后扣除本轮原始稳定 ID，续段只派剩余额度，并带 knownReplyCounts 保留跨段每主评论回复额度 |
| all_public | 尽量遍历本轮可访问主评论及回复，保留开始/结束与缺口 | newUniqueLimit=30 为一次执行的有界新增额度，maxScrollRounds/maxDurationSeconds 是执行预算；不是采全目标上限，没有每主评论 2 回复或总 30 上限 |
| up_to（2026-10-23 新增，`commentTarget` 1–1000） | 每篇采到 `commentTarget` 条不重复记录（含回复）就停；评论区先走到尽头也结束 | 与 all_public 同一遍历（TaskSpec 对插件仍写 `all_public`，原意图另记 `commentTarget`），每段 `newUniqueLimit=min(30, 目标−本轮已接纳稳定 ID)`；本轮已接纳稳定 ID 达到目标即完成，否则按 all_public 的遍历判据收尾。只有它带 `commentTarget`，其余范围带了即 422；请求未带该字段时指纹与旧请求逐字节一致 |
| none（0180 新增；页面只在主页浅看与文字版精选里使用） | 不采评论、不取媒体：浅看只做作者主页发现；文字版精选只取笔记详情正文 | TaskSpec 对插件仍写 `sample_30`，但详情阶段 commentLimit 与 replyExpandLimit 为 0、不请求媒体与识别；已有合格详情的作品直接复用、不新建工单，其余取到详情即以 `checkpoint.detailComplete` 收尾；run 在全部作品 detailComplete 后结束。服务端校验不按来源限制该取值，其他来源搭配 `none` 未经验证，不应使用 |

`collectionRunRef`、`commentMode` 和可选 `incrementalCommentBudget` 贯穿工单、TaskSpec、页面计划、采集器和回执。detail-only 0 表示不要求评论；all_public 由明确模式表示，绝不用 0 或超大整数替代目标。knownCommentIds 来自本轮原始接纳包，包含清洗掉的记录，避免为凑 30 条有效文本另采。knownReplyCounts 只允许单次标准模式，根 ID 有界、数值 0–2。

插件宣告 `collection.single_run.v1`；深采（all_public 与 up_to）另需 `collection.all_public.v1`。未支持的旧插件保持等待，不认领新合同、不悄悄降为采样。旧任务不带新字段时继续原 v1/v2 校验、outbox 和回执路径。

新的 all_public 完成回执必须具有 mainExhausted、repliesExhausted、identityReliable 的真实遍历依据。显示计数达到、no_progress 或主评论到底不能单独证明整篇采全。回复关系/稳定身份不足保留未知或部分；连续三段无新稳定 ID 后保留可说明的缺口，不无限自动重试。风控/访问受限停止受影响执行，不切换账号规避；一篇失败不连坐其他合格材料。

## 主页浅看与精选研究（0180）

- 来源 `creator_preview{authorExternalId}`：一次性读取某作者在小红书的主页资料与最多 30 张发现卡片。`workLimit` 固定 30、`commentMode` 固定 `none`，其余取值被拒绝（`creator_preview_scope_invalid`）；作者须已在该领域被发现过（`creator_not_discovered_in_domain`）。发现完成后 run 直接 `ended`：不排详情、评论或媒体阶段，不登记长期观察目标（提交作者资料包时，目标同步对浅看任务返回 `notApplicable`；判断「是否浅看任务」的查询失败时返回 503 `author_target_sync_pending` 让插件重试，不当作「不是浅看」）。
- 页面入口 `GET/POST /api/local/creators/{key}/preview`：POST 只接受 `domain` 与 `commandRef`（多余字段整条拒绝），同一 `commandRef` 重放同一 run、换内容返回 409；GET 读该领域＋作者最近一次浅看的冻结样本，逐张卡片带 `inDomain`（该作品在本领域是否已是正式研究材料，由服务端给出）与 `frozenCount`（冻结篇数，读不出的卡片只计入这里）；没有浅看过读到 `not_requested`。
- 来源 `creator_selected_works{previewRef, workRefs}`：只接受同一领域、已冻结（`scope_frozen_at` 已写）的浅看里的作品，1–30 篇、不重复、篇数等于 `workLimit`；页面发的 `commentMode` 是 `none`（文字版）或 `sample_30`（标准版，同普通精确勾选）；服务端对该来源不限制评论范围，`all_public`、`up_to` 搭配精选研究未验证。范围在建立时冻结，不会扩大。浅看、精选运行都可用 `POST /api/collection/runs/{runRef}/commands` 停止。
- 材料用途 `linggan_material_domain_usage.use_kind`：只有「已接纳的发现记录」会写用量行（`material_admission`）；属于这两类 run 的工单所写的行由触发器标为 `creator_context`，其余一律 `domain_research`（含存量）。精选研究只给浅看发现的作品补详情，不写用量行，所以不会把它们转为领域研究材料——迁移注释写明「转为正式成员」应是单独的显式动作，目前没有这个动作；普通搜索再次发现同一作品时，会另写一行 `domain_research`。库里已有的作品保留原来的 `domain_research` 行。
  下面是 2026-10-29 对该表全部读取点（含 `.sql` 文件）的**初判**分类。它是补过滤时的起点，不是可整体套用的清单：补过滤的 PR 要逐处复核，并为每处写测试。
  - **按标记过滤的只有**创作者发现、账号研究的样本选取，以及浅看卡片的 `inDomain` 标注。
  - **列表与统计类，目前不过滤，浅看发现的新作品会计入**：领域样本数（`observation_domain`）、语料作品列表与筛选（`material_query_sql`）、目标目录与计数（`target_catalog`、`collection_target` 的删除预览）、采集管理统计（`collection_management_api`）、评论研究的作品目录、词云、事实与候选（`comment_study_catalog` 的 `works`、`word_cloud` 与 `comments`/`facts`/`refresh_candidates`/`versions`/`insert_clean_cache` 各 `.sql`，以及 `comment_study_source/candidates.sql`）、评论总览的观察序列（`comment_research_overview/observation_series.sql`）、主题图谱读取（`topic_map::read`）、创作作品目录与评论目录（`research_dossier::creation_work_directory_in`、`material_social_read::creation_comment_directory_in`）、作者目标巡检（`target_inspection` 的 `materials`、`gaps`、`round_works.sql`）、关键词档案补详情（`keyword_archive_detail`；复核确认它只看长期目标自己的发现包，不会把浅看卡片拉去深采）。
  - **按作品引用判断「是否在本领域」的读取点，应保持不过滤**，否则浅看卡片在领域内打不开、不能精选或研究：作品打开与资源批读（`material_projection`，api 与 evidence 各一处）、研究底稿范围与创作作品校验（`research_dossier::creation_work_in_domain`、api 的 `research_dossier::ensure_scope`、`dossier_input::read`）、主题图谱的成员校验（`topic_map::store`、`topic_map::saved`、`topic_map::saved::sources`、`topic_map_structure`）、评论研究的成员校验（`comment_study_catalog::post_usage`、`comment_study_source::selected_source_in`、`comment_study_source::context::read_work_contexts`）、目标巡检的作品校验（`target_inspection::scope`）、精确勾选（`selected_works`）的准入（`collection_run::create_run`，所选作品都须在本领域）。
  - 准入里另有两处不属于上面任何一类：浅看准入按**作者**判断本领域是否已发现过该作者（`creator_not_discovered_in_domain`，读用量表）；精选研究准入不读用量表，只认冻结的浅看样本。写入侧自用的读取（`material_admission`、`acquisition_chain` 的去重与就绪探测）不在此列。
  - 是否让「列表与统计类」读取点排除创作者语境材料，是待 Mog 决定的产品口径。标记已落库、表为只追加，之后补过滤不需要回填。

## 控制、恢复与交付

`POST /api/collection/runs/{runRef}/commands` 接受 commandRef、domainRef、expectedRevision、action（pause/continue/stop）。命令身份重放原回执；冲突版本返回 409。run 控制、worker 推进与派发共用 advisory 400；命令身份另用 399。暂停禁止新动作和后续段，继续保留集合/参数，停止不可逆地取消尚未认领订单并保留材料和待交付。停止或结束的 run 不因心跳/迟到回执恢复派发。

插件每次页面动作前调用原本地凭据校验后的 `/api/local/collection/run-control`，失败关闭。执行中探针与推进/控制共用 run 咨询锁，正常锁争用等待至多 5 秒，不把调度器的临时 try-lock 让位解释为人工停止；取得锁后重新核对当前安装凭据和仍有效的领取租约，再判断运行、领域与能力资格。超时或资格变化仍失败关闭；队列派发继续使用原 try-lock，避免阻塞公平领取。已经采回的数据走 durable outbox，优先交付/对账，不重新采。逐页快照以冻结的 CaptureIdentity 和 Package 身份保存；尚在采集的暂存包可单调增加稳定 ID，已封存或已接纳包不可修改。后台退出后先恢复待交付，不能把网络超时当“没有接纳”。

同 run、同作品、同已确认账号、未过期的自有详情页与游标优先复用；仍取得新段的正式 grant 和 Task 身份。页面/账号/会话变化或过期时不盲移游标，重新进入并用本轮稳定 ID 去重。会话保留最多 10 分钟；其内存缓存不成为服务端范围或材料权威。

执行意图、范围结束、成果可用、交付和覆盖分别读取。资源等待不显示完成；领域暂停不停止无关领域。已取得材料立即可读，即使 run 暂停、停止或另有缺口。

## 成果、版本和分页

- `/api/collection/runs`：领域、q、all/active/attention/results/ended，20 条用户记录/页，统计与页数据同一 Repeatable Read 快照。
- `/api/collection/runs/{runRef}?domainRef=...`：真实汇总；`/works` 每页 20，精确顺位、复用依据、检查点、缺口。
- `/api/collection/executions`：原 Task、所有 Attempt/Package/Receipt、冻结作用域/Lease、交付对账，25 条/页；按 run 查询必须同时给领域。
- `/api/local/work-resources` 及详情/评论：携带 domain 和 collectionRunRef。作品原分页/游标指纹含 run；评论页 20、统计与结果同快照，领域不符拒绝或返回空集。

采回数来自原始包稳定 ID；可检索数来自合格共享 Comment，排除当前 restriction；本次新增来自身份级 `linggan_material_comment_first_admission` 的首次接纳包；累计数来自入选作品的共享材料，明确单列。首次接纳索引没有原文，ON CONFLICT 只产生一个归属，并发和重复投递不增加新增数。

本次详情取本次合格包或明确复用的旧包；评论只取本次接纳包的最新观察版本。30→500→30 不覆盖累计 500，历史第一个 run 仍读 30，第三个 run 的新增可为 0。全文词只匹配对应版本的正文/标题/作者/评论，来源关键词在任务搜索中处理，不补进原文。当前媒体/封面 OCR/作者档案与互动历史明确为共享当前上下文，不冒充旧任务本次所得。

## 既有建档、巡查和补采

新旧原工单仍完整可读。建档使用已验证 progressive root；自动创作者巡查详情关联合格发现包和具体 patrol 工单；自动关键词详情按真实发现轮次关联原工单，同批不混轮次。人工补采保留独立行及原发现血缘；不同巡查日期/工单不按目标粗合并。原关系不可靠的历史保持独立执行记录，不伪造历史模式、顺位、新增数或控制能力。迁移只建立关系读取，不触发历史执行。

## 迁移和候选发布

0125 已接在 main 0124 后，登记 local-runtime 与两个完整 schema fixture/hash。包含新增表/视图、用途约束/触发器、首接纳索引和历史 identity 回填；没有业务库执行授权。生产应用前需估计历史评论回填和索引/DDL 锁时间，保留旧版本 outbox，做备份与恢复预案。

发布顺序：独立复审与合并授权 → 经另行授权应用数据库增量 → 发布同版本 API/worker → 按工位单独授权加载 0.8.61 候选并验证能力/账号 → 若另获授权才做平台小样本。旧插件可继续旧任务，新单次保持等待；候选 ZIP 可复现只证明包一致，不证明已安装或真实平台覆盖。

### 推进窗口公平性（PR #486 审查修正）

API 每 15 秒的推进每次至多检查 20 次单次采集，按独立的 `last_advanced_at` 轮转，尚未检查的优先；等待工位的旧任务不长期占满窗口。该调度时间使用真实时钟，独立于业务 `updated_at` 与准入重试时钟，不改变冻结意图或平台观察时间。
