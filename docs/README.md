# 文档总索引

> 状态: 权威当前
> 最后核对: 2026-09-01
> 适用范围: 全仓库文档导航、权威状态与渐进式披露
> 事实来源: 当前 Git 文件树、`AGENTS.md` 与文档状态头
> 冲突时以谁为准: `AGENTS.md` 和真实代码、合同、测试、运行结果

本页是项目的“总书目”。Agent 不应先通读所有材料，而应从这里逐层进入与当前事项直接相关的文件。

## 固定阅读顺序

1. 根目录 [`AGENTS.md`](../AGENTS.md)：最高约束、事实优先级和不可违反规则。
2. [`current-state.md`](current-state.md)：当前阶段、唯一在办事项、阻塞和下一步。
3. 本页对应分类中的权威文档。
4. 只有当前任务明确需要来源核对时，才进入 [`references/README.md`](../references/README.md)。

## 当前权威与状态

| 文档 | 状态 | 用途 |
|---|---|---|
| [`current-state.md`](current-state.md) | 权威当前 | 当前阶段、事项队列和决策缺口 |
| [`development-stage-tracker.md`](development-stage-tracker.md) | 权威当前 | 从开发基线到真实运行与业务验收的全项目阶段总表、阶段证据和 Mog 跟进入口 |
| [`governance/file-placement-standard.md`](governance/file-placement-standard.md) | 权威当前 | 文件分类、命名、放置、归档与生成物规则 |
| [`governance/agent-collaboration.md`](governance/agent-collaboration.md) | 权威当前 | Agent 读取、任务、冲突、更新和验收流程 |
| [`governance/generated-artifacts-registry.md`](governance/generated-artifacts-registry.md) | 权威当前 | 所有生成型文件的固定位置与入库许可 |
| [`agents/issue-tracker.md`](agents/issue-tracker.md) | 权威当前 | GitHub Issues 的 Agent 任务追踪与授权边界 |
| [`agents/triage-labels.md`](agents/triage-labels.md) | 权威当前 | GitHub Issue 的五类任务分流角色 |
| [`agents/domain.md`](agents/domain.md) | 权威当前 | 工程技能读取 Linggan 领域语言、决策与不变量的适配规则 |
| [`agents/scope-001-execution-contract.md`](agents/scope-001-execution-contract.md) | 权威当前 | SCOPE-001 的 Closed World、状态分责、unknown、语义 Oracle 与证明边界执行合同 |
| [agents/ui-execution-contract.md](agents/ui-execution-contract.md) | 权威当前 | 所有 UI 协作 Agent 的读取、闭集执行、停工、验收与交接合同；不授予 UI 实现范围 |
| [`context/START-HERE.md`](context/START-HERE.md) | 权威当前 | 新机器和新 Agent 的项目背景入口 |
| [`context/current-system-inventory.md`](context/current-system-inventory.md) | 代码事实优先 | 固定来源资产及已证实能力盘点 |
| [`context/discussion-decisions.md`](context/discussion-decisions.md) | 权威当前 | 已确认结论和待决定事项摘要 |
| [`context/domain-language.md`](context/domain-language.md) | 权威当前 | DISC-001 已确认的产品、领域与采集责任共同语言；不预设数据库对象 |
| [`decisions/0001-greenfield-rust-clean-db.md`](decisions/0001-greenfield-rust-clean-db.md) | 权威当前 | 已接受的 Rust 与全新 PostgreSQL 决策 |
| [`product/PRD.md`](product/PRD.md) | 草案 | DISC-001 的产品输入，不是已接受实现合同 |
| [`product/domain-invariants.md`](product/domain-invariants.md) | 权威当前 | 跨 Gate 的领域不变量、判断资格与持续扩展的对抗性验收案例 |
| [`product/collection-monitoring-rules.md`](product/collection-monitoring-rules.md) | 权威当前 | 博主/关键词监控的产品规则：目标生命周期、深度建档、频率、爆款追踪；含 4 项待 Mog 决定 |
| [`plans/completed/observation-runtime-001-implementation-manual.md`](plans/completed/observation-runtime-001-implementation-manual.md) | 已完成；Issue #94 / PR #95 / PR #96 | 观察规则自动调度、插件无人领取、Package/Receipt、封面材料投影、发布与一次性验收的实施规格和已完成边界 |
| [`data-contracts/local-001-discovery-evidence-boundary.md`](data-contracts/local-001-discovery-evidence-boundary.md) | 权威当前 | `LOCAL-001 / 001C-0` 与 #34 localhost binding：平台 discovery 与本地 Evidence Library 检索、partial Coverage、DiscoveryOccurrence、接纳回执与本地封面引用的跨边界合同；不证明真实插件或媒体取得 |
| [`architecture/material-projection-data-map.md`](architecture/material-projection-data-map.md) | 权威当前；MATERIAL-PROJECTION-001 | 最新 Browser Producer 产出到服务接纳、作品级材料投影与 Evidence 列表/详情/受控媒体读取的逐 lane 映射及代表性 fixture |
| [`architecture/work-resource-read-contract.md`](architecture/work-resource-read-contract.md) | 权威当前；Issue #110 / #128 | Intelligence 跨页面唯一作品资源读取入口；`0.8.28` 真实链已验收封面/头像/作者-目标分责，评论图片非空真实样本仍未观察 |
| [design/README.md](design/README.md) | 权威当前 | 前端设计手册入口、权威地图、读取路径和当前已建立/未建立边界 |
| [design/design-governance.md](design/design-governance.md) | 权威当前 | UI 设计来源优先级、闭集执行、变更分类、例外和验收治理 |
| [design/reference-register.md](design/reference-register.md) | 权威当前 | 设计参考的登记格式与“参考不等于指令”边界；LIDS v2.0 的可继承/不继承边界已登记 |
| [design/lids/README.md](design/lids/README.md) | 权威当前；Proposed | LIDS v2.0 全项目设计表达标准的入口、来源回执、权威边界和成熟度 |
| [design/lids/system.md](design/lids/system.md) | 权威当前；Proposed | LIDS-SYS-001：Token→Primitive→Component→Pattern→Page、L1/L2/L3、状态、动效、场景、a11y 与治理 |
| [design/lids/tokens.md](design/lids/tokens.md) | 权威当前 | LIDS-TOK-001：完整 `--lgi-*` 数值基线与未来唯一 Token 真源迁移规则 |
| [design/lids/primitives.md](design/lids/primitives.md) | 权威当前 | LIDS-PRI-001：文字、按钮、状态、Surface、Readout、反馈和 Focus 的基础契约 |
| [design/lids/patterns.md](design/lids/patterns.md) | 权威当前 | LIDS-PAT-001：L1/L2/L3 页面 Pattern、结构与组合限制 |
| [design/lids/agent-execution-guide.md](design/lids/agent-execution-guide.md) | 权威当前 | LIDS-AGENT-001：UI Agent 强制决策树、禁止项和交付前检查 |
| [design/lids/language-policy.md](design/lids/language-policy.md) | 权威当前 | LIDS-LANG-001：中文主表达、英文技术旁注、状态诚实性和有界页面迁移规则 |
| [design/lids/prototype-audit.md](design/lids/prototype-audit.md) | 权威当前 | LIDS-AUD-001：Observatory V3 原型的品牌母题与非授权边界 |
| [design/lids/migration-log.md](design/lids/migration-log.md) | 权威当前 | LIDS-LOG-001：设计系统迁移、替代、例外与验证记录 |
| [design/templates/page-spec-form.md](design/templates/page-spec-form.md) | 权威当前 | 获准页面的设计执行规格表单 |
| [design/templates/component-spec-form.md](design/templates/component-spec-form.md) | 权威当前 | 可复用组件规格表单 |
| [design/templates/ui-change-manifest-form.md](design/templates/ui-change-manifest-form.md) | 权威当前 | 每次 UI 变更的来源、范围、影响与证明清单表单 |
| [design/templates/visual-acceptance-form.md](design/templates/visual-acceptance-form.md) | 权威当前 | 视觉、任务、状态和真实后果四层验收记录表单 |
| [design/pages/evidence-library-page.md](design/pages/evidence-library-page.md) | 权威当前 | `PAGE-EVIDENCE-001`：多材料 Evidence Library 产品手册与技术呈现要求；运行页已按现行 Material Projection 落地作品 lane 与 Inspector，真实垂直证明仍未完成 |
| [design/changes/work-resource-read-001-ui-change-manifest.md](design/changes/work-resource-read-001-ui-change-manifest.md) | 交付分支实现；Issue #110 | Evidence Library 的共享 Work Resource 接口、作者/目标诚实表达、时间来源状态和研读/表格/封面三种排版变更清单 |
| [design/changes/evidence-v9-001-ui-change-manifest.md](design/changes/evidence-v9-001-ui-change-manifest.md) | 交付分支实现；EVIDENCE-V9-001 | `/corpus/evidence` 的 V9 研读密度、行级原声引用读能力、Inspector 三档宽度与 LOCAL MEDIA 横向材料浏览变更清单，含七处与 V9 的有据偏离 |
| [design/pages/evidence-library-multi-material-reference.html](design/pages/evidence-library-multi-material-reference.html) | 权威当前；静态参考 | `EVIDENCE-PAGE-002`：多材料 Evidence Library 合成高保真参考；不连接 API、真实平台、媒体或处理器 |
| [design/pages/collection-workspace-page.md](design/pages/collection-workspace-page.md) | 权威当前；PAGE-COLLECTION-001 | Collection 五个子面的职责、状态诚实性、页面级例外与未证明边界 |
| [design/pages/plugin-producer-popup.md](design/pages/plugin-producer-popup.md) | 权威当前 | `PAGE-PLUGIN-001`：Linggan 自有 Browser Producer popup 的身份、本机接纳准备度与受限手动 Discovery receipt |
| [design/changes/local-001a-evidence-library-ui-change-manifest.md](design/changes/local-001a-evidence-library-ui-change-manifest.md) | 权威当前；LOCAL-001A | Issue #25 的 UI 来源、范围、例外和证明边界 |
| [design/changes/local-001d-unknown-published-discovery-ui-change-manifest.md](design/changes/local-001d-unknown-published-discovery-ui-change-manifest.md) | 权威当前；LOCAL-001D / Issue #62 | 默认最新已接纳读取、`PUBLISHED_AT UNKNOWN` 与显式发布时间窗口的来源和范围 |
| [design/changes/design-003-lids-visual-baseline-ui-change-manifest.md](design/changes/design-003-lids-visual-baseline-ui-change-manifest.md) | 权威当前；DESIGN-003 | Issue #44 的视觉基线换向：token、规则、页面与数据诚实性逐条核对 |
| [design/changes/design-004-corpus-rail-ui-change-manifest.md](design/changes/design-004-corpus-rail-ui-change-manifest.md) | 权威当前；DESIGN-004 | Corpus rail 面层差异化与选中态重音：规则依据、参考图取舍与重音计数 |
| [design/changes/design-005-collection-workspace-ui-change-manifest.md](design/changes/design-005-collection-workspace-ui-change-manifest.md) | 权威当前；DESIGN-005 | Collection 五个子面落地：Mog 三项裁定、清掉的冗余、消化的断层与两项待决 |
| [design/changes/design-006-collection-ux-revision-ui-change-manifest.md](design/changes/design-006-collection-ux-revision-ui-change-manifest.md) | 权威当前；DESIGN-006 | Collection 五面 UX 修订：顺序与默认入口、命名去重、空态分级、读数异常优先、无数据时结构留数据格延后 |
| [design/changes/design-007-chinese-first-evidence-library-ui-change-manifest.md](design/changes/design-007-chinese-first-evidence-library-ui-change-manifest.md) | 权威当前；DESIGN-007 | Evidence Library 中文优先表达：中文独立承担用户含义，英文仅作技术旁注 |
| [design/changes/evidence-page-002-multi-material-ui-change-manifest.md](design/changes/evidence-page-002-multi-material-ui-change-manifest.md) | 权威当前；EVIDENCE-PAGE-002 | 多材料 Evidence Library 产品/静态参考的来源、表面、状态和明确非目标 |
| [design/changes/evidence-runtime-001-ui-change-manifest.md](design/changes/evidence-runtime-001-ui-change-manifest.md) | 权威当前；EVIDENCE-RUNTIME-001 / Issue #90 | Material Projection 运行页的唯一数据入口、作品 lane、Inspector、敏感材料与一次性验收边界 |
| [design/changes/design-008-shared-shell-chinese-first-ui-change-manifest.md](design/changes/design-008-shared-shell-chinese-first-ui-change-manifest.md) | 权威当前；DESIGN-008 / Issue #68 | 共享 local-web shell 的中文主语义、英文技术旁注与 Corpus/Collection 双页面验收边界 |
| [design/changes/design-009-runtime-capacity-surface-ui-change-manifest.md](design/changes/design-009-runtime-capacity-surface-ui-change-manifest.md) | 权威当前；DESIGN-009 | 执行工位改为产能判定面，运行状态由真实容量读取而非写死文案 |
| [design/changes/plugin-001-producer-popup-ui-change-manifest.md](design/changes/plugin-001-producer-popup-ui-change-manifest.md) | 已被替代；PLUGIN-001 | Issue #33 popup 的历史 UI 边界；不再约束当前实现 |
| [design/changes/plugin-migration-001-producer-popup-ui-change-manifest.md](design/changes/plugin-migration-001-producer-popup-ui-change-manifest.md) | 权威当前；PLUGIN-MIGRATION-001 | Issue #37 popup 的 Discovery 行动与状态边界；启动恢复由 Issue #53 文档补充 |
| [design/changes/plugin-popup-recovery-001-ui-change-manifest.md](design/changes/plugin-popup-recovery-001-ui-change-manifest.md) | 权威当前；PLUGIN-POPUP-RECOVERY-001 | Issue #53 popup 启动恢复、无副作用 fallback 与 v0.4.2 release 边界；Issue #55 的运行时修订见当月进度记录 |
| [design/acceptance/local-001a-evidence-library-visual-acceptance.md](design/acceptance/local-001a-evidence-library-visual-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-001`：本地页面的视觉、状态与真实后果分层验收记录 |
| [design/acceptance/local-001d-unknown-published-discovery-acceptance.md](design/acceptance/local-001d-unknown-published-discovery-acceptance.md) | 一次性报告 | `ACC-LOCAL-001D-001`：未知发布时间默认读取和显式窗口的分层验收记录 |
| [design/acceptance/design-003-lids-visual-baseline-visual-acceptance.md](design/acceptance/design-003-lids-visual-baseline-visual-acceptance.md) | 一次性报告 | Issue #44 的双实例同屏实测：计量、诚实性逐条核对与未证明范围 |
| [design/acceptance/design-004-corpus-rail-visual-acceptance.md](design/acceptance/design-004-corpus-rail-visual-acceptance.md) | 一次性报告 | `ACC-RAIL-004`：rail 三视口实拍验收；hover/focus 因全部路由禁用而未验证 |
| [design/acceptance/design-005-collection-workspace-visual-acceptance.md](design/acceptance/design-005-collection-workspace-visual-acceptance.md) | 一次性报告 | `ACC-COLLECTION-001`：五个子面与抽屉四态实拍；无数据的行与图表未验证 |
| [design/acceptance/design-008-shared-shell-chinese-first-acceptance.md](design/acceptance/design-008-shared-shell-chinese-first-acceptance.md) | 一次性报告 | `ACC-DESIGN-008`：共享壳层的中文主语义、技术注释层级与未证明边界 |
| [design/acceptance/plugin-001-producer-popup-visual-acceptance.md](design/acceptance/plugin-001-producer-popup-visual-acceptance.md) | 一次性报告 | `ACC-PLUGIN-001`：Issue #33 popup 的静态 source/release 验收；不证明浏览器加载或真实采集 |
| [design/acceptance/plugin-migration-001-producer-popup-visual-acceptance.md](design/acceptance/plugin-migration-001-producer-popup-visual-acceptance.md) | 一次性报告 | `ACC-PLUGIN-002`：Issue #37 adapter/mock ingress/release 验收；不证明真实浏览器或采集 |
| [design/acceptance/plugin-popup-recovery-001-visual-acceptance.md](design/acceptance/plugin-popup-recovery-001-visual-acceptance.md) | 一次性报告 | `ACC-PLUGIN-POPUP-RECOVERY-001`：Issue #53 source/release 启动保护验收；不证明 Chrome 已加载或真实采集 |
| [design/acceptance/design-007-chinese-first-evidence-library-acceptance.md](design/acceptance/design-007-chinese-first-evidence-library-acceptance.md) | 一次性报告 | `ACC-DESIGN-007`：Evidence Library 中文优先表达的分层验收记录 |
| [design/acceptance/evidence-page-002-multi-material-reference-acceptance.md](design/acceptance/evidence-page-002-multi-material-reference-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-PAGE-002`：多材料 Evidence Library 静态参考的双视口、状态、互动和边界验收 |
| [design/acceptance/evidence-runtime-001-visual-acceptance.md](design/acceptance/evidence-runtime-001-visual-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-RUNTIME-001`：Issue #90 运行页的自动检查、桌面/375px 浏览器证据与未证明边界 |
| [design/acceptance/work-resource-read-001-visual-acceptance.md](design/acceptance/work-resource-read-001-visual-acceptance.md) | 一次性报告 | `ACC-WORK-RESOURCE-READ-001`：共享作品资源、目标/作者关系、时间资格和三种排版的分层验收；真实视觉与部署仍未核验 |
| [design/acceptance/dev-05-xhs-content-observation-001-visual-acceptance.md](design/acceptance/dev-05-xhs-content-observation-001-visual-acceptance.md) | 一次性报告 | `ACC-DEV-05-XHS-CONTENT-OBSERVATION-001`：Issue #133 的当前/历史字段、Coverage 历史与有界复观测分层验收；未重载运行时、未进行真实 XHS 或 Mog 验收 |
| [design/acceptance/evidence-v9-001-visual-acceptance.md](design/acceptance/evidence-v9-001-visual-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-V9-001`：V9 研读密度、材料完整度口径、Inspector 宽度与 LOCAL MEDIA/Lightbox 的真实数据走查；多图链路只有一个样本，未部署 |
| [design/acceptance/design-009-runtime-capacity-surface-acceptance.md](design/acceptance/design-009-runtime-capacity-surface-acceptance.md) | 一次性报告 | `ACC-RUNTIME-001`：产能判定、状态诚实性与视觉的验收记录 |
| [design/foundation/topic-intelligence-visual-language.md](design/foundation/topic-intelligence-visual-language.md) | 权威当前 | DESIGN-002 的 DS-001–DS-007：合成 Topic Reference Page 的受限视觉语言 |
| [design/patterns/evidence-candidate-and-boundary-patterns.md](design/patterns/evidence-candidate-and-boundary-patterns.md) | 权威当前 | DESIGN-002 的 PAT-001–PAT-004：观察、候选、边界与无副作用意图表达 |
| [design/components/component-promotion.md](design/components/component-promotion.md) | 权威当前 | 从 Reference Page 局部块到真实 CMP 的晋升条件；当前没有已晋升组件 |
| [design/pages/topic-intelligence-reference-page.md](design/pages/topic-intelligence-reference-page.md) | 权威当前 | PAGE-TOPIC-001 的组合规格、状态、互动和验收边界 |
| [design/pages/topic-intelligence-reference.html](design/pages/topic-intelligence-reference.html) | 权威当前 | 可本地打开的合成静态 Topic Reference Page；不是运行 Web 产品 |
| [design/pages/topic-intelligence-reference-acceptance.md](design/pages/topic-intelligence-reference-acceptance.md) | 权威当前 | ACC-TOPIC-001：静态参考实现的视觉、互动、自动检查和未证明边界 |
| [`architecture/technical-architecture-baseline.md`](architecture/technical-architecture-baseline.md) | 权威当前 | 全产品技术架构统一入口：权威层级、确定性、当前/目标边界、模块/入口/数据/运行主干与 SCOPE 符合性 |
| [`architecture/target-architecture.md`](architecture/target-architecture.md) | 草案 | DISC-001 已确认硬边界之上的总体架构建议；具体实现按 SCOPE 渐进冻结 |
| [`architecture/system-overview-diagram.html`](architecture/system-overview-diagram.html) | 草案 | AEDS 风格的整体架构图；在同一视图中区分当前代码骨架、已批准 Evidence 切片与后续目标能力 |
| [`architecture/business-process-diagram.html`](architecture/business-process-diagram.html) | 草案 | AEDS 风格的内容情报业务流程图；展示人、控制层、Capture、Evidence 与 Intelligence 的责任交接 |
| [`architecture/project-architecture-atlas.html`](architecture/project-architecture-atlas.html) | 代码事实优先；本机快照 2026-08-28 | 面向非技术项目负责人的 AEDS 中文架构全景页；以当前本机代码、PostgreSQL、loopback API 与页面读取核对为主，分层展示独立本地产品、受控浏览器采集、模块化单体、数据/事实边界、界面、运行环境、AI Agent 与路线图；不替代运行、长期采集或生产证明 |
| [`architecture/module-architecture.md`](architecture/module-architecture.md) | 草案 | Gate 6 Rust 模块、接口、依赖、adapter、测试表面与无巨型文件门禁；不授权创建 crate |
| [`architecture/agent-architecture.md`](architecture/agent-architecture.md) | 草案 | Gate 6 Agent Invocation、工具权限、预算/停止、恢复、结构化输出、隐私与评测；不选择模型或授权真实原文 |
| [`architecture/capture-plugin-architecture.md`](architecture/capture-plugin-architecture.md) | 代码事实优先 | Linggan 采集控制层与受控 Browser Producer 的当前责任边界；`0.8.28` 单工位标准详情真实链已通过，未实现的长期候选仍以草案待决策 |
| [`architecture/capture-control-contract.md`](architecture/capture-control-contract.md) | 草案 | ARC-001 首批真实 Canary 前的采集控制合同：Need/授权/准入、有限资源、lane、Coverage、部分结果、回执与恢复边界；不授权真实执行 |
| [`architecture/media-lifecycle-contract.md`](architecture/media-lifecycle-contract.md) | 权威当前 | 统一 Slot/Origin/Download/Blob/Materialization/Derivative/Relation 合同；`0.8.28` 真实封面、正文图、Live Photo 与作者头像链已验收，ASR 失败与 `comment.image NOT_OBSERVED` 继续如实保留 |
| [`architecture/runtime-operations-architecture.md`](architecture/runtime-operations-architecture.md) | 代码事实优先 | 当前 loopback API / 巡检 worker / 媒体 worker / PostgreSQL / scheduler 运行拓扑，以及未实现的长期运维候选边界 |
| [`architecture/data-architecture.md`](architecture/data-architecture.md) | 草案 | Gate 5 数据分类、身份、版本、Current、隐私、统计资格与 PostgreSQL 概念模型；不是 DDL |
| [`architecture/data-relations.md`](architecture/data-relations.md) | 草案 | `data-architecture.md` 的渐进披露子文档；收敛候选基数、外键责任、类型化关系与并发约束，不是最终表清单 |
| [`architecture/data-consistency.md`](architecture/data-consistency.md) | 草案 | 数据架构第三层；定义事务、重放、并发、隐私传播和 PostgreSQL 16 可证伪验收，不是 SQL 或 migration |
| [`architecture/current-v2-architecture.md`](architecture/current-v2-architecture.md) | 代码事实优先 | 现役 V2 固定点的参考架构 |
| [`architecture/rust-porting-map.md`](architecture/rust-porting-map.md) | 草案 | Rust 移植候选参考；只有正式 SCOPE 内明确列出的部分可以实施 |
| [`pages/page-map.md`](pages/page-map.md) | 草案 | 页面与情报工作流的初步映射 |
| [`pages/product-interface-architecture.md`](pages/product-interface-architecture.md) | 草案 | Gate 7 工作台、Topic/Corpus/研究/行动/运行中心、API 与外部 Agent CLI 入口架构 |
| [`pages/home-intelligence-surface.md`](pages/home-intelligence-surface.md) | 草案 | `HOME-01`–`HOME-17` 确认的首页情报面形态：三栏骨架、恒定地形与昨夜行动、常驻编队与总编、选题卡回流、校准回路；不是实现授权 |
| [`pages/topic-intelligence-surface.md`](pages/topic-intelligence-surface.md) | 权威当前 | DESIGN-002 的 Topic 深入任务、信息/状态/行动边界；当前只授权合成静态参考页 |
| [`migration/action-plan.md`](migration/action-plan.md) | 活跃计划 | 基础设计阶段形成的重建顺序与架构工作包参考；实时阶段状态、完成证据和决策门以 `development-stage-tracker.md` 为准 |
| [`migration/transfer-checklist.md`](migration/transfer-checklist.md) | 活跃计划 | 跨机器搬迁和隔离检查 |
| [`reviews/bootstrap-review.md`](reviews/bootstrap-review.md) | 一次性报告 | Bootstrap 固定点审查，不代表当前实现状态 |
| [`audits/gate-3-domain-model-audit-2026-08-20.md`](audits/gate-3-domain-model-audit-2026-08-20.md) | 一次性报告 | 已获用户整体确认的 Gate 3 设计快照与全链压力测试；不替代权威共同语言，也不证明 Gate 4–7 已通过 |
| [`audits/gate-5-data-architecture-audit-2026-08-20.md`](audits/gate-5-data-architecture-audit-2026-08-20.md) | 一次性报告 | Gate 5 平台无关概念数据模型的第一轮对抗审查、已修漂移、条件通过项和待确认决定；不等于数据库实现获批 |
| [`audits/gate-4-to-7-cross-consistency-audit-2026-08-20.md`](audits/gate-4-to-7-cross-consistency-audit-2026-08-20.md) | 一次性报告 | Gate 4–7 在统一决定与 SCOPE-001 前的采集、数据、架构、页面/CLI 跨文档审计 |
| [`audits/gate-7-synthetic-task-walkthrough-2026-08-20.md`](audits/gate-7-synthetic-task-walkthrough-2026-08-20.md) | 一次性报告 | 用完全合成的“任务启动困难”负面场景走查今日关注、Topic、50/100 运行状态与外部 Agent CLI |
| [`audits/pre-scope-001-readiness-2026-08-20.md`](audits/pre-scope-001-readiness-2026-08-20.md) | 一次性报告 | 不启动 SCOPE/代码的前提下预演首个 Content Evidence 切片的 fixture、数据库副作用、模块/文件预算和完成标准 |
| [`audits/scope-001-final-adversarial-audit-2026-08-20.md`](audits/scope-001-final-adversarial-audit-2026-08-20.md) | 一次性报告 | 正式 SCOPE-001 的独立代码前审查；P0/P1/P2、修订结果和最终实施确认边界 |
| [`audits/implementation-readiness-review-2026-08-20.md`](audits/implementation-readiness-review-2026-08-20.md) | 一次性报告 | 全项目正式编码前实施就绪终审；NO-GO 证据、P0/P1、15 个场景、真实最小闭环及主线 Agent 复核协议 |
| [`audits/pi-agent-kernel-upstream-assessment-2026-08-20.md`](audits/pi-agent-kernel-upstream-assessment-2026-08-20.md) | 一次性报告 | Pi 上游发布版/源码/安全边界审查；裁定为统一 Agent 执行适配器候选，不是权限、持久任务或业务事实内核，也不授权当前编码 |
| [`audits/pre-implementation-architecture-audit-2026-08-21.md`](audits/pre-implementation-architecture-audit-2026-08-21.md) | 一次性报告 | `ADV-AUDIT-001` 的独立复核最终处置：保留权威冲突、真实 producer 接缝、产品职责和 dirty worktree 风险，否定当前解除 Attempt 1:1、强加 `attemptRef`、改 crate 或整批提交的危险建议；不改变 F01 代码门 |
| [`runbooks/development-environment.md`](runbooks/development-environment.md) | 权威当前 | Rust 与 Docker PostgreSQL 16 的统一运行入口 |
| [`runbooks/local-evidence-library.md`](runbooks/local-evidence-library.md) | 权威当前 | Rust loopback host、health、受控 discovery ingress 与 Evidence Library 本地启动/验证；明确不含真实插件、平台或媒体链路 |
| [`runbooks/linggan-browser-producer-local.md`](runbooks/linggan-browser-producer-local.md) | 权威当前 | Browser Producer `0.8.28` 构建/发行/加载/工位/真实回执的分层核对步骤；操作授权仍以当次任务为准 |
| [`platforms/xiaohongshu/capture-capability-registry.md`](platforms/xiaohongshu/capture-capability-registry.md) | 权威当前；AUD-XHS-001 范围内 | 小红书页面字段、探针、插件交付能力、Coverage 与平台漂移的唯一登记入口；不保存真实原文或接入真实材料 |
| [`decisions/0002-comment-collection-completion-and-retry.md`](decisions/0002-comment-collection-completion-and-retry.md) | ACCEPTED | 小红书评论的标准 30 条窗口、全量深采完成判据、部分材料可用性、重新从详情页采集及当前去重投影规则 |
| [`proposals/real-canary-002-controlled-discovery-through-authorization-chain.md`](proposals/real-canary-002-controlled-discovery-through-authorization-chain.md) | 草案 | 让一次已获准过的 ADHD 发现面原样再走一遍，但全程经过「授权 → 准入 → 工单 → 租约 → 闸门 → 派发」，用于解锁采集控制合同 §12 第 5 条；不扩大平台足迹，不授权 001C-2/001C-3 |
| [`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) | 活跃计划 | F01 后、首个真实 producer 和用户可见切片前的产品形态、P0 页面、Canary、Capture/Media 与第一阶段运行架构决策图；当前首票为 `product-shell` |
| [`plans/active/design-002-topic-intelligence-reference-page.md`](plans/active/design-002-topic-intelligence-reference-page.md) | 活跃计划 | 合成 Topic Intelligence Reference Page 的范围、规则、原型与验证计划；不替代完整 P0 或业务 SCOPE |
| [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md) | 活跃计划 | 当前唯一获准实施事项：F01 synthetic fact-kernel technical tracer 受控进入 TDD；完成 loopback API + minimal CLI 后 hard stop，不扩入 F02–F10、真实平台/AI/插件/Web |
| [`plans/active/plugin-runtime-001-full-capability-retrofit.md`](plans/active/plugin-runtime-001-full-capability-retrofit.md) | 活跃计划；Issue #50 | 将灵感爆爆爆的成熟浏览器采集能力一次性适配为 Linggan 自有 Browser Producer Runtime；媒体本地资产与异步处理血缘为本事项第一类约束 |
| [`plans/active/aud-xhs-001-xiaohongshu-capture-probe-refresh.md`](plans/active/aud-xhs-001-xiaohongshu-capture-probe-refresh.md) | 活跃计划；Issue #74 | 在最小真实页面样本上刷新小红书搜索、详情（含最多 30 条评论）和作者页探针，形成不含真实原文的字段与能力登记；不接入或持久化真实材料 |
| [`plans/active/plugin-xhs-capture-upgrade-001.md`](plans/active/plugin-xhs-capture-upgrade-001.md) | 活跃计划；Issue #76 | 实现小红书搜索页面事实、详情 DOM 回退与“详情/媒体观察/最多 30 条评论”统一逻辑回执；不分析、不接入真实材料、不发布 |
| [`plans/active/plugin-xhs-active-collection-001.md`](plans/active/plugin-xhs-active-collection-001.md) | 活跃计划；Issue #78 | 将小红书搜索、标准详情、单篇评论深采与批量评论升级为目标驱动、可恢复的页面执行；不进行真实材料交付、真实页面回归或发布部署 |
| [`plans/active/plugin-xhs-finalization-001.md`](plans/active/plugin-xhs-finalization-001.md) | 活跃计划；Issue #128 | 一次性收口 Browser Producer 派发/回执/评论 Attempt 可靠性与统一 MediaResource；保留人工媒体窗口，真实链只对当前授权样本运行一次 |
| [`plans/completed/plugin-xhs-adaptive-scroll-and-detail-receipt-001.md`](plans/completed/plugin-xhs-adaptive-scroll-and-detail-receipt-001.md) | 已完成；Issue #80 / PR #81 | 将 XHS 页面加载滚动收敛为可解释的有限过程；分离 Intelligence 详情交付与人工媒体下载，并逐 lane 显示接纳回执 |
| [`plans/active/local-001-local-product-evidence-library.md`](plans/active/local-001-local-product-evidence-library.md) | 活跃计划 | 本地独立 Linggan 产品的第一个页面与最小真实采集接缝：V7 Evidence Library Gold Master、`http://localhost:3000`、只读投影，以及 discovery → media acquisition → 异步 OCR/ASR 的连续 Canary 路线；不扩张 SCOPE-001 |
| [`plans/active/local-001d-unknown-published-discovery-view.md`](plans/active/local-001d-unknown-published-discovery-view.md) | 活跃计划；Issue #62 | 修复默认 Evidence Library 把已接纳、来源发布时间未知的 discovery 卡片隐藏的问题；显式 7/30 天发布时间窗口保持严格 |
| [`plans/active/local-runtime-001-persistent-loopback.md`](plans/active/local-runtime-001-persistent-loopback.md) | 活跃计划 | Issue #38 的持久本地 PostgreSQL migration、loopback readiness 与重启保留证明；不含插件或真实平台 |
| [`plans/active/plugin-migration-001-linggan-owned-xhs-discovery.md`](plans/active/plugin-migration-001-linggan-owned-xhs-discovery.md) | 活跃计划 | Issue #37：将成熟旧插件的可见 XHS 搜索卡发现规则适配为 Linggan 自有包，且只启用一次手动 `ADHD` / “综合” / 前 20 卡 Discovery → 固定 localhost ingress |
| [`plans/active/plugin-rehome-001-full-legacy-browser.md`](plans/active/plugin-rehome-001-full-legacy-browser.md) | 合并后文档收口待办 | Issue #41 / PR #42：完整迁入 `linggan-boom v2.0.91` 的浏览器 source/UX 作为 Linggan 自有包，并切断旧内容工作台运行时；不证明真实采集 |
| [`plans/completed/gov-004-delivery-package-harness-autonomy.md`](plans/completed/gov-004-delivery-package-harness-autonomy.md) | 已完成；已被 GOV-005 替代 | Issue #70：确立交付包、表面/状态/依赖/验收矩阵；其中 Harness 自主编排已由 GOV-005 取代 |
| [`plans/active/gov-005-human-directed-collaboration.md`](plans/active/gov-005-human-directed-collaboration.md) | 活跃计划；Issue #72 | 将派单、并发、审查和合并的默认控制权交由 Mog；Agent 仅执行明确分配的 Work Package |
| [`plans/completed/plugin-retrofit-local-trusted-001.md`](plans/completed/plugin-retrofit-local-trusted-001.md) | 已完成计划 | Issue #43 / PR #46：已合并的 LOCAL_TRUSTED synthetic manual `TaskSpec → Attempt → durable outbox → loopback receipt`；不证明真实浏览器、平台、媒体、scheduler 或 Evidence Library 真实数据 |
| [`plans/completed/plugin-001-linggan-owned-producer.md`](plans/completed/plugin-001-linggan-owned-producer.md) | 已完成计划 | `PLUGIN-001`：Linggan 自有 MV3 基础包、loopback 探测与静态可安装 release；后续 Discovery 由 PLUGIN-MIGRATION-001 承接 |
| [`plans/completed/disc-001-project-foundation-design.md`](plans/completed/disc-001-project-foundation-design.md) | 已完成计划 | 七道项目基础设计关口与 `USER-DEC-01`–`06` 的完成记录 |
| [`plans/completed/env-001-development-environment.md`](plans/completed/env-001-development-environment.md) | 已完成计划 | ENV-001 的范围、执行和验收记录 |
| [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md) | 已完成计划 | GOV-001 的范围、交付和验收记录 |
| [`plans/completed/gov-002-agent-skills-configuration.md`](plans/completed/gov-002-agent-skills-configuration.md) | 已完成计划 | Matt Pocock 工程技能、GitHub Issues、triage 与领域文档适配的配置记录 |
| [`progress/README.md`](progress/README.md) | 权威当前 | 变更记录规则和月份索引 |
| [`progress/2026-09.md`](progress/2026-09.md) | 权威当前 | 2026-09 的重要变更记录 |
| [`progress/2026-08.md`](progress/2026-08.md) | 权威当前 | 2026-08 的重要变更记录 |
| [`progress/2026-09.md`](progress/2026-09.md) | 权威当前 | 2026-09 的重要变更记录 |

## 仓库外层资料入口

| 文档 | 状态 | 用途 |
|---|---|---|
| [`database/README.md`](../database/README.md) | 权威当前 | 新数据库交付与安全边界 |
| [`database/legacy-archive-manifest.md`](../database/legacy-archive-manifest.md) | 历史归档 | 旧数据库和媒体的只读归档清单 |
| [`references/README.md`](../references/README.md) | 历史归档 | 历史证据区阅读边界 |
| [`references/current-v2/SOURCE-PROVENANCE.md`](../references/current-v2/SOURCE-PROVENANCE.md) | 历史归档 | 固定源码快照来源证明 |

## 按任务渐进读取

- 项目管理或新增文件：先读 `governance/`，再读当前事项文档。
- 使用 Matt Pocock 工程技能：先读 `agents/` 中与任务追踪、triage 或领域文档相关的配置，再进入当前 Issue 和直接关联文档。
- UI 设计、前端实现或 UI 审查：先读 [agents/ui-execution-contract.md](agents/ui-execution-contract.md) 和 [design/README.md](design/README.md)，再进入对应产品页面、已批准设计规则、数据合同与当前 SCOPE；产品草案、截图、旧系统和参考材料不能单独授权实现。
- DESIGN-002 Topic Reference Page：依次读 [`pages/topic-intelligence-surface.md`](pages/topic-intelligence-surface.md)、[`design/foundation/topic-intelligence-visual-language.md`](design/foundation/topic-intelligence-visual-language.md)、[`design/patterns/evidence-candidate-and-boundary-patterns.md`](design/patterns/evidence-candidate-and-boundary-patterns.md)、[`design/components/component-promotion.md`](design/components/component-promotion.md) 与 [`design/pages/topic-intelligence-reference-page.md`](design/pages/topic-intelligence-reference-page.md)；静态 HTML 只用于视觉走查，不能当成真实 API/UI 已实现的证据。
- 实施或审查 SCOPE-001：读取当前 SCOPE 后必须读取 `agents/scope-001-execution-contract.md`；当前仅按 CONTROLLED OPEN FOR TDD 推进，新增产品含义、权限或真实范围仍须停止并确认。
- 环境配置：读 `current-state.md`、[`runbooks/development-environment.md`](runbooks/development-environment.md) 和 `database/README.md`。
- 产品讨论：读 `product/PRD.md`、`context/discussion-decisions.md` 和相关 ACCEPTED ADR。
- 首页与日常入口形态：先读 [`pages/page-map.md`](pages/page-map.md)，再读 [`pages/product-interface-architecture.md`](pages/product-interface-architecture.md)，最后读 [`pages/home-intelligence-surface.md`](pages/home-intelligence-surface.md)；三者均为草案，不得据此直接实现前端。
- 数据与 PostgreSQL 设计：先读 `architecture/data-architecture.md`；只有需要关系/基数时再读 `architecture/data-relations.md`，需要事务/并发/验收时再读 `architecture/data-consistency.md`，并同时遵守 `database/README.md` 和领域不变量；未确认草案不得直接生成 DDL。
- 架构或实现：先读 [`architecture/technical-architecture-baseline.md`](architecture/technical-architecture-baseline.md)，再按任务读相应专题架构、当前 SCOPE、真实代码与测试；文档不得替代代码事实。
- 来源审计：先读 `references/README.md` 和 provenance，再只打开被当前审计明确引用的 fixture、源码或 handoff。

## 入库完成标准

新文件只有同时满足“位置正确、名称稳定、状态明确、进入本索引、冲突已处理、变更已登记、检查通过”才算正式入库。
