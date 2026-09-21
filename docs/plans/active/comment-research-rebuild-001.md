# COMMENT-RESEARCH-REBUILD-001 · 单一合同重建

> 状态: 活跃计划
> 最后核对: 2026-09-21
> 适用范围: Issue #295；本机评论研究派生层 reset 与新实现
> 事实来源: DEC-0006、Mog 明确 reset 范围、现行 Evidence schema、提交前审查
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实代码/数据库/测试

> 当前阶段: Card 1–5、读取 API/UI、受限真实模型 adapter 与本机评论研究派生层 reset 均已完成并发布。setup 预览与 Target 冻结已收敛到同一 StudySource gate，运行中补充了语义／证据计数。真实运行随后证明 Pair 自动比较会接近穷举；`COMMENT-STUDY-PAIR-BOUNDS-001` 正在将其收束为每个 Signal 一次首选自动比较并补全可解释记录。P3 的真实质量出口、Recall@K 与业务价值仍未验证。

## Reset 边界

只删除旧研究派生关系及其视图/函数，绝不删除 `linggan_material_*`、`linggan_runtime_capture_package`、作品/评论原始正文、`domain_ref` 或媒体 disposition。旧名 `linggan_comment_research_restriction` 的限制事实会无损转入 `linggan_material_comment_restriction` 后删除旧名；它不是研究结果迁移。

旧的 V1 派生关系包括：`linggan_comment_research_policy_*`、`derivation*`、`run*`、`atom*`、`embedding_space`、`problem*`、`result_revision*`、`change_observation`、`problem_resolution*`、`problem_catalog_guard`。reset 使用明确清单和 receipt；不得用名称通配或 `CASCADE`。

## 当前合同（唯一）

1. `StudyWork`：`domain_ref` 必须等于当前 ADHD 域；作品为选择、共享上下文和预算单位。
2. `StudySource`：当前可读、具有稳定作者身份且作者明确不是该作品创作者的普通用户评论；评论为目标、成本和唯一 Atom 证据。作品作者的根评论或回复可作为已知父评论语境，但绝不能成为 target、Signal、社区词或 Problem 的证据。任一方作者身份未知时，角色为 unknown，不能默认当作用户声音。
3. `StudyContextSnapshot`：标题、正文、OCR/ASR 是同一作品、截至冻结时刻成功且当前未撤回/未受限的来源片段；sent、omitted、missing 都显式记录。
4. `StudyRun`：用户确认后冻结已选择作品、评论与上下文；没有自动重跑。
5. `StudySignal`：模型原子仅可引用当前评论的连续原文；没有 problem/need 准入时仍可保留非问题语义。
6. `StudyProblem`：只有符合 Frame、候选闭集决策与独立双证据创建条件才能建立或关联；其余保持 deferred，不混作结果失败。

## 实施顺序

1. **已完成**：建立 reset receipt 与新的 `linggan_comment_study_*` schema，并移除旧 V1 模块的活跃路径后启用本机 reset/init。
2. **已完成**：建立 source/work/context 查询，并以 ADHD、as-of、restriction、withdrawn OCR、父回复语境和作品作者声音排除的隔离 PostgreSQL 反例钉住。
3. **已完成**：建立按作品选择、评论预算、同篇 batch semantic invocation 与输出接纳，以及受限真实模型 adapter、读取 API/UI 和本机发布路径。
4. **已完成实现，真实质量出口未验证**：Problem resolution 只接受双独立证据；Embedding/FTS 候选召回已接入，但没有合格 embedding profile 时必须如实返回 `retrieval_incomplete`，不能判定新问题。当前私有 Gold Set 只含经授权的双代理交叉标注 spot check（1 正例、2 hard negative、1 unknown），不等同于 Recall@K 通过；真实模型端到端成功、完整 Recall@K 和 P3 出口仍未验证。
5. **已完成**：替换旧 Rust/API/UI/worker 活跃路径，并在不迁移旧评论研究结果的前提下完成本机 reset；下一次干净真实运行仍须在当前作者声音排除修复发布后由 Mog 验收。

## COMMENT-STUDY-PAIR-BOUNDS-001（实施中）

### 已确认的运行事实

一次有合格 embedding profile 的真实运行中，13 条 `deferred_novel` Signal 已产生接近全部两两组合的自动 Pair；大量 Pair 的模型调用已按机器合同接纳，但没有建立 Problem。现有 `rejected` 又把“不是同题”“证据不足／不确定”和“合同未接纳”压成一个表面状态。这个事实只证明自动搜索需要止损和结论需要可解释；它**不**证明任何相似度阈值、Top-K 或 Recall@K 门槛。

### 本包的最小变更

1. 每条 `deferred_novel` Signal 只参与一次由当前 qualified embedding profile 选择的首个合格候选 Pair；有效未建题后仍保持 `deferred_novel`，但不再由后台继续枚举其它候选。
2. 新 Pair 在既有 `pair_manifest` 中记录实际选择方法、embedding profile、原始召回位次与通过独立性筛选后的位次；不存储向量、距离、评论正文或完整候选池。
3. 新 Pair 的终态在同一 manifest 中记录明确代码：`approved`、`not_same_problem`、`ambiguous`、`insufficient_independent_evidence` 或细分的 `contract_rejected_*`。读取面以中文解释这些代码；不暴露 provider 原始输出。
4. 已有 Pair、其 invocation 收据及其缺失的历史选择信息均保持原样，不回填、不删除、不重算。

### 不在本包内的决定

- 不设相似度阈值、不固定 Top-K 或 Run budget；候选召回的历史 rank/distance 未被完整保存，私有 Gold Set 也不足以校准这些参数。
- 不降低双独立来源的 Problem 创建条件，也不将 `deferred_novel` 改写为失败。
- 不触发真实模型调用、共享数据库写入、runtime 部署或自动重跑；这些须在候选合并后的独立授权与真实验收中处理。

## 非目标

本次候选不自动清库、不应用共享 migration、不修改连续排程；真实模型回放、合并、部署和 Mog 前端验收必须分别以实际回执报告，不得由这份计划或隔离证明替代。
