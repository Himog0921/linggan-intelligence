# COMMENT-UNDERSTANDING-V2-001 · 评论研究 V2 实施计划

> 状态: 活跃计划
> 最后核对: 2026-11-19（G1、G2 已实施，见附录 B、C）
> 适用范围: 评论研究 V2：按作品分单元的批量理解、稳定需求身份、合并重定向与派生状态（G0–G6）
> 事实来源: Mog 2026-11-19 的决定（第 10 节）、`main@8891d71`、本机运行库只读核对、本机 WeMM 小样本实验
> 冲突时以谁为准: 用户最新明确授权、AGENTS.md、真实运行/数据库副作用、当前代码/migration/测试

产品决策（不变）：B——单条评论形成候选；同一需求获得至少两位可区分作者的有效支持后成为正式需求。\
适用仓库：`Himog0921/linggan-intelligence`。\
实施状态：G0、G1、G2 已实施（分支 `claude/comment-understanding-v2`，未合并、未部署）；G3 起尚未实施。G1 对本计划 §4.2、§5.3、§6.1 的修正已就地改正（附录 B）；G2 的实现差异见附录 C，并已写入 §5.2–§5.4 的实现说明。\
接手继续实施从[交接文档](comment-understanding-v2-001-handoff.md)开始。

---

## 0. 修订说明

### 0.1 修订依据

本版依据三类事实修订：基线代码、本地运行库的只读查询（2026-11-19），以及本地 WeMM 向量模型的小样本实验。第 1 节列出每条事实的来源。

### 0.2 与上一版相比的主要变化

| # | 上一版 | 本版 | 原因 |
|---|---|---|---|
| 1 | 复用现有 Embedding 做候选召回 | 生成调用里直接放入完整的领域需求目录；向量只用于候选去重，召回要等目录超过阈值才启用 | 实验显示向量分不清"同一问题"和"同话题不同问题"；目录小时召回只会漏掉候选 |
| 2 | 需求状态（候选/正式）存成字段，另保留正式化事件 | 状态从有效作者数**实时推导** | 现有表只追加不修改；推导后"并发时漏掉正式化"的问题不存在 |
| 3 | 召回不确定时输出 `uncertain` | 删除 `uncertain`；拿不准就选 `new` | 重复候选可以合并，误合并很难拆 |
| 4 | 同批次内的新需求如何归组没有定义 | 输出合同增加批内 `newKey`，一个批次内同一个新需求只建一次 | 同一作品下多位作者说同一件事是最常见的正式化途径 |
| 5 | 去重机制没有定义 | 新增"候选整理"任务：向量负责配对排序，LLM 判定，程序写重定向 | 召回不可能完美，重复候选会分走支持数 |
| 6 | 新增 Run / Work Group / Batch 三层执行对象 | **一个 batch item 就是一次模型调用（一个"研究单元"）**，执行器沿用现有的单 item 领取，不需要改成批量领取 | 改动集中在数据层，执行器框架不用重构 |
| 7 | 候选 revision 变化时重新处理；候选目录变化时重新绑定 | 删除 | 最坏的后果只是产生重复候选，由候选整理处理 |
| 8 | 新旧流程并存，旧结果只读兼容 | 若 G0 确认生产库也没有旧数据，**直接退役旧流程，不写兼容层** | 本地运行库中评论研究和需求相关的表全部为 0 行 |
| 9 | L1 缺失时评论进入"等待 L1"状态 | L1 缺失只作为选择过滤条件，并提供"为这些作品补 L1"入口，复用现有 L1 工作流 | 本地只有 41% 的评论所属作品有 L1 |
| 10 | 用人工标注样本验收，质量达标后才切换 | **先完整落地上线，再做质量优化。** 上线门槛只有程序不变量和功能验收；自一致性、跨模型复核和参考集回归都放到上线后 | 负责人无法做人工标注；负责人决定优先保证整套计划落地，上线后再局部优化和测试，并亲自事后抽查 |
| 11 | 一个 Signal 带一个 Problem Frame | Frame 只放在需求定义上；Signal 只保留 kind、proposition、evidence 和归属 | 现有逐字段 basis 校验，一处失败就作废整条评论，而且 token 成本高 |

---

## 1. 基线事实

### 1.1 代码（基线 `8891d71`，已读代码证实）

| 事实 | 位置 |
|---|---|
| 第二位作者的配对靠 `problem_identity`（actor/goal/barrier/context 规范化文本的哈希）完全一致；候选不入库、没有 ID | `ai_foundation/comment_problem.rs` 的 `pair_support_in`、`proposals_in` |
| `definition` 和 `membership` 两张表禁止 UPDATE/DELETE；`actor/goal/barrier` 是 NOT NULL；`review_invocation_ref` 是 NOT NULL；`membership` 主键是 `(request_ref, signal_ordinal)`；信号序号范围 0–11 | `database/migrations/0177_comment_problem_workflow.sql` |
| 作者为空时 `author_in` 直接报错，阻断后续流程 | `comment_problem.rs` |
| 一条评论建 3 个批次（语义提取、归并、配对复核），各一次调用 | `ai_foundation/comment_research.rs` 的 `start` |
| 任何一个 frame 字段的 `basis` 定位失败，整条评论的输出作废 | `executor/service/comment_research.rs` 的 `validated_output` |
| 读取支持关系时 `frozen_in` 会重新计算输入指纹，不一致就丢弃这条支持 | `comment_research.rs` 的 `frozen_in`；`comment_problem.rs` 的 `definitions_in` |
| 现在的作品语境来自 Research Dossier 的 `model_context`，不是 L1 | `executor/service/comment_research.rs` |
| 执行器按单个 item 领取任务 | `executor/queue` 中的 `ClaimedItem` |
| Opportunity 依赖 `membership.review_invocation_ref` | `opportunity/mentions.rs:137` |
| 本地向量模型 WeMM-Embedding-2B（512 维）的运行时代码还在，但没有调用方；向量存储表已被 `0075`、`0179` 删除；`comment_embedding` 任务为 `frozen` | `pi_adapter.rs:345`；迁移 `0075`、`0179`、`0131` |

### 1.2 本地运行库（`linggan_intelligence_dev`，2026-11-19 只读查询）

| 事实 | 数值 |
|---|---|
| 评论研究请求、结果、需求定义、需求归属、Opportunity 评论提及 | **全部为 0 行** |
| 评论总数 / 不同作者数 / 发过不止一条的作者 | 32,716 / 19,411 / 2,750 |
| 作者 ID 缺失 | 0 |
| 清洗后长度（direct 类） | 中位数 20 字，p90 79 字 |
| 每个作品的评论数 | 中位数 13，p90 30，最大 731 |
| 有评论的作品中有 L1 结果的 | 797 / 1,755 作品；11,910 / 29,350 条评论（41%） |

> G0 已确认：这就是生产库。本机运行时（3000 端口启动脚本，`runtime-identity.json` 的 revision 为 `8891d71`）连接的正是 `127.0.0.1:55432/linggan_intelligence_dev`，迁移头为 `0229_retire_note_interpretation`。按已保存的 L1 Core（`linggan_content_understanding_saved_core`）计算，可用 L1 的作品最多 810 个，对应 12,135 条评论，再经过 L1 资格复核还可能更少。详见附录 A。

### 1.3 本地向量实验（WeMM，document 模式；33 条真实 ADHD 评论，10 组；标签由 Claude 标注）

| 指标 | 数值 |
|---|---|
| 正例（同一问题）余弦中位数 | 0.650（区间 0.475–0.790） |
| 困难负例（同话题不同问题）中位数 | 0.584（区间 0.394–0.750） |
| 正例与困难负例的区分 AUC | 0.744；最佳单一阈值只能分对 72/102 |
| 评论对需求定义的召回（共 10 条定义） | 第一名命中 24/33，前三名命中 32/33 |
| 典型错误 | 向量不区分问题的主体：家长自己情绪失控和孩子情绪崩溃相似度很高 |

**结论：** 向量不能判定"是不是同一个需求"，只能做宽松的排序和召回。判定交给 LLM。

---

## 2. 产品定位与边界

沿用上一版第 1 节，只做以下收敛：

- 评论研究是 Comment Intelligence（用户需求情报），研究对象是评论者的表达。它不创建 Topic、不判断商业机会、不生成稿件。
- **L1 Core 是必需输入**，L2 不是。父评论有就提供，没有也不阻断。
- **L1 是评论研究的上下文，必须具备，不允许降级**（负责人 2026-11-19 确认）。没有 L1 的作品，不能用标题、正文或 Research Dossier 代替。
- **L1 缺失不是研究状态，只是选择过滤条件**：选择预览里显示"L1 未就绪 N 条（涉及 M 个作品）"，并提供"为这些作品排队 L1"，复用现有的 `content_understanding` 任务。L1 就绪后，由运营重新选择这些评论发起研究，**不做自动续跑**。
- 一条支持关系是否有效，**只看评论原文是否仍可用**（没有撤回、受限或隐私处置）。L1 版本和清洗版本只记录来源，**不作为有效性条件**。L1 重新生成，不会让已有的支持失效。
- 作者 ID 缺失的评论仍可研究，但不计入独立作者数。现有数据里没有这种情况，所以不为它设计额外的界面或流程。

---

## 3. 语义合同

### 3.1 通用提取规则

1. 一个 Signal 是一个可以独立成立的表达，外加一段**目标评论原文的连续片段**作为证据（取能支撑该表达的最短片段）。
2. `proposition` 用第三人称复述评论者说了什么，不升级、不诊断、不推断因果。写"评论者的孩子写作业需要反复催促才开始"，不写"孩子有启动障碍"。
3. **一条评论有多个困难时必须逐一提取**，不能只取最显著的一个。
4. 一个命题只给一个 kind。只有在表达不同命题时，同一片段才能产生多个 Signal。
5. 以下内容输出 `no_signal`：纯附和（"+1""同""蹲"）、纯 @、客套、广告和引流、对象无法确定的"求"。
6. 文字中的 emoji 可以作为 emotion 的证据。
7. 父评论和 L1 只用来理解指代和语境，**不能作为证据**，也不能补出评论里没说的事实。
8. 同批次其他评论的内容**不得**出现在本评论的 proposition 和证据里。

### 3.2 九种 Signal 的判定规则

| kind | 中文 | 判定问题 | 边界：容易混淆时归到哪里 |
|---|---|---|---|
| problem | 问题 | 能否同时说出"想做到什么"和"卡在哪"？ | 只有情绪、没有障碍 → emotion |
| need | 需求 | 是否明确想要某种帮助、资源或结果？ | 同一句里有明确障碍 → problem |
| question | 提问 | 是否在问关于内容本身的事实或澄清问题（"哪里买""第几页""什么药"）？ | **"有没有办法让孩子……""……怎么办"反映的是未满足的需求 → need 或 problem** |
| solution | 解决办法 | 是否说了自己用过或建议的、**具体可执行**的做法？ | 空泛的主张（"多陪伴就好"）→ belief |
| experience | 经历 | 是否描述了一件实际发生的具体经过？ | 带有方法性内容 → solution；只是身份处境 → context |
| belief | 观点 | 是否表达了判断、态度或认知？ | — |
| emotion | 情绪 | 是否有明确的情绪词、语气或 emoji？ | 不能从话题推断情绪 |
| context | 背景 | 是否明说了身份或处境（年级、确诊情况、家庭角色）？ | 不能推断 |
| quote | 典型表达 | **其他八类都不适用**，但原话凝练、可以直接用于创作？ | 只作为兜底类型，不与其他类型重复 |

只有 problem 和 need 可以归属到需求（Problem）。其他类型独立保存。

### 3.3 需求定义（Problem Definition）

需求定义只在新建需求时由模型给出，字段如下：

| 字段 | 规则 |
|---|---|
| `label` | 20 字以内，"主体 + 障碍"的形式，例如"孩子写作业启动困难" |
| `actor` | 角色类别：小学生家长、成年患者、评论者本人等。未知时为 null |
| `goal` | 主体想做到什么 |
| `barrier` | **区分不同需求的关键字段**，写清障碍机制 |
| `context` | 只在会改变障碍性质时填写，否则为 null |

**粒度：** 选能被一份内容或一套方法直接回应的层级。
- 太粗："孩子的学习问题"
- 太细："周三晚上写数学拖延"
- 合适："孩子写作业启动困难"

### 3.4 是否同一需求的判定

1. **同一需求 = 主体类别相同 + 目标相同 + 障碍机制相同。** 场景不同但障碍机制相同，算同一需求。
2. **主体不同，就是不同需求**：家长自己情绪失控 ≠ 家长不会应对孩子的情绪；成人启动困难 ≠ 孩子写作业启动困难。
3. 同属一个话题不等于同一需求：写作业**启动**困难（迟迟不开始）≠ 写作业**耗时**过长（开始了但拖到很晚）≠ 上课走神。
4. 方向规则：评论是某个已有需求的具体实例 → 选那个需求；评论比已有需求更宽，或只部分重叠 → 新建。
5. **拿不准就选新建**，不存在"不确定"这个出口。

> 第 2、3 条的示例直接来自 §1.3 实验里向量最容易混淆的几对。这些是向量分不清、必须由 LLM 区分的地方，提示词里要保留。

---

## 4. 数据设计

### 4.1 核心对象

```text
需求目录（按领域）
  需求 Problem（problem_ref 稳定）
    ├── 定义版本（只追加）
    ├── 支持关系：Signal → Problem（只追加）
    └── 重定向：被合并的 problem_ref → 存活的 problem_ref（只追加，可撤销）

需求当前状态视图（推导，不存储）
  解析重定向 → 汇总有效支持 → 统计去重作者数、评论数、作品数
  → 状态：candidate（作者数 < 2）/ formal（作者数 ≥ 2）
```

- **候选在第一次出现时就入库**，拿到 `problem_ref`。后续评论按 ref 匹配，不再靠文本哈希。
- **正式状态是推导出来的**，不存储、没有状态机、没有正式化事件表。作者数下降时，状态自然回到 candidate；历史支持记录本来就不会被删除。
- Opportunity 和创作模块都只读这个视图。

### 4.2 最小迁移（G1，`0230_comment_understanding.sql`）

| 变更 | 内容 |
|---|---|
| `request` 的 `batch_ref`、`command_ref` 唯一约束；`result` 的 `invocation_ref` 唯一约束 | **取消**唯一约束，改为普通索引：一个批次、一个命令、一次模型调用现在对应多条评论请求。`method_version` 的 CHECK 增加 `comment-understanding.v1` |
| 新表 `linggan_comment_research_unit` | `unit_ref`（主键，放进批次输入引用）、`item_ref`（唯一，指向 `ai_batch_item`）、`domain_ref`、`work_ref`、`l1_request_ref`（指向 L1 请求，即 analysisRef）、`created_at` |
| 新表 `linggan_comment_research_unit_target` | `request_ref`（主键）、`unit_ref`、`position`（从 1 起，同单元内唯一） |
| 新表 `linggan_comment_research_unit_dispatch` | `unit_ref`（主键）、`invocation_ref`（唯一）、`catalog_problem_refs uuid[]`：派发时模型看到的需求目录，别名 `p<n>` 即第 n 个元素；T2 靠它把别名还原成需求 |
| 单元完整性触发器 | L1 请求必须与单元同作品、同领域；目标必须是同作品、同领域、同批次的 V2 请求。这是"不同作品不共享上下文"的数据库保证 |
| `linggan_comment_problem_definition` | `actor`、`identity_hash` 改为可空；`review_invocation_ref` 改为可空；新增 `origin_invocation_ref`；CHECK 两者恰好有一个非空。`problem_ref` 保持唯一：V2 里没有任何操作会改定义，所以不建定义版本 |
| `linggan_comment_problem_membership` | `review_invocation_ref` 可空，新增 `origin_invocation_ref`，同样恰好一个非空 |
| 新表 `linggan_comment_problem_redirect`、`..._redirect_revocation` | 见下；视图 `linggan_comment_problem_active_redirect` 过滤已撤销的重定向 |
| 重定向校验触发器 | 同领域；`from` 与 `to` 都不能已经有生效的重定向；按领域加咨询锁串行化。因此每条边都从一个活的需求连到另一个活的需求，**不可能成环，无需递归检测** |
| 派生视图 | `linggan_comment_problem_resolution`（需求 → 当前活的需求）、`linggan_comment_current_research`（每条评论当前算数的研究）、`linggan_comment_problem_support`（经重定向解析后的有效支持）、`linggan_comment_problem_current`（每个活的需求一行：状态、支持信号数、评论数、作者数、作品数） |
| 任务注册 | `comment_understanding`：未配置预算和模型、开关关闭；草案提示词 v1；5 个失败码；同步更新 `ai_batch_task_kind_valid` 和 egress material 约束。**`comment_problem_consolidation` 随 G3 注册**（连同它的派发表和输入引用） |
| G4 才做：新表 `linggan_comment_problem_definition_vector` | `definition_ref`、`model_revision`、`vector(512)`；只给需求定义算向量，**不给评论算** |

**有效支持的口径（视图实现）：** 每条评论（按领域、作品、评论 ID）只取最新一条**有成功结果**的 V2 请求；评论已被限制的不算；最新请求失败时保留更早的成功结果。作者按（平台，作者 ID）去重，作者为空的评论计入评论数但不计入作者数。状态阈值：作者数 ≥ 2 为 formal，否则 candidate。

**G0 结论：生产库没有旧的评论研究数据，直接退役旧流程，不写兼容层。** 退役放到切换时的迁移 `0231`：把 `comment_study`、`comment_problem_resolution`、`comment_problem_pair_review` 设为 `retired`（沿用 `0229` 的做法：关闭开关、取消排队项），删除空的旧 stage 表，并同时删除旧代码。G1 不退役旧任务，因为旧流程的代码和测试此时仍在；两者分开，保证每个提交都是绿的。

---

## 5. 任务编排

### 5.1 总流程

```text
[运营选择评论]
      │
      ▼
T0 规划（确定性，Start 命令事务内）
  筛选资格 → 按作品分组 → 切分研究单元 → 冻结 L1 引用 → 建批次、单元、目标
      │
      ▼
T1 单元调用（模型，每个单元一次；同一领域串行）
  派发时读取当前需求目录 → 组装 prompt → 调用 → 记录用量
      │
      ▼
T2 接纳（确定性，结果落库事务内）
  逐目标、逐信号校验 → 写结果 → 建新需求（每个 newKey 一个）→ 写支持关系
      │
      ▼（本次运行的所有单元结束后）
T3 候选整理（模型，每个领域一次；本次运行有新候选时才触发）
  向量排序出候选配对 → LLM 判定 → 写重定向
      │
      ▼
T4 下游刷新（确定性）
  需求当前状态视图实时生效 → 标记受影响的 Opportunity 卡待刷新
```

只有 T1 和 T3 调用模型。100 条评论的预期调用次数：T1 约 5–7 次（每单元 20 条，按作品切分有零头），T3 约 1–2 次，**合计约 6–9 次**。旧流程最多需要 300 次。

### 5.2 T0 规划

**触发：** 运营在评论目录发起研究，或在研究记录里点击"重试失败"。

**步骤（全部在一个 Start 命令事务内，幂等键为 `command_ref`）：**

1. **资格筛选**，不合格的进入预览里的"无法处理"列表并注明原因：
   - 评论原文可读，清洗状态为 `direct` 或 `context`；
   - 所属作品有当前可用的 L1 结果；
   - 作品在当前领域和来源范围内；
   - 默认跳过已经有 `comment-understanding.v1` 成功结果的评论，除非运营勾选"重新研究"。
2. **按作品分组。** 不同作品不混在一个单元里。
3. **切分研究单元：** 每单元最多 `UNIT_MAX_TARGETS` 条，并满足 `UNIT_MAX_OUTPUT_EST`（按每条目标的估算输出累加）。同一作品切成多个单元时，按（讨论串根评论，评论 ID）排序后切分，让同一讨论串尽量落在同一单元里（实现没有用平台时间，见附录 C.2）。
4. **重试时自动缩小单元：** 如果某条目标上一次失败的原因是 `unit_truncated`，这次它所在单元的上限减半，最小为 1。
5. **冻结**每个单元的 `l1_result_ref`，以及每条目标的输入指纹（原文哈希、清洗版本、父评论、L1 引用，只用于审计）。
6. **写入：** 一个 `ai_batch`（`task_key = comment_understanding`）；每个单元一个 `ai_batch_item` 和一行 `unit`；每条目标一行 `request` 和一行 `unit_target`。
7. **单元排序：** 评论多的作品排在前面，让需求目录先由内容最丰富的单元建立起来。

**界面预览**只显示：已选评论数、涉及作品数、可研究数、L1 未就绪数（附"补 L1"入口）、其他无法处理的数量及原因、预计调用次数。

**G2 实现说明：** 预览与启动由同一个规划函数产生同一个 `planHash`（不含随机的单元 ID）；启动时重算，哈希不一致返回冲突，不会去研究和预览不同的东西。已有成功结果的评论、已在排队或运行的评论、上一次调用结果未知（`ai_dispatch_unknown`）的评论都不进计划；没有单独的"重试"入口，运营重新选择失败的评论即可，截断次数从历史结果推导。一次启动最多 1000 条选择。

### 5.3 T1 单元调用

**并发规则：同一领域同一时间只执行一个单元，不同领域可以并行。**
- 原因：每个单元都在派发时读取最新的需求目录，前一个单元新建的候选能被后一个单元直接匹配。跨单元的同义需求在这里就能合并，不必都留给 T3。
- 代价：同一领域的运行是串行的。按 300 条评论、每单元 20 条、每次调用约 40 秒估算，大约需要 10 分钟。对研究任务来说可以接受。
- 实现方式：执行器领取任务时加一个条件，同领域已经有 `comment_understanding` 的 item 处于 running 就不领取。不需要新的锁服务。

**派发时组装输入。** Prompt 按缓存友好的顺序排列：

| 顺序 | 内容 | 是否稳定 |
|---|---|---|
| 1 | 系统规则 + 第 3 节语义合同 + 输出合同 | 全局稳定 |
| 2 | 领域需求目录：用 `p1…pN` 作短别名，**按创建时间升序排列**，新需求只追加在末尾 | 同领域前缀稳定，可命中缓存 |
| 3 | 作品的 L1 精简投影（summary 加 discussion、author_position、provision、reader_value、audience_context 知识单元） | 同作品稳定 |
| 4 | 目标列表：用 `t1…tM` 作短别名，含评论原文和可选的父评论 | 每单元不同 |

所有引用都用带字母前缀的短别名，由程序映射回 UUID。之前的评估里出现过模型把数字下标整体错位一位的情况，带前缀的别名能减少这类错误。

**需求目录的两种模式：**

| 模式 | 条件 | 做法 |
|---|---|---|
| 全量（V2 默认） | 领域内当前需求数 ≤ `CATALOG_FULL_MAX` | 全部放入 prompt |
| 召回（G4 之后，满足门槛才启用） | 超过 `CATALOG_FULL_MAX` | 用 WeMM 计算目标评论原文与各需求定义的相似度，每条目标取前 `RECALL_TOP_K` 个，取并集后再加上本作品已有支持的全部需求，总数不超过 `CATALOG_RECALL_CAP` |

派发时实际放入 prompt 的需求 ref 列表，按顺序记在 `linggan_comment_research_unit_dispatch.catalog_problem_refs`：既供审计，也是 T2 把别名 `pN` 还原成需求的唯一依据。

**G2 实现说明：** 作品语境是冻结的 L1 Core 卡的投影：摘要、内容类型、主话题和 `discussion / author_position / provision / reader_value / audience_context` 五类知识单元（`id、unitType、statement、assertionKind、speakerRole`）；不含证据引用、候选标签和 L2。目录超过 `CATALOG_FULL_MAX`（200）时**整个单元跳过**（`comment_catalog_too_large`），不截断：截断的目录会让模型对看不见的需求重复新建。L1 卡找不到时跳过（`comment_l1_unavailable`）。派发前评论已不可读的目标，在同一个事务里写入失败结果 `ai_source_unavailable`，其余目标照常发送；全部不可读则整个单元跳过。

**失败处理：** 提供方错误和超时沿用执行器现有的重试策略，T1 不再另加重试逻辑。

### 5.4 T2 接纳

在模型结果落库的同一事务里执行，按以下顺序校验。

**整个单元的检查：**

| 情况 | 处理 |
|---|---|
| JSON 无法解析，或顶层合同不符 | 本单元所有目标标记失败，原因为 `unit_output_invalid` |
| 输出被截断 | 本单元所有目标标记失败，原因为 `unit_truncated`；下次重试时单元自动减半。**适配器把长度截断报告为失败的调用（`output_limit`），而不是一个较短的回答**，所以执行器按这个失败码和 `finish_reason=length` 识别 |

**逐条目标的检查：**

| 情况 | 处理 |
|---|---|
| 某条目标缺失或重复 | 该目标失败，原因为 `target_missing` 或 `target_invalid`；**同单元其他目标不受影响** |
| outcome 与 signals 不一致（`signals` 要求非空，`no_signal` / `needs_context` 要求为空） | 该目标失败，原因为 `target_invalid` |
| `reason` 为空或缺失 | 不失败：`reason` 只是说明，缺失时存空，超过 500 字截断 |
| 一条目标的信号超过 12 条 | 只检查前 12 条，其余计入 `droppedSignals`（信号序号上限与数据库一致） |

**逐条信号的检查：**

| 情况 | 处理 |
|---|---|
| 证据在目标原文里定位不到 | **只丢弃该信号**，记录丢弃数 |
| kind 不在枚举内 | 丢弃该信号 |
| 归属引用了不在本次目录里的 `pX` | 去掉归属，信号保留 |
| `newKey` 在 `newProblems` 里找不到，或同一个 `newKey` 被定义了两次 | 去掉归属，信号保留（重复定义有歧义，不择一） |
| problem / need 信号没有任何归属 | 信号保留，`eligibilityState` 为 `deferred_context` |
| 非 problem/need 的信号带了归属 | 去掉归属 |
| 一条目标的信号全被丢弃 | 该目标记为失败，原因为 `target_all_signals_invalid` |

**写入：**
1. 每条目标写一行结果（JSON 里记录接纳的信号和丢弃计数）。
2. 每个**至少被一条已接纳信号引用**的 `newKey` 建一个需求（`problem_ref` + 定义），没被引用的直接丢弃。
3. 每条带归属的信号写一行支持关系。如果 `pX` 已经被重定向，就解析到当前存活的需求。
4. 幂等：`(request_ref, signal_ordinal)` 主键保证同一信号只写一次；执行器重复投递同一结果时，整个事务不产生新行。

**目标状态**完全从现有数据推导，不新建状态字段：
- 排队 / 执行中：由单元 item 的状态决定；
- 已完成（有信号 / 无信号 / 需要语境）或失败（附原因）：由结果行决定；
- 已取消：沿用现有的取消表。

### 5.5 T3 候选整理

**触发：** 一次运行的所有单元结束后，如果本次运行新建了候选，就对该领域自动排一个整理任务。研究记录里也提供手动触发入口。

**步骤：**
1. **生成配对（确定性，不调用 LLM）：**
   - G4 之前：领域候选总数 ≤ `CONSOLIDATE_FULL_MAX` 时，把全部候选和正式需求一次性交给 LLM，由它自己找重复。
   - G4 之后：给需求定义算 WeMM 向量（`label + actor + goal + barrier`）。对每条**本次运行新建的候选**，取相似度最高的 `PAIR_TOP_K` 个需求，并要求相似度 ≥ `PAIR_FLOOR`。
2. **LLM 判定：** 输出合同为 `comment-problem-consolidation.v1`。每个配对输出 `same`、`different` 或 `narrower`（一方是另一方的子集）。**只有 `same` 会被执行。**
3. **写重定向，存活方按以下顺序选择：**
   1. 正式需求优先于候选；
   2. 有效作者数多的优先；
   3. 创建时间早的优先。

   被合并的定义保留为历史，支持关系不改写，由视图通过重定向解析。
4. **合并后，两个候选的作者合计达到 2 位时，自动成为正式需求。** 这是跨批次正式化的主要途径。
5. **撤销：** 研究记录里可以撤销某次重定向。撤销写入撤销表，视图立即恢复。

**参数依据：** §1.3 的实验里，正例相似度最低为 0.475，困难负例中位数为 0.584。所以 `PAIR_FLOOR` 取 0.45，偏向召回，宁可多送配对给 LLM 判定，也不漏掉真正的重复。

### 5.6 T4 下游刷新

- 需求当前状态视图实时生效，没有异步任务。
- 写支持关系、重定向和撤销时，标记受影响需求关联的 Opportunity 卡待刷新。复用 Opportunity 现有的刷新机制，新增两种来源事件。
- Opportunity 刷新失败**不回滚**研究结果。
- **Topic Map 是评论研究的另一个下游（G0 发现）。** 它的评论模式会读取每条评论的研究结果，并在读取时通过 `comment_research::frozen_in` 重新校验资格；主题研究自动化也按评论研究结果来挑选输入。V2 必须做到三点：
  1. 结果 JSON 里的信号字段与现有格式兼容（见 §6.3）；
  2. 用基于 L1 的 `frozen_in` 替换现有实现，语义见 §2：只校验评论原文是否可用；
  3. 把这些查询里"`request.batch_ref` 对应的 item 是否成功"的判断，改为"该请求是否有成功的结果行"（见附录 A.3）。

### 5.7 编排参数（初始值）

| 参数 | 初始值 | 依据 | 何时调整 |
|---|---|---|---|
| `UNIT_MAX_TARGETS` | 20 | 每个作品评论数 p90 为 30，多数作品 1–2 个单元就能装下；评论中位数只有 20 字 | P0 测输出长度和质量随单元大小的变化 |
| `UNIT_MAX_OUTPUT_EST` | 8k tokens | 估算每条目标 1–3 个信号、约 300 tokens | P0 实测截断率 |
| `CATALOG_FULL_MAX` | 200 | 每条需求约 60–80 tokens，200 条约 15k tokens，且可命中缓存 | 匹配质量随目录变大出现下降时 |
| `RECALL_TOP_K` / `CATALOG_RECALL_CAP` | 25 / 150 | 实验里前三名命中率在只有 10 条定义时就只有 32/33，所以必须放宽 | G4 门槛测试 |
| `CONSOLIDATE_FULL_MAX` | 150 | 与目录全量模式同理 | — |
| `PAIR_TOP_K` / `PAIR_FLOOR` | 8 / 0.45 | §5.5 | 根据跨模型复核的漏合并率调整 |
| 同领域并发 | 1 | §5.3 | 运行时长不可接受时，可以放到 2 并接受更多重复候选 |

参数放在 `comment_understanding` 的任务配置里，修改要走现有的任务配置修订流程。

---

## 6. 输出合同

### 6.1 `comment-understanding.v1`

```json
{
  "contract": "comment-understanding.v1",
  "newProblems": [
    {"newKey": "n1", "label": "孩子写作业启动困难", "actor": "小学生家长的孩子",
     "goal": "按时开始写作业", "barrier": "面对作业迟迟不动笔，需要反复催促", "context": null}
  ],
  "targets": [
    {"targetKey": "t1", "outcome": "signals", "reason": null,
     "signals": [
       {"kind": "problem", "proposition": "评论者的孩子被要求写作业时坐着发呆不开始",
        "evidence": "你跟他说去写作业,他就坐着发呆", "problem": {"ref": null, "newKey": "n1"}},
       {"kind": "emotion", "proposition": "评论者对此感到无奈",
        "evidence": "...", "problem": null}
     ]},
    {"targetKey": "t2", "outcome": "signals", "reason": null,
     "signals": [
       {"kind": "need", "proposition": "...", "evidence": "...", "problem": {"ref": "p7", "newKey": null}}
     ]},
    {"targetKey": "t3", "outcome": "no_signal", "reason": "纯附和", "signals": []}
  ]
}
```

约束（JSON Schema 由 Rust 端定义，同时在程序里校验）：
- `targets` 必须恰好覆盖本单元的每个 `tX` 一次；
- `outcome` 的取值为 `signals`、`no_signal`、`needs_context`；后两者的 `signals` 为空，`reason` 非空；
- `problem` 是 null 或 `{"ref","newKey"}` 两个字段恰好一个非 null（模型端 Schema 不用 anyOf，由接纳器校验），只有 problem 和 need 使用；
- 每条目标最多 12 个信号，与现有的序号上限一致；
- 模型**不输出**作者数、正式状态或任何 UUID。

### 6.2 `comment-problem-consolidation.v1`

```json
{
  "contract": "comment-problem-consolidation.v1",
  "pairs": [
    {"pairKey": "q1", "verdict": "same", "reason": "主体、目标、障碍机制一致，仅措辞不同"},
    {"pairKey": "q2", "verdict": "different", "reason": "一个是启动困难，一个是耗时过长"}
  ]
}
```

### 6.3 存储格式（与下游兼容）

T2 把每条目标的结果写入 `linggan_comment_research_result.result`，信号格式**沿用现有的 `comment-study.semantic.v1` 字段**，只新增 `problemRef`：

```json
{"contract": "comment-study.semantic.v2", "outcome": "signals", "reason": null,
 "signals": [{"kind": "problem", "proposition": "...", "sourceRef": "<uuid>",
              "evidenceSpan": {"start": 0, "end": 15}, "problemFrame": null,
              "eligibilityState": "eligible", "eligibilityReason": null,
              "problemRef": "<uuid 或 null>"}],
 "droppedSignals": 0}
```

- `problemFrame` 固定为 null，因为需求定义已经移到需求目录上。
- `eligibilityState` 继续由 `comment_study_semantic::accept_semantic_output` 计算。
- Topic Map 读取的 `kind`、`proposition`、`problemFrame`、`eligibilityState`、`eligibilityReason`、`evidenceSpan`、`sourceRef` 都保持不变。

---

## 7. 验收设计（无人工标注）

负责人无法做人工标注，所以下面四层都不依赖人工标签。**按负责人决定，先落地上线，后优化质量**：

| 层 | 用途 | 时间点 |
|---|---|---|
| 7.1 程序不变量 | **上线门槛**，必须为 0 | 实施各阶段和上线时 |
| 7.2 自一致性与扰动 | 质量优化的依据 | 上线后 |
| 7.3 跨模型复核 | 质量优化的依据；负责人事后抽查 | 上线后，由负责人决定何时开始 |
| 7.4 参考集回归 | 防止优化时退步 | 上线后建立 |

§7.5 的数值门槛**不阻断上线**，只作为上线后第一轮优化的目标。

### 7.1 第一层：程序不变量（上线硬门槛，必须为 0）

这一层由测试和生产巡检 SQL 共同检查：

- 证据不在目标原文里的信号；
- 证据或命题引用了同单元其他目标原文的信号（程序做子串检查）；
- 作者数重复计算（同一作者的多条评论被算成多位）；
- 作者数少于 2 却处于正式状态的需求；
- 重定向成环，或指向不存在的需求；
- 同一 `(request, signal_ordinal)` 出现多行。

### 7.2 第二层：自一致性与扰动测试

在同一批真实评论上运行，计算以下指标：

| 测试 | 做法 | 指标 |
|---|---|---|
| 重复运行 | 同样的单元跑 3 次 | problem/need 检出一致率；**同属一致率**（两条评论是否归到同一需求，在多次运行中保持一致的比例） |
| 顺序扰动 | 打乱单元内目标的顺序 | 同上 |
| 组合扰动 | 同一条评论分别放进不同的单元 | 同上；同时检查单元里其他评论是否影响了它的信号 |
| 目录扰动 | 需求目录的顺序反转 | 匹配到的需求 ref 一致率（沿用 topic single-pass 评估的方法） |

### 7.3 第三层：跨模型复核

用另一家提供方、能力更强的模型，对抽样结果做独立复核：

- T1 的匹配决策：抽样的每个 `{ref}` 是否真的是同一需求；每个 `{newKey}` 是否其实应该匹配目录里已有的需求；
- T3 的合并决策：每个 `same` 是否成立；`different` 判定里有没有漏合并；
- 每条信号的 kind 是否符合 §3.2 的判定规则。

复核模型与执行模型的分歧率作为指标。**分歧条目汇总成清单（不超过 30 条）放在验收报告里**，负责人可以选择性浏览，这是知情浏览，不是标注任务。

### 7.4 第四层：Claude 标注参考集（回归基线）

- 由开发会话中的 Claude 按第 3 节规则标注约 150 条真实评论：期望的 problem/need 检出和需求分组。§1.3 的 33 条可以作为起点。
- **已知偏差：** 标注者和规则设计者是同一个模型，不能作为绝对质量的证明，只用来做**回归检测**：方法或提示词修改后，指标不能明显下降。
- 参考集存放在仓库里，作为测试夹具。

### 7.5 质量目标（上线后第一轮优化使用；暂定，首次实测后校准）

| 指标 | 门槛 |
|---|---|
| 第一层所有不变量 | 0 |
| 重复运行的同属一致率 | ≥ 0.85 |
| 跨模型复核：`{ref}` 匹配被判定为不同需求（误合并） | ≤ 5% |
| 跨模型复核：`{newKey}` 被判定为应该匹配已有需求（重复候选） | ≤ 15%（T3 之后再测一次，应 ≤ 5%） |
| 与参考集相比的 problem/need 检出召回 | ≥ 0.85 |
| 单元失败率（`unit_output_invalid` 加 `unit_truncated`） | ≤ 5% |
| 每 100 条评论的模型调用次数 | ≤ 10 |

误合并的门槛比重复候选严格，因为重复候选可以由 T3 修复，误合并只能靠撤销。

---

## 8. 实施阶段

### 落地路线

```text
G0 → G1 → P0（与 G1 并行）→ G2 → G3 → G5 → 上线 → G6 持续评估与优化
                                                  ↘ G4 向量（满足进入条件时）
```

G0–G3 和 G5 都是上线的必要部分，G4 不是。

### P0｜提示词冒烟测试（与 G1 并行，不阻断实施）

**目标：** 在接入执行器之前，确定提示词的定稿和单元大小，避免 G2 返工。**这一步不是质量门槛。**

- 用脚本从运行库只读取样本：选 2–3 个有 L1、评论数在 10–40 条之间的作品，外加 1 个评论数超过 100 的作品，用来测试切分。
- 需求目录从空开始，按 §5.3 的顺序串行运行，让目录自然增长。
- 单元大小分别试 10、20、30，记录截断率和输出长度。
- 检查 §7.1 的不变量在真实输出上能否成立（证据定位、别名映射、newKey 引用）。

**产出：** 提示词定稿、`UNIT_MAX_TARGETS` 的取值、一份简短记录（模型、调用次数、费用、发现的合同问题）。

**模型配置（负责人 2026-11-19 决定：与证据库当前 L1 保持一致）：** 直接引用 `content_understanding` 当前任务配置里的 `model_config_ref = 4cb338a3-b359-4129-9d2b-8f0cf4b143b2`。

| 项 | 值 |
|---|---|
| 模型 | DeepSeek `deepseek-flash` |
| 接口 | `openai-responses` |
| thinking_level | high |
| 输入上限 / 输出上限 | 64,000 / 40,000 tokens |
| 超时 / 重试 | 3,600 秒 / 1 次 |

G1 里给 `comment_understanding` 和 `comment_problem_consolidation` 注册任务配置时，同样指向这个 `model_config_ref`。L1 以后换模型时，评论研究是否跟着换，由负责人在任务配置里另行决定，不自动联动。

**对参数的约束：** 输入上限 64k 意味着 prompt 里"规则 + 需求目录 + L1 投影 + 目标"合计不能超过 64k。按 `CATALOG_FULL_MAX` = 200 条、每条约 80 tokens 计算，目录大约占 16k；L1 投影和 20 条目标通常在 10k 以内，余量充足。P0 要实测一个评论较多的作品的 L1 投影长度。输出上限 40k，远高于 `UNIT_MAX_OUTPUT_EST`（8k），截断风险主要来自 thinking 内容是否计入输出，P0 需要确认。

**授权：** 按 DEC-0006，真实模型调用仍需负责人对 P0 的执行单独确认（调用次数预计 20–40 次）。

### G0｜基线确认（2026-11-19 已完成，结果见附录 A）

- 已确认 §1.2 查询的库就是生产库；
- 决定退役旧流程，不写兼容层；
- 改动清单见附录 A.2；
- 新发现 Topic Map 下游依赖，已补入 §5.6 和 §6.3。

### G1｜迁移与合同（已实施，实施记录见附录 B）

- 完成 §4.2 的迁移（不含向量表）；
- Rust 端：两个合同的 DTO、JSON Schema 和接纳器；
- 需求当前状态视图及其测试：作者去重、重定向解析、撤销、作者数下降后状态回退。

**验收：** 迁移可以在隔离的 PostgreSQL 上反复应用；视图测试覆盖 §7.1 的所有不变量。

### G2｜研究单元纵向打通（T0、T1、T2 和全量目录模式）（已实施，记录见附录 C）

- 实现 T0 规划，包括资格筛选、切分和重试时的单元减半；
- 执行器支持 `CommentResearchUnit` 输入，加上同领域串行条件；
- prompt 组装与缓存友好的顺序；
- T2 接纳、newKey 建需求、写支持关系。

**验收：**
- 同一作品的评论共享一份 L1，不同作品不共享；
- 单条目标异常不影响同单元的其他目标；
- 截断后重试会自动减半；
- 重复投递不产生重复写入；
- 后一个单元能匹配到前一个单元新建的候选；
- 端到端跑通，§7.1 的不变量全部为 0：用合成的已保存 L1、真实执行器和本机回环提供方（**没有调用真实模型**）；在 P0 样本上的真实运行属于 P0。

### G3｜候选整理（T3，不使用向量的全量模式）与下游

- T3 全量模式、重定向和撤销；
- Opportunity 改为读取需求当前状态视图，删除对 `review_invocation_ref` 的依赖；
- T4 刷新标记。

**验收：**
- 跨单元的重复候选合并后，作者合计达到 2 位的自动成为正式需求；
- 撤销后状态恢复；
- Opportunity 卡上的作者数与视图一致。

### G4｜向量能力（只在需要时进入）

**进入条件（满足任一）：**
- 某个领域的需求数超过 `CATALOG_FULL_MAX` 或 `CONSOLIDATE_FULL_MAX`；
- §7.3 的跨模型复核显示，全量模式下出现了因目录过长导致的漏匹配。

**内容：**
- 需求定义向量表，复用 `pi_adapter` 的 WeMM 运行时；
- T3 的配对排序；
- T1 的召回模式。

**启用门槛：** 在参考集上，召回模式取前 `RECALL_TOP_K` 时的命中率 ≥ 95%，否则不启用召回模式，只把向量用于 T3 配对。

### G5｜前端

- 评论目录：九类标签（紧凑徽标，最多显示 3 个加 `+N`）、执行状态与研究结果分开展示；
- 选择预览按 §5.2 的内容；
- 评论详情：原声 → L1 语境 → 信号 → 关联需求及其状态和作者数；
- 需求列表与详情：定义、状态、作者数、评论数、作品数、最近出现时间、合并历史和撤销入口；
- 研究记录：单元、调用、用量、失败原因、"重试失败"、"整理候选"。

### 上线

**上线条件（全部满足）：**
1. G0–G3、G5 完成并合并，已部署；
2. 在一个真实领域上完成一次端到端运行：选择 → 规划 → 单元调用 → 接纳 → 候选整理 → 需求列表与 Opportunity 可见；
3. 这次运行中 §7.1 的不变量全部为 0；
4. 旧流程已退役，或者兼容层已经就位（按 G0 的决定）。

**交付状态**分开记录：已实现 / 已测试（列出具体测试）/ 已合并 / 已部署 / 生产已验证。

### G6｜上线后的评估与优化

- 先做 §7.2 的自一致性和扰动测试，建立 §7.4 的参考集，用 §7.5 作为第一轮优化目标；
- 负责人按 §7.3 事后抽查，开始时间由负责人决定。抽查发现的问题作为优化输入，不回滚已上线的流程；
- 之后定期运行（例如每 500 条新研究评论一次），结果写入研究记录；参考集回归测试纳入 CI，只使用离线录制的结果。

---

## 9. 不做事项

**沿用上一版：**
- 不新建独立的 AI Executor；
- 不增加常规 Pair Review 调用；
- 不把 L2 设为前置条件；
- 不批量重跑历史评论；
- 不把整个作品的评论塞进一个无上限的请求。

**本版新增：**
- 不给评论算向量，也不在 V2 默认路径里做召回；
- 不存储需求状态，不建正式化事件表；
- 不做候选 revision 校验和重新绑定；
- 不保留 `uncertain`；
- 不为"等待 L1"建状态和自动续跑；
- 不为作者 ID 缺失设计专门的界面或流程；
- G0 确认没有旧数据时，不写新旧兼容层；
- 不新增执行器的批量领取模式。

---

## 10. 负责人决定（2026-11-19）

1. **L1 必需，不允许降级。** L1 是评论研究的上下文。没有 L1 的作品通过"补 L1"入口走现有 L1 工作流（§2、§5.2）。
2. **接受同一领域串行执行**，大约每 300 条评论 10 分钟（§5.3）。
3. **候选整理在每次运行结束后自动执行**，合并可以撤销（§5.5）。
4. **跨模型抽查推迟到上线后**，由负责人事后进行（§7.3、G6）。
5. **优先保证整套计划完整落地上线，再做局部优化和测试。** 上线门槛只有程序不变量和端到端运行（§7、§8 上线）。

6. **P0 和评论研究使用与 L1 相同的模型配置**：`deepseek-flash`，thinking 为 high（§8 P0）。

仍待确认：

- P0 的执行授权（约 20–40 次真实调用）。

---

## 附录 A｜G0 基线确认结果（2026-11-19）

### A.1 生产库

| 检查项 | 结果 | 依据 |
|---|---|---|
| 运行时使用的数据库 | `127.0.0.1:55432/linggan_intelligence_dev`（容器 `linggan-intelligence-postgres-1`） | `runtime-approved-api-launch.sh` 加载 `runtime-main/.env`，其中 `POSTGRES_PORT=55432`、`POSTGRES_DB=linggan_intelligence_dev` |
| 运行时版本 | `8891d71`，与本计划的基线一致 | `runtime-identity.json`、`runtime-target-revision` |
| 迁移头 | `0229_retire_note_interpretation` | `linggan_local_schema_migration` |
| 评论研究、需求、Opportunity 评论提及的旧数据 | 0 行 | 只读计数 |
| 可用 L1 Core 的上限 | 810 个作品 / 12,135 条评论 | `linggan_content_understanding_saved_core`；实际可用数还要经过 `core_cards_in` 的资格复核 |

**结论：** 退役旧流程，不写兼容层（§4.2）。

### A.2 改动清单（基线 `8891d71`，均为非测试文件）

**规划与入口（T0）**

| 文件 | 改动 |
|---|---|
| `crates/intelligence/src/ai_foundation/comment_research.rs` | 重写 `start`（规划、切分、建单元）、`preview`（新预览字段）、`latest` 和 `read`（目标状态从结果和单元推导）、`frozen_in`（改为基于 L1 和评论原文可用性） |
| `crates/intelligence/src/comment_study_selection.rs`、`comment_study_source.rs` | 输入准备从 Research Dossier 改为 L1 投影：通过 `content_understanding::core_cards_in(tx, domain, work, cutoff)` 读取并冻结 analysisRef |
| `crates/intelligence/src/ai_foundation.rs` | `TaskKey` 增加 `CommentUnderstanding`、`CommentProblemConsolidation`，退役旧的三个 |
| `crates/intelligence/src/ai_foundation/batch_store.rs` | `InputRef::CommentResearch` 改为 `CommentResearchUnit { unit_ref }`；更新任务合法性检查 |
| `ai_foundation/admission.rs`、`task_config.rs`、`prompt.rs`、`provider_store.rs` | 同步更新任务键列表 |
| `apps/api/src/local_web/comment_study_command_api.rs` | 保留 preview、start、runs、cancel、read；新增"重试失败"、"整理候选"、"撤销合并"、需求列表和详情接口 |

**执行与接纳（T1、T2）**

| 文件 | 改动 |
|---|---|
| `ai_foundation/executor/queue.rs` | 领取任务时加同领域串行条件 |
| `ai_foundation/executor/service.rs` | 分发新的任务键 |
| `ai_foundation/executor/service/comment_research.rs` | 重写为单元调用和逐目标、逐信号的接纳（§5.3、§5.4、§6.1、§6.3） |
| `crates/intelligence/src/comment_study_semantic.rs`、`comment_cleaning` | 复用现有的证据定位和 `eligibilityState` 计算 |

**需求目录与整理（T3、T4）**

| 文件 | 改动 |
|---|---|
| `ai_foundation/comment_problem.rs` 及 `contract.rs`、`publish.rs`、`read.rs` | 删除哈希配对（`problem_identity`、`pair_support_in`、`proposals_in`）；读取改用需求当前状态视图；新增整理合同 |
| `ai_foundation/executor/service/comment_problem.rs` | 改为候选整理的执行 |
| `ai_foundation/executor/service/opportunity_sync.rs` | 接入新的来源事件 |

**下游读取**

| 文件 | 改动 |
|---|---|
| `opportunity/mentions.rs`（第 137 行附近） | 删除对 `review_invocation_ref` 的依赖，改读视图 |
| `opportunity/card/snapshot.rs`、`card/voices.rs`、`invariants.rs`、`problem_metrics.rs`、`problem_metrics/inputs.rs` | 改读视图 |
| `topic_map_research/comment_signals.sql`、`topic_map_research/source.rs`（第 345 行的 `frozen_in`） | 成功判断改为基于结果行；使用新版 `frozen_in` |
| `ai_foundation/topic_comment_source.rs`、`topic_comment_source/read.sql` | 同上 |
| `ai_foundation/topic_research_automation/comment_candidates.sql` | 同上 |
| `comment_study_catalog/facts.sql`、`history.sql`、`comments.sql` | 状态改为从 `unit_target`、单元 item 状态和结果行推导，不再读 `linggan_comment_research_stage` |
| `ai_foundation/comment_research_overview.rs`、`effective_results.sql`、`observation_series.sql` | 同上 |

**前端**（`apps/web/src/pages/corpus/comments/`）

`CommentResearch.tsx`、`CommentResearchSelection.tsx`、`CommentsPage.tsx`、`Overview.tsx`、`ResearchDashboard.tsx`、`commentResearchApi.ts`、`studyApi.ts`、`labels.ts`、`overviewDtos.ts`、`testKit.tsx`；设置页里的 `AiTaskSection.tsx` 等需要同步任务列表。

**迁移**

`database/migrations/0230_comment_understanding.sql`（内容见 §4.2）。

**需要更新的测试**

| 位置 | 测试 |
|---|---|
| `crates/intelligence/tests/` | `comment_research_executor_postgres.rs`、`comment_study_*_postgres.rs`、`opportunity_mentions_postgres.rs`、`opportunity_metrics_postgres*`、`support/opportunity_creator_dataset.rs` |
| 源码内 | `topic_comment_source/tests.rs`、`topic_map_research/tests/saved_signal_fixture.rs` |
| `apps/api/src/local_web/` | `research_browser_tests.rs` |

### A.3 G0 新发现的问题（已补入正文）

1. **Topic Map 依赖评论研究的结果**：信号字段格式、`frozen_in` 资格复核、批次成功判断三处。原计划没有覆盖，已补入 §5.6 和 §6.3。
2. **多处查询用"请求所属批次的 item 是否成功"来判断这条请求是否成功。** 旧流程是一条请求对应一个批次、一个 item，这样判断没问题。V2 里一个批次有多个单元，单元成功也不代表其中每条目标都成功，所以必须改为看结果行。涉及 `comment_signals.sql`、`topic_comment_source/read.sql`、`comment_candidates.sql`，以及读 stage 表的 `facts.sql`。
3. **L1 可用性会随作品素材变化**：`core_cards_in` 会用当前素材快照复核 L1 卡片的资格。V2 在规划时冻结 analysisRef；之后素材变化不影响已经接纳的结果（§2），只影响下次能否选中这些评论。

---

## 附录 B｜G1 实施记录（2026-11-19）

分支 `claude/comment-understanding-v2`（基线 `8891d71`）。交付状态：**已实现、已在隔离 PostgreSQL 测试；未合并、未部署。**

### B.1 交付内容

| 项 | 位置 |
|---|---|
| 迁移 | `database/migrations/0230_comment_understanding.sql`，已登记到 `scripts/local-runtime.sh` 与完整 schema 夹具 |
| 合同与接纳器（纯函数，无数据库访问） | `crates/intelligence/src/ai_foundation/comment_understanding/`：`contract.rs`（请求载荷、响应 Schema、别名）、`accept.rs`（单元接纳、结果 JSON）、`consolidation.rs`（`comment-problem-consolidation.v1`） |
| 任务键 | `TaskKey::CommentUnderstanding`，已加入外发材料资格、任务配置和提示词编辑的清单。**此时仍不能入队**：`BatchKind::permits` 对它返回 false，入队属于 G2 |
| 测试 | 16 个单元测试（接纳器 13、整理 3，含 Schema 与载荷）；隔离 PostgreSQL 测试 `crates/intelligence/tests/comment_understanding_schema_postgres.rs`（5 个），已加入 `scripts/test-comment-study-productization-postgres.sh` |

### B.2 对原计划的修正（已就地改正正文）

| 原计划 | 实际 | 原因 |
|---|---|---|
| 只改 `method_version` 约束 | 同时取消 `request.batch_ref`、`request.command_ref`、`result.invocation_ref` 的唯一约束 | 这三个唯一约束隐含"一条评论一个批次、一次调用"，与"一个单元一次调用服务多条评论"直接冲突。上一版计划没有发现 |
| 单元表有 `catalog_size`、`l1_result_ref` | 单元表有 `unit_ref`、`l1_request_ref`；新增 `unit_dispatch.catalog_problem_refs` | 批次输入引用是 JSON，必须先生成 `unit_ref`（沿用现有 `request_ref` 的做法）；L1 的身份就是 L1 请求；T2 必须能把别名还原成需求，所以目录要按顺序存下来 |
| "定义版本只追加" | 不建定义版本，`problem_ref` 保持唯一 | V2 没有任何操作会修改定义（合并走重定向），建了就是没人用的结构 |
| 重定向需要检测环 | 不需要：每条边都连接两个"没有生效重定向"的需求，加领域咨询锁串行化 | 这样图里任何一条边的终点都没有出边，数学上不可能成环；比递归检测简单且并发安全 |
| 同时注册两个任务 | 只注册 `comment_understanding` | 整理任务的派发表和输入引用在 G3 才有用，现在注册只会留下没人用的结构 |
| 迁移中把新任务指向 L1 的 `model_config_ref` | 不指向：任务以"未配置"注册，开关关闭 | 那是生产库里的一条配置记录，写进迁移会让任何其他环境（CI、新机器）的外键断掉。绑定是部署时通过任务配置流程做的一步（P0 开始前执行，沿用 L1 的 `4cb338a3-…`） |
| `problem` 字段为联合类型（ref 对象或 newKey 对象） | 一个对象，`ref`、`newKey` 两个字段恰好一个非 null | 不依赖提供方对 `anyOf` 的支持；由接纳器校验 |
| 退役旧任务和删旧表放进 `0230` | 放进切换时的 `0231`，与删旧代码同一个提交 | G1 时旧流程的代码和测试仍在；提前退役会让它们全红，无法保证每个提交可验证 |
| 视图列 `last_supported_at` | `last_researched_at` | 它取的是研究请求时间，不是评论的观察时间，名字要诚实 |

### B.3 验证

- 单元测试：`cargo test -p linggan-intelligence --lib comment_understanding`，16/16。
- 隔离 PostgreSQL（随机容器，用后清理）：`comment_understanding_schema_postgres` 5/5，覆盖——
  - 状态推导：同作者两条评论只算一位作者；第二位作者使其成为 formal 且需求身份不变；作者为空的评论不计入作者数；评论被限制后立即不再计入；同一作者 ID 在不同平台算两位；重新研究同一评论会替换其支持；失去第二位作者后回到 candidate；更新的失败请求不抹掉旧的成功结果；V1 研究不计入；合并后经重定向汇总，撤销后恢复；
  - 重定向：跨领域、自指、已重定向的需求再重定向、成环（包括 A→B→A）都被拒绝；链式解析正确；撤销后可以换形状；
  - 单元完整性：L1 与单元不同作品或不同领域、目标与单元不同作品／领域／批次／方法版本、同一请求进两个单元、位置重复，均被数据库拒绝；
  - 一个批次、一个命令、一次调用可以服务多条请求；
  - 新任务以停用、未配置状态注册，约束包含该任务。
- 回归：见 B.4。

### B.4 回归（迁移 `0230` 应用后，原有测试）

| 范围 | 结果 |
|---|---|
| 隔离 PostgreSQL：`comment_study_catalog` 3、`catalog_read` 11、`inspection` 4、`p1_closure` 3、`start` 1、`work_catalog` 4 | 全部通过 |
| 隔离 PostgreSQL：`ai_foundation_postgres` | 33/33（需要 `AI_FOUNDATION_PROOF_NODE`、`LINGGAN_CREATION_ASSET_ROOT` 和 `apps/pi-adapter` 依赖；缺这些时的失败是环境问题，不是代码问题） |
| 隔离 PostgreSQL：`opportunity_postgres` 13、`opportunity_mentions_postgres` 6、`opportunity_asof_postgres` 4 | 全部通过 |
| 隔离 PostgreSQL：V1 三阶段执行器 `comment_uses_target_dossier_and_both_permissions` | 通过：旧流程在新 schema 上仍可运行 |
| `cargo test -p linggan-intelligence --lib` | 422 通过，19 个忽略 |
| `cargo check --workspace --tests`、`scripts/ci-preflight.sh origin/main` | 通过 |
| `cargo test -p linggan-api`（非 PG） | 115 通过，1 失败：`creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work`（503 而非 200）。**在未改动的基线 `8891d71` 上同样失败**，与本包无关，未处理 |

**没有跑：** `test-comment-study-productization-postgres.sh` 的完整脚本（上面是用等价的隔离运行器逐个跑了其中的测试目标，未含 `event_gate_*` 两个门控用例和 worker 的 `startup_contract`）；Topic Map、Evidence、Web 应用等其他 CI 套件，依赖 CI 在 PR 上全量触发（未知路径会触发全量）。

### B.5 未验证边界

- 视图在真实数据量下的性能没有测：本地库里评论研究表为空，只有合成数据。
- 接纳器没有对真实模型输出验证。这是 P0 的工作；提示词是草案 v1，P0 后以新版本保存。
- 草案提示词里的"同一需求"反例来自本机 33 条评论的向量实验（§1.3），样本小、标签由 Claude 标注。

---

## 附录 C｜G2 实施记录（2026-11-19）

同一分支同一 PR。交付状态：**已实现、已在隔离 PostgreSQL 测试；未合并、未部署；没有调用真实模型。**

### C.1 交付内容

| 项 | 位置 |
|---|---|
| T0 规划 | `comment_understanding/start.rs`（`preview`、`start`）、`plan.rs`（纯函数：按作品分组、切分、截断后减半、`planHash`）、`target.rs`（评论读取与资格） |
| T1/T2 执行 | `executor/service/comment_understanding.rs`（准备、调用、结算）、`comment_understanding/publish.rs`（结果、需求、支持的同事务写入）、`unit.rs`、`catalog.rs`、`context.rs`（L1 投影） |
| 队列接线 | `InputRef::CommentResearchUnit`、批次类型、准入（需要 note 与 comment 两类外发许可）、领取条件加入 `comment_understanding` 并加同领域串行（`executor/comment_understanding_order.sql`） |
| 运行回执与取消 | `comment_understanding/run.rs`（`read_run`：单元、目标状态与失败码、调用数与 Token；`cancel`） |
| 下游读取 | `comment_research::current_source_in`：按方法版本分发（V1 仍按冻结输入复核，V2 只要求评论仍可读）；Topic Map、Topic 评论来源、评论概览改用它。三处 SQL 的"成功"改为"有成功结果行"；评论目录的研究状态加上 V2 分支 |
| 测试 | `comment_understanding/execution_tests.rs`（8 个端到端）；`tests/support/comment_understanding_response.rs`（按评论文本决定回复的确定性回环提供方）；已加入 `scripts/test-comment-study-productization-postgres.sh` |

### C.2 与计划的差异

| 计划 | 实际 | 原因 |
|---|---|---|
| 同作品内按平台时间切分 | 按（讨论串根评论 ID，评论 ID）排序 | 平台时间表只覆盖部分评论，且需要另一套读取；按讨论串根排序已让同一讨论串相邻 |
| 准入只有三条 | 沿用 V1 来源门：创作者本人的评论排除，**作品作者未知也排除**；放开的只有"评论者作者未知" | 作品作者未知时分不出创作者的声音。这是保守的沿用，不是新决定；要放开需要 Mog 确认 |
| 失败目标自动重试 | 不自动重试；运营重新选择即可，预览只含未成功的 | 少一个状态机；截断次数从历史结果推导 |
| 目录过大时召回 | 跳过整个单元并给出原因 | 召回属于 G4；截断目录会造成重复需求 |
| `uncertain`、批内重试等 | 与 G1 一致，未引入 | — |
| 任务失败码需要新迁移 | 不需要迁移 | 批次条目的失败码是自由文本；只有调用账本的失败码有外键，已在 `0230` 注册 |

### C.3 验证

- 单元测试 25 个（规划、目标指纹、L1 投影、接纳器、整理合同）。
- 端到端（`execution_tests`，隔离 PostgreSQL，真实执行器，本机回环提供方）8 个，覆盖：
  - 两个作品两个单元、**两次调用而不是每条评论一次**；调用载荷不含评论者身份；
  - 同一单元内多条评论共用一个新需求；第二个单元读到第一个单元写入的目录，匹配而不重复新建；三位作者、两个作品使需求变为 formal；
  - 目标缺失、证据不在原文的目标各自失败，其余目标不受影响；证据范围指向原文中的正确位置；
  - 截断（`unit_truncated`）和无法使用的输出（`unit_output_invalid`）使整个单元失败，不写任何需求；
  - 缺 L1 的作品不进计划；已研究、已排队的评论不重复；
  - 启动幂等（同一命令重放不重复创建）、计划过期和任务版本过期被拒绝；
  - 取消后不调用提供方，所有评论为取消状态；
  - Topic Map 的信号查询、`current_source_in`、Topic 评论来源都读到 V2 结果；评论被限制后立即不再计入，需求退回候选；评论目录显示 V2 状态与信号类型；
  - 独立于视图重新计数的不变量为 0：证据范围在本条评论内、信号指向自己的来源、支持不跨领域、formal 的作者数不少于 2。
- 回归见 C.4。

### C.4 回归（G2 的全部改动之后，原有测试）

| 范围 | 结果 |
|---|---|
| 隔离 PostgreSQL：`comment_study_catalog` 3、`catalog_read` 11、`inspection` 4、`p1_closure` 3、`start` 1、`work_catalog` 4 | 全部通过（目录状态 SQL 已加 V2 分支） |
| 隔离 PostgreSQL：`comment_understanding_schema_postgres` | 5/5 |
| 隔离 PostgreSQL：`ai_foundation_postgres` | 33/33 |
| 隔离 PostgreSQL：`opportunity_postgres` 13、`opportunity_mentions_postgres` 6、`opportunity_asof_postgres` 4 | 全部通过 |
| 隔离 PostgreSQL：V1 三阶段执行器 `comment_uses_target_dossier_and_both_permissions` | 通过：旧流程在 G2 之后仍可运行 |
| lib 层 PostgreSQL（`topic_comment_source`、`topic_map_research`、`topic_research_automation`、`comment_study`、`content_understanding`、新的 `execution_tests`） | 15 通过，1 失败：`content_understanding::topic_tests::saved_core_drives_topic_queue_and_persisted_membership`（`topic_research_automation/tests.rs:29`，期望 4 个阶段、实际 1 个）。**在未改动的基线 `8891d71` 上同样失败**，与本包无关，未处理 |
| **机会卡全套**（`opportunity::card::snapshot/groups` lib 测试、`opportunity_metrics_postgres`、机会卡 API 证明） | **G2 提交时本地没跑，被 PR 的 CI 抓到**：机会卡取快照时审计"实际用到的表都在 `SOURCE_TABLES` 里"，G2 的目录状态 SQL 新读了 `linggan_comment_research_result/_unit/_unit_target`，名单里没有，整组 `card::*` 和机会卡浏览器链路失败。已把三张表加入 `opportunity/card/snapshot.rs` 的 `SOURCE_TABLES`。修复后本地：lib 5/5，`opportunity_metrics_postgres` 47/48（唯一失败的 `card_uses_canonical_metrics_debounces_and_removes_withdrawn_voices` 是 `SourcesBusy` 锁时序偶发，单独连跑 3 次通过），API 证明通过；浏览器链路本地未跑，靠 CI |
| `cargo test -p linggan-intelligence --lib` | 431 通过，26 个忽略 |
| `cargo check --workspace --tests`、`scripts/ci-preflight.sh origin/main` | 通过 |
| `cargo test -p linggan-api` | 沿用 B.4：1 个基线上同样失败的页面测试，未重复运行 |

环境说明：端到端和部分回归需要 `AI_FOUNDATION_PROOF_NODE`、`apps/pi-adapter/node_modules`、`LINGGAN_CREATION_ASSET_ROOT`、`LINGGAN_LOCAL_MEDIA_ROOT`；缺这些时的失败是环境问题（见交接文档第 8 节）。

### C.5 未验证边界

- 真实模型的输出质量、截断率、单元大小：属于 P0。
- 同领域串行靠第二个单元读到第一个单元的目录来证明（并发度为 4 时若没有串行条件，第二个单元的目录会是空的），没有直接测并发竞争的时序。
- 视图和规划在真实数据量下的性能：本机评论研究表仍是空的。
- 取消在"单元正在调用提供方"时的行为沿用现有批次取消机制（已在调用中的单元正常结算），没有为本任务单独测。
