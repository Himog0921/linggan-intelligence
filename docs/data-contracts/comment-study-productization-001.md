# 评论研究产品化：数据库与数据合同

> 状态：技术设计定稿，待用户批准开发；不是已实现或已验收声明
> 交付包：COMMENT-STUDY-PRODUCTIZATION-001 · 文档版 1.0
> 核对日期：2026-09-22
> 源码基线：`main@c74d72e3d17b9d5ecfb9953de025713c47e4560e`
> 责任：本包设计由 ChatGPT 整理；开发、共享库操作、真实模型调用与发布由 Mog 单独授权
> 事实与设计：标为“现状”的内容来自固定版本源码；“规定／新增／必须”为本次目标合同

## 1. 规范范围与记法

本文件是本包新增／变更数据库字段的唯一权威目标。SQL 片段是供实现者编写增量 migration 的设计参照，**本次没有执行，不能当作已通过 PostgreSQL proof 的迁移脚本**。现有字段以固定 SHA 的 bootstrap 和相关 migration 为准，不把旧 `linggan_comment_research_*` 字段拼入新合同。

数据库使用 `snake_case`；HTTP 使用明确列出的 `camelCase`。主键引用统一后缀 `_ref`。时间落库为 `timestamptz`，接口 RFC 3339，UI 用北京时间；原材料的 `observed_at` 当前是 text，不得在本包偷偷改写原始列。所有 ID 解析为 UUID，不以字符串拼 SQL。

## 2. 身份、证据与计数

| 名称 | 精确定义 | 禁止替代 |
|---|---|---|
| 作品身份 | `linggan_material_content.public_ref`，接口 `workRef` | 标题／URL 展示文字 |
| 稳定评论身份 | `(content_public_ref, comment_external_id)` | 一次 `material_ref`；作者昵称 |
| 材料版本 | `linggan_material_comment.material_ref`，接口 `sourceRef` | 新增评论数 |
| 某次研究目标 | `linggan_comment_study_target.target_ref` | 独立来源数 |
| 用户研究批次 | `linggan_comment_study_run.run_ref` | 内部模型 batch |
| 原声依据 | immutable raw comment 的 Unicode scalar `[evidence_start,evidence_end)` | researchText 的位置、UTF-16 JS 下标、作品正文 |
| 不同作者 | `(platform, author_external_id)`，身份须已知 | displayName；跨平台相同字符串 |
| 当前有效研究 | §8 的有效 Target head 与动态资格 | 全部历史 Signal 的并集 |

“独立依据”在 UI 改称 **不同评论／不同作者／不同作品**。这几个数如实计数，不等价于统计独立、目标客群比例或市场需求规模。相同原文由多人复制仍可计为不同评论，但不能据此宣传独立验证；本轮不额外提高建档门槛。

## 3. 现有表保留与变更清单

| 表 | 保留的核心字段／关系 | 本轮变更 |
|---|---|---|
| `linggan_material_comment` | material_ref、content_public_ref、comment_external_id、package_ref、原文／作者／父级／时间，append-only | 不改正文，不删除；仅加必要读取索引 |
| `linggan_material_comment_current` | 稳定评论最新版本 view | 不创建第二份 Current 真源；as-of 选择用同一顺序但添加冻结边界 |
| `linggan_comment_study_policy` | policy_ref、domain_ref、model_config_ref、contract、comment_budget、context_character_budget、created_at | 扩展方法版本四列；防止版本被修改 |
| `linggan_comment_study_active_policy` | singleton、policy_ref、updated_at | 保留；不是自动研究开关 |
| `linggan_comment_study_run` | run_ref、policy_ref、as_of、state、selection_manifest/hash、created_at/finished_at | 增三项实际预算、执行快照、外发控制 |
| `linggan_comment_study_work` | `(run_ref,content_public_ref)`、domain_ref、context_manifest/hash/state、selection_reason | 保留；每篇作品每 Run 一份语境 |
| `linggan_comment_study_target` | target_ref、run/work/source/parent、research_text/hash、dependency_state、state、input_manifest/hash | 增稳定评论 ID、内容 fingerprint、finished_at、terminal_reason；增 cancelled 终态 |
| `linggan_comment_study_batch` / `batch_target` | 输入输出、租约、模型收据、ordinal | 保留；新输入合同 v2，不改输出合同 v1 |
| `linggan_comment_study_semantic_attempt` | attempt_ref、target_ref、1–3 次尝试、rejection_code、output_manifest | 保留；缺依据不当 no_signal；费用与结构拒绝都留痕 |
| `linggan_comment_study_signal` | kind、proposition、evidence/span、problem_frame、canonical_text/hash | 不删除旧结果；当前读取与召回应用有效 head |
| `linggan_comment_study_resolution` | signal 唯一、候选／决策 manifest、invocation、state | 候选 manifest 固定 revision；请求历史留在新 snapshot 表 |
| `linggan_comment_study_problem_pair` | 两个 Signal、pair_manifest、state、invocation、created_problem_ref | pair_manifest 固定 ownerRunRef、比较方法、两侧输入；不新增用户概念 |
| `linggan_comment_study_problem` / `problem_revision` | 稳定身份、当前 revision、边界、版本 | 不改定义语义、不批量重建；依据不足通过读取面诚实显示 |
| `linggan_comment_study_problem_membership` | signal_ref、problem_ref、resolution_ref、created_at | 增对应 problem_revision_ref，旧未知可为 NULL |
| `linggan_comment_study_comparison` / embedding / pool_cursor | 现有缓存和进度 | 比较缓存新键命名空间；保留旧行；不复用错误判定 |
| `linggan_model_invocation` 与模型配置 | 通用 provider 事实与成本 | 不放评论正文；继续作费用真源 |

### 3.1 policy 新增列

| 列 | 类型／可空 | 约束／含义 |
|---|---|---|
| `method_name` | text，历史可 NULL | 新值 trim 后 1–100 Unicode scalar；用户识别名，可重名 |
| `parent_policy_ref` | uuid，NULL 允许 | FK 到同表；复制版本的父引用；不得指向自身 |
| `method_manifest` | jsonb，历史可 NULL | object；完整不可变方法，见 §5；不是 arbitrary skill 运行脚本 |
| `method_hash` | text，历史可 NULL | 小写 SHA-256；与 manifest 同时有值或同时 NULL |

新增版本必须三项 method_name/manifest/hash 全部存在；历史版本缺失保持 NULL。`parent_policy_ref` 与父 policy 必须同 domain；实现为 INSERT trigger 的校验，不新增复合家族表。复制时不读取“稍后可能改变”的全局默认。

版本保存后禁止 UPDATE/DELETE policy 整行，包括默认预算。改名字也保存新版本；不引入 name-only 可变副本。`activate_study_policy` 只更新 active_policy 的指针，并校验 domain 与模型连接当前可用。旧 policy 不可被新 Run 直接使用，因为其方法记录不完整。

### 3.2 Run 新增列

| 列 | 类型／可空 | 定义 |
|---|---|---|
| `comment_budget` | integer，历史可 NULL | 新 Run 必填 1–3000；本次目标数量上限，不承诺凑满 |
| `context_character_budget` | integer，历史可 NULL | 新 Run 必填 1–20000；沿用整片段截取规则 |
| `token_limit` | bigint，历史可 NULL | 新 Run 必填 1024–10000000；全部三阶段共享的记账额度 |
| `execution_manifest` | jsonb NOT NULL，默认 `{}` | 冻结方法 hash、选择方式、构建版本、限额与覆盖说明；不含秘密 |
| `dispatch_state` | text NOT NULL，默认 `stopped` | `enabled/paused/stopped`；只控制后续工作，不改写已经完成的结果 |
| `dispatch_reason` | text，NULL 允许 | enabled 必为 NULL；paused/stopped 必有合法原因 |
| `control_version` | bigint NOT NULL DEFAULT 0 | 非负；每次真实控制状态变化加 1，用于暂停／恢复／停止的 compare-and-set |

`dispatch_reason` 闭集：`user_paused`、`user_stopped`、`budget_exhausted`、`legacy_unrecorded`、`upgrade_guard`、`method_unavailable`。新的开始请求以 enabled/NULL 创建。迁移旧 Run 默认 stopped/legacy_unrecorded，防止后台用未知历史 prompt 继续外发；**不自动改写旧 Run/Target 原有状态、结果或调用记录**。旧未闭合队列的延续由运行手册处理。

实际预算、policy_ref、as_of、selection_manifest/hash、execution_manifest 对新 Run 不可变；控制状态可变。暂停可在额度足够且方法可用时恢复；stopped 不恢复、不在本轮提供在线加额动作。额度用尽停止后，保留可用结果，未处理目标明确取消；再次运行必须是新的、有明确额度的请求。

### 3.3 Target 新增列与约束

| 列 | 类型／可空 | 定义 |
|---|---|---|
| `comment_external_id` | text，历史允许 NULL | 新 Target 必填；必须逐字等于 source_ref 所指材料的 comment_external_id |
| `input_fingerprint` | text，历史允许 NULL | §6 的内容等价哈希；旧无法确认的输入不伪造 |
| `finished_at` | timestamptz，历史允许 NULL | 新 Target 进入 succeeded/no_signal/needs_context/failed/excluded/cancelled 时记录 |
| `terminal_reason` | text，NULL 允许 | 目标停止或失败的机器原因；与旧 exclusion_reason 分工见下 |

原有 state 增加 `cancelled`；其余枚举保留。原有 `exclusion_reason` 仍只用于 `excluded`，保持与 `input_invalid` 的原有约束。不得把“用户停止”“预算用尽”写成来源 excluded。

terminal_reason 的新合同闭集：`user_stopped`、`budget_exhausted`、`source_unavailable`、`input_limit_exceeded`、`attempts_exhausted`、`provider_failed`、`legacy_execution_stopped`。succeeded/no_signal/needs_context 的 terminal_reason 为 NULL；needs_context 的理由在输入或 semantic attempt 的 reason 中。failed 必有相应原因；cancelled 必为前三类中 user_stopped/budget_exhausted/legacy_execution_stopped。excluded 保留 exclusion_reason，terminal_reason 可为 source_unavailable。旧行不猜填。

新 Target 的 INSERT trigger 校验：Run 的 method_manifest 完整；source_ref 对应的作品及 comment_external_id 匹配；fingerprint/稳定 ID 非空；新终态必须有 finished_at。不得以“兼容旧行”为由允许新写 NULL。

```sql
-- 以下使用短索引名，避免 PostgreSQL 标识符 63 字节截断。
CREATE UNIQUE INDEX cs_target_run_comment_uq
  ON linggan_comment_study_target(run_ref,content_public_ref,comment_external_id)
  WHERE input_fingerprint IS NOT NULL;

CREATE UNIQUE INDEX cs_target_active_comment_uq
  ON linggan_comment_study_target(content_public_ref,comment_external_id)
  WHERE input_fingerprint IS NOT NULL AND state IN ('ready','queued','running');

CREATE INDEX cs_target_comment_history_idx
  ON linggan_comment_study_target(content_public_ref,comment_external_id,created_at DESC,target_ref DESC);
```

在途唯一索引不包含 method_hash：换提示词也不能让同一评论同时收费。历史 NULL fingerprint 行不用于假造执行唯一性；新启动查询还必须跳过其 **仍获准执行** 的 legacy 目标。升级时所有旧外发已停止，且旧 Worker 不得继续运行，因此不会出现两个制度并发写入。

稳定 ID 回填仅 JOIN 原始 source_ref 后逐字复制，保存行数核验。旧相同评论多次研究保留多行，不删除，不建对全部历史的全局 UNIQUE。

### 3.4 membership 新增列

`problem_revision_ref uuid NULL`，FK `(problem_revision_ref,problem_ref)` → `linggan_comment_study_problem_revision(revision_ref,problem_ref)`。新写入必须非空且等于本次真正比较的 revision。旧记录只有在请求／决定 manifest 能证明对应 revision 时才回填；否则 UI 显示“历史匹配版本未记录”。不能把当前 revision 倒填到历史。

## 4. 三张新增表的完整字段

### 4.1 `linggan_comment_study_clean_cache`

| 列 | 类型 | 约束 |
|---|---|---|
| `source_ref` | uuid | NOT NULL，FK raw comment(material_ref) |
| `cleaner_version` | text | NOT NULL，1–100；只接受当前已安装 cleaner 或历史已登记版本 |
| `raw_sha256` | text | NOT NULL，SHA-256(raw UTF-8)，不含作者／点赞 |
| `research_text` | text | NOT NULL，允许 dropped/anomaly 为空；最大 16000 scalar |
| `clean_state` | text | NOT NULL，`direct/context/dropped/anomaly` |
| `clean_reasons` | jsonb | NOT NULL，字符串数组；沿用 cleaner 输出，不发明价值评分 |
| `created_at` | timestamptz | NOT NULL default scope_001_now() |

PK `(source_ref,cleaner_version)`。不保存 offsets：需要证据偏移时对 ≤16000 字原文用同版本 cleaner 重新计算，与现有解析器一致；没有必要全库持久化每字节位置数组。查询、冻输入和证据校验不可使用三个不同 cleaner。

缓存写入使用 `ON CONFLICT DO NOTHING`，发生同键而内容 hash 不同属于实现缺陷，不覆盖；主动核验并报错。缓存可由原始材料再生，但查询必须关联当前来源权限；纯 emoji／纯无语义 mention 的 dropped 结果不进入正常原声列表。

索引：PK；`cs_clean_search_trgm_idx` 为 research_text 的 GIN gin_trgm_ops，谓词 clean_state IN ('direct','context')；`cs_clean_version_state_idx(cleaner_version,clean_state,source_ref)`。`pg_trgm` 通过批准的增量 migration `CREATE EXTENSION IF NOT EXISTS pg_trgm` 安装并验证，不是额外联网服务。

### 4.2 `linggan_comment_study_start_request`

| 列 | 类型 | 约束／含义 |
|---|---|---|
| `request_ref` | uuid | PK；客户端为一次明确开始意图生成，重试复用 |
| `domain_ref` | uuid | NOT NULL FK observation_domain |
| `policy_ref` | uuid | NOT NULL FK study_policy |
| `request_hash` | text | NOT NULL SHA-256，规范化命令＋可信 origin |
| `origin` | text | NOT NULL `manual/scheduled`；由服务器可信调用上下文指定 |
| `origin_ref` | uuid | manual 为 NULL；scheduled 为下阶段 schedule_ref；本轮只合成测试 |
| `scheduled_for` | timestamptz | manual 为 NULL；scheduled 必填，表示计划时间槽而非执行完成时间 |
| `command_manifest` | jsonb | NOT NULL object；范围、模式、预算、可选重新研究理由，不含正文 |
| `outcome` | text | NOT NULL `created/no_work/index_pending` |
| `run_ref` | uuid | NULL 允许，UNIQUE FK study_run；created 当且仅当非 NULL |
| `result_manifest` | jsonb | NOT NULL object；冻结目标数／排除数／索引覆盖等无正文回执 |
| `created_at` | timestamptz | NOT NULL default scope_001_now() |

该表只保存已经提交的完整结果，不写“processing”临时行。开始事务持有 domain advisory lock，完成 Run 后一起插入回执；崩溃全部回滚。跨 domain 恶意复用 request_ref 时 PK 阻断，事务回滚并返回 409，不报告另一个领域内容。

新增索引 `cs_start_domain_created_idx(domain_ref,created_at DESC,request_ref)`；scheduled 防重索引 `cs_start_schedule_slot_uq(origin_ref,scheduled_for)` WHERE origin='scheduled'。本轮没有真实 schedule 表，因此 origin_ref 仅 UUID，不伪造 FK；下阶段新增 schedule 时再添加 FK 和符合性验证。origin 的配套 NULL 规则用 CHECK 固定。

开始回执不可 UPDATE/DELETE。无工作回执本身也幂等：相同 requestRef 重试不因后来有了新评论而突然创建 Run。用户要再研究，产生新 requestRef。

### 4.3 `linggan_comment_study_model_request`

| 列 | 类型 | 约束／含义 |
|---|---|---|
| `invocation_ref` | uuid | PK，FK linggan_model_invocation |
| `run_ref` | uuid | NOT NULL FK study_run，唯一预算归属 |
| `policy_ref` | uuid | NOT NULL FK study_policy，必须与 owner Run 一致 |
| `stage` | text | NOT NULL `semantic/resolution/pair` |
| `batch_ref` | uuid | NULL 允许 FK study_batch |
| `resolution_ref` | uuid | NULL 允许 FK study_resolution |
| `pair_ref` | uuid | NULL 允许 FK study_problem_pair |
| `attempt_ordinal` | integer | NOT NULL 1–3；semantic 按 batch 固定 1，目标的多次尝试仍由 semantic_attempt 计 |
| `input_context_hash` | text | NOT NULL SHA-256(stageHash＋冻结比较输入)，不含尝试序号／时间；用于同上下文有限重试 |
| `request_manifest` | jsonb | NOT NULL object；实际 system、prompt、参数与材料依赖，见 §5.3 |
| `request_hash` | text | NOT NULL SHA-256；与对应通用 invocation.request_hash 一致 |
| `dispatch_started_at` | timestamptz | NULL 表示尚未持久领取外发；只能 NULL→一个时间值一次 |
| `deadline_at` | timestamptz | NOT NULL；semantic 等于 batch 租约截止；其他阶段为领取时间＋timeout＋15秒 |
| `created_at` | timestamptz | NOT NULL default scope_001_now() |

CHECK 三个 subject_ref 中恰好一个非 NULL，且与 stage 对应。分别建 `cs_request_resolution_attempt_uq(resolution_ref,input_context_hash,attempt_ordinal)` 与 `cs_request_pair_attempt_uq(pair_ref,input_context_hash,attempt_ordinal)` 部分 UNIQUE；semantic 对 batch_ref 建部分 UNIQUE。索引 `cs_request_run_idx(run_ref,created_at,invocation_ref)`、`cs_request_deadline_idx(deadline_at,invocation_ref)`。

字段内容与依赖不可改；只允许 dispatch_started_at 单向 CAS，deadline_at 不延长。请求失败、重试、subject 重新开启都追加新的 invocation/snapshot，不覆盖旧请求。不增加自己的 success/failed 状态列；调用成败仍在通用 ledger，业务结果仍在对应接受器。

## 5. JSON 合同：所有必填键固定

### 5.1 method_manifest：`comment-study.method.v1`

以下结构中大括号描述类型，不是允许模型自由发明字段。

```json
{
  "contract": "comment-study.method.v1",
  "cleanerVersion": "comment-clean.v2",
  "builderRevision": "comment-study.request-builder.v2",
  "safetyRulesRevision": "comment-study.safety.v1",
  "modelConfigRef": "00000000-0000-4000-8000-000000000010",
  "modelIdentity": {
    "modelRef": "00000000-0000-4000-8000-000000000011",
    "connectionVersionRef": "00000000-0000-4000-8000-000000000012",
    "modelId": "synthetic-model"
  },
  "stages": {
    "semantic": {"systemInstruction": "合成示例：实际值由方法保存器生成", "outputSchema": {}, "stageHash": "0000000000000000000000000000000000000000000000000000000000000000"},
    "resolution": {"systemInstruction": "合成示例", "outputSchema": {}, "stageHash": "0000000000000000000000000000000000000000000000000000000000000000"},
    "pair": {"systemInstruction": "合成示例", "outputSchema": {}, "stageHash": "0000000000000000000000000000000000000000000000000000000000000000"}
  }
}
```

这是结构示例，空 Schema、synthetic-model 和全零 hash 不可用于真实保存。服务器必须从当前严格 Rust 合同生成完整 Schema，验证 adapter 的 6144 字节 Schema 上限；保存器生成完整 systemInstruction，并计算 stageHash，不信任客户端传入 hash。三个阶段一次绑定同一已存在 immutable model_config；本轮不引入按阶段选择不同供应商的额外复杂度。

用户可编辑的是每阶段补充研究说明 `stageInstructions`，每阶段最多 8000 scalar；规则前缀、原声要求、字段闭集、Schema、来源权限与确定性接纳不可编辑。完整 systemInstruction 是固定规则＋用户说明，原文都可见并被冻结。仅保存配置不外发、不运行测试模型、不自动升级历史。

### 5.2 新 Run / Target / batch manifest

`selection_manifest` 新 contract 为 `comment-study.run-selection.v2`，必填：requestedWorkRefs、coveredWorkRefs、targetSourceRefs、selectionMode、commentBudget、contextCharacterBudget、tokenLimit、indexCoverage、exclusionCounts；三种 Ref 数组去重、有明确顺序。仍不把 rawText 复制进选择 manifest。

手动选择作品或评论时 work.selection_reason=`user_selected`；下一阶段全域backlog自动选择为`budget_selected`，不新增这个内部枚举。

`execution_manifest` 新 contract 为 `comment-study.execution.v1`，必填 methodHash、builderRevision、engineRevision、maxTargetsPerBatch(12)、selectionOrder(`work_round_robin_oldest_first.v1`)；engineRevision 是实际构建 commit，有 dirty 构建则额外标 dirty=true，不冒充基线版本。

新 Target `input_manifest` 为 `comment-study.target-input.v2`，必填 targetSourceRef、workRef、workContextHash、parentContext、rawSha256、cleanerVersion。**不再复制 workContext 全文**。同一 Run/work 的内容只存 study_work.context_manifest；旧 v1 原记录保持可读，不能批量删除其中内容来节省空间。

新 batch `input_manifest` 为 `comment-study.note-batch-input.v2`，必填 batchRef、runRef、contentPublicRef、workContext、targets。targets 每项包含 targetRef、sourceRef、rawText、researchText、dependencyState、parentContext。原始 target 文本只从已冻结 source_ref 获取；每批 workContext 仅一次。输出仍为 `comment-study.note-batch.v1`，避免无必要改写现有接受器。

原文和清洗文本不是两条证据：rawText 是引用唯一来源，researchText 是辅助理解。括号、全角符号、emoji 和空格清洗后，evidence 与 frame.basis 必须仍逐字命中 rawText；清洗 offsets 只用于核查，不能编造原文不存在的连续片段。

### 5.3 请求快照 `comment-study.model-request.v1`

request_manifest 必填：contract、stage、policyRef、methodHash、stageHash、builderRevision、modelConfigRef、modelIdentity、system、prompt、parameters、materialDependencies。

- system 与 prompt：实际发送给 adapter 的**字符串**，不是“可再次生成”的摘要。prompt 中含当次严格 outputSchema。
- parameters：实际使用的 operation=`analyze`、maxOutputTokens、timeoutMs；只有 adapter 确实支持并设置的参数才出现。未控制 temperature 不填一个假的 0。
- materialDependencies：目标／父评论／作品／媒体引用与原文 hash；每项 kind、sourceRef、workRef，评论项还含 commentExternalId。只来源依赖，不包含秘密。
- request_hash：固定计算 canonical_json_v1({system,prompt,modelIdentity,parameters}) 的 SHA-256。policyRef、methodHash、materialDependencies 等纯审计字段不加入这个 hash；prompt 本身实际含来源 ID 的地方不删除。这是应用提交给 adapter 的请求合同，不是包含认证头的原始 HTTP 抓包。请求快照与 ledger 同事务写入。

materialDependencies还必须覆盖候选Problem revision的seed来源及各Signal实际使用的父／作品／媒体语境；不是只登记当前Target而遗漏候选中的原文。读取与外发采用同一依赖检查。

快照只能通过本地受保护只读接口查看。任一依赖受限时，不返回整段 system/prompt 中的输入正文；方法说明可单独从 policy 读取。不能将快照暴露到通用日志、事件流、错误提示或无权限导出。

### 5.4 候选和 pair

resolution.candidate_manifest 新 contract 为 `comment-study.problem-candidate-set.v2`：candidateProblemRefs、candidates、priorRetrievalIncomplete（可 NULL）。candidates 每项固定 problemRef、revisionRef、definitionHash、definition、stableIdentity、includeCriteria、excludeCriteria。与 caller 所给闭集逐一对应，无遗漏／重复。空集仅在真实召回完整且无匹配时代表 novel；检索不完整仍保留 retrieval_incomplete。

新提出 Problem 的 stableIdentity 恰含 actor、goalOrExpectedState、barrierOrUnmetNeed、context，值为非空字符串（最长500 scalar）或 NULL；未知不得补全。新 Rust DTO 与 Schema 同步约束该形状，持久化为 core_frame；Signal 的 value/basis 结构不变。现有 canonical_text 已可读字符串字段及历史 subject 别名，因此无需改写旧 core_frame，也不改变相同问题判定门槛。

pair.pair_manifest 增 ownerRunRef、policyRef、stageHash、firstInput、secondInput；ownerRunRef 为 firstSignal 所属 Run，保持现有规则；两个输入都固定其 signalRef、targetRef、原声来源身份、proposition、problemFrame、方法来源。跨批次比较只在同 domain、当前来源合格且候选身份明确时进行，不通过第二条评论获得预算加倍。

## 6. 哈希与输入变化

统一 `canonical_json_v1(value)`：对象键按 UTF-8 字节序递归排序；数组顺序保持；UTF-8 无 BOM、无额外空白；字符串不擅自 Unicode 归一化、不 trim 原文；本包用于哈希的数字仅整数。实现放在本模块的纯函数，不引入通用序列化框架。固定字符、嵌套对象与数组顺序 fixtures 验证跨 Rust / 测试脚本一致性。

| 哈希 | 覆盖 | 特意不覆盖 |
|---|---|---|
| raw_sha256 | 原始正文 UTF-8 字节 | 点赞、采集时间、material_ref |
| research_sha256 | 当前 cleaner 输出 UTF-8 | 业务解释 |
| method_hash | 完整 method_manifest（stageHash 已计算） | 用户输入的假 hash、created_at、policy_ref |
| stageHash | 此阶段完整 system/schema/builder/模型身份/实际参数能力 | 其他阶段的说明变更；批次预算与时间 |
| input_fingerprint | 稳定评论身份、raw_sha256、research_sha256、cleanerVersion、dependencyState、所保留作品语境的 kind/order/text、父评论稳定身份与正文 hash | 仅引用 UUID 的变化、观测时间、点赞、未入包的媒体 |
| input_hash | 已冻结 Target/batch 的完整 manifest | 不删其中真实引用，用于审计 |
| request_hash | 实际完整请求 | 日志展示摘要 |
| comparison.cache_key | `comparison.v2`＋domain＋比较种类＋stageHash＋左右实际比较 payload hash | 时间；只用 canonical_hash 或 policy_ref 都不够 |

输入内容相同但材料版本 ID 变化，input_fingerprint 不变；审计 input_hash 可以改变。新增 OCR 只有真正进入保留语境或解决缺失依赖时才改变 fingerprint；未用的 OCR／点赞变化不自动触发重研。

每个算法常量／hash revision 变更先跑固定用例；方法改默认不追溯覆盖。缺 fingerprint 的旧记录标 `inputComparison=unknown`，默认跳过历史已成功材料；不能当作“全部输入发生变化”重跑。

## 7. 清洗、来源与索引覆盖

### 7.1 一套资格函数，三个用途

材料可显示：原文可读且确定性清洗非 dropped/anomaly。身份未知不等于无语义；可显示原声并标作者身份未知，但不进当前研究目标。已知作品作者声音默认不列入“用户评论”，在作者声音筛选／父语境中可见；不能当用户痛点证据。

目标可研究：当前 domain=ADHD、原文可读且未受限、评论作者／作品作者均已知且不同、clean_state direct/context。保留当前严格作者资格，不新增基于点赞／热度／完整率的门槛。

语境可用：先按 as-of 和版本排序确定**最新那一条**，再判断可读、来源归属和 restriction。最新 UNKNOWN 或受限时不回退到历史已知正文。父评论还须匹配同作品 parent_external_id；作品作者回复可解释指代，但绝不能成为当前 Target 的 evidence。

对作品媒体语境，在通过现有ownership、as-of、succeeded、retirement、disposition与OCR layering资格的候选中，每个 `(slot_key,kind)` 只保留最新可用产物，按created_at DESC、derivative_ref DESC稳定取一条；新未完成处理不阻断旧的仍合格可用产物。title/body优先、媒体slot ordinal顺序保留，缺序号在已知序号之后。同kind/slot不重复混入重采版本。语境被预算省略时仍保留可核查的omitted清单，不以缺一个OCR阻断整篇研究。

### 7.2 限制传播

执行前使用当前限制检查，冻结时间不允许绕过后来的受限状态。材料受限后：原声、researchText、proposition、evidence、problemFrame 含 basis、parentContext、带其内容的 request snapshot 一起降级，不以 `null` 原文配一个仍含正文的 frame。

由多条依据共同形成的 Problem 定义可能包含受限内容：若 seed_signal_refs 中任一依据受限，问题标题／定义／边界整体暂停展示，返回 metadata 与 `definitionState=restricted_basis`；不把剩余一个 seed 自动当成原定义证明。其他非 seed 支持受限时剔除相应支持与原声，不整页阻塞不受影响的问题。新的定义修订不在本包自动产生。

限制判断失败属于 unavailable，不返回旧缓存正文。已知来源失效只隔离受影响 Target／比较／Problem，不阻断其他作品。外发资格证明以dispatch领取时的已提交限制事实为边界；限制在请求已经发出后到达，无法撤回上游已收到的内容，必须在接纳与后续读取时停止继续传播，不宣称数据库事务能原子撤回远端HTTP。

模型语义请求还未外发时取消含受限输入的batch；受影响Target标excluded、dependency_state=input_invalid、exclusion_reason=frozen_input_restricted并终结，其他目标可重打包。不能把相同受限workContext的目标反复queued→打包→取消。重新使用已恢复或更少语境必须建立明确新输入，不修改旧冻结包。回程才发现限制时按Target依赖隔离，合法的其他目标继续接纳，已消费token照实记账。

### 7.3 清洗缓存更新

后台沿用当前进程，每 tick 最多清洗 128 个当前 raw head 缺失 `(source_ref,cleaner_version)` 的项目；对 shared source 不改写。使用 anti-join，不仅靠 lastCreatedAt 水位，防止晚提交事务永远漏掉。清洗是确定性本地工作，不调用 Pi / Embedding / OCR。

缓存首建采用 keyset 分页；先服务用户当前选中作品，后台再推进全域。允许只从已索引且合格材料建立部分 Run：返回 indexed 数、待索引数与实际冻结数；预算是上限，不强行凑满；已建 Run 不后补未冻结目标。一个合格目标也可运行，不等全域索引完成。完全没有可选已索引材料且有待索引项时返回 index_pending，不能返回 no_work。

搜索结果附 indexCoverage；索引未完成不得称“搜遍全部评论”。下一阶段自动扫描继续通过 anti-join 与研究历史补上未处理材料，不用成功水位直接跳过尾部。

## 8. 当前有效结果，不重写历史

只读 view `linggan_comment_study_effective_target` 的基本列：content_public_ref、comment_external_id、target_ref、run_ref、source_ref、state、created_at、finished_at。将成功 Target（succeeded/no_signal）按稳定身份分组，选 `created_at DESC,target_ref DESC` 的一条；旧 comment_external_id 为 NULL 时由 raw source JOIN 补身份。先选 head，再应用安全和当前正文一致性，不能在 head 无效后偷偷挑较老结论充数。

后续 failed/needs_context/cancelled 不替代旧成功 head；新的 no_signal 是明确成功结果，应替代旧 succeeded，当前 Signal 变为空。显示批次详情时仍能查全部历史，但不再纳入有效统计与召回。不同方法的反复研究不是新增用户依据。

当前 raw head 与成功 head 的 raw 正文 hash 不同，返回 `effectiveState=source_changed`：旧结果仅作为历史，可查其当时原声，不参与当前用户问题支持。仅语境新增但正文未改，旧依据可留在历史快照口径，显示 contextUpdateAvailable；不能因点赞或新增未使用图片把全部知识失效。

Problem候选目录允许有可读稳定定义的active和support_insufficient，排除merged/retired及定义依据受限者，避免仅因支持暂少就重复建一个同义问题；相同问题判定标准不变。

Signal / embedding / recall / pair / membership 当前统计都使用这一关系，加上各自来源限制与 eligibility。不得只修 UI 计数，后台仍把重研旧 Signal 当新独立依据。

### 8.1 聚合口径

| 字段／显示含义 | SQL 口径 |
|---|---|
| targetCount | 某 Run 所有 Target count，不先 JOIN work 扩行 |
| selectedWorkCount | selection_manifest.requestedWorkRefs 数量 |
| coveredWorkCount | study_work 的 count；未覆盖作品另列原因 |
| semanticProcessedCount | succeeded + no_signal；不含 needs_context/failed/cancelled |
| semanticSettledCount | 全部终态 Target，另分 needs_context/失败/取消 |
| signalCount | 明确 Run 下实际 Signal 行数，结构统计而非人数 |
| distinctCommentCount | 当前有效、可展示的稳定评论复合键去重 |
| distinctAuthorCount | 已知 `(platform,author_external_id)` 去重；未知单列 |
| supportCommentCount | Problem 下有效 membership 关联的稳定评论去重 |
| membershipCount | 内部记录数，不能包装成不同评论数 |
| firstObservedSupportAt | 同 Problem＋稳定评论历史首次合法 membership 时间；能证明才填 |
| addedSupportCommentCount | 时间窗口内 firstObservedSupportAt，且当前仍有效；重研不重计 |
| newProblemCount | Problem.created_at 在窗口内；不代表需求才刚出现 |

新旧数据无法证明某个首次时间时返回 NULL 与 unknownTimeCount，不用 created_at 猜采集或需求发生时间。观察图保持原先的“材料观察量”口径，不与以上知识更新数混画成市场趋势。

## 9. 必要读取索引与迁移顺序

新增读取索引优先使用已经具备的等价索引；实现前检查 pg_index，而不是重复建同义名字。计划索引：Target 稳定身份历史和在途唯一、model_request run/deadline、start_request domain/slot、clean_cache GIN/version、membership(problem_ref,created_at,signal_ref)、Signal(target_ref,created_at,signal_ref)、Run(created_at DESC,run_ref DESC)、原评论(content_public_ref,comment_external_id,created_at,material_ref)。as-of latest 排序中的 observed_at 文本转时间成本先测，不把不满足不可变要求的 cast 强塞表达式索引。

升级用三个逻辑 migration：`comment_study_productization_schema`（表／可空列）、`comment_study_productization_constraints`（确定性身份回填／新写约束／索引）、`comment_study_productization_views`（有效读取关系）。数字前缀在开发 exact HEAD 分配，不在本手册假定 0102 永远未被占用。每个完整 migration ID 与 SHA 按现有 ledger 注册。

迁移不清洗全库、不调用模型、不重跑研究、不 DELETE 原始／派生历史。缓存回填是可暂停的应用过程。建索引／约束失败只回滚对应迁移事务，不把异常当成空库继续初始化。运行脚本当前以一个事务包裹每个 migration，不能在里面使用 CREATE INDEX CONCURRENTLY。

新安装和已有 clean-study 升级是不同路径：已有系统只 delta；空库 proof 先合法建立保留材料和 clean bootstrap，再 delta。不得让新版普通启动脚本因为看不到 study 表就执行旧 reset。具体注册和现场停止点见运行手册。

## 10. 机器可读伴随文件

交付包 `validation/semantic-output.schema.json`、`resolution-output.schema.json`、`pair-output.schema.json` 给出新方法所用的闭集**传输形状**；`start-run.schema.json` 给出HTTP开始命令形状。它们不是新的执行内核，也不说明真实provider已经支持所有结构化输出特性。

实例关系仍由Rust校验：outcome与reason/signals对应、kind与problemFrame对应、basis逐字原文、确切Target和候选闭集、来源权限、revision、长度、独立作者与预算。客户端／provider Schema通过绝不替代这些规则。方法保存时应生成与这些伴随文件一致的完整Schema，不能将§5.1的空对象示意当成可用Schema；伴随文件需作为测试fixture随实现核对，不能另起自由修改的Schema版本。
