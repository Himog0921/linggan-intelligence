# DEC-0008 · 平级 Domain 与统一材料用途链

> 状态: 权威当前
> 最后核对: 2026-09-23
> 适用范围: Domain、Observation Target、采集用途、Corpus 与 Comment Study 的长期领域语义
> 事实来源: Mog 2026-09-23 对 DOMAIN-UNIFICATION-001 的明确决定；当前架构基线与领域不变量
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实代码/数据库/运行结果；本文冻结长期语义，不宣称目标实现已完成

## 决定

1. **正式 Domain 平级。** ADHD、考研自习、自闭症干预和之后正式创建的 Domain 共享同一基础采集、详情、评论、媒体、Corpus 与 Comment Study 能力。Domain 定义研究上下文，不是搜索关键词、能力开关、专用材料表或独立服务。
2. **Topic 仍从属于 Domain。** 领域内部的概念与研究问题是 Topic；关键词只有在观察配置中表达采集入口，不会因搜索或重复出现自动成为 Topic 或 Domain。
3. **Target 保持平台级真实身份。** 一个 creator 或 keyword Target 可以被多个 Domain 使用；每个 Domain 分别决定该 Target 的 `primary` 或 `reference` 用途。未分配 Domain 的候选 Target 不可排活。
4. **role 决定默认研究用途，不决定基础能力。** `primary` 默认进入该 Domain 的自动研究与统计；`reference` 可完整读取，也可由人显式选入研究，但默认不支持该 Domain 的结论。是否采详情、评论、回复、媒体、OCR 或 ASR 继续由具体执行范围决定。
5. **采集目的显式并冻结。** 一个 Acquisition Request 对应一个 Domain 和 role。多个授权、字段、Coverage、新鲜度、保留和风险边界相容的 Request 可以共享一次有界执行；每个目的保留独立用途，首次 claim 后用途集合冻结。已有执行目的不因后来修改 Target 当前关系而变化。
6. **材料身份与 Domain 分开。** 同一来源 Content 在不同 Domain 中保持一个 canonical 身份。每个 Domain 的材料资格来自合格的发现、明确复用或有标识的历史迁移用途；读取不得从当前 Target 关系、旧单值字段、名称或 ADHD 默认值推断。
7. **暂停保留历史、停止新增。** paused Domain 不接受新的采集 Request、未开始 WorkOrder claim、Comment Study Policy 或 Run；历史材料、运行与结果继续可读并显示暂停状态。已 claim 的有界 Attempt 可以完成。
8. **不保留“本领域 / 外部领域”双轨。** reference 不是低权限材料，不能被隔离到另一套 Evidence、Media 或 Comment Study。旧开发投影在迁移时由明确 legacy 用途表达；不能为无来源血缘的材料伪造执行记录，也不建设长期双写、fallback 或兼容 adapter。

### V1 Merge 相容条件

只有同时满足以下条件，新的 Acquisition Request 才能并入既有 WorkOrder：目的文本和有效 `authorization_ref` 完全相同；Target、lane、dispatch lane、冻结的 monitor rule revision、渐进归档模式、估算工作量及两侧具体作品范围完全相同；WorkOrder 仍为 `queued`，且从未有过 Lease。合并只追加该 Request 自己的 Domain/role usage，保留独立 Request 与目的记录。数据库禁止 usage 更新/删除，并在插入时锁定 Target、拒绝已 claim 的 WorkOrder。

Target 行锁串行化 Request 与首次 claim。claim 必须先锁 Target 再锁 WorkOrder；先 claim 成功后到达的 Request 不能并入，即使租约后来释放。它先按正常复用判断；未满足复用时创建自己的 WorkOrder。目的相同是当前 V1 在缺少结构化保留/风险策略字段时的保守兼容边界；新增这类字段后，必须逐项纳入相容比较才能放宽。

## 后果

- 每个 Domain 的读写入口必须显式携带 Domain 范围；无效范围显式失败，不回落 ADHD。
- 同一 Target 可在多个 Domain 具有不同 role；关系变更不重写历史 Request、WorkOrder 或材料用途。
- primary/reference 两类材料走相同的接纳和材料链；差异仅存在于默认研究选择、展示标签和结论资格。
- Content、Comment 与 Media 不按 Domain 复制。复用既有合格材料必须记录用途，不能制造新的采集或 Observation。
- DEC-0006 曾把评论研究范围写为 ADHD 单一领域；DEC-0008 修订该范围为显式 Domain，并保留其来源资格、隐私、Atom 证据与开发期清理要求。
- 本决定不代表迁移、共享数据库、main、runtime、插件、外部平台或业务验收已经完成。实现状态由当前代码、迁移和分层回执证明。
