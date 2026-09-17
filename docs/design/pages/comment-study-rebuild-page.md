# PAGE-COMMENT-STUDY-REBUILD-001 · 评论研究

> 状态: 权威当前
> 最后核对: 2026-09-17
> 适用范围: `/corpus/comments` 的 COMMENT-STUDY-REBUILD-001 页面与只读投影
> 事实来源: DEC-0006、COMMENT-RESEARCH-REBUILD-001、UI 变更清单、LIDS
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/数据合同和当前代码

## 1. 身份与授权

- LIDS 视觉强度 / 主 Pattern: L1 / Corpus Explorer。
- 用户任务: 看清指定 ADHD 作品的评论研究实际处理到哪里、哪些表达仍待归并、哪些已经成为有边界的稳定 Problem。
- 三秒答案: 这轮评论研究处理了什么，当前在哪一步。
- 五秒主动作: 切换读取视图；未来受控 action 才是开始一轮研究。
- 明确非目标: 不在此页编辑 raw comment、创建 Problem、发起采集、调用模型、重置或发布统计结果。
- 可用数据合同: `linggan_comment_study_*`、raw Evidence 的 current qualification、generic invocation ledger；不读取旧 V1/V2/V3 research relations。

## 2. 页面边界

- 入口: `/corpus/comments`；退出: Corpus 其它表面。
- 核心任务: 只读地追踪新评论研究的输入、处理中状态、Signal 与 Problem 归并事实。
- 责任分界: 本页只显示后端已写入的 receipt 和研究事实；模型配置属于模型设置，真实执行属于 worker，raw comment 仍属 Evidence。

## 3. 状态与行动

| 状态 ID | 触发/数据来源 | 用户应理解什么 | 禁止暗示什么 | 可用下一步 |
|---|---|---|---|---|
| `NO_RUN` | 无 StudyRun | 尚未开始本轮研究 | 评论不存在或本轮失败 | 未来受控的开始研究 |
| `RUNNING` | queued/running target 或 prepared/leased batch | 工作已创建，尚无最终结果 | 已有 Signal/Problem | 查看运行记录 |
| `NO_SIGNAL` | target state `no_signal` | 模型明确未发现可接纳 Signal | 缺输出/失败 | 查看原声 |
| `DEFERRED` | Signal resolution state | 信号存在，但当前不能安全归并 | 已建 Problem | 看等待条件 |
| `PROBLEM_LINKED` | membership | 已依据闭集比较关联稳定问题 | 规模、趋势或普遍性 | 查看定义和 evidence count |
| `PARTIAL` | accepted 与 retry/failed 同时存在 | 有效结果保留，其他项未完成 | 全量完成 | 查看运行记录 |
| `SOURCE_RESTRICTED` | target excluded / batch cancelled | 当前来源不可再处理 | 已外发或已安全完成 | 查看安全原因 |

## 4. 已批准组合

| 区域 | 来源 | 用途 | 禁止替代 |
|---|---|---|---|
| shell + corpus side nav | LIDS-SHELL-001 | 统一入口和范围 | 单页覆盖 shell 样式 |
| tabs | LIDS-PRI-001 ADR-11 | 概览、评论目标、待归并、用户问题、运行记录 | 分段控件/重复页面 header |
| 连续表格 | LIDS-PAT-001 Corpus Explorer | 20+ 同构目标、Signal/Problem 与执行记录 | 卡片瀑布流 |
| 原声列 | LIDS-PRI-001 | Serif 呈现用户评论原文 | 将系统描述伪装为原声 |
| 状态/空态 | LIDS-BOUND-001 + LIDS-LANG-001 | 中文、Known(0)/Unknown/Partial 分开 | `暂无数据` 万能空态或英文描述标签 |

## 5. 真实后果

当前切换视图、翻页和展开详情均只能调用只读本地 API；API 返回成功不等于模型完成。开始研究、模型配置、reset 和任何对外调用不属于本页本阶段动作；未来若接入，必须单独记录预条件、服务端回执、处理中、部分、失败和完成。

## 6. 验收边界

- 自动: 新 API fixture 对每个状态返回独立 JSON，并断言只读请求不新增 invocation 或修改 Evidence。
- 页面: 每个状态有中文主表达；原声、系统文本和技术状态不混淆；Unknown 不是 0 或失败。
- 人工: 新页面在桌面与窄屏确认表头、分页、状态与焦点；尚未执行前一律 NOT VERIFIED。
- 不证明: 真实模型质量、真实研究完成、共享运行库、3000 runtime、提交/合并/部署。
