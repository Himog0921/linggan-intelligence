# Agent Triage 标签词表

> 状态: 权威当前
> 最后核对: 2026-08-20
> 适用范围: GitHub Issues 的 Agent 任务分流与下一步责任表达
> 事实来源: `mattpocock/skills` 默认角色、当前 GitHub 标签和用户确认
> 冲突时以谁为准: `AGENTS.md`、当前事项授权、GitHub Issue 实际状态和用户最新确认

本文件把 Matt Pocock 工程技能使用的五个 triage 角色映射到 Linggan Intelligence 的 GitHub 标签。

| Matt 技能角色 | GitHub 标签 | 在 Linggan Intelligence 中的含义 |
|---|---|---|
| `needs-triage` | `needs-triage` | 尚未完成初步判断，需要确认问题性质、归属、优先级和下一步 |
| `needs-info` | `needs-info` | 当前信息不足，正在等待需求、来源、复现、产品选择或外部条件 |
| `ready-for-agent` | `ready-for-agent` | 当前范围、授权、依赖和可证伪验收已经足够清楚，可以交给 Agent 执行 |
| `ready-for-human` | `ready-for-human` | 必须由人完成产品决定、凭据操作、现实协调、敏感授权或其他不能委托的事项 |
| `wontfix` | `wontfix` | 已明确决定不实施；应保留原因和影响，而不是静默删除 |

## 标签只表达下一步责任

Triage 标签不替代以下项目状态：

```text
待讨论
需要决定
已确认
执行中
验证中
已完成
SOURCE_INCOMPLETE
DECISION_REQUIRED
BLOCKED
```

例如：

- `needs-info` 不等于现实中没有数据；
- `ready-for-agent` 不等于已经授予生产、插件或敏感数据权限；
- `ready-for-human` 不等于技术工作全部完成；
- `wontfix` 不等于历史讨论可以删除；
- Issue 被关闭不等于数据库、插件、部署或业务链路已经通过验收。

## 与任务类型标签共同使用

GitHub 默认标签描述“这是什么”，例如 `bug`、`enhancement`、`documentation`、`question`；Triage 标签描述“下一步由谁处理”。一个 Issue 可以同时拥有一种任务类型和一种 triage 角色，例如：

```text
enhancement + ready-for-agent
bug + needs-info
documentation + ready-for-human
```

正常情况下，一个 Issue 同时只保留一个当前 triage 角色，避免出现相互矛盾的责任状态。

## `ready-for-agent` 的最低条件

只有同时满足以下条件，才可以使用 `ready-for-agent`：

1. 目标和范围已经明确；
2. 明确写出非目标；
3. 必要的用户决定已经完成；
4. 没有阻塞该范围的 `DECISION_REQUIRED`；
5. 来源不足不会迫使 Agent 猜测；
6. 明确可运行的验证方法；
7. 需要的访问和修改权限已经在当前范围内成立；
8. 不会因为完成本任务而自动扩大真实采集、敏感数据、生产或现实行动权限。

## `ready-for-human` 的适用情况

包括但不限于产品方向选择、真实平台账号操作、Cookie/Token/密钥或生产凭据提供、法律/隐私或数据保留决定、外部供应商合同、现实内容发布、生产删除或不可逆操作，以及 Agent 无权替代的业务验收。

Agent 应尽量把需要人的问题整理成一个可判断的决策包，而不是把技术排查责任转给用户。

## 状态变更留痕

改变 triage 标签时，应在 Issue 中说明改变原因、新信息、下一步责任、是否影响正式计划或当前 SCOPE，以及是否需要同步项目文档。标签历史可以帮助追踪任务流转，但不能替代正式决策记录。
