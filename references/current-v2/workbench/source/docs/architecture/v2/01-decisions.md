# 01 — 已确认决策登记

> 只登记已由用户确认的决策。未决问题写"未决"。

---

## 已确认决策

| ID | 决策 | 确认来源 |
|---|---|---|
| DEC-001 | 单轨原则：不保留双写/双读/fallback/兼容层 | 用户在 R8-R12 多轮指令中反复确认 |
| DEC-002 | 历史数据物理退役（不回填、不伪造、不生成假包） | 用户明确"宁可放弃旧数据" |
| DEC-E01 | Evidence First：Evidence、Derived 与 Application Projection 必须分层；页面不直读原始采集数据，AI 不得污染原始事实 | 用户《V2 架构决策确认与协作包更新任务》 |
| DEC-E02 | Evidence 不可静默修改；错误以新增证据及新加工结果表达；合规/隐私/清理处置必须留痕与说明 | 同上 |
| DEC-E03 | 同一执行、相同 capture identity 与 payload hash 的重复提交关联既有 Evidence；不同时间重新观察必须新建 Evidence | 同上 |
| DEC-E04 | 真实执行与 Evidence 分离；非执行来源不得伪造 ExecutionJob 或 CaptureAttempt | 同上 |
| DEC-E05 | PostgreSQL Evidence Registry 是唯一事实来源；对象存储可承载大型 payload，但不是第二事实来源 | 同上 |
| DEC-E06 | Evidence 必须可表达质量、置信度与验证状态，供 Derived/AI 判断可信程度 | 同上 |
| DEC-E07 | 证据采集与留存应按对象类型形成策略，不采用无差别无限采集 | 同上 |
| DEC-E08 | `RawSnapshot` 是 V2 一次外部观察的唯一 Evidence Registry；`RawRecord` 仅是该快照内原始片段；既有 `RawEvidence` 不得成为 V2 新数据路径 | 用户于 2026-08-05 明确确认 D-E08 |
| DEC-E09 | 重复提交只以 capture identity + 完整 Capture Package hash 在包级关联既有观察；不同观察新增快照和片段，RawRecord 不跨快照合并/覆盖/移动 | 用户于 2026-08-05 明确确认 D-E09 |
| DEC-B1-001 | `ARCHIVED` Evidence 可继续分析与重算；`REDACTED`、`PURGED` 不可继续使用 | 用户《V2 架构最终收口与 B1 启动前执行指令》 |
| DEC-B1-002 | 同一业务实体的同一观察版本只允许一个 Canonical 业务投影；不同观察版本允许各自投影 | 同上 |
| DEC-B1-003 | V2 新写入的内容、评论、指标和媒体均须可追溯到 Evidence | 同上 |
| DEC-B1-004 | Capture、Evidence、Canonical、Media 状态职责分离；不得把一个成功状态当作另一个完成状态 | 同上 |
| DEC-B1-005 | Projection 是普通页面唯一展示入口；只有发布、审核、强一致运营页面可额外使用展示回执 | 同上 |
| DEC-B1-006 | 撤销业务媒体关系而非级联删除 Media Asset；撤销必须可审计 | 同上 |
| DEC-B1-007 | 数据库层必须保护 Evidence，不允许 AI 或 default 应用身份绕过受控入口修改 Evidence | 同上 |
| DEC-B1-008 | 同 capture identity、不同 hash 的冲突 Evidence 必须保留且不可投影，并纳入生命周期；不得静默覆盖 | 同上 |
| DEC-B1-009 | 只记录观察质量、观察置信度和验证结果；不保存绝对真相评分 | 同上 |
| DEC-B1-010 | 前端保留“打开原文”能力，但它只能作为 Projection 输出的来源动作，不得复用媒体交付 URL 或由页面自行拼接 | 用户于 2026-08-05 确认 |
| DEC-B1-011 | 现有媒体文件继续存放在本机现行存储；本阶段不迁入新数据库或对象存储 | 用户于 2026-08-05 确认 |
| DEC-B1-012 | Douyin 不纳入 V2 首期范围 | 用户于 2026-08-05 确认 |
| DEC-B1-013 | 按 Phase 0～5 分阶段实施；阶段仅受直接 blocker 门禁，最终物理删除才等待全量 blocker | 用户于 2026-08-05 确认 |
| DEC-B1-014 | 封面统一优先级：平台明确封面优先；同一内容、同一已认可观察中的首张已证明图片为第二顺位；两者均无才显示不可用 | 用户于 2026-08-05 确认 |
| DEC-B1-015 | 九工位插件由用户手动升级；Release-B 前只以实际版本核验为准，不保留旧插件协议或兼容入口 | 用户于 2026-08-05 确认 |
| DEC-B1-016 | B1 首期 capture identity 固定为 `(workspaceId,captureId)`，完整包 hash 固定为 `sha256`。Release-A 以条件唯一键同时保留 V1 `(workspaceId,captureId)` 语义和 V2 `(workspaceId,captureId,checksumAlgorithm,checksumValue)` 变体语义；同一 identity 只允许一个 `integrityStatus=verified` 快照，异 hash 快照登记为 `capture_identity_conflict` 且不得进入 Normalization/Projection。Release-B 的 EvidenceIngress 使用 SERIALIZABLE 事务并重试序列化/唯一冲突，重复包返回既有快照。 | 用户于本轮明确将剩余数据库技术取舍交由审核主线收口，以完成验证并进入执行阶段 |
| DEC-B1-017 | CaptureSubmissionV2 使用唯一严格协议 `capture-submission/v2`；外部 body 只含来源事实与唯一 base64 CapturePackage，workspace/receivedAt/sourcePrincipal 及四类授权身份由服务端注入；包内 header、records、artifacts 与 canonical/hash 规则以 `07-evidence-ingress-release-b-contract.md` 为唯一实施合同。 | 用户于 2026-08-05 确认按审核主线推荐方案收口 DR-B1-002～005 |
| DEC-B1-018 | CollectionContract 真值使用只追加的代码注册表；服务端对完整合同定义做 canonical sha256，B1 拒绝 unknown id/version 或客户端 hash 不一致，不新建 Contract 数据库表、不信任客户端自报。 | 同上 |
| DEC-B1-019 | migration 首期只作为 EvidenceIngress 内部受控能力和隔离测试合同存在；不创建 API/CLI/定时任务，不用它伪造迁移当前历史 Evidence，真实调用另立授权工单。 | 同上 |
| DEC-B1-020 | EvidenceIngress 只原子提交 Evidence；execution 在成功/verified replay 后以独立幂等 CAS 推进控制状态，失败靠同包 replay 续推；停发旧投影 outbox。SERIALIZABLE 最多三次尝试，仅处理已列明的冲突错误，耗尽返回可重试错误且绝不 fallback。暗态代码可分批提交，运行流量仍只允许最终一次性硬切。 | 同上 |
| DEC-B1-021 | XHS CollectionContract 采用插件先定义、工作台严格镜像的单一合同来源：先为六个现役 XHS Workflow 建立暗态 V2 合同、脱敏 fixture 与固定 hash，再在工作台代码注册表登记同一内容并做跨仓一致性验证；不接运行流量、不升级九工位。具体六份合同、字段、V1→V2 边界与验证规则以 `08-xhs-collection-contracts.md` 为唯一实施合同。 | 用户于 2026-08-05 确认 DR-B1-007 方案 A，并授权审核主线按现役 XHS Workflow 收口具体合同字段。 |
| DEC-B1-022 | `manual_import` 与 `recovery` 使用请求绑定的暗态 authority adapter：用户身份统一编码为 `user:<userId>`；manual importer 使用可回指服务端授权行的 `plugin-authorization:<authorizationId>`，且授权 workspace 必须非空并与已验证会话 workspace 相等；recovery 只接受 owner/admin，会话用户同时作为 sourcePrincipal 与 recoveryAuthorizedBy；receivedAt 仅由 adapter 内服务端时钟产生。adapter 拒绝 body/header authority 字段与全部 execution identity，绑定后任一 authority 篡改均失败。首批不接现役 route、不双写、不切流；migration 继续按 DEC-B1-019 禁用。 | 用户于 2026-08-10 明确确认“B1-B-09 按推荐方案执行”。 |
| DEC-B1-023 | execution authority 采用严格签名工位会话：`sourcePrincipal = execution-station:<stationId>`；V2 必须同时通过 stationToken、请求 HMAC、当前 PluginAuthorization token/状态及其→ExecutionStation 绑定，且 Station/Authorization/Job workspace 均非空并严格相等。验签结果是不可伪造且绑定原始请求 method/path/body hash 的能力对象，adapter 不接受同形普通对象或另一份 body。`executionPlanVersion` 是 `ExecutionJob` 上的服务端不透明版本；同一 job 的重试复用同一版本，计划变化必须新建 job，不使用 `ExecutionPlannerSnapshot`。首批只实现 schema expand、暗态 adapter 与隔离库证明，不接现役 route、不升级工位、不切流。 | 用户于 2026-08-10 明确确认“DR-B1-005 采用推荐方案 A，并授权主线实施 B1-B-10”。 |
| DEC-B2-001 | B2 的四条同源关系统一使用三列复合 FK：NormalizationRun→RawRecord、NormalizationRunCurrent→NormalizationRun、NormalizationRunCurrent→RawRecord、CanonicalObservation→RawRecord 均携带同一 `(workspaceId, rawSnapshotId, …Id)`，并引用已冻结的三列唯一键。先做七模型 schema expand 与隔离库正反例；不接 caller、不切流、不部署。 | 用户于 2026-08-10 明确确认“DR-B2-001 方案 A（推荐）授权你推进”。 |
| DEC-B2-002 | XHS 首期采用版本化、无损、确定性的 Derived 合同：`xhs.note/comment/author@1.0.0` 产生 `xhs.canonical/1` 技术 Canonical；identity、fieldPresence、canonical JSON/SHA-256、显式 retry、Normalization Current、唯一 evaluation member、错误优先级及 accepted/rejected Evaluation Current 推进全部按来源审计方案 A 固定。B2 只接受已审计且 lifecycle 可分析、integrity verified 的 Evidence 能力；首批保持暗态，不接 route/scheduler/outbox、V1 双写或 fallback。 | 用户于 2026-08-11 明确确认“DR-B2-002 采用方案 A，并授权主线实施 B2-B-02 暗态 Normalization / ContractEvaluation 服务”。 |
| DEC-B3-001 | XHS V2 领域身份采用严格相等与单一平台键：note 必须同时提供且满足 `noteId = platformContentId`，author 必须同时提供且满足 `authorId = platformAuthorId`；该约束已进入跨仓锁定的 `xhs.record-payload/v2`、六份 fixture 与 B2 adapter/fieldPresence，并在 ContractEvaluation accepted/CEC 推进前拒绝不一致，B3 只做纵深复核，任何层均不得 fallback。ContentAsset/Author 可在同一 B3 投影事务按现役平台唯一键确保存在，`authorEntityId` 只由服务端 `buildAuthorCode(platform,platformAuthorId)` 生成；Comment 身份固定为 `(workspaceId,platform,noteSourceId,sourceId)`。 | 用户于 2026-08-11 明确确认“DR-B3-001 方案 A”；B3-CONTRACT-SRC-001 以 adapter `2.0.0` / `xhs.canonical/2` 落地破坏性合同收紧。 |
| DEC-B3-002 | XHS note 的真实内容形态以插件字段 `type` 为 V2 唯一来源，值域严格为 `normal | video`；该字段已进入 `xhs.record-payload/v2`、六 fixture、Canonical fieldPresence 与严格 Domain projector。不得把所有 note 写死为同一 `contentType`，不得接受 V1 `contentType/noteType/itemType` 别名或从媒体猜测。note 只形成 ContentObservation，comment 只形成 Comment，author 只形成 AuthorObservation；未出现的可选字段保持 null 并由 fieldPresence 解释，必需身份/类型缺失则拒绝。 | 用户于 2026-08-11 明确确认“DR-B3-002 方案 A”；B3-CONTRACT-SRC-001 已完成跨仓实现与 hash 锁更新。 |
| DEC-B3-003 | B3 不创建未定义的 `ProjectionReceipt` 模型；Domain Projection 完成性由不可变 Observation 唯一键、CurrentProjection 指针/version 与事务结果证明。B6 已定义的 `PresentationReceipt` 只服务已登记的强一致展示页面，保持原边界。 | 用户于 2026-08-11 明确确认“DR-B3-003 方案 A”。 |
| DEC-B3-004 | `Comment` 是稳定业务身份实体，不代表某一次采集结果；`CommentObservation` 是不可变事实历史。页面/UI 只读取 `Comment.currentObservationId` 指向的当前有效观察，不直接读取 Observation。相同 comment identity + 相同内容 hash 视为 replay，不新增 Observation；相同 identity + 内容或状态变化产生新 Observation；未 accepted 的 Observation 保留但不得推进 current，accepted 后才单调推进。任何历史 Observation 均不得删除或覆盖。 | 用户于 2026-08-11 明确确认“DR-B3-004 采用方案 A”，并补充稳定实体、不可变历史、hash replay 与 accepted/current 推进语义。 |
| DEC-B3-007 | B3 首个领域纵切只实施 XHS ContentObservation/ContentCurrentProjection 暗态链。B3 提交时必须仍能以数据库并发机制证明精确 CEC 为 current accepted；相同 Observation replay 不增版本，新 accepted 才单调推进。accepted 只代表合同接受，不得默认解释为生命周期 active；来源不足时 lifecycleState 保持未知。旧 accepted 被 rejected 或更新的 accepted B 取代时，B2 必须在推进裁决的同一事务先 quarantine A；在 B3 完成 B 前允许短暂无展示，禁止继续把 A 作为当前事实。普通投影路径不能直接复活；Author、Comment、Metric 在物理合同关闭前延后。 | 用户于 2026-08-11 确认 B3 Projection 决策包的 Content-only、accepted≠active 与延后边界，并明确确认 accepted A→accepted B 采用“先隔离 A，B 完成后恢复”的方案 A。 |
| DEC-B3-008 | V2 accepted 媒体业务关系演进既有 ContentMediaUsage，不创建平行 receipt/关系表。每条 V2 active Usage 必须绑定同 workspace 的 accepted Evaluation、CanonicalObservation、CanonicalMediaSlot 与 MediaOrigin generation；同一物理 MediaItem 可承担多个有独立 slot 证明的业务用途。旧 accepted 被更新的 accepted 或 rejected 取代时，B2 在同一裁决事务关闭对应 active Usage 并保留历史行；B3 为新 accepted 完整建成 Projection 后才激活新 Usage。不删除共享 MediaItem/Origin/Blob；任一撤销失败使 CEC/Projection/Usage 整体回滚。 | 用户于 2026-08-11 确认媒体关系完整来源证明与原子撤销方案 A，并确认 accepted A→accepted B 先关闭旧关系。 |
| DEC-B3-009 | “打开原文”首期保持简单：ContentObservation.originalUrl 只来自同一 accepted CanonicalObservation 的已验证 XHS 分享链接，ContentCurrentProjection 只复制当前 Observation 的该值。只接受 XHS/xhslink HTTPS 来源；不创建 Action/Resolver，不解析重定向、不拼接、不改写，不读取旧 ContentAsset.sourceUrl、Media URL、delivery URL 或调用方自报值，也无 fallback。链接缺失如实为 null，非法则整笔 Projection fail-closed；URL 存在不代表生命周期 active。 | 用户于 2026-08-11 基于现役分享链接可直接打开的业务事实，明确选择简单来源 URL 而非 Action 化。 |

---

## 范围未决项（不属于 BLK-001～016 的模型决策）

> 曾被误登记为 `DEC-003` 至 `DEC-010` 的技术建议均不是实施授权。已确认的 B1 结果在下文统一登记；本节只保留尚未获得产品排期的范围事项。

| ID | 问题 | 状态 |
|---|---|---|
| UND-001 | F-O1：九工位插件 V2 实际升级完成水位 | 用户负责手动升级；Release-B 前仍须以九台设备真实版本与新协议探针记录核验，不能以计划替代完成。 |
| DR-B3-005 | `CanonicalMediaSlot.status=absent|unavailable` 的唯一来源与 reason 语义 | **方向 A 已确认，实施仍 SOURCE_INCOMPLETE**。业务原则已确认：producer 必须对每个预期媒体槽显式提交 `observed/absent/unavailable` 与封闭 reason，工作台不得推断；但 2026-08-11 的真实 producer 审计证明当前空 imageList/cover 不能区分“平台明确没有”和“未加载/失败”，因此在 producer 增加可验证事实、插件/工作台升级版本化合同并锁定 fixture/hash 前，只允许落 `observed`。从 candidate 缺失、CollectionContract slot 或 terminal 结果推断继续禁止。 |
| DR-B3-006 | XHS `live_photo` 如何映射到现有 Media Domain 的物理 kind、稳定 locator 与处理任务 | **SOURCE_INCOMPLETE**。现有 Media Domain 只确认 `image | video`，不能把 live photo 猜成普通图片、普通视频或两条互不关联的来源。B3-MEDIA-SRC-003 遇到任一 `live_photo` 时必须在事务写入前整批 fail-closed；待插件媒体合同与 Media Domain 处理语义均有确认来源后另行决策。 |

---

## 2026-08-05 已确认的 B1 决策结果

> 下表是当前实施唯一可用的决策结果。它取代历史 DC-001～DC-009 的候选选项；不得再将历史“推荐 A”误读为当前实施授权。

| 历史卡 | 已确认结果 | 直接影响 |
|---|---|---|
| DC-001 | `ARCHIVED` 可分析/重算；`REDACTED`、`PURGED` 不可读取、分析、投影 | BLK-001、002、003、016 |
| DC-002 | 同一业务实体、同一观察版本唯一 Canonical 业务投影；不把不同观察版本合并 | BLK-004～007 |
| DC-003 | V2 新写入 Comment、Metric、Content、Media 必须可验证地追溯至 Evidence | BLK-008、009 |
| DC-004 | 状态职责分离，但不新增组合状态体系 | BLK-010 |
| DC-005 | 不建设全量 PresentationReceipt；普通页面只读 Projection，额外回执仅限发布、审核、强一致运营页面 | BLK-011 |
| DC-006 | 撤销业务关系和对象 Projection，不级联删除共享 Media Asset | BLK-012～014 |
| DC-007 | 数据库层防止绕过 EvidenceIngress 修改或无审计读取 Evidence | BLK-015 |
| DC-008 | 冲突包保留为不可投影 Evidence，并受生命周期治理 | BLK-001、002、003、016 |
| DC-009 | 观察级质量/置信度/验证结果；不采用绝对真相评分 | BLK-001、002 |

## 历史决策卡（已由 B1 决策结果取代）

> 本节保留此前证据、候选项与理由，供审计追溯。九项均不再是 `DECISION_REQUIRED`，实现必须以“2026-08-05 已确认的 B1 决策结果”为准。

| 决策 ID | 需要负责人确认的一句话 | 不确认会阻塞什么 | 可选项 | 推荐但未代为决定的理由 | 证据 |
|---|---|---|---|---|---|
| DC-001 | `ARCHIVED` 的 Evidence 是否仍可作为 Derived/Current Projection 的输入，还是与 `REDACTED`/`PURGED` 一样立即停止所有投影？ | BLK-001、002、003、016 | A 保留可重算/可投影；B 归档即不可投影；`REDACTED`/`PURGED` 在两项下均不可读取/投影 | 推荐 A：归档通常是留存层级，不应抹掉可追溯观察；隐私/合规状态仍能彻底隔离。 | DEC-E02 的四种状态已确认但没有定义 `ARCHIVED` 的业务可用性；freeze:L165-L207 没有生命周期/处置审计字段。 |
| DC-002 | 每个 accepted CanonicalObservation 是否必须且只能由 DomainProjector 形成一个同源的 ContentObservation 或 AuthorObservation，并关联一个稳定 ContentAsset/Author？ | BLK-004、005、006、007 | A 一对一、同源、唯一写入者；B 允许一个 CanonicalObservation 形成零个或多个领域观察 | 推荐 A：才能保证当前展示可反查单一证据且不会重复投影；B 会把业务选择藏进投影层。 | freeze:L404-L413、L439-L461 缺少完整模型/FK/写入者；DEC-E01 要求 Canonical 作为领域投影输入。 |
| DC-003 | V2 新写入的 Comment 与 ContentMetricSnapshot 是否必须有其各自的 Canonical/Raw 同源来源，且不允许未溯源的人工或旧路径写入？ | BLK-008、009 | A 全部 V2 新行必有可验证来源；B 允许无来源例外 | 推荐 A：符合单轨与历史不伪造原则，避免再次产生无法审计的评论/指标事实。 | freeze:L464-L474 把关系写成可空/无 FK；只读库中 27,605 条指标没有 RawSnapshot、28,587 条没有 RawRecord。 |
| DC-004 | 五维任务状态是否必须各自有固定值域和唯一写入者，且“任务成功”不得替代 Evidence/Contract/Projection/Media 四个维度？ | BLK-010 | A 五轴独立并有明确终态；B 继续用单一执行状态汇总 | 推荐 A：否则会重现“采集成功但媒体/投影未完成”的当前失真。 | freeze:L476-L478 只列新增字段；当前 TaskStatusProjection 只有 execution/writeback/analysis 字段（schema:L4341-L4371）。 |
| DC-005 | Topic 与 Task 是否纳入 V2 PresentationReceipt；若纳入，是否必须有随可展示事实变化的持久 revision 与非空 workspace 归属？ | BLK-011 | A 纳入并提供真实 revision/workspace；B 首期明确排除 Topic/Task Receipt | 推荐 A：能令旧展示回执在对象变化后失效；`static` 无法做到。 | freeze:L953-L964 使用 `static`；Topic.schema 虽有 `version` 且服务会递增，但冻结规范未指定其是否为 Presentation revision。 |
| DC-006 | 当前 accepted 裁决被 rejected 替代时，是否必须同时撤销 Content、Author、所有关联媒体和旧展示回执，并且只能由之后新的 accepted 裁决恢复展示？ | BLK-012、013、014 | A 全部撤销，仅新 accepted 恢复；B 内容撤销但媒体继续可访问；C 允许人工直接恢复 | 推荐 A：内容和媒体可见性必须是一件事；B/C 会产生被拒绝内容仍可通过媒体 URL 访问的安全缺口。 | freeze:L582-L591 未覆盖媒体，且 canonical_writer 事务与权限表相互矛盾。 |
| DC-007 | EvidenceIngress 与 CapturePackage 原始包访问是否必须由数据库权限强制，包括 default_app 不得直写 RawSnapshot/RawRecord、不得直读原始包，且读取审计必须同 workspace 关联真实 CapturePackage？ | BLK-015 | A 采用冻结规范场景 9：数据库角色/受控函数/同 workspace 审计关系共同强制；B 仅靠应用代码约定。选择 B 即明确不满足当前冻结规范的“数据库权限拒绝”场景，必须先获得替代冻结合同批准，BLK-015 才能重新准备证明，B1 在此之前继续禁止。 | 推荐 A：只有 A 能在不修改冻结规范的前提下证明绕过写入、原始包读取和伪造审计均会被拒绝，符合 Evidence 不可绕过原则。 | DEC-E01/E02/E04/E05/E08；freeze:L516-L525、L604-L608 的审计关系与 default_app 权限均未完整定义，而 freeze:L1055 明确要求数据库拒绝越权操作。 |
| DC-008 | 同一 capture identity 但完整 Capture Package hash 不同的提交，是否必须保留为带完整 payload 的不可投影 Evidence，还是只保留不含 payload 的 security event？ | BLK-001、002、003、016 | A 另存不可投影 Evidence；B 只记 security event、拒绝保存冲突包 | 推荐 A：保留原始包才能复核来源冲突；但它会改变同 capture identity 的唯一键与保留成本。B 的安全面更小，但未来无法重算冲突事实。 | DEC-E09/00-contract:L86 要求“可审计非投影结果”；freeze:L1049-L1050 只写 conflict → security event；RawSnapshot 当前 `@@unique([workspaceId,captureId])`（schema:L4190），两种终态不能同时成立。 |
| DC-009 | DEC-E06 的质量、置信度和验证状态应按一次观察（RawSnapshot）、每个原始片段（RawRecord），还是两层同时持久化？ | BLK-001、002 | A 只在 RawSnapshot；B 只在 RawRecord；C 两层均有且各自语义不同 | 推荐 A：采集来源、完整包和验证结果天然属于一次观察；片段级解析质量已可由后续 Canonical/Normalization 表达。若业务要求片段本身也作为不同可信事实，才选 C。 | 00-contract:L66-L70 只确认 Evidence 必须支持三类判断，未定粒度；freeze:L163-L222 删除旧 quality 字段且只新增 integrity 字段，CanonicalObservation 才有 qualityStatus（freeze:L270-L294），没有 confidence/validation 状态来源。 |

### 不属于上述九张模型决策卡的既有事项

| 原编号 | 本轮处置 | 原因 |
|---|---|---|
| UND-001 | 保留为 Release-B 前的产品/插件排期决定 | 九工位插件 V2 升级不是 BLK-001～016 的模型语义，但未排期不得切换。 |
| UND-002 | 已由 DEC-B1-012 收敛 | Douyin 不进入 V2 首期；后续平台扩展须另立范围决策。 |
| UND-003 | 合并至 DC-007 | 本质是 Evidence 写入权限绕过。 |
| UND-004、UND-009 | 合并至 DC-006 | 本质是 rejected 裁决后的唯一可见性与恢复语义。 |
| UND-005 | 合并至 DC-005 | 本质是 Topic/Task 的 Presentation revision 与 workspace 语义。 |
| UND-006 | 已由 DEC-E05 收敛 | PostgreSQL Registry 是唯一事实来源；物理载体仍按已批准模型合同证明。 |
| UND-007 | 冻结规范已定义 B2 两事务失败语义 | freeze:L565-L577 是可转录来源，不再单列产品决策。 |
| UND-008 | 冻结规范已删除 ReleaseException | freeze:L544，不能重新引入例外机制。 |
| UND-010 | 冻结规范已指定代码注册的 PresentationPolicy | freeze:L480-L483；不新建表。 |
| UND-011 | 合并至 DC-001 / BLK-003 / BLK-016 | 同源链与当前指针依赖 Evidence 处置和完整键。 |
| UND-012 | 冻结规范已指定 EvidenceIngressReceipt | freeze:L108-L125；物理证明仍依赖 BLK-001。 |
| UND-013 | EvidenceAccessAudit 已有 ID 来源 | freeze:L516-L527，不构成独立业务决策。 |

---

## 已由本轮决策收敛的前序未决项

| 原编号 | 已由 | 收敛结论 | 仍未决定的内容 |
|---|---|---|---|
| UND-006 | DEC-E05 | PostgreSQL Evidence Registry 是唯一事实来源；大型 payload 可由其指向对象存储。 | 物理模型、指针、对象副本与访问控制。 |
| UND-014 | DEC-E04 | 非执行来源不得伪造 ExecutionJob 或 CaptureAttempt。 | 具体关联字段、约束与写入权限。 |
| UND-015 | DEC-E02、DEC-E03、DEC-E09 | 既有 Evidence 不覆盖；同次同身份同完整包 hash 重复关联既有证据，不同时间观察新建证据。 | 具体幂等键、哈希、迁移与现有 RawRecord 处置。 |

这些收敛结论只固定逻辑边界，不等于任何 blocker 已具备模型、DDL 或隔离数据库证明。
