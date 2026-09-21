# COMMENT-RESEARCH-REBUILD-001 · 单一合同重建

> 状态: 活跃计划
> 最后核对: 2026-09-21
> 适用范围: Issue #295；本机评论研究派生层 reset 与新实现
> 事实来源: DEC-0006、Mog 明确 reset 范围、现行 Evidence schema、提交前审查
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实代码/数据库/测试

> 当前阶段: Card 1–5、读取 API/UI、受限真实模型 adapter 与本机评论研究派生层 reset 均已完成并发布；P3 的真实质量出口仍未验证。当前专属候选只修复 StudySource 的作品作者声音排除，尚未合并或部署。

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
4. **已完成实现，真实质量出口未验证**：Problem resolution 只接受双独立证据；Embedding/FTS 候选召回已接入，但没有合格 embedding profile 时必须如实返回 `retrieval_incomplete`，不能判定新问题。真实模型端到端成功、Recall@K 和 P3 出口仍未验证。
5. **已完成**：替换旧 Rust/API/UI/worker 活跃路径，并在不迁移旧评论研究结果的前提下完成本机 reset；下一次干净真实运行仍须在当前作者声音排除修复发布后由 Mog 验收。

## 非目标

本次作者声音排除候选不调用真实模型、不自动清库、不应用共享 migration、不部署、不合并 main；它不替代下一次干净样本的人工真实验收。
