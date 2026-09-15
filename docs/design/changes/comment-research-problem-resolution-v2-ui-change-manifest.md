# COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2 · UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: Issue #285 对 `/corpus/comments` 概览、用户问题、运行记录与只读详情的状态表达
> 事实来源: PAGE-COMMENT-RESEARCH-V1-001、DEC-0004、COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、数据合同、真实读取/API 证据

## 事项与读取回执

- Issue / agent / worktree: #285 / `/root` / `codex/comment-research-problem-resolution-v2`
- 用户结果: 看见已形成 Problem 与尚待归并的合法信号，且能追溯决定依据。
- 非目标: 新页面、手工审批流、真实采集/模型调用、统计门槛修改、运行时部署。

| 来源 | 已核对的作用 |
|---|---|
| `AGENTS.md`、`docs/current-state.md`、Issue #285 | protected delivery、真实副作用和协作边界 |
| UI execution contract | 表面/状态/依赖/验收矩阵与停工规则 |
| PAGE-COMMENT-RESEARCH-V1-001 | 五视图职责、累计事实与统计版本分责 |
| DEC-0003 / DEC-0004 | 唯一语义内核、Problem 与 Atom 的责任边界 |
| LIDS Token/Primitive/Pattern/Data Boundary/Language/Agent guide | L1 shell、文字 Tab、表格/抽屉、未知与中文主表达 |
| 当前 worker/read/API/tests | 现有 direct-create 合同和可复用读取面 |

## 分类与表面

- 分类: 状态/语义；最高风险是把等待、失败、未知和 membership 混写。
- Pattern: 既有 L1 Corpus 评论研究工作区；不新建 CMP、Token、Scene 或 Motion。
- Data truth: confirmed membership、legal deferred、execution failure 和未评估各自使用独立字段/文案；`0` 仅表示已确认零。
- 用户问题页新增同一只读查询的 `全部 / 已形成问题 / 待归并信号` 筛选；详情顺序固定为受限原声说明与归一描述、frame 及出处、候选比较、结论、再评估条件、执行历史。

## 2026-09-15 · Run Health 状态细化

运行记录不得再将所有“未建立 membership 的 problem/need Atom”写成“待归并”。Run Health 以互斥执行事实表达：

- `待开始归并`：已提取为 problem/need，但尚无 resolution；
- `归并处理中`：已有 pending/running/retryable resolution；
- `等待独立同类证据`、`等待消歧`、`等待必要语境`：对应已成功写入的 V2 deferred disposition；
- `归并终态失败`：resolution 的 model_failed/incompatible，不能伪装为 deferred；
- `旧合同已跳过`：本 Run 曾错误接管 V1 backlog，现已恢复到原始历史；它不计入本 Run 的失败或待处理数。

此变更只复用现有运行记录文字摘要与数字 readout，不增加页面、Token、组件或写入动作。

## 停止与验收

不展示模型原文、原始敏感材料、内部 UUID 或无中文含义的状态码；不在打开抽屉时调用模型。若现有页面规格不足以裁定新的筛选/抽屉互动，则停止该 UI 部分并报告。

| 层级 | 验收方法 | 当前结果 |
|---|---|---|
| 任务可用 | API/read DTO、静态页面与脚本语法检查 | 已实现；隔离 PostgreSQL 全套回归运行中 |
| 状态诚实 | deferred 与 confirmed 的隔离 PostgreSQL 正反例 | 已新增；全套隔离 PostgreSQL 回归运行中 |
| 视觉一致 | LIDS/现有 CSS 合规检查与窄宽度走查 | 已复用 L1 table/filter/drawer；浏览器窄宽度走查待运行时授权 |
| 真实后果 | 仅隔离 PostgreSQL；不触及 runtime | NOT VERIFIED |

完成时同步页面规格、设计索引与月度记录；不借本清单取得 merge、部署或真实数据权限。
