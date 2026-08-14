# B3-A-03｜Media Source Audit、SRC-001 与 SRC-002 observed 基础收口

> 日期：2026-08-11
>
> 主库实施固定点：`38721ece5de80c781a4e059d67beda1388af64a6`
>
> 插件库固定点：`227e365192d2141407599fefbf066aa66bfa0b26`
> 结论：**B3-MEDIA-SRC-001、SRC-002 与 SRC-003 的 `observed` 暗态基础已完成；仍无 caller、Projection 或展示流量。** SRC-003 只复用现有 MediaItem/MediaOrigin/处理 Outbox 物理边界，并用五字段回执绑定 Canonical observation 与 slot；`absent/unavailable` 和 `live_photo` 分别受 DR-B3-005、DR-B3-006 阻塞并保持 fail-closed。

## 1. 范围与判定规则

本文件先记录 SRC-001 的只读来源审计与暗态来源合同，后续章节再记录 SRC-002 `observed` 基础的 schema/migration、reader/writer 与隔离库实施结果。冻结规范与现役 caller 始终未修改，正式测试库和生产数据库始终未触碰：

```text
XHS plugin media_inventory
  → CapturePackage / CaptureArtifact
  → B2 audited package read
  → CanonicalMediaSlot
  → existing Media Domain
  → Projection cover / open-original action
```

采用的已确认规则：

1. V2 媒体必须可追溯到 Evidence（`01-decisions.md:24`，DEC-B1-003）。
2. 拒绝或撤销时关闭业务媒体关系，保留共享 Media Asset，且撤销可审计（`:27`，DEC-B1-006；`06-v2-operating-contract.md:59-63`）。
3. “打开原文”只能是 Projection 输出的受控来源动作，不是媒体交付 URL，也不得由页面拼接（`01-decisions.md:31`，DEC-B1-010；`06-v2-operating-contract.md:34-41`）。
4. 复用现有 Media Domain 与本机存储，不创建平行媒体平台；首期只做 XHS（`01-decisions.md:32-33`，DEC-B1-011/012；`06-v2-operating-contract.md:36-37`）。
5. 封面顺序固定为“平台明确封面 → 同一内容、同一已认可观察中的首张已证明图片 → 不可用”，不是页面 fallback（`01-decisions.md:35`，DEC-B1-014；`06-v2-operating-contract.md:40-41`）。

文中的 `SOURCE_INCOMPLETE` 表示上述选择已经确定，但现有真实来源不足以唯一、可验证地实施；它不是新的架构选择。

## 2. 插件 `media_inventory`：fixture 与真实 terminal mapper

### 2.1 逐字段事实矩阵

| 字段/语义 | 固定 fixture | 真实 terminal 来源 | 审计结论 |
|---|---|---|---|
| Artifact 外壳 | `kind=media_inventory`、`encoding=base64`、payload/checksum/length/restricted（`xhs-contracts.cjs:231-247`） | 同一 helper 生成外壳（`:257-271`） | 外壳一致 |
| `candidates` | `{kind,url,width,height}`（`:232-236`） | `resultPackager.records.mediaAssets` 的原始对象（`xhs-terminal-mapper.cjs:260-271,311-314`） | **结构不一致** |
| 真实记录标识 | fixture 无 | 至少含 `assetId`、`contentId`、`collectionRunId`（`noteMediaDownload.js:248-252`） | fixture 不能证明真实映射 |
| 媒体类型/角色 | fixture 只有 `kind=image`，没有 cover/body 角色或 ordinal | `assetType`、`role`（`:243-253`） | 无统一值域合同 |
| 地址 | fixture 用 `url` | 真实记录用 `sourceUrl`、`candidateUrls`（`:258-264`） | URL 字段名与含义不一致；地址不得充当身份 |
| 尺寸 | fixture 有 `width/height` | 当前真实记录构造未保证尺寸 | 不能把 fixture 尺寸当现役来源事实 |
| 顺序 | fixture 明示 `ordinal` | `noteCollector.js` 从平台 `imageList` 序位形成并持久保留 `imageCandidateSlots.ordinal`，Live 只接受媒体自身 `imageIndex`；`noteMediaDownload.js` 透传该值，来源缺失时保留 null | 已完成来源合同；缺失 ordinal 在 V2 拒绝，下载队列/候选 URL 重排不改变 slot |
| candidate 校验 | helper 只检查输入是数组（`xhs-contracts.cjs:257-261`） | mapper 只检查 `mediaAssets` 是数组（`xhs-terminal-mapper.cjs:249-253`） | candidate 内部字段、值域和归属均未 fail-closed |
| `mediaPolicy` | 四个合同为 `metadata_only`、两个为 `not_required`（`xhs-contracts.cjs:314-400`） | 真实 mapper 只要观测到 mediaAssets 就发 Artifact（`xhs-terminal-mapper.cjs:311-314`） | policy 不是 candidate 状态，也不能替代事实合同 |

真实数据从插件 `mediaAssetStore` 按 `collectionRunId` 读取并原样进入 mapper；这证明它不是 fixture，但不证明其候选结构适合 Canonical。

另一个必须显式保留的现状是：`xhs-terminal-mapper.cjs:1-8` 声明 mapper 还没有 runtime caller；插件仓库的实际引用目前只有定义和测试。工作台跨仓校验 `cross-repo-verify.ts:33,61-77` 只加载固定 `FIXTURES`，不会执行真实 terminal mapper。因此 fixture 验证通过不能替代 runtime candidate 证明，后续切流门禁必须加入真实 mapper 输出。

### 2.2 平台明确封面无法由现有 `role=cover` 证明

`getNoteCoverCandidates` 同时收集平台封面字段和第一组/第一张图片（`noteMediaDownload.js:70-78`）。随后队列把这一组统一标为 `type=cover`（`:154-163`），持久化又统一变成 `assetType=image, role=cover`（`:243-253`）。因此当前 `role=cover` 可能只是首图 fallback，不能证明 DEC-B1-014 的第一顺位“平台明确封面”。

此外，选择 images 时 `includeCover` 被关闭（`:150-177`），第一张图片成为 `image-1/body`；同一来源事实会因插件下载选项产生不同角色。B3 不得从 `role`、`assetId` 后缀、数组位置或 URL 反推平台封面。

### 2.3 作者媒体

现有 fixture 的 author 只含身份、名称和 profile URL（`xhs-contracts.cjs:300-307`）；当前 B2 XHS author adapter 的字段存在性也未包含头像。故本切片没有可审计的作者头像 Artifact 来源。不得从 profile URL 猜头像，也不得在没有单独来源合同的情况下写 `AuthorMedia`。

## 3. CaptureArtifact 的持久化与读取

| 环节 | 真实行为 | 结论 |
|---|---|---|
| 外部提交类型 | Artifact 含 kind/encoding/base64 payload/checksum/contentLength/restricted（`src/lib/evidence/ingress/types.ts:122-145`） | 完整字节进入 CapturePackage |
| ingress 完整性 | 解码 base64，核对长度、checksum、restricted 传播（`validator.ts:269-285`） | 只证明字节完整性 |
| 结构校验 | descriptor 外壳被校验；没有解码并校验 `media_inventory.candidates` 的领域结构（`:609-623`） | **SOURCE_INCOMPLETE** |
| Registry 写入 | `CaptureArtifact` 只写 workspace/snapshot/kind/checksum/restricted（`evidence-ingress.ts:272-282`） | 描述符可检索 |
| 数据模型 | `CaptureArtifact` 没有 payload、encoding、contentLength 或 storageKey；注释规定它不是第二 payload store（`schema.prisma:4191-4206`） | candidate 字节只能从已审计 CapturePackage 读取，不能由描述符重建 |

这不要求新增媒体存储平台：当前包字节仍属于 Evidence，后续解析器必须从 EvidenceIngress 已验证且完整性允许使用的 CapturePackage 读取，并用 `CaptureArtifact.artifactChecksum` 做绑定校验。

## 4. B2 是否消费 Artifact

当前 B2 服务读取并验证 CapturePackage 后，只选择包内 records，写入 NormalizationRun/ContractEvaluation；没有读取或写入 `CaptureArtifact`，也没有调用 Media Domain（`b2-derived-service.ts:166-240`）。`b2-source-verification.ts:183-203` 只要求 `artifacts` 是数组，返回值只遍历验证 records。

现役 `raw-snapshot-media-registration-service.ts:127-142` 也不能承接 V2：它筛选 `RawRecord.recordType="note"`，而 EvidenceIngress 的 V2 record 写 `recordKind` 且 `recordType=null`（`evidence-ingress.ts:250-264`）；它解析 note payload，并不消费 `media_inventory`。可复用的边界是更下层现有 Media Domain writer，不是这条 V1 adapter。

SRC-001 审计时结论：CapturePackage 中即使有真实 media Artifact，当时 B2/B3 链路仍为零消费，尚未形成 Media 追溯链。SRC-002 随后新增了仅位于无 caller 暗态模块中的受控消费，最终现状见 §9～§10。

## 5. `CanonicalMediaSlot` 七字段

冻结规范的物理模型完整且唯一（`content-workbench-v2-design-freeze.md:384-399`）：

| 字段 | 类型/约束 | 已确认语义 | 当前缺口 |
|---|---|---|---|
| `id` | String，cuid，主键 | 槽记录身份 | 无 |
| `workspaceId` | String | 租户边界 | 无 |
| `canonicalObservationId` | String，复合 FK | 只属于一个 CanonicalObservation | 需要 Artifact candidate 到该 observation 的验证绑定；不以 CEC accepted 作为建槽条件 |
| `slotId` | String；与 observation 复合唯一 | observation 内稳定槽标识 | 插件没有统一 slotId 来源合同 |
| `status` | `observed | absent | unavailable` | 槽的 Canonical 状态 | Artifact 缺席、合同不要求、采集失败三者尚无映射规则 |
| `kind` | String | 媒体种类 | fixture `kind` 与真实 `assetType` 未统一值域 |
| `ordinal` | Int | 同一观察内确定顺序 | 真实数组/assetId 不能证明语义 ordinal |

模型另有 `(workspaceId,canonicalObservationId,slotId)` 和 `(workspaceId,id)` 唯一约束、到 CanonicalObservation 的复合 FK，并要求不可变。当前 WIP 已按冻结合同新增 `CanonicalMediaSlot` model、复合 FK/唯一约束与 append-only trigger；`observed` 的 `slotId/kind/ordinal` 只来自已验证 media inventory，另外两种状态在 DR-B3-005 关闭前不得赋值。

槽到 Media Domain 的物理处理回执也已有唯一验收形态，不需要给七字段补造 Media FK：freeze `:878-899` 要求每个 observed slot 对应一个 `media.processing_requested` OutboxEvent，且 `ledgerOrigin` 同时精确包含 `canonicalObservationId`、`slotId`、`originId`、`mediaItemId`、`generation`，并反查同工作区 MediaOrigin→MediaItem。该 Canonical 建槽/物理处理层不以 CEC accepted 为前提；accepted 只门禁 ContentMediaUsage/AuthorMedia 等业务关系的激活以及封面/action 展示。缺的是现役 writer 对这两层合同的实现。

## 6. 现有 Media Domain 的能力与缺口

### 6.1 可复用模型

| 模型 | 当前职责 | B3 可复用边界 |
|---|---|---|
| `MediaItem` | 媒体逻辑身份/lifecycle（`schema.prisma:3665-3683`） | 继续作为唯一媒体资产入口 |
| `MediaOrigin` | provider/stableLocator/fetchUrl/generation（`:3757-3780`） | URL 只可作为可变取回地址；稳定身份必须由 XHS 内容身份 + Canonical slot 导出 |
| `MediaMaterialization` | origin generation → blob 的处理状态（`:3782-3801`） | 继续走现有物化链 |
| `MediaBlob`/`MediaReplica` | 内容寻址字节与本机/副本交付（`:3624-3663`） | 不新增存储平台 |
| `ContentMediaUsage` | contentAsset ↔ mediaItem 的 purpose/ordinal/sourceRevision/有效期（`:3803-3822`） | 用有效期表达内容媒体关系；必须补齐 accepted observation 的可审计绑定 |
| `AuthorMedia` | author ↔ mediaItem 的 avatar 关系与有效期（`:3730-3755`） | 仅在将来有独立、已验证头像来源后使用 |

`media-library-identity.ts:1-8,90-116` 已明确 URL 不是稳定身份，且现有 locator 能从平台业务身份和槽位生成；这部分方向符合边界。`registerSourceMediaIdentitiesInTransaction` 能在一个事务中登记 Item/Origin/ContentMediaUsage 和处理 outbox，可作为 B3 adapter 的下层 writer。

### 6.2 写入与撤销审计

- 现有内容媒体 writer 在发现同一 active slot 已指向不同 MediaItem 时跳过/冲突，不会关闭旧关系（`media-library-write-service.ts:295-429`）。
- SRC-003 实施前，`ContentMediaUsage.sourceRevision` 可空且没有 CanonicalObservation FK，既有通用处理入口的 `ledgerOrigin` 也只有 originId/mediaItemId/generation，因此旧写入不能单独证明 DEC-B1-003（`media-library.ts:620-649`；freeze `:878-899`）。SRC-003 现已在同一 Outbox 类型上增加独立的 Canonical 严格入口，运行时强制 canonicalObservationId/slotId/originId/mediaItemId/generation 五字段；旧生产者仍使用原三字段入口，不得把两者混读或 fallback。
- 作者头像服务会关闭前一条 `AuthorMedia.validTo` 并创建新关系（`author-media-observation-service.ts:30-122`），但这是作者观察替换，不是 V2 rejected Canonical 的撤销证明。
- 当前生产代码没有关闭 `ContentMediaUsage` 的拒绝/撤销路径；`BLK-012` 仍在 blocker ledger 中开放（`04-blocker-ledger.md:172-182`）。共享 `MediaItem` 不应被删除。

因此需要的是在既有 Media Domain 上补充 Canonical 来源回执和关系撤销协调，不是新建媒体平台、复制 Blob，或以 URL/旧表承接。

## 7. 封面两级规则

现有读层 `media-library.ts:487-500,835-861` 的机械顺序是 active `source_cover` 优先、否则 `source_image ordinal=0`，且没有制造假关系。这可以复用为中央选择器的内部机制，但**不能原样作为 V2 证明**：

1. 插件 `role=cover` 不等于平台明确封面（见 2.2）。
2. `ContentMediaUsage` 没有强制绑定一个 accepted CanonicalObservation，现有读层也未按同一 accepted observation 过滤。
3. B3-MEDIA-SRC-001 已要求 producer 明示并验证图片 ordinal；不得从数组顺序、assetId 或 URL 推断。

在这些条件补齐前，Projection 必须输出明确 `unavailable`，不得退回旧 `sourceUrl`、Raw payload、跨 observation 的首图或页面自选首图。

## 8. “打开原文”受控 action

Canonical note 的 B2 输出已保留来源身份和 source payload（`xhs-derived-contract.ts:405-468`），但仓库中尚无 V2 Projection 的 `open-original` action model/type/writer。现有 `xhs-execution-target-url.ts:37-50,90-142` 会在多个候选间选择并可拼 XHS detail URL，带有执行期语义；它不能原样成为 Projection 合同。

窄合同至少必须证明：

- action 版本、平台（首期仅 XHS）、accepted `canonicalObservationId` 与业务 subject 绑定；
- 来源身份经 XHS allowlist/格式校验后由受控 resolver 生成动作目标；
- Canonical 被拒绝、替换或隔离时，action 与 Projection 同步失效；
- action 与 MediaOrigin.fetchUrl、MediaReplica.deliveryHandle 明确分离；页面只消费 Projection action，不自行拼接或 fallback。

这些是 DEC-B1-010 的实施前置，不是授权读取旧 URL。

## 9. 缺口账本与下一步窄工单

SRC-002 实施复核发现现有来源不能唯一推出 `absent/unavailable`。用户已确认 DR-B3-005 方向 A：由版本化 producer media contract 显式提交 slot status 与封闭 reason；但真实 producer 审计证明空 imageList/cover 目前不能区分平台无媒体与加载失败，所以实施仍为 **SOURCE_INCOMPLETE**。在 producer 事实、跨仓合同和 fixture/hash 闭环前只落 `observed`；从 CollectionContract/terminal/数组缺失推断继续禁止。以下按真实状态管理：

| ID | 前置工单 | 可证伪验收条件 |
|---|---|---|
| B3-MEDIA-SRC-001 | 统一 XHS `media_inventory/v2` candidate 合同与 source-shaped fixture | fixture 与真实 mapper 通过同一严格 validator；candidate 首期只接受同包 note subject，slot 精确为 `subjectKey:purpose:kind:ordinal`，并明示 observed address 及“平台明确封面”的独立 provenance；comment media、author/avatar、未知键/值域/跨 subject/重复或公式不匹配 slot 被拒绝；URL 不参与 stable identity |
| B3-MEDIA-SRC-002 | observed 基础已实施；状态合同方向已确认但来源未闭环 | 只从完整性可用的 CapturePackage 与注入审计 source 读取并核对真实 Artifact descriptor；不可伪造能力按同一 CanonicalObservation subject 生成七字段 observed slot，不以 CEC accepted 为建槽条件；同包多 note 分别建槽。`absent/unavailable` 在 producer 来源闭环前不写入；零 Projection 流量 |
| B3-MEDIA-SRC-003 | observed 暗态衔接已实施 | 只调用既有 Media writer 的物理 Item/Origin/processing 边界；stableLocator 不含 URL；每个 observed slot 的 `media.processing_requested.ledgerOrigin` 精确含 canonicalObservationId/slotId/originId/mediaItemId/generation，并通过 freeze `:878-899` 的同工作区反查；跨 workspace、歧义、迟到来源、入队故障与 live_photo 整批零推进；未激活 ContentMediaUsage/AuthorMedia 或展示，未创建平行模型/存储 |
| B3-MEDIA-SRC-004 | accepted observation 业务关系激活、替换/拒绝协调与 Projection 合同 | 只有 current accepted CEC 可激活 ContentMediaUsage/AuthorMedia、封面与 action；按 `02-model-contract.md:534-539` 的同一 canonical_writer CEC 事务关闭旧 Projection 及对应 active 业务媒体关系并留审计，不删除共享 MediaItem；封面严格执行两级规则；打开原文只由受控 action 输出；无旧 URL/Raw/page fallback |

顺序固定为 `SRC-001 → SRC-002 → SRC-003 → SRC-004`。前三步的 observed 暗态基础已经实现，但这不构成 caller、业务关系或 Projection 授权；`absent/unavailable` 合同、live_photo 物理语义与 BLK-015 仍未关闭。

### B3-MEDIA-SRC-001 收口（2026-08-11）

`xhs.media-inventory/v2` 已在插件与工作台分别实现并由 canonical 跨仓比较锁定。candidate 精确包含 `subject`、不含 URL 的稳定 `slotId`、`purpose`、`kind`、producer 从平台图片序位或媒体自身序位持久保留的非负 `ordinal`、`observedAddress` 与 `coverProvenance`；`slotId` 必须精确等于 `subjectKey:purpose:kind:ordinal`。封面 provenance 仅允许 `platform_explicit` 或 `first_observed_image`，普通候选必须为 `not_cover`。真实 terminal mapper 只接受同一批 emitted record 中存在的 note subject；comment media 与 author/avatar 在真实 producer/Artifact 来源闭环前拒绝，同时拒绝未知键/值域、补造 ordinal、跨 subject、重复或公式不匹配的 slot 与无明确 cover proof。fixture 由同一严格 validator 接受。六个 CollectionContract 已升级为 v2 并绑定 record/media 来源合同 hash，旧 v1 header 不可声明新来源规则。SRC-001 当时没有创建 CanonicalMediaSlot/Media writer，也没有以 CEC accepted 作为建槽条件；其后的 SRC-002 `observed` 基础实施结果见下节。

### B3-MEDIA-SRC-002 observed 基础收口（2026-08-11）

工作台已新增冻结七字段 `CanonicalMediaSlot`、同 workspace Observation FK/唯一键与 SQLSTATE 55000 append-only trigger；reader 不接受 caller 自报 checksum、descriptor 或 RawRecord，而只接受注入审计 source 的同源 package/snapshot/artifact/record 回执，并签发 WeakMap 绑定的不可伪造能力。writer 在 SERIALIZABLE 事务中按 Observation subject 选择同包 candidates；同包两篇 note 各写各自 slot，完全相同 replay，并发收敛，字段漂移、伪造能力、跨 workspace/snapshot/subject 与事务故障均零推进。生产没有默认数据库 reader、caller、Media Domain 或 Projection 写入。`observed` 已闭环；`absent/unavailable` 保持 DR-B3-005。

### B3-MEDIA-SRC-003 observed Media Domain 暗态衔接（2026-08-11）

Canonical adapter 只接受 SRC-002 签发的不可伪造能力与已绑定的 CanonicalObservation；在同一 SERIALIZABLE 事务内复核 snapshot/subject/七字段 slot，复用既有 `planSourceMediaIdentities`、`observeMediaOriginInTransaction` 和 `media.processing_requested` Outbox。队列边界运行时强制五字段 ledger origin，所有状态共用不可变幂等键；同地址 cover+image:0 共享物理 MediaItem/Origin 但各自保留 slot 事件。身份计划拒绝、跨 workspace、伪造能力、槽漂移、迟到旧地址、Outbox 故障和任一 live_photo 均在事务内 fail-closed。生产仍无 adapter caller，且 ContentMediaUsage、AuthorMedia、PublicationMedia、TopicMediaSelection 与 B3 Projection 表均未推进。

## 10. 验证记录

| 验证 | 结果 |
|---|---|
| 主库实施固定点 | `git rev-parse HEAD` = `38721ece5de80c781a4e059d67beda1388af64a6` |
| 插件固定点/来源合同提交 | `git -C /Users/gongyong/Services/linggan-boom rev-parse HEAD` = `c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`；SRC-003 未修改插件，未 push/发布 |
| 插件真实合同/mapper 测试 | `npm run check:contracts`：exit 0；其中 Node 合同测试 175/175、8 suites，且 tsc `checkJs` 通过 |
| 插件全量与构建 | `npm run test:douyin`：258/258；`npm run build`：exit 0，仅 3 项既有 bundle size warning |
| 跨仓固定合同校验 | `npx tsx src/lib/evidence/contracts/cross-repo-verify.ts /Users/gongyong/Services/linggan-boom`：exit 0，6/6 contracts + record/media source contracts + B2 hashes verified；runtime mapper/sanitizer 另由插件真实链路负例覆盖 |
| 工作台验证 | SRC-002 unit 6/6；隔离库 `content_workbench_b3_media_slot_proof_1786433997009` 19/19（含同包双 note）；最终全量 4603/4603（82 个显式隔离跳过）；`npx tsc --noEmit --incremental false` exit 0；lint 0 error/6 个既有 warning；Webpack 生产构建及 output trace/fingerprint 检查通过 |
| SRC-003 定向与隔离证明 | queue/slot unit 37/37；全新隔离库 `content_workbench_b3_media_domain_proof_1786439765604` 22/22，新增同 URL 不兼容槽 + 另一合法槽的整批零写入证明；最终账本回执 SQL denominator=1、numerator=1、unmatched=0；证明库保留供复核 |
| CanonicalMediaSlot 当前 schema | `rg '^model CanonicalMediaSlot\\b' prisma/schema.prisma`：1 命中；固定点 `38721ece` 为零命中 |
| B2 / 现役消费 | B2 service 仍不消费 Artifact/Media；新增消费只位于无 caller 的 `src/lib/evidence/media/`，全仓 route/service/scheduler/outbox 无实例化 |
| ContentMediaUsage 撤销 writer | 对生产 `src` 检索 `contentMediaUsage.(update|updateMany|delete)`：零命中 |
| 新文件空白检查 | `git diff --no-index --check /dev/null docs/code-review/v2-b3-media-source-audit-2026-08-11.md`：无诊断（命令因存在新增 diff 返回 1） |
| 项目治理检查 | `node scripts/check-project-governance.mjs --json`：exit 1；Markdown/model 快照无漂移，最终只剩固定点既有 TODO 229 行与 large service 2022 行两项债务 |
| 生产构建 | 默认 Turbopack 因本 worktree 的 `node_modules` 软链接越出 filesystem root 失败；按项目既有备用口径执行 `next build --webpack` 成功，275 个 output traces 边界检查零违规，运行指纹 `2529b4d4…176` |
| B3PF-R1 Projection/Usage 复核 | 全新隔离库 `content_workbench_b3_projection_proof_1786465077533` 40/40；最终 CO=35、CCP=32、CMU=18、MI=17、MO=16、OE=18。测试 helper 已用成功 SQL 自证不得误报 SQLSTATE；跨 snapshot 旧/等时 accepted、GUC spoof、直接 revoke 与 Current DELETE 攻击均被数据库事实不变量拒绝，较新 Current 与两条 active Usage 保持字节级不变。B2/B3 使用同一数据库 subject lock；BLK-015 物理调用者角色/GRANT 仍未落地，不据此宣称身份授权。 |
| B3PF-R2 Projection/Usage 复核 | 全新隔离库 `content_workbench_b3_projection_proof_1786467668899` 49/49；最终 CO=46、CCP=40、CMU=22、MI=21、MO=20、OE=22。test-only 数据库 barrier 的无行栅栏 RED 对照库 `content_workbench_b3_projection_proof_1786466909777` 复现 delayed A 错误成功与反向交错无 40001 重试；候选实现用内部 subject fence 强制陈旧 snapshot 序列化失败后整事务重试。双 note accepted 对两边 Current/Usage 零 mutation，且直接调用受控 advance 也以 55000 拒绝；visible Current 的 observed slots 在 COMMIT 时必须逐一匹配唯一 active Usage，direct advance/GUC/duplicate/close/replay gap 均以 55000 拒绝。BLK-015 物理调用者角色/GRANT 仍未落地。 |
| B3PF-R3 late/concurrent Slot 复核 | 最终全新隔离库 `content_workbench_b3_projection_proof_1786497794196` 55/55；最终 CO=50、CCP=44、CMU=23、MI=22、MO=21、OE=23。CanonicalMediaSlot INSERT 从 CanonicalObservation 数据库事实推导 XHS subject，进入同一 fence 与 deferred completeness；direct late slot、真实 standalone writer、两 client write-skew 均不能形成 visible+slot/no Usage。Projection 通过复用 writer 核心的 in-transaction seam 一次提交 Slot/Domain/Usage/Current；无 visible Current 的 standalone dark writer仍合法。case 41/42 各自独立 install/finally restore，单独过滤均通过，注入失败清理自证无 wrapper/sequence 残留。 |

最终治理、跨仓合同校验和工作树状态均按实际结果记录。SRC-002 与 SRC-003 分别在全新证明库 `content_workbench_b3_media_slot_proof_1786433997009` 完成 19/19、`content_workbench_b3_media_domain_proof_1786439765604` 完成 22/22；两库均保留供复核，未触碰正式测试库或生产数据库。

## 11. 明确未做事项

- 未修改 design-freeze、CONTEXT，未新增平行 Media model/resolver/storage；仅新增冻结的 CanonicalMediaSlot schema/migration。
- 未进入 Douyin 来源合同、评论媒体、作者头像推断或 Projection writer；Media Domain 仅完成无 caller 的 observed 暗态物理衔接，未激活业务媒体关系；`absent/unavailable` 与 `live_photo` 未猜测实现。
- 未接现役 caller、route 或 scheduler；暗态 adapter 只在事务内写既有 Media Domain 与既有处理 Outbox 事件，没有运行流量。未设计 fallback/双读/双写，未把 URL 定义为身份。
- 未 push、未部署、未制作插件 release package、未升级工位。

## 12. 实际 diff 与交付状态

- 工作台实际 WIP 在既有 DR-B3-004/来源合同/B2 基础上新增 CanonicalMediaSlot schema/migration、受控 Artifact reader、slot writer、Canonical Media adapter、现有媒体处理队列的五字段 Canonical 边界、单元测试及两座隔离库测试；SRC-003 未修改插件。无 caller、Projection 或业务关系写入。
- 插件最终提交为 `c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`；工作台最终 SHA 由主线提交完成后在交付报告记录。
