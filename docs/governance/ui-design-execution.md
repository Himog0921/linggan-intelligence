# LIDS-ALIGN-001 · UI Agent 渐进执行标准

> 状态: 权威当前
> 最后核对: 2026-10-19
> 适用范围: 项目内全部 UI 设计、实现、审查、规范维护和设计验收；不适用于全局 Agent 设置
> 事实来源: Mog 本次 LIDS-ALIGN-001 交付授权、已确认对话决定、现行 LIDS 与 main@9ff50b81；逐项来源见 review-alignment.json
> 冲突时以谁为准: 用户最新明确确认、产品/数据/权限合同与当前 SCOPE；已采纳设计按本文及明确替代关系执行，候选不自动生效


## 入口与权威

项目入口为根 AGENTS.md、CLAUDE.md、codx.md。Codex 通常自动读取 AGENTS.md；codx.md 是 Mog 指定的项目导航文件，不能假定工具会自动加载它。三个入口指向本协议，不复制视觉数值。字体、字号、行高、字重统一按已采纳B执行，角色数值只维护于LIDS的typography.json；Q01/Q02不再重复审阅。LIDS 继续是唯一设计表达标准；本协议只规定 Agent 怎么查找、选择、执行与交付。

规则正文在 [LIDS 入口](../design/lids/README.md)及各分册。已确认决定、既有继续有效规则、未确认候选分别记录；[逐项对齐表](../design/lids/review-alignment.md)是追踪记录，不是第二套规范。遇到历史规则与已确认新规则冲突，按明确替代者执行；未决项只阻断依赖它的改动，独立部分继续。

## L0 · 每次 UI 任务先读

1. 确认用户结果、当前授权及 checkout/HEAD；核对 AGENTS.md、docs/README.md、docs/current-state.md 与[既有 UI 执行合同](../agents/ui-execution-contract.md)。
2. 读本协议与 LIDS 入口；声明受影响的页面/模式、核心对象、区域及共享依赖。
3. 按下一表只展开本任务命中的模块。历史图、HTML 和审阅页用于举证，不能覆盖规则正文或直接授予业务实现。

## L1 · 按触发条件展开模块

| 触发条件 | 必须展开的唯一规则入口 | Agent 必须给出的结果 |
| --- | --- | --- |
| 视觉气质、留白、层级、边界 | [system](../design/lids/system.md) | 当前强度、结构与重音的用途；不靠填充内容制造完整感 |
| 字体、字号、字重、数字、原声 | [typography](../design/lids/typography.md) | 已选字体与B角色；只读typography.json，Q03原声OPPO已确认；Q11已采用自动swap/F2/普通右对齐/小数点对齐；业务接入验证另计 |
| 色值、尺寸、间距或运行时 Token | [tokens](../design/lids/tokens.md) | 现有语义 Token 映射；新增值先走规则审阅，不私造页面值 |
| 页头、面包屑、领域、侧栏 | [shell-zones](../design/lids/shell-zones.md) | 壳层所有者、位置/范围分责；按ALIGN-SHELL-007读取shell-layout.json的A与图标折叠；领域及三级页面Tab归Z02模块 |
| 页面组合、区域或动作位置 | [regions](../design/lids/regions.md) → [patterns](../design/lids/patterns.md) | 页任务、最小区域、允许/禁止内容；列出集合/对象/表单动作归属；按ALIGN-COMPOSITION-009的composition-layout.json逐模式执行含搜索的列表B/其余功能C，保留用户问题卡片 |
| 按钮、输入、下拉、FIND、焦点 | [primitives](../design/lids/primitives.md) | 查重、变体、状态、键盘与可读反馈；Q04按ALIGN-FOCUS-005采用C，参数从focus.json读取；按钮材质继续展开materials |
| 纹理、硬阴影、选中材质 | [materials](../design/lids/materials.md) | 按ALIGN-MATERIAL-006读取control-material.json的M2参数；保留文字避让与Q12局部例外 |
| 业务状态、计数、缺失、证据 | [data-boundaries](../design/lids/data-boundaries.md) | 四态、未知与已知零、来源/判断、执行/成果分责 |
| 任务表行高、列宽、字段或窄屏 | [regions](../design/lids/regions.md) → [review-items](../design/lids/review-items.md) | 一任务一行；Q07按ALIGN-LEDGER-008读取regions与ledger-layout.json的L1/44px/名称144px，保留字段自定义；Q08组合已采用B搜索列表/C功能，不缩字号、不自动减列或拆行 |
| 搜索、浮层、异步、停止或恢复 | [interaction-contracts](../design/lids/interaction-contracts.md) | 按ALIGN-OVERLAY-010/overlay-policy.json执行S1；声明触发、未保存/原请求跟踪、停止不可恢复及退出返回 |
| 新增或推广共享组件 | [component-promotion](../design/components/component-promotion.md) | 按ALIGN-REUSE-011查四类当前目录、既有变体和复用边界；查重及第二独立页面证据，候选C/B不冒用正式CMP |
| 文案、英文、技术标识 | [language-policy](../design/lids/language-policy.md) | 中文独立承载含义；FIND 是装饰识别，字体选择不扩大英文预算 |
| 冲突、例外、未确认项 | [review-items](../design/lids/review-items.md) → [decisions](../design/lids/decisions.md) | 条目 ID、影响、当前可继续部分及最小待决内容 |

## L2 · 具体页面与组件

通过 [设计索引](../design/README.md)进入对应 PAGE/产品/数据合同；按需在[对齐表](../design/lids/review-alignment.md)查 P/PT/Z/C/B/I/R/A。它们保留审阅标识，未自动晋升正式 CMP/PAT/PAGE。只加载当前模块对应条目及其直接依赖，不通读全部候选目录。先用 `python3 scripts/verify-lids-alignment.py --list components` 看当前分类，再以 `--item C16` 或 `--item P09`只读查询单项；输出中的candidateRequirement始终是候选，正式约束从normativeTarget读取。

17 类页面与 26 模式属于盘点覆盖；11 页型、48 组件、41 业务构件、32 交互和 14 动作预设的具体合同仍按逐项状态读取，不能由数量推导已批准或已实现。新业务字段、权限与命令继续由既有合同决定。

## L3 · 证据与交付

先给出一份有界声明：用户结果；页面/模式及区域组合；采用的正式规则与替代项；复用组件及业务适配；状态/交互矩阵；待决 ID；文件范围；验证与未验证边界。沿用 [UI Change Manifest](../design/templates/ui-change-manifest-form.md)，不另造模板。

按风险核对常态、空、加载、错误、部分、受限、未确认；长文本、键盘、窄屏、缩放和返回上下文。数据只读、写命令、真实回执和用户视觉验收分别记证据。没有产品依据的数字与动作不补造。

LIDS-ALIGN-001 是前期规范包的授权边界，其 runtimeMigrated=false 描述该包没有改业务。Mog 2026-10-20 已另授权 [LIDS-UI-001](../plans/active/lids-ui-implementation-001.md)在独立候选接入真实 React、原始字体与唯一运行Token；本机3000/主线与部署仍独立。具体实现覆盖与未验证项读该包验收，不能把前期历史状态误读成当前实现状态。

## 检查与完成口径

- `python3 scripts/verify-lids-alignment.py --check --self-test`：检查对齐目录、来源、状态、渐进路由和反例；只证明文档结构与声明。
- `./scripts/verify-ui-design-handbook.sh`：沿用正式 LIDS 检查，保持运行时 Token 镜像一致。
- `./scripts/check-project-governance.sh origin/main`：检查项目索引、状态、链接、留痕和本协议接入。
- 视觉与业务变化另执行当次任务的应用/浏览器验证；文档检查通过不代表样式、组件、运行、用户验收或主线合并。

本协议在包含它的项目 checkout 中可执行；独立分支入库不等于主线、其他 worktree 或常驻 runtime 已同步。交付必须报告文件落地、检查、提交/推送/合并、运行与 Mog 验收各层状态。

## ALIGN-MEDIA-013与整体收口入口

作品卡、媒体与工位材质任务先读[materials](../design/lids/materials.md)，仅按media-material.json采用E1/白场；其余八个规则源及区域路由保持。Q01–Q12均已确认，不将历史候选当当前可选标准。未裁定差异只登记[review-items](../design/lids/review-items.md)，核查证据见[LIDS-CLOSEOUT-18](../audits/lids-closeout-audit-2026-10-20.md)。完成声明分标准/镜像/原型/CMP/业务迁移/主线/运行七层，禁止以构建或文件检查替代后四层。

## 正式代码采用的检查入口

颜色按唯一 [color.json](../design/lids/color.json) 执行；字体、C、M2、A 和 L1 由 `python3 scripts/sync-lids-runtime.py` 映射到唯一 lids_tokens.css，`--check` 在应用测试中检查漂移。改变运行值时先修改已批准源再再生。既有语义别名只指向这一源，不得在页面复制数字。实现/发布进度只读 [LIDS-UI-001 验收](../design/acceptance/lids-ui-implementation-001-acceptance.md)，前期九源的 runtimeMigrated=false 是规范包快照，不是第二设计标准。
