# COMMENT-RESEARCH-RESET-001 · UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-10
> 适用范围: 评论研究与模型设置的用户界面替换
> 事实来源: PAGE-COMMENT-RESEARCH-V1-001、DEC-0003、用户测试反馈
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行证据

## 变更原因

用户测试确认旧页面存在全量超级查询导致卡顿、tab 语义重复、作者徽标进入研究文本、重复授权，以及把“语义向量准备”暴露给普通用户的问题。旧页面和旧自动计划不能保留为迁移期 fallback。

## 表面地图与替代

| 旧表面 | 处理 | V1 替代 |
|---|---|---|
| `/api/local/comment-intelligence` 全量快照 | 删除 | overview / voices / problems / changes / runs 五个独立只读 endpoint |
| 每日观察、查询、资产、标注、Task B/P4 设置 | 删除 | 一个保存 policy 的研究设置 modal |
| 旧模型设置中的 trial/backfill/automatic plan | 删除 | 连接、V1 JSON 测试、默认模型、向量配置和调用账本 |
| “等待语义向量准备”原声标签 | 删除 | 原声仅显示已通过身份过滤的普通用户证据、研究正文和研究结果 |

## 运行与可访问性

- 所有本地 mutation 保留同源 guard 与 `Cache-Control: no-store`。
- 状态使用文本和 `role=status`，不只用颜色区分。
- 运行中、无结果、不可比、配置缺失与失败是独立状态。
- 本次不改全局 LIDS token、shell 结构或非评论研究页面。

## 证明矩阵

| 断言 | 证据 |
|---|---|
| 旧入口不存在 | Rust route/page test 与 API route probe |
| 新 tab 不再共享重查询 | endpoint SQL / HTTP 计时测试 |
| 作者角色不污染研究正文 | isolated PostgreSQL derivation fixture |
| 设置后直接开始研究 | HTTP mutation test；模型调用数断言 |
| 运行时显示新页面 | exact-head runtime switch 与浏览器截图 |
