# DOMAIN-UNIFICATION-001 · 领域平权、统一材料链与领域管理

> 状态: 活跃计划
> 最后核对: 2026-09-24
> 适用范围: 平级 Domain、Domain 与观察目标关系、统一材料/评论/媒体接纳与读取、Comment Study 领域化、领域管理页面、开发期旧模型清理、主线集成与本机上线
> 事实来源: Mog 2026-09-23 最新决定与 2026-09-24 合并部署授权；origin/main@46e108375c1d96f16eed1ee039a66e3866485051、共享数据库迁移账本、runtime-main 身份与实际浏览器；隔离 PostgreSQL 证明；Issue #130 与 #337 的历史事实
> 冲突时以谁为准: Mog 最新明确决定；其次是真实运行、数据库副作用和可复现测试；再其次是当前代码、migration、ACCEPTED ADR 和权威当前文档
> 当前阶段: WP0–WP5 已实施并经完整隔离 PostgreSQL 套件与双轴独立复审；PR #343 已合并为 `main@46e10837`。共享开发库已应用 0103→0104，本机 `runtime-main` 与三个服务已切换到该版本，API 与领域/任务浏览器 smoke 通过。旧 cross 派生的 24 条考研样本与 245 条评论未伪造重投影或重采，迁移前快照保留；考研正式领域当前为 0 篇/0 评论。#338 须基于新 main 调整候选 migration 与 Comment Study 重叠代码；Mog 业务验收、真实平台重采及其数据能力证明尚未发生。原始开发基线=`origin/main@a42315e`；发布代码基线=`main@46e10837`

## 0. 下一位 Agent 从这里开始

本文件是本交付包唯一的开发推进文件。下一位 Agent 必须按以下顺序读取：

1. 仓库根目录 [AGENTS.md](../../../AGENTS.md)；
2. [文档总索引](../../README.md)；
3. [当前状态与事项队列](../../current-state.md)；
4. [Agent 领域文档读取规则](../../agents/domain.md)；
5. 本文件；
6. [领域共同语言](../../context/domain-language.md)、[领域不变量](../../product/domain-invariants.md) 与当前领取的 Work Package 所列直接依赖；
7. 修改 UI 时再读 [UI 执行合同](../../agents/ui-execution-contract.md)、[设计入口](../../design/README.md) 与 [LIDS 入口](../../design/lids/README.md)；
8. 只有实际触碰 SCOPE-001 fixture、合同或其 closed-world 执行面时，才读取并遵守 [SCOPE-001 执行合同](../../agents/scope-001-execution-contract.md)；普通 migration、API 或 worker 修改不能因此被误套 SCOPE-001 的封闭范围。

`docs/agents/domain.md` 与 `docs/context/domain-language.md` 当前仍包含“一级 Domain 只有 ADHD”的旧状态。开工前以本文件和 `docs/current-state.md` 顶部的过渡声明识别冲突；WP0 必须完成长期权威改写，不能让该过渡说明长期存在。

执行时重新 fetch 并记录 exact origin/main。本文件中的 a42315e 是 2026-09-23 的走查基线，不是永久开发基线。

### 0.1 Issue 与协作入口

- 复用 GitHub Issue #130 作为唯一主 Issue，不为 WP0–WP6 新建一串重复 Issue。
- Issue #130 原正文冻结的是旧架构：is_own_domain、跨行业独立表/独立 API、外部材料不是 Evidence。2026-09-24 已将原内容降为 Issue 历史，按当前 DEC-0008 与本计划重写标题/正文并链接权威文件；移除 `ready-for-agent`，改为 `ready-for-human`。首次独立审查指出实施前 Claim 评论缺失，不能倒填成事前 Claim；PR 正文保留实际执行身份与提交证据。
- PR #343 已关联 Issue #130；独立规格与规范审查指出的缺陷已在 `e23ea6f1` 修复并复审，随后合并为 `main@46e10837`，共享库和本机运行已切换。#338 在 #343 后调整集成。
- root 持有 codex/domain-unification-001 集成分支；各 Work Package 可以使用独立 worktree/commit，但最终只形成一个模块级 PR。
- 默认由一个写入 owner 按 WP0 → WP6 串行落到 `codex/domain-unification-001` 专属 worktree；其他 subagent 只读走查。只有 root 明确划定不重叠文件和集成点后，才允许并行写入。
- 其他 Agent 不得自行领取 #130、自行新建派生 Issue、自行增加 subagent 或自行改变并发。
- 所有实施在仓库内 .worktrees/<branch-slug> 的独立 worktree 中完成；共享 checkout、runtime-main 和其他 Agent 的 worktree 都不可直接写。
- 实施 Agent 只实现、测试、提交并交接 exact commit；最终审查、必要返修裁定、主线集成、共享迁移和上线只由 root 完成。

## 1. 用户结果与完成定义

本交付包完成后，用户应看到：

1. ADHD、考研自习、自闭症干预和之后正式创建的 Domain 在基础能力上平权；
2. Domain 只定义研究上下文，不再决定系统能不能采详情、评论、媒体、OCR、ASR、进入 Corpus 或执行 Comment Study；
3. 同一个真实 creator/keyword Target 可以服务多个 Domain，并在每个 Domain 中分别承担 primary 或 reference；
4. 所有 Domain 进入同一套 Material、Comment、Media、Work Resource 与 Comment Study 链；
5. Collection 新增“领域管理”页面，能够创建、说明、暂停/恢复 Domain，并管理 Target role；
6. Corpus 与 Comment Study 显式按 Domain 读取，不串域，不再静默回落 ADHD；
7. reference 拥有完整材料能力，但默认不进入该 Domain 的自动结论与默认统计；
8. 页面按 lane 诚实显示发现、详情、评论、回复、媒体槽位、媒体字节、OCR、ASR，不用一个总完整度百分比掩盖缺口；
9. 新建 Domain 不需要白名单、复制表或新增一套服务；
10. root 在最终审查通过后完成 main 合并、共享开发库迁移、runtime-main 切换和浏览器 smoke。

以下结果不可接受：

- 只把 is_own_domain 改名或把考研自习特殊设为 true；
- 页面统一但 admission、scope、API 或 Comment Study 继续双轨；
- 为新 Domain 新建第三套材料表、媒体系统或评论研究 adapter；
- Target、Content 或 Comment 因 Domain 不同被复制成多个真实身份；
- 双写、长期 fallback 或保留旧 cross API 作为兼容层；
- 把远程 URL 显示成已经本地化的媒体；
- 把 quarantined reply 当作 accepted；
- pause 只隐藏页面但后台继续排新工作；
- 为开发期旧投影建立逐行人工审批、长期迁移状态机或专门的数据保全系统。

## 2. 已确认的产品与领域决定

本节是实施输入，不要求开发 Agent重复向 Mog确认。只有出现新的用户后果、外部平台访问、敏感数据、明显成本扩大或不可逆范围扩大时才升级给 root。

其中，Domain 能力平权、统一采集/研究能力、领域管理、开发期简化以及由 root 最终审查合并上线，是 Mog 的明确决定；Target 可多 Domain、primary/reference 的默认研究规则和 pause 细节，是本计划为避免实现分叉采用的最小默认。实施 Agent 可以验证实现方式，不能把这些默认重新扩展成新产品层。

### 2.1 Domain 与 Topic

Domain 是长期管理的一级研究边界，拥有独立的：

- Observation Target 集合；
- Corpus 范围；
- Comment Study 范围；
- 研究输出生命周期；
- active / paused 运行状态。

Topic 是 Domain 内的概念、问题或内容主题，不因是热门关键词就自动升级为 Domain。

Domain 的判定问题是：

> 它是否需要独立管理观察目标、材料范围和研究输出，而不只是给同一批材料换一个分类标签？

新 Domain 的 V1 字段固定为：

| 字段 | 含义 |
|---|---|
| domain_ref | 稳定 UUID |
| name | 展示名称，不自动成为搜索任务 |
| description | 可空的领域说明 |
| research_goal | 可空的研究目标 |
| status | active 或 paused |
| created_at / updated_at | 创建与最近修改时间 |

本期不增加核心关键词、排除词、Prompt、权限、模板、配色或能力开关。采集关键词继续由 keyword Target 和监控规则表达，避免出现两份执行真相。

现有三个 observation_domain 行全部视为正式平级 Domain；不再存在“唯一 home Domain”。

### 2.2 Domain 与 Observation Target

Target 继续代表一个真实 creator 或 keyword 的全局身份，仍按平台身份去重。当前配置关系是：

    observation_domain_target
      domain_ref
      target_ref
      role = primary | reference
      created_at
      updated_at
      PRIMARY KEY (domain_ref, target_ref)

规则：

- 同一 Target 可以属于多个 Domain；
- 同一 Target 在不同 Domain 中可以有不同 role；
- 未分配任何 Domain 的插件候选可以保留，但不能排活；
- 旧 Target.domain_ref 通过一次 INSERT ... SELECT 建成关系，统一设为 primary；
- 不根据旧 is_own_domain=false 猜测 reference；
- “全都给姐上岸”归为“考研自习 / primary”，不自动添加“ADHD / reference”；
- Request 创建与 Admission 都必须验证当前 Domain–Target relation；Admission 时 relation 不存在或 role 已变化，则旧 Request 不得被静默改写，需按现行关系重新申请；
- V1 不提供删除 Domain，也不把改变当前关系解释为改写历史 Package。

若首个 migration 暂时保留旧 Target.domain_ref 以满足部署顺序，运行时代码必须停止读取它，并在本交付包最后的 cleanup migration 删除。不得留下长期双模型。

### 2.3 primary 与 reference

| 能力/用途 | primary | reference |
|---|---:|---:|
| discovery / detail / comments / replies | 相同能力 | 相同能力 |
| media slots / bytes / OCR / ASR | 相同能力 | 相同能力 |
| Corpus 查看与 Inspector | 可用 | 可用并显示 reference 标签 |
| Comment Study 人工选择 | 可用 | 可显式选择 |
| 自动研究、默认统计、默认结论 | 默认纳入 | 默认排除 |
| Claim 适用范围 | 当前 Domain 主材料 | 仅对照，不自动支持本领域结论 |

role 不决定某一单是否实际采媒体或多少评论。具体范围继续由 WorkOrder scope 中的 comment_limit、reply_expand_limit、acquire_media、allow_ocr、allow_asr 决定。

### 2.4 Request、WorkOrder 与多 Domain 执行范围

一个 Acquisition Request 只表达一个 Domain 下的一次目的，因此冻结单个 `domain_ref + role`。多个相容 Request 可以共享一次必要平台访问，但不能因为 Target 同时关联多个 Domain 就自动继承其他 Domain 的用途或授权。

WorkOrder 通过一张窄关系冻结一个或多个已准入用途：

    collection_work_order_domain_usage
      work_order_ref
      request_ref
      domain_ref
      role = primary | reference
      basis_kind = admitted | merged | legacy_migration
      created_at
      PRIMARY KEY (work_order_ref, request_ref)
      UNIQUE (request_ref)

规则：

- admitted Request 创建 WorkOrder 和第一条 usage；merge Request 只能在目标 WorkOrder 首次 claim 前指向相容 WorkOrder 并追加 `basis_kind=merged` usage，不再发起第二次平台访问。V1 要求 purpose 文本、有效 authorization、Target/lane/dispatch/rule revision、渐进归档模式、工作量和两侧作品范围完全相同；
- 能否 merge 必须比较授权、字段、Coverage、新鲜度、保留与风险边界，不能让一个用途借用另一个用途的权限；
- 相容性查找、决定和 usage 追加必须在同一数据库并发边界完成，防止两个同时到达的 Request 各自创建平台工作；Target 锁串行化 admission 与首次 claim，claim 后不再 merge，即使 Lease 后来释放；
- 一个 WorkOrder 可以有多个 Domain usage，但 Producer 仍只收到一份有界执行指令，不感知 Domain；
- 每条 Request usage 写入后不可改；WorkOrder 的 usage 集合在首次 claim 时冻结。首次 claim 后的新 Request 不得再 merge，已有新鲜 Evidence 可走 reuse，否则另建 WorkOrder；
- role 改动或解除当前 relation 不重写已经 admitted 的 Request usage，但会使尚未 Admission 的旧快照失去资格；
- 同一 Domain 的同一 Content 只要存在一条合格 primary usage，默认研究角色就是 primary；只有全部合格 usage 都是 reference 时才按 reference；
- 该关系只是必要的执行范围，不是 DomainMaterial、工作流引擎或新服务。

V1 对 pause 采用保守边界：未 claim 的共享 WorkOrder 只有在其全部冻结 Domain usage 都处于 active 时才可 claim；任一 Domain paused 就暂缓整单。已经 claim 的 Attempt 有界完成。V1 不增加“按 usage 拆 Lease”的第二状态机。

切换时，既有 Request 从旧 Target.domain_ref 冻结单个 Domain/primary，既有 WorkOrder 建一条 `legacy_migration` usage。旧 Target.domain_ref 为空时，按旧 0041 合同的一次性 home-Domain 语义迁为 ADHD/primary，并在迁移计数中单列；切换后删除这个 fallback，任何新 Request 都必须来自显式 relation。

### 2.5 Source、Content 与 Domain 使用关系

Source Object、Content、Comment 和 Media 的身份不包含 Domain 或研究目的。同一平台作品在多个 Domain 中仍只有一个 canonical content identity。

Domain 使用资格落到一张窄的、append-only 的读取投影；它记录“哪次合格用途让哪个 Domain 可以使用这份 Content”，不复制 Content、Evidence 或 Observation：

    linggan_material_domain_usage
      usage_ref
      content_public_ref
      domain_ref
      role = primary | reference
      basis_kind = accepted_discovery | admission_reuse | legacy_domain_migration
      request_ref       nullable
      work_order_ref    nullable
      package_ref       nullable
      record_ordinal    nullable
      created_at

来源约束：

- accepted_discovery 必须指向 accepted record disposition，并为该 WorkOrder 的每条冻结 domain usage 分别建立 usage；
- admission_reuse 必须指向明确的 reuse decision、Request 和已存在的合格 accepted material，不制造新平台访问或新 Observation；
- legacy_domain_migration 只在切换 migration 中，从当时 `linggan_material_content.domain_ref` 与 `first_package_ref` 一次性搬移旧单值领域事实；它必须明确标为 legacy，不得伪造不存在的 WorkOrder 或 accepted discovery；
- 切换后 Corpus 只读取该 usage 投影，不回落旧字段、Target 当前关系或字符串推断。

新执行血缘为：

    Domain–Target relation
      → Acquisition Request（冻结单个 domain_ref + role）
      → WorkOrder domain usages（合并相容 Request，冻结一组用途）
      → Task / Attempt / CapturePackage / Receipt
      → accepted discovery
      → canonical Content / Detail / Comment / Media
      → Material Domain Usage

详情、评论和媒体深化同一 Content，不重新创造 Domain 身份。Content 可以因多条 usage 在多个 Domain 的 Corpus 中出现，但平台身份仍只有一份。

切换 migration 必须先 seed legacy usage，再删除 `linggan_material_content.domain_ref`、`is_own_domain`、`home_domain_only` 和对应复合外键；不能只删约束却留下隐藏的单 Domain 权威。

除 `observation_domain_target`、`collection_work_order_domain_usage` 和 `linggan_material_domain_usage` 三张窄关系外，不新增 DomainMaterial 聚合根、领域专属材料表或通用 membership 服务。

### 2.6 Domain pause 与显式 Domain

- paused 阻止创建新的 Acquisition Request / WorkOrder、claim 未开始的 WorkOrder，以及创建新的 Comment Study Policy / Run；
- Request 创建和 Admission 验当前 relation；dispatch 只重验冻结 usage 中的 Domain active、授权与既有执行安全闸，不再要求当前 role/relation 与快照相同；
- pause 前已经 claim 的有界 Attempt 允许自然结束，不强制取消；
- Corpus、已有 Comment Study 结果和历史材料继续可读，并显示“领域已暂停”；新 Study Run/Policy 与新采集工作都被拒绝，pause 不等于隐藏或删除；
- Domain Management 同时显示 active 与 paused；
- 新 Target 关系和新工作不能指向 paused Domain；
- 显式请求不存在的 Domain 返回 not-found；paused Domain 保持只读研究可达，任何情况都不回落 ADHD；
- Collection 运维页面可以保留显式“全部领域”视图，但 Corpus 和 Comment Study 不混域；
- 创建 Domain 只写配置，不触发平台访问。

## 3. 2026-09-23 已核实基线

### 3.1 代码与合同分叉

| 层 | 当前事实 | 直接影响 |
|---|---|---|
| Domain schema | [0041](../../../database/migrations/0041_observation_domain.sql) 用 is_own_domain 和 partial unique 强制一个 home Domain | 新 Domain 自动进入降级路径 |
| Target | [0005](../../../database/migrations/0005_collection_observation_target.sql) 保持真实目标全局唯一；后续只加单值 domain_ref | 同一 Target 无法跨 Domain 承担不同 role |
| Admission | 历史 `material_admission.rs` 曾对 External 早退到独立 `cross_industry_admission.rs`；WP5 已移除该运行时代码 | 外部 Domain 不进入统一 Material/Media |
| Deepening scope | 通用 scope 已有媒体/OCR/ASR；cross scope 没有完整媒体权限 | 外部 Domain 没有 media_slots 任务 |
| Work Resource | canonical API 没有完整 Domain scope，外部领域走第二套 sample/comment API | 不能直接删除前端分支，否则会串域 |
| Evidence UI | [evidence_library.js](../../../apps/api/src/local_web/evidence_library.js) 按 isOwn 切 API，外部对象固定 LIST LEVEL ONLY | 已有详情和评论也不展示 |
| Comment Study | [comment_study_source.rs](../../../crates/intelligence/src/comment_study_source.rs) 与 API 固定 ADHD | 其他 Domain 无法完整运行研究 |
| Pause | Domain read 过滤 active，但 scheduler/dispatch 不检查 Domain status | UI 暂停会与后台事实冲突 |
| Collection IA | [collection.rs](../../../apps/api/src/local_web/collection.rs) 固定五个子面 | 新增第六页必须先更新页面合同 |
| Canonical Content | [0044](../../../database/migrations/0044_cross_industry_comment.sql) 给 `linggan_material_content` 增加单值 domain_ref/is_own_domain 与 home-only 约束 | 只移除 CHECK 仍会残留隐藏的单 Domain 权威 |

当前 [领域共同语言](../../context/domain-language.md) 仍声明“一级 Domain 只有 ADHD”。WP0 必须正式替换该决定，不能只改代码。

### 3.2 本机只读数据事实

“考研自习 / 全都给姐上岸”当前真实数据为：

| 事实 | 当前数量/状态 |
|---|---:|
| cross sample | 24 |
| 已接纳 detail | 24 / 24 |
| 已接纳一级 comment | 245，覆盖 24 / 24 作品 |
| reply record | 166，全部 quarantined |
| cover_source_url | 24 / 24 |
| detail payload 中的远程图片候选项 | 152 |
| media_slots Task/Package | 0 |
| 本地 media materialization | 0 |
| 与 canonical linggan_material_content 重叠 | 0 / 24 |

同日共享开发库只读核对还确认：

| canonical 基线 | 当前数量/状态 |
|---|---:|
| linggan_material_content | 1,333 |
| 没有任何 WorkOrder-linked discovery 的 canonical content | 350 |
| 同时存在有 WorkOrder 与无 WorkOrder discovery 的 canonical content | 179 |
| 当前 Domain 来源 | 1,333 行仍依赖 content.domain_ref/is_own_domain 表达 ADHD |

解释边界：

- 24 / 24 / 245 证明旧分叉中真实保存了详情和评论；
- 166 条 reply 的 task_package_contract_mismatch 是失败事实，不能借重构转成成功；
- 24 个封面 URL 和 152 个候选只证明页面当时报告过远程地址，不证明字节已取得；
- 媒体缺口是任务/接纳链没有执行，不是单纯 UI 不读；
- 上述数量是诊断快照，不是本项目必须保全的上线门槛。
- 1,333 / 350 / 179 是切换设计的只读快照；共享库可能继续变化，migration 验收应比较迁移当刻的前后集合，而不是把这些数字硬编码进生产逻辑。

## 4. 开发期数据处置政策

Mog 已明确：项目仍在开发期，架构正确性优先，不为了旧开发数据增加复杂审查门槛。

### 4.1 必须保留

以下对象不可因本项目清理：

- Domain 行；
- Observation Target 的真实身份、生命周期和现行监控规则；
- Runtime Attempt、CapturePackage、Receipt、record disposition 等原始运行事实；
- account、station、plugin installation 等执行配置；
- 已进入 canonical Material 链的既有材料；
- 与本项目无关的用户配置和正式决定。

### 4.2 必需的旧 Domain 搬移与可选重投影

在任何可选 cross 数据重投影之前，先执行一项确定性的 canonical Domain usage seed：

- 对 migration 当刻每一行既有 `linggan_material_content`，把其旧 `domain_ref` 搬到一条 `basis_kind=legacy_domain_migration` 的 usage；
- role 固定为 primary，因为旧 canonical schema 的数据库约束只允许 home Domain；
- `first_package_ref` 作为迁移来源引用保留，但 350 个没有 WorkOrder-linked discovery 的对象不得被伪造 WorkOrder 或 discovery 血缘；
- seed 后逐集合核对：切换前有 Domain 的 canonical content 在切换后都至少有一条等价 usage；然后删除 content 的 domain_ref/is_own_domain 与 home-only 约束；
- 该 seed 是单次 `INSERT ... SELECT` 加约束验证，不建立人工队列、回放服务或字段 fallback。

这一步属于 schema 语义搬移，必须通过；下面对旧 cross Package 的重投影仍是可选项。

现有 accepted profile_discovery、content_detail、comments、author_profile 可以通过现有 canonical admission seam 做一次确定性、幂等重投影。

执行规则：

- 不建立通用回放框架、人工复核队列或永久迁移状态机；
- 只消费原 CapturePackage 与 accepted disposition；
- 166 条 quarantined reply 一律跳过；
- 无法通过当前 canonical 校验的记录保留原 Package，输出按 package kind 汇总的 skipped 数量，不进入人工逐行审批；
- 重投影成功数量需要报告，但不要求精确恢复 24 / 24 / 245 才允许切换；
- 重投影不能从折叠后的 cross 表反造 package_ref/ordinal；
- 重投影不生成 media_slots，不把 detail payload 伪装成 Media Package。

如果实现者证明复用 canonical admission 仍需新增复杂框架，root 直接选择跳过重投影、删除旧投影并以后重新采集，不扩大本项目。

### 4.3 允许删除或重置

统一读写路径通过后，允许在同一开发交付中删除或重置：

- cross_industry_sample / detail / comment；
- cross_industry_creator_sample_observation 及其他只服务旧 cross 投影的关系、view 和 scope；
- collection_work_order_cross_industry_target；
- 只服务旧 cross API/admission/read 的 Rust、SQL、fixture 和测试；
- 引用旧 cross source、且可重新生成的 Comment Intelligence / Comment Study 派生状态；
- 旧外部领域的缓存、搜索索引和统计投影。

历史 migration 文件继续保留，不回写；cleanup 用新的 append-only migration 表达。

### 4.4 不建设的保全机制

本交付包不要求：

- 双写观察期；
- shadow table；
- 旧 API 兼容层；
- 字段级 fallback；
- 逐行人工审批；
- 可逆 down migration；
- 单独的数据保全 PR；
- 为 24 / 245 建专用迁移框架；
- 每删除一张已列明旧开发表再次请求批准。

共享开发库切换只保留四个简单保护：

1. 一次受影响表和行数预览；
2. 一份不进入 Git 的数据库快照；
3. fresh PostgreSQL 与当前 schema 副本升级测试；
4. 切换后的 Domain 隔离、关键 API、runtime 和浏览器 smoke。

回滚不建设 down migration：恢复切换前快照并切回前一 runtime exact revision。

### 4.5 即使开发期也不能做

- 把 quarantined / rejected 记录改写为 accepted；
- 从标题、作者名或字符串相似度猜 Source/Content 身份；
- 把远程 URL 写入 local_asset_path；
- 把未请求、未执行或失败的媒体显示成已取得；
- 伪造 Attempt、Package、Receipt、Disposition 或 Evidence；
- 让 Domain A 读取只在 Domain B 被接纳的材料；
- 部署时自动发起外部平台重采；
- 删除没有列入本节、且无法从当前交付目的推出的表或用户配置。

## 5. 目标架构

    ObservationDomain
      ├─ name / description / research_goal / status
      └─ ObservationDomainTarget(role)
             └─ ObservationTarget（真实身份唯一）
                    └─ AcquisitionRequest（一个 domain + role 目的）
                           └─ WorkOrderDomainUsage（合并后的一组冻结用途）
                                  └─ WorkOrder（一次有界平台执行）
                                         └─ Lease / Task / Attempt / CapturePackage / Receipt
                                                └─ 单一 canonical Material admission
                                                       ├─ discovery / detail
                                                       ├─ comments / replies
                                                       ├─ author
                                                       ├─ media slots / bytes
                                                       └─ OCR / ASR
                                                              └─ MaterialDomainUsage（用途投影，不复制材料）

    Domain-scoped Work Resource
      ├─ Evidence Library / Inspector
      └─ Comment Study(policy + run + source scope)

明确不新增：

- Workspace；
- DomainCapability / DomainWorkflow / DomainProfile 聚合根；
- 每 Domain 一套表；
- 通用 Material 父对象；
- 超出三张必要窄关系的 Domain membership 服务或聚合根；
- event bus / CQRS；
- 第二套 Media 状态机；
- 第二套 Comment Study；
- cross 兼容 adapter；
- “以后也许会用”的权限、Prompt、模板或配置框架。

## 6. Work Package 总表

默认串行顺序：

    WP0 → WP1 → WP2 → WP3 → WP4 → WP5 → WP6

WP2 与 WP3 只有在 WP1 合同和 migration exact head 已由 root 冻结、且文件边界无重叠时才可并行。WP4 在 WP3 API 稳定前只允许读规格，不允许凭猜测实现。

### WP0 · 权威语义与执行合同

用户结果：所有后续 Agent 使用同一组 Domain、role、Evidence 和开发期清理语义。

必须修改：

- 新增长期决定 docs/decisions/0008-peer-domains-and-unified-material-pipeline.md；
- 更新 [当前状态](../../current-state.md) 与 [Agent 领域文档读取规则](../../agents/domain.md)，移除“唯一一级 Domain 是 ADHD”的当前语义；
- 更新 [domain-language.md](../../context/domain-language.md)；
- 更新 [domain-invariants.md](../../product/domain-invariants.md)；
- 更新 Collection、Evidence Library、Comment Study 的页面合同和 UI change manifest；
- 将 [旧跨领域 Corpus UI manifest](../../design/changes/corpus-cross-domain-render-001-ui-change-manifest.md) 降级为历史基线；对其他旧 `cross_industry` / `is_own_domain` 文档命中做一次 `rg`，逐项标为更新、降级或历史保留，不批量改写历史证据；
- 在 Issue #130 明确旧 body 被本计划取代。

必须删除或替换的旧表述：

- “一级 Domain 只有 ADHD”；
- “跨行业材料不是 Evidence”；
- “领域名称就是可直接搜索的一级关键词”；
- “本领域/外部领域决定存哪张表、读哪个 API”；
- “reference 只能列表级读取”。

新的权威表述：

> 所有满足来源与接纳合同的材料都是 Evidence。primary/reference 限制材料在某个 Domain 中的默认研究用途和结论资格，不决定材料是否拥有 Evidence 身份或基础采集能力。

完成判据：

- 现行权威文档只存在一个 Domain 定义；
- 没有新建 Workspace、Capability 或第二套 Material 概念；
- 数据清理授权、上线边界和外部采集边界与本文件一致；
- 项目治理检查通过。

### WP1 · Domain–Target 关系与执行上下文

用户结果：同一 Target 可在多个 Domain 中承担不同 role；相容用途共享一次平台执行，同时保持每个 Domain 的目的与 role。

实施范围：

1. 新增 observation_domain_target；
2. observation_domain 增加 description / research_goal / updated_at，移除 is_own_domain 运行语义和唯一 home 约束；
3. 旧 Target.domain_ref 自动迁移为 primary 关系；
4. Acquisition Request 冻结单个 domain_ref + observation_role；新增 collection_work_order_domain_usage，让 admitted/merge Request 可以共享一张 WorkOrder；既有 Request/WorkOrder 按旧 Target 单值 Domain 做一次可计数的 legacy backfill；
5. 新增 linggan_material_domain_usage；先从旧 canonical content 单值 Domain 做一次 legacy seed，再删除 content.domain_ref、is_own_domain、home_domain_only 与复合外键；
6. scheduler 与 Request/Admission 校验当前 active Domain 和 relation；dispatch 只校验冻结 usage、Domain active、授权与既有安全闸；
7. Comment Study active policy 从全局 singleton 改为每 Domain 最多一个 active policy；
8. 为后续 cleanup 建立明确依赖顺序，不改写旧 migration。

攻击性反例：

- 一个 Target 同时关联 Domain A/primary 与 Domain B/reference；
- A/B 两个相容 Request merge 后只有一张 WorkOrder 与一次平台执行，并保留两条独立 usage；
- 两个同时到达的相容 Request 也不能并发创建两张 WorkOrder；首次 claim 后不能再向该 WorkOrder 追加 usage；
- 修改/解除当前 relation 后，已 admitted WorkOrder 的 usage 不变；新的 Request 按新关系处理；
- 任一冻结 Domain paused 时，共享 WorkOrder 不能 claim；已 claim Attempt 有界结束；
- 无 Domain 的候选不能排活；
- 同一 Content 不因 Domain 不同产生第二身份；
- 旧 canonical content 全部获得明确 legacy usage；无 WorkOrder-linked discovery 的 350 条基线对象不被伪造血缘，也不从 Corpus 静默消失。

完成判据：

- fresh PostgreSQL 全 migration 通过；
- 当前 schema 副本升级通过；
- 所有新增约束有正例与负例；
- migration 当刻的 canonical content 集合与 legacy usage seed 集合一致，旧单值 Domain 列已删除且无 fallback；
- migration 没有逐行人工复核依赖；
- 未切共享库、未部署、未访问外部平台。

### WP2 · 统一 WorkOrder、admission 与媒体能力

用户结果：任意 active Domain 的 primary/reference 都走同一条材料链。

实施范围：

- 删除 material_admission 的 External 早退；
- discovery、detail、comments、replies、author_profile、media_slots、media_bytes 全部进入 canonical admission；
- 所有 Domain 使用 collection_work_order_material_target；
- Domain/role 只信任冻结的 WorkOrder usage，不信任插件 payload 自报；
- 一次 accepted discovery Package 按冻结 usage 集合写入 Material Domain Usage；同域有 primary 时默认 primary，否则 reference；
- acquire_media / allow_ocr / allow_asr 继续由每个 WorkOrder scope 决定；
- 修复 future replies 把 replyExpandLimit 当身份键的问题，但不恢复当前 166 条 quarantined reply；
- producer/plugin 合同没有必要时不修改、不升版本。

攻击性反例：

- reference 与 primary 请求相同 scope 时，产生相同形状的 accepted canonical Material；
- A/B 相容 Request merge 后只有一个 Attempt/Package，但两个 Domain 都得到可追溯 usage；
- 新建 Domain 不需白名单；
- media_slots 成功、media_bytes 失败时保留分 lane 状态；
- reply contract mismatch 不能被 admission 忽略；
- Package accepted、record quarantined、Task completed 继续是三个事实。

完成判据：

- runtime active path 只有一套 admission；
- 新增 Domain 通过隔离 PostgreSQL 完整进入 canonical lanes；
- 无 cross-specific 新写入；
- 插件未被无必要改造。

### WP3 · Domain-scoped Work Resource 与 Comment Study

用户结果：Corpus、Inspector 和 Comment Study 统一工作且不串域。

实施范围：

- Work Resource / Corpus 查询显式要求 domain_ref；
- Domain membership 统一读取 Material Domain Usage；accepted discovery、admission reuse 与 legacy migration 的 basis 清晰可见；
- cursor/hash 包含 Domain；
- 删除 Evidence UI 的 isOwn API 分支与 LIST LEVEL ONLY；
- setup、policy、run、source gate、页面导航和写动作都显式携带 Domain；
- 默认 Comment Study source 只取 primary；
- 用户显式选择 reference 时，run/result 冻结并展示 role；
- Problem、Run、统计继续按 Domain 隔离；
- 不增加 cross comment adapter。

攻击性反例：

- Domain A 的材料不能由 Domain B 的 API、cursor 或 Comment Study 读到；
- 同一 Content 在 A/B 出现时只有一个 content identity；
- 不传或传错 Domain 时明确失败；paused Domain 可读取历史材料并显示状态，不回落 ADHD；
- A/B active policy 与 Run 互不覆盖；
- reference 不进入默认研究，显式选择后可进入且标签可读。
- paused Domain 可读 Corpus 与既有 Run，但不能新建 Policy/Run。

完成判据：

- 所有 Domain 只用共享 Work Resource/Comment API；
- 没有运行时 cross read fallback；
- API、Rust query 和浏览器状态词典一致。

### WP4 · Domain Management V1 与 pause 真语义

用户结果：用户可以在 Collection 中管理正式 Domain，而不是在新建 Target 弹窗里顺手制造“外部领域”。

信息架构：

- Collection rail 增加第六页“领域管理”，位于“观察目标”之后；
- 保持当前 local_web SSR + POST/Redirect，不引入 SPA 或通用后台框架；
- 移除 Target modal 中的第二套新建领域入口，改为选择现有 active Domain 和跳转领域管理。

列表字段：

- 名称、说明摘要、状态；
- creator / keyword Target 数；
- primary / reference 数；
- distinct 作品数、评论数；
- 最近一次 accepted observation。

详情与操作：

- 编辑名称、说明、研究目标；
- active / paused；
- 关联或解除 Target；
- 切换 primary/reference；
- 跳转筛选后的 Targets 与 Corpus；
- 按 discovery、detail、comments、replies、media slots、media bytes、OCR、ASR 展示真实 lane 状态。

明确不做：

- 删除 Domain；
- 能力开关；
- Prompt 编辑器；
- 权限、模板、颜色、图标、排序；
- 单一完整度百分比；
- 自动搜索或自动采集。

页面必须覆盖：

- 正常、有数据；
- 新建后的空 Domain；
- paused；
- relation 为空；
- 统计暂不可读；
- partial / failed lane；
- remote-only media；
- local media；
- reference 标签；
- POST 冲突和数据库不可用。

完成判据：

- 页面文字与 scheduler/dispatch 事实一致；
- 无源数量显示未知，不填 0；
- 远程 URL 不被渲染为本地 asset；
- 1440 桌面真实浏览器通过；其他当前已支持视口不得新增横向溢出；
- LIDS 与 UI 治理检查通过。

### WP5 · 开发期重投影、旧路径删除与 schema cleanup

用户结果：系统不再保留实时 cross 双轨，也不因保全开发数据建立复杂兼容层。

执行顺序：

1. 对受影响表和行数生成一次只读预览；
2. 在 disposable PostgreSQL 运行可选的 accepted Package 重投影；
3. 验证 canonical Domain 隔离与无身份复制；
4. 删除生产代码中的 cross admission/read/routes/view branches；
5. 用新 migration 按依赖顺序 drop 旧 cross 表、view、constraint 和 scope；
6. 重置只引用旧 cross source 的可再生派生状态；
7. 删除只证明旧分叉语义的 fixture/test，改成 Domain 平权反例；
8. 运行 rg，确保旧名字只存在于历史 migration、历史归档和本计划说明。

完成判据：

- 运行时代码无 is_own_domain 能力分支；
- 运行时代码无 cross_industry_* SQL、route 或 object kind；
- active schema 无旧 cross 投影和 cross scope；
- linggan_material_content 无 domain_ref/is_own_domain 单值权威，Corpus 无旧字段 fallback；
- 无双写、无 fallback；
- 重投影若执行，重复运行不新增重复身份；
- skipped 只做聚合报告，不建立人工审批队列；
- 旧历史 migration 未被改写。

### WP6 · root 终审、合并与本机上线

实施 Agent 交接必须包含：

- branch、exact commit、base；
- 修改文件与 migration 清单；
- 自动测试命令和结果；
- 删除/重置的数据范围；
- 可选重投影结果与 skipped 汇总；
- 已证明和未证明边界；
- 明确声明没有 merge、deploy、插件重载或外部采集。

root 的终审方式：

1. 冻结候选 PR exact head；
2. 按本文件做一次类别级完整审查；
3. 对每条 finding 判定成立、部分成立、证伪或证据不足；
4. 一次性给出合并整改清单；
5. 实施 owner 一次修完同类问题；
6. root 只复验整改清单和直接影响，不重新制造无穷审查轮次；
7. 代码提交按 AGENTS.md 再运行一次 commit-reviewer，只审不改，root 对结论逐条裁定；
8. 通过后由 root 合并 main、应用共享开发库 migration、切换 runtime-main 并做浏览器 smoke。

含 destructive cleanup 的上线顺序固定为：

    构建并冻结新 exact revision
      → 停止新调度并排空/停止旧 API、patrol worker、media worker
      → 受影响行数预览与一次切换前快照
      → 应用新 migration
      → 启动新 exact runtime
      → health / API / browser smoke

切换失败时：

    停止新 runtime
      → 恢复切换前快照
      → 启动上一 exact revision

不使用旧 runtime 撞已经 drop 的 schema，也不因此建设双写、down migration 或兼容 API。

上线口径：

    origin/main exact merge
      + shared development PostgreSQL migration
      + API / patrol worker / media worker runtime-main exact revision
      + :3000 health/API/browser

上线不自动包含：

- 外部平台访问；
- 历史 24 篇媒体重采；
- 新插件版本或 Chrome 重载（除非本项目实际修改插件）；
- Mog 业务验收。

若要恢复被删除或未重投影的考研材料，root 在代码上线后另行申请一次有界真实采集授权；不为此增加本项目兼容层。

## 7. 用户表面与状态词典

### 7.1 受影响表面

| 表面 | 本项目变化 |
|---|---|
| Collection rail | 新增领域管理 |
| Domain list/detail | 新表面 |
| Targets list/drawer/new | 从单 domain_ref 改为 Domain relation + role |
| Collection Operations/Tasks/Runtime | 读取冻结 Domain 与 pause 结果，不新造状态机 |
| Evidence Library | 一个 Domain-scoped Work Resource |
| Evidence Inspector | 所有 Domain 共用详情、评论、媒体 lanes |
| Comment Study setup/run/result | 完整传递 Domain 和 role |
| Domain picker / URL | 无效 Domain 显式错误；paused Domain 可读且有状态标记；不回落 ADHD |

### 7.2 状态词典

| 状态 | 精确含义 |
|---|---|
| active | 可建立关系并产生新的有界工作 |
| paused | 不产生/claim 新工作；历史材料仍可读 |
| primary | 默认领域研究材料 |
| reference | 完整可读、可显式研究；默认不进入自动结论 |
| NOT_REQUESTED | 当前 WorkOrder 没申请该 lane |
| NOT_OBSERVED | 没有合格来源证明该 lane 已发生 |
| PARTIAL | 已取得部分材料，Coverage 明确有缺口 |
| FAILED | 有明确失败 Attempt/处理结果 |
| REMOTE_ONLY | 观察到远程候选，没有受控本地 bytes |
| LOCALIZED | 有通过完整性校验的本地 Materialization |
| UNKNOWN | 合同或来源无法确定，不替换为 0、无或完成 |

禁止把这些状态压成一个“完整度 xx%”。

## 8. 验收矩阵

| 场景 | 自动/运行证明 | 通过标准 |
|---|---|---|
| Fresh PostgreSQL | 全量 migration + 核心集成测试 | 从空库建立目标 schema |
| 当前 schema 升级 | 当前 main schema 副本 | migration 与 cleanup 成功 |
| Target 多 Domain | PostgreSQL 正/负例 | 一个 Target 两个 Domain、不同 role |
| 多用途单次执行 | 两个相容并发 Request 的 admission/merge proof | 一张 WorkOrder、一次 Attempt/Package、两条独立 Domain usage；首次 claim 后 usage 冻结 |
| WorkOrder 快照 | PostgreSQL 并发/更新反例 | 后改/解除 relation 不改变已 admitted usage；新 Request 使用新关系 |
| 共享 WorkOrder pause | dispatch 反例 | 任一冻结 Domain paused 时不 claim；已 claim Attempt 有界结束 |
| 新 Domain 平权 | API + PostgreSQL | 无白名单走完整 canonical lanes |
| primary/reference 能力 | 同 scope 对照 | 接纳形状一致 |
| 自动研究默认 | Comment Study source 测试 | reference 默认排除 |
| reference 显式选择 | Run freeze 测试 | 可加入并保留 role |
| Content 身份 | 两 Domain 同作品 | 一份 canonical identity、两份 Domain 使用血缘 |
| legacy Domain seed | 当前 schema 副本前后集合核对 | 每个旧 canonical content 有等价 legacy usage；无伪造 WorkOrder；旧单值列已删除 |
| Corpus 隔离 | API A/B + cursor 反例 | A 不读 B，cursor 不跨域 |
| Comment Study 隔离 | setup/policy/run/source A/B | policy、selection、result 不串域 |
| pause | scheduler + dispatch + Comment Study write fence | 无新 Request/claim/研究写入；Corpus 和既有结果可读；已 claim Attempt 自然结束 |
| reply quarantine | admission 负例 | 166 历史记录与同类失败不变 accepted |
| media truth | API/UI/PG | URL、slot、bytes、materialization 分责 |
| Domain Management | 真实浏览器 | create/edit/pause/relation/role/空态/失败态可用 |
| 旧路径清理 | rg + schema inspection | 无 active cross write/read/fallback |
| 可选重投影 | 两次运行 | 不复制身份；skipped 聚合可读 |
| 治理 | diff/check scripts | 索引、进度、链接和状态头通过 |
| 本机上线 | main/migration/runtime/health/browser | exact revision 一致、页面可用 |

建议最低命令集由实施 Agent按实际改动精确化，至少包含：

    git diff --check
    cargo fmt --check
    cargo test --workspace --locked
    ./scripts/test-local-001-discovery-postgres.sh
    ./scripts/check-project-governance.sh origin/main
    ./scripts/verify-ui-design-handbook.sh

若插件没有改动，不运行发行包重建来制造无关工作；若实际修改插件，则补齐其 production build、测试、release verify 与可复现构建。

## 9. 停止与升级条件

实施 Agent只在以下情况停止受影响 Work Package并交给 root：

1. 必须引入本文件明确禁止的新抽象才能继续；
2. 现有 Package/WorkOrder 血缘无法提供 Domain 隔离，且有可复现 SQL 证据；
3. migration 会删除本文件未列出的不可重建用户配置或 canonical 材料；
4. 需要真实平台访问、账号切换、验证码处理、额外成本或第三方模型原文处理；
5. UI/API 出现两种会产生不同业务后果、且本文无法裁定的实现；
6. 发现其他活跃 worktree 正在修改同一 exclusive 文件并会形成真实冲突；
7. 当前 origin/main 已改变目标 schema/合同，继续会基于失效前提。

普通技术选择、局部重构、测试修复、可再生开发投影清理和本文已列 migration 不需要重复找 Mog批准。

## 10. 文件所有权与交接要求

root 派工时为每个 WP 写出 exclusive / shared / forbidden 文件。默认边界：

| WP | 主要 owner |
|---|---|
| WP0 | docs/context、docs/product、docs/decisions、相关页面合同 |
| WP1 | database/migrations、observation_domain、collection_target、acquisition/work-order/scheduler/dispatch |
| WP2 | material_admission、scope、execution eligibility、media/reply contract |
| WP3 | Work Resource query/API、Evidence JS、Comment Study Rust/API |
| WP4 | Collection Domain Management SSR/CSS/JS、LIDS manifest/tests |
| WP5 | cross modules/routes/schema cleanup、fixture 与 migration ledger |
| WP6 | integration branch、PR、shared DB、runtime-main、浏览器 |

shared 文件必须由 root 指定 integration owner；实施 Agent不得因为“顺手”吸收别人的未提交改动。

每个 WP 的交接格式：

    Work Package:
    Base / head:
    Changed:
    Migration:
    Tests:
    Data reset/reprojection:
    Proved:
    Not proved:
    Conflicts/risks:
    Next exact entry:

## 11. 分层完成台账

不得用一个总完成率。root 在本文件持续更新：

| 层 | 当前状态 |
|---|---|
| 产品决定 | 已确认 |
| 活跃计划 | 已建立 |
| Issue #130 同步 | 已完成旧架构正文降级与当前交付同步；事前 Claim 评论缺失已如实记录 |
| WP0 权威合同 | 已完成（DEC-0008、共同语言、不变量、页面合同、UI manifest、索引与进度已同步） |
| WP1 schema/context | 实施与隔离 PostgreSQL 证明完成（0103 additive migration、Domain–Target relation、Request/WorkOrder/Material usage；多 primary 拒绝任意择一；Request admission 与 lease claim 对 Domain status 持行共享锁至提交，pause 与新写入线性化；Merge 按授权、目的、执行边界和未 claim 状态匹配；数据库锁定首次 claim 并保护 usage 不可变；Target 锁覆盖 Request/首次 claim 并发边界；有持锁屏障证明） |
| WP2 unified admission/media | 实施与隔离 PostgreSQL 证明完成（统一接纳及 Domain usage；归档与精确材料补采 API 显式传 Domain；目标抽屉将 Domain 传到关键词基线/详情、创作者缺口和分批建档；Merge 共享 WorkOrder 并记录独立用途；scope/merge/reply/media lane 由完整 LOCAL-001 隔离证明覆盖） |
| WP3 Corpus/Comment Study | 实施与隔离 PostgreSQL 证明完成（Corpus list/detail/comments/cursor 按显式 Domain；移除 isOwn UI/API 分支与 LIST LEVEL ONLY；未选 Domain 显示选择提示且不发语料请求；Comment Study setup/policy/run/source 显式 Domain，reference 预览后显式纳入，Run/运行记录/评论目标/信号投影冻结 role；策略保存与 Run 创建对 active Domain 持共享行锁至提交，暂停竞态不能落新研究写入；旧 cross 运行路径由 WP5 清理）。独立审查发现的无 Domain 兼容发现卡 HTTP 入口已在 main 移除，运行时返回 404。 |
| WP4 Domain Management | SSR 页面与配置/关系操作、错误反馈已实现；reference 关联和 role 在同一事务提交，Target 行锁串行化关系变更与 Request；8 个材料通道状态按来源事实分别汇总；隔离 PostgreSQL API 路由与 2026-09-24 浏览器交互证明通过：配置创建/编辑、暂停/恢复、同一 Target 多 Domain 不同 role、冲突无写入；本机部署后 1440×1000、390×844 与 1085px 的 DOM 精确测量均无横向溢出。数据库不可读浏览器态、非空媒体 lane 与 Mog 验收仍未做 |
| WP5 reset/cleanup | 实施与隔离 PostgreSQL 证明完成。0104 cleanup migration 已登记至 fixtures 与本机迁移链；canonical legacy usage 只从旧 `content.domain_ref`/`first_package_ref` 事实 seed；旧 cross sample 没有 canonical Package 血缘时不会借用 Content 的首页 Package 冒充 peer 来源；可再生旧样本投影按合同清理/重投影，缺少替代事实的用户笔记会阻止清理；运行时代码不再引用旧 cross admission/read/routes/worker 分支。首轮当前 schema 副本升级发现 grant-attempt FK 阻止孤儿会话清理后，0104 改为保留 grant/risk 审计、解除可空会话引用并退役 session-scoped preparation；当前另保护 orphan session、session-free 旧 scope 的活跃 lease 和无提交回执 preparation，且锁定 scope 写表至迁移事务结束。|
| 自动检查 | 修复后完整 `./scripts/test-local-001-discovery-postgres.sh` 退出 0（含新增双 Domain 冻结任务用途与暂停状态回归；隔离资源清理确认）；API 非数据库单测 269 passed / 0 failed / 29 ignored，隔离 API 29/29、派发任务序列 36/36、worker 隔离 3/3。`cargo check --locked --workspace --tests --quiet`、定点 `rustfmt --check`、`node --check`、`git diff --check`、项目治理和 UI design handbook 检查通过。此前当前 schema 副本 0102→0103→0104 通过、源库未写；全仓 `cargo fmt --all -- --check` 仍被未修改基线文件的格式漂移阻断 |
| Fresh PostgreSQL | 完成：完整 LOCAL-001 disposable PostgreSQL 套件通过，包括 migration 全链、跨 Domain 并发合并及冻结用途、WP1–WP5 关键正反例、领域管理 API、worker 与调度证明；临时资源清理已核实 |
| 当前 schema 升级 | 历史隔离副本 0102→0103→0104 已通过（当时 1,352 Content）；随后实际共享开发库从 0102 升至 0104。共享库迁移前为 1,362 Content、24 个旧 cross sample/detail、245 条旧评论，且无活跃不可投影旧 scope 或未回执 preparation；迁移后 1,362 Content 均有 `legacy_domain_migration` usage、缺失 0，旧 cross 表和 canonical 单值 Domain 列在 `public` 下为 0；24 个孤儿 session 与 72 条 preparation 退役，212 条 grant attempt 保留，其中 24 条解除旧 session 引用。迁移前有效快照位于本机 runtime-backups（详见月度进度）。 |
| Root 合同/标准复核 | 首次独立审查在 `c56486ca` 指出跨 Domain 旧读取入口、任务用途可见性、暂停原因码和 CSS token 缺口；修复后规格轴复审未发现可复现缺陷。规范轴复审再指出固定列宽，已改为可收缩网格并完成最终只读复验，无剩余本轮代码 blocker。Issue 事前 Claim 缺失作为历史事实保留；#343→#338 技术顺序已确定，#338 的候选 migration/读取适配留在后续集成。 |
| PR | #343 已合并，merge commit `46e10837`；关联 `Refs #130`。GitHub checks 数为 0；独立规格/规范审查与本地验证另有记录，不冒充平台 check 或 Mog 验收 |
| origin/main 合并 | 已执行：`46e108375c1d96f16eed1ee039a66e3866485051` |
| 共享开发库 migration | 已执行：0103、0104 均在共享 `linggan_intelligence_dev` 账本，hash 与文件一致；已制作迁移前快照 |
| runtime-main 上线 | 已执行：本机安装后运行身份为 `46e10837`、migration head 0104；API/调度/媒体三个 launchd 服务运行，健康页 DB READY |
| 浏览器/UI | 隔离交互验收见 `docs/design/acceptance/domain-unification-001-acceptance.md`；部署后领域管理和任务页真实浏览器 smoke 通过，390/1085/1440 视口 `scrollWidth === innerWidth`。Evidence 与 Comment Study 的跨 Domain 真实运行数据、数据库不可读和非空媒体 lane 未验证 |
| 插件 | 本次未改插件代码或重载浏览器插件；不能据此证明插件端已跟随新运行合同 |
| 外部平台重采 | 不在自动上线范围 |
| Mog 业务验收 | 未执行 |

## 12. 直接参考

- [领域共同语言](../../context/domain-language.md)
- [领域不变量](../../product/domain-invariants.md)
- [Agent 领域文档规则](../../agents/domain.md)
- [统一 Work Resource 读取合同](../../architecture/work-resource-read-contract.md)
- [媒体生命周期合同](../../architecture/media-lifecycle-contract.md)
- [Collection 页面合同](../../design/pages/collection-workspace-page.md)
- [Evidence Library 页面合同](../../design/pages/evidence-library-page.md)
- [Comment Study 页面合同](../../design/pages/comment-study-rebuild-page.md)
- [领域管理页浏览器验收](../../design/acceptance/domain-unification-001-acceptance.md)
- [Issue 与协作规则](../../agents/issue-tracker.md)
- [Agent 协作协议](../../governance/agent-collaboration.md)
- [本机 runtime 发布 runbook](../../runbooks/local-runtime-deployment.md)
