# COLLECTION-SINGLE-RUN-001 · 一次采集的持久范围与读取合同

> 状态: 代码事实优先
> 最后核对: 2026-10-23（up_to 增量）
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

`collectionRunRef`、`commentMode` 和可选 `incrementalCommentBudget` 贯穿工单、TaskSpec、页面计划、采集器和回执。detail-only 0 表示不要求评论；all_public 由明确模式表示，绝不用 0 或超大整数替代目标。knownCommentIds 来自本轮原始接纳包，包含清洗掉的记录，避免为凑 30 条有效文本另采。knownReplyCounts 只允许单次标准模式，根 ID 有界、数值 0–2。

插件宣告 `collection.single_run.v1`；深采（all_public 与 up_to）另需 `collection.all_public.v1`。未支持的旧插件保持等待，不认领新合同、不悄悄降为采样。旧任务不带新字段时继续原 v1/v2 校验、outbox 和回执路径。

新的 all_public 完成回执必须具有 mainExhausted、repliesExhausted、identityReliable 的真实遍历依据。显示计数达到、no_progress 或主评论到底不能单独证明整篇采全。回复关系/稳定身份不足保留未知或部分；连续三段无新稳定 ID 后保留可说明的缺口，不无限自动重试。风控/访问受限停止受影响执行，不切换账号规避；一篇失败不连坐其他合格材料。

## 控制、恢复与交付

`POST /api/collection/runs/{runRef}/commands` 接受 commandRef、domainRef、expectedRevision、action（pause/continue/stop）。命令身份重放原回执；冲突版本返回 409。run 控制、worker 推进与派发共用 advisory 400；命令身份另用 399。暂停禁止新动作和后续段，继续保留集合/参数，停止不可逆地取消尚未认领订单并保留材料和待交付。停止或结束的 run 不因心跳/迟到回执恢复派发。

插件每次页面动作前调用原本地凭据校验后的 `/api/local/collection/run-control`，失败关闭。已经采回的数据走 durable outbox，优先交付/对账，不重新采。逐页快照以冻结的 CaptureIdentity 和 Package 身份保存；尚在采集的暂存包可单调增加稳定 ID，已封存或已接纳包不可修改。后台退出后先恢复待交付，不能把网络超时当“没有接纳”。

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
