# COMMENT-RESEARCH-CUMULATIVE-STATE-001 · 累计确认问题与运行完整度分离

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #281；评论研究的累计 Problem 读取、Run Health、未归并 Atom backlog、跨 Run 有界续办与页面状态表达
> 事实来源: Mog 2026-09-14 的明确产品决定、现有 current membership/ResultRevision 代码、PAGE-COMMENT-RESEARCH-V1-001
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实数据库/代码、数据合同与可复现测试

## 1. 用户结果

评论研究不再把“单轮是否达到统计发布覆盖”作为全部知识可见性的总开关。

一条 `problem`/`need` Atom 只要通过输出、候选绑定、Problem 定义与 membership 接纳合同，就立刻成为累计问题库的有效证据；页面可显示其绝对计数、最近确认时间和新出现问题。它不等待同一 Run 的其他 Atom 成功，也不改写 Raw Comment、Atom 或 membership 的不可变事实。

Run 仍是一次冻结研究的健康与完整度记录。只有达标 ResultRevision 才能支撑占比、排行、窗口比较、趋势与高阶 Intelligence；低覆盖 Run 只能说明已经观察到的绝对事实和未完成边界，不能声称代表完整分布。

## 2. 已确认逻辑

```text
可读评论 → 语义 Atom → 有效 membership → 累计问题库（立即可读）
                             ↘
冻结 Run → 完成率 / 归并率 / 失败 / backlog → 统计资格（独立判断）
```

| 层级 | 写入资格 | 前端用途 | 禁止用途 |
|---|---|---|---|
| Evidence / Atom | 既有严格语义与证据合同 | 原声与研究状态 | 伪装为稳定问题 |
| Current membership | 既有候选绑定、JSON、Problem 定义与幂等接纳 | 累计问题、累计证据、最近确认、新出现 | 占比、排名、趋势 |
| Run Health | 冻结 Run 和 resolution 状态 | 完成数、待归并、失败原因、覆盖率 | 否定已确认 membership |
| ResultRevision | 既有完整/部分覆盖与可比性门 | 分布、排行、变化、趋势 | 代替累计当前状态 |

`no_signal` 是完成的评论级判断，不是失败；`0` 个已确认 membership 是已确认零，`UNKNOWN`/未读取与待归并均不得显示为零。

## 3. 实施范围

1. 新增累计 Problem read projection：只读 current membership 与仍可读来源，返回累计评论/作品/Atom 数、最近确认时间及稳定定义；不计算 share、rank、环比或趋势。
2. 将概览与用户问题切换到累计 confirmed state；变化观察继续只读 published ResultRevision。既有 ResultRevision 元信息在页面保留为“统计版本”，不得冒充当前累计状态。
3. Run API 返回每轮的评论理解、信号、problem/need、已确认 membership、待归并、归并覆盖率与安全失败分类；UI 表示为“研究部分完成”而非笼统“未发布”。
4. 将跨 Run 未归并 Atom 作为可审计 backlog 读取：保留单 Atom 的唯一 live resolution，以及 `Atom × execution Run` 的不可改写执行历史、失败原因、最后尝试时间与终态；同一 Atom 永远只产生一个 current membership。
5. 每次已获授权的研究启动，冻结“本轮新增 Derivation + 符合止损规则的历史 resolution backlog”。这不是常驻 worker 的空转重发，也不需要用户每天重新授权：用户启用自动研究或明确启动研究，已经授权当前研究范围内未完成资产在后续 Run 继续处理。每次续办仍由新的 Run、当前 policy、预算账本和最多 40 条 backlog 上限约束。
6. 续办 eligibility 固定为：已知 provider/网络瞬时失败可在后续 Run 再试；`problem_resolution_admission_rejected` 仅在累计尝试少于 3 次时续办；JSON/Schema/输出合同失败只在 membership contract 或模型配置改变后续办。达到语义上限或仍不兼容的 Atom 保留为终态 unresolved 证据，不被每日自动消耗。
7. 以隔离 PostgreSQL/API/UI 验证 2/26 这类低覆盖 Run：2 个有效 membership 可见，24 个 backlog 可见，正式统计仍不可读或保持最近可读 ResultRevision。

## 4. 表面、状态、依赖与验收矩阵

| 表面 | 常用/部分/失败状态 | 数据来源 | 自动验证 |
|---|---|---|---|
| 概览 | 累计已确认问题；统计版本存在/不存在/过期 | cumulative membership + optional ResultRevision | read/API/UI fixtures |
| 用户问题 | 新出现、低证据、零条、累计证据 | current membership | PostgreSQL/API/page pagination |
| 变化观察 | 仅正式统计版本；覆盖不足不可比 | ResultRevision | 既有变化回归 |
| 运行记录 | 完成、部分完成、待归并、失败、覆盖 | Run/Item/Atom/resolution | SQL projection/UI copy |
| worker backlog | pending/retryable/终态失败、跨 Run 接管、重复接纳 | resolution + membership + execution Run | PostgreSQL/worker idempotency |

## 5. 非目标与停止条件

- 不开启连续自动排程，不在本事项中调用 provider、重置研究派生、修改 Raw Comment、配置模型或 embedding。
- 不降低 JSON、引用坐标、候选集合或 membership 接纳合同。
- 不让未归并 Atom 进入任何 share、rank、trend、change 或 ResultRevision 分母；续办 Run 本身不生成统计 ResultRevision，历史 Atom 成功后只补足其原冻结 Run 的组织覆盖。
- 不修改历史 published ResultRevision。

若必须把未验证输出写进当前 membership、将低覆盖 Run 作为统计快照、无上限地重新调用 provider，或发现共享页面壳有并行所有权冲突，停止受影响部分并报告。

## 6. 验证与未证明边界

- 必跑：focused Rust/worker、isolated PostgreSQL/API 评论研究 suite、页面 JS 测试、格式/治理检查。
- 要求覆盖：current membership 幂等、低覆盖 Run 的累计可见性、未归并不进统计、零/未知/部分文案。
- NOT VERIFIED：真实 provider 的语义质量、实际每日自动排程、共享数据库 migration、3000 runtime、浏览器人工验收与业务收益；均需后续明确授权。
