# UIUX-CONTENT-WORKBENCH-001 · 页面覆盖与证据附录

> 状态: 一次性报告
> 最后核对: 2026-10-14
> 适用范围: 第一阶段逐页覆盖、截图采信、源码定位与不可验证边界
> 事实来源: 2026-10-14 当次真实浏览器、DOM、代码及运行身份核对
> 冲突时以谁为准: 原始证据及当前代码；本表只描述当次基线，不证明后续状态

主结论、能力缺口和优先级见[审计报告](uiux-content-workbench-baseline-2026-10-14.md)。专项目标见[活跃计划](../archive/plans/uiux-content-workbench-001.md)。

## 1. 证据保存与查看

- 本机完整图册：`artifacts/private/uiux-content-workbench-001/2026-10-14/walkthrough.html`。51 组采信截图按编号展示，附当时状态、页面和相关问题；报告的 E 编号与图册对应。
- 同目录每组包含原始 `.jpg` 和 `.txt` DOM；`capture-index.json` 保留时间、实际 URL 与说明；`evidence-manifest.json` 保留每个文件的完整 SHA-256。均为当次工具原始截图，保存后读取同一文件检查，未生成模拟页面替代。
- 原始内容含内部材料/运营数据，按[生成物登记](../governance/generated-artifacts-registry.md)仅本机保留，不进入 Git、不上传。其他检出目录没有这些文件时，应在相同权限下重新走查；不可把缺文件解释为已通过。
- 主走查为 1440×1000，E51 为 1280×800，E52 为 390×844；E01 是初始窄幅登录页。E02 尺寸异常；E14/E28 处于加载中，均不作稳定页面证明。E30 文件名虽含 ready，实际是错误状态，按错误证据使用。

## 2. 页面与关键状态覆盖

| 页面族 | 当次完成的只读路径 | 代表证据 | 未执行/边界 |
|---|---|---|---|
| 根入口、全局壳 | 根到登录、从语料进入、领域选择、主导航、个人菜单、设置 | E01 E03 E48 E49 | 开发预览登录不代表正式认证；没有输入凭据 |
| 证据库 | 研读/封面布局，作品详情、证据、筛选、领域选择，采集作品精确定位 | E04–E07 E49–E52 | 表格布局未单独截图；评论深采、补采、复观测未提交 |
| 评论研究 | 概览、评论列表、单评论、研究批次、运行方法、用户问题；研究准备与方法查看 | E08–E16 | 新研究/版本保存/比较/采用/恢复未执行；比较入口存在不等于结果已验证 |
| 作者 | 目录、复合筛选、详情、作品证据 | E17–E20 | 未加入观察/重试分析；作者→语料返回合同检查代码，没有穷举全部筛选组合 |
| 主题图谱 | 主题概览、原作讨论阅读、结构表现、内容格局、来源分布、新方向、我方对照、生命旅程、我的备选、研究配置 | E21–E32 | 没有保存备选、结构调整、研究或补采；单主题各分支未穷举；旧冻结工作区没有当前可跟随实例，代码核对 |
| 采集目标 | 全部领域、作者概览/作品/巡查、异常只读、关键词概览/作品、跨页到语料 | E33–E36 E49–E50 E53–E54 | 生命周期图表子视角未逐项切换；未新增/删除/确认失效/补采/调整规则 |
| 采集任务 | 列表、历史任务进展和成果、单次采集配置入口 | E37–E40 | 未发起任务、写配置或验证新回执；历史任务的0/未知不证明材料不存在 |
| 工位、领域管理 | 运行工位、队列/容量状态、领域列表/表单与关联范围 | E41–E42 | 不操作插件、不暂停/恢复、不改额度/授权/领域 |
| 运营 | 发布记录、总览、账号、每日数据、导入导出 | E43–E47 | 写入口当前禁用；目标设定未保存，数据导出未下载，未访问外部发布链接 |
| 模型设置 | 个人菜单→设置→连接列表、默认模型/上限、后台调用记录 | E48 | 未展开密钥、测试供应商、保存或停用模型 |

旧 `/collection/operations` 与 `/collection/attention` 在当前源码重定向到任务入口，未作为独立新页重复计数。页面路由和按钮检查不代替后端真实执行证明。

## 3. 可复现路径

1. F01：主题图谱 ADHD 概览→用户生命旅程→出现版本冲突→重新读取→恢复；这是当次一次观察，不能保证每次复现。服务端冲突成因需要后续追踪。
2. F02：采集→全部领域→观察目标，比较顶部建档中数量和创作者行的档案状态。后续后台变化可能改变数量，但源码口径差异仍可核查。
3. F03：运营→总览→本周，在迁入尚未覆盖当前周期时，比较“未知”读数与漏斗结论。
4. F04：语料→创作者→有实际媒体的作者详情，在图片已呈现后比较截图与可访问名称。
5. F09：采集全部领域→某作者→作品→查看→选择 ADHD。语料仍选中原作品，但无应用内返回观察目标入口。
6. F11：1440×1000→运营发布记录，未横向滚动时“阅读量”表头矩形 x=1540、width=88，视口宽1440。
7. F18：语料研读保持同一选中作品，以1280×800、390×844抽检；两尺寸 document scrollWidth 等于 clientWidth，未观察到整体横溢出，但工具与导航占据过多纵向空间。

## 4. 源码定位（5511d75e）

下列定位为阅读过的当前运行版本。行号用于追踪，后续修改后应按函数/字段重新核对；不能引用其他工作树的未发布候选宣称运行能力。

| 证据 | 代码与位置 | 核对内容 |
|---|---|---|
| S01 | [local_web.rs](../../apps/api/src/local_web.rs) 293–294、473–482；[shell.rs](../../apps/api/src/local_web/shell.rs)；[login.html](../../apps/api/src/local_web/login.html) 555 附近 | 根/语料路由、一级导航、开发登录跳转 |
| S02 | [local_web.rs](../../apps/api/src/local_web.rs) 6106、6139–6178；`evidence_library.js` 5–29、379；`evidence_library.css`（这两个文件已随 WEB-REACT-001 阶段 D2 删除，证据库现为 React 页；行号为审计当日的旧页） | 技术说明/占位、领域与work参数、作者/任务返回、阅读模式 |
| S03 | [creators.rs](../../apps/api/src/local_web/creators.rs) 12–25；`creators.js` 65–85、131；`creators.css`（后两个文件已随 WEB-REACT-001 D1 删除，当次基线见 `origin/main@36f3fdf9`；创作者页现为 `apps/web/src/pages/corpus/creators/`） | 作者读取/写边界、证据跳转、图片及可访问名称 |
| S04 | [comment_study.html](../../apps/api/src/local_web/comment_study.html) 19–104；[comment_study.js](../../apps/api/src/local_web/comment_study.js) 166–187、283、737、855–903、1539；[comment_study.css](../../apps/api/src/local_web/comment_study.css) 42–72 | 列表/研究/方法入口、加载依赖、版本显示、弹窗与表格 |
| S05 | [topic_map.rs](../../apps/api/src/local_web/topic_map.rs) 49–76、110、320–330；[topic_map.js](../../apps/api/src/local_web/topic_map.js) 92–126、155–157、185、464、781–786；[topic_map.css](../../apps/api/src/local_web/topic_map.css) | 读取/命令边界、错误码映射、覆盖读数、旅程、人工备选及引用导出 |
| S06 | [collection_targets_view.rs](../../apps/api/src/local_web/collection_targets_view.rs) 636–750、784–835；[collection_target.rs](../../crates/evidence/src/collection_target.rs) 560–608；[target_drawer.rs](../../apps/api/src/local_web/target_drawer.rs) 1380–1402、1702–1721、3160；[local_web.rs](../../apps/api/src/local_web.rs) 354–393、3606–3632、3731–3739；[collection.rs](../../apps/api/src/local_web/collection.rs) 792–793 | 目标状态计数/行语义、作品跳转、失效默认选择、领域与任务路由 |
| S07 | [ops.rs](../../apps/api/src/local_web/ops.rs) 12–33；[ops_publications.html](../../apps/api/src/local_web/ops_publications.html) 15–22；[ops.js](../../apps/api/src/local_web/ops.js) 247；[ops_overview.js](../../apps/api/src/local_web/ops_overview.js) 249–271；[ops_io.html](../../apps/api/src/local_web/ops_io.html) | 运营只读、选题池占位、发布表列、漏斗结论、台账导出 |
| S08 | [model_settings.rs](../../apps/api/src/local_web/model_settings.rs) 18–36；[model_settings.html](https://github.com/Himog0921/linggan-intelligence/blob/f1ec98f385ed0760c822e0d20427a087015a8bbf/apps/api/src/local_web/model_settings.html)；[model_settings.js](https://github.com/Himog0921/linggan-intelligence/blob/f1ec98f385ed0760c822e0d20427a087015a8bbf/apps/api/src/local_web/model_settings.js) | 设置作用域、模型测试/保存边界与记录 |
| S09 | [topic_workspace.rs](../../apps/api/src/local_web/topic_workspace.rs) 21–27、123–154；[topic_workspace.js](../../apps/api/src/local_web/topic_workspace.js) | 旧工作区为定义和冻结材料读取；没有正文/图片编辑闭环 |
| S10 | [LIDS tokens](../design/lids/tokens.md) §0–1；[语言规则](../design/lids/language-policy.md) LANG-05；[primitives](../design/lids/primitives.md)；[patterns](../design/lids/patterns.md)；[lids_tokens.css](../../apps/api/src/local_web/lids_tokens.css) | 已有规则、目标/运行 Token 分层、中文规则和组件模式；本轮未改规范数值 |

能力缺口依据是：本轮主路由盘点、模块实现和真实入口均不能完成该用户结果，不是仅靠搜索某个关键词没命中。没有逐行审查整个仓库；本报告不是安全审计或全量代码质量审查。

## 5. 截图逐项索引

时间为北京时间。文件前缀与 E 编号一一对应；全部原始文件在第1节私有目录。排除项保留作审计记录，不放入有效状态图册。

| ID | 时间 | 文件前缀 | 采信及说明 |
|---|---|---|---|
| E01 | 16:03:59 | `01-login` | 采信：根入口跳转登录；开发模式；登录能力尚需查代码 |
| E02 | 16:04:19 | `02-login-desktop` | 排除：排除：变更视口后的截图仍为旧窄幅裁切，不作1440桌面证据 |
| E03 | 16:04:31 | `03-domain-gate` | 采信：语料初始无领域时自动弹出选择；大量禁用控件与技术旁注 |
| E04 | 16:04:55 | `04-evidence-reading` | 采信：语料研读默认：列表及检查器、技术语言、材料可读性 |
| E05 | 16:05:07 | `05-evidence-detail` | 采信：证据标签读取正文与评论；原始材料可读，未有制作入口 |
| E06 | 16:05:18 | `06-evidence-filters` | 采信：筛选开启：过滤选项与工具栏组织 |
| E07 | 16:05:29 | `07-evidence-cover` | 采信：封面布局，比较卡片信息与研读的一致性 |
| E08 | 16:05:43 | `08-comments-overview` | 采信：评论概览；研究统计和自动化关闭状态 |
| E09 | 16:05:56 | `09-comments-library` | 采信：评论列表与筛选，采集/研究状态和九类信号 |
| E10 | 16:08:47 | `10-comment-detail` | 采信：评论详情只读：原声与研究状态 |
| E11 | 16:08:52 | `11-research-runs` | 采信：研究批次列表，只读检查 |
| E12 | 16:09:12 | `12-run-method` | 采信：本次方法只读视图 |
| E13 | 16:09:21 | `13-user-problems` | 采信：用户问题当前列表与新表达状态 |
| E14 | 16:09:30 | `14-research-setup` | 排除：排除稳定页面验收：研究配置仍在加载；由E15代替 |
| E15 | 16:09:54 | `15-research-setup-ready` | 采信：研究准备已读取，仅打开配置未提交 |
| E16 | 16:09:55 | `16-method-viewer` | 采信：研究配置中的方法详情，只有一个现有版本 |
| E17 | 16:10:02 | `17-creators` | 采信：创作者目录默认状态 |
| E18 | 16:10:07 | `18-creator-filters` | 采信：创作者筛选，检查复合筛选语义 |
| E19 | 16:10:29 | `19-creator-detail` | 采信：作者详情概览；真实头像已显示，辅助标签仍写未取得 |
| E20 | 16:10:29 | `20-creator-evidence` | 采信：作者作品与判断依据 |
| E21 | 16:10:35 | `21-topics` | 采信：主题图谱入口 |
| E22 | 16:10:52 | `22-topic-reading` | 采信：主题内原作与讨论读取完成 |
| E23 | 16:10:58 | `23-topic-structure` | 采信：结构与表现实际数据 |
| E24 | 16:11:08 | `24-topic-landscape` | 采信：内容格局视角 |
| E25 | 16:11:15 | `25-topic-sources` | 采信：来源分布 |
| E26 | 16:11:23 | `26-topic-new-directions` | 采信：新方向与人工备选关系 |
| E27 | 16:11:28 | `27-topic-owned` | 采信：我方对照与再做 |
| E28 | 16:11:35 | `28-topic-journey` | 排除：排除稳定页面验收：旅程仍在加载；错误与恢复分别见E30、E31 |
| E29 | 16:11:51 | `29-saved-alternatives` | 采信：我的备选当前空状态 |
| E30 | 16:11:51 | `30-topic-journey-ready` | 错误证据：用户生命旅程读取失败：topic_map_version_conflict |
| E31 | 16:12:11 | `31-topic-journey-retry` | 采信：用户生命旅程点击重新读取后 |
| E32 | 16:12:34 | `32-topic-research` | 采信：研究进度与设置只读，未改变调度 |
| E33 | 16:12:40 | `33-collection-targets` | 采信：采集观察目标入口 |
| E34 | 16:13:02 | `34-target-overview` | 采信：异常作者档案概览 |
| E35 | 16:13:52 | `35-target-works` | 采信：目标作品目录列表；生命周期视图入口可见，本轮未逐项切换 |
| E36 | 16:14:06 | `36-target-patrol` | 采信：目标巡查规则与运行历史 |
| E37 | 16:14:19 | `37-collection-tasks` | 采信：采集任务列表 |
| E38 | 16:14:47 | `38-task-progress` | 采信：历史巡查任务进展，多项交付未知 |
| E39 | 16:14:48 | `39-task-results` | 采信：任务成果视图 |
| E40 | 16:15:05 | `40-single-collection-setup` | 采信：单次采集准备，仅查看不发起 |
| E41 | 16:15:17 | `41-collection-runtime` | 采信：工位管理状态；不改设置 |
| E42 | 16:15:48 | `42-domain-management` | 采信：领域管理表单与关联规则 |
| E43 | 16:18:45 | `43-ops-publications` | 采信：发布记录只读；阅读量列位于宽表右侧；缺标题封面；未填写不同于0 |
| E44 | 16:19:00 | `44-ops-overview` | 采信：运营总览以加微与团队产出为重心；阅读量不是主要决策指标；保留缺失说明 |
| E45 | 16:19:24 | `45-ops-accounts` | 采信：账号管理已迁入只读列表；编辑禁用；团队字段较多 |
| E46 | 16:19:53 | `46-ops-daily` | 采信：每日数据以团队发布、加微为主；当前只读 |
| E47 | 16:20:03 | `47-ops-io` | 采信：导出为运营台账CSV/JSON，不是图文成品；导入未开放 |
| E48 | 16:20:29 | `48-model-settings` | 采信：全局设置文案实际聚焦评论研究；连接/默认/后台记录密集；未测试或保存配置 |
| E49 | 16:21:56 | `49-target-to-corpus-scope` | 采信：从全部领域的采集作品跳语料要求重选领域；work保留但无回目标入口 |
| E50 | 16:22:11 | `50-target-to-corpus-selected` | 采信：选ADHD后右侧保留原work定位；列表仍按最近观察排列；未提供回到来源目标的应用内入口 |
| E51 | 16:22:30 | `51-corpus-1280` | 采信：1280x800桌面抽检；页面无整体横溢出，主材料首行y322，研读空间受多层工具条挤压 |
| E52 | 16:22:36 | `52-corpus-390` | 采信：390x844响应式抽检；导航和工具条回流，首屏几乎无材料；非首批移动端验收 |
| E53 | 16:22:58 | `53-keyword-overview` | 采信：关键词概览能够清楚分开最近命中、新增、下一次巡查及是否需处理；应保留此任务导向结构 |
| E54 | 16:23:28 | `54-keyword-works` | 采信：关键词命中表能关联作者、详情进度与采集语境；列表信息密集 |

## 6. 可访问性与验证限制

本轮保存 DOM/可访问名称并检查代表性交互。确认了作者媒体名称不同步的问题；抽查发现小字号和长字段裁切风险。没有运行全站读屏、全键盘任务、自动对比度扫描、200%缩放或完整浏览器矩阵，因此不宣称 WCAG 合规。截图的视觉密度判断与功能/数据缺陷分开登记。

没有人为断网、注入接口失败、改数据库制造空态、触发模型或调整运行策略。可见错误/空态为既有真实状态。页面存在的方法比较、规则保存、任务恢复等写入能力统一列为 NOT VERIFIED。后续应在隔离环境验证异常合同，再由 Mog 验收真实模块。
