# B3-A-04-R2：现役触发链与 Content Projection 读取硬切来源审计

> 日期：2026-08-12
> 性质：只读来源审计；不是 caller、读取切换或上线授权。
> 工作台固定点：`744deeeeb455d109e5654578679f7c4c7c8d9318`
> 插件固定点：`c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`

## 1. 固定点与范围

| 仓库 | 分支 / 状态 | HEAD |
|---|---|---|
| `/Users/gongyong/Services/content-workbench/v2-b3-projection-readiness` | `v2/b3-projection-readiness`；仅本报告 untracked | `744deeeeb455d109e5654578679f7c4c7c8d9318` |
| `/Users/gongyong/Services/linggan-boom` | `main...origin/main [ahead 6]`；工作树 clean | `c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a` |

本轮完整读取 B3-A-04 指定权威资料，以生产代码为当前运行事实；没有把 export、类型、测试、fixture、同名 Projection 或暗态 composition root 当生产 caller。

## 2. Figure A：现役写入与控制链

### 2.1 execution

```text
插件 taskPoller.flushDeltasBeforeCleanup / flushDeltas
  → background deps.flushDeltas
  → taskDeltaReporter.flush
  → deltaOutbox.runFlush（本地持久队列 claim/in-flight/retry/terminal）
  → background commitTaskDelta
  → taskLeaseClient.commitCollectionTaskDeltaThroughSync
  → POST /api/execution-stations/sync
  → route：签名、PluginAuthorization、workspace scope
  → execution-sync-service.handleSyncRequest
  → commit_raw_snapshot / handleCommitRawSnapshot
  → raw-snapshot-ingest-service.commitRawSnapshot
     同一事务：RawSnapshot + RawRecord + OutboxEvent(raw_snapshot.committed)
              QueueEntry/ExecutionJob → raw_committed
              ExecutionTaskRuntime → completed；TaskAttempt → success（当前 catch）
  → sync 200/accepted（只证明提交阶段）
  → scheduler tick 或 route after() 调用 outbox-worker
  → raw-snapshot-handler → WritebackDelivery / TaskStatus / tag
  → WritebackDelivery retry_wait / dead / applied
  → media.registration_requested
  → required delivery + media registration 全完成
  → writeback-delivery-service.closeExecutionJob → succeeded；阻塞终态 → failed
```

| 边界 | 真实 writer / 消费者 | 成功事实 | 失败传播与 retry owner |
|---|---|---|---|
| 插件提交 | `linggan-boom/src/workbench/runtime/taskLeaseClient.js:828` | 形成 `commit_raw_snapshot` operation | 插件按 operation result 重试；不能自报服务端终态 |
| route | `src/app/api/execution-stations/sync/route.ts:39-104` | 验签、授权、workspace 绑定后得到 service response | 401/400/426 等返回插件；不宣称异步链完成 |
| V1 Evidence 提交 | `execution-sync-service.ts:927-928,1010-1028` → `raw-snapshot-ingest-service.ts:157` | RawSnapshot/RawRecord/Outbox 与控制推进同事务 | captureId P2002 为 duplicate；无效 lease/identity mismatch/等待终态不正常推进 |
| 初始控制态 | `raw-snapshot-ingest-service.ts:497-518` | Job=`raw_committed`，Runtime=`completed` | **不是** Job `succeeded`；TaskAttempt 错误被 catch，不能当完整成功证明 |
| Outbox | `sync/route.ts:97-99,144-150`；`outbox-worker/dispatcher.ts:61-191` | claim 后按 handler outcome 标记 | transient→retry_wait/backoff，超限→dead；stale in-flight 恢复 |
| 业务写回 | `outbox-worker/raw-snapshot-handler.ts:24-231` | required WritebackDelivery 被创建 | throw→transient；quality/stale 显式 skip；Delivery 自有 retry 扫描 |
| 终态 | `writeback-delivery-service.ts:1794-1897` | required delivery 完成且 media registration processed 后 succeeded | blocking/dead/identity mismatch/media failure 可 failed；retry owner 是 Outbox 与 Delivery 扫描 |

`sync accepted`、`RawSnapshot committed`、`Runtime completed`、`Job succeeded` 是四个不同事实。现役 execution 链没有调用 V2 EvidenceIngress、B2 或 B3。

插件 terminal 到 sync 的真实调用与持久重试链：

| 节点 | 精确文件/行 | 事实与失败行为 |
|---|---|---|
| terminal flush | 插件 `src/workbench/runtime/taskPoller.js:1230-1239,1272-1283,1325-1334,1984,2192,2274,2318,2418` | terminal/cleanup 前调用注入的 `flushDeltas`；dependency 抛错或返回 `{success:false}` 都由 `flushDeltasBeforeCleanup` 转成异常，再由 terminal wrapper 返回 `flushPending`。各终态调用点检查该结果，不清理 active lease。 |
| dependency wiring | 插件 `src/background/index.js:2588-2590` | `flushDeltas` 精确代理 `taskDeltaReporter.flush()`。 |
| reporter | 插件 `src/workbench/runtime/taskDeltaReporter.js:7-54` | 以持久 store 创建 DeltaOutbox；`commitDelta` 继续调用 background 注入函数。 |
| persistent outbox | 插件 `src/workbench/runtime/deltaOutbox.js:142-216,218-248` | 先 recover stale in-flight，再 list pending/mark in-flight；按 frozen attemptId/leaseToken/leaseEpoch 分组。缺 execution identity → terminal；`retryable=false` → terminal；其它错误 → store.markRetry。 |
| background commit | 插件 `src/background/index.js:2324-2348` | 读取真实 station identity、authorization、runtime snapshot 后调用 `commitCollectionTaskDeltaThroughSync`。缺 station identity 抛 `retryable=true`。 |
| sync envelope | 插件 `src/workbench/runtime/taskLeaseClient.js:775-890` | 构造 `commit_raw_snapshot`，对 operation result 分类；永久拒绝不无限重试，暂时拒绝由 DeltaOutbox 重试。 |

重要现役事实：DeltaOutbox 的普通暂时失败被内部 catch 后会将 row 标记 retry，并让 `flush()`以 `{success:false}` 正常 resolve（`deltaOutbox.js:203-212`）；`taskPoller.flushDeltasBeforeCleanup` 会检查该结果并主动抛出 retryable error（`taskPoller.js:1230-1237`）。`flushTerminalDeltasBeforeCleanup` 随后返回失败，各终态调用点据此返回 `flushPending` 并保留 active lease，持久 row 继续由 DeltaOutbox store 重试。V2 仍须以自己的版本化合同和攻击测试证明同等语义，不能仅依赖 V1 实现细节。

### 2.2 manual_import

```text
POST /api/execution-tasks/manual-import
  → plugin-manual-import-service.importPluginManualRecords
  → ensureImportJob：创建/复用 V1 ExecutionJob + ExecutionTaskRuntime(running)
  → execution-task-import-service.importExecutionTaskResult
       事务内导入 Topic/Comment/Author/Media
       execution-task-observation-service 创建 V1 RawSnapshot + RawRecord
  → Runtime → completed
  → plugin-manual-import-service：Job → succeeded（失败则 Job/Runtime → failed）
```

| 项 | 当前事实 |
|---|---|
| caller | `src/app/api/execution-tasks/manual-import/route.ts:11-38` |
| V1 writer | `plugin-manual-import-service.ts:260-350,372-425`；`execution-task-import-service.ts:368-742`；`execution-task-observation-service.ts:537-654` |
| 完成 / 失败 | 同步完成后 Job=succeeded、Runtime=completed；`markImportFailed` 写 failed |
| retry | caller 重试；fingerprint Job 复用和 in-progress timeout 是 V1 语义 |
| V2 边界 | 当前路径创建 Job/Runtime 并给 RawSnapshot 填 jobId，不能冒充 V2 manual_import（非执行来源不得伪造执行关系） |
| V2 caller | `NO_RUNTIME_CALLER` |

### 2.3 recovery / migration

recovery 真实链：`POST /api/execution-tasks/recovery-import` → owner/admin session → `plugin-local-recovery-service.importPluginLocalRecovery` → 查同 workspace 既有 Job 作来源定位 → 创建 V1 partial RawSnapshot。它不创建/更新 Job、Queue、Runtime、Attempt；人工重提并以 `recovery-<entryId>` 去重。当前 V1 RawSnapshot 仍关联既有 jobId，但没有伪造新执行或推进控制；它不是 V2 recovery。V2 caller 为 `NO_RUNTIME_CALLER`。

migration 为 `NO_RUNTIME_CALLER`：首期无 API、CLI、定时任务或 composition root；既有 DR-B1-006-M-* 不变。

### 2.4 V2 构造、import 与 caller 证明

```bash
rg -n "EvidenceIngress|B2DerivedService|ContentProjectionService" src \\
  --glob '!**/*.test.ts' --glob '!**/*.integration.test.ts'
rg -n "submitExecutionEvidence|submitManualImportEvidence|submitRecoveryEvidence|evidence-ingress-orchestrator" src \\
  --glob '!**/*.test.ts' --glob '!**/*.integration.test.ts'
rg -n "new B2DerivedService|new ContentProjectionService|\\.derive\\(|\\.project\\(" src \\
  --glob '!**/*.test.ts' --glob '!**/*.integration.test.ts'
```

| 能力 | 定义 / 构造 | 生产 import/caller | 结论 |
|---|---|---|---|
| V2 EvidenceIngress | `evidence-ingress.ts:44`；orchestrator `:52-130` 构造三类 submit | 三个 submit 无生产 import/call | `NO_RUNTIME_CALLER` |
| B2DerivedService | `derived/b2-derived-service.ts:112` | 仅定义/测试；无生产构造或 derive | `NO_RUNTIME_CALLER` |
| ContentProjectionService | `projection/content-projection-service.ts:57` | 仅定义/测试；无生产构造或 project | `NO_RUNTIME_CALLER` |

`rawEvidenceIngress`、`TaskStatusProjection`、`projection-updater-service`、`ContentMetricSnapshot` 与 V2 Content Projection 不是同一领域。

## 3. Figure B：生产读取面与对账

### 3.1 固定搜索与实数

```bash
rg -l 'ContentAsset|contentAsset' src --glob '*.ts' --glob '*.tsx' \\
  --glob '!*.test.ts' --glob '!*.test.tsx' --glob '!*.integration.test.ts' --glob '!*.integration.test.tsx'
rg -l 'sourceUrl' src --glob '*.ts' --glob '*.tsx' \\
  --glob '!*.test.ts' --glob '!*.test.tsx' --glob '!*.integration.test.ts' --glob '!*.integration.test.tsx'
rg -l 'ContentMediaUsage|contentMediaUsage' src --glob '*.ts' --glob '*.tsx' \\
  --glob '!*.test.ts' --glob '!*.test.tsx' --glob '!*.integration.test.ts' --glob '!*.integration.test.tsx'
```

| 集合 | 文件数 |
|---|---:|
| `ContentAsset|contentAsset` | 115 |
| `sourceUrl` | 117 |
| `ContentMediaUsage|contentMediaUsage` | 28 |
| 三集合 union | 175 |

附录 A 对 175 个文件逐项列出命中、分类、消费者/排除理由和批次。类型/暗态代码明确排除；其余一律保持 `RUNTIME_CANDIDATE`，未逐字段证明前不得声称可切。

附录是**词法库存，不是 175 个已确认消费者**。其审计层级固定如下：

| 附录分类 | 审计层级 | 当前读法 |
|---|---|---|
| `TYPE_CONTRACT` | `TYPE_ONLY` | 仅类型/常量命中，排除为 caller/consumer 证明 |
| `B3_DARK` | `DARK_NONCALLER` | 暗态实现/fixture；生产 caller 已单独证明为 0 |
| `WRITE_PATH` | `WRITE` | 写入或转换候选，不得作为产品读取端 |
| K-01～K-08 表中有完整调用链的文件 | `CONFIRMED_DIRECT_OR_INDIRECT_READ` | 真实消费者，使用唯一主批次；其它领域只列依赖 |
| 其它 `UI/API_ROUTE/* service` | `UNRESOLVED` | 只有词法命中和路径分类；必须继续追 import/query/serializer/UI，不能声称完整 consumer |

因此 SI-03 是显式 blocker：本报告只交付实施前验收包，不把未追踪行用于制定代码切换范围。

### 3.2 关键消费者合同

| ID | 产品/服务 | 根数据源与调用链 | identity / 可见性 | fallback/混读 | 批次结论 |
|---|---|---|---|---|---|
| K-01 | Material“打开原文” | page → content-material-service → ContentAsset.sourceUrl | workspace+ContentAsset.id；无 V2 current+accepted+visible join | 当前 V1 ContentAsset | C1 PARTIAL；先建唯一 subject→visible Current read service |
| K-02 | Topic 市场样本/Agent | MetricSnapshot→platformContentId→ContentAsset，并读 AuthorMetric | MarketSample/contentAssetId；依赖 Author/Metric | 多领域 V1 | 主批次 C4；依赖 C2/C5/C6，不可只换 URL |
| K-03 | Monitor 页面 | MonitorOpportunity query/serialize → opportunity.sourceUrl | Opportunity/monitor identity，非 V2 Content subject | observation/radar candidate | C5 NOT_READY |
| K-04 | Topic 样本抽屉 | TopicMarketContext.samplePreviews | 上游有 MarketSample.contentAssetId，但同时依赖 Topic/Metric/Author | V1 MarketSample/ContentAsset | 主批次 C5；依赖 C2/C4/C6 |
| K-05 | Demand Radar | RadarSignal/opportunity item | signalKey/observationId，非 Content Current | observation URL | 主批次 C5；依赖 C6，NOT_READY |
| K-06 | Data Foundation | workspace inventory / ContentAsset 管理读模型 | workspace scoped；非普通页面 | Raw/V1 管理链 | C6 |
| K-07 | Media/Material/Export | ContentMediaUsage→MediaItem/Origin | workspace+contentAssetId；需区分 V2 六项 provenance | V1 Usage 同表存在 | C6 NOT_READY |
| K-08 | AI/Agent/创作 | 多服务组装 ContentAsset/Material/Corpus | 含 Author/Comment/Metric/Media | 多 V1 事实层 | 主批次 C6；依赖 C2/C3/C4 |

`sourceUrl` 不是统一 Content originalUrl：MonitorOpportunity/RadarSignal、Topic/MarketSample、Comment、Author profile、Media source/delivery、用户输入/task target 均有不同身份和写入者，不能因同名直接替换。

### 3.3 已证实现役旧读 / fallback / 混合权威

| ID | 精确位置 | 当前行为 | 硬切要求 |
|---|---|---|---|
| OLD-01 | `src/app/materials/[id]/page.tsx:393-406` | 从 Material 的 `asset` 读取 `ContentAsset.sourceUrl`，并另走既有 Media resolver | 建立唯一 visible Projection read 后一次切；不得 `V2 ?? asset.sourceUrl` |
| OLD-02 | `src/lib/services/content-material-service.ts:1405-1418` | platform/id/sourceUrl 在 Topic 与 Raw payload 多字段间 fallback | V2 Content consumer 禁止沿用；不同领域需求另建独立合同 |
| OLD-03 | `src/lib/services/monitor-opportunity-service.ts:475-513` | `observation.sourceUrl ?? radarCandidate.sourceUrl`，并写/确保 ContentAsset | 属 MonitorOpportunity 写模型，不得伪装成 Content Projection read fallback |
| OLD-04 | `src/lib/services/radar-signal-service.ts:675-700,877-880` | RadarSignal 从 observation/candidate/existing sourceUrl 合并 | 属 Radar Signal 领域；不能直接替换为 originalUrl |
| OLD-05 | `src/lib/services/data-foundation-workspace-inventory-service.ts:1050,1125,1168,1205` | ContentAsset 缺值时从多来源/Raw payload 补 sourceUrl | 管理/维护写路径；V2 新数据不得使用，不能作为页面兜底 |
| OLD-06 | `src/lib/services/content-material-service.ts:960-1037,1753` | Material 同时读取 ContentAsset、Topic、Comment、Metric/Usage 并组装 Agent 输入 | 主批次 C6；Content-only Projection 不完整承接 |
| OLD-07 | `src/lib/services/media-library-read-model.ts:75-77` 与 `workspace-export-service.ts:79,292` | 读取同一 ContentMediaUsage 表；既有 V1 行与 V2 provenance 行物理共存 | 读取必须明确 V2 六项 provenance/visible Current；禁止“查同表即 V2” |

没有发现或授权可保留的 V1 fallback。上表所有旧读只用于界定删除/隔离范围；产品请求不得新增 V1/V2 择优或缺失回退。

## 4. ingress 与控制矩阵

| kind | 当前 caller / 写入 | V2/B2/B3 | 控制完成 / retry | 切换缺口 |
|---|---|---|---|---|
| execution | sync → V1 Raw+Outbox | 全部 NO_RUNTIME_CALLER | commit→raw_committed；Outbox/Delivery/Media→succeeded/failed；插件+Outbox+Delivery retry | BLK-015、owner、任务五轴、九工位 |
| manual_import | manual route → V1 Job/Runtime+Raw+业务表 | 全部 NO_RUNTIME_CALLER | 同步 succeeded/failed；caller+fingerprint | 禁止 V2 伪执行；唯一硬切 |
| recovery | recovery route → V1 partial Raw；不推进控制 | 全部 NO_RUNTIME_CALLER | route result；人工+captureId | non-execution authority+BLK-015 |
| migration | 无 | 全部 NO_RUNTIME_CALLER | 无 | 首期禁用；既有 DR 保留 |

## 5. C1-C6 与 BLK-011

| 批次 | 候选 | 状态 / 缺口 |
|---|---|---|
| C1 Content-only | K-01 Material URL | PARTIAL/NOT_READY；无唯一 V2 identity→visible Current 读服务，当前 0 READY |
| C2 Author | author archive/context/dossier/lifetime/monitor | BLOCKED；无 V2 Author Projection |
| C3 Comment | Comment UI/services/monitor comment | BLOCKED；无 V2 CommentObservation Current |
| C4 Metric | Topic market/performance/metric | BLOCKED；BLK-009，首期无 metric RawRecord |
| C5 页面/Topic/Monitor | Topic、Monitor、Radar、Material | BLOCKED；不同业务实体不能按同名字段替代 |
| C6 AI/Agent/导出/管理/媒体 | 附录对应分类 | BLOCKED；需独立输入/写入合同 |

BLK-011 只适用于**明确登记**的发布、审核、强一致运营页面。当前没有生产页面的 B6 Presentation routeId/revision/receipt caller：Topic detail/deep evaluation/market sample 和 Task detail/workbench 只能标 `SOURCE_INCOMPLETE`；普通 Material 页面不自动适用 BLK-011，但仍受 Projection read contract 阻塞。没有任何 READY 硬切消费者。

## 6. Stage 1～5 实施前验收矩阵（全部 BLOCKED）

| Stage | 精确候选文件范围 | 输入事实 / runtime validator | 成功定义 | failure / retry owner | transaction 与禁止部分成功 | DB / 集成攻击 | 退出条件 | 用户决策 |
|---|---|---|---|---|---|---|---|---|
| 1 暗态 trigger | 未来只允许在 `src/lib/evidence/ingress/evidence-ingress-orchestrator.ts`、`src/lib/evidence/derived/b2-derived-service.ts`、`src/lib/evidence/projection/content-projection-service.ts` 与**获确认的新 owner 文件**内实施；现役 `execution-stations/sync/route.ts`、manual/recovery route 暂不改 | 只接受已提交 `EvidenceIngressReceipt + RawSnapshot` 及受控 reader 能力；运行时重验 workspace、integrity、current CEC、合同/version/hash，不能信任 event payload/type assertion | 四类获准入口只进入 EvidenceIngress；合格 snapshot 可被 B2/B3 幂等推进；各层状态真实独立 | DR-B3-CALLER-001 决定同步或后台 owner；若选 A，事件 worker 拥有 retry/dead；插件只负责 Evidence submission，不重放伪成功 | Evidence 提交不可因后续失败被回滚成未发生；B2 事务与 B3 SERIALIZABLE 事务各自原子；禁止 B3/Media 部分成功、V1/V2 双写 | default_app 直写/直读、跨 workspace Audit、伪造 reader；Evidence后/B2前、B2后/B3前、B3故障；双 worker/40001/死信重放 | BLK-015 `CLOSED`；DR-001 确认；Task 五轴 owner 来源化；九工位/三入口另授权；运行代码/隔离库门禁全绿 | **需要** DR-B3-CALLER-001；还需单独切流授权 |
| 2 Coverage/parity | 新的只读报告/审计模块应落 `src/lib/evidence/` 或 `scripts/`（需文件治理核准）；只读 `EvidenceIngressReceipt/CEC/ContentCurrentProjection/Usage`；不得改产品 route | DB 当前事实为分母；validator 拒绝冲突、redacted/purged、非 current、非 accepted、缺 Input/Usage provenance；不得以 fixture/mock 为分母 | 给出真实 workspace/cutoverAt 的 Evidence→B2→B3 覆盖、延迟、拒绝原因；V1 继续唯一服务产品 | Stage1 worker retry owner；报告失败必须显式失败，不隐藏为 0 或 V1 success | 只读，无写事务；产品请求不得双读；不得把 V1/V2 值择优 | 跨 workspace 查询、空分母、冲突/拒绝误计、陈旧 Current、缺 Usage、延迟/死信；结果由独立 SQL 反向核对 | 连续观察期阈值由用户批准；每个差异可追到 Evidence/状态，不含未知 silently excluded | **需要** coverage/延迟阈值和观察期业务选择 |
| 3 Content read service | 未来唯一 read module（建议 `src/lib/evidence/projection/content-projection-read-service.ts`，文件名非授权）及专用 API route；不得复用页面 Prisma/Media resolver | 输入 `workspaceId + platform + platformContentId` 或已证明稳定 contentAssetId；runtime 校验 Current visible、Observation、current accepted Evaluation/Input、原文 URL 合同、active V2 Usage 六项 provenance | 返回一个版本化 Content-only DTO：title/body/type/published/originalUrl/中央 cover/media；缺失保持合同规定的 null/unavailable | 请求失败 fail-closed；无 V1 fallback；read service 无重试 owner，调用方只能重试同一 DB 读取 | 单一一致性快照读取；不能先读 Current 再从旧表/Media 补值；不能部分标 success | 直接 SQL 伪 current、跨 workspace、rejected/quarantined、CEC supersede、late slot、invalid URL、V1-only Usage；并发读取不得见混合版本 | Stage2 达标；完整 DTO/权限/空值体验确认；静态扫描证明服务不引用 Raw/ContentAsset 展示字段/Media URL fallback | **需要** Content DTO 与缺失展示体验确认；普通页面不自动需要 BLK-011 receipt |
| 4 Batch C1 硬切 | 只限完成身份审计的 K-01 链：`src/app/materials/[id]/page.tsx`、其 API/`content-material-service.ts` 中精确读取边界；其它附录项禁止夹带 | 页面只接 Stage3 DTO；validator/serializer 不接受 V1 `asset.sourceUrl` 或任意 URL 注入 | K-01 的全部 Content-only 字段来自一个 Projection revision；不存在时明确不展示/不可用 | 请求错误向用户显式失败或不可用；不回退 V1；重试同一 read service | 一次部署启用新读；**不在同次部署删旧路径**；禁止字段级混读/部分成功 | E2E 注入 V1 与 V2 不同值，页面必须只显示 V2；V2 缺失时不得显示 V1；workspace/visibility/revision 并发攻击 | K-01 身份/字段/空值体验全证明；真实暗态 coverage 达门禁；用户批准 C1 切换 | **需要** 短暂无展示/延迟展示体验与硬切时点 |
| 5 旧读阻断/观察 | 删除/阻断 K-01 经证实旧读；静态规则覆盖 `ContentAsset.title/bodyText/summary/sourceUrl/contentType` 及页面 Media fallback；Release-C 物理删除仍后置 | 构建期扫描 + 运行 telemetry；validator 以 Projection 为唯一页面 DTO | 线上 48h 零旧读、零旧写、零 fallback、零页面绕过，错误/延迟在批准阈值内 | 新链 owner 保持 Stage1 retry；页面错误不可降级；回滚只能回滚部署，不恢复双读代码 | 与 Stage4 分开部署；不得在观察期破坏历史；共享 Media asset 不删除 | 静态绕过变体、动态 import、raw SQL、旧 helper、页面直查 Media、故障时 fallback、跨 workspace；运行日志/SQL 计数反证 | 全部 Release-B blocker `CLOSED`、48h 门禁通过，才进入 Release-C；未证明消费者继续 `UNRESOLVED` | **需要** 观察阈值、Release-C 删除授权 |

上述路径是**实施边界候选**，不是代码总工单；新 owner/read module 的具体文件只有在决策与文件治理完成后才能固定。任何一行未满足即保持 BLOCKED。

BLK-015 准确锚点：`04-blocker-ledger.md:222-233` 定义数据库强制与攻击负例；`:275` 明确受控 reader 和单独切流授权前禁止接 caller。DEC-B1-023 已确认 execution `sourcePrincipal=execution-station:<stationId>`，`execution-adapter.ts:206` 已实现，不是新决策。

## 7. DECISION_REQUIRED

### DR-B3-CALLER-001：B2/B3 暗态触发与 retry owner

| 项 | 内容 |
|---|---|
| 非技术问题 | Evidence 安全保存后，是让当前请求等待完整推导，还是由可靠后台接力，并让“采集成功”和“可展示”独立？ |
| 来源冲突 | B2 允许 Derived 独立提交、B3 失败独立重试；现役已有 `raw_snapshot.committed`，但属于 V1 写回且混合 best-effort/skip。权威来源未指定 V2 owner，也未授权复用。 |
| A（推荐） | 新增版本化、幂等 V2 后台事件/队列：绑定 receipt/snapshot；B2/B3 重验 DB 事实并独立重试；不把 V1 event payload 当 Evidence。 |
| B | 在 EvidenceIngress 请求内同步运行 B2/B3；失败由 caller 重放整链。 |
| 收益 | A 不延长插件请求、可独立重试并保留 pending 事实；B 组件较少。 |
| 风险 | A 需新事件合同/owner/证明，误复用会双轨；B 易超时、促使重复提交，且不能回滚已提交 Evidence。 |
| 阻塞 | Stage1～5；不得自行挑现役 Outbox 或同步路径。 |
| 可证伪验收 | Evidence后/B2前、B2后/B3前、B3事务中故障；无假 Projection/部分媒体；重试收敛；双 worker 不重复；死信可审计；任务成功不替代 B2/B3；无双写/双读/fallback。 |

## 8. SOURCE_INCOMPLETE

| ID | 缺口 | 影响 |
|---|---|---|
| SI-01 | BLK-015 角色、GRANT/REVOKE、受控 reader、Audit 同 workspace 关系 | 禁止 Stage1 |
| SI-02 | 哪些 Topic/Task 页面明确属于发布、审核或强一致运营 | BLK-011 无法实施映射 |
| SI-03 | 附录候选除 K-01～K-08 外的逐字段产品合同 | 分类不是切换授权 |
| SI-04 | K-01 stable Content identity→visible Current join/API 与空值体验 | 当前 0 READY |
| SI-05 | 九工位真实 V2 版本/协议水位 | 禁止 Release-B |

## 9. 治理、验证与声明

| 命令 | exit | 真实结果 |
|---|---:|---|
| `git diff --check` | 0 | clean |
| `git diff --no-index --check /dev/null <本文件>` | 1 | 文件存在 diff；无 whitespace diagnostics |
| `node scripts/check-project-governance.mjs --json` | 1 | 3 failures |

Governance 三项：

1. `docs/project-audit.md count drift: 当前有 353 个 Markdown 文件`：本轮 Markdown drift；`FOLLOW_UP_DOCUMENT_SYNC_REQUIRED`，工单禁止改第二文件。
2. `docs/TODO.md is 229 lines`：固定点既有债务，本轮未改。
3. `execution-sync-service.ts (2022 lines)`：固定点既有债务，本轮未改。

未修改生产代码、schema、migration、架构索引、TODO 或插件；未接 caller、未切流、未写数据库；未提交、push、merge、deploy；未新增双写、双读、fallback 或兼容路径。Stage1～5 全部 BLOCKED。

本报告的终态是：**阻塞台账 + 实施前可证伪验收包**。由于 DR-B3-CALLER-001、BLK-015、真实消费者逐链合同和各 Stage 用户门禁尚未关闭，本报告**不生成、也不授权下一张代码总工单**。

## 附录 A：三集合 union 全量逐文件分类

图例：`CA=ContentAsset|contentAsset`，`URL=sourceUrl`，`CMU=ContentMediaUsage|contentMediaUsage`。`RUNTIME_CANDIDATE` 必须继续逐字段审计，不表示可切；`EXCLUDE` 只排除其自身作为生产消费者的证明效力。
| 文件 | 命中 | 分类 | 消费者/排除理由 | 批次 |
|---|---|---|---|---|
| `src/app/api/content-assets/resolve/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/content-assets/understand/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/corpus-feedback/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/business-refs/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/business-refs/sync/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/content-assets/[id]/agent-analysis/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/content-assets/backfill-acceptance/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/content-assets/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/content-assets/sync/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/data-foundation/production-validation/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/materials/[id]/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/media-assets/cover/route.ts` | CA+URL | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/monitors/[id]/comment-collection/route.ts` | URL | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/monitors/content-assets/audit/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/topics/[id]/material-actions/route.ts` | CA | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/api/topics/route.ts` | URL | API_ROUTE | RUNTIME_CANDIDATE—需随下游服务逐链证明 | C6 |
| `src/app/data-foundation/page.tsx` | CA+URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/insights/DemandRadarInspector.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/library/page.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/materials/[id]/page.tsx` | CA+URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/monitors/page.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/topics/[id]/TopicDeepEvaluationSection.tsx` | CA+URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/topics/[id]/TopicDetailClient.tsx` | CA | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/topics/[id]/_hooks/useTopicDetailActions.ts` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/app/topics/[id]/page.tsx` | CA | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/comments/CommentCleaningTab.tsx` | URL | COMMENT | CONFIRMED_DIRECT_READ—Comment UI 读取评论来源字段；依赖 Comment 合同 | C3 |
| `src/components/comments/CommentInspector.tsx` | URL | COMMENT | CONFIRMED_DIRECT_READ—Comment UI 读取评论来源字段；依赖 Comment 合同 | C3 |
| `src/components/corpus/CorpusFeedbackGovernanceWorkspace.tsx` | CA | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/corpus/CorpusUsageAuditWorkspace.tsx` | CA | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/corpus/VoiceAssetsClient.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/covers/CoverDetailModal.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/inspiration/DailyInspirationPage.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/monitors/AuthorWorksCalendar.tsx` | CA+URL | AUTHOR | CONFIRMED_INDIRECT_READ—作者作品组件；主依赖 Author Projection | C2 |
| `src/components/monitors/KeywordOpportunityCard.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/monitors/MonitorAuthorDetail.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/monitors/MonitorSignalInspectorDrawer.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/monitors/MonitorSignalsWorkbench.tsx` | CA+URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/topics/CollectionTraceCard.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/topics/EvaluationCard.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/topics/SourceInfoSection.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/components/workbench/collection/ExecutionTaskCreator.tsx` | URL | UI | RUNTIME_CANDIDATE—页面/组件；未逐字段证明前不得切 | C5 |
| `src/lib/constants.ts` | CA+URL | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/data-foundation/agent-contract.ts` | URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/data-foundation/content-identity.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/data-foundation/evidence-card.ts` | URL+CMU | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/data-foundation/ingress-standard.ts` | URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/evidence/media/canonical-media-adapter.ts` | CA+URL | B3_DARK | EXCLUDE—暗态基础设施/fixture，不是现役产品读者 | — |
| `src/lib/evidence/projection/content-projection-proof-fixtures.ts` | CA | B3_DARK | EXCLUDE—暗态基础设施/fixture，不是现役产品读者 | — |
| `src/lib/evidence/projection/content-projection-service.ts` | CA+CMU | B3_DARK | EXCLUDE—暗态基础设施/fixture，不是现役产品读者 | — |
| `src/lib/execution-task-page-fingerprint.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/execution-task-serialization.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/execution-task/task-contracts.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/feishu-card-templates.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/media-library-maintenance.ts` | CA | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/monitor-target-identity.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/author-archive-audit-service.ts` | CA+CMU | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-archive-health-service.ts` | CA | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-archive-monitor-increment-service.ts` | CA | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-archive-service.ts` | CA | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-archive-topic-materialization-service.ts` | CA+URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-context-package-builder.ts` | CA+URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-context-service.ts` | CA+URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-deep-archive/archive-items.ts` | URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-deep-archive/contracts.ts` | CA+URL | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/services/author-deep-archive/snapshot-stages.ts` | CA+URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-demand-mapper.ts` | URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-dossier-audit-service.ts` | CA | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-lifetime-performance-service.ts` | CA+URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/author-media-observation-service.ts` | CA+URL | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/author-service.ts` | URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/collection-service.ts` | CA+URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/services/comment-cleaning-service.ts` | URL | COMMENT | RUNTIME_CANDIDATE—依赖 Comment 合同 | C3 |
| `src/lib/services/comment-service-helpers.ts` | URL | COMMENT | RUNTIME_CANDIDATE—依赖 Comment 合同 | C3 |
| `src/lib/services/comment-service.ts` | CA+URL | COMMENT | RUNTIME_CANDIDATE—依赖 Comment 合同 | C3 |
| `src/lib/services/comment-supply-service.ts` | CA+URL | COMMENT | RUNTIME_CANDIDATE—依赖 Comment 合同 | C3 |
| `src/lib/services/content-asset-understanding-service.ts` | CA+URL+CMU | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/content-ingestion-service.ts` | CA+URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/services/content-material-service.ts` | CA+URL+CMU | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/corpus-assembly-service.ts` | URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/corpus-feedback-service.ts` | CA | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/corpus-writing-service.ts` | CA+URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/creative-package-service.ts` | CA+URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/daily-boom-digest-service.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/data-foundation-agent-run-service.ts` | CA | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/data-foundation-agent-writeback-service.ts` | CA | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/data-foundation-business-ref-service.ts` | CA+URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-content-asset-agent-service.ts` | CA+URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/data-foundation-content-asset-service.ts` | CA+URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-daily-validation-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-dashboard-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-governance-maintenance-service.ts` | CA+URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-health-audit-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-history-maintenance-service.ts` | URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-ledger-summary-service.ts` | CA+URL+CMU | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-low-follower-viral-service.ts` | URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-milestone-acceptance-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-production-validation-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-production-validation-status-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-raw-evidence-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-snapshot-service.ts` | CA | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/data-foundation-topic-agent-service.ts` | URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/data-foundation-video-transcript-governance-service.ts` | CA+URL+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/data-foundation-workspace-inventory-service.ts` | CA+URL+CMU | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/demand-evidence-governance-service.ts` | URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/demand-radar-service.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/execution-task-import-service.ts` | CA+URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/services/execution-task-media-ledger-service.ts` | CA+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/governance-demand-mapper.ts` | CA+URL | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/inspiration-service.ts` | CA+URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/local-video-transcription-service.ts` | CA+URL+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/manual-collection-demand-mapper.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/manual-collection-writeback-service.ts` | CA+URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/services/market-insight-content-classification-service.ts` | CA | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/market-insight-history-audit-service.ts` | CA+CMU | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/media-cover-upload-service.ts` | CA+URL | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/media-job-processor-service.ts` | CA+URL | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/media-library-identity.ts` | URL | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/media-library-read-model.ts` | CA+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/media-library-write-service.ts` | CA+URL+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/media-library.ts` | CA+URL+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/media-processing-queue-service.ts` | CA+URL+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/monitor-agent-eligibility-service.ts` | CA+URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/monitor-author-archive-projection-service.ts` | CA | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/monitor-content-asset-audit-service.ts` | CA+URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-demand-mapper.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-detail-service.ts` | CA+URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-detail/author-lifecycle-projection.ts` | CA+URL | AUTHOR | RUNTIME_CANDIDATE—依赖 Author 合同 | C2 |
| `src/lib/services/monitor-detail/serialization.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-detail/timeline.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-detail/types.ts` | CA+URL | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/services/monitor-intelligence-feed-service.ts` | CA+URL+CMU | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-list-service.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-media-readiness-audit-service.ts` | CA+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/monitor-observation-service.ts` | CA+URL+CMU | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-opportunity-action-service.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-opportunity-evidence-service.ts` | CA | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-opportunity-query-service.ts` | CA+URL+CMU | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-opportunity-service.ts` | CA+URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-surge-history-service.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/monitor-surge-tracking-service.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/operation-sync-demand-mapper.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/operations-center-flow-service.ts` | CA | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/operations-center-object-service.ts` | CA+CMU | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/operations-center-today-service.ts` | CA+CMU | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/outbox-worker/media-handlers.ts` | CA+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/outbox-worker/raw-snapshot-handler.ts` | CA+CMU | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
| `src/lib/services/platform-target-normalization-service.ts` | URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/services/radar-featured-service.ts` | CA+URL+CMU | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/radar-signal-action-service.ts` | URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/radar-signal-service.ts` | CA+URL | MONITOR_SIGNAL | RUNTIME_CANDIDATE—机会/雷达/监控实体，非当然 Content | C5 |
| `src/lib/services/raw-snapshot-media-registration-service.ts` | CA | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/signal-tag-service.ts` | CA | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/task-demand-service.ts` | URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/services/taxonomy-tagging-service.ts` | CA+CMU | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/topic-audio-transcription-service.ts` | CA | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/topic-decision-boundary-service.ts` | CA | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/topic-doubao-material-understanding-service.ts` | URL | AI_AGENT | RUNTIME_CANDIDATE—AI/Agent 输入或写回；需独立合同 | C6 |
| `src/lib/services/topic-library-sync-service.ts` | URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/topic-market-comparison-service.ts` | CA+URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/topic-material-action-service.ts` | CA+URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/topic-material-demand-mapper.ts` | URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/topic-performance-rollup-service.ts` | CA | METRIC | RUNTIME_CANDIDATE—依赖 Metric 合同 | C4 |
| `src/lib/services/topic-service.ts` | CA+URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/services/transcript-quality-calibration-service.ts` | CA+CMU | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/voice-asset-service.ts` | CA+URL | MEDIA | RUNTIME_CANDIDATE—Media/转录链；Content-only 不足 | C6 |
| `src/lib/services/workspace-export-service.ts` | CMU | ADMIN_AUDIT | RUNTIME_CANDIDATE—管理/审计/导出；非普通页面 | C6 |
| `src/lib/services/writeback-delivery-service.ts` | CA+URL | WRITE_PATH | RUNTIME_CANDIDATE—写入/转换路径，不是可直接切的读取端 | C6 |
| `src/lib/topic-deep-evaluation-types.ts` | CA+URL+CMU | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/topic-instant-material-snapshot.ts` | URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/topic-market-comparison-types.ts` | CA+URL | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/topic-material-actions-types.ts` | CA | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/use-cases/monitors/handle-monitor-comment-collection-action.ts` | URL | COMMENT | RUNTIME_CANDIDATE—依赖 Comment 合同 | C3 |
| `src/lib/use-cases/topics/create-topic-use-case.ts` | URL | TOPIC_MATERIAL | RUNTIME_CANDIDATE—Topic/素材语义；逐身份验证前不属 C1 | C5 |
| `src/lib/workflow-types.ts` | URL | TYPE_CONTRACT | EXCLUDE—类型/合同定义本身不证明运行时消费 | — |
| `src/lib/xhs-execution-target-url.ts` | URL | SERVICE | RUNTIME_CANDIDATE—生产服务；需逐字段合同后分批 | C6 |
