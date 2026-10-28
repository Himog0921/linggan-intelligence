# 研究自动化执行合同

> 状态: 代码事实优先
> 最后核对: 2026-10-27
> 适用范围: 评论研究、主题研究、创作者单篇与账号研究的当前候选
> 事实来源: ai_foundation 业务处理器、Axum 路由、迁移 0173–0179 与隔离 PostgreSQL/浏览器证明
> 冲突时以谁为准: 用户最新明确授权、当前代码和实际回执；候选不代表共享服务已发布

## 输入、版本与公共调用

作品语境只从 `research-dossier.v2` 取得。来源引用保留作品、来源版本、读本、块与 Unicode scalar 范围；图片缺页、未识读和未知页数随材料进入研究。评论原声与父评论分别有不可变来源，父评论和作品语境不能补成目标评论的原声证据。

每次提交原子冻结所有阶段 task config、提示词与 Skill；设置迭代只影响后续提交。提示词与 Skill 在 AI 设置直接编辑为不可变新版本，并可切换、回退；操作本身不派发模型。模型能力、输入额度、发送许可、用量及回执使用共享物理调用层，业务选择和接纳由各模块负责。

同一命令只重放原回执。确定拒绝后可修正条件；已经可能发送而缺少有效回执时记录 `ai_dispatch_unknown`，保留预留，不自动再发。同一冻结来源范围的未决请求阻止新命令重发。取消、暂停或撤销许可导致不能接纳结果，也不能将物理发送未知改成确定失败。

## 评论研究

| 接口 | 行为 |
|---|---|
| `GET /api/local/comment-research/preview` | 固定领域、作品、精确评论来源与观察角色，返回 cutoff、输入身份和当前阶段配置版本 |
| `POST /api/local/comment-research` | 显式提交单条目标；网页批量选择最多 50 条，逐条使用稳定命令编号，部分提交保留已确认回执 |
| `GET /api/local/comment-research/{requestRef}` | 读取原声提取、问题归并、独立核验三阶段与冻结目标原声；来源不可读时不返回原文和结果 |
| `GET /api/local/comment-research/latest`、`runs` | 精确来源的最近请求；领域最近 100 条工作流回执，列表不返回原声正文 |
| `GET /api/local/comment-research/problems` | 当前合格问题、精确原声依据及等待独立支持的候选 |
| `POST /api/local/comment-research/cancel` | 追加请求级取消意图，停止余下阶段及未接纳候选的后续支持；不改写历史模型回执 |

任务分别为 `comment_study`、`comment_problem_resolution`、`comment_problem_pair_review`。提取必须能逐字还原到目标 raw comment；归并基于主体、目标、障碍、情境。单条原声形成候选，共性问题至少由两位独立作者支持并通过独立核验。后来完成的核验可接纳先前等待的候选；详情保留早先阶段等待记录，汇总标记后续接纳。

定义和归属均不可变。当前支持原声失格时隐藏问题；失格旧定义不能永久占用同一边界，新的合格作者对可以建立新定义。待支持状态即使三批已结束也能取消，取消与发布共享同一领域串行边界。冻结候选的资格在下一阶段发送前、最终接纳前重新检查。

`/api/local/comment-study` 只保留评论来源目录、索引覆盖、父评论、作品目录、版本与改帖标注。旧研究 run/policy 写入口返回退役，旧研究表不再参与读写。

## 主题研究

`POST /api/local/topic-research` 显式选择最多 10 篇作品、最多 50 个完整底稿/评论窗口。`GET /api/local/topic-research/runs?domainRef=...` 返回最近 30 个 run、窗口及实际阶段回执；summary 是当前领域完整研究历史的阶段合计。`POST /api/local/topic-research/cancel` 只取消精确 run 的余下阶段。

四阶段任务为 `topic_research`、`topic_research_extract_review`、`topic_research_resolve`、`topic_research_resolve_review`。引用精确、提炼校验、概念归属与独立核验全部通过后才接纳图谱；无信号只执行前两阶段，余下明确标为无需归属。词语或向量相似不能替代主题边界判断，未知不能强建主题。

归属候选是完整的当前合格定义集合；不以固定前 N 条截断来隐瞒范围。完整包超过配置额度时发送前拒绝，需在设置调整能力内额度。归属核验前及结果接纳前重查冻结候选的当前资格。定义依赖取实际 dispatch context 与 accepted core，比较邻居或父定义的来源撤回会沿依赖传播，失格定义不能再次成为模型候选。

图谱、人工结构操作和已保存备选仍使用现有 topic-map 接口；每次读取/保存/导出重新检查来源与定义资格。主题采集配置与研究执行分开，`linggan_topic_collection_policy` 只管既有采集意图，不保存模型配置或研究状态。旧 `/topic-map/research` 读写入口返回退役。

## 创作者研究

单篇 `GET /api/local/creator-research/preview`、`POST /api/local/creator-research`、`GET /api/local/creator-research/{requestRef}` 使用当前合格标准底稿，独立任务 `creator_research`。

账号对应 `creator-account-research` 三个接口及独立任务 `creator_account_research`。preview 明确固定选中的 1–30 篇合格作品及可用公开简介，start 核对精确列表及 fingerprint。抽样限制随 preview 和页面展示，不冒称账号全量画像；笔记材料与公开简介许可分别管理。研究结果动态复核来源，人工覆盖保存在独立当前表。

## 数据库与历史边界

0173–0177 保存新研究请求、阶段、派发、结果、定义与归属；0178 保留人工覆盖与阈值后删除旧创作者分析表；0179 用 RESTRICT 退役旧评论与主题研究表/视图/函数。历史 SQL 迁移及升级测试保留为历史，不是活跃双轨。首次建库由有序迁移准备并移除历史脚手架，重复迁移不得重建旧表。

实施与验证回执见[统一研究工作包](../plans/active/unified-research-automation-001.md)。真实证据笔记配合本机合成模型只能证明流程、来源及版本链；真实模型研究质量、发布和业务验收分别记录。
