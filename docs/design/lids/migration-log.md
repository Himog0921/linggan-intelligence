# LIDS-LOG-001 · LIDS 迁移与变更记录

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: Linggan Intelligence LIDS Token、Primitive、Component、Pattern、Page、Motion、Scene 和 Data Truth 规则的实际变更、替代、例外与验证边界
> 事实来源: [system.md](system.md)、[README.md](README.md)、DESIGN-002 Issue #7、项目 progress 记录和实际验证输出
> 冲突时以谁为准: 真实代码/合同/测试、用户最新确认、当前 SCOPE 和 ACCEPTED 决策；本日志不把计划写成已实现事实

任何影响 LIDS 五层或横向约束的事项必须在同一 PR 更新本记录：变更是什么、取代什么、影响页面/组件、验证结果和仍未证明什么。日志不是路线图，更不是运行时真相。

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
