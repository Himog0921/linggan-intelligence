# DEC-0003 · 评论研究采用唯一语义内核

> 状态: 权威当前
> 最后核对: 2026-09-09
> 适用范围: 评论研究的研究输入、语义提取、向量召回、问题归并、结果发布、变化观察和对应读模型
> 事实来源: Mog 2026-09-09 的明确决定、Issue #213 的三路只读代码走查、当前 `main@45ffdf0`
> 冲突时以谁为准: 用户最新决定、`AGENTS.md`、真实代码/迁移/运行证据

## 决定

评论研究在开发期采用一条且仅一条正式语义路径：

```text
Raw Comment
→ Research Derivation
→ Semantic Atom
→ Embedding Candidate Recall
→ Problem Resolution
→ Atom–Problem Membership
→ Frozen Result Revision
→ Overview / Voices / Problems / Changes
```

旧 Task B（candidate/relation/problem-vector）与当前 P4（V4/V5 adapter、HDBSCAN/Leiden group）均不再作为正式研究路径、页面读取来源或 fallback。开发期可以删除其历史研究派生结果；不将旧 Problem、Atom、Group、Member、Vector 或变化结果迁移进新内核。

## 保留与删除的边界

保留的不是旧研究结果，而是可独立复用的事实和基础能力：原始评论、采集包、来源资格、作品与作者身份事实、原始评论正文、清洗的原文位置映射、模型调用账本、预算、lease/retry、来源撤回传播、受控外部计算边界。

`作者` 平台徽标属于作者回复身份，不是评论文本。原始正文不得改写；新的 `Research Derivation` 必须得到 `ordinary_user`、`content_author_reply` 或 `author_identity_unknown`。默认只有已确认 `ordinary_user` 进入用户问题与变化统计；其余仍可按权限作为原声/上下文读取，但不是研究分母。

向量只用于候选召回，始终是可重算派生物，不形成第二事实源。相似度不能自动证明同一问题；每个 Atom–Problem 关系必须保存关系结果、依据、版本和证据片段。问题不自动升级为正式 Topic、市场趋势或现实需求事实。

## 被替代的内容

CI-AUTO-004、CI-AUTO-003 及更早评论研究合同中的旧问题候选、旧成员投影、V4/V5 兼容 Atom、HDBSCAN/Leiden 组织、旧超级查询、旧页面状态和旧变化计算，只保留为当时实现/运行的历史证据。它们不得再规定 Issue #213 的未来架构。

这不否定旧发布回执，也不授权立即清空共享数据库、切换 runtime、自动外发或运行真实评论研究。那些动作必须在 Issue #213 的代码、隔离数据库、API/UI 证明完成后，另由 Mog 明确授权。
