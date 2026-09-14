# COMMENT-RESEARCH-PUBLISH-001 · 覆盖可见的部分研究版本

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #264；Comment Research V1 的 ResultRevision 发布资格、读取合同与五视图中的部分研究表达
> 事实来源: Mog 2026-09-14 的开发期推进与派生层可重置授权、PAGE-COMMENT-RESEARCH-V1-001、当前运行 `af79c247-104e-4618-b12c-3b0d1dba6bdb` 的真实回执
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实运行/数据库/代码/数据合同

## 1. 用户结果

用户不应该因为一条评论或少数问题归并调用没有产出合格结构化结果，而看不到同一轮已经完成且可回溯的用户问题研究。

当一轮已经终态、来源仍可读，并且达到明确的研究覆盖与问题归并覆盖门槛时，系统发布一个 **PARTIAL** 的只读研究版本。页面必须说明：冻结了多少条、实际纳入多少条、未纳入多少条、未纳入的安全原因、问题归并覆盖多少 Atom，以及到运行记录的回溯路径。

这不是把失败伪装成成功：Run 保持 `completed_with_failures`，失败 Item 和失败调用仍保留在运行记录中；没有纳入的评论、Atom 和失败归并绝不进入问题统计、变化或证据计数。

## 2. 已核验触发与适用边界

真实 Run `af79c247-104e-4618-b12c-3b0d1dba6bdb` 已终态：

- 冻结评论 20；14 条提取到研究信号、5 条确认无研究信号、1 条语义 JSON schema 失败；
- 已接纳 Atom：problem 13、need 6、experience 10、solution 3；
- 当前 Problem membership：problem 11、need 6；两个 problem Atom 因归并 JSON schema 失败没有 membership；
- 现行规则仅接受 Run `completed`，所以没有 ResultRevision，概览、用户问题和变化观察没有可显示结果。

此卡不重置、不再发送模型调用、不回写原始评论。它以这轮已经取得的对象验证发布策略。

## 3. 发布资格合同

### 3.1 必须全部满足的硬条件

1. Run 已终态：`completed` 或 `completed_with_failures`；不能有 queued/running/retryable 的 RunItem 或未结算的 embedding / problem-resolution。
2. 冻结来源当前仍可读；来源不可读继续拒绝发布。
3. 每个已接纳的 `problem` / `need` Atom 要么存在 current Problem membership，要么被明确计为未组织 Atom；未组织 Atom 不写入统计。
4. 没有通过前端、SQL 或调用账本把失败项改成 succeeded / no_signal。

### 3.2 对 `completed_with_failures` 的门槛

只有同时达到下列门槛，才可发布 `PARTIAL` revision：

- **研究覆盖**：`(succeeded + no_signal) / selectedSources >= 90%`。
- **问题组织覆盖**：`current memberships / accepted (problem + need) atoms >= 85%`；若不存在 problem/need Atom，分母为 0 并按 100% 处理。

`completed` 也走相同完整性检查，但不需要在页面称为 PARTIAL。未达到任一门槛、存在未结算项目或 Run 已失败/取消时，仍没有可发布版本。

门槛故意只判定本轮“能安全代表多少被冻结输入”，不判定市场需求、趋势可靠性或模型质量。变化页仍沿用完整窗口、最小样本与比较条件；缺少可比覆盖就显示不可比，而不是推断趋势。

## 4. 冻结数据与 API 合同

无需 migration。每一份 `ResultRevision.input_counts` 写入以下冻结事实：

```json
{
  "selectedCommentCount": 20,
  "includedCommentCount": 19,
  "excludedTerminalCommentCount": 1,
  "researchCoverage": {"numerator": 19, "denominator": 20},
  "problemBearingAtomCount": 19,
  "organizedProblemAtomCount": 17,
  "unorganizedProblemAtomCount": 2,
  "problemOrganizationCoverage": {"numerator": 17, "denominator": 19},
  "publicationCoverage": "partial",
  "terminalFailureCounts": {"runItems": 1, "problemResolution": 2}
}
```

`includedCommentCount` 仅包含 `succeeded` 和 `no_signal`；问题与变化的评论分母继续只从这批实际被接纳的来源计算。`excludedTerminalCommentCount` 和未组织 Atom 不能暗中加入任何分母。

`input_counts` 是 ResultRevision 的冻结事实，读取 API 直接返回；它不是新的用户对象、运行时计数或覆盖世界全量的声明。

## 5. 页面与交互清单

### 表面地图

| 页面 | 用户需要理解 | 本次行为 |
|---|---|---|
| 概览 | 这一版是否完整、当前结论基于多少原声 | result meta 显示“本版研究覆盖 19 / 20；1 条未纳入”，链接/按钮进入运行记录 |
| 用户原声 | 原始证据当前状态 | 不依赖 ResultRevision；失败项仍是“模型研究未完成”，不被本次发布掩盖 |
| 用户问题 | 哪些问题来自已组织的 Atom | 显示与结果版本相同的 PARTIAL 覆盖说明；无 membership 的 Atom 不出现 |
| 变化观察 | 哪些结论满足既有可比条件 | 显示同一覆盖说明；不因 PARTIAL 创建额外趋势 |
| 运行记录 | 失败发生在哪里，影响是什么 | 已发布的 `completed_with_failures` 显示“已发布（部分覆盖）”，并保留逐项与调用失败原因 |

### 状态词典

| 状态 | 数据来源 | 用户应理解 | 禁止暗示 |
|---|---|---|---|
| 完整研究版本 | `publicationCoverage=complete` | 本轮冻结输入均有可用评论级结论、所有 problem/need Atom 已组织 | 模型或市场判断绝对正确 |
| 部分研究版本 | `publicationCoverage=partial` 且冻结覆盖事实 | 这版只覆盖明确数量的原声；失败项未纳入 | 全部评论已研究、失败被修复或趋势已证实 |
| 没有可发布版本 | 发布资格未满足 | 没有一版可作为研究结果读取 | 当前没有评论或没有讨论 |
| 运行失败 | Run/Item/Invocation 的安全 failure code | 哪一步未完成，且结果未发布或仅部分发布 | “没有发现” |

### LIDS 组合与范围

- L1 Corpus Explorer 的现有 shell、tabs、table、状态说明和读取区保持；不新增导航、仪表盘、组件或技术控制台。
- 使用已有 `--lgi-*` token、中文主表达、真实状态文本和已有运行记录 table；不改全局 token、shell 或非评论研究页。
- `PARTIAL` 是覆盖状态，使用 warning 语义与中文解释；不把它渲染为失败或成功徽章。
- 用户动作只有“查看运行记录”；不新增复跑、人工审核、手动选择、模型调用或数据重置动作。

## 6. 实施顺序与验收矩阵

1. 扩展发布器：锁定 terminal Run，计算/冻结覆盖事实，放行符合门槛的 `completed_with_failures`，并保留 Run state。
2. 扩展 worker 的待发布查询：可选择符合资格的 terminal failure Run，不创建新的模型调用。
3. 扩展 read projection/API：将 ResultRevision 覆盖事实作为 result envelope 的真实字段返回。
4. 更新页面的 result meta 与运行记录文案，明确部分覆盖和回溯入口。
5. 增加 PostgreSQL/worker/API/UI focused tests：完整发布、达标部分发布、不达标拒绝、未组织 Atom 排除、既有 Run 再发布无模型调用。
6. 使用真实 Run 验证新 revision、页面 API 和调用账本不增加。

| 层级 | 证明 |
|---|---|
| 任务可用 | 真实 `completed_with_failures` Run 可以在不新建调用的情况下发布可读 revision |
| 状态诚实 | API/UI 明示冻结、纳入、未纳入、组织覆盖和 failure counts；Run 状态不被改写 |
| 数据边界 | 未纳入评论和无 membership Atom 不进入问题、变化与分母 |
| 视觉一致 | 现有 L1 页面/Token 下的桌面浏览器走查，PARTIAL 与失败均有中文文本 |
| 未证明 | 单批不证明模型语义质量、市场趋势或连续自动排程可开启 |

## 7. 非目标与停止条件

不修改 raw corpus、capture、模型/向量配置、调用账本、Research Fingerprint、清洗、Prompt/Skill、自动排程或全局 UI。

若实现需要迁移一份重复的运行状态、把未组织 Atom 写进问题统计、将 `completed_with_failures` 改为 `completed`、读取或记录原始模型输出、或改写原始评论，立即停止该部分并报告。
