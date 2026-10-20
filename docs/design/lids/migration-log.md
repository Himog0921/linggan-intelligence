# LIDS-LOG-001 · LIDS 迁移与变更记录

PR #407 合并前修正（2026-10-13）：运营页焦点/强调线与新增共享页头 padding 改为现有间距 Token 的等值表达，保持 Mog 已确认的外观、断点和导航尺寸。未新增共享 Token 或扩大页面/权限范围。范围及验证见 [D2 验收记录](../acceptance/ops-page-001-visual-acceptance.md)。

> 状态: 权威当前
> 最后核对: 2026-10-19
> 适用范围: Linggan Intelligence LIDS Token、Primitive、Component、Pattern、Page、Motion、Scene 和 Data Truth 规则的实际变更、替代、例外与验证边界
> 事实来源: [system.md](system.md)、[README.md](README.md)、DESIGN-002 Issue #7、项目 progress 记录和实际验证输出；LIDS-ALIGN-001 本次授权与对齐记录
> 冲突时以谁为准: 真实代码/合同/测试、用户最新确认、当前 SCOPE 和 ACCEPTED 决策；本日志不把计划写成已实现事实


当前采用索引（2026-10-20）：Q01–Q12均已确认；当前未决只读[review-items](review-items.md)，已采用参数只读分册及唯一JSON。下文按日期保留的“待审”描述当时状态，不是新的当前选项。

## WEB-REACT-001 H · 全站唯一前端源（2026-10-19）

Mog完整收尾授权：Token与共享shell从apps/api迁到apps/web/src/styles，不改变既有数值；#434新增V3/V8 Token及局部外壳依原批准源集成。Rust镜像校验、两个插件构建入口、React/Vite/CI同步引用最终唯一源。登录继承LOGIN-ANATOMY-001已批准局部值，不推广为全站主题；CSS Module冻结哈希和实际36对零差异共同约束其迁移，修复全局焦点/短屏滚动泄漏。历史路径保留在旧记录，仅作可追溯来源。

## 2026-10-18 · WEB-REACT-001 D3 评论研究四视图

保留正式3000的A无边框概览和现有全部表面；JSX与CSS Modules替代旧原生renderer，复用唯一AppShell/shell.css/LIDS，不改Token。两份原CSS同名类保持有效层叠；旧未定义token按实际computed fallback保留，尺寸改为既有Token等值calc，不借迁移修视觉债。原声/问题来源受限、未知与0、未确认命令/显式重试仍分开。实际六宽度与完整滚动对照、隔离PG/浏览器回执及未部署边界见 [D3验收](../acceptance/web-react-001-d3-acceptance.md)。F已于#441部署的当前补记见其验收记录；本日志旧“未部署”仍为历史候选时点。

## 2026-10-17 · WEB-REACT-001 F：Mog选定B材料优先概览

仅 /topics 概览采用已确认的 B 连续白色台面、紧凑读数、左材料右分布、左侧双视图；完整功能/服务器统计与来源资格不改。CSS Modules只消费既有token，未改全局shell/Token/业务内核。图表标明当前返回范围，未知分开，跨阈值桶不判成高表现；四宽度与极端输入见 [F修订验收](../acceptance/web-react-001-f-acceptance.md)。此项取代本页旧F条目中概览像素保真要求；旧证明保留历史范围。未部署或Mog体验签收。

## 2026-10-16 · WEB-REACT-001-F 主题图谱与 Topic Workspace

迁移已确认的当前 UI：六页签、双视图/单侧栏、阅读与研究/来源/结构弹窗及独立 Workspace。复用唯一 LIDS token 与 shared shell，局部 CSS Modules，原几何用等值 token/calc 表达；不改 token 值、颜色或算法语义。React JSX/生命周期取代原生 HTML renderer，保持滚动与重建行为。新增的异常写入恢复只显示真实未知回执与显式原请求重试，不自动外发。四视口整页/滚动 2,905 组逐像素一致；已知旧 Workspace JSON 缺 domain 与空按钮行为保留。候选、真实隔离链路、性能/审查和未部署边界见 [F 验收](../acceptance/web-react-001-f-acceptance.md)；不宣称全站 LIDS 欠账或 Mog 签收完成。

## 2026-10-16 · WEB-REACT-001 阶段 D2：证据库迁到 React（Issue #433 / #430）

- **实现载体变化**：`/corpus/evidence` 改由 `apps/web` 的 React 应用提供（`apps/web/src/pages/corpus/evidence/`，经 D1 的 `CorpusFrame`）；旧 `evidence_library.js/.css`、`evidence_observation.js`、`research_dossier.js`、`research_dossier_batch.js` 与 Rust 页面拼接删除。语料三页里只剩评论研究仍由 `shell.rs` 渲染。
- **未新增全局资产**：没有新 Token、Primitive、CMP、Scene、Motion 或第二套外壳样式；页面样式为 CSS Module，只消费 `--lgi-*`，`tokenDiscipline.test.ts` 自动检查；样式契约测试另守封面多列栅格、3:4 完整显示与检查器栏宽（三档加关闭）。组合沿用 L1 Corpus Explorer + 右侧 Split Evidence Inspector；1024px 及以下检查器为抽屉。
- **共享外壳接缝**：侧栏「证据库」项的计数徽标改由页面填写（DR-5）；D1 的 `CorpusSideNav`／`CorpusFrame` 只多一个可选值，与 `shell.rs` 的对照测试照旧通过。
- **页面局部变化**（均记在[阶段 D 分册 §4](../../plans/active/web-react-001-d-corpus.md)，规格修订见 [PAGE-EVIDENCE-001](../pages/evidence-library-page.md) §14）：移除读模型给不出结果的三个控件（「相关度」排序、近 7／30 天时间窗、「旧 OCR 已停用」状态）与未接通占位（「我的视图」「打开原文」「补采」「补采媒体」）；立即复观测先确认、回执分已提交／未受理／未完成／未确认；阅读主序在前、技术事实默认折叠；评论原文点击后才读取。
- **验证边界**：视觉与行为证据见 [ACC-WEB-REACT-001-D2](../acceptance/web-react-001-d2-acceptance.md)；合成数据，未经 Mog 审美验收，未合并、未部署。

## 2026-10-16 · WEB-REACT-001 阶段 D1：语料共享外壳与创作者页迁到 React（Issue #433 / #430）

- **实现载体变化**：`/corpus/creators` 改由 `apps/web` 的 React 应用提供，旧 `creators.html/.js/.css` 删除；语料三页共用的左侧二级导航、面包屑领域选择器与一级导航「采集」状态词在 React 里有了共享实现（`apps/web/src/corpus/`），数据来自只读接口 `GET /api/local/corpus/shell`。旧证据库、评论研究页仍由 `shell.rs` 渲染，页头与侧栏位置、样式与它们同源（`shell.css`），前端测试逐项对照 `shell.rs` 的侧栏登记表与标记。
- **未新增全局资产**：没有新 Token、Primitive、CMP、Scene、Motion、按钮或第二套外壳样式；页面样式为 CSS Module，只消费 `--lgi-*`，页面里的 `--creator-*` 局部变量是现有 Token 的 `calc` 组合（沿用旧页做法）；`tokenDiscipline.test.ts` 自动检查不写死颜色、字号、行高与间距。组合沿用 `LIDS-PAT-001` 的 L1 Corpus Explorer + 右侧 Split Evidence Inspector（依据抽屉）。
- **页面局部变化**（均记在[阶段 D 分册 §3.1](../../plans/active/web-react-001-d-corpus.md)）：回执分「已完成 / 未完成 / 未确认」，用文字标签 + 左侧色条表达，不只靠颜色；「加入观察」先弹确认；头像、封面的可访问名称与实际加载状态一致；封面按 3:4 完整显示；窄屏（≤640px）分析状态入口只留「设置」，与旧页一致；未知显示「—」，≥1 万缩写并在悬停给精确值（LIDS BOUND-001）；时间统一 `YYYY-MM-DD HH:MM`。
- **语义呈现**（来自 PR #384 调查，只读）：作品表达与作者倾向分列分层并写明各自的判定门槛与可并存关系；分析状态「排队 / 失败 / 已分析但依据不足 / 尚未取得结果 / 未开启」分别显示；筛选条件可组合、逐项可移除，「重置」只清列表条件，范围统计与筛选结果分开。
- **验证边界**：视觉与行为证据见 [ACC-WEB-REACT-001-D1](../acceptance/web-react-001-d1-acceptance.md)；合成数据；2026-10-16 已合并上线（`baaf5a51`），未经 Mog 审美验收。

## 2026-10-15 · WEB-REACT-001 阶段 B：模型设置页迁到 React（Issue #430 / Draft PR #431）

- **实现载体变化**：`/settings/models` 改由 `apps/web` 的 React 应用提供（DEC-0010），旧 `model_settings.html/.css/.js` 删除。页面样式为 CSS Module，只消费 `lids_tokens.css` 的 `--lgi-*`；外壳仍用同一份 `shell.css`，React 页头与 `shell.rs` 结构一致（前端测试逐项对照导航登记表）。
- **未新增全局资产**：没有新 Token、Primitive、CMP、Scene、Motion 或第二套外壳样式；Token 文件直接被前端构建引用，不复制第二份。
- **新增自动检查**：`apps/web/src/test/tokenDiscipline.test.ts` 扫描 React 页面样式，拒绝写死颜色、字号、行高与间距（边框线宽除外）。它只覆盖 `apps/web`，旧页面仍靠自觉与审查。
- **页面局部变化**：回执分「已完成 / 未完成 / 未确认」三种语气，用文字标签 + 左侧色条（success/danger/warning 三轴）表达，不只靠颜色；内容区可滚动；与面包屑重复的小标题删除；时间统一为 `YYYY-MM-DD HH:MM`。
- **验证边界**：视觉与行为证据见 [ACC-WEB-REACT-001-B](../acceptance/web-react-001-b-acceptance.md)；未经 Mog 审美验收，未部署。

## 2026-10-14 · CORPUS-COPY-001 语料两页文案快修（Issue #421）

Page 层文案修订，依据 LANG-01 / LANG-05 与 Mog 2026-10-14 的审查决定：评论研究删去读数带上方的领域编号与汇总时间行，时间统一为北京时间 `YYYY-MM-DD HH:MM`，可见描述性英文改为中文，用户评论默认按点赞排序并只在例外时显示资格与标题来源；证据库删去「系统视图」「我的视图」的英文对照和页头 `NO LEGACY FALLBACK`。不改 Token、Primitive、CMP、Pattern、Motion、版式或数据合同。规格见 [PAGE-COMMENT-STUDY-REBUILD-001 §4.8](../pages/comment-study-rebuild-page.md) 与 [PAGE-EVIDENCE-001](../pages/evidence-library-page.md)；候选尚未合并、部署或经 Mog 验收。

## 2026-10-14 · UIUX-CONTENT-WORKBENCH-001 V2 原型提案

用户要求提高研读密度与文案沉浸感，并提供差异截图。V2 在独立子目录增加三栏研读、结构画布和双栏改稿：浅绿为新增文字，淡红为删改，浅黄为原文精确定位；仅局部提案值，未晋升正式 token。短屏改稿采用面板内滚动，资料次级控件以紧凑尺寸及菜单替代操作兼顾密度。真实业务与全站迁移不在本轮范围；详见同一设计提案和验收记录。

## 2026-10-14 · UIUX-CONTENT-WORKBENCH-001 独立原型候选

三页独立原型沿用白底、信号色与中文排版，提出紧凑导航与按任务分步创作。仅设计目录源，生产 token 和壳层不变；不声称迁移完成。见[提案](../changes/uiux-content-workbench-001-prototype.md)与[原型验收](../acceptance/uiux-content-workbench-001-prototype-acceptance.md)。

## 2026-10-14 · OPS-MIGRATION-001 D3 合并后状态修复（Issue #417）

#418 已合并上线为 14ee80b1，真实运行/浏览器证明见 ACC-OPS-PAGE-002 §11。后审查读屏意见的候选仅将已有部分截止日纳入趋势图可访问名，不改 Token、视觉、字段或权限；JS、原生 AX 与负控通过，候选尚未合并/部署，见 §12。

依据 PAGE-OPS-002 §2.1 / §3.1 和 OPS-HTTP-001 §4：未覆盖排行、平台占比、矩阵显示「这段时间还没有迁入的数据」；趋势未迁入爆款复用 `ops-gap` 未知色，部分周/月累计在图旁及悬停说明截至日；昨日目标始终为本周。沿用 L1 页面、空态、SVG 和 `--lgi-*` Token，不新增样式、Token、CMP、Motion 或页面结构。D3 原交付已上线，回执见 ACC-OPS-PAGE-002 §9；本次修复的验证随 PR 回执记录，尚未合并/部署或 Mog 签收。

## 2026-10-13 · OPS-MIGRATION-001 D3 · 运营「总览」

Mog 派定（Issue #409）：迁入工作台的运营总览。按 [PAGE-OPS-002](../pages/ops-overview-page.md) 在运营壳层里加第五个二级页，沿用 D2 的页头、面包屑、左侧栏、文字页签与表格；正文是五块从上到下的连续工作面。只用 `--lgi-*` Token，没有新增 Token、CMP、Scene 或 Motion。

**图表配色**：LIDS 没有分类色板，所以不为每个运营人造颜色（工作台是每人一色叠柱），改为运营人页签切换；颜色只表达语义：小红书 `--lgi-signal`、抖音 `--lgi-ink`、加微 `--lgi-info`、未迁入的日子 `--lgi-unknown-soft`，结论圆点用状态点 Token。图按容器实际宽度绘制，手机上坐标文字保持 Token 字号。

**验证**：在共享库的一次性副本上，总览在 1440×900、1280×800、390×844 下无页面级横向溢出；走查发现并修正了矩阵账号名压到相邻列、手机上图表文字被缩到约 5px、时间段一行被截断、读取失败时新页签下留着上一个时间段的数字等问题。详见 [ACC-OPS-PAGE-002](../acceptance/ops-page-002-visual-acceptance.md)。未合并、未部署，Mog 未验收。

## 2026-10-13 · OPS-MIGRATION-001 D2 · 一级页面「运营」与页头六项收紧

Mog 派定（Issue #405）：「运营」做成一级页面，页头和面包屑与其他一级页面同一套逻辑，左侧栏放二级页面，先只读。新页面按 [PAGE-OPS-001](../pages/ops-page.md) 复用共享页头、面包屑领域选择器和语料侧栏写法；正文是 Corpus Explorer 的表格组合（读数条、筛选、连续表格、分页），运营人与平台用文字页签加 3px 信号下划线（ADR-11），只用 `--lgi-*` Token，没有新增 Token、CMP、Scene 或 Motion。

**共享壳层改动**：一级导航从 5 项变成 6 项（B 区上限），原 1500/1180px 两档放不下，1181–1361px 与 1501–1545px 宽度会截掉「个人菜单」。`shell.css` 增加页头自己的台阶：原 1500px 档提到 1560px；1380px 以下去掉品牌副标题和弹性留白、导航项 90px；1240px 以下隐藏导航英文旁注（LANG-05 本就不要中英并列）；960px 以下导航项 80px。服务状态胶囊、侧栏宽度和约束区仍按原 1180px 档。手机上第 6 项按 2026-09-02 的规则换到第二行，未改。

**验证**：在共享库的一次性副本上，运营页与语料页在 901–1920px 的 29 个宽度上都不截；四个运营页在 1440×900、1280×800、390×844 下无页面级横向溢出；主题图谱的两项页面检查在新页头下通过；壳层台阶有源码回归测试，并做过反向核验。详见 [ACC-OPS-PAGE-001](../acceptance/ops-page-001-visual-acceptance.md)。未合并、未部署，Mog 未验收。

## 2026-10-13 · TOPIC-MAP-COMPACT-382 / PR #404 合并前焦点修复

Mog 授权修复当前提交的两项审查问题后，再确认最终提交。Context Bar 中本页入口的焦点描边改为消费 `calc(var(--lgi-space-1) / 2)`；基线仍为 2px，颜色 `--lgi-focus`、偏移 `--lgi-space-1` 保留。不新增或修改共享 Token，不扩展其它页面。另将工作区窗口恢复统一到本页视图切换入口，保持两类显式重置的全时间行为；见[本次清单](../changes/topic-map-compact-382.md)。键盘焦点与 Token 计算的浏览器验证待运行；未合并部署。

## 2026-10-12 · TOPIC-MAP-WORKFLOW-382 单侧栏材料工作区

本次用户直接确认主题概览/用户生命旅程左置、主区材料优先，替代 V4.1 对应布局。沿用本页运行时 Token、控件和原文阅读器；232px 单侧栏组织树/阶段/工具，主区共用检索、条件筹码、内容列表与当前方法研究状态，旧图表和概览保留。原文阅读器为侧向大面板；760px 以下折叠导航，搜索与内容自然换行，不改全站 shell/token。空态给出清词、全部阶段、待分析与全部材料路径。变更范围、实测和待验收层见[工作区计划](../../plans/active/topic-map-workflow-382.md)。

## 2026-10-11 · TOPIC-MAP-CORE-001 候选质量与研究口径

沿用 `/topics` 与既有共享页头、面包屑、Token、控件与阅读 Pattern。旧机器候选在领域树提供可展开的历史入口与明确质量标识；具体讨论显示领域相关性、主题抽象和精确父包含依据。撤回/旧定义版本不继续显示相关派生说明；所有文本转义。进度分开初次任务窗、至少处理一窗的作品和定义维护，说明跨方法/配置或重新分组可能重叠，不计算假的覆盖率。合成 UI 验证与实际浏览器候选验收分别在[活跃计划](../../plans/active/topic-map-core-001.md)记录，正式 3000 和真实模型语义质量尚未验证。

## 2026-10-10 · TOPIC-MAP-CORE-001 主题内核的页面状态投影

既有 `/topics` 概览、主题判断、来源阅读、我方/外部对照与备选保存消费新的独立讨论及精确定义：显示纳入/排除条件、作者/评论/引用角色、支持/反例/背景、未确定及当前覆盖；同名不替代主题身份，跨作品比较按原结果去重并保留各引用实际所属作品。来源失去资格时隐藏相关原文与派生说明，已接纳部分讨论继续可读。沿用现有 LIDS Token、组件和页面结构、共享 Header 与五阶段旅程，不新增跨页组件或全站视觉规则。

研究弹窗同时补齐真实比较阶段、失败/派发未知的有界任务明细，以及按后端允许状态呈现的暂停/恢复/停止；历史排队检查点不冒充现态，日额度等待不能被解释为可绕过预算。分类为状态表达与已有交互的修正；读取回执、表面/状态/依赖及验收矩阵见[活跃计划](../../plans/active/topic-map-core-001.md)，数据来源见 [HTTP 合同](../../data-contracts/topic-map-http.md)。

当前合成 JS 回归 14 组通过；本包前序 Chrome 验证覆盖 15 个桌面/窄屏状态，新进度增量以实际 JS 断言验证，不能沿用前序截图称其已经浏览器验收。原生 PostgreSQL 与最终复审结果在[当月进度](../../progress/2026-10.md)登记。此为开发候选，正式运行页、真实模型语料质量和 Mog 验收尚未发生，也不表示全站 LIDS 已完成迁移。

## 2026-10-09 · CREATOR-DISCOVERY-001 创作者 L1 页头候选

现有创作者页按 Corpus Explorer 和 DESIGN-003 收回重复视觉标题与常驻说明；搜索、快捷筛选、结果数和作者表格优先，范围筛选、五项统计及高级条件可展开，保留未知/来源限制与全部原行为。仅影响本页组合，不改 Token、Primitive、CMP、共享壳或数据合同。验证和运行边界见 `docs/plans/active/creator-discovery-001.md` 与当月 progress；在本机正式页及 Mog 验收前仍是候选。

同日继续按 Mog 的 Evidence Library 并排走查修正：搜索和按钮改为本页方角粗边框节奏，快捷视图改为文字下划线，作者表格由七列合为五列并去除固定行高。保留当前范围统计与来源分析限制、未知读数、上下分页及观察动作。沿用已有 LIDS Token；不提升为跨页组件，不修改 Evidence 页面或共享壳。

同页后续反馈收回面包屑下方重复的领域选择与表格前分页；SSR 注入当前合法领域隐藏值，表格后保留完整分页。面包屑领域 picker 已含范围作品数，页头页面 meta 插槽只标注“命中作者”与查询读取时刻（`displayAsOf` 来自 `scope_001_now()`，不表示材料更新），未加载/失败不沿用旧数；此页局部候选依据 Mog 本轮明确反馈，与 `shell-zones.md` 约束区不承载数字的通用规则存在局部冲突，不晋升为共享壳规则。原来源依据说明移入高级条件，状态区正常成功时收起，保留错误、空态、来源限制与动作回执。取消定时及 focus 自动读取。Token、共享 shell、API/数据合同未变；源码检查与运行验收分别记录在活跃计划及当月 progress。

## 2026-10-08 · COMMENT-STUDY-RELEASE-POLISH-358 · 窄屏评论表格候选

评论研究 L1「用户评论」复用既有表格横向滚动包裹层，把原来浏览器计算为 0 的 `calc(var(--lgi-space-24) * 6)` 改为有效的现有 Token 加法，并给五列评论表指定 960px 最小宽度和列宽。未改变 Token 值、Primitive、Pattern、共享 shell、HTML、数据和模型动作。只读 CSS 代理连接已部署 `:3000` 的真实 GET：390／520px 的最小宽度均计算为 960px，390px 可以滚向末列且整页无水平溢出；1440px 五列在页内可见。此为候选验证，未合并或刷新正式运行页。见[变更清单](../changes/comment-study-release-polish-358-ui-change-manifest.md)和[验收记录](../acceptance/comment-study-release-polish-358-acceptance.md)。

## 2026-10-06 · COMMENT-STUDY-RELEASE-POLISH-358 · 页面领域和状态表达

评论研究 L1 页面复用现有 `data-domain-name`，使概览标题跟随所选领域；最近 Run 复用已存在的中文状态词典。调用记录的状态沿数据库闭集译为中文，阶段沿现有三阶段名称显示。未选领域时四视图呈现选择引导，研究入口禁用。没有修改 Token、Primitive、Component、Pattern、共享 shell、数据合同或模型动作。真实 API 的只读本机代理浏览器验证 ADHD、考研自习、无领域四 Tab、评论→信号→浏览器返回和调用记录详情；这是候选脚本验收，正式 `:3000` 尚未刷新。见[变更清单](../changes/comment-study-release-polish-358-ui-change-manifest.md)和[验收记录](../acceptance/comment-study-release-polish-358-acceptance.md)。

## 2026-09-20 · COMMENT-STUDY-P3-QUALITY-GATE-010 未完成归并状态投影

- **范围**：既有 `/corpus/comments` 的 L1 Corpus Explorer 只读投影；不新增页面、路由、动作、状态、API/数据字段、Token、Primitive、CMP、Scene 或 Motion。
- **表达**：`retrieval_incomplete` 显示“候选目录未查全，当前不能判定是否为新问题”；`budget_stopped` 显示“归并预算已到上限，当前未完成判断”。二者均保留在既有“待归并”列表，而不是显示裸机器码、消失或被写成无匹配。
- **Data Truth**：两个字面状态和其迁移不由 UI 改写。前者表示候选目录未完整可查，后者表示当前比较预算已停止；它们均不证明没有匹配、没有新问题或模型失败。
- **验证与边界**：JS syntax、API 静态回归、format、diff 与治理检查覆盖中文标签和待归并过滤集合；本包未启动 runtime 或浏览器，因此真实渲染、响应式/键盘走查与 Mog 验收为 `NOT VERIFIED`。

## 2026-09-20 · OCR-CONTENT-LAYERING-001 Evidence Library 图片文字分层

- **范围**：Evidence Library 既有列表与 Inspector 增加“封面 OCR”标题来源和图片文字分层事实；不新增页面、导航、动作、全局 Token、CMP、Scene 或模型调用。
- **表达**：标题补位只在平台标题为空且后端已证明 `cover_headline` 时显示轻量来源标记。Inspector 保持原始 OCR 原文，同时明确显示版面记录、分层状态、规则/视觉判定来源、封面主文案和干净语料资格；历史结果显示“旧 OCR 已停用”，不以 Unknown 冒充。
- **Data Truth**：`image_substantive_text` 只在后端标为 `ACCEPTED` 时才可进入干净语料与普通列表引用；封面 OCR 仍不可成为默认列表引文。`PARTIAL` 保留可审计原文和已证明 headline，但不冒充完成的语料清洗。
- **验证与边界**：UI 标签、状态筛选与 Inspector 字段均直接读取现有 Work Resource 合同；样式只消费已有 `--lgi-*` token。真实 Paddle runtime、共享 migration、历史重跑、`3000` 刷新和 Mog 视觉/业务验收不由这条源码记录证明。

## 2026-09-17 · COMMENT-STUDY-LAYOUT-001 候选初始输入面

- **范围**：只重排 `/corpus/comments` 新评论研究的受控启动、作品选择和读取空态；不修改共享 shell、Token、API、schema、worker、模型、运行时或部署。
- **表达**：移除无信息的 visual Hero，采用 L1 Corpus Explorer 的紧凑工具栏与连续表格。M-00 白场、既有 `--lgi-*` token、1px 结构线、24px checkbox 命中区和 Primary 的 2px Ink 框/既有硬投影保持不变；没有新 CMP、Scene、Motion 或纹理。
- **交互与 Data Truth**：筛选只对当前已加载 `eligibleWorks` 生效，已选择集合不因筛选隐藏而收缩；显示“已加载”与“当前筛选命中”两个不同事实。作品标题与可研究评论数直接读取既有 setup 合同，未把 Unknown、空集或读取失败写成 0。
- **验证与边界**：页面静态测试、JS check、`git diff --check`、真实 100 篇本机读取、桌面与 390px 浏览器走查通过；未提交、合并、刷新 3000、创建 Run 或调用模型。见 [布局验收记录](../acceptance/comment-study-layout-001-acceptance.md)。

## 2026-09-15 · KEYWORD-ARCHIVE-002 巡查前建档门槛

- **范围**：Collection Targets 的关键词主操作与回执，及其服务端规则命令/巡查准入状态表达；没有新增页面、Token、CMP、Scene 或 Motion。
- **表达**：复用既有 L1 Collection 表格、L2 Drawer、状态反馈与 `--lgi-z-toast`。未知建档状态只导向“查看档案”，不以“设置巡查”制造可点击但必被拒绝的假动作。
- **Data Truth**：关键词进入巡查只认已接纳的搜索面完成事实和详情完成事实；已提交/运行中/空读/未知均不是完成。耐久拒绝原因和 scheduler decision 明示未完成或不可读，不把历史生命周期猜作证据。
- **验证与边界**：单元、编译和隔离 PostgreSQL 证明分别记录在本 PR 的验收记录；不自动重写历史规则、不证明 runtime、浏览器实测或 Mog 业务验收。

## 2026-09-06 · DES-INTELLIGENCE-20260906 产品与首页设计原型

- 变化：独立首页加四工作区，展开 16 个二级页；市场洞察先运营比较，Three.js 只用于首页稳定领域导航。
- 来源：Mog 当前设计委托与指定讨论。移出选题库/制作发布；更新旧页面的范围提示，保留历史。
- LIDS：新原型使用 v7 语义 token、中文主表达与 HTML 主操作；自有原生 CSS 位于独立设计源码，不修改当前运行时 token，不晋升 CMP。
- 验证与边界：详见[首页专项](../../pages/intelligence-home-threejs.md)第 8 节；合成交互不证明模型/采集/统计质量、生产实现或 Mog 验收。

任何影响 LIDS 五层或横向约束的事项必须在同一 PR 更新本记录：变更是什么、取代什么、影响页面/组件、验证结果和仍未证明什么。日志不是路线图，更不是运行时真相。

## 2026-09-15 · COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2 问题读取状态面

- **范围**：评论研究既有 L1 工作区新增 confirmed / deferred 筛选和只读详情；不新增页面、全局 Token、CMP、Scene、模型调用或手工归并动作。
- **表达**：使用既有文字 Tab、连续表格、Drawer、`--lgi-page-max-workbench`、`--lgi-z-drawer` 和 1px 结构线；不以页面局部像素宽度或边线厚度另造视觉 token。
- **Data Truth 与敏感边界**：confirmed membership 与 deferred signal 不混写。Deferred 详情只显示归一描述、frame、冻结候选、结论和执行状态；逐字评论、研究正文以及内部 UUID 均留在受限证据区，不进入普通问题读取 DTO 或前端。
- **验证与边界**：静态 API/UI 检查与隔离 PostgreSQL 的 deferred read 反例覆盖本边界；真实 runtime、浏览器窄宽度、模型语义准确性和 Mog 业务验收须由后续部署/人工步骤独立证明。

## 2026-09-10 · COMMENT-RESEARCH-RESET-001 V1 唯一页面

- **范围**：旧评论研究的每日观察、保存查询、资产、标注、Task B/P4 与技术队列表面被移除；新页面用概览、用户原声、用户问题、变化观察、运行记录五个独立读取面替代。
- **表达**：只组合既有 L1 shell、文字 tab、表格、状态与 modal，消费 `--lgi-*` token；不新增全局 token 或视觉风格。原声不显示 embedding 队列，确认作者回复的 `作者` 徽标不进入研究正文。
- **Data Truth**：变化观察只呈现窗口 observation/不可比原因，禁止复用概览；运行失败、无已发布版本和向量未就绪是不同状态，不能以旧结果或空数字替代。
- **验证边界**：页面/API 单元与隔离数据库证明在本交付分支通过；真实 runtime、浏览器交互、性能和 Mog 业务验收仍待上线步骤，见 [V1 页面规格](../pages/comment-research-v1-page.md) 与 [验收记录](../acceptance/comment-research-reset-001-acceptance.md)。

## 2026-09-08 · TARGET-INSPECTOR-PERFORMANCE-001

- **范围**：Collection Targets 保持 L1 Collection Control；右侧保持 L2 Split Inspector。没有新增 Token、全局 CMP 或主题。
- **表达**：移除 Targets Context Bar 的 runtime prose；表格工具栏使用文字 tab、40px 控件和受限 signal/hard-shadow 重音。Drawer 页头只承担身份与关闭/外链，状态进入概览；creator 作品内使用 `列表｜表现` 二级文字 tab。
- **Data Truth**：Target Inspector 直接消费 single-as-of 投影；queued 不写 running，Known zero 与 Unknown 分开，读取失败不回退为健康或空数据。表现图仅使用已有 qualified/KNOWN lifecycle points；没有分类时明示边界。
- **验证边界**：源码/focused/workspace、新 inspector 隔离 PostgreSQL、项目治理、UI handbook 与 1440×900 浏览器交互/实拍已通过；完整 PostgreSQL 脚本仍有两项未修改的 collection-control 基线失败。PR #205 已合并，本机 API `runtime-main@f0ed68d` 与真实 Targets 页面已核验；worker、插件、平台、真实采集与 Mog 业务验收仍未证明，不能由本机 API 运行替代。

## 2026-09-13 · TARGET-INSPECTOR-PERFORMANCE-001 表现页复核构图

- **范围**：Collection L1 与现有 L2 Split Inspector 不变；creator 的「作品｜表现」以 Mog 提供的静态参考稿重排为控制、覆盖、趋势、复核和证据五层。参考稿不提供运行数值、目标身份或状态。没有新增 Token、CMP、壳层、数据合同或动作。
- **Data Truth**：趋势和摘要仅消费 qualified / KNOWN lifecycle point；空 point 集显示不可成图原因与真实 exclusion，而不以中位数、密度或巡查新增 `0` 填充。header 的持续观察标记区分 active、paused、dismissed 与 inactive，既不把未启用猜为暂停，也不借用 scheduler／Lease／Attempt 状态。
- **Token / Layout / A11y**：新增样式只消费现有 `--lgi-*` Token，所有功能文字使用 11px 及以上的 Token 档位；趋势图例可折行，避免 901px 以上但右侧摘要仍并列的中间宽度横溢。2026-09-13 反馈后，逐篇散点提升为趋势后的常驻同级分析面，再接近期证据表；趋势 SVG 与图例的唯一受控色阶只用既有 body / signal-ink / signal，由低饱和墨色到信号橙表示同一条中位数线的连续读取，不能扩散到背景、正文或状态。散点保留逐点链接与具名替代文字。
- **验证边界**：focused render 反例覆盖 Known(0)、Unknown-only 空点、观察状态分支和 URL 恢复；format、check、UI handbook 与治理检查通过。共享 runtime、插件、外部平台、采集与 Mog 业务视觉验收仍由发布后的独立回执区分。

## 2026-09-13 · TARGET-INSPECTOR-PERFORMANCE-001 同画布趋势／分布切换

- **替代范围**：本条替代上一条“趋势与逐篇分布连续常驻”的页面布局结论。Collection L1 与既有 L2 Split Inspector 不变；同一个表现画布通过 URL-owned `life_chart=trend|distribution` 切换两种阅读，不新增 Token、CMP、主题、全局壳层或动作。`life_grain=month|week` 仅在趋势成立，分布不显示或消费粒度控制。
- **Data Truth**：两视图消费同一时间窗、当前指标、as-of 与 qualified / KNOWN 单篇作品集。趋势以发布密度、时间桶内当前指标中位数与当前窗口逐篇中位基准回答常态是否移动；分布以“发布时间 × 当前指标”的逐篇原始值回答变化由哪些作品构成。Known zero 继续参与当前指标；指标未知仍在 exclusion 中，不以零补点。
- **复核信号**：评论／点赞 `>20%` 只是逐篇分布的 signal 外圈，不改写纵轴或形成第二张指标图。仅评论、点赞已知且点赞大于零才可判；未知计数或 Known zero 进入“讨论率未可判”计数，不被推断为低讨论。巡检新增是独立轻标记，可与高讨论信号共存。
- **LIDS / A11y**：趋势保持单一 Ink 主线与同序列的淡填充，橙红只用于 active tab 与复核 signal；不把渐变当作另一项指标或方向编码。图例、SVG 替代文字与逐点到精确作品的链接随视图同步，右侧复核栏只解释当前图。新增 CSS 只消费既有 `--lgi-*` token。
- **验证边界**：源码 focused render tests 与只读 rate 计算单测覆盖 URL、视图分离、Known zero／Unknown、未可判率与逐点链接。`cargo fmt`、API check、focused API/evidence tests、UI handbook、governance 与 diff 检查的结果以本轮回执为准；本机 source `:3001` 虽 health READY，但 Targets 读取呈“当前未知”，因此实际图表浏览器像素为 `NOT VERIFIED`。未提交、未合并、未刷新 `:3000`，更不证明共享 runtime、插件、外部平台、采集或 Mog 视觉验收。

## 2026-09-07 · OBSERVATION-TARGET-SEMANTIC-FEEDBACK-001

- Targets 目录仍是 L1 `Collection Control`：creator / keyword 共享编号、对象、平台、分组的列起点；creator 的“作品目录”和“详情进度”以既有 12px token 间距分开；最后“操作”列改为剩余空间且右对齐，行内按钮恢复既有主/次组件的 40px 命中区和状态样式。
- 建档回执使用现有 success/warning token 组合；不新增 token、CMP、Scene 或全局壳层。历史目录、待建标准目录、待处理建档任务分别有可读中文，不用颜色或按钮状态替代事实。

## 2026-09-07 · Collection Targets creator 宽表布局回归

- **范围**：只修复 `Collection Control` L1 的 creator 12 列网格；不改变 Pattern、Token 定义、CMP、状态、动作、数据合同或页面结构。
- **原因与表达**：未声明的 `--lgi-space-64` 令 CSS `grid-template-columns` 失效，浏览器退回单列自动布局。改为已声明 spacing token 组合，继续让 creator 身份列有界，避免挤占 platform 及后续事实列。
- **验证边界**：样式源回归断言、creator lifecycle / target list 自动测试及治理检查覆盖；真实 `:3000` 视觉核验、合并和 Mog 业务验收不由本条目证明。

## 2026-09-07 · OBSERVATION-TARGET-DOSSIER-UI-001 目录基线与首层密度修正（已被后续语义修正）

- **来源与范围**：Issue #158 与 Mog 对图妈 Vita 历史 31 篇误读的直接反馈。只修正 Collection Targets 的 current-directory 事实口径和既有首层 creator/platform 栅格间距；不创建版本化“200 篇档案”，不修改 Pattern、Token、CMP、路由或平台访问。
- **已替代的 Data Truth**：后续 `OBSERVATION-TARGET-SEMANTIC-FEEDBACK-001` 纠正了“旧历史一律不显示”的误读。已接纳、target-scoped 且详情完整的历史目录是当前在库事实，必须显示真实目录/详情；它并不自动证明 200 篇或主页末端，也不宣称平台全量。partial/retry 仍不混入受当前标准证明的目录边界。
- **已替代的 Pattern / Page**：面向不满足当前边界证明的恢复动作统一叫“建立标准目录”，不再使用暗示数据被破坏的“重建目录”；完整的历史目录不展示该动作。creator / keyword 栅格和操作组件的后续细化见本日志首条。
- **验证与未证明**：源代码 format、针对列表/抽屉/证据的测试、隔离 PostgreSQL 目录重建和巡查新增证明及治理检查分别运行；真实图妈重建、共享 `:3000`、插件、平台和 Mog 业务验收不由本记录证明。

## 2026-09-06 · MODEL-PI-001 模型设置

- L1 Settings / Governance、一个外层 Form Surface、四区索引及原生 dialog，复用当前 LIDS Token，不新增 3D 或内核一级页。
- 用户明确要求个人菜单→设置，原系统区的未实现 command 槽替换为个人菜单；两个系统槽数量不变。该适配按最新用户授权覆盖旧 command-only 用途，未扩大其它导航整改。
- 当前模型状态、来源试运行、真实调用和用量回执接到评论研究。合成 preview 有独立存储模式说明；未知用量/金额不显示零，人工研究与模型结果分责。
- 集中整改补齐计划表格内「恢复此计划」及确认窗口，保留原来源/配置/次数/额度；重复来源回执可跳转真正所属计划，旧修订有冲突提示。复用既有 dialog/表格，HTTP 与隔离状态验证通过，完整浏览器验证仍由根代理完成。
- [变更清单](../changes/model-pi-001-ui-change-manifest.md) 和 [验收记录](../acceptance/model-pi-001-acceptance.md) 列出代码/真实 SDK/隔离 PG/HTTP 与根代理浏览器、Mog 验收的差别。

## 2026-09-04 · OBSERVATION-TARGET-DOSSIER-UI-001 creator 档案工作区替代旧控制表

- **来源与范围**：Issue #158 与 Mog 最新确认把观察目标定义为“持续观察的创作者及其不断生长的作品档案”。旧内容工作台只贡献生命周期曲线与深度建档的行为对标；V4 synthetic 文件只贡献内容数据库式密度。事实、权限和视觉标准仍归 Intelligence 当前合同与 LIDS。
- **Pattern / Page**：Collection L1 仍为五个子面，creator drawer 保持受限 L2；首层改为 creator / keyword 各自适用的宽表，creator 工作区收敛为 `概览｜档案｜巡查`，keyword 只保留 `概览｜巡查`。默认目录不再显示批量复选框、总健康分、工程回执或常驻双动作。
- **视觉方向**：采用白色连续阅读场、黑灰硬结构和内容数据库密度；真实头像与生命周期散点成为内容，Signal 只用于主要行动、选中和最近巡查新增。未增加 token、CMP、颜色、字体、图标库、渐变或移动端分支。
- **Data Truth**：stable Work 去重目录、详情进度、live deep-archive Lease、成功巡查时间、目录关联/作者确认和 latest accepted patrol 分责。空心点、实心点与 Signal 外圈分别表达这三个事实；UNKNOWN 不画成 0。复合指标、创作者内分位、滚动中位线、算法版本和扫描工程回执退出默认 UI。
- **真实行动**：“建立档案”不再只是文案。它要求至少 200 Works 的有效授权，冻结 200 篇主页链接扫描上限与版本 marker，再由 worker 以每批 3 篇推进详情、最多 30 条评论、媒体及已授权数据化；旧无 marker 的 WorkOrder 不自动扩权。
- **支持与证明边界**：只验收 1440 CSS px 桌面全屏；1280/390/手机和小于 13 寸屏幕不进入本包。自动、隔离 PostgreSQL、浏览器与 exact-head 审查以 [`../acceptance/observation-target-dossier-ui-001-visual-acceptance.md`](../acceptance/observation-target-dossier-ui-001-visual-acceptance.md) 为准。共享数据库、`:3000`、插件、真实平台、部署、merge 与 Mog 验收不由本记录证明。
- **完整清单**：[`../changes/observation-target-dossier-ui-001-ui-change-manifest.md`](../changes/observation-target-dossier-ui-001-ui-change-manifest.md)。Issue #148 的四职责/算法展示只保留历史演进，不再约束当前观察目标页面。

## 2026-09-03 · COLLECTION-LIFECYCLE-001 creator 生命周期成为目标抽屉默认核心

- **来源与范围**：Issue #148 与 Mog 最新决定将 Evidence 呈现留在 Corpus；creator target 抽屉只做生命周期决策面与精确跳转，不保留 Evidence tab，不引入监控价值、机会评分或趋势预测。
- **Pattern / Page**：Collection L1 Operations 不变，右抽屉保持受限 L2。四个职责为概览、基线、巡检策略、追踪；creator 概览以真实发布时间散点为单一视觉核心，keyword 明示不适用。完整清单见 [`../changes/collection-lifecycle-001-ui-change-manifest.md`](../changes/collection-lifecycle-001-ui-change-manifest.md)。
- **Token / Motion / A11y**：新增 page-local `target_drawer.css`，只消费 `--lgi-*`；review remediation 在唯一真源新增 `--lgi-focus: #335e72` 并同步 132 项文档镜像，用主题文档级 2px outline/2px offset 替代页面自己的 Signal/Ink 焦点和 SVG `outline:none`。owner 显式覆盖 link、button、input、select、textarea、summary 与 `[tabindex]`，自然 specificity `0,3,0` 可越过后加载旧规则，也能命中作为 `.v7-app` sibling 的固定 drawer，不使用 `!important`。控件至少 40px，SVG 点用透明 `r=6 + 24px stroke` 提供 36px 有效 hit target，并以独立可见点、4px focus stroke/scale 和具名链接提供非纯颜色提示；reduced-motion 有显式分支，390px 收为单栏。Corpus/Collection 代表控件和 lifecycle SVG 已在 1440/390 真实键盘下读取一致 computed focus；完整辅助技术认证与 Mog 验收仍不由本卡宣称。
- **Data Truth**：stable author exact match、qualified platform epoch、field-wise latest KNOWN 和 `KNOWN 0` 由服务端读模型负责。90 天是 `Asia/Shanghai` 90 个含首尾日历日。分析版本冻结为 `creator-percentile-v1` 与 `trailing-5-work-median-v1` / 5；前端不重复计算。
- **读取预算**：最多扫描 2000 + 1 探针并显式回执；Collection 只在 creator overview 执行，baseline/patrol/trace 与 keyword 不读，独立 API 保留。
- **证明边界**：branch 自动与隔离 PostgreSQL 证据以 [`../acceptance/collection-lifecycle-001-visual-acceptance.md`](../acceptance/collection-lifecycle-001-visual-acceptance.md) 为准。未应用 shared migration、未切 runtime、未部署、未访问外部平台、未取得 Mog 业务验收。

## 2026-09-03 · DESIGN-011 生产流与执行工位落地 v7（首个运行时迁移）

- **来源与事项**：Mog 于 2026-09-02 指定「用这个设计标准修改 `/collection/operations`」，09-03 指定「同样的，构建执行工位页」并裁定先接能力矩阵数据再做页面。这是 DESIGN-010 之后 v7 的**首个运行时落地**。
- **表达迁移**：两页删除全部英文描述性标签（阶段副标题、`SYSTEM CONCLUSION`、`LIVE OBSERVATION`、`SEMANTIC EVENTS ONLY` 等），保留 `DISCOVER`/`CHANGE`/`EXCEPTION` 与能力机器名——它们是数据合同的字面取值（`LANG-05` 第 2 类）。字号下限提到 11px、字重收到 400/600/700、按钮中文改回 Sans。
- **材料**：`/collection/operations` 深色实时流顶边使用 `M-05` 低亮传感面（`steps()` 按格跳）；`/collection/runtime` 已登记工位行使用 `M-03` 序列索引轨。两处均只在边缘，待认领安装不画轨——它们尚未成为身份。**第一版 `M-05` 曾扫过正文并被本次自查推翻**，改为裁剪在顶部 72px 内，该约束已写入测试。
- **新读模型**：`crates/evidence` 新增 `StationCapability` / `CapabilityState` / `read_station_capabilities`，合成声明、成功与失败三源。三条判断分开：未验证不是降级、执行失败不是能力缺陷、读不到不是没有。
- **Token**：不改任何 `--lgi-*` 值；页面局部 v7 阶梯以 `--c-*` 承载，属 `DESIGN-010-UI-EX-01`。`ADR-04` 信号色拆分仍未落地——页面样式表的无色值护栏拦下了它，且拦得对。
- **未做**：共享壳层未动，`ADR-10` 的页头/上下文行区位收口仍是欠账；同一样式表中目标页、抽屉与检视面板的 10px 与 800 字重未迁移，护栏测试范围因此只到本页类名。
- **验证与边界**：72 项测试通过、`cargo fmt` 干净、token 镜像 131/131、能力矩阵 SQL 先在生产库直连验证再写入 Rust、1440 下浏览器计量（<11px 元素 0、非法字重 0、无横向滚动）。**390×844 未实测**（resize 本轮不生效）；降级分支在真实库中无数据，只有单元测试覆盖。完整清单见 [`../changes/design-011-collection-surfaces-v7-ui-change-manifest.md`](../changes/design-011-collection-surfaces-v7-ui-change-manifest.md)。

## 2026-09-02 · DESIGN-010 LIDS 升级到 v7.0（规则层换代，运行时未迁移）

- **来源与事项**：Mog 于 2026-09-02 指定 `/Users/moglenny/Downloads/linggan-design-system-v7.html`（SHA-256 `1093462bcea81c10584e18119a92ae6b51645d1b480667e161d993e863ca4e36`），并明确裁定本次范围为「只改治理文档 + 记录迁移欠账」。来源登记见 [`../reference-register.md`](../reference-register.md) 的 `REF-DS-V7-001`。
- **规则层**：新增 [materials.md](materials.md)（8px 采样点阵六态、材料预算 70/20/10）、[shell-zones.md](shell-zones.md)（页头 3 区 + Context Bar 4 区冻结、静默区保护）、[data-boundaries.md](data-boundaries.md)（四态渲染契约，组件准入条件）、[decisions.md](decisions.md)（11 条 v7 ADR + 2 条项目 ADR）。修订 system / tokens / primitives / patterns / language-policy / README / agent-execution-guide 及外层三份治理文件。
- **Token**：[tokens.md](tokens.md) 重构为两层——§2 v7 三层架构（L1 PRIMITIVE → L2 SEMANTIC → L3 GEOMETRY，组件只引用 L2/L3）为**目标态与新工作选值依据**；§3 的 127 项 `--lgi-*` 仍是**唯一运行时值源**，逐字节未改。§4 给出差异表与五步迁移顺序（加不减 → 换值 → 换引用 → 删别名 → 材料与密度）。
- **语言**：新增 `LANG-05` Mono 预算——英文只允许机器事实、系统状态枚举、结构编号三类；**描述性标签一律只用中文，不配英文对照**。判据为"这个词在数据合同里是不是一个字面取值"。这是对 `LIDS-LANG-001` 的执行口径收紧（`ADR-P01`），不是推翻。
- **不改事实**：本次没有修改任何 `.rs` / `.css` / `.js` / 测试；没有改动路由、权限、状态判定、数据合同、API、采集或媒体。所有状态轴定义、`PARTIAL + VALID`、`UNKNOWN` 语义原样保留。
- **已登记欠账**：运行时 token（信号色/字重/字号下限/线宽/圆角）、材料与密度机制、壳层区位违规（`shell.rs`）、文案中英对照（`shell.rs` / `evidence_page.rs`）、静默区与英文预算的自动检查——五项均未实现，各需独立受控事项。清单见 [`../changes/design-010-lids-v7-adoption-ui-change-manifest.md`](../changes/design-010-lids-v7-adoption-ui-change-manifest.md) 第 3 节。
- **命名冲突**：运行时 CSS 类前缀 `v7-*` 来自 `REF-V7-001`（Evidence Library 页面 Gold Master，2026-08-24），与设计系统 v7.0 无关，已在两处登记以防误读。
- **验证与边界**：Token 镜像逐项比对 127/127 相等；`scripts/verify-ui-design-handbook.sh` 通过（含本次新增的门禁项）。它**不证明**任何页面呈现发生变化、不证明现有页面符合 v7、不证明来源文件标注的对比度在本项目实际组合下全部达标（本次未复算），也不构成 Mog 的视觉验收。LIDS 成熟度仍为 `PROPOSED`。
## 2026-09-02 · TOPIC-WORKSPACE-REAL-001 首个真实 L2 Topic 工作区（当前主线整合）

- **来源与事项**：Mog 明确允许关闭 #133 后推进下一大阶段；Issue #112、PAGE-TOPIC-WORKSPACE-001 与 TOPIC-WORKSPACE-REAL-001。
- **Token / Primitive**：不新增 token；复用白色 canvas、煤黑结构、Signal 当前选择、semantic soft 状态、共享 focus 与 100–160ms motion。Reduced Motion 关闭过渡。
- **Component / Pattern / Page**：新增 page-local Definition Header、Classification Lens、Frozen Material Row、Work Resource Inspector 与 Source Boundary；均不晋升全局 CMP。页面为 L2 Research/Analysis，旧 PAGE-TOPIC-001 合成静态 reference 保持独立。
- **Data Truth**：真实只指本地数据库中不可变 Definition/Run/Pack/Receipt，不等于正式知识。页面固定显示 `PROVISIONAL`、`HUMAN_ADJUDICATED` 和来源不得外推；未读取不写成空/零，角色材料数只描述 frozen pack。
- **整合约束**：当前 main 的 `0027` 已归 unified media，因此 Topic 新增 migration 为 `0031`；最终验证不得把旧 Draft 的浏览器/数据库证据外推至当前 head。完整清单见 [`../changes/topic-workspace-real-001-ui-change-manifest.md`](../changes/topic-workspace-real-001-ui-change-manifest.md)。
- **Shared Shell 窄屏收口**：Topic 作为第五个一级职责暴露了 `≤640px` 将每项固定为 86px、要求横向滑动才能到达「采集」的真实可达性缺口。共享 `shell.css` 改为 auto-fit 72px 最小列的网格，并取消 parent global row 的遗留横向 scroller：390px/430px 同屏展示五项，320px 和未来职责自动换下一行。保留真实 link、disabled 语义、中文职责、状态和 visible focus；仅移动端隐藏英文技术旁注。source regression 与隔离浏览器 1440/430/390/320、exact stylesheet 复验均通过；不证明 shared runtime、外部 Chrome/完整辅助技术或业务验收。

## 2026-08-31 · XHS-MEDIA-AUTHOR-EVIDENCE-001 作者头像与身份分栏

- **来源与事项**：Mog 要求重新采集持续进量样本，并让 Evidence Library 显示全部已取得媒体；作品作者与监控目标必须拆开，作者区显示头像。
- **Token / Primitive**：不新增 token。头像使用既有圆形身份锚点和 `INLINE_SAFE` 状态；两类事实使用现有 1px 结构线、中文主标签和技术状态副标。
- **Component / Pattern / Page**：Corpus Explorer 与 Split Evidence Inspector 不变；Work Resource row 增加 `creator fact / target fact` 两个明确区块，媒体 Inspector 将 avatar 与内容媒体放在同一连续资源列表。
- **Data Truth**：头像只来自统一 `media.avatar` 的受控本地句柄；远程 URL、监控目标头像和目标显示名都不得替代作品作者事实。缺失保持 `NOT_OBSERVED/FAILED/RESTRICTED`。
- **验证与边界**：JS/Rust 聚焦测试与隔离 PostgreSQL author-avatar 链已通过；真实 Chrome 重载、目标作品新 Package/Receipt/媒体物化与 Mog 视觉验收尚未完成。

## 2026-08-31 · WORK-RESOURCE-READ-001 共享作品资源与三种资料库排版

- **来源与事项**：Mog 要求所有 Intelligence 页面共用 Media V2 上方的封面/作者/时间/标题资源读取，不允许每页各走 API；Issue #110。
- **Token / Primitive**：不新增 token。新增三段 layout selector，使用现有黑白/Signal、40px 可点击目标、focus-visible 和 160ms 颜色/按压反馈。
- **Component / Pattern / Page**：同一 Work Resource row 形成 Research、Table、Cover 三个纯展示变体；Corpus Explorer、当前选择、Inspector、查询和数据回执不变。`layout` 与状态 `view` 分离，切换布局不重新读取。
- **Data Truth**：作品作者与监控目标分开，只有平台作者 ID 匹配才可写 `MATCHED`；时间新增 `SOURCE_TEXT_ONLY`，禁止相对文本或观察时间冒充精确发布时间。
- **验证与边界**：交付分支已加入 JS/Rust 静态与自动门禁；真实 Chrome 桌面/窄屏、真实签名详情字段、部署和 Mog 视觉验收仍未完成。完整清单见 [`../changes/work-resource-read-001-ui-change-manifest.md`](../changes/work-resource-read-001-ui-change-manifest.md)。

## 2026-08-21 · LIDS v2.0 导入为 Linggan 的权威设计表达标准

- **来源**：Mog 明确指定 `/Users/moglenny/Downloads/Linggan_Intelligence_Design_System_v2.0`；主源文件校验值与不继承清单见 [README.md](README.md)。
- **吸收**：`Token → Primitive → Component → Pattern → Page`、L1/L2/L3、暖灰/煤黑/Signal 的语义、Sans/Mono 分工、状态五轴、`PARTIAL + VALID`、动效/场景/响应式/a11y 边界、Agent 决策树、规格门与变更纪律。
- **项目适配（截至 2026-08-21）**：把来源包的运行时代码、目标目录、React/Three/Blender 路线、V3 模拟数据/状态和静态原型降级为未来候选；当时未创建 Web 应用、主题 CSS、组件、真实数据或部署。LOCAL-001A 的后续受限实现见本日志 2026-08-24/25 条目。
- **替代**：DESIGN-002 原有 DS-001–DS-007 的局部“暗色 Acid/新粗野主义”视觉值被 LIDS Token 与 L2 Pattern 取代；原有 Evidence/Boundary 和组件晋升的事实边界仍保留，并与 LIDS Data Truth 对齐。
- **影响**：全项目未来 UI Agent；DESIGN-002 Topic 静态参考页、其 PAGE 规格、执行合同、设计治理、模板、索引和检查脚本。
- **验证目标**：手册链接/状态头、LIDS 检查、Topic 专项检查、项目治理检查、静态浏览器走查与独立审查。
- **未证明（截至 2026-08-21）**：LIDS 当时仍为 `PROPOSED`；没有真实 L1/L2/L3 页面、运行时 Token、组件、真实数据/Agent 状态、3D 资产、性能、部署或 Mog 最终视觉验收。LOCAL-001A 的后续受限实现不改变该历史记录。

## 2026-08-24 · LOCAL-001A 首个运行时 L1 Evidence Library token 映射

- **来源与事项**：Mog 确认的 LOCAL-001、`REF-V7-001` 页面 Gold Master、Issue #25、`PAGE-EVIDENCE-001`。
- **实际实现**：`apps/api/src/local_web/lids_tokens.css` 是当前唯一完整的运行时 token 值编辑源，完整承载 `LIDS-TOK-001` 的 107 个 `--lgi-*` token；`docs/design/lids/tokens.md` 是从该源单向同步的版本化规范与校验镜像，不能独立改值。`apps/api/src/local_web/evidence_library.css` 只能消费而不声明 token。Rust 测试逐项对照 107 个名称和值，没有引入第二个全局主题、组件库或视觉前缀。
- **页面组合**：本页采用 `LIDS-PAT-001` 的 L1 `Corpus Explorer`，嵌入右侧受限 `Split Evidence Inspector`。原声、材料、搜索、动作和真实状态均未实现；首屏唯一视觉核心是来源不足的材料边界说明。
- **局部例外**：`LOCAL-001-UI-EX-01` 仅为 V7 三栏比例保留 216px 左 rail、440px right inspector 与 2px 中央结构线。例外在 PAGE/Manifest 中可查询，未推广为 token 或跨页组件；以后第二页面复用前必须重新审查。
- **Data Truth**：页面只表达 `SOURCE_INCOMPLETE`、`NOT_CONNECTED`、`UNKNOWN` 与“本页没有可展示的已接纳材料”。这些不表示系统库为 0、平台不存在内容、捕获失败或任何趋势；没有模拟数值、`LIVE`/`FRESH`、假按钮或前端回执。
- **验证目标**：Rust route test、loopback HTTP、指定视口浏览器走查、CSS token/a11y 检查和治理检查。实际结果与未证明边界记录在 `ACC-EVIDENCE-001`。
- **未证明**：LIDS 整体仍为 `PROPOSED`；本项不证明 Materials read model、真实 Evidence/Observation/Capture、数据库、插件/真实平台、媒体、OCR/ASR、跨页组件、部署或 Mog 验收。

## 2026-08-25 · LOCAL-001A LIDS 当前状态对账

- **原因**：独立 Spec 审查发现 README 与 system 仍将 2026-08-21 的“没有 Web runtime/主题 CSS/真实页面”写成当前事实，与已实现的 LOCAL-001A 相冲突。
- **当前受限事实**：已有 loopback Rust host、一个 `/corpus/evidence` Evidence Library 页面，以及 `apps/api/src/local_web/lids_tokens.css` 这一唯一运行时 Token 值编辑源。
- **仍未证明**：没有真实数据或 Materials read model、通用组件库、L2/L3 页面或场景、Agent runtime、部署、完整产品 Web 或 Mog/业务验收。LIDS 成熟度仍为 `PROPOSED`。
- **治理与验证**：README/system/tokens 的当前表述按这一区分同步；Rust 测试精确核对 107 个运行时 Token 名称和值，页面 CSS 不声明 Token。此对账不改页面代码、路由、视觉数值、数据或任何运行能力。

## 2026-08-25 · Issue #29 Evidence Library V7 精确视觉/骨架例外

- **直接授权与替代范围**：Mog 明确要求 `/corpus/evidence` 按 `REF-V7-001` 完整 1:1 复刻。该直接授权只替代旧 `LOCAL-001-UI-EX-01` 中“仅 216px / 440px / 2px、其余使用 LIDS 页面值”的窄例外；不改变任何数据、权限、行动或跨页设计决定。
- **实际页面范围**：`evidence_library.css` 以页面局部 `--v7-*` 变量承载 V7 的桌面 78px + 50px = 128px 全局页头、216px rail、440px inspector（响应式 420/380px）、全白底、黑色硬线、`#E8003F` 全局强调色与 `#EF4F25` Evidence 强调色，连同 V7 的页头、视图/筛选/查询、FACT LAYER、三栏和 Inspector HTML 骨架。桌面为 `100vh` 固定工作台并将滚动限制在 Results/Inspector；移动端首行改为自动高度以容纳换行导航，随后顺序折叠并使用页面滚动。
- **非继承边界**：V7 的模拟运行状态、计数、帖子/评论/转录、时间、引用及成功回执不进入本页；未接通控件保留位置和视觉，但均 disabled/`aria-disabled` 且无副作用。中央区只显示 `SOURCE_INCOMPLETE` / `NO_ACCEPTED_MATERIAL_AVAILABLE`，Inspector 仅显示 no-selection / `UNKNOWN`。
- **不形成第二套系统**：`lids_tokens.css` 未修改，仍是唯一全局 `--lgi-*` token 值源；`--v7-*` 不可被其他页面、CMP 或后续页面当作全局 token 使用。需要复用时必须重新进行 LIDS/页面审查和独立授权。
- **验证与未证明**：四个指定 Chrome 视口的本地截图/几何、route/HTML 的诚实边界测试、format/clippy/test 与治理检查记录在 `ACC-EVIDENCE-001`。本项只证明本页静态视觉与禁用骨架；不证明 Materials read model、Evidence、数据库、插件、媒体/OCR/ASR、Agent、部署或业务验收。

## 2026-08-25 · PLUGIN-001 自有 Browser Producer popup 的受限 L1 表达

- **来源与事项**：Mog 已确认 Linggan 及其 Browser Producer 是同一独立系统；Issue #33 / `PLUGIN-001` 与 `LOCAL-001C0-DISCOVERY-BOUNDARY-V1`。
- **实际实现**：新 popup 为受限 `L1 / Settings / Governance` Surface，仅呈现 `LINGGAN / version`、固定 `localhost:3000`、最小 `/health` 可达性、`NOT_AUTHORIZED` 和 `NOT_CONNECTED`。构建包从 Linggan 当前唯一 runtime token 值源复制 `--lgi-*` token，popup CSS 只消费该副本，不新增全局 Token 或 CMP。
- **安全边界**：Manifest 只有 `http://localhost:3000/*` host permission 且 `permissions=[]`；无内容脚本、Cookie、下载、脚本注入、平台 host、真实材料、媒体或外部链接。Discovery 控件持续 disabled；本卡没有 ingress、Evidence 接纳、浏览器加载或平台采集。
- **验证与未证明**：source/release 静态检查将记录在 `ACC-PLUGIN-001`；浏览器视觉走查、真实 health、安装加载、Discovery Package 接纳、平台/账号/媒体/OCR/ASR 和业务结果仍是 `NOT VERIFIED`，不得由安装包存在推断为已接通。

## DESIGN-003 · 视觉基线换向 v2.0 → v3.0（2026-08-25）

**触发**：Mog 在设计评审会话中逐项确认新的视觉方向，并要求手册与运行时页面同步到该方向。原基线的「暖灰纸面」与 Mog 的实际选择冲突。

**Token 层**：107 项 → 117 项。

| 项 | 原值 | 新值 | 理由 |
|---|---|---|---|
| `--lgi-canvas` | `#ecebe6` | `#ffffff` | Mog 明确选择纯白、否定暖灰 |
| `--lgi-canvas-low` | `#deddd7` | `#f7f8f8` | 纯白体系层次向下做 |
| `--lgi-canvas-sunken` | 不存在 | `#eef0f1` | 新增第三层面 |
| `--lgi-ink` | `#121211` | `#111315` | 暖黑换冷黑，与纯白同调 |
| `--lgi-body` / `muted` / `ghost` | 暖灰三级 | 冷灰三级 | 同上 |
| `--lgi-success*` | 橄榄绿 | 祖母绿 `#05674a` / `#0e9e6e` | Mog 指定祖母绿 |
| `--lgi-warning*` | 芥黄 | `#a67a04` / `#eaaa05` | Mog 指定蒙德里安参考色 |
| `--lgi-danger*` | `#9e2517` | `#a42001` | 同一参考的砖红 |
| `--lgi-info*` | 靛蓝 `#345a6f` | 灰蓝 `#42555a` / `#8d9a9d` | 原靛蓝在这套配色中过于跳脱 |
| `--lgi-unknown*` | 不存在 | 三项 | 未知此前无专属角色，被迫借用 ghost |
| `--lgi-danger-dot` / `--lgi-info-dot` | 不存在 | 新增 | 五轴对称，每轴都有文字 / 填充 / soft |
| `--lgi-shadow-brutal` / `-lg` | 不存在 | `4px 4px 0` / `7px 7px 0` | 粗野重音需要实色硬阴影 |
| `--lgi-mesh-fine` / `-major` | 不存在 | 两级点阵 | L1 背景纹理改用点阵而非划线网格 |

**规则层**：

- `system.md` 视觉定义改写为「纯白台面上的精密情报基础设施」，新增「新粗野主义作为重音，不作为底色」及每屏 8 处上限；L1 纹理条款由「关闭或极弱」改为「允许点阵测量场，禁止渐变/光晕/噪点/动态纹理」。
- `primitives.md` 新增线条六原则；Primary 由 Ink 实底改为 Signal 实底 + Ink 边 + 硬阴影；hover 位移由全面禁止改为受限允许（仅重音元素、固定位移量、模糊阴影仍禁止）；新增禁用态规范；状态标签改实心填充并冻结填充色与文字色的分工。

**运行时**：

- `evidence_library.css` 不再 author 任何色值，全部经 `--v7-*` 别名解析到 LIDS token；测试新增断言禁止页面 CSS 出现 `#`。
- 修复 11 处此前解析失败的变量引用（`--lgi-surface-base`、`--lgi-text-primary` 等从未在 token 中定义），这些引用全部位于真实 Discovery 卡片样式上，此前因窗口无卡片而未暴露。
- 第二签名色 `#e8003f` 退役，签名色收敛为 `--lgi-signal`。
- 可见边框由 194 条降至约 100 条，剩余部分几乎全部落在可交互元素上。

**测试同步**：`runtime_token_source_matches_the_full_lids_baseline` 的数量断言 107 → 117；`evidence_page_keeps_the_v7_shell_and_three_column_geometry` 中被锁定的 `--v7-brand-red:#e8003f` 与 `--v7-red:#ef4f25` 两条常量断言，改为断言页面消费 token 且第二签名色已退役。这是基线换向导致的合同更新，不是为通过测试而放宽断言。

**未处理**：`.v7-*` → `.lgi-*` 类名迁移、键盘可达性与 ARIA 补全、状态色图例，均另立卡。

## DESIGN-004 · Corpus Rail 面层差异化与选中态重音（2026-08-25）

无 Issue，Mog 在 DESIGN-003 合并后于会话中直接指定。范围仅限 Evidence Library 左侧 216px rail 的呈现层。

**Token**：无新增、无修改。全部值经既有 `--v7-*` 别名解析到 `--lgi-*`。

**规则层**：无改写。本次是对既有规则的首次落地应用——`primitives.md` 线条原则 2「面代替线」与「粗野重音的位移」中「当前选中项」一项，此前均未在运行时页面兑现。

**运行时**（`evidence_library.css`）：

- rail 面由 `--v7-white` 改为 `--v7-gray` + 56px/14px 双层点阵，与上下文行同款，两者垂直连续。
- 导航项常态底改为 `transparent`；hover 改为白实面浮起并把序号转 signal。
- 导航序号 9px → 11px Mono `600`，常态色 `--v7-muted` → `--v7-ghost`。
- 选中项追加：6px 棋盘像素纹理（自右边缘向左四档离散衰减、固定 84px 宽）、`--v7-brutal` 实色硬阴影、`translate(-2px,-2px)` 位移。
- `prefers-reduced-motion:reduce` 下新增关闭选中项常驻位移。
- 栏脚上边线 `--v7-line` → `--v7-line-strong`。

**离散衰减而非渐变**：纹理密度用四档 hard stop 分级，不使用连续渐变，以同时满足像素/ASCII 语言与 `system.md` 的 L1 渐变禁令。宽度用绝对像素而非百分比，使 rail 在移动端展开为全宽区块时纹理不等比放大。

**参考图取舍**：Mog 提供的胶囊按钮参考图只吸收像素纹理。厚胶囊圆角、模糊阴影、暖棕底色三项不采纳，理由是与 DESIGN-003 中 Mog 本人的确认直接冲突，依据分别为 `system.md` §4.5、`primitives.md` 位移条款与 DESIGN-003 画布换向。

**重音计数**：不新增重音元素，仅强化已有一处；当前屏计 7 处，仍在 8 处上限内。

**测试**：无断言变更，`cargo test -p linggan-api` 8 passed / 5 ignored（5 项需隔离 PostgreSQL 证明库）。页面 CSS 内 `#` 计数保持为 0。

**未验证**：hover 与 `:focus-visible` 规则已写入但无法触发——五个导航项当前全部 `disabled`。待任一路由接通后补验。

**未处理**（沿用 DESIGN-003 遗留）：`.v7-*` → `.lgi-*` 类名迁移、键盘可达性与 ARIA 补全、状态色图例。

### DESIGN-004 修订 · rail 面层回滚（2026-08-26）

Mog 在实际页面上判定：rail 复用与上下文行完全相同的点阵纹理后，原本独一份的「上下文层」不再特殊，页面层次被压平。按用户最新确认优先，面层回滚。

- `.v7-side` 背景由 `--v7-gray` + 双层点阵回到 `--v7-white`；rail 与主工作面继续由 2px 结构线分隔。
- `.v7-side-nav:hover` 由「白实面浮起」回到 `--v7-gray`（白面上白色浮起不成立）；序号 hover 转 signal 保留。
- `.v7-side-foot` 上边线由 `--v7-line-strong` 回到 `--v7-line`。
- 选中态的像素纹理、实色硬阴影、`translate(-2px,-2px)` 位移，以及序号 11px / ghost、`:focus-visible`、reduced-motion 处理全部保留——它们各有独立规则依据，与面层选择无关。

**实测结论**：`primitives.md` 线条原则 2「面代替线」不能无条件套用。当页面已存在一条以纹理承担语义的横向带时，纵向面复用同款纹理会消掉那一层的唯一性，减少的线条不抵消失的层次。此处记为本页一次实测观察，**不构成新规则**；是否需要在 `primitives.md` 中补充「纹理的唯一性」约束，留待出现第二个候选面时再判断。

**验证**：`cargo test -p linggan-api` 8 passed / 5 ignored；1440×900 真实 Chrome 复核。1280×800 与 390×844 未复拍（本次仅色值变更，不涉及几何）。

## DESIGN-005 · Collection Workspace 落地（2026-08-26）

无 Issue，Mog 直接指定落地 `REF-V4-001`（Collection Workspace V4 Gold Master），并在实施前裁定三项：真实产品页而非演示页、保留深绿终端配色、按方案 B 清掉纯冗余。

**Token**：117 → 127。新增 `--lgi-stream-*` 十项（bg / bg-raised / ink / muted / dim / mint / cyan / amber / red / line），承载全产品唯一的深色面。同一提交同步 `tokens.md` 镜像与数量断言。

**规则层**：无改写。新增一条页面级长期例外 `DESIGN-005-UI-EX-01`：深色实时观察流与 `system.md` §4.10「不使用黑底荧光绿终端」冲突，Mog 明确保留；仅限运行态 / NOW 右栏，禁止扩散。V4 原型在该面大量使用 7–9px 功能文字，本次一律提到 DESIGN-003 已确认的 11px 下限，仅时间戳与刻度保留 9px。

**共享层抽取**：全局页头此前写死在 Evidence Library 的页面模板里。做第二个页面之前先抽出 `shell.rs`（页头生成）与 `shell.css`（reset、页头、上下文行、216px 导轨），两页共用。抽取前后 `/corpus/evidence` 的 HTML 输出**逐字节一致**，导轨的英文标注由硬编码的 `CORPUS` 改为 `attr(data-readout)` 参数化。

**运行时**：新增 `collection.rs`（五个子面渲染）、`collection_workspace.css`（页面层，整文件无字面色值）、`collection_workspace.js`（仅抽屉 tab / 宽度 / Escape）。导航状态全部由服务端路由渲染，首屏即正确视图——不复制原型「先画错页再客户端切换」的行为。

**深色面的两次修正**：首版把结构线 token（20%）误用作扫描线与环境亮，整片泛绿；按 Gold Master 的 2.5% / 7% 强度改用 `color-mix` 从 mint 派生后复拍通过。深色面上的小标题从 dim 提到 muted 才可读。

**自我复查记录**：首版实现在观察生产流里给六个阶段各写四格 `UNKNOWN`，一屏 24 个——正是本次审核批评 Gold Master 的那类冗余。已改为每阶段一格加一句区域级说明。

**测试**：新增 6 项 Collection 专属断言，其中两项是防伪造：真实路由不得出现 Gold Master 的任何示例数字（146 / 07-08 / 具体博主名 / 具体指标），以及不得出现 `>0<`。全量 14 passed / 5 ignored。

**未处理**：`.v7-*` → `.lgi-*` 类名迁移；键盘可达性完整覆盖；目标行与两张图表的几何（无数据可渲染）；1440 / 1280 / 移动视口复拍。
## 2026-08-26 · Issue #53 Popup Startup Recovery 的 L1 token 消费

- **来源与事项**：Issue #53 / Draft PR #54、`PAGE-PLUGIN-001` 与 `PLUGIN-POPUP-RECOVERY-001`。此前 v0.4.0 工具栏 popup 在初始渲染缺少 formatter import，真实用户点击时呈现空白；本事项只修复这个启动故障与其诚实失败路线。
- **实际实现**：`plugins/linggan-intelligence-browser/webpack.config.cjs` 在 build 时将唯一 runtime 值源 `apps/api/src/local_web/lids_tokens.css` 复制为 release 内的 `themes/lids-tokens.css`，`src/popup/popup.html` 加载该副本。新 `popup-startup-failure*` CSS 仅消费 `--lgi-*`；没有新增 token 值、全局主题、CMP、Scene 或 Motion。
- **LIDS 组合**：局部 `L1 / Settings / Governance`，沿用 `InstrumentSurface` 和 L1 可读错误反馈；2px border 是 LIDS 已批准的结构线，而非新的视觉数值。文案只表达 `UNKNOWN` 与“该提示没有发起新的采集或传输”，不把失败页面写成 host、receipt、Evidence 或平台状态。
- **验证与边界**：受控首次渲染 harness 复现 v0.4.0 缺失 import 的 `ReferenceError`，并验证 v0.4.2 正常或 fallback 输出；build/release verifier 要求 token CSS 同时存在于 `dist` 和 ZIP。它不证明 Chrome 像素画面、辅助技术、真实浏览器加载、平台、Cookie、Discovery、接纳、媒体或研究结果。

## 2026-08-26 · LOCAL-001D Evidence Library 未知发布时间的默认读取表达

- **来源与事项**：Issue #62、`LOCAL-001D`、PAGE-EVIDENCE-001、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1` 与 Mog 的明确裁定。真实 Canary 的聚合事实表明：已接纳 discovery 卡片可能没有来源发布时间；此前 URL 缺省读法等同隐式 `PUBLISHED:30D`，会使这些材料在页面中完全不可见。
- **Data Truth 修订**：URL 缺省时页面使用命名的内部 `latest_accepted_discovery` 视角，而不是发布时间窗口。卡片可显示 `PUBLISHED_AT UNKNOWN`，并明确不使用首次发现、观察、接收或重放时间代替来源发布时间。显式 `last_7_days` / `last_30_days` 保持严格 published-time 筛选，未知对象继续排除并单独计数。
- **运行时表达**：只增加既有 Unknown token 的 page-local label；不新增全局 token、Primitive、CMP、Scene、Motion、按钮、筛选器或跨页模式。L1 `Corpus Explorer` + embedded L2 Inspector 的既有组合和 V7 几何不变。
- **验证与边界**：合成 contracts、runtime/fallback PostgreSQL proof、API/页面 render tests 验证 default/explicit view 分离、未知标签和无替代日期。本事项不读取或改写真实 Canary 材料，不证明真实浏览器呈现、平台、采集、媒体、OCR/ASR、趋势或用户验收。

## 2026-08-26 · DESIGN-007 中文优先的 Evidence Library 表达

- **来源与事项**：Issue #65、`LIDS-LANG-001` 与 Mog 的明确裁定「中文为主，英文只用来装饰或作为注释」。本项只审查并修订 `/corpus/evidence`；它不构成其它页面已完成翻译或可读性验收。
- **规则层**：新增 `language-policy.md`，固定用户理解必须由中文独立承担。英文只可作为品牌/固有名或紧邻中文的等宽技术旁注，不能单独作为按钮、筛选、状态、空态或错误处置。该规则保持 `UNKNOWN`、`NOT_ACQUIRED`、`DISCOVERY_ONLY` 等数据边界原义，且不翻译原始用户材料。
- **页面落地**：Evidence Library 的导航、筛选、读投影、严格发布时间窗口、Discovery 卡片、Coverage、空态和 Inspector 均替换为中文主表达；`PUBLISHED_AT UNKNOWN`、`MEDIA NOT ACQUIRED`、`ACCEPTED RUNTIME MATERIAL`、`OBSERVED / QUOTA` 等保留为紧邻中文的技术键。默认「最新已接纳」及显式窗口显示「近 7 天／近 30 天」，不让 `7D/30D` 单独承担筛选含义。
- **不改写事实**：Discovery 卡片继续只是已接纳的发现材料；封面仍只允许本地媒体副本，未取得时明确写「媒体尚未采集」；未知发布时间继续不以首次发现、观察或接收时间填补。没有接通详情、评论、媒体、OCR/ASR、查询行为、按钮行为、路由、Token、共享 Shell 或其它页面。
- **验证与边界**：由 focused Rust render test、现有严格窗口/默认读取测试、格式/lint/governance 与本机 DOM 检查记录。它不证明真实平台材料、媒体取得、跨页中文迁移、部署或 Mog 的最终可读性验收。

## 2026-08-26 · DESIGN-008 共享壳层采用中文主语义

- **来源与事项**：Mog 明确确认「中文为主，英文只用来装饰或作为注释」；Issue #68 / `DESIGN-008`。项目级 `LIDS-LANG-001` 由独立的 Issue #65 / Draft PR #67 定义，本条只记录共享 shell 对该规则的采用，不复制或替代其页面局部规则。
- **实际实现**：`shell.rs` 将一级导航、品牌副标题、本机边界与共享上下文中的静态运行码渲染为中文主文案加紧邻的 `v7-tech-key` 英文技术注释；`shell.css` 规定中文使用 Sans 主层、英文技术键使用较小 Mono 注释层，并把 `CORPUS` / `COLLECTION` 导轨读数改为中文可见语义。Collection 的 `NOW / TRACE / REVIEW` 短模式标签改为当前 / 追溯 / 复核。
- **Data Truth**：`UNKNOWN` 仍为未知，`UTC+08` 仍为同一时区，连接/来源/运行时状态仍由原有调用方提供；本项只改变显示层，未变更状态判定、数据、查询、路由、权限或任何动作。
- **不扩张**：无 Token、Primitive、CMP、Pattern、Scene、Motion、API、数据库、采集、插件、媒体或真实材料改动。Evidence Library 的页面局部模板、动态卡片及局部英文由 #67 单独处理。
- **验证与集成**：shared shell 单元测试同时覆盖 Corpus 与 Collection 输入；最终 DOM/视觉走查与治理检查记录在 `ACC-DESIGN-008`。建议先合并 PR #67，再将 #68 rebase 至 main；两个事项的运行时文件边界不重叠，但 focused test / 文档索引需由 integration owner 做行级整合。

## 2026-08-28 · EVIDENCE-PAGE-002 多材料 Evidence Library 静态参考

- **来源与事项**：Issue #85、`PAGE-EVIDENCE-001` 与 `MEDIA-RECON-001`。本项冻结作品级材料集合、lane 摘要与 embedded L2 Inspector 的产品/视觉合同，并提供合成静态高保真参考；当前 discovery-only 运行时不在本卡修改范围。
- **LIDS 组合**：继续采用 L1 `Corpus Explorer` + embedded L2 Inspector、216px 导轨与 440px Inspector 基线；直接消费现有 `lids_tokens.css`，没有新增或重声明全局 `--lgi-*` token。lane strip 是 page-local candidate，不晋升 CMP。
- **状态诚实性**：分别呈现 `PARTIAL`、`RISK_CONTROL`、`WITHDRAWN_OR_RESTRICTED`、`PROCESSING`、`BYTES_CLEANED`、`NOT_ENABLED`、`NOT_REQUESTED`、`UNKNOWN`、无匹配结果和读取错误；一次 Package 的同一 slot 只有一个来源观察组/generation，declared Bundle 下的 still/motion 组件与逐地址 candidate assertions 分责，下载尝试绑定精确 `candidateRef`，历史代次不混入当前来源组。不生成总体完整度，不把 slot 当 bytes，不把未知当零或成功。
- **验证与边界**：Chrome 桌面/窄屏渲染确认无横向溢出并验证结果/空态/错误态、作品选择和 Inspector tab；专用静态 verifier 检查合成标记、状态、Token 消费、无外部请求与 reduced motion。它不证明运行时、真实材料、媒体取得、OCR/ASR、数据库、插件或 Mog 验收。

## 2026-08-29 · EVIDENCE-RUNTIME-001 多材料 Evidence Library 运行页

- **来源与事项**：Issue #90、`PAGE-EVIDENCE-001`、PR #87 静态参考与 PR #88 `MATERIAL-PROJECTION-001`。页面从 discovery-only server render 切换为只读作品级 Material Projection；没有修改卡 3 合同。
- **LIDS 组合**：沿用共享 Shell 与原生 `lids_tokens.css + shell.css`，页面 CSS 只使用 `ev-*` 局部类；L1 Corpus Explorer 由中央连续作品列表承担，右侧 embedded L2 Split Evidence Inspector 核验详情。九 lane 状态带是唯一视觉锚，signal 只表达选择。
- **状态与敏感边界**：`UNKNOWN` 不写成 0，Slot 不冒充 bytes，ACK 不冒充完整；普通列表不显示评论原文或平台身份，本机授权详情才按页显示匿名上下文。媒体仅在同源受控句柄且 `INLINE_SAFE` 时内联；未知/不安全/受限/已清理不回退远程地址。
- **响应与交互**：作品列表和评论通道按 cursor 有界读取；详情 Tab 使用 roving tabindex，作品行支持方向键/首尾键/确认键；≤900px 转顺序流，≤640px lane 为两列，所有控件至少 40px 并支持 reduced motion。
- **未新增全局资产**：没有 Token、Primitive、CMP、Scene 或第二前端框架变更；作品行、lane 与 Inspector section 继续是 page-local candidate。
- **验证与边界**：正式自动检查和一次桌面/375px 浏览器证据记录于 `ACC-EVIDENCE-RUNTIME-001`。它不证明真实平台、历史回填、媒体/OCR/ASR provider、部署、长期稳定性或 Mog 业务验收。

## 2026-08-31 · WORK-RESOURCE-READ-001 Evidence Library 首屏层级收口

- **来源与事项**：Issue #110、`PAGE-EVIDENCE-001` 与 Mog 对当前运行页的直接反馈。用户明确要求删除重复的“语料 / 证据审查”“以作品为顶层的多材料证据库”，质疑左侧状态按钮用途，并指定 V7 原型中“工具条在上、列表为主体、Inspector 稳定”的布局作为参考。
- **Pattern 修订**：五项状态按钮的真实责任是 `view` 预设查询，不是 rail 或页面导航；因此移入结果头的横向“快速筛选”。三项 `layout` 仍只重排同一 items、选择和 Inspector。独立状态侧栏及其结果/选择/读取重复统计退役，主工作面恢复 `Corpus Explorer + Split Evidence Inspector` 两列结构。
- **LIDS 影响**：无新 Token、CMP、Scene、颜色、圆角或框架。继续消费现有 `lids_tokens.css`，使用直角编辑网格、signal 选中态、focus-visible、40px 级命中区与 160ms 颜色/按压反馈。查询回执、结果数和选择状态各留在唯一责任面，不通过删除说明而删除事实边界。
- **验证与边界**：新增 Rust source 断言拒绝 retired copy/旧 context 栏并固定 `view` 映射；隔离只读实例以 13 个本机作品集合完成 1440×900、375×812 in-app Browser 几何/截图和实际按钮切换。未发布到 `:3000`，未做 Chrome/900/390px/Mog 最终验收；隔离实例未接当前运行快照的独立媒体根，因此图片字节不在视觉结论内。

### 2026-08-31 follow-up · 双视图策略与 Table 可读性

- **用户确认**：参考稿红框内的 `SYSTEM VIEWS / 系统视图` 与 `MY VIEWS / 我的视图` 是应保留的产品语言；当前 Table 字号过小。
- **Pattern 修订**：恢复双区工作台。系统视图绑定五项已有 `view` 查询；我的视图在保存合同缺失时显示不可交互空态，不照搬原型中的假视图、假数量或保存能力。截图红框按评审批注处理，不进入产品边框。
- **Typography 修订**：Table 标题/上下文/辅助正文/状态提升到 14/12/11/10px，桌面行高提升到 92px；375px 堆叠保持字号。无新 Token、CMP、Pattern 或数据能力。
- **验证与边界**：1440×900 computed style、440px Inspector、13 行与无横向溢出已核验；375×812 `scrollWidth=clientWidth=375`，系统视图只在自身区域横向滚动。系统视图实际点击写入 `view=partial` 且 `layout=table` 保持；个人视图控件数为 0。仍未发布到 `:3000`，Mog 最终视觉验收待确认。

### 2026-08-31 follow-up · Layout selector tab rail

- **用户确认**：不采用三个独立方格的 `研读 / 表格 / 封面`，改用参考稿的水平文字 tab 形式。
- **局部修订**：选择器改为上下细线围合的单条 rail；非当前项保持 muted 文字，当前项以 signal 色 4px 下划线表达，不再使用黑底。保持 40px 以上命中区、focus-visible、按压反馈与 `aria-pressed`，不改变 `layout` 行为。
- **验证与边界**：1440/375 in-app Browser 实拍；rail 上下边线 1px、独立按钮边框 0、当前 signal 下划线 4px、三个命中区高度 44px。375px 三项宽度约 84px，document 无横向溢出；实际点击 `封面 → 表格` 后 URL/DOM 同步。键盘焦点使用灰底与顶部短信号线，不恢复四边框。这是 page-local 样式修订；没有新 Token、Primitive、CMP、数据能力或路由。

## 2026-09-04 · COLLECTION-FIVE-PAGE-V4-UI-001 五页面桌面工作区采用与复审修正

- **来源与事项**：Issue #149 扩展、`REF-V4-IA-CN-001`、Draft PR #154。参考只提供五个 Collection 子面的桌面密度、ledger/Inspector 关系与工作区层级，不继承演示事实或产品能力。
- **Pattern / Page 影响**：五页继续使用唯一 `Collection Control` Pattern；Targets 保留 URL-owned lifecycle drawer，Attention/Tasks 采用 ledger + 右侧事实 Inspector，Operations 采用有界阶段 ledger + 唯一深色持久决定面，Runtime 继续消费单一 capacity evaluator。没有新 Pattern、CMP、Scene、Token 或全局 shell。
- **LIDS 冲突裁定**：首版照搬了参考的可见页标题与第二条五格读数，经 exact-head 独立复审判定与 `LIDS-PAT-001 / DESIGN-003` 冲突。修正后每页只保留 `.v7-sr-only` `h1`，两个读数只进共享 Context Bar，并直接显示 scope/source；共享 `shell.css` 仍是 KPI 样式唯一 owner。删除新增渐变与字符图标，新增 spacing 只使用 4/8/12/16/24/32px ladder，新增 focus 不在页面层重声明。
- **Data Truth / 交互修正**：Operations `trace/review` 不再被 `now` 投影覆盖；恢复数只计入 `recovery_for` 有明确动作的原因；Runtime 不再把存在插件版本冒充在线工位；creator/keyword 共用同一份 kind-aware lifecycle 文案。Task Frozen Work 先选最近 Work / Lease，再展开该 Lease 全部 Task，并继续按选中 `task_id` 注入；Inspector 状态 class 与文案同步，零任务/schema 不可用/读取失败均为明确终态；tab 补齐 ARIA、`hidden` 与方向/Home/End 键。
- **验证边界**：main 集成后的 format、workspace check/test、JavaScript、UI handbook、governance、完整 Work/Material/Corpus/Topic/API PostgreSQL 69 项、dispatch 8 项与 Collection Control `8 + 11` 项已通过；一次性数据库、container、volume 已清理。当前 source tree 的无数据库 Chrome 152 / 1440×900 五页终态为 0 overflow、2 个直接显示 scope/source 的 KPI、唯一 1×1 读屏 h1、0 第二读数条、0 loading 与 0 应用 console issue。有数据交互由 PostgreSQL/source tests 而非已拒绝首版浏览器结果证明；exact-head 复审、main 合并与 `:3000` 运行时证据按实际动作另记。不包含共享 DB、Worker、插件、真实平台、外部部署或窄屏支持。

## 2026-09-06 · COMMENT-RESEARCH-001

Issue #167 在交付分支新增 L1 评论研究三视图和已存查询，复用共享 shell 和 token，Serif 原声、连续表格、文字 Tab 与受控 dialog；Evidence 侧栏仅接通两个真实入口。无 3D、新主题、旧系统样式、生产选题或虚构模型结果。API/PostgreSQL 证明及未验证层见 [评论研究验收](../acceptance/comment-research-001-acceptance.md)。真实模型、部署与用户验收仍未发生；根代理固定 HEAD 审核以 PR 评论为准。

### MODEL-PI-001 同包返修

Mog 明确将四区常驻设置改为单页+关键配置弹窗。复用 L1 Settings Pattern、现有 dialog 与 `--lgi-*`，没有新增 Token/导航。API 地址预览、测试中/失败/数据库未初始化状态就地显示。来源与验证见 `docs/design/changes/model-pi-001-ui-change-manifest.md`。

2026-09-07 MODEL-PI-001：输出上限失败反馈区分“模型已响应”与“评论结果不完整”，沿用原 L1 状态组件、布局和 token，无新增视觉值。

## 2026-09-07 · COMMENT-DAILY-001 原声表格与每日研究

按 Mog 最新要求，原声浏览采用与证据库接近的连续紧凑表格：13px Sans、两行预览、7px 纵向内边距，Serif 留在展开研读。此处为评论页局部密度约定，未改共享 token。新增点赞、评论时间、作品/清洗筛选，试跑和每日设置保持按钮+弹窗。每日研究为评论研究内部第四视图，未改变一级导航。合成浏览器验证实际表格与批次回原声路径；共享部署、真实语义质量和 Mog 验收分别待完成。详见评论研究验收记录。

## 2026-09-08 · CORPUS-CROSS-DOMAIN-RENDER-001 外部领域样本与领域菜单

- **来源与事项**：Mog 在真实本机页面选择“考研自习”后看到领域数为 21、结果列表却为空，并指出 Context Bar 原生下拉框不符合 LIDS；Issue #130 仅提供外部领域语义，不授权本次建立领域配置能力。
- **语义修复**：候选页面把跨行业接口的 `sampleRef` 适配为“跨行业参照样本”，不再拿它冒充 Work Resource `publicRef` 或调用详情路由。列表与 Inspector 只显示接口实际给出的列表级字段，显式写明详情、正文、评论、媒体与来源血缘尚未读取；检索、材料筛选、状态视图和排序在外部领域禁用并解释原因。
- **LIDS 修复**：`corpus_domain_picker` 从原生 `select` 改成方形 `details/summary` 触发器和链接菜单；菜单使用现有 `--lgi-*`、1px Ink 边界、白色 Surface、单层 `--lgi-shadow-brutal`、中文主标签和 focus-visible。无 Token、通用 CMP、导航区或新数据能力。
- **验证边界**：候选 `:3107` 用当前本机只读数据实际显示 21 行，首项 Inspector 不再显示“来源信息不完整”；focused Rust/DOM、format 与 JS 检查通过。未修改数据库/API/插件、未部署 :3000、未合并 main，Mog 验收待后续集成。

## 2026-09-24 · DOMAIN-UNIFICATION-001-CORPUS-ENTRY 语料领域入口

语料未选领域入口改为现有 Evidence Library 壳层上的原生 `<dialog>`；外观只消费 LIDS 现有 token、黑白结构、中文主标题、红色信号，不增 Token/CMP/Scene。共享页头按既有 `--lgi-z-header` 层级高于评论研究 sticky 工具条，修复领域菜单被内容覆盖。桌面及 390/375px 本机浏览器验证见 `../changes/domain-unification-001-ui-change-manifest.md` §6；此条不是部署回执。

## 2026-09-26 · DOMAIN-UNIFICATION-001 全部领域作品直达

Collection 的“全部领域”作品深链经既有领域弹窗保留 `work`，正确领域进入同篇 Inspector；`material_not_found` 由中文反馈区解释为“当前领域无法读取这篇作品”，不将其一概断言为领域未收录。只改既有 Page 交互与状态表达，不新增 Token、Primitive、CMP、Pattern、材质或动效。真实数据副本的正确领域流程已浏览器验证；不可读反馈尚未独立浏览器验证，`main`/`:3000` 未刷新。范围与回执见 `../changes/domain-unification-001-ui-change-manifest.md` §7。

## 2026-09-15 · EVIDENCE-COVER-CARD-MATERIAL-004 第七种材料 `M-06` 纸面残留

- **来源与事项**：Mog 2026-09-15 派定只改 `/corpus/evidence?layout=cover` 作品卡的视觉层级、材质、3:4 主视觉区与既有 hover 翻面，并在材质选型（候选 B「采样残留」）落地后明确指示「登记为第七种」。这是 `LIDS-MAT-001` §1 家族程序要求的路径：先证明格点归属，再按 [design-governance.md](../design-governance.md) 的变更分类成为长期决定。
- **材料**：`M-06` 纸面残留进入材料表。常态 24px 格（3 个点阵步长）取 9 个格点中的 `(0,0)`、`(0,8)`、`(8,16)`；加密态（选中卡）在同一格里多留一个 `(8,8)`，1/192 → 1/144（约 1.3×）—— 同一套点阵上的密度变化，不是第二种图案或第二种颜色。点半径 `.6px`、墨色 `--lgi-ink`、α 封顶 `.20`（亚显影档）。允许区位只有有 1px 边界的独立卡板；连续阅读区背后禁用。登记的前提是格点对齐，因此候选稿的 20px / 偏移 13/6 周期改为 24px：同面积点数比候选稿少约三成，纸面更淡一档，α 与点半径不变。
- **登记过程中修正的一版**：第一版按格点归属取点，用的是 `(0,0)`、`(8,16)`、`(16,8)` —— 全部落在 8px 格点上，但这三点构成点阵的一个子群（格点坐标 `y ≡ 2x (mod 3)`），于是每个点都是无穷长点列的第一点，场在 45° 方向排成每 11.31px 一个点的连续排线；加密态的 16px 格（`y ≡ x (mod 2)`）同样如此。格点归属的断言对这一版是全绿的 —— 判据 2「不排成线」因此是与判据 1 并列的条件而不是它的推论。同一次还确认 16px 格放不出残留：4 个格点里任取 2 个，两点之差都会在真实场里生成无穷长排线，所以常态格只能是 24px（同时满足两条判据的最小格）。
- **其它表面**：卡面是 `--lgi-canvas-sunken`、舞台 `--lgi-canvas-hi`、铭牌 `--lgi-canvas-low`，全部为既有 token；整圈黑框只剩主视觉一条 1px。舞台测量场是 CSS 铺满的 8px / 1px 点阵（与卡面材质不同图案），六个几何模板重画到同一安全区（视图 300×400 的 x 48..252 / y 80..320）。卡片标题从页面大标题字阶改回 `--lgi-text-longform`（16px/700/1.1）。`M-00`…`M-05` 的规则与运行时位置不变。
- **Token**：不改任何 `--lgi-*` 值，不新增 Token、CMP、Scene 或全局动效。颗粒与点阵的周期、偏移、半径是材料自身的密度参数（`LIDS-MAT-001` 的"不得写死间距值"按其字面不适用于材质参数），墨色统一为 `--lgi-ink` 同 RGB，只变透明度。
- **验证边界**：`cargo test -p linggan-api local_web::tests::evidence` 12 passed；`M-06` 的三条判据各以性质断言守（每个点与重复周期都是 8px 的整数倍；场中不存在三个等距共线的点、间距小于 17px；密度封顶 `.20` / `.6px` / 墨色 `(17,19,21)`），回填验证 11 组：常态改回子群取法 → 报 `draws three dots on one line 11.31px apart`；三点等距排成一行 → 报 16.00px apart；选中态在同一 24px 格上排成一行 → 同样变红；只写一个坐标、重复声明 `background-size`、节点落在 tile 之外、新增第三条覆盖规则、格点改成 `(13,16)`、α 提到 `.6`、点半径提到 `4px`、墨色换成另一种颜色 → 各自只让对应断言变红。1920×929 真机窗口、检查器关闭、50 张真实作品实测：六列、卡宽 277.33px、卡高 544.64px、舞台 240.03×320.05、封面盒与舞台内容盒差 0。真实浏览器计算样式确认卡面 24px、选中 24px、舞台 8px、内容层 z-index 2 在颗粒之上。**部署回执**：实现 `2f7a322`、登记 `9a537fc`，PR #289 合并为 `41aba73`；`./scripts/runtime/install.sh` 已把三个服务切到 `runtime-main@41aba73`，`/health` schema `READY`，`/corpus/evidence?layout=cover` 与其 `/assets/evidence-library.css` 实测为登记后的取值。仍未证明：远端生产、真实触摸设备、1440×900、长列表滚动性能与 Mog 的业务验收。

## 2026-09-17 · COMMENT-STUDY-TABS-001 补齐 `PAGE-COMMENT-STUDY-REBUILD-001` 已批准的 5 个复核 Tab

`/corpus/comments` 的复核区此前只有三个迷你统计框，未实现设计文档 §4 已批准的"概览、评论目标、待归并、用户问题、运行记录"5 个文字 Tab（`ADR-11`）。本包补齐，使用既有共享工作区骨架、既有 `.study-table` 滚动/表头/token 惯例与旧评论研究页沿用的 Serif 原声呈现（`--lgi-font-evidence`），不新增 Token、CMP 或 Scene。"评论目标"Tab 新增对 `linggan_material_comment.body_text` 的读取，直接在读取时核对 `linggan_material_comment_restriction`，冻结后被限制的来源如实显示限制说明而不是继续返回原文。技术状态词（`target.state`/`eligibility_state`/`resolution.state`/`problem.state`）全部补了中文标签，未识别取值原样显示。`cargo test -p linggan-api local_web::comment_study::tests --locked` 8 passed；隔离 PostgreSQL `test-comment-study-rebuild-postgres.sh` 15 passed（含来源限制后原文消失的新证明）。共享部署、真实浏览器逐 Tab 走查与 Mog 业务验收详见验收记录。

## 2026-09-20 · RUNTIME-STATION-V7-2-001 执行工位内容区按 v7.2 稿复刻

- **来源与事项**：Mog 2026-09-20 提供 `linggan-execution-station-v7-2-runtime-drawer.html` 并要求按其细节完整复刻到 `/collection/runtime`；同日当面拍板三项范围裁定（只复刻内容区 / 按 LANG-05 只留中文 / 抽屉换成真读得到的四块）。Issue #309。完整清单见 [`changes/runtime-station-v7-2-001-ui-change-manifest.md`](../changes/runtime-station-v7-2-001-ui-change-manifest.md)。
- **落地的 v7 语法**：控制条（`.c-deck`，`auto | 1fr | auto` 三列，四格读数 + 动作区）、接单仪器面（`.c-instr`，深色表头 + 左通道列 + 右四读数格）、区段头（`.c-sect`，徽章 + `h2` + 右侧 meta）、六格读数条（`.c-ops`）、量级额度轨（`.c-usage`）、右侧运行概览抽屉（`.c-rdrawer` + 贴边把手）。这些语法全部在**本页内**建立，未升格为共享 CMP，也未被其它页面引用。
- **同日第二轮修订（Mog 四项指示）**：控制条由卡片改为**夹在上下两条线之间的跟栏**（四边 `border` → 只留上 / 下两条 `--c-line-rail`，去掉 `--v7-brutal` 硬影）；删掉与贴边把手重复的「运行概览」按钮。带状态那一格改为「上沿 3px 色带 + 同色数值」，**四档各自定义**（能接活 / 部分接不了活 / 接不了活 / 读不到），不铺稿中的同色淡底——照抄会把「读不到」拉到 4.14:1（15px 粗体需 4.5:1），去掉后为 4.71:1。详见清单 §4b 与 §5c。
  - **该轮此条已被第三轮更正，此处保留原文以留痕**：第二轮把「不要用新粗野主义做阴影处理」应用到 `.c-deck-actions` 的**全部三个**按钮上（取消 `box-shadow` 与悬停位移）属过度应用。第三轮先是一度把该覆盖整块删除（三个按钮全部恢复硬投影，同样不对），最终按稿改为**两级**：主按钮（新增工位）保留硬投影，次按钮（运行概览 / 去观察目标页）`box-shadow:none`。作用域仍限 `.c-deck-actions`，不改 `.c-btn` 本身，故不构成对共享语法或 `--v7-brutal` 这条硬边语言的修改。详见清单 §5d-2。
- **同日第三轮修订（Mog 七项指示）**：① 运行概览入口由贴页边把手移回控制条内，把手节点与其三条规则、`body.c-rdrawer-open` 类一并删除；② 行内按钮按稿分两级（上条），并把「新增工位」由 `<a>` 改为 `<button type="button">`——垂直居中的根因是元素类型而非 CSS 值；③ 行内登记表单移入居中弹窗（复用本页既有模态语言，不引入原生 `<dialog>` 与 `::backdrop`），失败时弹窗自身渲染成开着；④ 接单仪器面表头右缘贴 `--lgi-mosaic-on-dark`，面积 **35%**、配 **8 段硬停止**量化遮罩（`materials.md:113` 要求溶解量化，出现平滑渐变即不合格），并用 `color-mix(in srgb, var(--lgi-ink) N%, transparent)` 而非稿中的 `#00000026` 字面量，以过本样式表「不得自撰颜色」的硬断言；**有意偏离稿一处**——稿把 `INSTALLATION_STATE` 压在纹路上，`tokens.md:343` 明写「绝不覆盖文字」，故把纹路收进独立保留轨、文字全部让开（实测间隙 10.4px），同时纹路取 35% 而非稿中的 30%（`tokens.md:343` 的登记区间为 35%–45%）；⑤ `INSTALLATION_STATE` 保留，作为 **LANG-05 的用户授权单点例外**登记于 `../lids/language-policy.md`「已登记的单点例外」（它是描述性标签，按判据本属被禁的第四类）；⑥ 仪器面由「左通道列 ＋ 右四读数堆叠」改为**五列一行**（对齐稿中 `.alert-body` 的 `1.2fr repeat(4,.72fr)`），容器 294.8 → **221.8（−24.8%）**；⑦ 删除说明句「账号能不能用由服务端逐台判定，插件自己说了不算。」。**未新增、未修改任何 Token。**
- **本页 v7.2 第三轮的两条既有状况（留证，不在本轮范围）**：`--lgi-warning` 作字角色时在白底上为 3.88:1（未达 4.5:1，`74e87ad` 起即如此，要动须先动 Token 基线）；`.c-page` 的 1040px 下限与 900–1040 区间被 `html,body{overflow:hidden}` 裁掉且无滚动条，在交付基线与 `origin/main` 上逐字相同，本轮未触碰。
- **同日第四轮修订（Mog 四项指示）**：① 四格文字改白（`--lgi-on-dark`）；② 「3 条通道」与 `INSTALLATION_STATE` 移到右侧；③ 内容区上内边距 `40 → 20`；④ 工位表「今日采集」列改居中。改动面仍只有 `collection_workspace.css` 一个文件，未改 `.rs` / `.js`（四个格子是既有节点；工位表那一列用既有类名 `.c-tg-station-grid`，它同时挂在表头行与数据行且只出现在这张表，故无需新增标记）。
  - **本条取代第三轮那处「有意偏离稿」**：第三轮把纹路收进独立保留轨、四个格子全部让开（实测间隙 10.4px）；本轮用户要求两个标记移到右侧，即稿中那种文字压纹路的构图。**依据是 `tokens.md:343` 的原文**——它要求纹路「绝不覆盖文字」，即纹路不得盖住文字，而非文字不得落在纹路上方；文字绘在纹路上层（`.c-instr-head > *{z-index:1}`，即稿中 `.instrument-head>*` 的写法）即为满足。纹路仍取 **35%**（登记区间 35%–45% 的下限）。第三轮那段原文保留在上一条里以留痕。
  - **连带的底色更换，必须记明**：徽标底由 `--lgi-stream-red` 改为 `--lgi-signal-ink`。只把字改白会**引入一次对比度回归**——深墨字配原底 5.56:1 达标，白字配原底只有 **3.28:1**。`ADR-04` 规定信号色分**不可互换**的两档，承载文字的那一档是 `signal-fill`（`tokens.md:340`、`system.md:101`、`primitives.md:108`）。取 `--lgi-signal-ink` 的理由是运行时镜像**没有** `--lgi-signal-fill` 这个 token，而 `--lgi-signal-ink` 配 `--lgi-on-dark` 在本仓库已有两处先例（`comment_study.css:11`、`comment_research.css:10`）。实测 **6.71:1**，过 4.5:1。**未新增、未修改任何 Token。**
  - **一处治理缺档（留证，待 Mog 定夺）**：手册 `tokens.md:77` 登记的 `signal-fill` 在运行时镜像 `lids_tokens.css` 里没有对应 token；镜像的 `--lgi-signal` 是「单值双用」的 `#ef4f25`，既不等于手册的承字档，也不等于手册的色块档。任何在信号色实底上承载文字的界面（不止这一处徽标）都会碰到同一问题；补档或把 `--lgi-signal` 对齐手册属 Token 基线决定，本轮只在本页这一格取了已过线且有先例的一档。
- **同日第五轮修订（Mog 三项指示）**：① 删掉说明句「现在没有任何一条通道派得出任务。下面逐条说明卡在哪一样。」并继续压低接单仪器面的高度；②「新增工位」默认墨底、悬停变红；③「运行概览」与「去观察目标页」默认墨底、不带硬阴影。清单 §5f。
  - **删的是「复述判定」的那一类，不是全部**：四档判定里只有 `Verdict::None` 那句是纯复述（表头本来就写着「3 项阻塞 / 接不了活」，下面又逐条写着卡在哪），故只删它。`All`（「派得出任务」不等于「有活在跑」）与 `Some`（几种堵塞不能合并成一件事）留着的理由是它们说的是**该怎么读**这个判定，表头那几个字装不下；「读不到」那一档的说明句更是仓库不变量的要求（不得从读不到推断成不存在或坏了）。删 `Some` / `All` 会让这块的高度在不同判定之间跳变，也是保留的一个次因。`verdict_markup` 的 `why` 因此由 `&str` 改成 `Option<&str>`。
  - **压缩实测与一处「改了但当前不生效」如实记录**：同一份数据上把删掉的散文与 16 的内边距按原样放回去量的改前值 `247.8`，改后 `208`，**−40.0px / −16.1%**（散文 19.8 + 其下外边距 12 + 内边距上下各 4）。同批把 `.c-instr-issue` 的下限由 36 收到 32，**这一条在当前数据下不起作用**——行高是内容撑出来的（备注在 232px 宽里折两行 = 36，加内边距与分隔线 = 45），远高于下限，只在备注短到一行时才兜底。已在 CSS 注释与清单里注明。
  - **剩下的高度在数据上，不在留白上**：这块现在的高度由每条通道那句备注自己折几行决定；再矮只有把备注写短或把左栏加宽两条路，两条都会动到信息本身，超出本轮授权，留待 Mog 定夺。
  - **三个按钮的改法与一处状态重做**：`新增工位`（`.c-btn-primary`）默认 `--v7-black`、悬停 `--v7-red-hover`，保留硬投影与悬停位移；两个次按钮（`.c-btn-quiet`）默认同为墨底白字但 `box-shadow:none`、悬停只换底色不走位移＋阴影。**「运行概览」打开抽屉时原本靠「墨底」区别于默认态，本轮默认态也是墨底，该状态下只剩「下缘 3px 信号色条」可认，故这一档的悬停不再变红**（红条压在红底上会消失，指针移上去状态反而没了），两规则靠源序压过通用悬停。钩子锚在 `data-runtime-drawer-toggle` 而非 `[aria-expanded]`——「新增工位」也有 `aria-expanded`（它开弹窗）。作用域全部限在 `.c-deck-actions`，本页弹窗里的「登记」与其它页面的按钮未动。
  - **悬停为什么用 `--v7-red-hover` 而不是 `--v7-red`**：按钮上压白字，`ADR-04` 要求承载文字的信号色 ≥ 4.5:1；白字压在 `--v7-red`（`rgb(239,79,37)`）上只有 **3.61:1**，属只做色块与轨的那一档，`--v7-red-hover` 实测 **5.19:1**。这与上面那条「一处治理缺档」是同一个根因（镜像缺 `signal-fill` 档），本轮仍只在单点取已过线的一档，未动 Token 基线。
  - **验证**：`cargo test -p linggan-api --bin linggan-api` **240 passed; 0 failed; 21 ignored**，与基线逐位一致（删掉的那句散文全仓 grep 只命中源码本身，无测试断言）。真实指针悬停实测三枚按钮默认态均 `rgb(17,19,21)` 墨底白字 **18.62:1**、悬停态均 `rgb(199,58,21)` **5.19:1**；两个次按钮悬停时 `box-shadow` / `transform` 均为 `none`，主按钮投影由 `4px` 增至 `7px`、位移 `(-2,-2)` 保留；真点击打开抽屉后悬停中的「运行概览」仍为墨底，下缘色条 `3px × 54px` 在场。
- **同日第六轮修订（Mog 两项指示，2026-09-20）**：① 删掉运行概览抽屉左缘的加粗线条／阴影；② 接单仪器面每条通道那句备注太长，要求「直接截断，不允许换行」。改动面仍只有 `collection_workspace.css` 一个文件，未改 `.rs`／`.js`。清单 §5g。
  - **该轮开始前，第五轮已由 PR #310 合并（`52cd83d`）并按 `runbooks/local-runtime-deployment.md` §3 部署，本机 `:3000` 上可见**——上一轮末尾那句「未部署、未推送、未合并」据此作废，旧文不回改，以本条与 `progress/2026-09.md` 的部署回执为替代。
  - **Ⓐ 左缘那道 6px 黑杠是「边 + 影」叠出来的，不是其中一样**：`.c-rdrawer` 原有 `border-left: 2px`（`--c-line-rail`）与一块向左下偏移的硬阴影 `-4px 4px 0 var(--v7-black)`。**删阴影、留边**——这块是覆盖在页面上的抽屉，最后一条边也去掉就与底下的内容糊在一起，读不出分界在哪；用户给的是二选一，删的是阴影那一半。纵向偏移 `+4px` 而抽屉底边压在视口底，那 4px 只可能落在视口外，删它不动底缘。实测 `box-shadow: none`、`border-left: 2px rgb(17,19,21)`。
  - **Ⓑ 截断落在 `.c-lane-name` 上**：`white-space:nowrap` ＋ `overflow:hidden` ＋ `text-overflow:ellipsis`，另加 `min-width:0` 让它允许被压到内容宽度以下（grid 轨道默认 `min-width:auto` 会撑住不缩）。截断点由列宽决定，**不写死字符数**。那句备注是服务端给的完整一句话（`row.reason`），长短不归这一栏管；让它折行，这块的高度就跟着数据走，三条通道还可能折成不同行数，行高参差。
  - **上一轮「下限不生效」那句随之作废**：`.c-instr-issue` 的 `min-height: calc(var(--c-cell) * 4)`（=32）本轮开始**真正兜住行高**（一行内容 18 ＋ 内边距 8 = 26，低于下限）。旧注释与原「不起作用」的记录不回改，清单 §5g 注明替代关系，不留两个版本。
  - **实测与一处如实记录的损失**：同一份数据三条通道行高 `45 / 45 / 44` → **`32 / 32 / 32`**，仪器面 `208 → 170`（**−38.0px / −18.3%**；表头 46 ＋ 正文 120 ＋ 边框 4）；截断确证 `scrollWidth 411 > clientWidth 290`，判定词列 35px、判定码列 79px 未变。完整那句是「这台工位已有一份有效 Lease，首单观察完成前不会并发领取。」，页面上现在止于「…首单…」——原文仍在 DOM，界面读不到后半句。用户说的是「直接截断」，本轮按字面执行，**未自行加 hover 提示**；是否给截断格补 `title` 留作建议。
  - **验证**：`cargo test -p linggan-api --bin linggan-api` **240 passed; 0 failed; 21 ignored**，与基线逐位一致；样式表「不得自撰颜色」硬断言复验 `#` 字面量计数 **0**。**未推送、未合并、未部署。**
- **Token**：不新增、不改值。新增样式只消费既有 `--lgi-*` 与 `DESIGN-010-UI-EX-01` 例外块内已登记的 `--c-*` 别名；新登记一个页内变量 `--c-runtime-drawer: 356px`（抽屉宽度，与稿中的 `--runtime-drawer-w` 同值）。`lids_tokens.css` 未改。第五轮三个按钮用的 `--v7-black` / `--v7-red-hover` / `--v7-white` 均为 `shell.css` 已有的 v7 别名，不是新值。
- **材料**：深色只出现在两处局部面（仪器表头、抽屉执行结果卡），沿用 `M-05` 的既有深色面语法与 `--lgi-stream-*` 取值；稿中的「传感器」斜纹与点阵未引入。
- **语言**：稿中描述性英文标签（`LIVE` / `SUCCESS` / `FAIL` / `TOP 3 · 24H` / 各模块英文小标题）一律不加，按 `LANG-05` 只留中文。
- **数据诚实**：稿中抽屉三块内容（回传成功率与柱状图、补采失败任务计数、失败原因 TOP 3）在本系统**没有真实读模型**，按 `data-boundaries.md` 换成四块真读得到的内容；稿中数值一个都不出现。逐条偏离见清单 §4d。
- **验证**：`cargo test -p linggan-api --bin linggan-api` **240 passed; 0 failed; 21 ignored**，与改动前基线一致；抽屉开合契约、把手与控制条动作按钮的几何关系、`.c-runtime` 子树溢出扫描均为脚本实测。**未部署、未推送、未合并**，本机 `:3000` 上尚不可见。

### 2026-09-20 · RUNTIME-STATION-V9-FINAL / M-02E

- **规则变更**：Mog 明确允许 `/collection/runtime` 的量化深色右缘 mask 与「暂停／恢复」白底按钮右缘采样栅格；登记为 `M-02E`，同时收窄到本页、两种准确区位和既有 token。`system.md` 的渐变禁令与 `tokens.md` 的 Mosaic 区位说明均显式引用该受控例外，未产生新的 token、颜色或主题。
- **运行时表达**：保留深色看板表头 35% 的八段硬停止 mask；白底纹理只在真实 POST 动作的 8px 右缘。两处均不覆盖文字、不铺到表格／背景、不作为状态唯一通道。
- **已验证／未验证**：`station_view` 42 条、完整 API 二进制 243 passed / 21 ignored、JS 语法、API 编译、设计手册／治理／diff 检查已通过；真实浏览器的两处材料范围、管理弹窗与 POST 回执，以及本机 runtime 发布仍待当次回执，不能由本条推断已发布。


## 2026-10-09 · TOPIC-MAP-V41-001 公共页头恢复

Mog 最新要求主题图谱页头、个人菜单和面包屑沿用其他页面的同一设计，替代本包初次Demo纠偏的独立64px页头。页面移除所有局部 `.v7-*` 覆盖，复用 `shell::global_header` / `SHELL_CSS` / `corpus_domain_picker`，不改shared shell或Token值；主体仍按Demo布局。既有公共壳层仍有历史规则与运行时差异，本次恢复复用不宣称全站完成LIDS迁移。验收和部署边界见主题图谱实施验收与active plan。


## 2026-10-11 · PR #386 已确认创作者八列与分区抽屉

Mog确认的后续设计并授权合并部署：八列表格、两行工具栏、独立筛选草稿弹窗、三分区抽屉及固定动作/分页。沿用canonical Token和共享SSR壳，不改公共shell或Token；有限短屏覆盖只作用本页。此前#384的五列候选不是本次发布设计，后端调度仍独立待派定。完整壳层/HTTP/离线证明与部署回执追加既有creator验收文档，Mog视觉/业务签收不由自动测试代替。


## TOPIC-MAP-COMPACT-382 · 2026-10-13

用户明确指定 `/topics` 领域选择旁切换概览/旅程、左树贴页边、E 统计先呈现及双旅程视图始终可达。仅该页面消费 shell 现有 crumb/meta 插槽并局部改变布局，不修改 shell.css/rs 或 lids_tokens.css；该用户授权优先于默认 Context 区位限制。继续用现有文字 tab、按钮及单 Inspector；五阶段/合格来源/三种高表现口径不变。

来源、作用区、状态词典与验收：[本次清单](../changes/topic-map-compact-382.md)。JS 实际函数回归通过；浏览器、Rust 模板编译、独立复核、运行时和业务验收此时未验证，不宣称全站 v7 完成迁移。


### TOPIC-MAP-COMPACT-382 · 2026-10-13 可阅读性修订

按 Mog 最新截图意见，当前单平台不预留多平台空槽；重组既有四组信息为总量与带表头表现区，材料搜索归 F，限制正文阅读宽度，候选标记中性化、旅程节点字号与 SVG 体量分别控制。只消费运行 token，不新增全局数值或改共享 shell。新验证与真实体验边界见 [本页清单](../changes/topic-map-compact-382.md)。

## LOGIN-ANATOMY-001 · 2026-10-13

- Mog 确认 Login V2 后授权接入并提交推送合并发布，开发期不接真实认证。
- 原登录 HTML 的局部视觉变量、布局和 Anatomy 材质参数为页面级例外，范围仅 /login；不修改全站运行 Token。
- [变更规格](../changes/login-anatomy-001.md) 定义表面、状态与验证；装置为合成装饰流程，开发提交只有导航，不制造认证或真实研究成功。
- 候选验证与正式运行证据分别追加到当月进度；本记录不表示正式部署或 Mog 验收已发生。


## 2026-10-14 · UIUX-CONTENT-WORKBENCH-001 V3 局部材料表面

用户明确要求视频中的空间书页、纸条放大与分布索引。本轮在独立 V3 中引入纸色、书脊、页边投影、420ms 翻页与 160ms 纸条聚焦动效，系统 reduced-motion 时禁用运动。仅此材料浏览表面的授权提案，不改变正式 token、不推广全站 hover 抬升。原型继承 LIDS 应用壳，作品与纸面色保持局部。实现、源码/浏览器验证与未接入项见同事项提案和验收记录。


## 2026-10-14 · V3 方向确认与文案空间调整（仅原型）

Mog 认可 V3 并要求删除常驻描述性文字。原型移除重复标题/教学文案、合并创作工具栏；正式壳层与 token 没有迁移。设计规则候选及用户确认见 UIUX-CONTENT-WORKBENCH-001 提案 §12–13，实际版面验收见同事项 §8。

## 2026-10-15 · 首篇创作正式页面规则（#425）

Mog确认V3并授权实施；[PAGE-CONTENT-WORKBENCH-001](../pages/content-workbench-page.md)将紧凑沉浸布局、局部原地纸条聚焦、书页/索引联动和差异表达正式化。仅新增创作页面，消费现有token，原型保留；运行与审美验收另记。

## 2026-10-15 · 内容工作台转入 WEB-REACT-001 阶段 C

原生候选界面与 shell/ops 导航增量已撤出后端交付，未进入主线。已认可 V3 的阅读/创作产品规则保留；LIDS 仍是唯一标准，React 实现按阶段 C 在 AppShell/CSS Modules 中验收。此前“正式化”记录仅为候选工作的历史，无正式 UI 发布证明。


## 2026-10-15 · WEB-REACT-001 阶段 C / V3 页面尺寸例外

依据已认可 V3 与 PAGE-CONTENT-WORKBENCH-001，React CSS Modules 仅在工作台页面作用域声明 `--cw-reading-min:28rem`、`--cw-strip-min:14rem`、`--cw-lane-width:18rem`、`--cw-control-width:21rem`、`--cw-dialog-width:60rem`、`--cw-editor-width:48rem`、`--cw-paper-focus:1.075`。这些是书页、纸条、画布列、编辑与对话框的 Page 层尺寸/选中聚焦参数；不晋升为全站 Token，不移植 V3 CSS，不改变颜色、字体、字号或间距标准。后者全部消费现有 LIDS；`prefers-reduced-motion` 下禁用纸条放大过渡。登记先于 React 样式实现，正式浏览器密度与溢出证据随后补齐。

正式创作渲染采用本地分发字体文件；不请求外网字体，字体来源/许可/hash 随资产登记。工作台路由 React.lazy 按需加载，不把图页绘制与字体资源放入设置页首屏依赖。

2026-10-15 短屏核对修正：单页编辑预览按 55vh 与扣除四个现有 `--lgi-space-24` 的视口空间共同限高，CSS 保持 3:4 完整画幅；桌面随编辑区停靠，窄屏正常滚动。仅 Page 层响应布局，不改变目标 PNG 1080×1440、绘图字体或全站 Token。

阶段 C 状态文案：React 工作台正在提供作品写入，当前一级入口不沿用旧运营台账的“只读”旁注；AppShell 接收当前页明确的辅助状态“创作”。导航登记、页头结构和样式保持既有基线，旧页面无需改写。

### 2026-10-15 · 阶段 C 工作台自身滚动边界修正（实施前登记）

root 目检真实隔离浏览器的 390×844 正文、图组和材料截图发现左侧控件被裁切，尽管 `document.scrollWidth` 等于视口宽度；原证明只测根文档宽度，遗漏内部滚动位置与可见控件边界。工作台 `main.workspace` 作为 AppShell 网格项原先只有 `max-width` 与自动外边距，没有明确可收缩尺寸及独立滚动边界；内容固有尺寸与 `scrollIntoView` 可将外层可裁切区域横向移动。按 root 本轮指示，修正范围仅为本页根 `main` 的 `width:100%`、`min-width:0`、`min-height:0`、明确高度与 `overflow:auto`，取消网格项自动居中的歧义；共享 `shell.css`、字体与 LIDS Token 不改。正文、图组及材料仍保留各自已批准的布局与局部滚动。

验收改为同时记录工作台、AppShell 与根文档的宽度及横向滚动位置，并在实际滚动后核对必要输入、按钮和预览画布的左右边界位于视口内；不再仅凭根文档 `scrollWidth` 报告窄屏可用。本条登记修法与边界，修后几何、截图及未验证项另由本轮隔离浏览器回执记录，不提前宣称通过。

同轮增强断言随后定位到材料索引的独立溢出：真实 390px 隔离 DOM 中索引行宽 366px、`scrollWidth` 为 552px；各定位按钮的默认 `min-width:auto` 保住 4px 指示条与 1px 边框，百余片段的固有最小宽度撑开材料区，使工作台 `scrollWidth` 达 564px。修法限定为索引按钮 `min-width:0`，允许既有 flex 按可用行宽收缩；内部指示条增加 `max-width:100%`，防止其反向撑开按钮。片段数量、身份、位置联动与点击动作不变，不用裁切或隐藏索引修饰结果；不新增 Token。修后仍须重跑完整浏览器几何与实际导出证明。

修后截图目检另发现图页主导出按钮的文字不可见：本页引用的 `--lgi-signal-fill` 在当前运行 Token 镜像不存在，背景回落为透明而文字仍为 `--lgi-on-dark`。按本记录既有承字信号色缺档处理，以及本工作台其他主按钮的当前实现，局部改用已存在的 `--lgi-signal-ink` 配 `--lgi-on-dark`；不新增或修改 Token 真源。真实浏览器增加背景不透明及文字对比度至少 4.5:1 的检查，避免仅凭“使用 var()”漏过不可见动作。

该按钮的悬停背景同时明确使用既有 `--lgi-signal-hover`，避免父工作台通用按钮 hover 浅底盖掉主按钮承字底色；浏览器分别检查默认和 hover 两态的文字对比度，不扩展到其他按钮或全站样式。

### 2026-10-15 · 阶段 C 创作失败反馈中文化（实施前登记）

按 Mog 对工作台主文案不展示后端术语的要求，本页将 AI 失败原因、取消中与未知状态映射为中文，并给出保留输入、刷新结果、核对材料或模型设置等下一步。接口错误的未知错误码不再拼进主文案；来源状态附带的技术码移入按需诊断详情。原始 `ApiError.failure`、AI 状态与 `failureCode`、来源状态 DTO 继续原样保留，诊断与用户文案分责。本次只改创作页及其错误说明，不改变接口、操作、重试权限、收费边界、样式或全站规则；验证由页面与 API 的针对性回归承担。

## 2026-10-16 · WEB-REACT-001-E 运营台账五页 React 保真迁移（#438）

沿用 PAGE-OPS-001/002 的五页面、列、图表、状态和交互，样式改为 CSS Modules，Token/shell.css 不改；同包删除旧 Rust 输出、HTML/JS/CSS 与纯 DOM 测试。新增 JSON 解码、可取消的 query、目标冻结请求与真实回执边界。256/256 组当前 3000/最终同数据只读候选 RGB 像素一致（含显式周/月目标）；GPT 独立审查两项发现已修正并 PASS，必有 DTO 字段缺失不冒充未知/未迁入；真实写入和导出证明另在一次性 PG + 正式 Router + React 完成。见[阶段 E 验收](../acceptance/web-react-001-e-acceptance.md)。仍为候选，未部署/未 Mog 验收；保真不等于清除旧版全部 LIDS 欠账。


## 2026-10-16 · WEB-REACT-001-F 主题图谱与独立工作区 React 保真迁移（#440）

主图谱双视图、六页签、既有弹窗/绘图和独立 Workspace 改为 React/TypeScript/CSS Modules，保留当前页面布局与原绘图/聚合语义；原生 HTML/4资产及纯 DOM runner 同包退役。只消费当前 token/shell，未新增全站视觉方向。保真对照、正式隔离 Axum/PG/React 回执、性能及明确未覆盖边界见 [F 验收](../acceptance/web-react-001-f-acceptance.md)。独立审查的表现图平台状态、合法失效文本 null、已有机器候选工作区三项兼容问题已修正并复测；既有 Workspace 的固定人工标签/JSON 缺域链接/404空按钮明确登记，不将保真说成语义欠账已清除。候选未部署，Mog 签收仍待完成。


## 2026-10-15 · WEB-REACT-001-C-V3-FIDELITY 再实施前登记

Mog 退回 #432 的真实创作页面，要求按已认可 V3 重新落地。本轮沿用 CSS Modules 与唯一 LIDS Token，不再把功能清单作为设计验收。页面规格增加选题相关材料预备、检索加入本篇、完整原文与真实评论上下文；恢复 V3 纸条语义筛选/矩阵、连续正文/并排改稿和四步图文创作。唯一 React AppShell 可增加局部 compact/immersive 模式，此项覆盖阶段 B/C 先前不采用紧凑外壳的限制；不改旧页面外观或旧 shell。新增局部几何变量由实际实现清单追加，颜色/字体/间距仍引用 Token。当前仅已派定，尚无新候选视觉或链路通过证明。

本轮局部实现补记（实施前）：新增 `--cw-rail-width:13rem` 为V3工作区局部导航宽度，不新增全站布局；AppShell.module.css沿已有58px紧凑页头Token，只有显式workbenchMode消费，默认设置页不变。成品模板新增creation-v3-type/split/list与creation-renderer.v3，保留v1算法与已冻结历史成品的身份，配色仍沿cream/sage/ink。后端材料集合需要的窄类型/验证/来源传播修改由材料代理负责；root负责exports版本白名单，避免文件并行写冲突。

### 2026-10-15 · WEB-REACT-001-C · V8 动作按钮局部纠偏（实施前登记）

Mog 最新明确要求创作工作台动作按钮沿其提供的 `linggan-design-system-v8 (1).html` 动作层级（原件 CSS 第 536–589 行）实现，覆盖本轮临时采用旧 `signal-ink` 棕色主按钮与既有通用“禁止 hover lift”的限制，仅作用于工作台显式消费的共享 React `ActionButton`。导航页签、材料纸条、图页缩略图、模板及配色选择保持安静；设置页、旧原生页面和现有 Token 值保持原状。

共享组件放在 `apps/web/src/shared/ActionButton.tsx` 与同名 CSS Module，支持 primary/ink/outline/soft/danger/quiet 与 loading 状态。唯一 Token 真源 `apps/api/src/local_web/lids_tokens.css` 新增专用 `--lgi-action-*`，逐值映射 V8：主承字色 flame-600 `#c63214`、直角黑边、4px 硬投影、右缘点阵 idle 16px 一列 / hover 40% / active 满面及阶梯遮罩、离散四格 busy、虚线休眠 disabled、包住投影的双层键盘焦点环与减少动态效果。新增值只被该组件引用，不调整任何旧 Token。V8 明定 primary/ink 悬停 -1px、按下 4px；该局部显式授权不推广为全站动效例外。

请求语义保持：disabled 或 busy 均禁止原生激活，busy 用 aria-busy 并保留动作原有文案；四格为装饰，不能被辅助技术误报为四条状态。普通动作默认 type=button，表单提交显式 type=submit。实施后由组件聚焦/禁用/busy 回归、Token 检查、工作台真实浏览器逐态与既有冻结导出回归验证；本条不提前宣称已通过或已部署。

### 同轮 V3 材料纸面值回填

Mog 要求完整保留 V3 的书页与纸条体验；此前把暖纸统一替成结构灰会改变已经确认的构图。将原型 `v3/material-browser.css` 的 paper/edge/ink/selected/book-cover 原值分别登记为唯一运行时源的 `--lgi-reading-paper/edge/ink/selected/cover`，只供工作台材料阅读与摘录纸条消费。它们不构成全站第二主题；页面布局/文字字号仍沿现有 Token，旧 Token 不换值。回退仅撤掉本页引用和本次新增声明。需核对真实原文高亮和对比度；新值不证明材料资格或语义分类。


## 2026-10-19 · WEB-REACT-001 D3错误与领域收尾

线索来源读取失败的“重新读取”改为同一抽屉内重新GET，评论侧栏只携带服务器shell.selectedDomain，未确认领域不传播。无视觉值、页面结构或正常状态变化；4个回归反例先复现失败、修后26项通过；实际只读两宽6场景旧页失败/候选通过且保留16张原图，最终部署3000复验另记。此为既有错误/导航合同修复，不升级LIDS成熟度或代Mog验收。


2026-10-19 WEB-REACT-001收尾小项：AI格式资格/连接分别按成功、失败、未知显示，重开时读取当前配置；D3页头展示服务器采集状态，既有布局/Token不变。实际1440/390弹窗和页头截图GET-only核对、前端965项通过。四份当前页面规格的Token路径统一为apps/web/src/styles/lids_tokens.css；发布及Mog审美签收仍待完成。

同日最终措辞补充：连接false也可能正在探测，改为“模型连接测试尚未通过”；配置读取中/失败时不展示资格结论，已读资格仍区分成功/失败/unknown。增加对应时序反例，完整前端970项通过，页面结构/样式不变。

2026-10-19 WEB-REACT-001历史恢复P2：材料浏览器保持同一实例时，在稿件从已准备返回未准备后重置登记标记。既有样式/区位/准备规则不变；971项前端与C22步（两次恢复后精确摘录保存）通过，root目视核对390按钮启用与画布计数，正式3000待发布。


2026-10-19 · WEB-REACT-001收尾新增审查：PAGE-CONTENT-WORKBENCH-001缺标注不推断语义的既有合同落到materialReading，保留既有阅读提示与筛选，并在筛选/矩阵明确本机词语匹配未核对、非研究标注；评论原声仅表达来源类型。保留类型控件、矩阵/纸条/书页身份、布局、Token与引用范围；无新视觉规则或语义标注流水线。检索按所选材料族跳过无关读取，全类型与prepare原合同保留。反例与当前978前端及23项PG/22步浏览器证据见收尾验收§10；正式3000尚未部署本增量。


## 2026-10-19 · TOPIC-ATLAS-V3-001 · 紧凑证据带（候选）

Mog 批准 V3 HTML 的 React 主题图谱局部替代：面包屑双视图、去标题主题树、真实统计与六档分布同排、全宽材料阅读。全部消费既有 LIDS Token/CSS Modules，不新增全局纹理、色值或另一个设计系统。昨日读数在现有 API 中不存在历史证据，正式显示“—”而非复制原型 +86/+21/+53/+4；图表基于已加载作品而非全量服务端统计。来源、未知、动态高表现阈值及原有动作均维持，详见 TOPIC-MAP-COMPACT-382 当前增补。候选与运行部署分别验证。

## 2026-10-19 · TOPIC-ATLAS-V3-001 合并后视觉对照修复

用户对照 V3 HTML 与真实 React 截图，要求修复 KPI 基线错位、样本规则按钮在指标中抢占位置、右侧图高/色彩、左栏顶端冗余和标题筛选过量。本包复用 LIDS 现有字号、字距、间距、灰黑及信号橙红 token；三层 KPI、连续分布横轴和右图规则入口属本页局部 Pattern 调整，不修改 Token 或通用组件。不使用演示昨日数据；保留未知为 —。真实版式验收以 2048px Chromium 几何断言和全站回归补齐此前仅通过单元测试的空白。参见 TOPIC-MAP-COMPACT-382 当前增补与 Issue #448。

## 2026-10-19 · LIDS-ALIGN-001 · 规范采用与项目Agent渐进执行

Mog本次直接授权的文档交付包：基于main@9ff50b81，在专属分支codex/lids-review-alignment对齐已确认视觉、字体、领域位置、区域职责、FIND/输入焦点、纹理方向与A单行账本；项目AGENTS/CLAUDE/codx接入治理协议，213项审阅条目逐项映射正式落点/依据/待审状态，Q01–Q12保留未决参数。旧冲突按分册ALIGN标记局部替代，运行时Token镜像保持。仅新增/更新文档与治理验证器，无Issue外发、业务代码/资产/数据库/服务/部署变更；本地文档候选提交；未推送或合并主线，未宣布正式组件晋升或用户视觉验收。同步期间保留main已合入的#450记录；仅合并追加式文档冲突，业务代码来自该main基线，本任务无业务差异。

验证回执：213项清单/26模式与派生视图一致；10个错误声明反例被拒绝；按C16/P09/组件分类的渐进查询及未知ID拒绝检查通过；LIDS手册/运行时Token镜像检查、项目治理检查与diff空白检查通过。同步至9ff50b81后，对1352个tracked apps/crates/database/plugins文件的聚合指纹前后保持4995028146ca66c2a83a933f489fd0d8241ceaaa2f0f85b2015b9c39c3c1fe69；变更范围仅项目入口、docs和治理脚本。文档完整性不证明业务迁移、全状态视觉/键盘或Mog整体审美验收，主线合并与部署未执行。

## 2026-10-19 · LIDS-TYPE-001-B 采用与唯一参数收口

Mog明确“全部按照 B 来落实，确保当前的一致性，以及只有一个标准”。关闭Q01字号／行高和Q02字重，完整采纳TYPE-04中的B；新增typography.json作为唯一排版数值维护源，typography.md与独立审阅registry均生成并校验。旧七阶／三档字重在本范围被替代；对齐清单移除Q01/Q02未决引用，增加U09采用依据。项目AGENTS/CLAUDE/codx沿治理协议按需读同一来源。原声Sans/Serif、48px任务行高及Q03–Q12不自动采纳。

修改仅规范、项目治理验证和独立合成审阅工程；业务apps/crates/database/plugins、字体资产、运行Token、数据库、服务、全局配置与部署保持。候选仍在codex/lids-review-alignment，未推送／合并主线。验证回执随本节追加。

验证：213项对齐目录与生成字级表通过，12项反例被拒；B镜像与唯一源逐项一致；构建前守卫实测拒绝把数据字号从B14改为A11的漂移副本。UI设计手册、186项Token镜像、项目治理与diff空白检查通过；独立审阅构建和4项静态服务检查通过（保留既有>500kB包体提示）。6类浏览器场景实测B参数；普通创作者与嵌入预览均14/500，字体总览1.02行高残留修为B1.04，旧字重选择与A/C入口关闭，旧URL自调参数不能覆盖B。跟踪业务源与字体资产前后指纹一致；无推送、主线合并、业务迁移或部署。

## 2026-10-19 · Q03原声字族审阅与ALIGN-NAV-003

Mog要求同一长评论进入评论详情／阅读页，对照Sans与Serif，保持已采纳B且不改业务；另明确所有三级页面Tab固定归面包屑模块，为主要操作留空间。Q03继续待审，新增合成8段长评与本地Noto Serif SC试样（官方Google Fonts固定revision和OFL许可），只作用于隔离原声区；B参数源及原字体文件保持。共用PagePreview各多模式页及C25示例将三级Tab放入Z02，Z04-N保留职责ID；集合／展示切换和详情内部Tab按原职责就近。规则同步shell-zones、regions、patterns、项目Agent路由和逐项对齐；不改变原声正式规则、业务导航／路由、数据、服务或部署。检查与实际验证随本节追加。

验证：B参数源与既有字体SHA不变，业务跟踪文件前后指纹一致；源字体和许可固定revision。浏览器实测8段原文逐段一致，两字族均16px/400、29.6px行高；界面14/400及字族保持B，切换保留阅读锚点；并排两侧等宽；查找、详情开关、意见导出均验证，工具测试意见已清空。三级Tab在评论页、主题概览／旅程和C25均有Z02祖先且不在Z04主区或Z03侧栏；方向键焦点与Enter激活分离。390px产品预览无外溢且不缩字，段落索引局部滚动。修复隔离壳无侧栏仍留双列导致的窄列；重新测得805px/351px正常阅读列宽。14项文档反例、61项声明样例、项目治理／手册／Token镜像／B构建守卫通过，审阅构建及4静态服务检查通过；保留既有包体提示。仅本地文档分支与隔离审阅工程，无推送、合并、业务迁移或部署；Q03仍待Mog决定。

## 2026-10-19 · ALIGN-VOICE-004收口与Q04-FOCUS-07审阅

Mog明确Q03原声统一OPPO Sans，继续审Q04，B及三级Tab归面包屑规则不变，业务不动。typography.json新增evidence字族并关闭Q03，B全部roles／weights及原字族原样；typography、primitives、patterns、system与tokens目标说明按新决定替代强制Serif，运行时镜像字节不改。项目AGENTS/CLAUDE/codx与L1路由同源；D06只在原声字族范围确认，不晋升C40或整套组件。旧voice-06与Noto样例保留历史身份，当前详情／阅读页仅Sans。

Q04仅有无外圈／键盘可见原则已确认，A现稿（FIND反白、暖底、字段3px信号内底边）、B白底2px墨色内底边、C鼠标暖底与键盘反白／2px墨色内底边均为候选。独立审阅复用Find、Select、Button、PagePreview，提供组件／创作者／新建采集表单片段；操作及意见仅内存、无业务API。精确方案不进入全局控件CSS、正式Token或产品默认。

浏览器已核对三方案实际样式：字段／搜索为B输入13/400/1.5，原声详情／阅读16/400/1.85并用OPPO；外圈为none，候选底线均为inset。A信号对暖底5.52、墨色对白18.46仅为实测纯色对比，不等于完整无障碍认证。搜索林间命中1条，切候选保留条件；错误就近文字与定位、下拉Escape不提交／Enter提交、只读可聚焦及disabled跳过均实际验证。390px产品预览与搜索无整块外溢，表格366→700局部横滚，字号不缩小。构建、声明及治理结果与未验证边界保留在本机typography-review/focus-07回执；未推送／合并／业务迁移／部署，Q04继续待Mog决定。

## 2026-10-19 · ALIGN-FOCUS-005采用C与Q05-MATERIAL-08

Mog确认Q04采用C（鼠标轻输入、键盘清晰定位），继续审Q05，保持字体、B与ALIGN-NAV-003，业务不动。Q04从待审清单移至已解决，focus.json为已采纳参数唯一源；primitives说明输入方式、只读／disabled／错误分责及深底领域保留可见Signal内底边。项目Agent入口同路由，旧焦点候选记录降为历史；审阅通过sync-lids-focus.py生成镜像并在predev/prebuild校验，所有隔离组件／代表页面共用InputMethodScope与焦点CSS，当前focus入口仅C。业务Token镜像及CSS未迁移。

Q05在同一组合提供M1当前墨色9px纹理／无静止投影，M2深橙主动作／2px硬投影，M3的8px低强度纹理探索；参数唯一在隔离src/materialReview/presets.json，所有三组与临时微调均待审。场景复用Button／Select／FIND／ProductShell／PagePreview，涵盖组件、采集任务、创作者与评论研究；可切状态样张、实际忙态、菜单／领域、原位参数微调与意见导出。材质不铺正文，M-02E、M-06及工作台V8范围不变，未新立正式CMP或材质Token。

浏览器实测三组颗粒／强度／影子／底色均匹配，主按钮13／600、忙态208px稳定且native disabled；M2悬停上移1px、按下2px且影子消失。P09四行各48px且nowrap，切材质保留查询；P05与P04的390px样本整块无溢出，表格局部滚动，三级Tab归Z02。共享浮层实际鼠标暖底无外圈／无内线，Tab回到输入显示2px墨色内底边，Escape回原按钮。发现共享引用样例遗留section字号!important覆盖B；仅删除隔离旧覆盖，复测原声为OPPO16／400／29.6px，未修改B角色值。截图／实测与构建、声明、治理结果留同任务material-08；本轮无业务／字体资产／模型／数据库／全局设置／部署，主线集成和Mog对Q05的审美决定未发生。

## 2026-10-19 · LIDS-ALIGN-001：Q05采用M2（ALIGN-MATERIAL-006）

- Mog选择“还是喜欢M2”，U13登记整组采用；control-material.json成为按钮／壳层材质唯一手工参数源。materials与primitives明确局部替代旧4px／7px投影、双轴位移及D07墨色无投影候选；Q05关闭，Q06–Q12继续待审。
- 项目AGENTS／CLAUDE／codx沿既有渐进路由按需进入materials；sync-lids-material.py生成隔离materialApplication镜像，predev／prebuild拒绝SHA或内容漂移。当前审阅组件与代表页面共用M2，M1／M3与微调退出活跃入口，采用前material-08证据不改写。
- 保持B字体／字号／行高／字重（只更新待审清单）、OPPO原声、Q04 C焦点、Z02三级导航及M-02E／M-06／V8_workbench范围；无业务CSS／Token／字体资产、运行／数据库／服务／全局配置／部署变化。未推送或合并主线，组件认证与业务验收独立。验证结果见本节后续回执与同任务material-adoption-09。

验证回执：213项对齐及16项反例、B／C／M2同源镜像、正式UI手册与运行Token镜像、项目治理、diff空白检查通过；隔离构建和4项静态服务检查通过（既有包体>500kB提示保留）。10项浏览器核对包括普通C09及P09共享M2、旧M1/M3链接归M2、忙态防重复／宽度、菜单投影、C鼠标／键盘焦点、按钮位移样张与390px评论页Z02三级Tab；无新增控制台错误。1352个业务源文件、17个字体文件指纹不变，B值与C源不变；在临时副本将M2纹理步长改为9时同源守卫拒绝。共享变量作用域问题在测量时发现并修正，最终深橙底为#ba381b。未推送、合并或部署，未批准Q06–Q12。

## 2026-10-19 · LIDS-ALIGN-001：Q06容量与窄屏收纳审阅

- Mog要求组件及代表页面交互对照；Q06-SHELL-10保持待审，A横轨／B折叠抽屉／C当前项＋更多仅隔离候选。字体、B角色、C焦点和M2源不变，领域／三级Tab归Z02；不改业务、运行或部署。
- 复用个人菜单、Select、TertiaryTabs及PagePreview内容，隔离内容插槽不改变原默认页面。工具支持五档宽度、领域五态、长名称、时钟占位、实际容量与审阅意见导出；旧326／400／120px与时钟去留单列，不自动取代。
- 来源、模块读取与待审边界同步shell-zones／review-items／对齐清单及项目索引，固定证据为同任务typography-review/shell-10。验证回执后续追加；不宣称Q06已获审美采用或完成全站迁移。

验证回执：最终三案×五档宽度15组容量核对均无整壳横溢、三级导航唯一且归Z02，维持B的13px控件角色；另32项交互／回归记录覆盖B抽屉与焦点返回、C更多菜单、A键盘完整标签、长路径及领域五态、查询保持、时钟占位、组件及单行任务、原C01与PagePreview默认壳。3处压力测试问题已修复并保留诊断：末尾Tab局部显露、C菜单边界、B入口宽度上限的换行。新增控制台错误0。审阅意见JSON内容及未批准状态已核对，工具意见已清空，浏览器下载落盘位置未认证。213项声明与18项反例、B／C／M2同源守卫、61个声明样例、10个生成参考页、UI手册／项目治理、隔离构建与4静态服务检查通过；既有>500kB提示保留。1352业务文件、17字体文件及三个已确认参数源字节不变；Q06仍待Mog审阅，未推送／合并／部署。


## 2026-10-19 · ALIGN-SHELL-007采用A与Q07-LEDGER-11待审

U15采用Q06 A并补充手动图标折叠；shell-layout.json为唯一源，生成镜像与predev/prebuild守卫防漂移。C01/C03/Q06/Q07使用同一ShellFrame，旧B/C退出活跃入口，历史证据保留。Q07比较三种列结构、四档行高、自定列宽与字段优先级，组件和P09共用同一批fixture；一任务一行，B字体/C焦点/M2材质和Z02规则固定。Q07–Q12保持待审；业务/字体资产/运行Token/服务未改，无推送、合并或部署。证据登记在同任务shell-adoption-ledger-11，验证回执随后追加。

验证回执：最终三案×五档宽度15组均保持8条任务、单tr/48px、正文14px/操作13px及整壳横溢0；另记录42项交互与绘制检查（含1项共享提取回调问题，已修复复验），覆盖44/48/52/56px、字段P0锁定、超宽名称解除吸附、查询/选择保留、列组、领域/筛选、长值详情、Escape返回和JSON意见。C01/C03/Q06/Q07共享图标折叠；旧B/C入口归A、窄屏三级Tab完整显露且归Z02。控制台新增错误0。213项声明与21项反例、B/C/M2/A同源守卫、临时副本B漂移拒绝、61个声明样例/10个参考页、UI手册/项目治理/diff与构建/4静态服务检查通过；既有大包提示保留。1352个基线业务文件与17个字体文件哈希不变；B仅移除Q06待审元数据，全部字族/角色/权重参数不变，C/M2字节不变。仅容器宽度模拟，不冒称原生移动端或真实业务/屏幕阅读器验收；Q07仍待Mog选择。改动留在专属工作树，尚未新增提交、推送、合并或部署。


## 2026-10-19 · ALIGN-LEDGER-008采用与Q08-COMPOSITION-12待审

U16选择L1/44px，当前任务名称滑杆144px留证；其他列保留L1各宽度。ledger-layout成为唯一源，生成镜像/守卫接入；共享账本、普通P09、Q06/Q08与原完整任务稿同源；L2/L3退历史。Q08提供11页型/17页/26模式的适用分带/合带/常驻，区域、组件状态与动作归属可检视；业务不连接，Q12例外不取消。字体B/C/M2/A源参数保持，未新增提交/推送/合并/部署；证据登记ledger-adoption-composition-12，验证待后续回执。

验证回执：30种适用组合×五档宽度150组、26模式×宽窄52组最终无意外横溢，领域/三级Tab归Z02；任务15组均44px/名称144px/17列。另35条交互与诊断记录覆盖查询/草稿保持、对象资格与焦点返回、表单归属、每日数据不分业务、状态/组件、字段P0、键盘横滚、意见JSON和窄屏菜单。审阅标注4px外溢、对象预览焦点丢失、旧任务稿48px排版覆盖三处问题已修复复验并保留历史，控制台错误0。213项声明/24项反例、五源镜像与临时48px漂移拒绝、61静态样例/10参考视图、手册/项目治理/diff、构建和4静态服务检查通过，保留既有大包提示。1352业务文件/17字体文件哈希不变；B仅关闭Q07待审元数据，其余角色值及C/M2/A字节不变。仅容器模拟和本地审阅验证，不是全部真实业务状态或移动端认证；Q08待Mog选择，治理工作树未新增提交、推送、合并或部署。

## 2026-10-19 · ALIGN-COMPOSITION-009：Q08含搜索列表B/其余功能C与卡片保留

Mog U17采用检索B/功能C并保留用户问题现有卡片；U18进一步明确所有含搜索的列表用B，覆盖卡片、研究批次和采集任务。composition-layout.json为唯一模式组合源，26模式逐项映射；入口表单A豁免，卡片不改为通用表格。用户问题真实卡片结构/材质在独立原型以合成文本补齐，保留阅读/紧凑密度；业务来源组件/CSS不动。项目AGENTS/CLAUDE/codx按patterns渐进展开，Q08关闭；Q09–Q12、业务合同和组件认证不随视觉采用批准。此前三案/简化表格与12证据保留历史，13证据已登记；验证回执随后追加。

验证回执：26模式×三档容器宽度78组采用映射正确，无意外横溢，领域/三级Tab归Z02；P09仍44px/名称144px/17列。10项交互核对涵盖卡片搜索/阅读与紧凑/线索语义/详情焦点返回、C常驻与局部表单，卡片引用额外装饰已修正；控制台错误0。六源镜像、错误模式镜像拒绝、真实卡片JSX/CSS结构比对、213对齐/27反例、61样例/10视图、手册/治理/diff与构建/4服务检查通过。1352业务文件和17字体文件指纹未变；B仅关闭Q08待审元数据，C/M2/A/L1源字节不变。完整回执见同任务composition-adoption-13；仅容器与独立审阅验证，Q09–Q12、生产接入/组件认证/业务验收均未扩大，未新增提交、推送、合并或部署。

## 2026-10-19 · Q09-OVERLAY-14浮层、离开与长任务交互审阅

按U19搭建独立review=overlays，3种只读详情形态、S1/S2策略与5场景；纯内存状态演练和手动合成回执，沿现有原请求/停止不可继续/材料保留合同，不接业务。复用已采用B/C、卡片、Z02及固定参数；共享PagePreview/TaskLedger只增可选回调与受控演练值，默认行为保持。七项候选合同和实际源码差异进入interaction-contracts，Q09仍待审，未批准Q10–Q12。新目录/证据已登记overlays-14；验证随后追加。无业务/运行/字体资产/全局变更或新增提交推送合并部署。

验证回执：45组（5场景背景×3详情形态×1440/768/320px）无页面/壳/浮层/正文意外横溢，47条浏览器观察覆盖焦点、遮罩、脏稿、Z02离开、提交跟踪、同身份核对、迟到回执、停止/继续、材料与配置对象隔离；另核对320px表单和Q08默认卡片/详情。提交回执替换后焦点失位、配置示例绕过保护、停止后旧“可继续”提示三处已修复复验。控制台错误0。六源字节与镜像守卫、213声明/30反例、61样例/10视图、UI手册/治理/diff、隔离构建和4服务检查通过；保留既有大包提示。1352业务/17字体/真实与隔离卡片源不变。完整证据见同任务overlays-14；诊断探针和工具超时记录保留。仅内存合成演练，原生浏览器离开提示/刷新稿件恢复/移动端/屏幕阅读器及真实命令未认证。Q09仍待选择，未新增提交、推送、合并、部署。

## 2026-10-19 · ALIGN-OVERLAY-010采用S1与Q10-COMPONENT-15待审

U20明确采用Q09 S1，保留脏稿保护、原请求跟踪和停止不可恢复。唯一机器规则overlay-policy.json，经sync-lids-overlay生成审阅镜像与第七个predev/prebuild守卫；旧S2退出活跃入口，14证据保留。只读形态仍按对象合同，S1采用不新增全球唯一抽屉形态。组件、32交互逐项认证和Q10–Q12不继承批准。

Q10独立review=components提供48候选目录、合法变体/状态、3代表页及8个复用边界案、7项晋升自查和意见JSON。基础复用/页面适配/领域专属/先补合同为待审分类；现存源码观察、隔离演示和正式CMP证据分开。新目录及15证据创建前登记；共享卡片、Z02、B/C、44/144保持，业务代码/Token/字体/运行/全局不动，无新增提交推送合并部署。验证回执随后追加。

验证回执：Q09 S1采用后7条回归通过，旧S2归S1、脏稿保护/原身份收起跟踪、停止不可恢复与在途材料保留已核对。Q10有48宽+48窄容器默认预览、9代表页布局及36交互观察，整页/壳意外横溢0，两个页控制台错误0；FIND防抖/提交差异、44/144/17字段、原卡片/Z02、冻结非模态对象、意见不批准均留证。七源守卫/S2镜像反例、213对齐/36反例、61样例/10视图、手册/治理/diff与构建/4服务检查通过，保留既有大包告警。1352业务/17字体/卡片源不变；B只去掉Q09待审元数据，角色值及其余五源相同。逐C查询只披露一项候选边界。Q10仍待审，默认结构不冒充全状态/读屏/原生移动/生产认证，无新增提交推送合并部署。证据为同任务overlay-adoption-components-15。

## 2026-10-19 · ALIGN-REUSE-011采用与Q11-FONT-16待审

U21明确采用基础复用/页面适配/领域专属/先补合同四类，七项晋升门槛保持，48项分类不授予CMP或完整候选合同。component-reuse.json为唯一源，生成spec及catalog镜像，第八个守卫防漂移；逐C输出componentUseRule和未认证状态，原Q10待审转为已采用路线。

Q11提供现有本机4原文件/5FontFace对象加载、受控缺失、两类回退、原字重范围、Manrope数字测量与代表页。B角色不变，不重新选字族或数字字体；受控缺失不破坏文件、首访/弱网及全部字形未宣称。业务/字体资产/运行/全局不动，创建前登记16证据，验证待追加，无新增提交推送合并部署。


2026-10-20 · ALIGN-REUSE-011 / Q11-FONT-16验证：唯一四类源/镜像及七门槛，213项/40反例与61声明样例核对通过，Q11隔离加载/回退/字重/数字与27布局组合及意见导出已验证，详见progress的本轮回执及登记证据reuse-adoption-fonts-16。1352业务、17字体/许可、B角色数值与卡片源未变；原始热刷新诊断保留，完整重载后的控制台0。Q11/Q12仍待审，运行字体、性能预算、全部字形和组件认证未扩大授权。


## 2026-10-20 · ALIGN-FONT-012采用与Q12-EXCEPTIONS-17待审

U22采用Q11自动换入/F2回退、普通数字右对齐/小数列小数点对齐。唯一typography.json纳入deliveryDecision和完整F2序列；B角色数值与原字族、字体资产/许可不变。sync-lids-typography及第一守卫扩展到交付字段，独立审阅共享应用，Q11旧候选退出当前选择，只留16历史。项目Agent入口沿typography渐进展开，不改运行Token或业务。

Q12提供作品卡M-06、媒体contain/缺失态、工位M-02E两处的可操作E1/E0对照；封面卡测量舞台与M-01区位限制差异单独待决，工作台V8范围和用户问题卡片保持。新目录创建前登记于font-adoption-exceptions-17，源/状态/依赖/验收见README；验证回执随后追加。Q12仍待Mog，未授予组件认证/真实命令/字体资产迁移，无提交推送合并部署。


验证回执（2026-10-20）：Q11自动换入/F2已在共享审阅字体中应用，加载5对象无手动放行；普通右缘/小数点对齐实测，19.50及2.000125位数与未知保持。B角色值/字重映射、1352业务文件、17字体/许可、真实及隔离问题卡片、原fonts.css/system.css均未变，其余7采用源字节相同。Q12的18组双材料/三页/三容器和12组媒体比例/状态无页面/壳意外横溢，阅读16px/白场、Z02和局部8px工位按钮范围保持；S1/焦点、检索、翻面/选中、未知与合成回执/意见导出留证，控制台0。E0伪元素选择器问题已修复重验；首次清空不变的观察保留诊断，独立复点和两次循环通过。阅读页模块上下文与已启用侧栏title已核对修正。213对齐/45反例、八源/字体镜像反例、61声明/10视图、手册/治理/diff、构建/4打包测试通过，原大包告警保留。证据font-adoption-exceptions-17；Q12/Mog总体设计及真实运行/媒体/工位/弱网/读屏/全产品材料预算未认证，无提交推送合并部署。

## 2026-10-20 · ALIGN-MEDIA-013 / LIDS-CLOSEOUT-18

U23采用Q12 E1及媒体白场，九源/镜像/构建守卫与项目Agent渐进入口统一。普通页及历轮代表页使用同一B/C渲染器，旧视觉提案/交互壳标历史；B主按钮500、密度不缩行高、媒体白场、Q03不阻塞回退、F2诊断继承、只读遮罩与局部未保存保护均核对收正。213条对齐及46反例、17页26模式宽窄和组件默认样张/关键交互证据见closeout-18及整体审计报告；色板A18-01待裁定，其他CMP/业务/性能/主线验证缺口单列，不重开Q项。业务/原字体/卡片源保护；无新增提交/推送/合并/部署，不能宣称其他checkout或主线Agent已获得这些未集成规则。


## 2026-10-20 · LIDS-UI-001 本地正式前端候选

基于 origin/main@612dcbea 实施共享基础、原字体/许可、A壳和任务L1。独立治理树按共同祖先三方衔接，保留最新React规则与评论Z02读数合同；旧只读规范包的范围不再限制本次直接授权。运行值由 sync-lids-runtime.py 映射唯一Token，具体已证明/未证明见本轮验收；main/3000尚未变更。


## 2026-10-20 · ALIGN-WORKBENCH-015 工作台动作按钮并入 M2

LIDS-UI-001 两页发布补验时发现：共享 `ActionButton`（V8 动作族，主干上仅内容工作台使用）随创作者页、证据库页采用 M2 而整体换参，工作台浏览器证明因按钮外观断言失败。Mog 在发布核对中选择「工作台也变成新样式」，裁定记入 decisions.md；`tokens.md`、`primitives.md`、`materials.md`、`patterns.md`、`component-promotion.md` 中「V8 工作台按钮保持」的说法加替代标记，历史数值保留。证明侧：`workbench-real-chain.mjs` 的按钮断言改读 control-material.json；无新增后端、迁移、业务 Token 或字体变化。工作台按钮外观在 3000 上随发布变化（阴影变薄、右缘纹理更密），功能与请求语义不变。

## 2026-10-20 · COLLECTION-LIDS-UI-001 三页候选

当前对话直接派定，基于#457@4700352b，单Agent专属树。采集目标/任务B、工位C；采用B字级/M2/C焦点/L1账本/Sidebar及现行唯一Token；不修改规则参数或全站样式。目标固定列/局部横滚、新建所属表单与草稿收起，任务17字段一行44px/144px与字段显隐，工位常驻工具及管理入口保留。读数“已有插件”按权威active_plugin_version计数，不假称心跳在线。

strict类型/lint/92文件1046项/构建，Token/LIDS46反例/手册/治理通过；1440/1280/820/390真实视口三页无根/main横溢出，任务44px。详情/返回/字段/空结果和管理读取通过；真实写、独立复审、CI、main/#457整合、3000、采集与Mog签收未验证。见[验收](../acceptance/collection-lids-ui-001-acceptance.md)。

## 2026-10-20 · COLLECTION-LIDS-UI-001 截图反馈续修

按Mog七张截图：目标标题上移6px、吸附格整行hover、编号/正向新增红色、两类目标独立冻结字段及横向同步；任务旧前端诊断退役、详情按实际面包屑底沿定位并压缩为672px业务进展/成果、FIND完整边界与M2提交。唯一Token/共享壳层/后端历史不改。最新strict类型/lint/94文件1053项/构建通过；真实hover、两类冻结、搜索清空、1280/820/390详情边界和长成果内部滚动留证，详见本包验收。正式运行、main/CI/独立复审及Mog签收仍分层待验。

## 2026-10-20 · COLLECTION-LIDS-UI-001 工位管理弹窗

按Mog最新直接裁定，Z02工位管理由大页面转居中ModalDialog，复用同一唯一Token/ActionButton/原CommandForm；四页签只保留工位及能力/新增、当前执行、自动排程、插件认领，旧runtime深链保留安全返回与工位定位。运行大看板与概览退役，所含后台数据不删。新增按钮右对齐、侧栏分组字样删除。能力分栏、中文名称，最近安装优先与历史折叠、空选择禁止认领。权限/命令/未知与真实0保持，验证结果见本包验收第三轮。
