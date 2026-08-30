# TOPIC-WORKSPACE-REAL-001 · Topic Workspace Contract

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: 首个真实但明确暂定的 Topic 定义、人工裁定运行、冻结 Material Pack 与本机读取面
> 事实来源: 用户本轮授权、DISC-001 Topic 语义、Work Resource Read、当前 migration/Rust/API/tests
> 冲突时以谁为准: 用户最新确认、AGENTS.md、正式领域语言、不变量、当前代码与真实运行证据

## 1. 一句话合同

Topic Workspace 是对一组已接纳 Work Resource 的**版本化人工研究判断**。它保存 Topic 身份、暂定定义、一次完整的人工裁定运行、精确材料引用、裁定理由和来源边界；不复制原文，不制造 Evidence，不发布正式领域定义，也不自动形成趋势、Claim 或行动。

## 2. 深模块边界

`linggan-intelligence::TopicWorkspace` 负责一个完整、可解释的研究承诺：

```text
Topic Identity
  └─ Definition Version (provisional)
       └─ Classification Run (human_adjudicated / completed)
            ├─ exact Work Resource references + role + rationale
            └─ Material Pack (frozen boundary)
                 └─ Import Receipt (idempotency + exact version)
```

调用者只需要提交一个受控 `TopicWorkspaceImport`，再按 canonical key 读取当前版本。事务、版本竞争、幂等冲突、未知 Work Resource、追加式持久化和读模型拼装都留在模块内部；页面不得自行拼 SQL 或复制 Work Resource 字段。

## 3. 数据所有权

| 事实 | Owner | Topic 是否保存 |
|---|---|---|
| 平台作品身份、标题、作者、发布时间、媒体、lane、来源血缘 | Work Resource / Evidence | 否；只存 `work_public_ref` |
| Topic 长期身份 | Topic Workspace | 是；`topic_ref + domain_key + canonical_key` |
| 当前定义文字与版本 | Topic Definition | 是；首版及修订均为不可变版本 |
| 材料属于支持/挑战/边界 | Classification Run | 是；人工裁定且逐条有 rationale |
| 当前研究集合及其来源限制 | Material Pack | 是；由 exact run 冻结 |
| 正式 Domain Definition Release | 未接通的正式发布责任 | 否 |
| Claim、Signal、Trend、Market Insight | 各自未来责任 | 否 |

## 4. 写入合同

`import_topic_workspace(database, request)` 在单个 PostgreSQL 事务中：

1. 验证 key、文字长度、材料数量与角色组合；至少一条支持材料，且至少一条挑战或边界材料。
2. 对同一 Domain + canonical key 获取事务级 advisory lock。
3. 同 `idempotency_key` 且 exact request hash 相同返回原 Receipt；内容不同返回冲突。
4. 验证每个 `work_public_ref` 已存在于 `linggan_material_content`；任一未知则整笔不写。
5. 新 Topic 只接受 `expectedVersion = null`；既有 Topic 只接受当前版本号，成功后递增一版。
6. 原子写入 Definition、Classification Run、Material Pack、成员与 Import Receipt；所有表均由 append-only trigger 拒绝更新和删除。

失败不会留下半个 Topic、半个材料包或被覆盖的旧版本。

## 5. 读取合同

`read_topic_workspace(database, canonical_key)` 返回当前最高 Definition Version 及其唯一 Run、Pack 和按 ordinal 排序的成员引用。loopback API 再通过共享 `read_work_resource` 逐条组合展示数据：

- `topic` 保留研究判断与边界；
- `materials[].classification` 保留人工角色与理由；
- `materials[].workResource` 来自唯一中立 Work Resource Read；
- `detailUrl` 只指向同源 loopback Work Resource JSON。

若 Topic 引用存在但 Work Resource Read 无法履行，API 返回冲突/不可用，不把丢失对象静默过滤，也不返回一个看似完整的缩小材料包。

## 6. 状态与资格

| 轴 | 当前可声明 | 当前不得声明 |
|---|---|---|
| Truth | 已保存的暂定定义、人工裁定角色、精确 Work 引用 | 正式知识、Claim、趋势事实 |
| Coverage | 材料包内 exact N 条；来源边界原文 | 总体覆盖率、领域代表性、平台全量 |
| Validity | 请求校验、外键、版本与幂等通过 | 定义科学有效、分类准确率 |
| Freshness | Definition/Run/Pack 的各自时间 | “当前世界最新”、材料持续新鲜 |
| Operation | 本机 loopback import/read 成功或明确失败 | 自动采集、Agent 自动发布、现实行动成功 |

## 7. Agent 准入缝

Issue #113 只能读取一个已冻结的 `materialPackRef`。Agent 输入不得是动态查询、页面当前筛选或原始评论集合；执行前必须把 exact Definition Version、Classification Run、Material Pack 与成员引用解析成不可变输入。Agent 输出只能是带引用的候选分析和 unknown/gap，不得回写正式 Topic、Claim、采集计划或行动。

## 8. 已证明与未证明

- 已证明：Rust validation、PostgreSQL 16 原子接纳、未知引用全回滚、exact replay、异内容幂等冲突、版本竞争、完整 migration、loopback API 与 Work Resource 组合。
- 未证明：共享本机数据库应用 `0027`、部署、真实材料人工裁定质量、长期并发负载、正式 Topic 发布、Agent 执行、Mog 业务验收。
