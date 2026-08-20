# TypeScript V2 → Rust 模块迁移地图

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: V2 参考能力到 Rust 模块的迁移顺序
> 事实来源: 固定 V2 源码、fixture、测试与目标架构
> 冲突时以谁为准: 真实 producer 合同、fixture 和目标 Rust 代码

本地图是 Bootstrap 阶段的迁移假设。DISC-001 完成前不得按本表启动逐模块移植；最终迁移顺序必须由已确认产品工作流、领域边界、采集合同和数据设计共同决定。

| 现役参考 | Rust 目标 | 迁移方式 |
|---|---|---|
| 插件 `protocol/v2` | `crates/contracts` | 保留真实 JSON fixture，Rust runtime validate |
| `evidence/ingress` | Gate 6 决定的 Evidence 接入边界 | 保留 canonical/checksum、capture identity、幂等重放、不同 hash 冲突隔离和包级原子接入；新项目另补安全失败记录、下游 Record/Object 隔离与受限原始载荷策略，不照搬旧表 |
| `evidence/security` | `crates/storage-postgres` + API capability | 重建最小权限和审计，不复制旧角色 SQL |
| V2 execution queue、station sync 与插件 lease/reconcile | Gate 6 决定的 Rust 执行协调边界 | 忠实保留单一执行权、账号约束、claim/lease/fencing/retry/reconcile 和真实并发测试，不把它误称为已具备采集准入 |
| Evidence 复用、等价需求合并、时间价值和资源准入 | 新项目设计；V2 尚无完整已证实实现 | 从 Gate 2–5 的产品、领域、合同和数据语义重新设计，禁止由旧队列字段反推 |
| `evidence/derived` | Gate 6 决定的 Observation / Derived Analysis 边界 | 来源字段解析/格式归一化与 AI 语义分类/评估/解释按 Gate 3–5 资格拆分；不把旧 `derived` 目录整体移植成 Observation，也不保留整个包因单条下游失败而一并失效的通用规则 |
| `evidence/projection` | Gate 6 决定的查询/投影边界 | 保留历史 Observation 与 Current view 分离及旧 Observation 不倒退 Current 的不变量；不照搬“最大 `observedAt` + 整条 Observation 指针”作为全部 Current 语义。迟到材料、解析修订、来源差异、部分 Observation 和字段来源先经 Gate 4–5 收口 |
| `evidence/media` | Gate 6 决定的媒体边界 | 忠实保留 Media identity、origin、slot、usage 的分责；不把媒体降级成 Note JSON URL，也不在 producer 审计前断言它是否为独立 Source Object 或归属某个固定 crate |
| Material V2 provider | `apps/api` 查询服务 | 版本化 DTO、单一致快照、无 fallback |
| 旧 Prisma schema | 只读参考 | 不移植；从已确认领域合同生成新 migration |

## 迁移单元

每个单元按以下顺序完成：

1. 固定真实 producer fixture。
2. 固定旧实现输出 bytes/hash/数据库结果。
3. 写 Rust 失败测试和 PostgreSQL 攻击测试。
4. 实现最小 Rust 模块。
5. 对照输出完全一致。
6. 才允许下一模块依赖它。

不进行逐文件、逐 class 或逐 Prisma model 翻译。

现役 V2 已证明的重点是执行协调可靠性，不是完整的“有限资源信息分配系统”。Rust 迁移时必须把已证明的领取、续租、恢复、幂等与不可证明终态失败路径保留下来；采集准入和需求满足闭环属于新能力，必须另行提供真实 Evidence 复用、合并、过期重评估和数据库副作用证明。

现役 `monitor_checkpoint`、`monitor_patrol` 等 lane 是执行分类，不能作为“比较性观察与探索/调查已经隔离”的证明。Rust 新项目必须从已确认的观察用途、比较条件和最低 Evidence 合同重新设计这条解释边界，不复制历史 lane 名称充当领域语义。

## 中期审查后的禁止直译项

- 现役小红书 producer 对相对时间的近似解析只能作为来源审计样本。Rust 不得把“月等于固定天数”或基于采集时刻推算的值忠实移植为精确 `publishedAt`；新合同必须保留原始表达、来源、参照时间、解析版本和精度。
- `target_mismatch`、来源记录内部身份不匹配、`capture_identity_conflict` 和媒体主体不匹配不能统一映射成一个 Rust 错误；Gate 4 先确认真实触发点和后果，再决定错误合同。
- 现役 `slotReports`、计数、cursor/`has_more`、terminal 和失败只证明 Coverage 来源片段，不能直接移植成一个万能覆盖率字段。
- 现役 CapturePackage 已能包含 `platform_response`、`page_snapshot`、`dom_fragment`、`media_inventory` 和 `context` Artifact；这不要求 Rust 新增一个所有 producer 必经的 Raw Artifact 真相层。哪些 Artifact 必需、怎样保留，必须由真实合同、成本和隐私共同决定。
- V2 接入会在事务前拒绝无效包，未证明“完整无效原始包已持久保存”。Rust 只能先保证安全发生/失败记录和 Coverage 影响；受限隔离区、原始载荷范围和期限等待 Gate 4–5，不能用“不可变”授权永久保存全部字节。
- V2 已证明的包级事务与下游 B2 整包评估语义不能整体直译。新项目冻结“Capture Package 接入原子 + 接纳后 Record/Source Object 隔离处理”；一条下游坏记录不能使其他合格来源记录无故丢失，也不能被整包成功掩盖。
- V2 小红书评论合同以 `noteId + commentId` 为最低身份参考；缺少稳定身份的文本不能在 Rust 中自动创建 `Expression Object` 兜底，只能保持未解析材料或受限 Corpus 片段候选，等待真实合同。
- V2 六个 XHS 合同没有 metric RawRecord，媒体主要以 Artifact/清单和独立责任存在。这既不能证明 Rust 不需要指标/媒体领域模型，也不能授权从旧 Prisma 表复制模型；Comment、Metric 和 Media 的物理身份等待 Gate 4–5。
- 同 capture/同 hash 重放、同 capture/不同 hash 接入冲突、API/页面来源差异、较晚复观测和 parser 更正必须映射为不同语义；不得用统一 upsert、`conflict` 或 Current 覆盖旧历史。
- V2 的 `ContentCurrentProjection` 已证明按 `observedAt` 防倒退和同时间歧义 fail closed，但 Evidence Ingress 尚未落实 schema 预留的客户端时钟偏差/时间来源语义。Rust 不得把客户端 `observedAt`、服务端 `receivedAt`、来源 `publishedAt` 和 parser/analysis 时间压成一个时间，也不得以最后写入或最大时间自动决定全部 Current。
- parser 修正只产生有血缘的新解释；迟到 Evidence 进入原观察时间的历史；Current Resolution 必须保留每个采用值的 Observation/来源陈述与选择规则，来源冲突允许未知。整对象、字段级或混合投影的物理方案等待 Gate 5，不提前创建通用 Resolution Engine。
- 不移植 `Discovered → Resolved → Observed → Changed → Deleted → Unavailable` 万能 Source Object 状态机。发现、身份解析、Observation、派生变化、来源删除、本次不可访问和隐私处置分别落在自己的责任边界。
- 不建立跨用途 Evidence 可信度分数或通用 `Evidence Interpretation` 层。Evidence 对具体 Claim 的适用性在用途、Coverage 和观察条件下判断；来源归一化与 AI 语义分析保持分离。
- 同一来源对象的去重不能消灭 Discovery Finding / 发现记录；同一分析版本也不能只保存模型名而不保存输入集合。
- 现役调用链中的任务或研究上下文不能自动成为 Observation 身份。Rust 需要支持一份合格 Observation 被多个持续观察目标、主动研究问题和 Information Need 复用，并分别保存用途与派生关系；不建立统一 `Observation Intent` 父对象。
- V2 中结果、投影或人工确认不能被翻译成“正式知识为真”。Topic/Term 发布、Derived Analysis、Claim 和 Outcome Evaluation 必须保持资格边界。
- 不移植旧 `Topic`、`TaxonomyNode.parentId/level/aliasKeywords` 或页面层级作为新领域词表。Rust 责任必须保持 Topic Identity、Topic Definition Version、Domain Definition Release、Knowledge Change Proposal、实验词表快照、Classification Run/Human Adjudication、Knowledge Relation 与 Map View 分离；最终持久关系由 Gate 5 决定。
- 不用“最后写入/最大版本”选当前正式词表，也不在新定义发布时自动让旧分类和统计冒充已重算。查询与后续 Rust 接口必须能固定定义/分类版本并诚实返回未就绪；具体模块、作业和错误合同留到 Gate 5–6。
- 旧系统中“页面可见”“内部可读”或某条历史授权不能直接移植成新项目的数据传播资格。Rust 边界必须落实 DEC-01：受限原文内部受控，工作台默认脱敏，外部 Agent 不批量读取；隐私处置传播到索引、向量/特征、摘要、引用、缓存和 Agent 输出，并保留最小审计。
- 旧 API、脚本或 Agent 能够直接调用某项能力，不等于新项目继续授予同等权力。DEC-03 要求外部 Agent 只在当前调用者与用途委托内读取、分析、建议和申请；有后果动作由人授权后仍进入正常知识治理、采集准入或 Decision/Action 链。不得移植直连数据库、直接创建插件任务、共享永久令牌或“批准即成功”的旁路。
- 旧 `OperationPublication`、公开指标和 `leadCount` 字段不能按表移植成 Outcome。DEC-04 要求先证明 Action 与真实平台对象关联、metric producer/同源合同、观察时间与缺失；私信、咨询、销售、访谈和产品行为没有自动来源时保持未知。人工陈述与系统观察必须保留不同来源资格，不得用旧字段制造反馈闭环。

## 插件升级门

现役插件在 DISC-001 中继续作为只读参考和当前 producer 证据，不因 Rust 架构讨论自动获得升级授权。插件升级必须等待 Gate 4 真实合同与 Gate 6 责任边界确认，并至少交付：脱敏真实 fixture、时间/身份/Coverage 负例、服务端有界工作单元、lease/恢复/重试证明、兼容性策略，以及工位/账号与隐私授权。此前只能做只读审计和不改变现役执行语义的验证。
