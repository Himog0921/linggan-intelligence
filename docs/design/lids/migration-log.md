# LIDS-LOG-001 · LIDS 迁移与变更记录

> 状态: 权威当前
> 最后核对: 2026-09-02
> 适用范围: Linggan Intelligence LIDS Token、Primitive、Component、Pattern、Page、Motion、Scene 和 Data Truth 规则的实际变更、替代、例外与验证边界
> 事实来源: [system.md](system.md)、[README.md](README.md)、DESIGN-002 Issue #7、项目 progress 记录和实际验证输出
> 冲突时以谁为准: 真实代码/合同/测试、用户最新确认、当前 SCOPE 和 ACCEPTED 决策；本日志不把计划写成已实现事实

任何影响 LIDS 五层或横向约束的事项必须在同一 PR 更新本记录：变更是什么、取代什么、影响页面/组件、验证结果和仍未证明什么。日志不是路线图，更不是运行时真相。

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
