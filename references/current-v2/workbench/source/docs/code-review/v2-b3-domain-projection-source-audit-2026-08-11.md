# B3-A-01：Domain Projection 来源审计（主线复核版）

> 日期：2026-08-11
>
> 工作台固定点：`424e180f3ef3de217fde4492bf2519e9509d0b63`
>
> 插件固定点：`227e365192d2141407599fefbf066aa66bfa0b26`
>
> 结论：**B3_CODE_BLOCKED_BY_SOURCE_GAP**。不是“六个 B3 模型都不存在”：`CanonicalMediaSlot` 是来源完整但尚未实施的 Canonical/B2 模型；`ProjectionReceipt` 不是已定义模型名。真正阻塞 B3 的是领域身份、Observation 完整合同、CEC/撤销事务所有权，以及 Canonical media 到现有 Media Domain 的关系来源。

## 1. 固定点与实际工作树

| 项目 | 实际值 |
|---|---|
| 工作台 HEAD | `424e180f3ef3de217fde4492bf2519e9509d0b63` |
| 插件 HEAD | `227e365192d2141407599fefbf066aa66bfa0b26` |
| 分支 | `v2/b3-projection-readiness` |
| 工作树 | HEAD 未变；`M docs/project-audit.md` + `?? docs/code-review/v2-b3-domain-projection-source-audit-2026-08-11.md`，**不是 clean** |
| 运行流量 | 未接 caller、未双写/双读/fallback |

## 2. 模型分层与来源完整性

| 模型/对象 | 归属 | 当前 schema | 来源状态 | 可证伪依据 |
|---|---|---:|---|---|
| `CanonicalObservation` / `ContractEvaluationCurrent` | B2 Canonical | 已存在 | 已暗态实施 | `schema.prisma`；`b2-derived-service.ts` |
| `CanonicalMediaSlot` | Canonical/B2 前置 | 不存在 | **SOURCE_COMPLETE / IMPLEMENTATION_MISSING** | freeze §5.12 给出 7 字段、键、FK、不可变语义；`02-model-contract.md:321-338` 已逐字段转录 |
| `ContentObservation` | B3 | 不存在 | **SOURCE_INCOMPLETE** | freeze `:404-413` 仅字段速记，无类型、PK/default、完整关系与写入者 |
| `ContentCurrentProjection` | B3 | 不存在 | **RELATION_INCOMPLETE** | freeze `:419-436` 有完整字段，但 `currentObservationId` 无同 workspace FK，状态所有权受 BLK-005/014 阻塞 |
| `AuthorObservation` | B3 | `AuthorSnapshot` 尚未演进 | **SOURCE_INCOMPLETE** | freeze `:439-441` 仅散文式 rename/add/drop，无完整模型、键、FK |
| `AuthorCurrentProjection` | B3 | 不存在 | **RELATION_INCOMPLETE** | freeze `:446-461` 有完整字段，但 `currentObservationId` 无同 workspace FK，受 BLK-007/014 阻塞 |
| `Comment` | B3 现有模型演进 | 已存在 | **SOURCE_INCOMPLETE** | freeze `:464-466` 只说新增 `canonicalObservationId`，无 FK/同源约束；现役唯一键不含 `noteSourceId` |
| `ContentMetricSnapshot/Latest` | B3 现有模型演进 | 已存在 | **NOT_IN_FIRST_XHS_SLICE** | 六合同无 `metric` RawRecord；不得用自造 fixture 宣称完成 |
| `ProjectionReceipt` | 未定义名词 | 不存在 | **SPEC_CORRECTION_REQUIRED** | freeze `:582` 使用此名，但 §5.14 只定义 `PresentationReceipt`；DEC-B1-005 明确普通页面不建设全量回执 |

因此，不能把 `CanonicalMediaSlot` 算作 B3 的不完整模型，也不能凭 `ProjectionReceipt` 一处文字自行新建第七种回执。

## 3. 六份 XHS 合同到 Canonical 的真实形状

### 3.1 合同覆盖

| 合同 | RawRecord kinds | 首期 B3 对象 |
|---|---|---|
| `xhs.list-scan` | note | Content |
| `xhs.note-detail` | note, comment | Content + Comment |
| `xhs.note-full` | note, comment | Content + Comment |
| `xhs.comment-probe` | comment | Comment |
| `xhs.author-profile` | author, note | Author + Content |
| `xhs.author-links` | note | Content |

插件 fixture 的 note 同时带 `noteId` 与 `platformContentId` 且二者相等；author 同时带 `authorId` 与 `platformAuthorId` 且二者相等。这个正例只能证明相等时可通过，不能证明真实运行中字段不相等时应绑定哪一个。

### 3.2 B2 已实施的 subjectKey 与 B3 现役键

| kind | B2 subjectKey 真实公式 | B3 现役业务键 | 结论 |
|---|---|---|---|
| note | `xhs:note:${encodeURIComponent(noteId ?? platformContentId)}`，实际优先 `noteId` | `ContentAsset @@unique(workspaceId, platform, platformContentId)` | **未证明等价**；fixture 只覆盖两值相等 |
| comment | `xhs:comment:${encodeURIComponent(noteId)}:${encodeURIComponent(commentId)}` | `Comment @@unique(workspaceId, platform, sourceId)`，`noteSourceId` 仅索引 | **冲突**；数据库不能表达已确认的 note+comment 组合身份 |
| author | `xhs:author:${encodeURIComponent(authorId ?? platformAuthorId ?? userId)}`，实际按该顺序取首值 | `Author @@unique(workspaceId, platform, platformAuthorId)`，且另需 `authorEntityId` | **未证明等价/字段不足**；fixture 未证明 `authorEntityId` 来源 |

源码锚点：`xhs-derived-contract.ts:450-510`、`schema.prisma:2371-2414`、`schema.prisma:740-809`、`schema.prisma:3415-3449`。在完成 DR-B3-001 前，不得把三类关系标成 `VERIFIED_DIRECT`。

### 3.3 Canonical payload 真实路径

B2 写出的 `CanonicalObservation.payload` 固定为：

```text
{
  platform,
  recordKind,
  subjectKey,
  observedAt,
  sourcePayload: <RawRecord.payload 的结构化副本>
}
```

证据：`xhs-derived-contract.ts:405-432`。因此 note 的标题/正文/来源链接只能候选读取 `payload.sourcePayload.title/content/url`；comment 文本是 `payload.sourcePayload.text`；author 当前 fixture 只证明 `payload.sourcePayload.name/profileUrl`。`payload.title`、`payload.content` 等路径不存在。

## 4. 逐模型字段来源

### 4.1 ContentObservation

| 字段 | 已证明来源 | 状态 |
|---|---|---|
| `workspaceId`、`canonicalObservationId`、`rawSnapshotId`、`rawRecordId`、`observedAt`、`fieldPresence`、`qualityStatus` | `CanonicalObservation` 同名/关系字段 | VERIFIED_DIRECT |
| `adapterVersion` | `CanonicalObservation.normalizationRunId → NormalizationRun.adapterVersion` | VERIFIED_DERIVED |
| `title` | note `payload.sourcePayload.title` | VERIFIED_OPTIONAL_SOURCE |
| `bodyText` | note `payload.sourcePayload.content` | VERIFIED_OPTIONAL_SOURCE |
| `publishedAt` | 六份 fixture/当前 note adapter 无该字段 | SOURCE_MISSING；不得宣称已映射 |
| `contentType` | freeze 只给可空字段；值域/映射未定义 | DECISION_REQUIRED（DR-B3-002） |
| `contentAssetId` | 需把 Canonical note 身份绑定到 `ContentAsset` | DECISION_REQUIRED（DR-B3-001） |
| `authorId` | note fixture 不含可证明的 Author 关系 | SOURCE_MISSING（DR-B3-001/002） |
| `id`、`createdAt` 及所有字段类型/default | freeze 仅字段速记 | SOURCE_INCOMPLETE；不得自行补 `cuid()`/`now()` |

唯一键 `(workspaceId,id)`、`(workspaceId,contentAssetId,canonicalObservationId)` 与 Canonical 三列 FK 已有文字来源，但不足以生成完整 Prisma/DDL。

### 4.2 ContentCurrentProjection

冻结字段只有：`contentAssetId`、`workspaceId`、`currentObservationId`、`title?`、`bodyText?`、`contentType?`、`publishedAt?`、`authorId?`、`lastObservedAt`、`projectionVersion @default(1)`、`lifecycleState?`、`visibilityState @default("visible")`、`updatedAt`。

- `title/bodyText/contentType/publishedAt/authorId` 候选复制同一 `ContentObservation`。
- `lastObservedAt` 候选来自该 Observation 的 `observedAt`。
- `currentObservationId` 的同 workspace FK、`projectionVersion` 推进规则、`lifecycleState` 值域/转换、default_app 与 canonical_writer 防覆盖规则均未完成来源化。
- **不存在** `coverImageUrl`、`openOriginalUrl` 或 `revision` 字段；不得在 schema 中补造。

### 4.3 AuthorObservation

冻结规范只规定从现有 `AuthorSnapshot` 演进：`workspaceId` 改为非空；新增 `canonicalObservationId/rawSnapshotId/rawRecordId/observedAt/adapterVersion/fieldPresence/qualityStatus`；`userGroup/userNote/userTags` 移回 `Author`；删除 `rawData`；Release-C rename。现有 AuthorSnapshot 仍包含 `id/authorId/authorEntityId/platformAuthorId/platform/name/description/fans/follows/interactions/ipLocation/profileUrl/handle/keywords/collectedAt/createdAt` 等字段。

author fixture 当前只证明 `authorId/platformAuthorId/name/profileUrl`，且 Canonical 业务字段仍位于 `payload.sourcePayload`。`authorEntityId`、数值指标、简介、地域、handle、keywords 的首期来源/缺失语义，以及完整 PK/unique/FK/删除行为均未定义，不能用“结构同 Content”替代逐字段合同。

### 4.4 AuthorCurrentProjection

冻结字段为 `authorId`、`workspaceId`、`currentObservationId`、`name?`、`fans?`、`follows?`、`interactions?`、`ipLocation?`、`profileUrl?`、`handle?`、`lastObservedAt`、`projectionVersion @default(1)`、`visibilityState @default("visible")`、`updatedAt`。同 workspace FK、版本推进和 quarantine 防覆盖尚未定义。

### 4.5 Comment 与 Metric

- Comment 的 V2 Canonical 身份是 `(noteId,commentId)`，但现役唯一键不含 `noteSourceId`；冻结规范又只给一个可空 `canonicalObservationId`，无同 workspace/snapshot FK。BLK-008 保持 OPEN。
- 六份首期合同不产生 metric RawRecord。Metric 不在本切片实施；BLK-009 保持 OPEN，不能用历史 V1 metric 写入代替 V2 同源证明。

## 5. 现役 V1 写入者盘点（不是 B3 的来源授权）

| 对象 | 已核实的生产写入入口 | 关键行为/风险 |
|---|---|---|
| `ContentAsset` | `content-ingestion-service.ts:158-190`；`data-foundation-content-asset-service.ts:260-282` | 两条路径均按现役键 update/upsert；B3 不得偷偷复用为 fallback |
| `Author` / `AuthorSnapshot` | `author-service.ts:99-122,145-226`；`data-foundation-governance-maintenance-service.ts:696-730` | 主服务 create/update 后建 snapshot；治理维护可直接 upsert Author，写入者不唯一 |
| `Comment` | `manual-collection-writeback-service.ts:181-221`；`comment-service.ts:353-385` | 现役按 `(workspace,platform,sourceId)` upsert/create；另有清洗、生命周期与 voice 服务更新非身份字段 |
| `ContentMetricSnapshot` | `data-foundation-metric-snapshot-service.ts:272-290`；`outbox-worker/content-metrics.ts:259-280` | createMany 与 upsert 并存；均为 V1 路径 |
| `ContentMetricLatest` | `content-metric-latest-service.ts:214-292`，由 `outbox-worker/content-metrics.ts:239-257` 调用 | raw SQL UPSERT、按 observedAt 推进；不是 B3 Canonical writer |
| `ContentMediaUsage` | `media-library-write-service.ts:385-417,950-984` | 现役 Media Domain 建有效关系；不得从 Artifact 直接伪造关系 |
| `AuthorMedia` | `author-media-observation-service.ts:64-99` | 替换头像时关闭旧关系并追加新关系；共享 MediaItem 不删除 |

该盘点证明当前仍是多条 V1 writer，不能据此推导 B3 的唯一 writer。B3 暗态代码不得接这些 caller，也不得双写。

## 6. Media、封面与“打开原文”

### 已确认规则（不是新决策）

- DEC-B1-014：平台明确封面优先；否则同一已接受观察的首张已证明图片；否则不可用。
- DEC-B1-010：打开原文只能是 Projection 输出的受控来源动作，页面不得从 raw、旧 `sourceUrl` 或 Media URL 自行拼接。
- DEC-B1-006：撤销业务媒体关系，不删除共享 Media Asset。

### 尚缺来源

`media_inventory` 是 `CaptureArtifact`，不是 RawRecord。`CanonicalMediaSlot` 虽已有完整物理合同，但当前没有 schema/writer；其七个字段也没有 `mediaItemId` 或来源引用，无法从现有文本唯一推出 Artifact 条目如何绑定 `MediaItem/MediaOrigin/ContentMediaUsage/AuthorMedia`、如何证明“同一已接受观察”、以及 rejected 时关闭哪些有效关系。这里是 DR-B3-004，不是“创建 coverImageUrl 字段”。

同理，note 的 `payload.sourcePayload.url` 只能作为受控来源动作的候选输入；Projection action 的数据结构、校验及失效规则未定义。它不是 `ContentCurrentProjection.openOriginalUrl` 字段。

## 7. CEC 推进、Projection 与撤销事务

当前 B2 暗态实现 `advanceEvaluationCurrent()` 在 `b2-derived-service.ts:643-678` 内直接创建/推进 `ContractEvaluationCurrent`。冻结规范同时要求：

- B3 accepted 路径经 CEC 读取后在事务内写 Observation + CurrentProjection（freeze `:581-582`）；
- accepted→rejected 时，推进 CEC、quarantine Content/Author Projection、删除 PresentationRequirement 必须处于同一 canonical_writer 事务（freeze `:584-591`）；
- `default_app` 可正常推进 CurrentProjection，而 `canonical_writer` 只改 visibilityState（freeze `:623-625,634`）。

冻结边界不是待选择项：B2 继续是 CEC 唯一 owner，且 B2 不创建领域投影；B3 只消费已经 accepted 的 CEC 创建/推进正常投影。唯一需要补入 B2 同一事务的是 accepted→rejected 时的 quarantine/requirement 失效，待媒体关系合同明确后还须按 DEC-B1-006 同事务撤销相应业务媒体关系。现状尚未实现这些动作，并且 default_app 可能把 quarantined 写回 visible。BLK-012/013/014 保持 OPEN；不能另起最终一致任务、补偿任务或 fallback。

## 8. 更正后的 DECISION_REQUIRED 与强制前置工单

### DR-B3-001 — XHS 领域身份与实体建立

- 非技术问题：同一篇笔记/同一作者/同一评论，系统究竟用哪个平台编号认定“这是同一个对象”，对象不存在时是否在同一投影事务创建？
- 已知事实：B2 对 note/author 使用候选优先级；现役 ContentAsset/Author 使用平台键；Comment 现役唯一键与 B2 组合身份不一致；正 fixture 只覆盖候选值相等。
- 方案 A（推荐）：把 V2 合同收紧为 note 必须同时提供且满足 `noteId == platformContentId`、author 必须同时提供且满足 `authorId == platformAuthorId`；不相等即拒绝/隔离，不做 fallback。B3 按平台唯一键在同一投影事务确保 ContentAsset/Author 存在；Comment 扩为 `(workspaceId,platform,noteSourceId,sourceId)` 唯一。`authorEntityId` 固定采用现役 `buildAuthorCode({platform,platformAuthorId})`（`content-identity.ts:81-87`）的确定性结果，禁止客户端自报覆盖。
- 方案 B：B3 只允许绑定已存在的 ContentAsset/Author；缺失即拒绝并进入人工审核，不自动创建。Comment 仍必须修正组合唯一键。该方案不产生错误合并，但会让新的合规 Evidence 无法自动进入 Projection。
- 可证伪验收：note/author 两身份相等正例成功；任一不等负例在写库前拒绝；两条相同 commentId、不同 noteId 可共存；跨 workspace 关系被数据库拒绝；重复输入不新增第二实体。

### DR-B3-002 — XHS note 类型来源与 Domain projector 字段合同

- 非技术问题：哪些 Canonical 字段可以进入内容/作者/评论投影，缺失时是留空还是拒绝？
- 已知事实：业务正文位于 `payload.sourcePayload`；fixture 的 note 只有 title/content/url，comment 只有 text，author 只证明 name/profileUrl。现役插件 noteCollector 可产生 `type`（normal/video 等），但 V2 合同、fixture、B2 fieldPresence 尚未登记它；freeze 没给 ContentObservation/AuthorObservation 完整类型与缺失规则。把所有 note 写成 `contentType="note"` 会丢失现役视频语义。
- 方案 A（推荐）：先把现役 note `type/contentType` 的单一来源、值域和 normal/video 映射收进插件 V2 合同与六 fixture，再由版本化、recordKind 分支的严格 projector 读取。note 只写 ContentObservation，comment 只写 Comment，author 只写 AuthorObservation；未出现的可选字段保持 null 并由 fieldPresence 解释，必需身份/类型缺失则拒绝。
- 方案 B：`ContentObservation.contentType` 保持 null，且因为现役 `ContentAsset.contentType` 非空，B3 只绑定已存在的 ContentAsset、不创建新 ContentAsset；其他可选字段仍按严格 projector 映射。该方案不会猜类型，但限制新内容自动投影。
- 可证伪验收：六 fixture 覆盖 normal/video 与每种 recordKind；`payload.title` 等错误路径测试失败；未知类型拒绝；删除/改名任一已登记字段时产生确定的 null 或拒绝；未知字段不进入投影。

### REQUIRED-B3-001 — CEC 与撤销的冻结事务边界（不是用户选择）

- 冻结结果：B2 仍为 CEC 唯一 owner，accepted 时 B2 提交 CEC 后由 B3 消费；B2 不创建领域投影。若新 rejected 替换旧 accepted，则 B2 在推进 CEC 的同一 canonical_writer 事务中 quarantine 已有 Content/Author CurrentProjection 并令对应 PresentationRequirement 失效；default_app 不得把 quarantined 直接恢复为 visible。
- 可证伪验收：accepted 的 B2 事务不创建领域投影；B3 失败不回滚既有 accepted CEC；rejected 故障注入在 CEC/quarantine/requirement 任一点均整笔回滚；并发切换只有一个 revision 成功；default_app 直接恢复 visible 被数据库拒绝。

### SOURCE-AUDIT-B3-001 — CanonicalMediaSlot 到现有 Media Domain（不是架构选择）

- 非技术问题：采集包里的图片清单如何变成“这次已接受观察确实拥有的封面/图片”，并在拒绝时只撤销关系、不误删共享文件？
- 已知事实：CanonicalMediaSlot 物理字段已冻结但没有媒体身份 FK；Artifact→Slot→MediaItem/关系以及受控打开原文 action 结构均未定义。
- 强制工单：先完成独立 B2/B3 media source audit，只使用现有 Media Domain，逐字段确定 Artifact 条目身份、CanonicalMediaSlot 与 MediaItem/业务关系的键、接受/撤销事务和来源 action。审计确认前，B3 可做非媒体模型准备，但不得生成封面或原文 action。
- 可证伪验收：同观察平台封面优先、无平台封面取首张已证明图片、无候选为 unavailable；跨观察/跨内容图片拒绝；rejected 关闭关系但 MediaItem 仍在；页面不存在 raw/旧 URL/media delivery URL fallback。

### DR-B3-003 — `ProjectionReceipt` 未定义名词的规范修正

- 非技术问题：B3 每次正常投影是否需要另存一张“投影处理回执”，还是只依靠 Observation/CurrentProjection 的唯一键证明完成？
- 已知事实：freeze `:582` 只出现一次 `ProjectionReceipt` 名称，没有模型、字段、键或权限；§5.14 定义的是 B6 `PresentationReceipt`；DEC-B1-005 只限制展示回执，不能自动证明两者是同一个东西。
- 方案 A（推荐）：权威规范明确删除 B3 `ProjectionReceipt` 名称；B3 以不可变 Observation 唯一键、CurrentProjection 指针/version 和事务结果证明完成。B6 `PresentationReceipt` 保持不变。
- 当前没有第二个可直接实施的合规方案：若业务确实需要独立 B3 回执，必须另做字段/键/写入者/权限/幂等来源审计后再形成新选项，不能从名称补造模型。
- 可证伪验收：权威规范、模型合同、B3 工单不再混用 Projection/Presentation Receipt；重放同 CanonicalObservation 不新增第二 Observation；CurrentProjection 只按单调 version 推进。

## 9. 不再作为用户决策的事项

- **封面优先级**：DEC-B1-014 已决定，只缺媒体关系实施合同。
- **打开原文是否保留**：DEC-B1-010 已决定，只缺受控 action 数据合同。
- **普通页面是否需要 PresentationReceipt**：DEC-B1-005 已决定为不需要；已定义的 `PresentationReceipt` 仅属于 B6 强一致页面。未定义的 `ProjectionReceipt` 是否另有含义仍按 DR-B3-003 处理，不得自行等同。
- **是否首期实现 metric**：六 XHS 合同没有 metric RawRecord，本切片不实施；不是允许伪造来源的选择题。

## 10. 推进结论

1. **暂不进入 B3 schema/code**：ContentObservation 与 AuthorObservation 尚不能无猜测转录；身份与事务未闭环。
2. 用户确认 DR-B3-001～003 的推荐方案后，先执行两个窄工单：
   - B3-A-02：完整 Domain model contract，并把 REQUIRED-B3-001 的冻结事务边界转成可执行合同；
   - B3-A-03：Media source audit（Artifact→CanonicalMediaSlot→现有 Media Domain + 受控 action）。
3. 两个审计都通过后，才发 B3-B-01 schema expand；仍保持暗态、无 caller、无双写/双读/fallback。

## 11. 验证与治理实况

| 验证 | 结果 |
|---|---|
| `git rev-parse HEAD` | `424e180f3ef3de217fde4492bf2519e9509d0b63` |
| `npx tsx src/lib/evidence/contracts/cross-repo-verify.ts /Users/gongyong/Services/linggan-boom` | exit 0；6/6 contract、fixture、normalization、evaluation 跨仓一致 |
| `node scripts/check-project-governance.mjs --json` | exit 1；Markdown 数量已同步为 350；仅余固定点既有 `TODO.md` 229 行与 `execution-sync-service.ts` 2022 行两项债务 |
| `git diff --check` + 新文档尾随空白扫描 | exit 0 / 零命中 |

## 12. 未做事项

- 未修改 schema/migration/src/tests/package.json。
- 未写任何数据库，未触碰正式测试库。
- 未接 caller、未双写/双读/fallback。
- 未 push、未部署、未合并 main。
