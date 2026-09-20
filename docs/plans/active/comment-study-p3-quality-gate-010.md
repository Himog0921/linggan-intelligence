# COMMENT-STUDY-P3-QUALITY-GATE-010 · 向量质量闸门与主线整合准备

> 状态: 活跃计划

> 最后核对: 2026-09-20

> 适用范围: Issue #316；P3 候选的主线基线核对、质量验证协议与不接触真实评论的自动验证准备

> 事实来源: Mog 2026-09-20 明确授权、`DEC-0006`、当前 `origin/main`、P3 候选代码与 Greenfield P3/P4 规格

> 冲突时以谁为准: 用户最新决定、`AGENTS.md`、真实运行/数据库副作用、当前代码与测试

## 用户结果与边界

本工作包的结果不是“P3 已完成”，而是让 P3 具备可执行、可审计的质量闸门：任何未来的向量召回、冷启动建题或真实链路回放，都有固定的样本结构、指标口径、失败归因和停止条件，且不会把未比较或未测内容说成没有匹配或已经通过。

本包允许：候选分支在本机重放到最新 `origin/main`、编写协议和合成/隔离 fixture、运行不接触真实评论和外部模型的检查。

本包禁止：真实评论读写或导出、真实 WeMM/第三方模型调用、共享数据库 migration/reset、runtime 切换、部署、push、PR、merge；不得把 P4/P5 候选改动作为本包交付。

## 已确认事实

1. P3 候选 `feat/comment-study-p3-vectors` 原为以 `5371311` 为基线的 P3–P5 混合 40 个提交。经本包授权的本地 rebase 后，它现在是 `ab0a7eb`，与 `origin/main@8348096` 的左右差为 `0 / 40`。原 `129c9e3` 保留为本地安全分支 `codex/comment-study-p3-vectors-pre-rebase-20260920`。这只证明文本重放完成，不证明可合并、已审查或可运行。
2. rebase 的唯一文本冲突是 `docs/progress/2026-09.md`；主线的授权、工位及 Paddle OCR 记录与 P3 记录均被保留。源码/共享 migration 没有直接文本冲突。
3. P3 候选包含 canonical 编码、缓存、精确召回、未归并池、比较缓存、冷启动配对、本机 embedding probe 和 profile 资格记录；其历史进度仍明确缺少真实标注 Recall@K 与 Rust 到真实 WeMM 的端到端回放。候选不是 Issue #316 的实现输入，更不是可直接整合的交付物。
4. 当前 main 的 OCR 已改为受控分层。评论研究语境的候选查询仍读取 `linggan_material_derived_text`，因此“哪些图像文本可以进入 StudyContextSnapshot”必须在最新基线上通过来源/资格回归后才可认定兼容。
5. 参考规格建议的首轮人工评估规模是 100–200 条真实评论、至少 40 对问题比较、20 个已知召回案例；它们是工程验证规模，不是预先批准的真实数据范围，也不是自动通过阈值。

## 质量闸门协议

### 1. 版本化评估包（尚不创建真实样本）

未来每个评估包必须独立记录：评估包版本、抽样规则、冻结时间、允许用途、脱敏方式、标注指南版本、双人复核/裁定记录、输入的来源引用或受控替代引用、embedding profile、候选目录快照、K 值、执行代码 revision 与结果生成时间。

真实原文、可反向识别评论文本、导出向量和模型提示词不得写入 Git、普通日志或本计划。真实样本开始前，Mog 必须明确最小样本、访问者、存放位置、脱敏/保留和处置方式，以及是否允许任何模型接收何种转换后的内容。

### 2. 必测分层

| 分层 | 正例与反例必须覆盖的判断 | 失败归因 |
|---|---|---|
| 语境资格 | 原生标题/正文、已接受图像实质文本、部分/退役 OCR、ASR、父级上下文 | source gate、来源资格、上下文 manifest |
| Frame | 已知/未知主体、目标、障碍、身份相关场景 | 抽取、清洗、标注不一致 |
| 比较边界 | 同题、相似但不同障碍、否定/反转、同目标不同障碍、不同主体/场景 | canonical、向量候选、比较判断 |
| 独立性 | 同篇不同已知账号、同账号跨评论、同条多 Atom、身份未知 | 资格/去重/事务规则 |
| 召回 | 既有 Problem 核心、代表 Atom、未归并池、空目录与目录不完整 | profile、过滤、K、索引/向量、候选折叠 |
| 可见性 | `retrieval_incomplete`、`budget_stopped`、`deferred_novel`、`deferred_ambiguity` | UI/API 状态投影，不得降格为无匹配 |

### 3. 指标与报告

召回报告必须把既有 Problem 与未归并池分开，按 route（身份键、固定核心、代表 Atom、未归并池）记录 Recall@K。每个漏召回必须归类为 canonical 表述、embedding/profile、过滤、K/预算、目录不完整或标注争议；不能只输出一个总分。

比较报告必须分列 subject、goal、barrier、context 及 overall，并单列模型/规则不确定性。确定性不变量（同账号不能构成两独立支持、重复投递不重复建题、并发 CREATE 的原子性、未比较不写成 no match）是零容忍项，不由平均指标抵消。

性能报告只在实际测量后写入：M4 峰值内存、P50/P95、每目标成本、失败/修复率及与 OCR/ASR 的并行影响。任何通过阈值由 Mog 在真实样本范围和模型边界获准后决定；本包不自行设定数字。

### 4. 分层验证顺序

1. **本包内、无真实数据**：最新主线基线、合成/隔离 fixture、source context provenance、状态枚举可见性及治理检查。
2. **需新授权的受控真实样本**：完成样本账本和人工标注；不调用模型时可先产生离线标注报告。
3. **需新授权的真实模型回放**：Rust → Pi adapter → 本地 WeMM → PostgreSQL 的最小闭环，记录模型与 profile 身份；不把 provider 返回成功当作语义通过。
4. **P3 出口裁定**：同篇两个已知不同账号可按共同核心建题、同账号不能、模型不确定时不能建题；同时提供 Recall@K、冷启动端到端和当前支持事务证明。没有这些证据，状态保持未完成。

## 本包执行项与停止点

| 顺序 | 动作 | 可证伪验收 | 停止点 |
|---|---|---|---|
| A | 本地 rebase 候选到当前主线 | `0 / 40` 分歧、无冲突标记、`git diff --check` | 出现源码/迁移语义冲突则不臆断处理 |
| B | 冻结本质量协议与索引/进度记录 | 本文件被索引、状态边界不声称真实验证 | 需要改变权威产品/数据语义则升级决定 |
| C | 设计 OCR→StudyContextSnapshot 的合成回归 | accepted / partial / retired 资格与来源可由 fixture 证伪 | 若需改 worker、schema 或 migration，停止并报 Mog |
| D | 设计未比较状态的 API/UI 回归 | `retrieval_incomplete` 与 `budget_stopped` 不被隐藏或归为无匹配 | 真实 UI 修改前先读 UI 执行合同并更新 Claim 文件边界 |
| E | 建立真实样本与真实模型的授权申请包 | 用途、样本、人员、模型、输出、保留/处置均明确 | 未获逐项授权时不接触真实评论或模型 |

### C 的本地执行记录（2026-09-20）

完成 source context provenance 的合成回归，而未修改 worker、schema 或 migration。`StudyContextSnapshot` 现在保留非 OCR 派生语境；对 OCR，只读取同一 derivative 最新、未退役、未撤回且 `ACCEPTED` 的非空 `image_substantive_text`，不再把 raw `ocr_text`、`PARTIAL` 或旧 accepted layer 当作模型语境。隔离 fixture 直接覆盖 accepted、latest partial 覆盖旧 accepted、retired、withdrawn 与同 slot 重观测，因而这些条件可由 41 条 PostgreSQL proof 中的对应断言证伪。

这只完成 C 的无真实数据部分；D 已进入受控的 UI 状态投影回归准备，E（真实样本/模型授权申请包）仍未开始。

### D 的 UI 变更清单（实施前，2026-09-20）

**事项与目标。** Issue #316；worktree `comment-study-p3-quality-gate-010`。目标是在既有 `/corpus/comments` 的 L1 Corpus Explorer 中，让 `retrieval_incomplete` 与 `budget_stopped` 两个既存归并结果以中文独立表达，并保留在既有“待归并”工作面。用户由此能看出“尚未完成判断”，而不会把它误读成无匹配或新问题。非目标：新增状态、API/数据库字段、权限、按钮、路由、真实运行、评论或模型访问，以及任何 Token、Primitive、CMP、Scene、Motion 改动。

**读取回执。** 已读取 `AGENTS.md`、`docs/current-state.md`、`docs/agents/ui-execution-contract.md`、`docs/design/README.md`、`docs/design/lids/README.md` 与 `agent-execution-guide.md`；页面规格 `PAGE-COMMENT-STUDY-REBUILD-001`；`LIDS-LANG-001`、`LIDS-BOUND-001`；以及当前 API/UI 代码和 `comment-study-001.sql`。它们共同要求 L1 只读投影、中文独立承载状态含义、PARTIAL/未知不得隐藏或降格为失败，并禁止以 UI 改写数据语义。

**分类与依据。** 分类为“展示 + 状态语义”；最高风险是把目录未查全或预算中止降格为“没有匹配”。依据是质量协议第 2 节可见性、PAGE 的 `DEFERRED`/`PARTIAL` 约束，以及 `comment_study_recall.rs` 的不完整召回合同。无需 `DECISION_REQUIRED`：两个数据库字面状态、现有“待归并”页面和其只读职责均已存在。页面为 L1，唯一 Pattern 为 Corpus Explorer；不触及 Token、Primitive、CMP、Scene、Motion，仅修正 Data Truth 的文字投影。

**Surface map 与状态字典。** 受影响面仅为 `/corpus/comments`：概览的“归并判断结果”、目标表 Signal 状态、Signal 卡片和既有“待归并”列表。`retrieval_incomplete` 的中文为“候选目录未查全，当前不能判定是否为新问题”：它不等于无匹配，也不等于已建题。`budget_stopped` 的中文为“归并预算已到上限，当前未完成判断”：它不等于无匹配，也不等于模型判断失败。二者均加入既有待归并过滤集合；其余状态、后端字面值与状态迁移一律不变。

**依赖与验收矩阵。** `comment-study-001.sql` 是状态闭集来源；`comment_study_recall.rs` 是 `retrieval_incomplete` 的语义来源；本地 API 投影与 `comment_study.js` 只负责展示。静态 API 回归须证明两个中文标签存在、待归并集合包含二者且仍按 `resolutionState` 筛选；`cargo test -p linggan-api --bin linggan-api --locked`、格式、diff 与治理检查须通过。因为本包不启动 runtime 或浏览器，真实页面渲染、响应式/键盘走查与业务验收均为 `NOT VERIFIED`；不会用静态字符串测试冒充这些证据。停止条件：发现需新增状态/API/动作、须改 CSS/Token 或真实数据/运行时才可解释时，停止并回报 Mog。

## 文件与交付边界

- 本包 exclusive：本文件、`crates/intelligence/src/comment_study_{canonical,embedding,recall,candidate_recall,comparison_cache,problem_resolution,problem_store,resolution_worker,read,source}.rs`、`crates/intelligence/src/{model_runner,pi_adapter,lib}.rs`、`crates/intelligence/tests/comment_study_rebuild_postgres.rs` 及其 P3 测试支持脚本、`apps/api/src/local_web/comment_study.rs`、`apps/pi-adapter/src/wemm_runtime.py`、`database/bootstrap/comment-study-001.sql` 和配套 reset。它们由 Issue #316 的 [原 Claim](https://github.com/Himog0921/linggan-intelligence/issues/316#issuecomment-5749282350)、[revision-read 更正](https://github.com/Himog0921/linggan-intelligence/issues/316#issuecomment-5749395396) 及 [OCR context amendment](https://github.com/Himog0921/linggan-intelligence/issues/316#issuecomment-5749506006) 明确列入；均仅限本地代码与隔离 fixture。
- 本包 shared：`docs/README.md`、`docs/current-state.md`、`docs/progress/2026-09.md`，仅作索引与有界状态记录。
- 本包 forbidden：`database/migrations/**`、`apps/worker/**`、P4/P5 领域实现、所有 runtime/deployment 配置。`apps/api/src/local_web/comment_study.js` 由 [D 项 Claim amendment](https://github.com/Himog0921/linggan-intelligence/issues/316#issuecomment-5749597843) 列为 exclusive，但仅限既有状态的中文投影与待归并可见性回归；这不构成对真实运行的授权。

本包结束时分别报告：协议/设计、代码、自动检查、真实链路、部署、Mog 业务验收。后一四层如未发生，必须写为 `NOT VERIFIED` 或 `N/A`，不得以 rebase 或合成测试替代。

## 2026-09-20 本地 P3 导入后的修复证据

在最新基线中仅重放 P3 所属的十个候选提交后，合成隔离验证发现并修复以下“修订版为权威”的断裂：Problem 定义的 `includeCriteria` 未写入 revision；活跃 Problem 固定核心没有进入 embedding 待办；Problem 读取与旧 lexical candidate recall 仍读取已从主表移走的字段；probe API 也没有区分 runtime 与 storage 不可用。随后也收紧 OCR→StudyContextSnapshot：只有最新、未退役、未撤回的 accepted image substantive text 可作为图片语境。

修复后，Problem 的当前 revision 成为 definition/core frame/inclusions/exclusions 的唯一读取面；embedding 待办按 canonical hash 对 Signal 与 Problem core 合并去重；candidate recall 直接 join `current_revision_ref`。新增的隔离 PostgreSQL 回归显式验证 lexical candidate recall 读取 revision，而不是依赖向量路径间接覆盖。

本记录只证明本地代码与合成数据库契约：不构成真实标注 Recall@K、真实 Rust→WeMM 回放、OCR 语境资格回归、runtime 切换、共享数据库 migration、部署或 Mog 业务验收。
