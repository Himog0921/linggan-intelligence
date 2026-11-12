# 机会卡（Opportunity Card）实施手册

- 版本：v1.3（2026-11-12 修订，见 §0.5）
- 日期：2026-11-12
- 读者：负责推进的执行 agent（下称“执行者”），以及分派任务、审查结果的项目负责人（下称“负责人”）
- 性质：本手册是实施的**唯一规格来源**。执行者看不到产品讨论的上下文，所以必要的背景都写在这里。

---

> 状态: 活跃计划
> 最后核对: 2026-11-12
> 适用范围: 机会卡 v1 的 T0.1b 至 T3 与采集侧 Stage C 的任务卡、验收标准与实施顺序。
> 事实来源: 负责人 2026-11-12 的裁定（DECISIONS D-001 至 D-031），以及 T0.1/T0.1b 勘察结果与实际代码、迁移。
> 冲突时以谁为准: DECISIONS.md 的最新裁定；两者仍有出入时在 QUESTIONS.md 提出。

## 0. 使用说明

### 0.1 给执行者：开工前必须做的三件事

1. 完整读完 §1、§2、§3，然后才能修改任何代码。
2. 每个任务开始前，重读该任务卡和 §2 的硬规则。
3. 第一个任务永远是 T0.1（代码库勘察）。勘察结果写在 `docs/opportunity/RECON.md` 里。之后所有任务涉及现有代码时，都以它为准。

### 0.2 优先级

冲突时按以下顺序执行：

1. §2 硬规则
2. 任务卡中的“验收标准”和“禁止”
3. §6 核心规格
4. 本手册其余内容

如果手册和代码现实冲突（例如手册假设某张表存在，实际不存在），按 §3.6 处理：**停下，记录，询问**。不要自行“修正”手册，也不要绕过去。

本手册与此前的技术方案文档有出入时，以本手册为准。例如：提及率的分母改为直接从评论表按发布时间统计，不再单独维护采集量表。

### 0.3 占位符约定

文中写作 `<<RECON:名称>>` 的地方，必须替换为 RECON.md 里记录的真实值，例如 `<<RECON:comments_table>>`。**禁止猜测占位符的值。** 如果 RECON.md 里没有，就先补勘察。

### 0.4 SQL 方言

RECON 已确认数据库为 PostgreSQL 16（含 pgvector）。文中 SQL 均为 PostgreSQL。

### 0.5 v1.1 修订记录

T0.1 勘察后，负责人对 10 个阻塞问题作出裁定，记录在 `docs/opportunity/DECISIONS.md` 的 D-001 至 D-020。**DECISIONS.md 与本手册具有同等约束力**；两者如有出入，以 DECISIONS.md 为准，并在 QUESTIONS.md 提出。本版同步修改了以下内容：

| 位置 | 修改 | 依据 |
| --- | --- | --- |
| §2 R12、§5、§6.2、§8 | “首次采集时间”统一改名为 `first_observed_at`，由视图推导，不改现有表 | D-007 |
| §6.2 | 统计周期按**作品发布周**归期（`period_basis = work_published_cohort`） | D-008 |
| §6.2 | 领域分母、作者归属、排除计数的具体规则 | D-009、D-010、D-012 |
| §6.1、§6.2、§6.7 | 评论身份改为 `(content_ref, comment_external_id)` | D-011 |
| §6.1、§6.7 | 物理表名前缀、路由、camelCase 字段、id 类型跟随现有 | D-004 |
| §4、§10 | 迁移只能向前，不改现有表 | D-005 |
| §6.4.3、T0.3 | 周期计算放在 PostgreSQL 函数中 | D-020 |
| T0.6 | 评论者哈希的密钥、算法与未知评论者处理 | D-013 |
| T0.5、T1.1 | 问题来源为 0177 实体 | D-014 |
| §6.10 | 增加评论抽样上限、时间基准的说明 | D-008、D-016 |
| §7 | 新增 T0.1b（基线同步与补充勘察）；改写 T0.2 | D-001 等 |
| §8.2 | 测试数据集增加排除用例 | D-010、D-012、D-013 |
| §6.1.3 | SQLite 对照作废 | RECON |
| **v1.2** §6.3、§6.4.4、§6.5、§8.2、T0.3 | 测试参数集与生产参数分开；生产 v1 为 `min_n_period = 100`、`creator_min_n = 30` | D-024 |
| **v1.2** §6.2.1 | 领域归属按 `created_at <= as_of`，迁移回填行自始有效 | D-023 |
| **v1.2** T0.2 | 验收增加分布报告和回复占比 | D-024、D-025 |
| **v1.2** T0.6 | 密钥路径由与 `model_secrets` 相同的解析函数得出 | D-022 |
| **v1.2** §6.6.2、T2.1 | 判定输入优先使用整篇材料快照的四个字段 | D-026 |
| ~~**v1.2** D-008~~ | ~~评论时间的 v2 计划暂停~~（已被 v1.3 取代） | D-021 |
| **v1.3** §7 | 新增**阶段 C：评论时间与采集协议**（C1–C4），与阶段 0、1 并行 | D-028、D-030 |
| **v1.3** §6.2.1 | 统计周期支持两种基准；评论发布时间为目标口径，作品发布周为过渡口径 | D-028 |
| **v1.3** §6.3 | 生产参数恢复为与测试参数集相同；新增采集目标参数 | D-029 |
| **v1.3** §6.7 | 新增 flags `collection_below_target`、`collection_protocol_changed`；新增排除原因 `comment_time_unknown` | D-028、D-029、D-030 |

---

## 1. 背景：执行者必须理解的最少上下文

### 1.1 现有产品

一个面向小红书内容研究的“语料库”系统，已有三个界面：

- **证据库**：浏览作品（笔记），查看单篇作品的底稿和整篇理解结果。
- **评论研究**：从评论中归并出“用户问题”。一个问题至少有 2 位不同评论者支持才算“已核验”，只有 1 位的叫“待支持候选”。
- **创作者**：浏览作者，管理观察关系。

已有能力（以 RECON 实测为准）：

- Rust 后端（本地 Web 服务）
- Web 前端
- 独立的采集侧，通过任务和回执与主系统交互
- AI 任务队列，具备预算、许可、提示词与 Skill 版本管理
- 作品的整篇理解结果
- 来源资格管理：撤回或受限的来源不再展示

### 1.2 要做什么

做一个以“用户问题”为枢纽的**机会卡**，在一页里同时回答三件事：

- **需求**：多少人在提这个问题，趋势如何。
- **供给**：现有作品有没有正面回答它。
- **渠道**：哪些作者的评论区里这个问题最集中。

配套做一个**研究集合**：把证据和人的判断冻结成可交接的版本。

### 1.3 科学性约束（硬规则的由来）

以下结论已经过专门审查。它们是需求本身，不是可选项：

| 约束 | 含义 | 落到实现上 |
| --- | --- | --- |
| 提及多不等于重要 | 已有研究显示，需求被提及的频次和它的重要性只有弱相关 | 指标只能叫“提及人数”，不能叫“支持人数”或“重要性”；页面必须显示“重要性未校准” |
| 评论有自选择偏差 | 留言的人不代表全体用户 | 不得用评论数据推算“多少比例的用户有此问题” |
| 采集量变化会造成假趋势 | 多采了评论，看起来就像“升温” | 趋势用提及率（比例），不用绝对增量 |
| 小样本比例不稳 | 3/40 不比 7/120 更可信 | 用 Wilson 区间下界排序；分母不足的不参与排序 |
| AI 判定必须验证 | 大模型标注在不同任务上表现差异很大 | 质量门通过前，AI 结果只能显示为“AI 建议” |
| 回测不能用未来信息 | 用今天的分类去算过去，等于提前知道答案 | 所有查询支持 `as_of`，只使用当时已存在的数据和定义 |

### 1.4 范围

**范围内**：§7 列出的五个阶段。

**范围外（不要做）**：
- 多平台接入
- 自动生成选题文案
- 问卷系统本身（只做结果导入）
- 账号与权限体系重构
- 新建嵌入模型或向量数据库（除非 RECON 确认已有，见 T2.2）

---

## 2. 硬规则（Invariants）

每条规则都有对应的验证方法。任务验收时，必须逐条确认相关规则的验证结果。

| # | 规则 | 验证方法 |
| --- | --- | --- |
| R1 | **事实层只追加。** 不修改、不删除评论、作品、人工标注的内容字段。撤回走现有来源资格机制。 | 代码审查：新代码中不出现对事实表的 `UPDATE` 或 `DELETE`。唯一例外是 T0.2 一次性回填新增列。 |
| R2 | **派生数据必带版本。** 每一行派生数据都记录产生它的 `taxonomy_version`、`metric_def_version`；AI 结果还要记录 `prompt_version`、`model`、`content_hash`。 | 表结构上这些列为 `NOT NULL`（人工记录按 §6.6.5 填固定值）。 |
| R3 | **阈值只来自 `metric_definition`。** 业务代码里不得写死任何阈值、z 值、窗口天数、最小分母。 | `grep` 新模块中的数字字面量，逐个解释；seed 迁移和测试代码除外。 |
| R4 | **前端不计算指标。** 前端只渲染 API 返回的值和质量标记，不做除法、不比较阈值、不推导等级。 | 代码审查。 |
| R5 | **缺数据就显式为空。** `n = 0` 时比例和区间为 `null`，同时给出原因标记。严禁用 0 或默认值顶替。 | 单元测试 + §8.3 不变量 SQL。 |
| R6 | **AI 结果受质量门约束。** 对应质量门未通过时，AI 结果的状态只能是 `ai_suggested`，界面文案为“AI 建议”。 | API 快照测试覆盖两种门状态。 |
| R7 | **人工优先。** 同一（作品，问题）对同时有人工和 AI 结果时，有效结果取人工。 | 单元测试。 |
| R8 | **作者排序规则固定。** 分母 `n < creator_min_n` 的作者不参与排序，单独返回。排序键依次为：`ci_low` 降序，`n` 降序，`creator_id` 升序。 | 单元测试（含并列情况）。 |
| R9 | **评论者身份只存哈希。** 新表只存 HMAC 哈希；日志、导出、新 API 不出现评论者原始 ID 或昵称。作者（creator）不受此限。 | `grep` 日志语句；API 快照测试。 |
| R10 | **撤回内容不外泄。** 已撤回或受限来源的正文和图片，不得出现在任何新接口、读模型、导出中，只能以墓碑形式出现。 | 集成测试：撤回一条后，重建读模型并检查各接口输出。 |
| R11 | **时间统一。** 时间戳一律以 UTC 存储。统计周期按 `metric_definition.period_tz`（默认 `Asia/Shanghai`）的自然周切分，周一 00:00 开始。 | 周期边界单元测试（§6.4.3）。 |
| R12 | **时点一致。** 任何带 `as_of` 的计算，只能使用 `first_observed_at <= as_of` 的事实，以及 `created_at <= as_of` 的定义版本。 | 回测集成测试。 |
| R13 | **任务幂等。** 重复执行同一任务不产生重复行。用唯一约束保证，而不是靠“先查后插”。 | 同一任务连跑两次，行数不变。 |
| R14 | **只新增，不破坏。** 不改变现有接口和页面的行为。新功能放在特性开关 `LINGGAN_OPPORTUNITY_CARD` 之后，默认关闭（D-006）。 | 现有测试结果与 PROGRESS.md 记录的既有失败基线一致（D-002）；开关关闭时界面与现状一致。 |
| R15 | **AI 输出不合格就丢弃。** 大模型输出必须通过结构校验和引文校验（§6.6.3），否则丢弃并记录原因。**不得把无效输出当成 `none`。** | 单元测试：构造非法输出，确认不写入判定表。 |

---

## 3. 防幻觉工作协议

这一节针对执行者最容易犯的错误：编造不存在的函数、表、字段、依赖或接口，以及为了让测试通过而改写逻辑。

### 3.1 先勘察，后编码

- T0.1 产出的 RECON.md 是现有代码的“地图”。
- **RECON.md 中没有记录的现有符号（表、列、函数、路由、类型、配置项），一律视为不存在。** 需要用时，先勘察并补进 RECON.md，然后才能引用。

### 3.2 引用即验证

- 每个任务的 PR 描述必须有“依赖的现有符号”表（模板见附录 C），每一行写明 `文件路径:行号`。
- 负责人会抽查。找不到出处的引用视为编造，整个任务退回。

### 3.3 遇到以下情况立即停止并询问

不要“先按合理假设写一版”。写进 `docs/opportunity/QUESTIONS.md`，把任务标为 `blocked`：

1. 数据库类型、迁移工具、ORM 或查询方式无法确定。
2. 现有评论数据里没有可用于去重的稳定评论者标识。
3. 现有“用户问题”的数据结构与 §6.1 的映射方式不清楚。
4. 评论缺少发布时间，或者无法确定首次采集时间的来源。
5. 采集侧不支持某个任务需要的能力（例如定向搜索补采）。
6. AI 任务队列的提交、回执、预算接口与预期不一致。
7. 任何硬规则看起来无法满足。
8. 需要新增第三方依赖。

### 3.4 禁止清单

- 编造 crate、npm 包或它们的 API。新增依赖必须先在 QUESTIONS.md 申请，并说明理由。
- 编造小红书平台字段。只能使用现有数据契约里已有的字段。
- 编造采集侧或 AI 队列的接口。
- 在已合并的代码里留 `todo!()`、`unimplemented!()`、空函数体，或返回假数据的桩。
- 为了让测试通过而修改测试、放宽断言、改动 §6.4 的标准答案。
- 捕获错误后返回 0、空列表或默认值继续往下走。错误必须向上传递，或者显式变成带原因的 `null`。
- 在非测试代码里对可能失败的 IO、解析、数据库操作使用 `unwrap()` 或 `expect()`。
- 静默跳过数据行。凡是被跳过的行，都要计数并记录原因。
- 改动现有公共接口的签名或行为。

### 3.5 验证闭环：没有贴出输出，就不算完成

每个任务结束时，在 PR 描述中贴出以下命令的**实际输出摘要**。命令以 RECON.md 记录的为准：

1. 构建
2. 全部测试
3. Lint（例如 `clippy`，按 RECON 记录的配置）
4. 前端类型检查（涉及前端时）
5. §8.3 中与本任务相关的不变量 SQL 及其结果
6. 涉及页面时，提供截图或渲染结果

只说“已通过”而不贴输出，视为未完成。

### 3.6 冲突处理

手册与代码现实冲突时：

1. 在 `docs/opportunity/DECISIONS.md` 新增一条，写明背景、可选方案、建议、影响范围。
2. 如果冲突涉及 §2 硬规则或 §6 规格，**停止并等负责人决定**。
3. 只有不涉及硬规则和规格的实现细节（例如模块放在哪个目录），执行者可以自行决定，并记录在案。

### 3.7 小步提交

- 一张任务卡对应一个分支、一个 PR。
- 除迁移和测试外，单个 PR 的改动建议不超过约 400 行。超出时先拆分，并说明理由。

### 3.8 完成前自检

提交前逐条回答，答案写进 PR 描述：

1. 我引用的每个现有符号都在 RECON.md 里吗？
2. 我新增的每个数字字面量都来自 `metric_definition` 或测试吗？
3. 有没有任何一处把“缺数据”变成了 0？
4. 有没有任何一处让 AI 结果绕过了质量门？
5. 重复运行我的任务会产生重复行吗？
6. 撤回一条来源后，我的输出里还会出现它的正文吗？
7. 我是否修改了任何测试的期望值？如果改了，原因是什么，负责人是否批准？
8. 有没有被我跳过、但没有计数和记录的数据？

---

## 4. 工作区与产物约定

```
docs/opportunity/
  RECON.md        # 代码库勘察结果（T0.1）
  PROGRESS.md     # 任务进度日志（格式见 §9.5）
  DECISIONS.md    # 设计决定与冲突记录
  QUESTIONS.md    # 阻塞问题，等待负责人回答
  GLOSSARY.md     # 复制自 §5，执行中只增不改
```

- **代码位置**：后端业务逻辑放 `crates/intelligence/src/opportunity.rs` 与 `opportunity/`；接口放 `apps/api/src/local_web/opportunity*.rs`；前端页面放 `apps/web/src/pages/opportunity/`（D-004）。
- **迁移文件**：遵循仓库的前向迁移约定，登记到 `scripts/local-runtime.sh`；只创建 `linggan_opportunity_*` 对象，不 `ALTER` 现有表（D-005）。
- **特性开关**：环境变量 `LINGGAN_OPPORTUNITY_CARD`，只有 `enabled` 为开启（D-006）。

---

## 5. 术语与命名（强制）

代码、接口、数据库、文档中统一使用以下标识。**不得自创同义词。**

| 中文 | 代码标识 | 取值或类型 |
| --- | --- | --- |
| 用户问题 | `problem` | 实体 |
| 问题层级 | `level` | 1、2、3 |
| 问题谱系 | `problem_lineage` | 操作：`merge`、`split`、`rename`、`reparent` |
| 分类版本 | `taxonomy_version` | 整数，按领域递增 |
| 问题归属（评论提及） | `comment_mention` | 实体 |
| 提及人数 | `mentioners` | 去重评论者数 |
| 本期提及数 | `k` | 本期内各评论者对该问题的首次提及评论数 |
| 本期评论数 | `n` | 本期领域内有效评论数 |
| 提及率 | `rate_per_1k` | `k / n × 1000`；`n = 0` 时为 `null` |
| 置信区间 | `ci_low`、`ci_high` | 存储为 0–1 的比例；接口层再换算为每千 |
| 基线 | `baseline_k`、`baseline_n` | 前 B 期合并 |
| 证据等级 | `evidence_tier` | `candidate`、`established`、`significant` |
| 作品回答判定 | `work_problem_judgment` | 标签：`answers`、`mentions`、`none` |
| 判定来源 | `source` | `ai`、`human` |
| 判定状态 | `judgment_status` | `verified`、`ai_suggested` |
| 供给状态 | `supply_state` | `not_evaluated`、`awaiting_understanding`、`insufficient_coverage`、`covered`、`suspected_gap` |
| 供给侧覆盖率 | `coverage` | `collected / found` |
| 作者问题集中度 | `creator_problem_density` | 实体 |
| 样本不足 | `insufficient_sample` | 质量标记 |
| 重要性未校准 | `not_calibrated` | 质量标记 |
| 质量门 | `quality_gate` | 实体 |
| 研究集合 | `research_collection` | 实体 |
| 集合快照 | `collection_snapshot` | 实体，冻结后不可变 |
| 墓碑 | `tombstone` | 撤回内容的占位表示 |
| 口径定义 | `metric_definition` | 实体 |
| 数据截至 | `as_of` | UTC 时间戳 |
| 首次采集时间 | `first_observed_at` | UTC 时间戳，由视图推导（D-007） |
| 作品发布时间（周期锚点） | `work_published_at` | UTC 时间戳（D-008） |
| 统计周期基准 | `period_basis` | v1：`work_published_cohort`（D-008） |
| 评论身份 | `content_ref` + `comment_external_id` | uuid + text（D-011） |
| 资格排除原因 | `eligibility_exclusion` | `period_anchor_missing`、`work_author_unknown`、`creator_self`、`restricted`、`observed_at_unparseable`、`not_in_domain_primary`（D-012） |

完整追加项见 `GLOSSARY.md` 的 2026-11-12 追加节。

---

## 6. 核心规格

### 6.1 数据表

#### 6.1.1 设计约定

- 新表放在 `public` schema，物理表名加前缀 `linggan_opportunity_`（D-004）。下文 DDL 为了可读，省略了前缀。
- id 类型跟随现有：`domain_ref`、`problem_ref`、`content_ref` 等使用现有的 uuid 或 text 类型（D-014）。下文中 `domain_id BIGINT` 一律读作 `domain_ref` 及其现有类型。
- 机会卡内部生成的自增主键（如 `problem.id`）保留 `BIGSERIAL`。
- 事实表只增不改（R1）。派生表允许按主键覆盖写入（upsert），因为它们随时可以重算。
- `collection_snapshot` 冻结后不可变，用数据库触发器禁止 `UPDATE` 和 `DELETE`。

#### 6.1.2 表定义（PostgreSQL）

```sql
-- ===== 口径配置 =====
CREATE TABLE metric_definition (
  version         INT          PRIMARY KEY,
  params          JSONB        NOT NULL,        -- 见 §6.3
  created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
  created_by      TEXT         NOT NULL,
  note            TEXT
);
CREATE TABLE metric_definition_current (
  singleton       BOOLEAN      PRIMARY KEY DEFAULT TRUE CHECK (singleton),
  version         INT          NOT NULL REFERENCES metric_definition(version),
  switched_at     TIMESTAMPTZ  NOT NULL
);

-- ===== 问题登记 =====
CREATE TABLE taxonomy_version (
  domain_ref      <现有类型>   NOT NULL,
  version         INT          NOT NULL,
  created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
  note            TEXT,
  PRIMARY KEY (domain_ref, version)
);
CREATE TABLE problem (
  id                    BIGSERIAL    PRIMARY KEY,
  domain_ref            <现有类型>   NOT NULL,   -- 跟随 0177 的 domain_ref
  level                 SMALLINT     NOT NULL CHECK (level IN (1,2,3)),  -- v1 导入一律为 1（D-014）
  parent_id             BIGINT       REFERENCES problem(id),
  statement             TEXT         NOT NULL,
  def_version           INT          NOT NULL DEFAULT 1,
  status                TEXT         NOT NULL CHECK (status IN ('active','merged','split')),
  source_ref            <现有类型>   NOT NULL,   -- 0177 的 problem_ref（D-014）
  source_definition_ref <现有类型>   NOT NULL,   -- 0177 当前的 definition_ref
  created_at            TIMESTAMPTZ  NOT NULL DEFAULT now(),
  created_by            TEXT         NOT NULL,
  UNIQUE (source_ref)
);
CREATE TABLE problem_lineage (
  id              BIGSERIAL    PRIMARY KEY,
  from_problem_id BIGINT       NOT NULL REFERENCES problem(id),
  to_problem_id   BIGINT       NOT NULL REFERENCES problem(id),
  op              TEXT         NOT NULL CHECK (op IN ('merge','split','rename','reparent')),
  taxonomy_version INT         NOT NULL,
  created_at      TIMESTAMPTZ  NOT NULL DEFAULT now(),
  created_by      TEXT         NOT NULL,
  CHECK (from_problem_id <> to_problem_id)
);

-- ===== 派生：评论提及 =====
CREATE TABLE comment_mention (
  content_ref              UUID        NOT NULL,   -- 评论身份第一部分（D-011）
  comment_external_id      TEXT        NOT NULL,   -- 评论身份第二部分
  problem_id               BIGINT      NOT NULL REFERENCES problem(id),
  taxonomy_version         INT         NOT NULL,
  commenter_hash           TEXT,                   -- HMAC-SHA256 十六进制；评论者未知时为 NULL（D-013）
  commenter_digest_version TEXT        NOT NULL,   -- v1：'commenter-hmac-sha256-v1'
  source_material_ref      UUID,                   -- 来源观察，只用于溯源
  source_membership_ref    TEXT        NOT NULL,   -- 0177 membership 的引用，格式见 RECON
  assigned_by              TEXT        NOT NULL CHECK (assigned_by IN ('ai','human','migrated')),
  excluded_reason          TEXT        CHECK (excluded_reason IN ('marketing','commenter_unknown')),
  created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (content_ref, comment_external_id, problem_id, taxonomy_version),
  CHECK ((commenter_hash IS NULL) = (excluded_reason IS NOT DISTINCT FROM 'commenter_unknown'))
);
CREATE INDEX ON comment_mention (problem_id, taxonomy_version);
-- 作者、作品发布时间、首次采集时间不冗余存储：一律在计算时经 §6.2.1 的资格函数按 as_of 取得，
-- 以保证 R12（as_of 一致）。creator_self 由资格函数判定，不在本表标记。

-- ===== 派生：问题指标 =====
CREATE TABLE problem_metric_period (
  problem_id         BIGINT      NOT NULL REFERENCES problem(id),
  period_start       DATE        NOT NULL,   -- period_tz 下的周一日期
  taxonomy_version   INT         NOT NULL,
  metric_def_version INT         NOT NULL,
  k                  INT         NOT NULL CHECK (k >= 0),
  n                  INT         NOT NULL CHECK (n >= 0),
  ci_low             DOUBLE PRECISION,       -- n=0 时为 NULL
  ci_high            DOUBLE PRECISION,
  baseline_k         INT         NOT NULL,
  baseline_n         INT         NOT NULL,
  mentioners_total   INT         NOT NULL,   -- 截至 as_of 的累计去重提及人数
  evidence_tier      TEXT        NOT NULL CHECK (evidence_tier IN ('candidate','established','significant')),
  computed_as_of     TIMESTAMPTZ NOT NULL,
  computed_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (problem_id, period_start, taxonomy_version, metric_def_version),
  CHECK (k <= n),
  CHECK ((n = 0 AND ci_low IS NULL AND ci_high IS NULL) OR (n > 0 AND ci_low IS NOT NULL AND ci_high IS NOT NULL)),
  CHECK (ci_low IS NULL OR (0 <= ci_low AND ci_low <= ci_high AND ci_high <= 1))
);

-- ===== 派生：作者问题集中度 =====
CREATE TABLE creator_problem_density (
  creator_id         TEXT        NOT NULL,
  problem_id         BIGINT      NOT NULL REFERENCES problem(id),
  window_days        INT         NOT NULL,
  taxonomy_version   INT         NOT NULL,
  metric_def_version INT         NOT NULL,
  k                  INT         NOT NULL CHECK (k >= 0),
  n                  INT         NOT NULL CHECK (n >= 0),
  ci_low             DOUBLE PRECISION,
  ci_high            DOUBLE PRECISION,
  sufficient         BOOLEAN     NOT NULL,
  computed_as_of     TIMESTAMPTZ NOT NULL,
  PRIMARY KEY (creator_id, problem_id, window_days, taxonomy_version, metric_def_version),
  CHECK (k <= n)
);

-- ===== 派生：作品回答判定 =====
CREATE TABLE work_problem_judgment (
  work_id          TEXT        NOT NULL,
  problem_id       BIGINT      NOT NULL REFERENCES problem(id),
  problem_def_ver  INT         NOT NULL,
  source           TEXT        NOT NULL CHECK (source IN ('ai','human')),
  content_hash     BYTEA       NOT NULL,
  prompt_version   TEXT        NOT NULL,   -- human 记录填 'human'
  model            TEXT        NOT NULL,   -- human 记录填 'human'
  label            TEXT        NOT NULL CHECK (label IN ('answers','mentions','none')),
  evidence_quote   TEXT,                   -- label <> 'none' 时必填
  reason           TEXT,
  reviewer         TEXT,                   -- human 记录必填
  judged_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (work_id, problem_id, problem_def_ver, source, content_hash, prompt_version, model),
  CHECK (label = 'none' OR evidence_quote IS NOT NULL),
  CHECK (source = 'ai' OR reviewer IS NOT NULL)
);
CREATE TABLE judgment_reject (
  id               BIGSERIAL   PRIMARY KEY,
  work_id          TEXT        NOT NULL,
  problem_id       BIGINT      NOT NULL,
  prompt_version   TEXT        NOT NULL,
  model            TEXT        NOT NULL,
  reason           TEXT        NOT NULL CHECK (reason IN ('invalid_json','invalid_label','missing_quote','quote_not_found','too_long')),
  raw_output_hash  BYTEA       NOT NULL,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE supply_search_run (
  id               BIGSERIAL   PRIMARY KEY,
  problem_id       BIGINT      NOT NULL REFERENCES problem(id),
  query            TEXT        NOT NULL,
  found            INT         NOT NULL CHECK (found >= 0),
  collected        INT         NOT NULL CHECK (collected >= 0 AND collected <= found),
  receipt_ref      TEXT        NOT NULL,   -- 采集侧回执引用
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ===== 人工标注与质量门（标注是事实，只追加）=====
CREATE TABLE label_task (
  id               BIGSERIAL   PRIMARY KEY,
  task_type        TEXT        NOT NULL CHECK (task_type IN ('work_problem_judgment')),
  work_id          TEXT        NOT NULL,
  problem_id       BIGINT      NOT NULL REFERENCES problem(id),
  stratum          TEXT        NOT NULL,   -- 抽样分层标识
  batch_id         TEXT        NOT NULL,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (task_type, work_id, problem_id, batch_id)
);
CREATE TABLE label (
  id               BIGSERIAL   PRIMARY KEY,
  task_id          BIGINT      NOT NULL REFERENCES label_task(id),
  annotator        TEXT        NOT NULL,
  value            TEXT        NOT NULL CHECK (value IN ('answers','mentions','none')),
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (task_id, annotator)
);
CREATE TABLE eval_run (
  id               BIGSERIAL   PRIMARY KEY,
  task_type        TEXT        NOT NULL,
  prompt_version   TEXT        NOT NULL,
  model            TEXT        NOT NULL,
  batch_id         TEXT        NOT NULL,
  n_items          INT         NOT NULL,
  human_kappa      DOUBLE PRECISION,
  per_class        JSONB       NOT NULL,   -- {"answers":{"precision":..,"recall":..,"support":..},...}
  metric_def_version INT       NOT NULL,
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE quality_gate (
  task_type        TEXT        NOT NULL,
  prompt_version   TEXT        NOT NULL,
  model            TEXT        NOT NULL,
  passed           BOOLEAN     NOT NULL,
  eval_run_id      BIGINT      NOT NULL REFERENCES eval_run(id),
  decided_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (task_type, prompt_version, model)
);

-- ===== 读模型 =====
CREATE TABLE opportunity_card (
  problem_id       BIGINT      PRIMARY KEY REFERENCES problem(id),
  payload          JSONB       NOT NULL,   -- 结构见 §6.7
  as_of            TIMESTAMPTZ NOT NULL,
  built_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ===== 研究集合 =====
CREATE TABLE research_collection (
  id               BIGSERIAL   PRIMARY KEY,
  owner            TEXT        NOT NULL,
  title            TEXT        NOT NULL,
  visibility       TEXT        NOT NULL CHECK (visibility IN ('private','team')),
  created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE collection_item (
  id               BIGSERIAL   PRIMARY KEY,
  collection_id    BIGINT      NOT NULL REFERENCES research_collection(id),
  item_type        TEXT        NOT NULL CHECK (item_type IN ('problem','work','comment','creator')),
  item_ref         TEXT        NOT NULL,
  note             TEXT,
  added_by         TEXT        NOT NULL,
  added_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
  removed_at       TIMESTAMPTZ,
  UNIQUE (collection_id, item_type, item_ref, added_at)
);
CREATE TABLE collection_snapshot (
  collection_id    BIGINT      NOT NULL REFERENCES research_collection(id),
  version          INT         NOT NULL,
  frozen_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
  frozen_by        TEXT        NOT NULL,
  items            JSONB       NOT NULL,   -- [{item_type,item_ref,content_hash,note}]，不含原文
  metrics          JSONB       NOT NULL,   -- 冻结时的指标数值
  versions         JSONB       NOT NULL,   -- 冻结时的各口径版本
  PRIMARY KEY (collection_id, version)
);
-- 触发器：collection_snapshot 禁止 UPDATE / DELETE（实现见 T3.2）
```

#### 6.1.3 SQLite 对照

作废。RECON 已确认数据库为 PostgreSQL 16。

#### 6.1.4 其他表中的 `work_id`、`creator_id`

- `work_id` 一律指 `content_ref`（即 `linggan_material_content.public_ref`，uuid）。
- `creator_id` 一律指作品作者的 `author_external_id`（text），由 `linggan_material_content_author_at(content_ref, as_of)` 取得。

### 6.2 指标定义

#### 6.2.1 有效评论（依据 D-007 至 D-012）

评论按稳定身份 `(content_ref, comment_external_id)` 计数，同一条评论被观察多次也只算一条。在给定领域、`as_of`、时区下，一条评论是“有效评论”，当且仅当按以下顺序**全部**满足。不满足时，记录第一条不满足的原因，每条评论只记一个原因：

| 顺序 | 条件 | 不满足时的排除原因 |
| --- | --- | --- |
| 1 | 所属作品在领域研究主集合中：`linggan_material_domain_usage` 中 `domain_ref` 匹配、`use_kind = 'domain_research'`、`role = 'primary'`，且 `created_at <= as_of`（迁移回填行不受此限，D-023） | `not_in_domain_primary` |
| 2 | 作品作者已知：`linggan_material_content_author_at(content_ref, as_of)` 返回非空的 `author_external_id` | `work_author_unknown` |
| 3 | 有周期锚点。`period_basis = work_published_cohort` 时：截至 `as_of` 的最近一次详情观察中，`linggan_material_content_detail.published_at` 非空。`period_basis = comment_published` 时：`linggan_material_comment_platform_time` 中有该评论的平台时间（D-028） | 前者为 `period_anchor_missing`，后者为 `comment_time_unknown` |
| 4 | `first_observed_at` 可以解析 | `observed_at_unparseable` |
| 5 | 未受限：不存在 `restricted_at <= as_of` 的 `linggan_material_comment_restriction` 记录 | `restricted` |
| 6 | 评论者不是作品作者本人（评论的 `author_external_id` 不等于作品作者的 `author_external_id`；评论者 ID 为空时视为非作者） | `creator_self` |

另外，`first_observed_at > as_of` 的评论在该时点还不存在，既不是有效评论，也不计入任何排除原因。

该评论所属周期 = `linggan_opportunity_period_start(锚点时间, period_tz)`（D-020）。锚点时间按 `period_basis` 取作品发布时间或评论平台发布时间，**同一次计算只用一种**。

切换前，`comment_published` 口径以影子模式同步计算并保存，不在页面展示（D-028）。两个函数的签名在全文统一为四参数，**以此处为准**：

- `linggan_opportunity_eligible_comments(domain_ref, as_of, tz, period_basis)`：返回 `content_ref`、`comment_external_id`、`commenter_external_id`（可空）、`work_author_external_id`、`period_anchor_at`、`period_start`、`first_observed_at`。`period_anchor_at` 是本次计算实际使用的锚点时间：按作品发布周时是作品发布时间，按评论发布周时是评论平台发布时间。
- `linggan_opportunity_eligibility_exclusions(domain_ref, as_of, tz, period_basis)`：返回 `(reason, comments)`。

以上逻辑只允许在一处实现。`tz` 与 `period_basis` 只能从 `metric_definition.params` 读取，不得写死。C3 完成之前，若传入 `period_basis = comment_published`，函数必须明确报错 `comment_published_basis_not_ready`，**不得**悄悄退回按作品发布周计算；该行为要有测试覆盖。

两个函数必须共用同一段 SQL（例如同一个内部函数或视图），保证“有效评论数 + 各原因排除数 = 全部候选评论数”。这条等式本身要有测试。

`commenter_external_id` 的取值规则：取该评论 first_admission 对应那次观察的值；没有 first_admission 时，取最早一次观察的值。同一评论多次观察中该值不一致时，计数并在运行记录中报告，不得静默取任意一次。

营销号排除只作用于分子（`comment_mention.excluded_reason = 'marketing'`）。v1 没有现成的营销号规则，暂不标记（T1.1 记入 DECISIONS.md）。

#### 6.2.2 本期提及数 `k`

`k` = 本期有效评论中，提及该问题（当前分类版本，`excluded_reason IS NULL`）的**去重评论者数**。

每位被计入的评论者至少有一条本期有效评论，所以 `k ⊆ n` 成立，Wilson 区间适用。评论者未知（`commenter_unknown`）的提及保留在 `n` 中，不计入 `k`。

#### 6.2.3 计算 SQL

```sql
-- 参数：:domain_ref, :problem_id, :tv, :period_start, :as_of, :tz, :period_basis, :digest_version
WITH e AS (
  SELECT content_ref, comment_external_id
  FROM linggan_opportunity_eligible_comments(:domain_ref, :as_of, :tz, :period_basis)
  WHERE period_start = :period_start
),
fm AS (
  SELECT DISTINCT m.commenter_hash
  FROM linggan_opportunity_comment_mention m
  JOIN e ON e.content_ref = m.content_ref
        AND e.comment_external_id = m.comment_external_id
  WHERE m.problem_id = :problem_id
    AND m.taxonomy_version = :tv
    AND m.excluded_reason IS NULL
    AND m.commenter_hash IS NOT NULL
    AND m.commenter_digest_version = :digest_version
)
SELECT (SELECT count(*) FROM fm) AS k,
       (SELECT count(*) FROM e)  AS n;
```

`:tz` 只能取自 `metric_definition.params.period_tz`，`:period_basis` 只能取自 `metric_definition.params.period_basis`，`:digest_version` 只能取自当前配置。不得写死。

#### 6.2.4 其余指标

| 指标 | 定义 |
| --- | --- |
| `rate_per_1k` | `k * 1000 / n`；`n = 0` 时为 `null` |
| `ci_low`、`ci_high` | `wilson_interval(k, n, z)`，存为 0–1 比例；`n = 0` 时为 `null` |
| `baseline_k`、`baseline_n` | 当前周期之前连续 `baseline_periods` 个周期的 `k`、`n` 分别求和（同一 `as_of`） |
| `mentioners_total` | 截至 `as_of`，该问题在所有周期有效评论上的未排除提及的去重 `commenter_hash` 数 |
| 作者 `k`、`n` | 窗口：`work_published_at` 落在 `[as_of − creator_window_days, as_of)`。`n` = 该作者作品上的有效评论数；`k` = 其中提及该问题的去重评论者数 |
| 作者 `sufficient` | `n >= creator_min_n` |
| 排除计数 | 每次计算都调用 `linggan_opportunity_eligibility_exclusions`，结果写入运行记录；按 §6.7 生成 flags |
| 代表原声 | 只从有效评论中选取；点赞按 D-019 处理 |

### 6.3 参数集

参数分两套（D-024）：

- **测试参数集**：即下面这段 JSON，永久固定。§6.4.4、§6.5、§8.2 的标准答案都在这套参数下计算。它只存在于测试代码中，不写入生产库。
- **生产参数 v1**：与测试参数集数值相同（D-029 取代了 D-024 第 2 款）。由 seed 迁移写入 `metric_definition`。采集不足时页面显示“样本不足”，不降低门槛。

`metric_definition` 是生产环境全部阈值的唯一来源（R3）。测试参数集：

```json
{
  "z": 1.959963984540054,
  "period": "week",
  "period_basis": "work_published_cohort",
  "period_tz": "Asia/Shanghai",
  "week_start": "monday",
  "baseline_periods": 4,
  "series_periods": 8,
  "min_k_significant": 3,
  "min_n_period": 500,
  "min_total_for_significant": 5,
  "creator_window_days": 90,
  "creator_min_n": 50,
  "supply_min_coverage": 0.6,
  "supply_max_answers_for_gap": 2,
  "supply_engagement_ref": "domain_median_likes_90d",
  "supply_max_unjudged_ratio": 0.2,
  "gate_min_labeled": 250,
  "gate_min_kappa": 0.70,
  "gate_min_precision": 0.85,
  "gate_min_recall": 0.80,
  "gate_classes": ["answers", "mentions"],
  "collection_volume_shift_alert": 0.30,
  "max_unanchored_share": 0.20,
  "max_author_unknown_share": 0.20,
  "collection_target_per_period": 700,
  "comment_time_switch_coverage": 0.90,
  "card_rebuild_debounce_seconds": 300
}
```

修改任何参数都必须新增一个版本，再切换 `metric_definition_current`。**不得原地修改已有版本。**

加载器的额外校验：`period_basis` 只接受 `work_published_cohort` 或 `comment_published`；`week_start` 只接受 `monday`（D-008、D-020、D-028）。生产 v1 使用 `work_published_cohort`，切换到 `comment_published` 必须新增口径版本（D-028）。

### 6.4 统计函数与标准答案

#### 6.4.1 参考实现（Rust）

统计函数放在独立的纯函数模块里，不访问数据库。以下实现可以直接使用。如果改写，必须通过 §6.4.2 的全部测试：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsError {
    EmptySample,
    CountExceedsTotal { k: u64, n: u64 },
    InvalidZ,
}

/// Wilson 得分区间。n = 0 返回 EmptySample，调用方必须把它映射为 null（R5）。
pub fn wilson_interval(k: u64, n: u64, z: f64) -> Result<Interval, StatsError> {
    if !(z.is_finite() && z > 0.0) {
        return Err(StatsError::InvalidZ);
    }
    if n == 0 {
        return Err(StatsError::EmptySample);
    }
    if k > n {
        return Err(StatsError::CountExceedsTotal { k, n });
    }
    let n_f = n as f64;
    let p = k as f64 / n_f;
    let z2 = z * z;
    let denom = 1.0 + z2 / n_f;
    let center = p + z2 / (2.0 * n_f);
    let margin = z * (p * (1.0 - p) / n_f + z2 / (4.0 * n_f * n_f)).sqrt();
    let low = ((center - margin) / denom).clamp(0.0, 1.0);
    let high = ((center + margin) / denom).clamp(0.0, 1.0);
    Ok(Interval { low, high })
}

pub fn rate_per_1k(k: u64, n: u64) -> Option<f64> {
    if n == 0 {
        None
    } else {
        Some(k as f64 * 1000.0 / n as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceTier {
    Candidate,
    Established,
    Significant,
}

pub struct TierInput {
    pub mentioners_total: u64,
    pub k_cur: u64,
    pub n_cur: u64,
    pub k_base: u64,
    pub n_base: u64,
}

pub struct TierParams {
    pub z: f64,
    pub min_k_significant: u64,
    pub min_n_period: u64,
    pub min_total_for_significant: u64,
}

pub fn evidence_tier(i: &TierInput, p: &TierParams) -> EvidenceTier {
    if i.mentioners_total < 2 {
        return EvidenceTier::Candidate;
    }
    if i.mentioners_total < p.min_total_for_significant
        || i.k_cur < p.min_k_significant
        || i.n_cur < p.min_n_period
        || i.n_base < p.min_n_period
    {
        return EvidenceTier::Established;
    }
    let baseline = i.k_base as f64 / i.n_base as f64; // n_base >= min_n_period > 0
    match wilson_interval(i.k_cur, i.n_cur, p.z) {
        Ok(ci) if ci.low > baseline => EvidenceTier::Significant,
        _ => EvidenceTier::Established,
    }
}
```

`min_n_period` 必须大于 0（seed 中为 500）。加载参数时校验，不满足就拒绝加载。

#### 6.4.2 Wilson 区间标准答案（z = 1.959963984540054）

测试容差为 `1e-6`（比例值）。**这些数值不得修改。**

| k | n | 点估计 | ci_low | ci_high |
| --- | --- | --- | --- | --- |
| 7 | 120 | 0.058333 | 0.028541 | 0.115526 |
| 3 | 40 | 0.075000 | 0.025836 | 0.198642 |
| 5 | 96 | 0.052083 | 0.022450 | 0.116185 |
| 6 | 1850 | 0.003243 | 0.001487 | 0.007058 |
| 0 | 10 | 0.000000 | 0.000000 | 0.277533 |
| 10 | 10 | 1.000000 | 0.722467 | 1.000000 |
| 1 | 1 | 1.000000 | 0.206549 | 1.000000 |
| 50 | 100 | 0.500000 | 0.403832 | 0.596168 |
| 10 | 1000 | 0.010000 | 0.005441 | 0.018309 |
| 11 | 1003 | 0.010967 | 0.006135 | 0.019531 |
| 3 | 1000 | 0.003000 | 0.001021 | 0.008783 |
| 4 | 60 | 0.066667 | 0.026229 | 0.159254 |

错误情况：
- `(k=0, n=0)` → `EmptySample`
- `(k=5, n=3)` → `CountExceedsTotal`
- `z = 0`、`z = NaN` → `InvalidZ`

性质测试（随机生成 `0 <= k <= n <= 100000`）：

- `0 <= low <= p <= high <= 1`，其中 `p = k / n`；
- `n` 固定时，`k` 增大，`low` 不减小；
- `k / n` 固定时，`n` 增大，区间宽度不增大。

#### 6.4.3 周期边界标准答案（`period_tz = Asia/Shanghai`，周一开始）

| `published_at`（UTC） | 所属 `period_start` |
| --- | --- |
| 2026-11-01T15:59:59Z | 2026-10-26 |
| 2026-11-01T16:00:00Z | 2026-11-02 |
| 2026-11-08T15:59:59Z | 2026-11-02 |
| 2026-11-08T16:00:00Z | 2026-11-09 |

周期函数按 D-020 在 PostgreSQL 中实现（`linggan_opportunity_period_start`、`linggan_opportunity_period_bounds`），在隔离数据库中用以上四条用例验证。Rust 侧不做时区换算，也不新增日期库。

#### 6.4.4 证据等级标准答案（使用测试参数集）

| 用例 | mentioners_total | k_cur | n_cur | k_base | n_base | 期望 |
| --- | --- | --- | --- | --- | --- | --- |
| A | 14 | 10 | 1000 | 4 | 4800 | `significant` |
| B | 3 | 3 | 1000 | 0 | 4800 | `established`（总人数小于 5） |
| C | 1 | 1 | 1000 | 0 | 4800 | `candidate` |
| D | 20 | 2 | 1000 | 4 | 4800 | `established`（k 小于 3） |
| E | 20 | 10 | 400 | 4 | 4800 | `established`（n 小于 500） |
| F | 20 | 10 | 1000 | 4 | 300 | `established`（基线 n 小于 500） |
| G | 20 | 3 | 1000 | 30 | 4800 | `established`（ci_low 0.001021 小于基线 0.00625） |

### 6.5 作者排序

1. 过滤：`sufficient = true` 的进入 `ranked`，其余进入 `insufficient`。
2. `ranked` 排序：`ci_low` 降序，再按 `n` 降序，再按 `creator_id` 升序。
3. `insufficient` 排序：`n` 降序，再按 `creator_id` 升序。`insufficient` 不显示名次。

标准答案（测试参数集，`creator_min_n = 50`）：

| 作者 | k | n | 期望归属与顺序 |
| --- | --- | --- | --- |
| A | 7 | 120 | ranked 第 1（ci_low 0.028541） |
| B | 5 | 96 | ranked 第 2（ci_low 0.022450） |
| C | 6 | 1850 | ranked 第 3（ci_low 0.001487） |
| D | 3 | 40 | insufficient |

### 6.6 供给判定

#### 6.6.1 候选作品召回

对 `evidence_tier` 不低于 `established` 的问题，候选作品取以下来源的并集：

1. **评论来源**：当前分类版本下，有该问题提及的作品。
2. **搜索来源**：该问题最近一次 `supply_search_run` 中已采集的作品。
3. **向量来源**：问题表述与作品理解摘要的相似度前 20 篇。**仅当 RECON 确认已有嵌入与检索能力时启用**；否则关闭该来源，并在 DECISIONS.md 记录，不要自建。

候选中缺少整篇理解结果的作品，提交到现有的整篇理解任务，等结果回来后再判定。

#### 6.6.2 大模型输入与输出契约

**输入**：

- 问题表述（`problem.statement`）及其 `def_version`；
- 作品文本：优先使用整篇材料快照 `linggan_note_material_fragment` 的 `title`、`body`、`ocr`、`transcript` 四个字段，各段之间用换行拼接；T2.1 确认快照满足 D-026 的三个条件后才采用，否则退回截至 `as_of` 的 `linggan_material_content_detail.title` 与 `body_text`（D-017、D-026）。
- 辅助上下文（可选）：`scope = 'note'` 的整篇理解结果。引文校验只针对原始文本，不针对理解结果。

**标签定义**（必须原样写入提示词）：

- `answers`：作品的主要内容或某个明确段落，给出了针对该问题的解决方法或解释。
- `mentions`：作品提到了该问题，但没有给出可操作的回答。
- `none`：作品没有涉及该问题。

**输出**：严格 JSON，不允许任何其他文本：

```json
{"label": "answers", "evidence_quote": "作品原文中的一段，不超过 120 字", "reason": "不超过 80 字"}
```

**提示词管理**：通过现有的提示词或 Skill 版本机制注册（`linggan_ai_prompt_version` 与 `linggan_ai_task_config.prompt_version_ref`，见 RECON §4.2）。如果模型参数可配置，温度设为 0。

#### 6.6.3 输出校验（R15）

按顺序校验，任一项失败就写一条 `judgment_reject`，**不写判定表**：

1. 能解析为 JSON 对象 → 否则 `invalid_json`
2. `label` 属于三个枚举值之一 → 否则 `invalid_label`
3. `label != "none"` 时 `evidence_quote` 非空 → 否则 `missing_quote`
4. `evidence_quote` 不超过 120 字，`reason` 不超过 80 字 → 否则 `too_long`
5. 对输入全文和 `evidence_quote` 分别做 NFKC 归一化并把连续空白合并为一个空格后，`evidence_quote` 是输入全文的子串 → 否则 `quote_not_found`

失败后用相同输入重试 1 次。第二次仍失败，该（作品，问题）对保持“未判定”，计入 `unjudged`。

#### 6.6.4 缓存键

```
cache_key = sha256(content_hash || problem_id || problem_def_ver || prompt_version || model)
```

判定表主键已包含这些字段。提交任务前先查是否已存在，存在就跳过，并用唯一约束兜底（R13）。

#### 6.6.5 有效标签与判定状态

- **有效标签**：同一（作品，问题，`problem_def_ver`）下，有 `human` 记录就取最新的 `human`；否则取当前提示词版本和模型下的 `ai` 记录。
- 人工记录的 `prompt_version`、`model` 都填 `'human'`，`content_hash` 填被复核时作品内容的哈希。
- **`judgment_status`**：参与计算的有效标签全部来自 `human`，或者全部来自已通过质量门的（提示词版本，模型）时，为 `verified`；否则为 `ai_suggested`。

#### 6.6.6 供给状态机

按顺序判断，命中即停：

1. `evidence_tier = candidate`，或没有任何候选作品 → `not_evaluated`
2. 未判定数 ÷ 候选数 > `supply_max_unjudged_ratio` → `awaiting_understanding`
3. 没有 `supply_search_run`，或 `coverage < supply_min_coverage` → `insufficient_coverage`
4. `answers` 数量 ≤ `supply_max_answers_for_gap`，并且（`answers = 0`，或所有 `answers` 作品的最高点赞 < 领域近 90 天作品点赞中位数）→ `suspected_gap`
5. 其他情况 → `covered`

供给状态中**没有“确认缺口”**。即使质量门已通过，也只是 `suspected_gap` 加上 `judgment_status = verified`。原因：缺口和“做了就有效”之间没有已验证的因果关系，必须等第 4 阶段回测验证。

### 6.7 机会卡接口响应结构

`GET /api/local/opportunity/problems/{id}/card?asOf=` 的响应。前端只能使用这些字段（R4）。

**线上字段一律为 camelCase**（D-004）。下面的类型为了与表结构对照，保留了 snake_case 写法；实现时按 D-004 的规则换算，例如 `as_of` → `asOf`、`rate_per_1k` → `ratePer1k`。枚举值保持 snake_case。

```ts
type QualityFlag =
  | "insufficient_sample"
  | "not_calibrated"
  | "no_baseline"
  | "collection_volume_shift"
  | "stale_read_model"
  | "period_anchor_missing"          // D-008
  | "author_unknown_share_high"      // D-010
  | "engagement_reference_missing"   // D-019
  | "collection_below_target"        // D-029
  | "collection_protocol_changed";   // D-030

interface Interval1k { low: number; high: number }  // 已换算为每千

interface OpportunityCard {
  problem: {
    id: string;
    statement: string;
    level: 1 | 2 | 3;
    parent_id: string | null;
    redirected_from: string | null;   // 请求的是已合并的旧 ID 时填写
  };
  as_of: string;                       // ISO-8601 UTC
  versions: { taxonomy: number; metric_def: number; judgment_prompt: string | null; model: string | null };
  demand: {
    mentioners_total: number;
    evidence_tier: "candidate" | "established" | "significant";
    current: { period_start: string; k: number; n: number; rate_per_1k: number | null; ci_per_1k: Interval1k | null };
    baseline: { k: number; n: number; rate_per_1k: number | null };
    series: Array<{ period_start: string; k: number; n: number; rate_per_1k: number | null }>; // 长度 = series_periods
    sample_voices: Array<VoiceRef>;   // 最多 3 条，按点赞降序，排除墓碑后补位
    user_methods: Array<{ label: string; mentioners: number }>;  // 来自现有“办法与经历”，没有就为空数组
    importance: { status: "not_calibrated" } | { status: "calibrated"; survey_id: string; importance: number; satisfaction: number; opportunity_score: number };
  };
  supply: {
    state: "not_evaluated" | "awaiting_understanding" | "insufficient_coverage" | "covered" | "suspected_gap";
    judgment_status: "verified" | "ai_suggested" | null;
    coverage: { found: number; collected: number; ratio: number } | null;
    counts: { answers: number; mentions: number; none: number; unjudged: number };
    works: Array<{ work_id: string; title: string | null; creator_id: string; likes: number | null; label: "answers" | "mentions" | "none"; source: "human" | "ai"; tombstone: boolean }>;
  };
  channel: {
    window_days: number;
    min_n: number;
    ranked: CreatorDensity[];
    insufficient: CreatorDensity[];
  };
  flags: QualityFlag[];
  exclusions: Array<{ reason: string; comments: number }>;   // D-012，来自资格排除函数
  limitations: string[];   // 固定说明，见 §6.10“依据与局限”
}

interface VoiceRef {
  comment_ref: { content_ref: string; comment_external_id: string };  // D-011
  text: string | null;      // 墓碑时为 null
  likes: number | null;
  work_id: string;
  tombstone: boolean;
}

interface CreatorDensity {
  creator_id: string;
  name: string;
  followers: number | null;
  k: number;
  n: number;
  rate_per_1k: number;
  ci_per_1k: Interval1k;
  observed: boolean;        // 是否已在观察名单
}
```

- 响应中不得出现评论者 ID、昵称或哈希（R9）。
- `flags` 规则：
  - `not_calibrated`：没有导入问卷时**必须**出现；
  - `no_baseline`：`baseline_n < min_n_period` 时出现；
  - `collection_volume_shift`：本周期与上周期的领域有效评论数变化超过 `collection_volume_shift_alert` 时出现；
  - `stale_read_model`：读模型的 `as_of` 早于最新事实的 `first_observed_at` 超过 24 小时时出现；
  - `period_anchor_missing`：因 `period_anchor_missing` 被排除的评论占候选评论的比例超过 `max_unanchored_share` 时出现；
  - `author_unknown_share_high`：因 `work_author_unknown` 被排除的比例超过 `max_author_unknown_share` 时出现；
  - `engagement_reference_missing`：领域点赞中位数没有可用的 `KNOWN` 值时出现（D-019）；
  - `collection_below_target`：当周领域有效评论数低于 `collection_target_per_period` 时出现（D-029）；
  - `collection_protocol_changed`：统计窗口内评论采集协议发生变化时出现，趋势图需标记变化点（D-030）。

### 6.8 质量门

对一个（任务类型，提示词版本，模型）组合：

1. 从候选（作品，问题）对中分层随机抽样，分层维度为问题和 AI 标签，生成一个 `batch_id` 批次。
2. 至少两位标注员独立标注全部样本。计算 Cohen's kappa。kappa < `gate_min_kappa` 时，**先修改标签定义**，不得继续评估模型。
3. 以两人一致的样本为金标准（不一致的由第三人裁定），计算 AI 在 `gate_classes` 每个类别上的精确率和召回率。
4. 同时满足以下条件才通过：金标准样本数 ≥ `gate_min_labeled`，且每个类别精确率 ≥ `gate_min_precision`、召回率 ≥ `gate_min_recall`。
5. 提示词版本或模型一旦变化，就是一个新组合，默认未通过，需要重新评估。

### 6.9 研究集合

- **草稿**：`collection_item` 中 `removed_at IS NULL` 的条目。
- **冻结**：生成 `collection_snapshot` 的下一个版本。快照里保存：
  - 条目引用和内容哈希；
  - 条目备注；
  - 当时机会卡的关键指标：`k`、`n`、区间、等级、供给状态、判定状态；
  - 全部口径版本。

  **快照不保存原文。**
- **展示快照时**：按引用实时读取原文，同时检查来源资格。已撤回的条目显示为墓碑（保留类型、采集时间、撤回时间），不显示正文。原文的当前哈希与快照哈希不一致时，标注“内容已变化”。
- **导出**：只能导出快照版本。普通评论者统一显示为“读者”。导出内容同样遵守 R10。

### 6.10 界面文案（固定）

| 场景 | 文案 |
| --- | --- |
| 指标名 | 提及人数；提及率（每千条评论，按作品发布周） |
| 证据等级 | 候选；成立；显著 |
| 供给状态 | 暂不判断；等待作品理解；供给侧覆盖不足；已有好内容；疑似供给缺口 |
| 判定状态 | AI 建议；已复核 |
| 样本不足 | 样本不足，暂不排序 |
| 重要性 | 重要性未校准 |
| 墓碑 | 来源已撤回：原文不再展示，保留引用记录 |
| 固定说明（首页表格下方） | 提及人数反映讨论热度，不等于问题对用户的重要性；评论只代表愿意留言的人，且每篇作品只采集平台排序靠前的部分评论。 |
| 依据与局限（机会卡，固定显示） | 按当前口径显示其一。作品发布周口径：“提及率按作品发布周统计，表示该周发布的作品下的评论中，提及该问题的比例；近期作品积累评论的时间较短，当周数字偏少。”评论发布周口径：“提及率按评论发布周统计。”两种口径都要附上当前的采集协议说明（排序方式与单次上限）。 |

---

## 7. 阶段与任务卡

**依赖关系**：阶段 0 → 阶段 1 → 阶段 2；阶段 3 依赖阶段 1；阶段 4 依赖阶段 0，可以与阶段 3 并行。**阶段 C 在 T0.1b 之后即可开始，与阶段 0、阶段 1 并行**；C3 需要 T0.2 的资格函数做切换准备。每个阶段结束时有质量门（G0–G4），由负责人确认通过后才能进入下一阶段。

### 阶段 0：口径地基

#### T0.1 代码库勘察

- **前置**：无
- **目标**：产出 `docs/opportunity/RECON.md`，按附录 A 模板填写。
- **要求**：
  - 每一项都给出 `文件路径:行号`，或者给出实际执行的命令及其输出。
  - 查不到的项写“未找到”，并说明查找方法，**不得填写推测值**。
  - 运行一次现有测试，记录命令和结果。
- **验收**：附录 A 中所有必填项都已填写；负责人抽查 5 项，全部可以复现。
- **禁止**：本任务不修改任何代码。

#### T0.1b 基线同步与补充勘察

- **前置**：T0.1，以及负责人对 Q-01 至 Q-10 的裁定（DECISIONS.md）
- **目标**：按 D-001 同步基线，补齐以下勘察，结果写入 RECON.md 新增的“§7 补充勘察”。
  1. 同步与差异：新基线提交号、迁移链头，以及 `ad19a4d7..HEAD` 中影响 RECON 的变化（D-001）；在新基线上重新记录既有失败（D-002）。
  2. first_admission 覆盖率：有多少稳定评论身份没有 `linggan_material_comment_first_admission` 记录（D-007）。
  3. 评论时间来源：采集侧原始 package 与 `crates/contracts` 的 fixture 中，评论对象是否带有平台评论时间；如果有，给出字段名和示例位置（D-008）。
  4. `linggan_material_domain_usage` 是否有可用于 `as_of` 判断的时间列（D-009）。
  5. 当前代码是否仍写入 0064 的 problem 相关表（D-014）。
  6. 0177 是否存在问题合并、取代或撤销的语义（D-014）。
  7. 每篇作品已采评论数的分布（P50、P90、最大值），以及多次采集是否会补到新评论（D-016）。
  8. 图片识别文字和口播转写是否有存储位置（D-017）。
  9. HMAC 函数的可见性、`model_secrets` 读取函数能否复用（D-013）；前端读取后端只读状态的现有写法（D-006）。
- **要求**：只读。真实数据库只允许 `SELECT`，统计结果只写聚合数字，不写评论内容。
- **验收**：每一项都有 `文件路径:行号` 或命令输出；出现新的阻塞就写入 QUESTIONS.md。
- **禁止**：修改代码；为了勘察而写入数据库。

#### T0.2 评论事实视图与资格函数

- **前置**：T0.1b、T0.3
- **目标**：按 §6.2.1 和 D-007、D-012 建立：
  - 视图 `linggan_opportunity_comment_fact`：每个稳定评论身份一行，包含 `content_ref`、`comment_external_id`、`commenter_external_id`、`first_observed_at`、`first_observed_backfilled`、解析是否成功；
  - 函数 `linggan_opportunity_eligible_comments(domain_ref, as_of, tz, period_basis)` 和 `linggan_opportunity_eligibility_exclusions(domain_ref, as_of, tz, period_basis)`，两者共用同一段实现。T0.2 只需实现 `period_basis = work_published_cohort`；传入 `comment_published` 时报错 `comment_published_basis_not_ready`，不得回退。
- **要求**：
  - 不 `ALTER` 任何现有表（D-005）。
  - 所有 `observed_at::timestamptz` 转换只在这里做（D-018）。
  - `commenter_external_id` 的取值按 §6.2.1 的规则；同一评论多次观察中该值不一致的，计数并报告。
- **验收**：
  - 在 §8.2 数据集上，有效评论数和各排除原因的计数与 §8.2.3 一致；
  - “有效 + 各原因排除 = 全部候选”的等式有测试；
  - 在真实库上用只读查询统计 `first_observed_at` 三种来源（first_admission、最早观察回填、无法解析）各有多少条，写入 PROGRESS.md；
  - 按 D-024 第 4 条出分布报告；按 D-025 报告回复所占比例（或记录“无法区分”）。
- **禁止**：在视图和函数之外，另写一份时间解析或资格判断。

#### T0.3 口径定义与周期工具

- **前置**：T0.1b
- **目标**：建 `metric_definition` 相关表，由 seed 迁移写入 §6.3 的**生产参数 v1**；测试参数集放在测试代码中；实现参数加载和周期切分。
- **要求**：
  - 参数加载（Rust）时逐项校验类型和取值范围（例如 `z > 0`、`min_n_period > 0`、比例在 0–1 之间、`period_basis` 与 `week_start` 只接受 v1 允许的值），不合法就拒绝启动相关任务。
  - 周期函数按 D-020 写成 PostgreSQL 函数：给定 UTC 时间和时区，返回 `period_start`；给定 `period_start` 和时区，返回 UTC 起止区间。
  - 新建 `scripts/test-opportunity-postgres.sh`，仿照现有 `scripts/test-*-postgres.sh` 在隔离数据库中运行。
- **验收**：§6.4.3 的全部用例通过；参数非法时有测试证明会拒绝。
- **禁止**：在业务代码中出现时区字符串或星期常量。

#### T0.4 统计函数模块

- **前置**：T0.1
- **目标**：实现 §6.4.1 的函数。
- **验收**：§6.4.2 和 §6.4.4 的标准答案全部通过；三条性质测试通过。如果 RECON 显示项目已有性质测试库就使用它，否则用固定种子生成 10,000 组随机输入。
- **禁止**：修改标准答案；改变函数签名语义（例如把错误变成默认值）。

#### T0.5 问题登记与谱系

- **前置**：T0.1b、T0.3
- **目标**：建 `taxonomy_version`、`problem`、`problem_lineage` 三张表；把 0177 中已有的问题登记进来（`source_ref = problem_ref`，`source_definition_ref = definition_ref`，`level = 1`，D-014）；实现 `resolve_problem_id`。
- **要求**：
  - `resolve_problem_id(id)` 沿 `merge` 和 `rename` 链找到当前活动问题，返回结果和完整路径。
  - 检测到环时返回错误，不能死循环。
  - `split` 不自动重定向，返回所有子问题，由调用方处理。
  - 每次合并或拆分都递增该领域的 `taxonomy_version`。
  - 同步任务可以重复运行：同一 `problem_ref` 的 `definition_ref` 变化时，`def_version` 加 1；没变化时不写入（R13）。
  - 0177 有合并语义时，按 DECISIONS.md 中记录的映射规则写入谱系；没有时，谱系表只建不写（D-014）。
- **验收**：测试覆盖直达、两跳合并、改名后合并、拆分、成环五种情况；现有问题全部登记，数量与评论研究一致（写入 PROGRESS.md）。
- **若遇到**：已有问题的数据结构无法映射 → §3.3 第 3 条，停止并询问。

#### T0.6 评论者哈希

- **前置**：T0.1b
- **目标**：实现 `commenter_hash = HMAC-SHA256(secret, author_external_id)`，按 D-013。
- **要求**：
  - 密钥文件：`<support_dir>/opportunity/commenter-digest.key`，至少 32 字节。`<support_dir>` 由从 `model_secrets.rs` 提取出的同一个 `pub` 解析函数得出（D-022），不得另写路径解析或缺省值。密钥不得写入代码和日志。
  - 密钥缺失或过短时，机会卡的写入和计算任务拒绝启动，不得退回空密钥。
  - 复用 `crates/evidence/src/collection_control.rs` 中现有的 HMAC-SHA256 实现；如需改为 `pub`，不得改变其行为，现有 RFC 4231 测试必须继续通过。不得复制一份加密代码。
  - 输出十六进制字符串，同时记录 `commenter_digest_version = "commenter-hmac-sha256-v1"`。
  - `author_external_id` 为 NULL 时返回“未知”，由调用方写 `commenter_hash = NULL`、`excluded_reason = 'commenter_unknown'`。
- **验收**：同一 ID 得到相同哈希；不同密钥得到不同哈希；日志中不出现原始 ID（测试捕获日志断言）。
- **禁止**：使用无密钥的普通哈希。

#### T0.7 测试数据集与不变量检查

- **前置**：T0.1b 至 T0.6
- **目标**：
  - 实现 §8.2 的确定性测试数据集构建器；
  - 实现 §8.3 的不变量检查命令（例如 `opportunity check-invariants`），输出每条检查的名称和违规行数。
- **验收**：在测试数据集上，不变量检查全部为 0 违规；构建器连续运行两次，结果完全相同。

#### G0 阶段 0 质量门

- 同一份数据、同一 `as_of` 连续计算两次，所有派生结果逐字节一致。
- 不变量检查全部为 0。
- RECON.md 经负责人确认。

### 阶段 C：评论时间与采集协议（D-028、D-030）

本阶段改动采集插件和采集侧数据，范围比机会卡模块更广。除 §2、§3 的全部规则外，还要遵守：现有采集控制（频率、风险冷却、授权）一律不得绕过；包合同的任何变更都要同步更新 `crates/contracts` 的校验和 fixture。

#### C1 采集侧勘察

- **前置**：T0.1b
- **目标**：只读勘察，结果写入 RECON.md 新增的“§8 采集侧”。
  1. 评论请求的排序方式：插件现在请求的是哪种排序；平台是否支持按时间排序；依据写到 `capture-capability-registry.md` 的哪一行，或者代码位置。**没有依据就写“未验证”，不得推断。**
  2. 单次评论条数上限 30 的来源（代码位置），以及是否可以配置。
  3. 包或记录上现有的版本字段（插件版本、合同版本、协议字段），能否用来区分改造前后的包。
  4. 包合同中评论记录 `publishedAt` 的类型与校验规则：是否允许 `null`；改为允许 `null` 需要改哪些校验和 fixture。
  5. `0048_comment_intelligence.sql:40-42` 中 `linggan_ci_source` 从 payload 取时间的逻辑，能否复用为投影器的提取逻辑。
  6. 重新采集的现有入口和控制（频率、冷却、授权）在哪里，重新采集一篇作品的评论需要走哪些步骤。
- **验收**：每项有出处；需要负责人决定的写入 QUESTIONS.md（至少包括：机会卡统计使用哪种排序协议）。
- **禁止**：修改代码；为了测试去访问平台。

#### C2 插件写入评论时间并去掉回退

- **前置**：C1，以及负责人对排序协议的决定
- **目标**：
  - 插件把平台评论时间以毫秒 epoch 写入 `publishedAt`；取不到时写 `null`。
  - 删除 `recordNormalization.js:162` 中对 `record.createdAt` 的回退。
  - 包上带有能区分改造前后的版本标识（按 C1 结论），并记录采集协议（排序方式、单次上限、触发来源）。
  - 同步更新包合同、Rust 侧校验和 fixture。
- **验收**：
  - 插件单元测试覆盖：带 `create_time` 的样例、不带时间的样例（写 `null`，不是 0，也不是采集时间）、时间单位为秒和毫秒的样例（按 C1 确认的平台实际单位处理）；
  - 合同测试通过；旧版本的包仍能被正确接收（向后兼容）；
  - 用 fixture 证明 `createdAt` 存在时也不会写入 `publishedAt`。
- **禁止**：编造平台字段名，只用 C1 中有出处的字段；用展示文本推算 epoch。

#### C3 平台时间投影与校验

- **前置**：C2、T0.2
- **目标**：
  - 新建只追加的表 `linggan_material_comment_platform_time(content_public_ref, comment_external_id, published_at, precision, source_package_ref, protocol_version, created_at)`，以评论身份为主键；
  - 投影器从包中提取，同一身份以第一条有效值为准，多个包给出的值不一致时计数；
  - 资格函数支持 `period_basis = comment_published`；影子模式下同步计算并保存这一口径的指标。
- **验收**：
  - `published_at > first_observed_at` 的违例数为 0；
  - 与绝对日期型展示文本的同日一致率 ≥ 99%，不一致的逐条列出（展示文本只用于核对，不入库为时间）；
  - 每个领域最近 12 周有效评论中有平台时间的比例，每周报告一次。
- **禁止**：`ALTER` 现有评论表；用展示文本补时间。

#### C4 重新采集补齐历史

- **前置**：C2
- **目标**：对仍可访问的作品，在现有采集控制允许的范围内重新采集评论，补齐统计窗口内的平台时间。
- **要求**：
  - 先补最近 12 周内发布的作品，按领域轮流进行；
  - 每一批都记录：作品数、新获得平台时间的评论数、失败与冷却次数；
  - 重新采集使用 C2 之后的协议。
- **验收**：每周报告各领域的覆盖率进展；达到 D-028 的切换条件时提交切换申请，由负责人批准后新增口径版本。
- **禁止**：绕过频率限制、风险冷却或授权；为了加快进度提高并发。

#### GC 阶段 C 质量门

- C3 的三项验收全部达标；
- 至少一个领域满足切换条件，影子口径与过渡口径的对比报告提交给负责人；
- 负责人批准后，该领域切换到 `comment_published`。

### 阶段 1：机会卡只读版

#### T1.1 评论提及数据

- **前置**：G0
- **目标**：建 `comment_mention` 表。先把 0177 中已有的“评论 → 问题”归属迁移进来（`assigned_by = 'migrated'`），再实现新评论的增量归属。
- **关联链**（D-014）：`linggan_comment_problem_membership.request_ref` → `linggan_comment_research_request.source_ref`（= `linggan_material_comment.material_ref`）→ 该观察的 `(content_public_ref, comment_external_id)`。同一稳定评论通过多次观察关联到同一问题时，只写一行。
- **要求**：
  - 迁移阶段只搬运已有结果，不重新调用 AI。
  - 增量归属：如果评论研究已有归属流程，就**挂接它的输出**，而不是另写一套；没有的话停止并询问（§3.3 第 3 条）。
  - `creator_self` 由资格函数判定，不在本表标记。本表只标记 `commenter_unknown`（D-013）；营销号 v1 不标，在 DECISIONS.md 记录。
  - 0177 判定为“已发布”的问题，其 `mentioners_total` 应当不小于 2。不满足的逐条列出，并分析原因（例如作品不在研究主集合、作者未知）。
- **验收**：
  - 迁移后每个问题的去重提及人数与评论研究中的人数一致；不一致的逐条列出原因；
  - 增量流程重复运行不产生重复行。
- **禁止**：在本任务中新建 AI 归属算法。

#### T1.2 问题指标计算

- **前置**：T1.1
- **目标**：按 §6.2 计算 `problem_metric_period`，覆盖最近 `series_periods + baseline_periods` 个周期。
- **要求**：
  - 只重算有新数据的问题（增量）；提供 `--full` 全量重算入口。
  - `n = 0` 时 `ci_low`、`ci_high` 为 NULL（R5）。
  - 等级用 T0.4 的 `evidence_tier`。
- **验收**：§8.2 测试数据集上的 P1、P2、P3 结果与 §8.2 期望值一致；不变量检查为 0。

#### T1.3 作者集中度计算

- **前置**：T1.1
- **目标**：按 §6.2.4 计算 `creator_problem_density`。
- **验收**：测试数据集上 C1、C2 的结果与 §8.2 一致；排序逻辑通过 §6.5 的标准答案测试。

#### T1.4 机会卡读模型

- **前置**：T1.2、T1.3
- **目标**：构建 `opportunity_card.payload`，结构严格符合 §6.7。
- **要求**：
  - 上游变化时发出“问题已变更”事件；在 `card_rebuild_debounce_seconds` 窗口内合并后再重建。
  - 提供全量重建命令。
  - 阶段 1 中供给栏固定为 `state = "not_evaluated"`、`judgment_status = null`，**不得填入假数据**。
  - `sample_voices` 只从可研究来源中选取，撤回的不出现（R10）。
- **验收**：
  - 测试数据集上 P1 的 payload 与 §8.2.4 的快照一致；
  - 撤回一条代表原声并重建后，该原声不再出现，并由下一条补位。

#### T1.5 只读接口

- **前置**：T1.4
- **目标**：实现以下接口：
  - `GET /api/local/opportunity/status`（D-006，开关关闭时也返回 200）
  - `GET /api/local/opportunity/domains/{domainRef}/opportunities?period=&tier=&supplyState=&cursor=&limit=`
  - `GET /api/local/opportunity/problems/{id}/card?asOf=`
  - `GET /api/local/opportunity/problems/{id}/evidence?kind=comments|works|creators&cursor=&limit=`
- **要求**：
  - 路由风格、错误格式、分页方式沿用现有约定（RECON 记录）。
  - 请求已合并的旧 ID 时，返回当前问题，并填写 `redirected_from`。
  - 不传 `as_of` 时读读模型；传了就按时点实时计算（阶段 4 之前可以返回“暂不支持”的明确错误码）。
- **验收**：接口快照测试；开关关闭时，除 `status` 外的路由返回 404 和 `code = "opportunity_card_disabled"`（D-006）。

#### T1.6 首页（机会列表）

- **前置**：T1.5
- **目标**：实现机会首页：按任务开始的三个入口、本周上升的问题表、最近的研究集合（阶段 3 之前隐藏该区块）。
- **要求**：
  - 表格列依次为：用户问题、提及人数、提及率（每千条评论，含上期对比）、供给情况、受众集中的作者、证据等级。
  - 文案严格使用 §6.10。
  - 表格下方固定显示 §6.10 的说明文字。
  - 组件只渲染接口字段（R4）。
- **验收**：截图；前端类型检查通过；没有任何前端计算（审查）。

#### T1.7 机会卡页面

- **前置**：T1.5
- **目标**：实现机会卡页面：标题区、四个关键数字、需求、供给、渠道三栏、判断与依据。
- **要求**：
  - 渠道栏用条形图同时画出点估计和区间；`insufficient` 单独成组，标题为“样本不足，暂不排序”。
  - 需求栏固定显示“重要性未校准”提示块。
  - 供给栏在阶段 1 显示“暂不判断”。
  - 每个带数字的组件都必须接收质量标记参数。
- **验收**：用测试数据集的 P1、P2、P3 分别截图；墓碑原声的显示正确。

#### T1.8 数据质量检查与告警

- **前置**：T1.2
- **目标**：每次计算后运行检查：分母为 0、领域采集量周变化超过阈值、读模型过期、不变量违规。
- **要求**：结果写入日志，并在卡片 `flags` 中体现；告警方式沿用现有机制。
- **验收**：在测试数据集中人为制造每类问题，确认都能被检出。

#### G1 阶段 1 质量门

- 3–5 位运营同事完成真实任务测试（任务由负责人提供），记录完成率和卡点。
- 不变量检查为 0。
- 开关关闭时，现有界面行为不变（现有测试全部通过）。

### 阶段 2：供给判定与质量门

#### T2.1 判定相关表与提示词注册

- **前置**：G1
- **目标**：建 `work_problem_judgment`、`judgment_reject`、`supply_search_run` 三张表；按 §6.6.2 编写并注册提示词。
- **先决条件**：先提交新 AI 任务的注册方案（`linggan_ai_task`、batch 的 task/kind 约束、egress 的 material_class），**由负责人单独批准**后再实施（D-017）。
- **输入**：按 §6.6.2 与 D-026，先确认整篇材料快照是否满足三个条件，并把结论写入 DECISIONS；引文校验只针对作品原始文本；整篇理解只用 `scope = 'note'` 的结果作辅助。
- **验收**：提示词在现有版本系统中可查，标签定义与 §6.6.2 逐字一致。

#### T2.2 候选召回

- **前置**：T2.1
- **目标**：按 §6.6.1 实现候选召回。
- **要求**：只有 RECON 确认已有向量能力时，才实现向量来源；否则用开关关闭。缺少整篇理解的候选提交到现有理解任务。
- **验收**：测试数据集上 P1 的候选集合与 §8.2 一致。

#### T2.3 判定任务

- **前置**：T2.2
- **目标**：提交 AI 判定任务，按 §6.6.3 校验输出，并写入结果。
- **要求**：
  - 走现有队列的预算与许可检查；
  - 提交前按缓存键跳过已有结果；
  - 校验失败写 `judgment_reject`，重试 1 次。
- **验收**：用模拟模型返回以下五种非法输出，确认每种都被拒绝且原因正确：非 JSON、未知标签、缺引文、超长、引文不在原文中。合法输出能正确写入。重复提交不产生重复调用（计数断言）。
- **禁止**：把无效输出写成 `none`；在测试以外绕过预算检查。

#### T2.4 定向搜索补采（有条件）

- **前置**：T2.1
- **目标**：通过采集侧发起“按问题关键词搜索”任务，回执回来后写入 `supply_search_run`。
- **要求**：只有 RECON 确认采集侧支持这类任务时才实现；否则保持开关关闭，`coverage` 为 null，供给状态因此停在 `insufficient_coverage`。**不得自行编造采集接口。**
- **验收**：用回执样例完成端到端测试，或者在 DECISIONS.md 中记录“不支持”。

#### T2.5 供给状态计算

- **前置**：T2.3
- **目标**：按 §6.6.5 和 §6.6.6 计算有效标签、判定状态和供给状态，并写入读模型。
- **验收**：测试数据集上 P1 得到 `suspected_gap` + `ai_suggested`；把 W_b 改为人工复核后，P1 得到 `suspected_gap` + `verified`。另外为状态机的每个分支各写一个用例。

#### T2.6 标注工具

- **前置**：T2.1
- **目标**：建 `label_task`、`label` 两张表，实现分层抽样生成批次，以及一个简单的标注页面。
- **要求**：
  - 标注页面一次显示一对（问题表述，作品文本），提供三个按钮，**不显示 AI 的标签**，避免锚定。
  - 同一人对同一任务只能提交一次。
- **验收**：生成 250 对样本的批次；两人标注流程端到端可用。

#### T2.7 评估与质量门

- **前置**：T2.6
- **目标**：按 §6.8 计算 kappa、各类精确率和召回率，写入 `eval_run`，更新 `quality_gate`。
- **要求**：读模型的 `judgment_status` 根据 `quality_gate` 自动切换，无需改动页面。
- **验收**：
  - 用人工构造的混淆矩阵验证精确率和召回率的计算；
  - kappa 用已知数值的例子验证；
  - 质量门在通过和未通过两种状态下，接口快照都正确。

#### G2 阶段 2 质量门

- 在真实数据上完成至少 250 对样本的双人标注，并完成一次评估。
- 是否通过质量门以实际数据为准。**未通过也是合法结果**，页面继续显示“AI 建议”。

### 阶段 3：研究集合

#### T3.1 集合与条目

- **前置**：G1
- **目标**：建 `research_collection`、`collection_item` 两张表，实现创建集合、添加和移除条目、修改备注的接口。
- **验收**：接口测试；移除条目只写 `removed_at`，不物理删除。

#### T3.2 冻结与快照不可变

- **前置**：T3.1
- **目标**：实现 `POST /api/local/opportunity/collections/{id}/freeze`，按 §6.9 生成快照。
- **要求**：数据库层用 `BEFORE UPDATE OR DELETE` 触发器禁止修改快照表，触发时 `RAISE EXCEPTION`。
- **验收**：尝试修改快照时数据库报错（测试）；冻结两次得到版本 1 和 2，两者互不影响。

#### T3.3 快照展示与墓碑

- **前置**：T3.2
- **目标**：实现 `GET /api/local/opportunity/collections/{id}?version=`，按 §6.9 实时检查来源资格并生成墓碑，同时检测“内容已变化”。
- **验收**：撤回一条已冻结的原声后，读取快照时该条显示为墓碑，接口响应中不含其原文（R10）。

#### T3.4 匿名化导出

- **前置**：T3.3
- **目标**：实现 `export=md|json`，只导出快照版本。
- **验收**：导出内容里普通评论者统一为“读者”，不含任何评论者标识；撤回内容为墓碑。

#### T3.5 “当前集合”栏

- **前置**：T3.1
- **目标**：在证据库、评论研究、创作者三个现有界面顶部加入当前集合栏，任意对象都可以“放入集合”。
- **要求**：页面地址只带集合 ID。开关关闭时，三个界面与现状完全一致（R14）。
- **验收**：在三个界面各完成一次“放入集合”；现有界面测试全部通过。

#### G3 阶段 3 质量门

- 一位同事仅凭集合链接，就能复现同一批证据、数字和口径版本。

### 阶段 4：回测与校准

#### T4.1 时点查询

- **前置**：G0
- **目标**：让 §6.2 的全部计算支持任意 `as_of`，严格满足 R12。
- **要求**：分类版本取 `created_at <= as_of` 的最新版本；口径定义版本同理。
- **验收**：§8.2.3 的迟到数据用例通过，即同一问题在两个 `as_of` 下得到两组不同的期望值。

#### T4.2 回测运行器

- **前置**：T4.1、G2
- **目标**：对历史上每个周期，在当时的 `as_of` 下标出 `suspected_gap`，再看之后 4–8 周内这些问题上是否出现高互动作品，并与对照组比较。
- **要求**：
  - 回测配置（时间范围、高互动阈值、对照组规则）写成文件，**在运行前提交到仓库**，运行后不得修改。
  - 报告中写明用到了哪些当时还不存在的定义（未来信息泄漏），并说明影响。
- **验收**：报告按附录 D 生成；配置文件的提交时间早于报告的生成时间。

#### T4.3 问卷导入与校准

- **前置**：T1.2
- **目标**：导入外部问卷结果（每个问题的重要性和满意度），按机会算法计算机会分，写入卡片的 `importance`，并输出“提及率与机会分”的秩相关报告。
- **要求**：只做导入，不做问卷系统。机会分公式：`importance + max(importance − satisfaction, 0)`，两项都用 0–10 分制，量纲以导入文件说明为准。
- **验收**：用样例文件导入；导入后卡片的 `not_calibrated` 标记消失。

#### G4 阶段 4 质量门

- 回测报告和校准报告提交给负责人，由负责人决定 `suspected_gap` 是否保留，或者调整定义。

---

## 8. 测试与验证

### 8.1 测试层次

| 层次 | 内容 | 要求 |
| --- | --- | --- |
| 单元测试 | 统计函数、周期、等级、排序、状态机、输出校验 | 标准答案全部覆盖 |
| 性质测试 | Wilson 区间的三条性质 | 固定种子，10,000 组 |
| 集成测试 | 测试数据集 → 派生表 → 读模型 → 接口 | 与 §8.2 期望值逐项比对 |
| 不变量检查 | §8.3 | 每次计算后运行，CI 中运行 |
| 接口快照 | 卡片 JSON | 变化必须在 PR 中解释 |
| 界面截图 | 首页和机会卡 | 每个页面任务提交截图 |

### 8.2 测试数据集（确定性构建）

#### 8.2.1 设定

- 参数使用测试参数集（§6.3）。
- 领域 `D1`，`period_tz = Asia/Shanghai`，`period_basis = work_published_cohort`。
- 当前周期 W0 = 2026-11-02（周一）。前四个周期为 W-1 至 W-4。
- `as_of = 2026-11-10T00:00:00Z`。
- 测试数据写入隔离数据库中的现有表（作品、详情、作者归属、领域用途、评论、first_admission、package、restriction）和 0177 的问题表，通过 `scripts/test-opportunity-postgres.sh` 构建。
- 除特别说明外：
  - 所有作品都在 D1 的研究主集合中（`use_kind = 'domain_research'`、`role = 'primary'`），作者已知，有合格的 `published_at`；
  - “评论发布于某周”指**评论所属作品的发布时间在该周**（D-008）；
  - 所有评论者互不相同，ID 非空；
  - 评论有 first_admission 记录，`first_observed_at = 作品发布时间 + 1 小时`。

#### 8.2.2 数据

| 编号 | 内容 |
| --- | --- |
| 评论量 | W0 有 1,000 条有效评论；W-1 至 W-4 各有 1,200 条有效评论 |
| 迟到评论 | 另有 3 条评论在 W0 作品下，但 `first_observed_at = 2026-11-10T01:00:00Z`（晚于 `as_of`），其中 1 条提及 P1，来自一位新评论者 |
| 作者自评 | 另有 1 条评论发布于 W0，评论者是作品作者本人，提及 P1（不计入 n，也不计入 k） |
| P1 提及 | W0 中有 12 条提及 P1 的评论，来自 10 位评论者（其中 2 位各发 2 条）；另有 1 条提及 P1 的评论来自营销号（`excluded_reason = 'marketing'`）；另有 1 条提及 P1 的评论，评论者 ID 为空（`commenter_unknown`）。以上 14 条都在 1,000 条之内。W-1 至 W-4 各有 1 条提及 P1 |
| P2 提及 | W0 中有 3 条，来自 3 位评论者；更早的周期没有 |
| P3 提及 | W0 中有 1 条 |
| 作者 C1 | 窗口内其作品上共有 60 条有效评论，其中 4 条是 P1 的首次提及（这些评论包含在上述各周的有效评论之内，C2 同） |
| 作者 C2 | 窗口内其作品上共有 40 条有效评论，其中 3 条是 P1 的首次提及 |
| 排除：作者未知 | 另有 1 篇 W0 作品，`linggan_material_content_author_at` 返回 NULL，其下 5 条评论，其中 1 条提及 P1 |
| 排除：无周期锚点 | 另有 1 篇作品没有合格的 `published_at`，其下 4 条评论 |
| 排除：不在主集合 | 另有 1 篇作品在 D1 中 `role = 'reference'`，其下 3 条评论，其中 1 条提及 P1 |
| 排除：受限 | 另有 2 条 W0 作品下的评论，有 `restricted_at = 2026-11-09T00:00:00Z` 的限制记录 |
| 回填来源 | W-4 中有 10 条评论没有 first_admission 记录，其 `first_observed_at` 由最早观察回填（`first_observed_backfilled = true`），仍是有效评论 |
| 供给（P1） | 候选作品：W_a（人工标注 `answers`，412 赞）、W_b（AI 判定 `answers`，268 赞）、W_c（AI 判定 `mentions`，3,204 赞）；领域近 90 天作品点赞中位数为 600；`supply_search_run` 为 found = 34、collected = 28；判定质量门未通过 |

#### 8.2.3 期望结果

| 对象 | 期望 |
| --- | --- |
| P1 W0 | k = 10，n = 1000，rate_per_1k = 10.0，ci_low = 0.005441，ci_high = 0.018309，baseline_k = 4，baseline_n = 4800，mentioners_total = 14，等级 `significant` |
| 排除计数（`as_of` = 2026-11-10T00:00:00Z） | `not_in_domain_primary` = 3，`work_author_unknown` = 5，`period_anchor_missing` = 4，`observed_at_unparseable` = 0，`restricted` = 2，`creator_self` = 1；有效评论总数 = 1000 + 4 × 1200 = 5800；等式 5800 + 15 = 5815 = 全部候选评论（不含 3 条迟到评论） |
| 回填计数 | `first_observed_backfilled = true` 的有效评论 = 10 |
| P1 迟到用例 | 把 `as_of` 改为 2026-11-11T00:00:00Z：k = 11，n = 1003，ci_low = 0.006135，ci_high = 0.019531，mentioners_total = 15 |
| P2 W0 | k = 3，n = 1000，mentioners_total = 3，等级 `established`（总人数小于 5） |
| P3 W0 | k = 1，n = 1000，mentioners_total = 1，等级 `candidate` |
| C1 | k = 4，n = 60，ci_low = 0.026229，ci_high = 0.159254，`sufficient = true`，进入 ranked |
| C2 | k = 3，n = 40，ci_low = 0.025836，ci_high = 0.198642，`sufficient = false`，进入 insufficient |
| P1 供给（阶段 2） | coverage.ratio = 0.823529（28/34），counts.answers = 2，`supply_state = suspected_gap`，`judgment_status = ai_suggested` |
| P1 供给（W_b 改为人工复核后） | `suspected_gap` + `verified` |

#### 8.2.4 卡片快照

T1.4 首次通过时，生成 P1 卡片的 JSON 快照并提交到仓库。此后快照的任何变化，都必须在 PR 中逐项解释。

### 8.3 不变量 SQL

每条查询的期望结果都是 **0 行**：

```sql
-- I1 计数关系
-- 说明：以下表名省略了前缀 linggan_opportunity_（D-004），执行时补上。
SELECT * FROM problem_metric_period WHERE k > n;
-- I2 区间合法
SELECT * FROM problem_metric_period
 WHERE ci_low IS NOT NULL AND NOT (0 <= ci_low AND ci_low <= ci_high AND ci_high <= 1);
-- I3 缺数据显式为空
SELECT * FROM problem_metric_period
 WHERE (n = 0 AND (ci_low IS NOT NULL OR ci_high IS NOT NULL))
    OR (n > 0 AND (ci_low IS NULL OR ci_high IS NULL));
-- I4 非 none 判定必须有引文
SELECT * FROM work_problem_judgment WHERE label <> 'none' AND (evidence_quote IS NULL OR evidence_quote = '');
-- I5 人工判定必须有复核人
SELECT * FROM work_problem_judgment WHERE source = 'human' AND reviewer IS NULL;
-- I6 提及必须指向存在的稳定评论身份
SELECT m.* FROM linggan_opportunity_comment_mention m
LEFT JOIN linggan_opportunity_comment_fact f
  ON f.content_ref = m.content_ref AND f.comment_external_id = m.comment_external_id
WHERE f.content_ref IS NULL;
-- I6b 评论者未知与哈希为空必须一致
SELECT * FROM linggan_opportunity_comment_mention
WHERE (commenter_hash IS NULL) <> (excluded_reason IS NOT DISTINCT FROM 'commenter_unknown');
-- I7 样本不足的作者不得标记为充足
SELECT * FROM creator_problem_density d
WHERE d.sufficient AND d.n < (SELECT (params->>'creator_min_n')::int FROM metric_definition WHERE version = d.metric_def_version);
-- I8 谱系不成环（递归检查，深度上限 100）
WITH RECURSIVE walk(start_id, cur_id, depth) AS (
  SELECT from_problem_id, to_problem_id, 1 FROM problem_lineage WHERE op IN ('merge','rename')
  UNION ALL
  SELECT w.start_id, l.to_problem_id, w.depth + 1
  FROM walk w JOIN problem_lineage l ON l.from_problem_id = w.cur_id AND l.op IN ('merge','rename')
  WHERE w.depth < 100
)
SELECT * FROM walk WHERE start_id = cur_id;
-- I9 读模型中不含撤回内容（接口层还需集成测试）
SELECT oc.problem_id FROM opportunity_card oc
WHERE EXISTS (
  SELECT 1 FROM jsonb_array_elements(oc.payload->'demand'->'sampleVoices') v
  WHERE (v->>'tombstone')::boolean = false
    AND EXISTS (
      SELECT 1 FROM linggan_material_comment_restriction r
      WHERE r.content_public_ref::text = v->'commentRef'->>'contentRef'
        AND r.comment_external_id      = v->'commentRef'->>'commentExternalId'
    )
);
-- I10 未通过质量门的 AI 结果不得被标为 verified
SELECT oc.problem_id FROM opportunity_card oc
WHERE oc.payload->'supply'->>'judgmentStatus' = 'verified'
  AND EXISTS (
    SELECT 1 FROM jsonb_array_elements(oc.payload->'supply'->'works') w
    WHERE w->>'source' = 'ai'
      AND NOT EXISTS (
        SELECT 1 FROM quality_gate g
        WHERE g.task_type = 'work_problem_judgment' AND g.passed
          AND g.prompt_version = oc.payload->'versions'->>'judgmentPrompt'
          AND g.model = oc.payload->'versions'->>'model'
      )
  );
```

---

## 9. 编排：负责人如何驱动执行者

### 9.1 节奏

1. 一次只分派一张任务卡，使用附录 B 的提示模板。
2. 执行者提交 PR 后，负责人按 §9.3 审查。
3. 审查通过再合并，然后分派下一张。
4. 阶段结束时检查质量门；未通过就不进入下一阶段。

阶段 3 和阶段 4 可以由两个执行者并行推进，但它们修改的表不能重叠。

### 9.2 分派时必须附带的信息

- 任务卡全文（从 §7 复制）
- 本任务涉及的 §6 规格小节
- 最新的 RECON.md、DECISIONS.md
- 上一个任务的 PR 链接

### 9.3 负责人审查清单

1. PR 描述中的“依赖的现有符号”逐条有出处，抽查至少 3 条。
2. 构建、测试、Lint、类型检查的输出已贴出。
3. `grep` 新增代码中的数字字面量，每个都能对应到 `metric_definition` 或测试。
4. 没有修改任何标准答案和已有测试的期望值；如有修改，理由充分。
5. 搜索 `unwrap_or(0`、`unwrap_or_default`、`.unwrap()`、`todo!`、`unimplemented!`，逐个确认合理。
6. 涉及 AI 的代码都经过了质量门判断。
7. 涉及展示的代码都处理了墓碑和 `null`。
8. 不变量检查结果为 0 行。
9. 开关关闭时，现有行为不变。
10. §3.8 自检问题全部作答。

### 9.4 必须升级给负责人的情况

- 任何 §3.3 列出的停止条件。
- 同一任务连续两次验收未通过。
- 执行者提出修改硬规则或核心规格。
- 估计工作量超出任务卡预期一倍以上。

### 9.5 PROGRESS.md 格式

```markdown
## 2026-11-20 T1.2 问题指标计算
- 状态：done | in_progress | blocked
- PR：<链接>
- 结果：测试数据集 P1/P2/P3 与期望一致；不变量 0 违规
- 数据：真实数据重算 1,284 个问题，耗时 3 分 12 秒
- 遗留：无 | <问题编号，链接到 QUESTIONS.md>
```

---

## 10. 发布、回滚与运维

- **特性开关**：`opportunity_card` 默认关闭。先在内部环境打开，G1 通过后再对运营打开。
- **回滚**：关闭开关即可恢复原有体验。新对象只有 `linggan_opportunity_*`，不影响现有表；需要清除时，写一个前向迁移删除它们（D-005）。
- **重建**：提供以下命令：
  - 全量重算派生表
  - 全量重建读模型
  - 运行不变量检查

  命令名沿用现有命令行约定。
- **定义切换**：新增口径版本后，先全量计算新版本，校验不变量为 0，再切换“当前版本”指针。旧版本数据保留。
- **监控**：派生任务耗时、AI 调用次数与花费、`judgment_reject` 各原因的比例、读模型过期数量、不变量违规数。

---

## 附录 A：RECON.md 模板

```markdown
# RECON（代码库勘察）
勘察日期：
勘察人：

## 1. 基础
- 仓库结构概览（顶层目录及用途）：
- 后端语言与框架、入口文件：
- 前端框架、页面目录、路由方式：
- 构建、测试、Lint、类型检查命令（附一次实际运行的结果摘要）：

## 2. 数据库
- 类型（SQLite / PostgreSQL / 其他）与版本：
- 访问方式（ORM / 查询构建器 / 原生 SQL）与示例位置：
- 迁移工具、迁移目录、命名规则、回滚方式：

## 3. 现有数据（逐项给出 表名.列名 和定义位置）
- comments_table / comment_pk / comment_work_fk：
- comment_published_col / comment_observed_col（不存在则写“无”）：
- commenter_col（平台评论者标识，是否稳定）：
- works_table / work_pk / work_domain_col / work_creator_col：
- 点赞数字段（作品、评论）：
- 来源资格：eligibility_predicate 与 is_withdrawn 的判断方式：
- 评论研究中的“用户问题”实体、与评论的关联方式、“已核验”的判定位置：
- “用户提到的办法与经历”的存储位置：
- 整篇理解结果：存储位置、understanding_fields 实际字段：
- 观察关系的存储位置：

## 4. 能力
- AI 任务队列：提交、回执、预算、许可的接口与示例调用位置：
- 提示词与 Skill 版本注册方式（prompt_registry）：
- 采集侧：可用任务类型；是否支持按关键词定向搜索：
- 向量检索 / 嵌入能力：有 / 无，位置：
- 密钥与配置读取方式：
- 特性开关机制：
- 日期时间库、加密库（Cargo.toml / package.json 中已有的）：
- 性质测试库（如有）：

## 5. 约定
- 后端模块组织与命名：
- API 路由风格、错误格式、分页方式：
- 日志方式：
- 现有测试的组织方式：

## 6. 未找到的项
- （列出项目、查找方法、影响哪些任务）
```

## 附录 B：任务提示模板（负责人分派时使用）

```text
你是本项目的执行 agent。请完成任务卡 <任务编号>。

开始前必须：
1. 阅读 docs/opportunity/playbook.md 的 §2（硬规则）和 §3（防幻觉协议）。
2. 阅读 docs/opportunity/RECON.md 和 DECISIONS.md。DECISIONS.md 与手册具有同等约束力。
3. 阅读下面的任务卡和相关规格小节。

工作方式：
- 只使用 RECON.md 中记录过的现有符号。需要新的，先勘察并补进 RECON.md，再使用。
- 遇到 §3.3 的任何停止条件，立即停下，写入 QUESTIONS.md 并告诉我，不要按假设继续。
- 不得修改标准答案和已有测试的期望值。
- 完成后，按附录 C 写 PR 描述，贴出构建、测试、Lint、类型检查和不变量检查的实际输出。

任务卡：
<粘贴 §7 中的任务卡全文>

相关规格：
<粘贴 §6 中的相关小节>
```

## 附录 C：PR 描述模板

```markdown
## 任务
<任务编号与名称>

## 改动摘要
-

## 依赖的现有符号
| 符号 | 位置（文件:行号） | 用途 |
| --- | --- | --- |

## 新增的数字字面量
| 值 | 位置 | 来源（metric_definition 键名 / 测试） |
| --- | --- | --- |

## 验证输出
- 构建：
- 测试：
- Lint：
- 类型检查：
- 不变量检查：
- 截图（如涉及页面）：

## §3.8 自检
1.
2.
3.
4.
5.
6.
7.
8.

## 决定与遗留
- DECISIONS.md 新增：
- QUESTIONS.md 新增：
```

## 附录 D：回测与校准报告模板

```markdown
# 回测报告
- 配置文件与提交时间：
- 时间范围、周期数：
- “高互动作品”定义：
- 对照组规则：
- 结果：
  - 被标为 suspected_gap 的问题数、之后出现高互动作品的比例：
  - 对照组的同一比例：
  - 按提及人数排序前 N 的同一比例：
- 未来信息泄漏说明（用到了哪些当时不存在的定义）：
- 结论与建议（保留 / 调整 / 取消 suspected_gap）：

# 校准报告
- 问卷样本量、问题数：
- 提及率与机会分的 Spearman 秩相关系数：
- 结论：提及人数能否用于排序，还是只用于发现问题：
```
