# 评论研究产品化：接口、事务与执行合同

> 状态：技术设计定稿，待用户批准开发；不是已实现或已验收声明
> 交付包：COMMENT-STUDY-PRODUCTIZATION-001 · 文档版 1.0
> 核对日期：2026-09-22
> 源码基线：`main@c74d72e3d17b9d5ecfb9953de025713c47e4560e`
> 责任：本包设计由 ChatGPT 整理；开发、共享库操作、真实模型调用与发布由 Mog 单独授权
> 事实与设计：标为“现状”的内容来自固定版本源码；“规定／新增／必须”为本次目标合同

## 1. 入口与共同类型

保留页面 `/corpus/comments?domain=<uuid>` 与 API 前缀 `/api/local/comment-study`。新读取响应 contract=`comment-study.read.v2`；与前端同一发布切换，不维持两套研究写路径。生成式输出合同仍沿用已有 v1，方法、输入和读 API 的版本不混为一谈。

HTTP 只负责参数、来源 guard 和 DTO。业务入口是 Rust `start_study_run`，不是在 Worker 中模拟点按钮。所有读取继续 loopback 与当前本地 host/origin guard；新增写接口必须复用并测试该 guard。拒绝非允许 Host/Origin、跨域写和非 JSON body；CORS 不允许 `*`。字段拒绝 unknown，UUID／数量／模式均有边界。授权错误不泄露其他 domain 的数据。

### 1.1 通用响应

分页响应含 contract、domainRef、items、page、indexCoverage（仅目录类）。page 必填 limit、hasMore、nextCursor、asOf；`nextCursor` 无下一页为 NULL。总量放 summary 接口或明确的 summary 字段，不能使用 items.length 代替。金额与模型 usage 未知返回 NULL，不当 0。

错误响应固定：`{error:{code,message,retryable,details},requestRef}`。details 只包含安全数量与状态；禁止 SQL、DSN、secret、provider 原始正文或真实评论。code 见 §12。没有请求 UUID 时 requestRef 可 NULL。

列表默认 limit=50，允许 1–100；单次 Run 仍 ≤100 篇、≤3000 Target。page 不是提交上限；能浏览全部作品，不意味着一次研究全站。

### 1.2 稳定分页

游标为 base64url 的版本化 JSON，不是权限凭证。严格字段为 `v,resource,scopeHash,asOf,last`，长度 ≤2048；scopeHash 由 domain、q、筛选与排序规范化计算。跨资源／筛选复用返回 cursor_scope_mismatch；页大小可变但不能越过上限。游标参数始终绑定 SQL 参数，不插值进 ORDER BY。

Run／Signal／Problem／调用列表使用 `(created_at DESC,ref DESC)` keyset。Target 采用 `(created_at ASC,target_ref ASC)`，匹配冻结目标顺序；评论目录采用 `(source.created_at DESC,content_public_ref,comment_external_id COLLATE "C")`；作品采用 `(content_public_ref ASC)`，不把实时 eligibleCount 排序当稳定游标。可额外展示当前计数，但不得据此改变翻页位置。

asOf 是列表材料截止时间，不是跨 HTTP 长期持有的数据库快照。当前限制可令后页减少，晚提交或权限变化后需刷新；只有 Run 冻结清单是准确不可变样本。不得把长事务挂到用户浏览会话。

### 1.3 公共嵌套对象与计数优先级

indexCoverage 恰含 state(`ready/partial/unavailable`)、indexedCount(integer或NULL)、pendingCount(integer或NULL)、asOf。indexedCount 是当前scope的最新可读材料中存在匹配cleaner缓存的条数，包含已经判为dropped的缓存；它不是“可研究数”。pendingCount 是尚无该缓存的最新可读材料数。只有完成范围核对且pendingCount=0才ready；数据库失败用503，不返回假ready。

开始/预览的 exclusionCounts 键固定：sourceRestricted、bodyUnavailable、indexPending、textNotResearchable、workAuthorUnknown、commentAuthorUnknown、creatorVoice、inProgress、notSelectedByMode、budgetNotSelected，值均非负整数。按此顺序每个稳定评论只归入一个计数；通过全部条件且入包者计入targetCount，needs_context也属于目标。相同冻结快照内 `scopeCommentCount = targetCount + sum(exclusionCounts)`。未知的commentKey/跨domain引用是请求错误，不混为“清洗排除”；有效作品但无评论可正常no_work。

latestStudy/effectiveStudy 恰含 runRef、targetRef、state、createdAt、finishedAt(可NULL)、inputComparison(`same/changed/unknown`)、sourceState(`known/unknown/restricted`)，对象本身可NULL。effectiveState（评论行字段）为 `none/effective/source_changed/source_unavailable`；旧失败不会抹去有效head，当前正文变化不伪装为当前有效。

costSummary 恰含 recordingState(`full/legacy_partial/unavailable`)、tokenLimit(历史可NULL)、invocationCount、dispatchedCount、chargedTokens、knownInputTokens、knownOutputTokens、unknownUsageInvocationCount、runningReservedTokens。known前缀表示只汇总已知usage，不能当全部实际消费；费用USD不在本包强行推算。通用账本有已证明金额来源时再单独扩展金额合同。旧Run的已知invocation通过semantic_attempt、resolution、pair三条现有关系UNION DISTINCT恢复，只读列出可证明的记录，recordingState=legacy_partial；不能因为旧Run没有新snapshot就报“0次调用”。新Run要求所有三阶段invocation都可由model_request关联，否则视为完整性缺陷。

所有目录、预览、启动共用 `comment_study_source.rs` 中的一套SQL资格CASE与Rust错误枚举映射：缓存只承担clean语义，作者／限制判断统一由同一关系计算。现有Rust逐行classify入口改为消费该关系或调用同一纯判定测试，不在UI/SQL/Rust维持三套互不一致的业务门槛。单条冻结查询可用CTE先构建候选和计数，再只返回有界选中材料＋计数，不把全库正文fetch_all进Rust。

## 2. 目录与来源接口

| 方法／路径 | 请求 | 响应职责 |
|---|---|---|
| GET `/comments` | domain、q、workRef、voiceRole、studyState、cursor、limit | 有效文本原声目录，不依赖 runRef |
| GET `/comments/detail` | domain、workRef、commentExternalId | 当前原声＋历史版本＋作品与父评论＋研究历史入口 |
| GET `/works` | domain、q、studyState、cursor、limit | 可浏览全部作品与研究覆盖；不截断前 100 |
| GET `/catalog-summary` | 同目录筛选，但无 cursor/limit | 当前原声、可研究、身份未知、作者声音、无效、待清洗数量及 asOf |
| POST `/selection-preview` | 与启动命令相同的 domainRef/policyRef/scope/mode/limits，不含 requestRef | 不持久化样本、不占位、不外发；返回可预估的选择数量和限制 |

q 是 1–200 字的**字面子串**，Unicode 正常文本可查；空字符串等于不筛选。对 `\`、`%`、`_` 做 LIKE 转义，使用显式 ESCAPE。不能把 `simple` FTS 当成中文分词，也不以点赞提高词义相关性。1–2 个中文字允许搜索；若触发执行时间上限返回明确查询错误，不返回假空结果。

voiceRole 闭集 reader/creator/unknown/reader_and_unknown/all，默认 reader_and_unknown；creator 只可查阅，不进入当前研究。studyState 闭集 all/never_studied/in_progress/studied/needs_context/failed/input_changed。这些是投影，不新增落库研究状态。

CommentItem 必填：`commentKey:{workRef,commentExternalId},sourceRef,commentText,researchText,voiceRole,cleanState,cleanReasons,sourceState,studyEligibility,latestStudy,effectiveStudy,effectiveState,observedAt,receivedAt`。可空：authorDisplayName、authorExternalId、parentCommentKey、likeCount、likeCountAsOf、publishedAt。原材料没有可靠点赞／发布时间就为 NULL；不得从作品点赞推评论点赞，observedAt 不当 publishedAt。

studyEligibility=`{eligible:boolean,reasons:string[]}`；latestStudy/effectiveStudy 分别表示最后尝试和当前有效结果，可 NULL。两者至少有 targetRef/runRef/state/createdAt/inputComparison；这样最后一次失败不会覆盖仍有效的旧结果。

作品行：workRef、displayTitle、displayTitleSource、eligibleCommentCount、indexedCommentCount、pendingIndexCount、studiedCommentCount、inProgressCommentCount、needsContextCount、lastStudyAt。标题调用现有 Evidence 显示标题解析器；没有合格封面 OCR 就显示未命名作品，不把任意 OCR 长文直接塞标题。

## 3. 方法版本 API

| 方法／路径 | 合同 |
|---|---|
| GET `/policies` | domain/cursor/limit；返回 policyRef、methodName、parentPolicyRef、createdAt、methodHash、recordingState、defaults、isActive |
| GET `/policies/{policyRef}` | 返回完整可读 method_manifest、严格 Schema、模型元信息；不返回连接秘密 |
| POST `/policies` | domainRef、methodName、parentPolicyRef 可 NULL、modelConfigRef、defaults、stageInstructions；复制／创建不可变新版本 |
| POST `/policies/{policyRef}/activate` | expectedActivePolicyRef 可 NULL；CAS 修改默认，冲突返回 409；不影响已运行批次 |

defaults 恰有 commentBudget/contextCharacterBudget；从实际配置读取建议 tokenLimit，仅作为 UI 初始值，启动仍必须明确提交。stageInstructions 恰有 semantic/resolution/pair；每项字符串 ≤8000 字。无 parent 时，服务器以当前编译版本内的固定规则和完整 Schema 构建首个方法；用户研究说明可空。复制时完整加载父方法，不根据父名字猜内容。

客户端不能提交 outputSchema、safetyRulesRevision、methodHash 或 server origin；这些均由服务端构建。保存方法执行静态合同校验，不自动调用模型。旧 `/policy` 写入口切换为 410 policy_endpoint_retired，前端同次移除。旧设置记录只读保留；不能悄悄把旧请求兼容成新版本并启动。

## 4. 单一启动命令

### 4.1 HTTP 形式

POST `/runs`，所有字段均必填，scope 与 reason 规则如下：

```json
{
  "requestRef": "00000000-0000-4000-8000-000000000101",
  "domainRef": "00000000-0000-4000-8000-000000000001",
  "policyRef": "00000000-0000-4000-8000-000000000102",
  "scope": {
    "kind": "works",
    "workRefs": ["00000000-0000-4000-8000-000000000103"]
  },
  "mode": "new_only",
  "limits": {"commentBudget": 100, "contextCharacterBudget": 6000, "tokenLimit": 100000},
  "reason": null
}
```

示例数值是合成配置，不是推荐真实 token 预算。scope.kind 为 works 时恰有 kind/workRefs，1–100 个 UUID；为 comments 时恰有 kind/commentKeys，1–3000 个 `{workRef,commentExternalId}`，且作品去重 ≤100。commentExternalId 字节上限 512，必须逐字匹配原始字段。集合规范化去重；不得静默新增未选择作品或评论。

mode=new_only/input_changed/retry_failed/reanalyse。new_only 的 reason 必须为 NULL；非 new_only 必须有 trim 后 1–500 字 reason，并在最终确认中展示；不是逐条人工审核，只是一次批量意图确认。reason只作动作审计，不自动插进模型提示词。自动调用暂不通过 HTTP 接受，origin=manual 由服务器注入。

返回 HTTP 201（新 Run），200（同请求回放或 no_work/index_pending）：requestRef、outcome、runRef（可 NULL）、asOf、requestedWorkCount、coveredWorkCount、targetCount、queuedCount、needsContextCount、limits、exclusionCounts、indexCoverage、dispatchState、idempotentReplay。**文字是“已创建并进入执行队列”，不能说“尚未授权模型”。** Worker 还未领取时显示排队，不伪造 running。

### 4.2 内部 Rust 合同

```rust
pub struct StartStudyRunCommand {
    pub request_ref: Uuid,
    pub domain_ref: Uuid,
    pub policy_ref: Uuid,
    pub scope: StudyScope,
    pub mode: StudySelectionMode,
    pub limits: StudyRunLimits,
    pub reason: Option<String>,
}
// StudyScope: Works(Vec<Uuid>) | Comments(Vec<CommentKey>)
// 下一阶段增加 DomainBacklog { after_work_ref: Option<Uuid> }；不对 HTTP 开放。
// Origin 不是 JSON 字段，由已通过权限边界的调用者构建。
pub async fn start_study_run(
    database: &Database,
    command: StartStudyRunCommand,
    origin: TrustedStudyOrigin,
) -> Result<StudyStartReceipt, StudyStartError>;
```

TrustedStudyOrigin 本轮 Manual；Scheduled{schedule_ref,scheduled_for} 只用于测试入口，公开 HTTP 无法构建该分支。不把“允许枚举”当成已开启计划。未来 schedule 与启动共用事务时，复用私有 `start_study_run_in_transaction`，不是 copy-paste handler。

### 4.3 选择模式真值表

先检查动态资格，再排除已有获准在途 Target。所有模式都受同一在途唯一性限制。

| 历史／当前事实 | new_only | input_changed | retry_failed | reanalyse |
|---|---|---|---|---|
| 从未创建过 Target | 选择 | 不选择 | 不选择 | 选择，需明确理由 |
| succeeded 或 no_signal，输入未变 | 跳过 | 跳过 | 跳过 | 选择 |
| 成功结果且 fingerprint 明确变化 | 跳过 | 选择 | 跳过 | 选择 |
| needs_context，依赖仍相同／缺失 | 跳过 | 跳过 | 不选择 | 可选择，但 UI 说明仍可能无调用 |
| needs_context，父语境明确补齐且 fingerprint 变化 | 跳过 | 选择 | 不选择 | 选择 |
| failed | 跳过 | 仅输入确实变化时 | 选择 | 选择 |
| cancelled | 跳过 | 仅输入确实变化时 | 选择 | 选择 |
| 任何获准在途目标 | 跳过 | 跳过 | 跳过 | 跳过，不支持 force |
| 旧方法未记录、所属Run已安全stopped且无存活调用的历史未完成目标 | 跳过 | 跳过 | 可明确续做 | 可明确重研 |
| 历史输入方法无法确定 | 有历史 Target 则跳过 | 跳过并标 unknown | 失败记录可选 | 明确重研可选 |
| 旧excluded，但当前来源已恢复合格 | 跳过 | fingerprint明确变化才选择 | 不选择，使用input_changed | 可明确重研 |
| 来源当前受限／身份不合格／无效文本 | 不选择 | 不选择 | 不选择 | 不选择 |

new_only 是避免自动重复成本的保守默认，不等于“把所有没有成功的东西每天无限重试”。未调用就取消的目标也保留历史，使用 retry_failed 批量续做。下一阶段自动计划默认只处理 new_only；可单独授权输入补齐模式，不擅自启用所有失败重放。

排序：各选定作品内部按首次可用材料的稳定顺序取最早未处理评论，再按作品逐条轮转，最多 commentBudget。具体序 `(row_number_in_work,content_public_ref,comment_external_id COLLATE "C")`，作品内 `(source.created_at ASC,source.material_ref ASC)`；排序不是“随机代表性样本”。新 Run 记录 selectionOrder。缺缓存的材料不作无效删除，部分可用量如实入队。

## 5. 启动事务与并发

### 5.1 明确选择 READ COMMITTED

启动事务使用 READ COMMITTED。先取得 domain 事务级 advisory lock，再执行幂等检查和**一条冻结输入 SELECT**。该 SELECT 一次产出所选 raw head、合格父语境、每作品语境、历史及缓存结果。Rust 对已返回输入进行确定性处理后写入；不得再在另一条 SELECT 换一份当前语境拼进去。

这既保留同次冻结输入的一致性，又避免现有 Repeatable Read 中“等到锁以后仍看旧快照”的陷阱。网络请求／模型／OCR／Embedding 不得出现在此事务内。

锁 key 固定为 SHA-256(`"comment-study/domain/" + UUID小写字符串`) 的前 8 字节，按有符号 big-endian i64 解读，调用 `pg_advisory_xact_lock($1)`。这是并发互斥，不是实体身份；极小概率 hash 冲突只多串行，不合并数据。后续仍验证真实 domain_ref。

### 5.2 事务步骤

1. 纯参数校验与 request_hash 规范化；开始 READ COMMITTED。
2. 获取 domain lock；检查 request_ref。若存在，同 hash 返回原回执；不同 hash 返回 409。不得重新挑选样本。
3. 读取明确 policy_ref，确认完整方法与当前 domain；读取不可变 model_config 和连接启用事实；不读活动 policy 代替参数。
4. 获取服务端 asOf；执行单条冻结查询，按已定义模式、资格、输入和稳定评论历史选择。context 按作品仅组装一次，不在每评论 lateral 重复巨量 JSON。
5. 0 个结果：有待清洗则 index_pending，否则 no_work；写终态 request 回执，提交，不创建空 Run。
6. >0：写入 queued Run、实际预算、选择／执行快照、Work 和 Target；needs_context 可直接终态，不依赖模型。写入稳定身份及 fingerprint，让数据库在途唯一约束兜底。
7. 调用 close_run_if_settled，使全 needs_context 的 Run 也能结束语义阶段；写入 request.created 回执。提交后 Worker 方可处理。
8. 返回 committed receipt；请求响应丢失后由同 request_ref 重放，不猜执行是否发生。

事务锁超时 3 秒、statement_timeout 初值 15 秒、序列化／死锁／在途唯一冲突最多重试整个事务 3 次（退避 50/150/450ms，仅工程初值）。retry 时保持 request_ref，重新读取最新资格；真正的 request_hash 冲突不可重试成另一个动作。超限返回 retryable 错误，不创建半个 Run。

### 5.3 全链路锁顺序

所有涉及研究启动、预算、控制、三阶段接纳的短写事务按：**domain advisory → owner Run → subject（batch/resolution/pair）→ Target（UUID 升序）→ invocation**。一事务操作多个 Run 时 UUID 升序；同域 pair 不持有另一域锁。清洗缓存和普通读不拿 domain 写锁。

现有接受器中先锁 Target 再 close_run 的顺序必须一起整理；不能只给开始接口加新锁而保留反向锁序。取得锁只包住短 SQL 和确定性校验，不包模型耗时。事务事务层可共享一个小 helper，不新建锁服务。

## 6. 三阶段完整请求与严格接纳

semantic/resolution/pair 都从 owner Run.policy_ref 读取 frozen method。完整请求在外发前形成 model_request；真实 system/prompt 与参数参与 hash 和预算估算。模型 id、配置与 connection version 使用已有不可变记录；secret 按既有安全存储临时取得，不落 snapshot。

### 6.1 语义提取

最多 12 个同篇 Target；输入给每 Target rawText/researchText/parentContext，workContext 一次。按完整 system＋prompt＋outputSchema 的估算决定装入数量，不能只估 inner manifest。单个目标也不适配时标 failed/input_limit_exceeded，不无限留 queued 卡队首；不能裁掉原评论让模型凭残文引用。

模型输出保持现有闭集 results `{targetRef,outcome,reason,signals}`。每个 Signal 为 kind/proposition/evidence/problemFrame；problem/need 的 frame 四字段为 actor/goalOrExpectedState/barrierOrUnmetNeed/context，每项 value/basis；其余 kind 的 frame=NULL。no_signal 和 needs_context 是明确模型结果，遗漏目标不等于 no_signal。局部合法目标独立保存，其他目标有界重试。

### 6.2 归并与配对

候选集合沿用现有最多20项的服务端边界。完整候选比较请求超输入／输出预算时不静默删掉后半候选：保留不完整／预算原因，等待可用配置，不把剩余几项的no_match当全候选无匹配。

resolution 的严格 Schema 必须完整写出 candidates[].problemRef、dimensions 四项，每项枚举 same/different/unknown；additionalProperties=false，不能只写 object。pair 的 proposedProblem 也完整定义 title/definition/stableIdentity/includeCriteria/excludeCriteria；复用 Rust 当前接受规则，不把旧 equivalent/compatible 枚举带回系统。

先校验 JSON、闭集、来源和候选 revision，再接纳，再写合法比较缓存。cache verdict 由实际 Rust relationship 函数推导，不保留另一套推断。protocol_rejected 的 receipt 不是调用业务成功；通用账本必须准确记录结构拒绝，usage 照实保存。

缓存命中在 reserve invocation 前执行，只有完整、合法且候选 revision 仍匹配的候选集合才可接纳；部分命中不能包装成“全部比较过”。缓存无法写入不回滚已经成功的业务结果，但必须有安全告警，不能静默买重复比较而不留原因。

## 7. 预算与付费副作用

每次外发预留前锁 owner Run，先判断 dispatch_state=enabled，再计算该 Run 的已记账消费：JOIN model_request→通用 invocation，SUM(charged_tokens)。覆盖 semantic/resolution/pair，pair 只归属 firstSignal 所属 Run。每 invocation 一条快照，避免 JOIN 放大费用。

预留 R=`config.input_token_limit + config.output_token_limit`；沿用通用 ledger 的保守预留模型。若 spent＋R>token_limit，本次不创建 invocation，Run 置 stopped/budget_exhausted；未外发目标取消、已完成结果保留，在途已领取外发请求正常结算。UI 显示“预算已用尽，保留部分结果”，不是系统成功清空队列。

有可靠 usage 后 charged_tokens 改为真实输入＋输出；明确未外发的错误为 0；已经可能外发但 usage 未知保留预留，不按 0 释放。USD 只有现有来源能证明时才显示；token 额度不是汇率／定价估算，也不是上游账单的绝对金额担保。真实 usage 超出预留时照实入账、停止下一次调用并报告超额，不能裁掉统计。

预算固定在 Run，不在本轮提供“后台自动加钱”。之后要继续，通过明确新请求和新额度续做未完成项。自动阶段会加跨 Run 的计划日额度；不能仅靠每 Run 限额宣称每日消费已经有界。

## 8. 领取、暂停、停止与崩溃恢复

### 8.1 领取

在已记录 request snapshot 后，外发前再次短事务检查来源、当前 subject.invocation_ref、预算和 dispatch_state，CAS `dispatch_started_at IS NULL → now`。成功者才可发出一次 adapter 请求。相同 invocation 被另一个 Worker 看到，不可再次外发。

semantic 租约长度采用 frozen timeout_seconds＋15 秒，不再固定 60 秒；仍落在既有最大 300 秒范围。其他阶段 deadline 同规则。外发开始前剩余窗口必须大于实际 timeout＋安全余量 5 秒，否则结算为未外发窗口不足；不能偷偷延长租约或修改 snapshot 参数。

### 8.2 控制 API

POST `/runs/{runRef}/pause|resume|stop` body 恰有 expectedControlVersion。CAS 按 Run.control_version，成功状态变化＋1，冲突 409 并返回当前安全状态。pause=paused/user_paused；resume 仅从 user_paused，方法／额度可用且记录完整才 enabled；stop=stopped/user_stopped。stopped 不可恢复。

pause/stop 不撤销已经发出的远端请求，不承诺停止上游计费。在途请求按原 invocation/token/deadline 和现行来源资格接纳后结束；之后不发新请求。尚未外发的已预留 snapshot 安全结算，明确 charge=0；停止时 queued/ready 和未外发 batch 的 Target 置 cancelled、原因 user_stopped，保留所有先前有效 Signal。不能因为 Run 后来 stopped 就拒收一份已授权且按时完成的合法在途结果。

暂停保留队列与在途唯一性；恢复继续原冻结样本。停止只停止这个 Run，不给原评论添加永久 restriction，不意味着未来用户选择同评论违法。

### 8.3 恢复矩阵

| 崩溃／异常位置 | 持久事实 | 下次处理 |
|---|---|---|
| 启动事务提交前 | 无 Run／无回执 | 相同 request_ref 重试 |
| 提交后响应丢失 | 有回执和准确 Run | 返回同一 Run |
| batch 已准备但未 reserve | frozen targets + batch | 正常领取，非丢失任务 |
| reserve 后、dispatch CAS 前 | request+ledger，未外发 | 有界恢复；可明确 0 usage；原 invocation 不复用 |
| CAS 后、HTTP 之前或进行中 | 可能已外发，不能确认 | 过期结算为 unknown dispatch；保留预留，计一次尝试 |
| provider 返回非法 JSON | 有真实 usage | usage 先入账；业务拒绝；有界重试，不写缓存 |
| 单目标输出缺失 | 同批其他合法结果 | 保存合法结果，仅缺失目标计尝试 |
| resolution/pair ledger 失败但 subject 指针未清 | 可审计历史 request | 同事务或恢复器终结对应状态；不永远 pending |
| 来源后来受限／候选 revision 变化 | 旧输入仍可审计但当前不可用 | 隔离相应项；revision 变化为 retrieval_incomplete，不造新问题 |
| 过期 invocation 晚到结果 | 当前指针已变化或 ledger 已终态 | 拒绝，不更新 membership／Signal／最新 usage 为成功 |

每个 Target 的语义尝试最多 3 次；resolution/pair 对同一个冻结比较上下文也最多 3 个请求快照。预构建／预外发失败也计入防自旋上限；不能把 unavailable secret 每 tick 放回队列而无限重试。上下文改变后开启的是明确的新比较上下文，不是重置同样失败的计数。

这里保证的是**数据库写入幂等、每 invocation 至多一次本地外发领取、有限重试**。网络不确定时无法保证上游只计费一次；不得把这些机制命名为端到端 exactly-once。

## 9. 小型 Worker 顺序，不建新调度器

每次 tick 固定先回收全部三阶段的到期请求（每类最多 32），再清洗至多 128 条；随后循环尝试 semantic/resolution/pair 三个 lane，从上次成功 lane 的下一个开始。每个 tick 至多一次外部模型请求；至多 16 条本地 embedding。游标只为进程内公平性，重启归零不影响数据正确性。

resolution lane：先完整缓存命中，再召回／补齐条件，再外发；pair lane 只处理合格且未完成的 pair。semantic lane 有已准备 batch 时优先执行，而不是不断准备新 batch；本 tick 可先准备一批再执行，不用“准备成功”返回阻断所有执行。

单 lane Model/Adapter/契约错误写安全原因并继续试其他 lane；数据库连接整体失败则结束该 tick。embedding 不可用降级为 retrieval_incomplete，但不阻塞 semantic。重任务 OCR/ASR 在运行时仍沿用已有本地资源让路机制。

保持现有 10 秒 tick 初值和 drain 接口；这些是工程起点，可通过实测优化。不得新增每 Tab 轮询一次 Worker 或启动多进程模型竞争来提高表面吞吐。

## 10. 结果、问题与概览读取

| 路由 | 参数 | 必须返回 |
|---|---|---|
| GET `/runs` | domain/cursor/limit | RunItem 含 requested/coveredWorkCount、targetCount、targetStates、dispatchState/reason/controlVersion、method概要、origin、createdAt |
| GET `/runs/{runRef}` | domain | frozen scope/limits/method、semanticSummary、knowledgeSummary、costSummary；历史缺失字段标 NULL |
| GET `/targets` | domain/runRef/state/cursor/limit | 完整分页目标，source 状态、latest attempt、停止或依赖原因 |
| GET `/signals` | domain/runRef/kind/resolutionState/cursor/limit | 完整分页 Signal 和原声、问题关联；不混合当前 head 与历史口径 |
| GET `/runs/{runRef}/requests` | domain/cursor/limit | 三阶段请求元数据、模型、次数、耗时、usage／未知费用 |
| GET `/requests/{invocationRef}` | domain | 有权限时完整 snapshot；缺记录或受限明确返回 recordingState |
| GET `/problems` | domain/q/state/cursor/limit | title、definition、定义可读性、边界、revisionRef、不同评论/作者/作品数、supportState、最近新增依据 |
| GET `/problems/{problemRef}` | domain | 当前 revision 与定义、历史 revision 可达、合法依据分布、可比边界 |
| GET `/problems/{problemRef}/evidence` | domain/cursor/limit | 先按稳定评论分页，再附该评论的有效 Signal；不按 membership 展示重复原声 |
| GET `/problem-candidates` | domain/state/cursor/limit | 仅投影现有未建档 Signal/resolution/pair，非新实体 |
| GET `/overview` | domain | 保留 observationSeries；另含独立计算的 corpusSummary/researchSummary/knowledgeSummary 与 asOf |

knowledgeSummary 精确字段：assignedSignalCount、pendingResolutionCount、retrievalIncompleteCount、deferredNovelCount、deferredAmbiguousCount、deferredContextCount、budgetStoppedCount、protocolRejectedCount、pendingPairCount。不是每条 Signal 只能显示一个“最终成功百分比”；维度计数不必强行加成100%。

Run.state completed 仅意味着目标语义阶段结束；UI 用“提取结束”。knowledgeSummary 仍可能有 pending/deferred。Problem 内容来自 revision，不由前端临时概括；未建档表达仍可读、可查理由，不为填页面降低“两评论不同作者”建档要求。

## 11. 页面刷新与读取负载

只有当前可见 Tab 请求自己的列表；总览读数单请求，切换不并发刷新所有 Tab。运行中当前 Run 10 秒刷新元数据与必要当前页；页面隐藏暂停刷新，终态改手动刷新。使用 AbortController 中止旧筛选请求，旧响应不得覆盖新 domain/run/filter。

公开的本地响应 Cache-Control=no-store；浏览器持有的研究数据只在内存，关闭详情／切换领域清理，不把正文写 localStorage。滚动／游标／筛选位置可放 sessionStorage，但不得包含评论正文、密钥或完整模型请求。

## 12. 错误码最小闭集

| code | HTTP | 含义／是否可重试 |
|---|---|---|
| invalid_request / unsupported_domain / invalid_limit | 400 | 参数错误，不自动重试 |
| cursor_scope_mismatch / invalid_cursor | 400 | 刷新筛选，不拼旧页 |
| local_access_denied | 403 | 本地边界不满足 |
| resource_not_found | 404 | 不存在或不允许披露的跨域引用 |
| idempotency_conflict | 409 | 同 requestRef 不同命令；必须明确新意图 |
| policy_unrecorded / policy_unavailable | 409 | 方法不可用于新 Run |
| control_version_conflict / run_stopped | 409 | 控制冲突；读最新状态 |
| policy_endpoint_retired | 410 | 旧写入口已经替代 |
| study_busy | 409 | 锁或并发重试耗尽，可同 requestRef 重试 |
| catalog_unavailable / query_timeout | 503 | 查询没有成功，不是0结果 |
| study_schema_unavailable | 503 | 缺增量 schema；不自行 bootstrap/reset |
| model_runtime_unavailable | 503 | runtime 未就绪；与无待处理材料不同 |

no_work/index_pending 是明确开始回执 outcome，不伪装为网络错误。其后重新点击开始使用新 requestRef；同一个旧请求只回放原结果。
