# COMMENT-RESEARCH-001 UI 变更清单

> 状态: 历史归档
> 最后核对: 2026-09-07
> 适用范围: Issue #167 评论研究受控源码交付
> 事实来源: Mog 当前派定、PAGE-COMMENT-RESEARCH-001、0039 migration 与 API
> 冲突时以谁为准: 用户最新确认、真实来源/权限合同与 LIDS

> 替代说明（2026-09-09）：本文件保留 Issue #167 的 UI 实施边界与历史验收；后续页面的用户任务、状态和读取模型以 [COMMENT-RESEARCH-RESET-001](../../plans/active/comment-research-reset-001.md) 为准。

- 来源：用户确定语料库为基石，评论取得即研究；旧工作台只继承精确语料选择和研究语义，未复制导航、Prisma、样式或聚类阈值。
- 表面：新增 `/corpus/comments` 四视图、`/corpus/queries` 评论查询入口；Evidence 侧栏只把对应两个入口从禁用变为真实链接。创作者继续未接通。
- 数据：页面消费 Evidence 读取接缝、Work Resource owner、Intelligence 选择/分析记录；没有原型样例进入生产页面。
- 交互：查询、分页、源详情、精确片段收存、集合创建/筛选、人工标注修订。资产整理 dialog 支持修改理由、加入/移出/移动单集合、撤销误收存和查看修订；查询 dialog 支持改名、条件修改和删除。集合改名/跨集合复用/导出仍未实现。
- 样式：L1 Corpus Explorer、共享壳层、LIDS token、中文主表达、13px Sans 原声表格、两行截断与文字 Tab，Serif 留在详情；页面前缀 `lgi-research`，无新主题或全局样式覆盖。
- 权限：本机研究 Host/Origin 校验、no-store、来源资格收缩沿源/资产/分析读取传播；不新增敏感材料外发许可。
- 验证：见 [验收记录](../acceptance/comment-research-001-acceptance.md)。合成 UI 来源均显式含 SYNTHETIC / NOT EVIDENCE；未发生真实模型访问、共享迁移、部署或用户验收。

## COMMENT-DAILY-001 增量（2026-09-07）

Mog 要求参照证据库表格，新增点赞、评论时间/入库时间、清洗和分析状态、作品与清洗筛选、选择分析。原声仍是主视图，没有大型卡片。每日研究为第四个内部视图，设置、分析范围和逐条明细按需弹窗；模型设置页仅链接到这里发起新研究，原计划历史仍可查。局部复用 Evidence 的 13px 字号和 7px 单元格内边距，不修改共享 Token、证据库或一级导航。字段来自单条接纳记录与既有 owner；无伪造互动、作者或缺失上下文。
