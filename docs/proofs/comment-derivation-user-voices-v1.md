# 评论清洗派生与用户原声 V1 · 隔离 PostgreSQL 证明

状态：PROVEN IN ISOLATION
日期：2026-09-13
范围：把一条不可变 `CommentObservation` 变成一条可追溯、版本化的确定性清洗派生，并将「用户原声」列表收敛为只显示当前、可研究的清洗表达。此包不调用模型、不创建任务、不生成问题、不使用向量服务，也不访问网络来源。

## 用户可见的结果

用户原声表格默认显示「全部可用」的**清洗后研究表达**：

- **可直接研究**：不依赖额外讨论上下文也能阅读的表达；
- **需要上下文**：如「同问」这类仍有语义、但应结合作品或回复链理解的短表达；
- **等待清洗**：已有当前 CommentObservation、但尚未物化本合同派生的数量单独显示，不伪装成 0 或可研究；
- 纯 emoji、纯 mention、纯标点和控制字符异常的派生不进入用户原声表，也不进入三个可用筛选。

详情抽屉明确分开「清洗后研究表达」与「原始采集原声」。原始文本只从当前 Evidence locator 的详情读取；表格不会把清洗文本写成原文，也不会暴露清洗原因、派生 ID 或 fingerprint。

筛选只有 `available`（默认）、`ready`、`needs_context`。不存在面向用户的 `dropped` 或 `anomaly` 筛选，避免把确定性噪声重新放回语料工作流。

## 数据边界

`0003_comment_derivation_v1.sql` 新增 `comment_derivation_v1`：

- 每个 row 锚定一个 `comment_observation_id`，原文仍只存于 Observation；
- 身份是 `(comment_observation_id, cleaning_contract)`，而非只按 Observation 唯一。新的清洗合同可与 V1 并存；当前列表显式只读 `comment-cleaning.v1`；
- `research_state`、`research_text`、`reason_codes` 由 Cleaning Contract V1 确定，数据库约束校验状态、正文和原因组合；
- `research_text_integrity_md5` 是 PostgreSQL 从已存派生正文生成的校验值；浏览器 DTO 不返回它；
- 派生表 append-only。没有 `derivation_current`：列表始终从 `comment_current_v0.current_observation_id` join V1 派生，所以旧 Observation 的清洗结果不会出现在当前原声中。

新接入或推进 Current 的 Observation 在同一个 admission transaction 中同步物化 V1 派生。对迁移前已存在的 current Observation，调用方只能显式使用：

```text
materialize_missing_current_comment_derivations_v1(workspace_id, limit)
```

这个操作每次最多处理 100 条、有幂等的 Observation × V1 合同唯一约束、不会由页面/API GET 自动触发，也没有模型或网络副作用。

## 运行证明

```text
scripts/prove-comment-derivation-user-voices-v1.sh
```

脚本启动随机命名的 `postgres:16-alpine` 容器、随机数据库和随机 loopback 端口，只向本次 ignored integration test 提供临时 DSN，结束后删除容器。它不读取共享数据库或现有运行时环境。

已验证：

| 场景 | 结果 |
| --- | --- |
| 迁移前的 current Observation | V1 列表返回 `awaiting_cleaning_total=1`，没有伪造可用行 |
| 有界物化与重复执行 | 第一次物化 1 条；第二次为 0；不重复写派生 |
| 合同版本并存 | 同一 Observation 可保存 `comment-cleaning.v1` 与 `.v2`；V1 列表不混入 V2 |
| 新接入 | 每个 Created/Advanced Observation 同事务创建 V1 派生 |
| exact replay / 同 raw 新 Evidence | 不创建新 Observation，也不创建重复 V1 派生 |
| 原始正文变化但清洗文本相同 | 新 Observation 和新 V1 派生仍然创建；当前列表使用新来源 Evidence |
| dropped / anomaly | 原始 Observation 保留，派生不出现在 User Voices 可用列表 |
| 数据库保护 | 派生 UPDATE/DELETE 返回 PostgreSQL `55000`；不合法 state/text 组合返回 check violation |
| 派生文本校验 | 数据库生成 `research_text_integrity_md5 = md5(research_text)` |
| API 筛选 | `available`、`ready`、`needs_context` 返回正确范围；非法 `dropped` 返回 400 |
| API 纯读与隐私 | GET 前后 Evidence、Observation、Current 和 Derivation 行数不变；DTO 没有 raw text、reason codes、derivation ID、fingerprint、作者、点赞、时间、OCR/ASR 或媒体字段 |

## 未证明、不得声称

- 真实模型调用、Prompt/Skill、Context Pack、分析合同、运行队列、失败重试；
- 向量、聚类、Problem、趋势、自动调度；
- 生产/shared 数据库 migration、main 合并、3000 端口或浏览器实际验收；
- 清洗合同之外的 URL/电话脱敏策略；此包仅持久化和展示确定性清洗结果，不生成模型输入。
