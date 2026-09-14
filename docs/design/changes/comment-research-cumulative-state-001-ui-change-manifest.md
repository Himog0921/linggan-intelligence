# COMMENT-RESEARCH-CUMULATIVE-STATE-001 · 评论研究累计状态 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-14
> 适用范围: Issue #281；`/corpus/comments` 的概览、用户问题和运行记录
> 事实来源: Mog 2026-09-14 的累计知识与统计资格分离决定、PAGE-COMMENT-RESEARCH-V1-001、COMMENT-RESEARCH-CUMULATIVE-STATE-001
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实数据合同与运行代码

## 1. 事项

- Issue / SCOPE: #281 / COMMENT-RESEARCH-CUMULATIVE-STATE-001
- Agent 与 worktree: `/root` / `/Users/moglenny/proma/linggan-intelligence/.worktrees/comment-research-cumulative-state-001`
- 目标: 在既有评论研究五视图中，将累计已确认知识与单轮统计资格分开呈现。
- 用户可见结果: 低覆盖 Run 的已确认 membership 立即可见；本轮失败/待归并仍可见；比例、排名、趋势只来自合格统计版本。
- 明确非目标: 新页面、全局 Token/壳层、自动排程、人工审核动作、页面读取时的模型调用或低覆盖趋势。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / current-state | 已读 | 受保护交付、文档留痕、运行边界 | 是 |
| UI execution contract | 已读 | 表面/状态/依赖/验收矩阵 | 是 |
| PAGE-COMMENT-RESEARCH-V1-001 | 将被本事项更新 | 五视图职责与已发布门控冲突 | 是 |
| LIDS PAT-001 / BOUND-001 / LANG-001 | 已读 | L1 Corpus Explorer、PARTIAL/UNKNOWN/0 与中文状态 | 是 |
| 现有 read/result/worker 合同 | 已读 | membership 已即时写入但前端只读 ResultRevision | 是 |

## 3. 变更分类

- 分类: 状态语义 + 展示。
- 最高风险类别: 状态语义。
- 依据: Mog 的明确决定；累计 membership 与 ResultRevision 的现有数据边界。
- DECISION_REQUIRED: 无；自动排程与无界重试明确不在范围。
- L1 / 主 Pattern: L1 Corpus Explorer；既有五视图 Tab + 连续表格/分页。
- Data Truth: `PARTIAL + VALID`、待归并、失败、0 与未知严格分开；不新增 Token、CMP、Scene 或 Motion。

## 4. 表面和状态词典

| 状态 | 来源 | 用户应理解 | 禁止暗示 |
|---|---|---|---|
| 累计已确认 | current membership | 已通过全部接纳合同的证据已计入问题库 | 本轮完整或趋势成立 |
| 本轮部分完成 | Run Health 有成功与 backlog/失败 | 已确认结果已更新；其余等待处理 | 研究白跑或全部失败 |
| 待归并 / 终态失败 | 无 membership 的 resolution backlog 与安全失败原因 | 尚未形成稳定问题，不计入统计；符合止损条件的历史 backlog 只在下一次已启动 Run 中续办，终态 unresolved 不自动消耗 | 没有信号、计为 0 证据或页面正在重试 |
| 无统计版本 | 无符合资格 ResultRevision | 不可读分布/趋势结论 | 没有评论、没有累计问题 |
| 已发布统计版本 | ResultRevision | 覆盖事实限定下的分布/变化结论 | 当前所有新证据已纳入 |

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 隔离 API/UI fixture：2 个 membership 在低覆盖 Run 后可读 | 待实施 | 真实日常运行 |
| 状态诚实 | 常用、部分、失败、零、未知字符串/API 断言 | 待实施 | 真实模型质量 |
| 视觉一致 | 既有 L1 表格、Tab、分页与中文状态检查 | 待实施 | 3000 人工浏览器验收 |
| 真实后果 | 不调用 provider 的隔离 PostgreSQL/API proof：新 Run 可接管符合条件的历史 backlog，成功后补入原 Run 的累计知识；B→C 续办保留 B 的独立失败历史 | 候选源码与隔离证明通过；3000 人工验收未做 | 部署与自动排程 |

## 6. 交接

- 修改文件: comment research read/API/page 及其 focused tests；本计划、页面规格、当月进度和索引。
- 验证: isolated PostgreSQL/API、Rust/JS/format/governance；不部署。
- 例外: 不调整全局 shell 或 Token；旧 `PARTIAL` 统计版本保留且降级为“统计版本”。
- PR / reviewer / integration owner: PR 交付；独立 reviewer 依当前仓库受保护交付要求，合并由 Mog 另行授权。
