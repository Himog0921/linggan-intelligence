# 评论研究 B-04 落地：前后端接口约定（新增字段全部是"加法"）

所有新增字段都是**可选的增量**：旧客户端忽略即可；新前端在字段缺失时必须优雅降级（显示「—」或隐藏该块），不能报解码错误。
不新增迁移（最新迁移是 0239，属于 #643）。所有读取遵守现有来源资格/脱敏规则，与同一端点已有字段的过滤口径一致。

根路径：`/api/local/comment-research/v2`（研究）与现有评论列表端点（`.../comments?...`，见 `apps/web/src/pages/corpus/comments/commentDtos.ts`）。

## 1. `GET /runs` —— 每个 run 增加
```json
"targetCount": 3,          // 本批次评论数
"workCount": 2,            // 涉及作品数
"unitCounts": { "completed": 1, "failed": 1, "pending": 0 },   // pending = pending + running
"outcomes": { "signals": 2, "needsContext": 0, "noSignal": 0 } // 仅统计本批次已完成单元写入的结果
```
前端用途：研究记录对话框的「任务规模 / 处理情况 / 结果摘要」三列。缺失时这三列显示「—」，展开后仍可用 `GET /runs/{batch}` 的单元明细。

## 2. `GET /problems` —— 每个 problem 增加
```json
"summary": "孩子知道需要完成作业，但难以从意图转入行动……"   // 一句话描述，可为 null
```
来源优先级：已存的问题描述字段 → 否则由 goal / barrier 组合成一句（服务端决定措辞，前端不拼接）。缺失或 null 时卡片不显示描述行。

## 3. `GET /overview` —— 增加两块
```json
"researchBreakdown": {            // 四段加起来 = corpusSummary.eligibleCommentCount
  "withResult": 16,               // 当前有有效结果
  "running": 2,                   // 研究中且尚无有效结果
  "failedWithoutResult": 1,       // 最近一次失败且尚无有效结果
  "never": 5                      // 从未进入研究
},
"recentSignals": [                // 最多 5 条，按写入时间倒序
  { "requestRef": "…", "proposition": "…", "kind": "scenario|emotion|solution|experience|…",
    "authorDisplayName": "…|null", "acceptedAt": "RFC3339",
    "commentKey": { "workRef": "…", "commentExternalId": "…" } }
]
```
前端用途：概览「已进入研究」分段条 + 图例（有结果 / 研究中 / 最近失败 / 未研究），以及概览下半区「近期研究信号」。缺失时分段条退回单段（已有结果 / 可研究），近期信号块显示空态。

## 4. 评论列表 —— 数字分页
请求新增 `offset`（整数，默认 0；与 `limit` 一起用；与现有 `cursor` 互斥，传了 offset 就忽略 cursor）。
响应 `page` 增加 `totalCount`（当前筛选下的总条数，整数；算不出时为 null）。
前端用途：表格底部「第 1–10 条 · 共 24 条」「‹ 1 2 3 ›」「跳至 N 页」「每页 10/20/50 条」。`totalCount` 为 null 时退回「上一页 / 下一页」（cursor）模式。

## 不在本轮
- 词云的短语级抽取：沿用现有 `word-cloud` 接口，前端只改样式（墨色、不再标红首位、字重 ≤600）。
