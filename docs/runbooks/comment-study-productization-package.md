# 评论研究产品化 · 完整手册与执行入口

> 状态: 权威当前（本交付包的已批准目标）；实施进行中，不是已交付声明
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 的文档、实现步骤与阶段验收
> 事实来源: Mog 当前对话的开发授权、手册 v1.0、已确认产品原型、main@c74d72e3
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库证明；字段以对应合同为准

## 授权及版本

Mog 已明确要求开始实际增量开发，先将完整手册入库，逐步检查是否偏离。追踪在 Issue #295 的本次交付启动回执；其历史 reset 授权不适用于本包。专属分支为 `chatgpt/comment-study-productization-001`，源码基线为 `c74d72e3d17b9d5ecfb9953de025713c47e4560e`。

以下八份正文保留文档交付时的版本状态头。它们原来的“待用户批准开发”描述的是成文时点；本入口只将**本阶段开发授权**更新为已批准，不代表迁移、模型、合并或部署已经获准。自动化分册始终是下一阶段设计，不是本轮计划表或定时外发授权。技术字段、规则和验收不得因这一状态更新而改变。

## 模块化手册

| 阅读任务 | 唯一正文 |
|---|---|
| 范围、D01–D12、P0–P5 | [技术裁定与范围](../plans/active/comment-study-productization-001.md) |
| 源码依据、复用和模块边界 | [架构](../architecture/comment-study-productization-001.md) |
| 数据库表、列、哈希、限制传播 | [数据库合同](../data-contracts/comment-study-productization-001.md) |
| HTTP、事务、预算和三阶段执行 | [接口与执行合同](../data-contracts/comment-study-http-001.md) |
| 四个Tab、用户路径与状态 | [四视图规格](../design/pages/comment-study-productization-001.md) |
| 开发顺序、迁移、T01–T54、回退 | [开发与升级手册](comment-study-productization-001.md) |
| 下阶段日计划接口及额度 | [自动化衔接](../plans/active/comment-study-automation-002.md) |
| 每轮开发前后读什么 | [Agent执行入口](../agents/comment-study-productization-001-handoff.md) |

以模块源文件为准，不用重新阅读所有历史聊天。字段只在数据库/HTTP合同中维护；页面只引用。全部编译、数据库、真实浏览器、模型质量、发布证明必须分别报告，不得用文档检查替代。

## 每轮开发回看

1. 核对分支与main差异，复用已合并工作。
2. 打开本轮对应分册和T编号，确认没有增加手册外表/服务/执行通道。
3. 记录实际文件、执行命令、PASS/FAIL/NOT_RUN及依据。
4. 发生合同冲突，先记录与解决受影响部分，不静默改需求。

## 当前边界

保留已有Raw、Run、Target、Signal、Problem等全部合法历史；不执行reset，不改已应用migration。GitHub提交不等于本机应用已更新。完整文档入库是P0的来源固定部分，本机schema、在途任务和数据保护摘要还需要授权环境的实际只读证明。
