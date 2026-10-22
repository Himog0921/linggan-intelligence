# CORPUS-STUDY-CREATOR-UX-20261022 · UI 变更清单

> 状态: 一次性报告
> 最后核对: 2026-10-22
> 适用范围: Issue #433 的 `/corpus/comments` 与 `/corpus/creators` React UI 收口；不覆盖证据库、采集、研究执行或模型设置
> 事实来源: PR #470 精确 head、`PAGE-COMMENT-STUDY-PRODUCTIZATION-001`、`PAGE-CORPUS-CREATORS-V1-001`、LIDS、自动检查
> 冲突时以谁为准: 用户最新授权、AGENTS.md、真实运行/代码/合同

## 结果与范围

分类为展示、交互与状态语义；最高风险是筛选草稿和未知观测状态的表达。评论概览默认观察 56 天，图表与表格在同一高度容器中阅读，`none` 覆盖仍显示为未知而非零；评论目录使用现有 LIDS 筛选/选择控件，所有筛选（含已匹配作品）仅在“应用筛选”后提交。创作者页保留八列表格与现有读取/动作合同，低频 AI 说明移入上下文和详情，不再常驻占用列表空间。

非目标：Rust、SQL、API、数据口径、研究资格/上限、模型、worker、采集、真实写入和证据库路径均不改。

## 表面、状态、依赖和验收

| 表面 | 状态与来源 | 验收 |
|---|---|---|
| 评论概览 | `observationSeries` 的 56 天读取；缺记录保持 `—` | `ObservationSeries.test.tsx` 覆盖默认窗口与缺失值 |
| 评论目录 | URL 提交态与本地 `draft` 分离；服务端继续判定研究资格 | `CommentsPage.test.tsx` 覆盖应用前不读目录、应用后携带 `workRef` |
| 评论选择 | 现有 canonical selected set、跨页选择及 3000 条/100 作品上限 | `CommentsPage.test.tsx` 与既有选择测试 |
| 创作者目录 | 现有创作者读取投影和八列合同 | `CreatorsPage.test.tsx`、`CreatorWrites.test.tsx` |

已运行的自动证明：`npm --prefix apps/web run verify`（类型、lint、1107 项 Vitest、LIDS 与 build），本 PR 修订后另运行评论定向 35 项、typecheck、lint、LIDS sync/check 与 `git diff --check`。真实页面浏览器走查、合并和部署在本记录生成时尚未发生，必须在发布回执中单独陈述。

## 依赖与停止边界

依赖 `FindField`、`SelectField`、`ActionButton`、现有评论/创作者读取合同和 LIDS token。若发现需要修改研究资格、任何写命令、API/数据库、模型或采集状态，即停止此 UI 包；本次无此需求。视觉只使用既有 token，选择框使用既有 1px 控件边界，不引入 1.5px 边框例外。
