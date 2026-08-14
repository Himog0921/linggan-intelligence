# B3 Projection Gate 合并决策包

> 日期：2026-08-11
>
> 工作台固定点：`38721ece5de80c781a4e059d67beda1388af64a6`
>
> 范围：`SRC004-GATE-001/002/003/004/005/007`
>
> 状态：`CONFIRMED_READY_FOR_DARK_IMPLEMENTATION`。用户于 2026-08-11 确认 DR-B3-007/008 方案 A，并将 DR-B3-009 收口为简单来源 URL 方案；本确认只授权暗态实现与隔离证明，不授权接 caller、切流或部署。

## 1. 审计结论

六个 Gate 的业务方向已经存在，但并发、复合 FK、关系撤销、数据库权限和来源动作仍没有唯一物理合同：

- B3 只允许消费 `ContractEvaluationCurrent` 当前指向的 `accepted + full|partial` Evaluation，并要求提交前确认同一 CEC 仍为 current；现有资料明确说普通读取不足以证明这一点，但没有指定锁/CAS 协议（`docs/architecture/v2/02-model-contract.md:346-348`）。
- B2 是 CEC 唯一 owner，现役 writer 用 `contractEvaluationId + revision` 做 B2 内部 CAS（`src/lib/evidence/derived/b2-derived-service.ts:625-678`；`prisma/schema.prisma:4441-4452`），这不能自动证明另一个 B3 事务提交时仍消费同一 current。
- `ContentMediaUsage` 只有 ContentAsset/MediaItem FK 和可空 `sourceRevision`，没有 accepted Evaluation、CanonicalObservation、CanonicalMediaSlot、MediaOrigin generation 或五字段 Outbox 的数据库绑定（`prisma/schema.prisma:3803-3822`）。五字段只在物理媒体处理边界被运行时验证（`src/lib/services/media-processing-queue-service.ts:42-56,220-227,335-429,496-504`）。
- accepted→rejected 必须由 B2 在推进 CEC 的同一事务撤销 Projection，且不能删除共享 Media Asset；但冻结步骤遗漏 ContentMediaUsage，现役媒体 writer 也只跳过冲突、不关闭旧关系（`docs/architecture/content-workbench-v2-design-freeze.md:584-593`；`src/lib/services/media-library-write-service.ts:378-417`；`docs/architecture/v2/06-v2-operating-contract.md:59-63`）。
- 权限矩阵让 `canonical_writer` 隔离 Projection，同时又给 `default_app` 普通 UPDATE，数据库没有机制阻止后者把 quarantined 直接改回 visible；PresentationRequirement 的撤销 owner 也互相冲突（`docs/architecture/content-workbench-v2-design-freeze.md:622-634`；`docs/architecture/v2/04-blocker-ledger.md:198-220`）。
- ContentObservation 的 ID/default/PK、同 workspace 关系和 onDelete 尚未定义；ContentCurrentProjection 的 Observation FK、CAS 与版本转换也未定义。AuthorObservation/Current 的来源缺口更多（`docs/architecture/v2/02-model-contract.md:357-407,409-468`；`docs/architecture/v2/04-blocker-ledger.md:76-122`）。
- “打开原文”已确认必须由 Projection 输出受控动作，但仓库没有 action model/type/writer；CurrentProjection 也没有 `openOriginalUrl` 字段。现役执行期 URL helper 会在多个候选间 fallback 或拼 URL，不能作为 Projection 合同（`docs/architecture/v2/01-decisions.md:31`；`docs/code-review/v2-b3-domain-projection-source-audit-2026-08-11.md:96-103,134-146`；`src/lib/xhs-execution-target-url.ts:67-105,121-152`）。

这些缺口存在依赖关系，不应重复决策：Gate 001/004/005 是同一个“CEC 与 Projection 原子性”问题；Gate 002/003 是同一个“accepted 媒体关系生命周期”问题；Gate 007 是独立的“受控来源动作”问题。

## 2. DR-B3-007 — CEC 与 Content Projection 原子性

覆盖：Gate 001、004、005。

### 已确认事实

1. B2 保持 CEC 唯一 owner；accepted 的 B2 事务不创建 Domain Projection（`docs/architecture/content-workbench-v2-design-freeze.md:567-582`）。
2. B3 只投影提交时仍为 current accepted 的 Evaluation；相同 Observation replay 不增版本，新 accepted Observation 才单调推进（`docs/architecture/v2/02-model-contract.md:348,399-407`）。
3. rejected 替换 accepted 时，B2 必须在同一事务 quarantine 旧 Projection；异步补偿和 fallback 不允许（`docs/architecture/content-workbench-v2-design-freeze.md:584-593`；`docs/architecture/v2/02-model-contract.md:548-553`）。
4. Content 模型尚可收口为首个窄切片；AuthorObservation 的大量字段、复合 FK 和缺失语义仍为 `SOURCE_INCOMPLETE`，CommentObservation 的完整物理合同也仍受 BLK-008 阻塞（`docs/architecture/v2/02-model-contract.md:409-468,531-541`）。

### 方案 A（推荐）

采用 **Content-only 首切 + 数据库受控状态转换**：

- B3 在一个 SERIALIZABLE 事务中锁定精确 CEC 行，锁后重新读取 Evaluation/Input，只有它仍为 current accepted 才写 ContentObservation、CAS 推进 ContentCurrentProjection；B2 更新同一 CEC 行必须等待或冲突重试。
- ContentObservation 使用同 workspace 的 ContentAsset/CanonicalObservation 复合 FK；CurrentProjection 使用 `(workspaceId, contentAssetId, currentObservationId)` 绑定同一个 ContentObservation，全部 `onDelete=Restrict`。
- `accepted` 只表示合同裁决接受，不证明内容仍在线、已发布或处于任何业务生命周期；首切不得把 `lifecycleState` 默认成 `active`。在 producer/Canonical 提供可验证生命周期来源前，该字段保持显式未知（物理上可空且不参与 visible eligibility），页面可见性只由 current accepted 与 `visibilityState` 决定。
- 首次 accepted 为 `version=1`；同 Observation replay 不增版本；不同的新 current accepted 以旧指针+旧版本 CAS 为 `version+1`；stale accepted 零推进。
- quarantine 不伪装成新 Projection 版本；它是可见性撤销。`default_app` 不得直接写 visibilityState。正常推进/恢复和 B2 撤销分别走数据库受控函数，函数在数据库内验证 current accepted 或 rejected 切换。
- accepted A 被更新的 accepted B 取代时也执行同一撤销边界：B2 推进裁决的同一事务先 quarantine A 并关闭 A 的 active 媒体关系；在 B3 完成 B 前允许短暂无展示，不允许继续把 A 当作当前版本。B 完整提交后才恢复 visible 并激活 B 的关系。
- 只有新的 current accepted Observation 可以把 quarantined 恢复为 visible。
- 本切片只建 ContentObservation/ContentCurrentProjection；Author、Comment、Metric 不建空壳、不复用 V1 writer，等待各自来源合同关闭。

仍需在用户确认后由主线转录的物理细节包括：ContentObservation 的 `String @id @default(cuid())`/`createdAt @default(now())`、复合 target unique、列级权限与 SECURITY DEFINER 函数签名。它们在确认前仍是候选，不能先落库再反证来源。

### 方案 B

只在 service 内先读 CEC、写 Projection，依靠 TypeScript、SERIALIZABLE 或事后检查发现漂移；visibilityState 继续由普通 UPDATE 修改。

风险：B2 在 B3 校验后推进 CEC 时，B3 仍可能提交过期 Projection；普通应用也能解除隔离。测试 happy path 可以通过，但数据库无法证明“错误事实不可能产生”。

### 用户只需确认的非技术原则

> 先只实施来源已经完整的“内容/笔记”暗态投影；系统必须在落库时确认它仍是当前被接受版本，隔离只能由后来新的被接受版本解除。被接受不等于内容仍在线，生命周期未知时必须如实保持未知。作者和评论宁可延后，也不补猜字段。

### 可证伪验收

- CEC 在 B3 校验后、提交前切换：旧 B3 事务必须等待后失败或重试后拒绝，零旧 Projection 推进。
- accepted A→accepted B 在 B2 提交后、B3 尚未运行的检查点：A 已 quarantined、A 的 active V2 Usage 已关闭、页面查询为零；B3 完整提交后 B 才 visible。
- 两个相同 accepted 并发请求收敛为一条 Observation、一个 current、版本不重复增加。
- stale accepted 不能覆盖较新 current；跨 workspace/currentObservation FK 被 PostgreSQL 拒绝。
- 任一 fixture 缺少生命周期事实时，Projection 不得写入 `active/published/online` 等肯定状态，且生命周期未知不得阻止 current accepted 的暗态 Projection 证明。
- `default_app` 直接 quarantined→visible 被数据库拒绝；新 current accepted 经受控函数可以恢复。
- schema 中不存在本轮猜造的 AuthorObservation、AuthorCurrentProjection、CommentObservation 或 Metric V2 写入。

## 3. DR-B3-008 — accepted 媒体关系激活与撤销

覆盖：Gate 002、003；依赖 DR-B3-007 的同一 CEC 锁与事务边界。

### 已确认事实

1. observed CanonicalMediaSlot 到 MediaItem/MediaOrigin/Outbox 的五字段物理证明已完成，但不以 accepted 为建槽条件，也没有激活 ContentMediaUsage（`docs/code-review/v2-b3-media-source-audit-2026-08-11.md:97-121,169-171`）。
2. 只有 current accepted Observation 才能激活业务媒体关系；rejected 必须撤销关系但保留共享 MediaItem（`docs/architecture/v2/01-decisions.md:27`；`docs/architecture/v2/06-v2-operating-contract.md:59-63`）。
3. `sourceRevision` 是可空字符串，不能充当 FK 或五字段事实证明（`prisma/schema.prisma:3803-3822`）。

### 方案 A（推荐）

演进既有 `ContentMediaUsage`，不创建平行媒体关系表：

- 为 V2 Usage 保存 accepted `contractEvaluationId`、`canonicalObservationId`、`canonicalMediaSlotId/slotId`、`mediaProcessingEventId`、`mediaOriginId` 与 `originGeneration`。
- 既有 V1 行暂时允许全部 provenance 为 null；V2 行必须全部非空，数据库 CHECK 禁止“只填一半”。V2 writer 不接受 legacy/null 形态。
- 复合 FK/受控数据库校验必须同时证明：Evaluation accepted 且包含该 Observation；Slot 属于该 Observation；Outbox 五字段与 Slot/Origin/MediaItem/generation 完全一致；Usage 属于同 workspace ContentAsset。
- 同一物理 MediaItem 可以对应 cover 和 image:0 两个业务槽，不能因物理身份合并而丢掉任一 Usage。
- accepted→rejected 时，B2 在推进 CEC 的同一事务只把旧 accepted Evaluation 激活且仍 active 的 Usage 写入 `validTo`；保留整行作为审计，不删 MediaItem、MediaOrigin、Blob 或 Outbox。
- 同一 rejected replay 不重复关闭；任一 Usage/Projection/action 撤销失败，CEC 推进整体回滚。

确认后仍需由主线把字段名、复合 unique/FK、partial unique、trigger/函数签名逐项写入模型合同；本卡确认的是“一条现有关系携带完整证明并用有效期撤销”，不是预先授权任意字段设计。

### 方案 B

保持 ContentMediaUsage 不变，另建一张独立 activation/revocation receipt，页面通过 Usage 与 receipt 双读决定是否展示。

风险：同一业务关系出现两份真值，任何漏 join、延迟或 fallback 都可能让已 rejected 媒体继续展示；也会重新引入本项目明确禁止的双读判断。

### 用户只需确认的非技术原则

> 每条正在展示的媒体关系本身都必须带齐“哪次接受、哪个观察、哪个槽、哪个物理来源”的证明；拒绝时整笔关闭这批关系，但共享图片文件永远不删除。

### 可证伪验收

- 五类 provenance 任一为空、跨 workspace、Evaluation 非 accepted、Observation 不在 EvaluationInput、Slot/Outbox 五字段任一不一致：Usage 零创建。
- cover + image:0 共用一个 MediaItem：一个物理资产、两个有独立槽证明的 active Usage。
- accepted→rejected：对应 active Usage 全部 `validTo` 非空，其他 accepted Observation/其他内容关系不受影响，共享资产仍存在。
- 在关闭任一 Usage 时注入失败：CEC、Projection、全部 Usage/action 撤销均零残留。
- 生产页面/服务不存在 `sourceRevision`、JSON 或“找不到新证明就读旧关系”的 fallback。

## 4. DR-B3-009 — 简单、可追溯的“打开原文”链接

覆盖：Gate 007；依赖 DR-B3-007 的 CurrentProjection 和 DR-B3-008 的同一撤销事务。

### 已确认事实

1. 打开原文必须由 Projection 输出，不是 MediaOrigin.fetchUrl、媒体交付 URL、旧 `sourceUrl`，页面不得拼接或 fallback（`docs/architecture/v2/06-v2-operating-contract.md:34-41,43-57`）。
2. B2 已保存严格 note 身份，但当前没有 Projection action 的模型、版本、validator 或失效合同（`docs/code-review/v2-b3-media-source-audit-2026-08-11.md:135-146`）。
3. CurrentProjection 没有 `openOriginalUrl`；不能自行补造 URL 字段（`docs/code-review/v2-b3-domain-projection-source-audit-2026-08-11.md:96-103`）。

### 已确认方案（简化来源 URL）

不创建 `ContentProjectionAction`、Resolver 或独立凭证表。`ContentObservation.originalUrl` 只接收同一 accepted CanonicalObservation 的 `payload.sourcePayload.url`，`ContentCurrentProjection.originalUrl` 只复制当前 ContentObservation 的该值：

- 只接受 `https:` 且 host 为 `xiaohongshu.com`/其子域或 `xhslink.com`/其子域的来源值；校验后保留该分享链接，不解析重定向、不拼接、不替换。
- 来源字段缺失时 `originalUrl=null`，如实表达不可用；存在但协议/host 非法时整笔 Projection fail-closed，不能静默置空后成功。
- 不读取或复制旧 `ContentAsset.sourceUrl`、Raw payload、MediaOrigin URL、delivery URL、调用方自报 URL，也不复用 execution target helper。
- 页面/API 只能从 `visibilityState=visible` 的 ContentCurrentProjection 输出该链接；quarantined 行不得展示，因此不需要另建 action 生命周期。
- URL 存在不代表内容生命周期 active；它只是一条原始来源入口。

该方案是用户基于当前分享链接稳定可点击的实际业务确认，优先保持简单；未来若出现权限隔离、动态凭证或跨平台解析需求，再另立决策，当前不得预建扩展层。

### 未采用方案

建立结构化 Action/Resolver，或从旧字段/现役 helper 生成链接。

未采用原因：当前只有可直接点击的 XHS 分享链接需求，引入 Action/Resolver 属于过度设计；旧字段/helper 方案则会重新产生多候选和 fallback。

### 用户只需确认的非技术原则

> 页面直接使用当前被接受观察中已经验证的小红书分享链接；不额外建设动作系统，也不允许页面拼接或从旧字段兜底。

### 可证伪验收

- 合法 XHS/xhslink HTTPS 分享链接原样进入 Observation/Current；缺失时为 null。
- `javascript:`、`http:`、非 XHS host、MediaOrigin/delivery URL、旧 `ContentAsset.sourceUrl` 或调用方自报 URL 均不能进入 Projection。
- URL 非法时 Domain/Current/Usage 全部零推进；同 accepted replay 不增 Observation 或 projectionVersion。
- rejected 后 Current 为 quarantined，页面查询返回零链接；新 accepted 只使用自己的来源链接。
- 页面静态扫描与集成测试证明不存在 URL 拼接、旧 URL 读取或找不到 originalUrl 时的 fallback。

## 5. 一次性组合推荐与确认文本

三张卡不是三个独立架构方向，而是一条闭环：DR-B3-007 证明“谁是当前 accepted”，DR-B3-008 让媒体关系携带这份证明并可原子撤销，DR-B3-009 只把同一 current Observation 的已验证分享链接复制到 Projection。

用户确认组合：**DR-B3-007 A + DR-B3-008 A + DR-B3-009 简化来源 URL**。

用户只需一次确认：

> DR-B3-007、DR-B3-008 采用方案 A；DR-B3-009 采用简单来源 URL。先实施 Content-only 暗态 Projection；任何成功必须在数据库提交时仍能证明 current accepted。accepted 不得被解释为内容生命周期 `active`，来源不足时保持未知。媒体关系必须携带完整 Canonical/物理来源证明，rejected 在 B2 同一事务关闭关系但保留共享资产；打开原文只复制同一 accepted Observation 中已验证的 XHS 分享链接，不建设 Action/Resolver，不拼接或 fallback。Author、Comment、Metric 在各自物理来源合同关闭前延后，不补猜。

下一张代码工单可据此转录精确 schema/migration、数据库强制机制和攻击性隔离证明；仍不得接 caller、切流或部署。
