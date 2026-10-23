# Agent 任务追踪：GitHub Issues

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: GitHub 工作单、Claim、PR 和交接操作
> 事实来源: 当前仓库与 Mog 确认的工作包协作改革
> 冲突时以谁为准: 用户最新授权、AGENTS.md；流程以协作协议为准

交付分级、协作权限、审阅频率、合并与文档更新只由[协作协议](../governance/agent-collaboration.md)定义。此文件只提供 GitHub 操作约定，不额外增加审批门。

仓库为 `Himog0921/linggan-intelligence`。Issue 记录责任、依赖和证据链接，不是产品/领域事实或生产权限的第二来源。任务明确时只读相关 Issue，不批量加载所有历史任务。

## 工作包与 Claim

快车道可以只使用 PR 正文作为工作包摘要，免建 Issue/Claim。保护车道关联已有 Issue 或用户已确认的交付包标识；不为了新建卡片重复请求实施授权。

保护车道的 Claim 记录一次：执行者/task-id、base commit、分支/worktree、exclusive/shared/forbidden 范围、依赖和升级条件。有已授权 Issue 写入范围时记到该 Issue；否则写到现有包文档或 PR 正文，不额外发送消息。协调者维护共享文件所有权和包内分工。

新增直接依赖按协作协议记录后处理，不把初始文件清单当成遗漏修复的借口；跨包冲突和新权限仍按协议升级。

## Issue 操作

- 创建工作单时写授权、目标、边界、依赖、验收及停止条件；实际提交/验证结果在发生后补。
- `ready-for-agent` 表示信息足以执行；由已授权协调者核对范围和依赖后标记，不表示生产或敏感数据权限。
- 稳定 task-id 区分共享 GitHub 账号下的执行者；仅标签、assignee 或关闭不能证明交付完成。
- 标签说明见[triage 标签](triage-labels.md)。没有额外 `in-progress` 标签。
- 已授权多阶段包可使用一个总 Issue 与任务清单；只在确有独立责任/阻塞时建立子 Issue。
- 外部 Issue 创建、评论和关闭遵循用户明确授权，不因本指南列出命令就自动执行。

```bash
gh issue view <number> --repo Himog0921/linggan-intelligence --comments
gh issue list --repo Himog0921/linggan-intelligence --state open --limit 20
```

写入正文使用工具结构化参数或临时正文文件加 `--body-file`，保留真实换行，避免 shell 插值。长期决定更新相应权威文档，Issue 只引用。

## PR 与交接

PR 使用仓库模板，描述问题、最终行为、风险分级、相关验证及未证明层。快车道的 Issue 可以写“不适用”；保护车道链接主 Issue 或已确认包标识。一个包可分阶段 PR，但共同授权和规格只维护一份。

有 Issue 时使用 `Refs #...`；合并后核对 main、真实副作用和缺口，再按已授权范围关闭。PR/Issue 关闭不表示数据库已迁移、插件已发布、服务已部署或 Mog 已验收。

独立审阅与最终 head 的复核按协作协议执行。仓库缺少强制分支保护时，由协调者核对最终 CI、审阅、冲突和合并授权，不能用模板勾选替代证据。保护车道仍需 Mog 对最终 head 的授权。

## 渐进读取

先读 AGENTS 与工作包摘要，再读直接引用的合同/计划/测试。只有运行身份、发布或跨包依赖需要时才读 current-state；历史来源核对需要时才进入 references。
