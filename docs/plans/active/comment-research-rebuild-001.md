# COMMENT-RESEARCH-REBUILD-001 · 单一合同重建

> 状态: 活跃计划
> 最后核对: 2026-09-16
> 适用范围: Issue #295；本机评论研究派生层 reset 与新实现
> 事实来源: DEC-0006、Mog 明确 reset 范围、现行 Evidence schema、提交前审查
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实代码/数据库/测试

> 当前阶段: Card 1–5 核心数据、批次、局部恢复、lease 与模型调用账本预留已实现；尚未接入真实模型 adapter、API/UI 或本机 reset。

## Reset 边界

只删除旧研究派生关系及其视图/函数，绝不删除 `linggan_material_*`、`linggan_runtime_capture_package`、作品/评论原始正文、`domain_ref` 或媒体 disposition。旧名 `linggan_comment_research_restriction` 的限制事实会无损转入 `linggan_material_comment_restriction` 后删除旧名；它不是研究结果迁移。

旧的 V1 派生关系包括：`linggan_comment_research_policy_*`、`derivation*`、`run*`、`atom*`、`embedding_space`、`problem*`、`result_revision*`、`change_observation`、`problem_resolution*`、`problem_catalog_guard`。reset 使用明确清单和 receipt；不得用名称通配或 `CASCADE`。

## 当前合同（唯一）

1. `StudyWork`：`domain_ref` 必须等于当前 ADHD 域；作品为选择、共享上下文和预算单位。
2. `StudySource`：当前可读普通用户评论；评论为目标、成本和唯一 Atom 证据。
3. `StudyContextSnapshot`：标题、正文、OCR/ASR 是同一作品、截至冻结时刻成功且当前未撤回/未受限的来源片段；sent、omitted、missing 都显式记录。
4. `StudyRun`：用户确认后冻结已选择作品、评论与上下文；没有自动重跑。
5. `StudySignal`：模型原子仅可引用当前评论的连续原文；没有 problem/need 准入时仍可保留非问题语义。
6. `StudyProblem`：只有符合 Frame、候选闭集决策与独立双证据创建条件才能建立或关联；其余保持 deferred，不混作结果失败。

## 实施顺序

1. 先建立 reset receipt 与新 schema，使用新的 `linggan_comment_study_*` 名称；旧 V1 模块尚未删除前，新 schema 不对外接单。
2. 建立 source/work/context 查询并用 domain/readability/disposition/时间反例钉住。**进行中：已完成 ADHD、as-of、restriction、withdrawn OCR 和父回复语境的隔离 PostgreSQL 证明。**
3. 建立按作品选择、评论预算、同篇批次 semantic invocation 与输出接纳。**进行中：已完成用户选笔记的冻结 Run、评论级目标、同篇 batch 输入冻结、lease 领取/过期恢复、逐 target 输出接纳与漏项局部重试，以及一 batch 一通用 invocation 的预留、幂等复用、attempt 关联和终态回写；尚未接入真实模型 adapter 或 API。**
4. 建立 Problem resolution；只接受新的双独立证据创建合同。**已完成规则内核、候选闭集持久化和独立双证据建档；Embedding/FTS 候选召回、真实模型 worker 和读取 API/UI 尚未接入。**
5. 替换 API/UI/worker 后删除旧 Rust 模块、旧页面与旧 endpoint；最后才启用本机 reset/init。

## 非目标

不调用真实模型、不自动清库、不应用共享 migration、不部署、不合并 main。Mog 在新 UI/API 构建完成并确认本机 reset 后，才执行一次实际清空与人工真实测试。
