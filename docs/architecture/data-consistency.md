# PostgreSQL 一致性、事务与可证伪验收

> 状态: 草案
> 最后核对: 2026-08-20
> 适用范围: DISC-001 Gate 5 的原子边界、并发收敛、重放、迟到、隐私阻断、异步传播和 PostgreSQL 16 验收场景
> 事实来源: `data-architecture.md`、`data-relations.md`、领域不变量、Gate 4 已确认 Attempt/Package 关系
> 冲突时以谁为准: 用户最新确认、父级数据架构、权威共同语言、ACCEPTED ADR、真实 producer/fixture/测试与数据库副作用

本文是数据架构的第三层渐进披露：

```text
data-architecture.md
为什么这样分
        ↓
data-relations.md
谁与谁相连
        ↓
data-consistency.md
失败与并发时怎样仍然守约
```

本文不是 SQL、migration 或实现授权。每条“必须证明”以后都要转成 PostgreSQL 16 集成测试；当前只冻结候选原子责任和失败语义。

## 总原则

1. **事务只保护数据库能够共同决定的事实。** 不把浏览器访问、对象存储上传、第三方模型或平台发布假装成 PostgreSQL 事务的一部分。
2. **先写不可丢失的事实与 durable work，再让 worker 异步处理。** 页面成功、内存队列和日志不是 durable handoff。
3. **幂等来自业务唯一键、状态前置条件和内容 hash。** 不依赖“请求通常只来一次”。
4. **失败边界跟业务责任一致。** Package envelope 失败可以拒绝整包；接纳后的单 Record 归属失败只隔离对应范围；隐私阻断先同步生效，派生清理可异步完成。
5. **所有完成声明都必须有数据库副作用。** 返回 `ok`、worker 结束、编译通过或页面出现卡片，不能替代权威行、约束和回执。

## PostgreSQL 使用策略

- 默认使用短事务、foreign key、unique、check、条件更新和必要的行锁；不把全库切成高成本的全局串行执行。
- 对“同一业务身份只能有一个结果”的竞争，优先使用唯一约束和 `INSERT ... ON CONFLICT` 类原子收敛，而不是先查后插。
- 对“只有当前版本持有者可以切换”的竞争，使用 version/etag 前置条件或锁定单一指针行；最后写入不自动获胜。
- 需要跨事务异步传播时，事实与 durable work/outbox 同事务写入；worker 至少一次执行，消费者按幂等键收敛。
- 不把数据库隔离级别当万能修复。每个边界明确竞争对象和不变量后，再选择条件更新、行锁、延迟约束或必要的更强隔离。

## 一、Work Order 领取与 Attempt 建立

### 必须共同成功

1. Work Order 仍处于可领取前置状态；
2. Admission 所依据的 Need、Authorization revision、期限与资源边界仍适用；
3. 没有排斥性的等价执行已经占用同一工作；
4. 生成新的 Execution Attempt 和服务端 Capture Identity；
5. 写入工位/账号 lease、开始条件和必要审计。

### 不能共同假装成功

- “曾经批准”不能替代当前 Authorization；
- “排队中”不能阻止已有新 Evidence 使任务过期；
- lease 续期不创建新 Attempt；
- 租约失效后的重新准入必须创建新 Attempt，旧 checkpoint 不改挂。

### 并发结果

两个工位同时领取时只能一个获得有效 lease/Attempt；失败方得到明确竞争结果，不创建一个稍后会偷偷执行的影子 Attempt。

## 二、Package 接入事务

### 事务内必须完成

1. 锁定或原子验证 Attempt/Capture Identity 的接入前置条件；
2. 验证 authority fence。具体是“接收时仍有效”还是“在获准执行窗口内产生且在允许交付期限内提交”，必须由 Gate 6 执行合同明确，不能使用事务外布尔查询；
3. 对 canonical bytes/hash 做首次、replay 或 conflict 判定；
4. 验证目标、producer/contract version、Package envelope 与允许的成员边界；
5. 写入冻结 Package、Record/Artifact 元数据、Coverage/终态来源事实；
6. 写入 Ingress Delivery Occurrence、receipt/outcome 和必要安全审计；
7. 写入下游 Record 处理、对象存储确认或解析所需 durable work。

任一步骤失败时，不得出现“Package 已接纳但成员/coverage/receipt 缺失”的半包。对象存储字节若在事务前上传，数据库失败后进入可回收 orphan 状态；若数据库先登记上传意图，只有 hash 已验证和持久化回执完成后才能把 Artifact 标为可用。

这里不把每条 Record 的最终解析、身份收敛、Observation 或 Corpus/分析资格塞进同一接入事务。接入事务只保证冻结 Package、可枚举成员、Coverage、回执和后续工作没有半包；逐 Record 处理结果由 durable work 在自己的事务中追加。否则一个慢解析、模型调用或未知平台字段会重新把全部合格原料锁成整批连坐。

Package 级硬门应保持最小且可重复验证：authority、Attempt/Capture Identity、目标/合同、canonical/hash、manifest/成员边界和最小 Record/Artifact envelope。若这些失败，只留下最小安全失败审计，完整原始载荷是否隔离由已确认保留政策决定；系统不承诺永久保存无法验证身份或成员边界的任意字节。若硬门通过，后续对象身份 unresolved、来源关系冲突或 parser 错误只影响相应 Record 的处理回执，不撤销 Package 中其他成员。

### 部分执行不是半包接入

目标 100 条、实际取得 50 条时，终态 Package 可以完整提交：

```text
50 条实际 Record
+ 同单位的 attempted/emitted/failed/not_attempted/unknown 来源事实
+ 风险或页面异常停止原因
```

Package 本身仍以这次真实交付为单位原子接入。它“不完整”是相对于 Work Order 目标的 Coverage 语义，不是数据库只写了一半 Package。50 条中各 Record 后续是否能解析到 Source Identity/Observation，再按下游边界决定。

### replay 与 conflict

- 同 Capture Identity + 同 canonical hash：返回同一业务接纳结果，追加或聚合必要 Delivery/replay 审计，不创建新 Evidence/Observation；
- 同 Capture Identity + 不同 hash：记录安全冲突，冻结下游，不用后来内容更新已接纳 Package；
- 新 Attempt：使用新 Capture Identity，即使目标相同也不是 replay；它是否属于有意复观测由业务计划说明。

## 三、Record 处理与 Source Identity 收敛

### Record 处理

Package 接纳后，每个 Record 拥有独立处理回执：待处理、已解析、身份 unresolved、关系冲突、隔离或可重试等具体责任；不使用整包 `success` 推断全部 Record 已形成 Observation。

“独立处理回执”不是一套所有模块共用的万能状态机：网络 replay 记录在 Ingress Delivery；多个 Record 指向同一现实对象由 Source Identity/Discovery 关系表达；隐私隔离与 parser 待修复也不能共享一个无期限 `quarantined` 桶。每个结果必须由拥有该责任的模块保存原因、版本、访问边界和下一步。

一条 Record 失败时：

- 保留原 Accepted Evidence 和失败原因；
- 只回滚该 Record 本次下游事务；
- 不删除同包其他合格 Record；
- 不把解析失败伪装成 producer 没有返回；
- parser 修复后产生新的 Interpretation Revision/处理回执，不更新旧失败历史。

### Source Identity 并发

两个 worker 同时解析同一个业务键时：

```text
(source_system, identity_namespace, source_object_type, stable_external_id)
```

唯一约束只允许一个 Source Identity。失败方读取已存在身份并验证类型/namespace 一致；若来源字段冲突，进入 identity discrepancy，不把冲突记录强连到对象。

Source Identity 与 Author/Content/Comment typed anchor 在同一短事务建立，避免 Registry 已存在但类型锚点永久缺失。身份合并、拆分和别名使用专用治理事务，普通 ingestion 无权更新稳定主键。

## 四、Observation 追加与迟到材料

Observation 事务必须同时写入：

- 匹配类型的 Source Identity；
- 类型化 Observation/Assertion；
- exact Evidence Record/Artifact 使用关系；
- observed/source time 原文、精度和 Context；
- 本 Record 的处理回执；
- 需要重算 Current、检索或分析资格的 durable work。

Observation 只追加，不在此事务中覆盖旧 Observation。迟到材料使用原 observed/source time 进入历史；`received_at/accepted_at` 只决定系统处理顺序。Current 是否变化由后续 Resolution 决定。

如果 Observation 写入成功而异步工作投递失败，说明事务边界错误；durable work 必须与 Observation 同事务持久化。

## 五、Current Resolution 与发布

### 构建阶段

Resolution Run 固定规则版本、输入水位和对象范围。计算可以在事务外或 worker 中分批完成，候选 projection revision 在完整前不可被读取为 active。

### 发布事务

1. 锁定或条件验证当前 active pointer/version；
2. 确认候选 revision 完整且所有来源 FK 有效；
3. 重新应用当前 Privacy/Access 阻断；
4. 检查人工 Adjudication revision 没有在构建后变化，或明确接受固定输入水位；
5. 原子切换 active projection pointer；
6. 写入发布回执和下游投影刷新 work。

两个 Resolution 同时发布时，后提交者不能仅凭较晚完成覆盖前者；它必须基于预期 active version 成功条件更新，否则重新评估。新的 Observation 在输入水位以后到达不使已完成 revision 失真，只触发下一轮候选。

### 隐私竞争

当前读取始终经过权威 access/disposition gate，不能只相信 projection 内的旧可见标志。因此即使 Privacy Disposition 与 Current 构建同时发生，处置事务成功以后新读取立即停止返回原文；projection 后续重建负责移除过时引用。

## 六、Domain Definition Release

Release 构建时冻结 exact Topic Definition、Term relation 和 Knowledge Relation revisions。发布前验证：

- 全部成员已 finalized 且属于目标 Domain；
- broader/narrower 等层级关系没有自环或循环；
- Knowledge Change Decision 指向同一 manifest/hash；
- Release 没有混入实验词表或未确认 Candidate；
- 影响范围与需要重分类/重建的读取模型已登记。

发布事务原子写入 Release manifest、Decision 关系、active release pointer 和 durable recomputation work。新 Release 成为当前不表示分类/统计已就绪；就绪状态由各自 Run/Projection 回执证明，页面不能提前拼接。

## 七、Analysis Run 与 Claim Revision

### Analysis Run finalization

Input Set 先冻结 exact members 或内容寻址 manifest。分析结果可以分批写入 run-scoped staging/typed results，但正式 finalize 必须验证：

- 输入 manifest/hash 与计数匹配；
- 模型、规则、提示、代码、Definition/Vocabulary 版本完整；
- 计划分片都有终态和 Coverage；
- 失败、丢失和 unknown 被记录；
- 输出与输入引用没有越权/已处置材料。

失败 Run 的部分结果可以用于诊断或继续研究，但不能被 Claim、Brief 或正式统计当作完整结果。finalize 与正式可引用指针/回执原子切换。

### Claim Revision

Claim Revision 的主张、范围、时间、支持、反例、限制、不能证明什么和 exact basis 共同冻结。可以先在草稿区构建，但 finalized revision 不能缺一半 basis。后续支持或反例产生新 revision/assessment，不更新一个 confidence。

Decision 或 Brief 只引用 finalized exact revision；Claim 没有全局 truth/current 开关。隐私处置使 basis 失效时，旧 Revision 不消失，但当前使用 gate 标记受影响并要求重评估。

## 八、Decision、Action 与 Outcome

Decision 事务固定 exact proposal/request/plan revision、当时依据、Actor/Delegation、决定结果和条件。若决定产生 Authorization 或 Action Plan 创建权，相应 durable work/授权记录同事务写入；不能只写 `approved` 再依赖内存继续。

现实 Action 不进入 Decision 事务：

```text
Decision
→ Action Plan + Expected Outcome
→ Action Attempt
→ qualified external Action Occurrence
→ Outcome Observation
→ Evaluation
```

发布 API 返回、用户点击和 worker 完成分别只能证明自己的步骤。Outcome 使用来源业务键/idempotency key 防重复；迟到结果按实际观察窗口进入历史，不回填 Expected Outcome。尚未接入的私信、咨询、销售、访谈和产品行为不创建零值结果。

## 九、Privacy Disposition 与传播

### 同步事务

1. 写入授权者、理由、范围和目标的 Disposition Event；
2. 写入或切换权威 access block，使新读取立即失败关闭；
3. 保存不含敏感原文的最小 tombstone/hash；
4. 为所有已登记派生消费者写入 durable propagation work；
5. 写入可查询的处置回执。

### 异步传播

消费者分别处理对象存储、结构化敏感字段、Fragment、全文索引、embedding/feature、Transformation、缓存、Material Pack、Agent/Brief 引用和其他已登记派生物。每个消费者使用处置事件 ID 幂等，回写完成、失败、重试和无法自动处理的原因。

系统只有在全部强制消费者完成或形成明确人工处置决定后，才能声称“传播完成”。单个 worker `completed` 不能代表全链删除完成。历史 Claim/Brief 保留非敏感影响说明，当前读取不再返回被撤回原文。

平台来源暂时不可访问或明确删除只形成来源 Observation/Assertion，不触发 Privacy Disposition；反过来，系统隐私处置不伪装成平台删除。

## 十、数据库权限最小边界

第一阶段仍是一个 PostgreSQL 主数据库和模块化单体，不为每个模块建立独立数据库。最低权限原则为：

- migration/schema owner 与运行身份分开；运行身份无 DDL 权限；
- 普通应用读取不能直接取得受限原始 Artifact/正文；原文读取通过有用途、Actor/Delegation 和审计的窄接口；
- Evidence ingestion 只能写入其负责的 append-only/接入区域，不能发布 Topic、改 Claim 或创建业务 Action；
- projection/worker 只能写自己的派生区域，不能修改 Evidence；
- 隐私处置使用独立高权限路径并保留审计，普通业务 `UPDATE/DELETE` 不能绕过；
- 对象存储凭证按读、写、删除和路径范围分离，不把数据库权限当 blob 权限。

具体 PostgreSQL role 名称、RLS 是否需要和池数量留到 Gate 6 威胁模型；第一阶段不因单用户就让运行账号拥有 schema owner 或任意原文读取。

## 十一、PostgreSQL 16 可证伪验收目录

下面是实现前的测试合同候选。每个测试必须同时检查 API/服务结果和数据库副作用，不能只断言 Rust 返回值。

| 编号 | 场景 | 必须看到的数据库结果 | 一旦出现即失败 |
|---|---|---|---|
| DB-P01 | 两 worker 并发领取同一 Work Order | 只有一个有效 Attempt/lease | 两个插件都获得有效执行权 |
| DB-P02 | 同 Capture Identity、同 hash 重传 | 一个 Package/Evidence 结果，replay 可审计 | 重复 Observation 或成员重复 |
| DB-P03 | 同 Capture Identity、不同 hash | 原 Package 不变，冲突记录存在，下游冻结 | 后一包覆盖前一包 |
| DB-P04 | authority 在接入边界竞争失效 | 按同一原子 fence 全部接纳或全部拒绝 | 越权半包或先查后写 |
| DB-P05 | 目标 100、实际合格 50 后风险中断 | 50 条 Record 与真实 Coverage 同包存在 | 整批丢失，或宣称完成 100 |
| DB-P06 | 包接纳后两条 Record 身份冲突 | 其他合格 Record 继续，冲突两条隔离 | 整包回滚或冲突记录变 Observation |
| DB-P07 | 两 worker 并发解析同一来源 ID | 一个 Source Identity 与匹配 typed anchor | 重复对象或错类型锚点 |
| DB-P08 | 迟到 Observation 比 Current 历史更早 | 历史追加，Current 不按入库时间倒退 | 丢弃迟到 Evidence 或覆盖当前 |
| DB-P09 | 两个 Current revision 并发发布 | 一个完整 active revision，失败方重评估 | 半新半旧或最后写入获胜 |
| DB-P10 | Current 构建与隐私处置并发 | 处置提交后读取立即阻断 | 旧 projection 继续泄露原文 |
| DB-P11 | 发布含循环 Topic 层级的 Release | Release 不成为 active，旧 Release 保持 | 部分发布或循环进入当前 |
| DB-P12 | 新定义发布、重分类未完成 | active definition 可见但统计明确未就绪/旧版 | 新标题配旧统计冒充完成 |
| DB-P13 | Analysis 分片缺失后尝试 finalize | Run 保持不可正式引用，缺口可见 | Claim 引用不完整 Run |
| DB-P14 | Claim basis 写到一半失败 | finalized Revision 不存在 | 正式 Claim 缺支持/反例成员 |
| DB-P15 | Decision 成功、外部发布失败 | Decision/Plan/Expected/Attempt 真实存在，无 Occurrence/Outcome | `approved` 被写成已发布 |
| DB-P16 | 同一 Outcome 重复提交或迟到 | 幂等一个来源结果，真实时间保留 | 重复计数或回填事前预期 |
| DB-P17 | 隐私传播某消费者失败 | 读取已阻断，整体仍未完成且失败可恢复 | 报告删除完成但索引/缓存仍可读 |
| DB-P18 | Source Identity 合并/拆分请求走普通 ingestion | 普通路径拒绝，历史身份不被覆盖 | ingestion 修改主键或无痕合并 |

## 十二、Gate 5 一致性退出条件

只有满足以下条件，本文才可从草案进入确认：

1. Package authority fence 的有效时间语义在 Gate 6 前确定，不留下“合法执行因网络迟到被误拒”或“过期授权仍可写入”的双向漏洞；
2. Source Identity、Current、Definition Release、Analysis finalize、Decision 和 Privacy 的竞争对象与原子前置条件明确；
3. DB-P01–P18 都能在无业务表阶段先转成可执行测试计划，物理实现后使用真实 PostgreSQL 16 验证；
4. 不以一个巨型事务保护整个研究，也不依赖异步补偿修复本应原子的事实；
5. 不创建万能状态机、万能多态外键、隐藏原文权限或第二事实源；
6. 仍未执行未经授权的真实平台访问，也未创建正式业务 migration。
