# COMMENT-RESEARCH-RESET-001 · UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-12
> 适用范围: 评论研究与模型设置的用户界面替换
> 事实来源: PAGE-COMMENT-RESEARCH-V1-001、DEC-0003、用户测试反馈
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行证据

## V1-REAL-CLOSURE-001 补充（2026-09-14）

“准备本轮研究”继续只由服务端选择范围，用户不再跨页勾选或凑批。确认 modal 必须把
`尚未进入研究` 和 `可恢复` 分开：后者表示旧 Run 没有形成有效结论，按当前研究输入可由
系统重新纳入；它不能再被写成“不会重新加入本轮”。已提取信号、无信号、执行中和当前合同
已拒绝的项仍需各自如实说明。

这只是现有 modal 的数据诚实性修正：不改变 LIDS token、壳层、按钮层级、tab、任何全局
组件或其它页面，也不把研究指纹、队列、向量或模型内部状态暴露为用户对象。

## COMMENT-RESEARCH-PUBLISH-001 补充（2026-09-14）

### 1. 事项

- Issue / SCOPE：Issue #264 / COMMENT-RESEARCH-PUBLISH-001
- Agent 与 worktree：`/root` / `codex/comment-research-publish-001`
- 目标：当达到冻结覆盖与问题组织覆盖门槛时，让 `completed_with_failures` 的真实研究结果以 PARTIAL ResultRevision 可读；失败继续可见、不可计入。
- 用户可见结果：概览、问题、变化和运行记录用中文说明“本版覆盖多少冻结原声、哪些未纳入、原因在哪里”，并提供回到运行记录的路径。
- 明确非目标：不新增页面/导航/人工审核/复跑操作；不改 raw corpus、模型、向量、排程或全局 LIDS token。

### 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / current-state | 已读 | 受保护交付、真实状态与文档留痕边界 | 2026-09-14 |
| UI execution contract | 已读 | 表面、状态、依赖和验收矩阵 | 2026-09-14 |
| PAGE-COMMENT-RESEARCH-V1-001 | 已读并更新 | 五视图职责与运行失败表达 | 2026-09-14 |
| LIDS Token / Primitive / Pattern / Data Truth / Language | 已读 | 既有 L1 Corpus Explorer 的部分状态表达 | 2026-09-14 |
| COMMENT-RESEARCH-PUBLISH-001 数据合同 | 本清单与 active plan 冻结 | 覆盖事实、分母和不可计入边界 | 2026-09-14 |
| 当前代码/真实 Run | 已读 | `af79…` 的 20 条冻结输入与现行未发布原因 | 2026-09-14 |

### 3. 分类、边界与验收

- 分类：混合（状态语义 + 展示）；最高风险：状态语义。
- L1 / Pattern：L1 Corpus Explorer；复用 title/status、tab、table 与 empty pattern。
- 受影响页面：`/corpus/comments` 的 overview/problems/changes/runs；voices 保持独立读取。
- 状态：完整结果、PARTIAL 结果、无结果、运行失败；PARTIAL 不是 INVALID，也不是完整覆盖。
- 依赖：ResultRevision `input_counts`、read envelope、现有 run receipt；不触及 shared shell 或 token。
- 停止条件：若必须改写 Run state、失败 Item、原始评论、调用账本或引入未批准视觉组件，停止。

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | PostgreSQL/API 与真实终态 Run 发布 | 待实施 | 不证明长期自动研究 |
| 状态诚实 | 分母/失败/未组织 Atom 回归断言与页面文本 | 待实施 | 不证明模型质量 |
| 视觉一致 | HTML/JS 断言与 localhost 浏览器走查 | 待实施 | 不做全站 token 迁移 |
| 真实后果 | 账本调用数不变、ResultRevision 可读取 | 待实施 | 不重发模型请求 |

## 变更原因

用户测试确认旧页面存在全量超级查询导致卡顿、tab 语义重复、作者徽标进入研究文本、重复授权，以及把“语义向量准备”暴露给普通用户的问题。旧页面和旧自动计划不能保留为迁移期 fallback。

## 表面地图与替代

| 旧表面 | 处理 | V1 替代 |
|---|---|---|
| `/api/local/comment-intelligence` 全量快照 | 删除 | overview / voices / problems / changes / runs 五个独立只读 endpoint |
| 每日观察、查询、资产、标注、Task B/P4 设置 | 删除 | 一个保存 policy 的研究设置 modal |
| 旧模型设置中的 trial/backfill/automatic plan | 删除 | 连接、V1 JSON 测试、默认模型、向量配置和调用账本 |
| “等待语义向量准备”原声标签 | 删除 | 原声独立读取 canonical V1 的 current、readable、`ordinary_user + eligible` 证据、研究正文与受控中文最新研究状态；它不等待 ResultRevision，也不显示评论作者名、Atom 枚举或向量实现状态 |

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
| 没有 published ResultRevision 时仍可浏览当前原声 | isolated PostgreSQL `read_voices` / HTTP proof：canonical V1 version filter、200、分页、无作者名/Atom 枚举、无模型调用；overview/problems/changes 仍返回 result unavailable |
| 设置后直接开始研究 | HTTP mutation test；模型调用数断言 |
| 运行时显示新页面 | exact-head runtime switch 与浏览器截图 |
