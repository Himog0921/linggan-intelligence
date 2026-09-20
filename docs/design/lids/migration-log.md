# LIDS-LOG-001 · LIDS 迁移与变更记录

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: Linggan Intelligence LIDS Token、Primitive、Component、Pattern、Page、Motion、Scene 和 Data Truth 规则的实际变更、替代、例外与验证边界
> 事实来源: [system.md](system.md)、[README.md](README.md)、DESIGN-002 Issue #7、项目 progress 记录和实际验证输出
> 冲突时以谁为准: 真实代码/合同/测试、用户最新确认、当前 SCOPE 和 ACCEPTED 决策；本日志不把计划写成已实现事实

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
- **Token**：不新增、不改值。新增样式只消费既有 `--lgi-*` 与 `DESIGN-010-UI-EX-01` 例外块内已登记的 `--c-*` 别名；新登记一个页内变量 `--c-runtime-drawer: 356px`（抽屉宽度，与稿中的 `--runtime-drawer-w` 同值）。`lids_tokens.css` 未改。第五轮三个按钮用的 `--v7-black` / `--v7-red-hover` / `--v7-white` 均为 `shell.css` 已有的 v7 别名，不是新值。
- **材料**：深色只出现在两处局部面（仪器表头、抽屉执行结果卡），沿用 `M-05` 的既有深色面语法与 `--lgi-stream-*` 取值；稿中的「传感器」斜纹与点阵未引入。
- **语言**：稿中描述性英文标签（`LIVE` / `SUCCESS` / `FAIL` / `TOP 3 · 24H` / 各模块英文小标题）一律不加，按 `LANG-05` 只留中文。
- **数据诚实**：稿中抽屉三块内容（回传成功率与柱状图、补采失败任务计数、失败原因 TOP 3）在本系统**没有真实读模型**，按 `data-boundaries.md` 换成四块真读得到的内容；稿中数值一个都不出现。逐条偏离见清单 §4d。
- **验证**：`cargo test -p linggan-api --bin linggan-api` **240 passed; 0 failed; 21 ignored**，与改动前基线一致；抽屉开合契约、把手与控制条动作按钮的几何关系、`.c-runtime` 子树溢出扫描均为脚本实测。**未部署、未推送、未合并**，本机 `:3000` 上尚不可见。
