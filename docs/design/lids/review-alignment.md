# LIDS-ALIGN-001 · 审阅稿逐项对齐表

> 状态: 权威当前
> 最后核对: 2026-10-20
> 适用范围: 设计采用追踪与候选导航；不是另一个设计规则源
> 事实来源: review-alignment.json及其中记录的用户确认、审阅源SHA和代码基线
> 冲突时以谁为准: 用户最新确认及引用的正式LIDS分册；候选要求不自动生效

由 `python3 scripts/verify-lids-alignment.py --render` 从 [清单源](review-alignment.json)生成；表格不手改。

先查当前任务的ID，再读对应规则模块；只在需要审阅候选细节时打开JSON中的candidateRequirement。

## 采用口径

- 已确认原则：按正式落点执行；仅确认列出的范围。
- 沿用既有规则：语义继续有效，具体适用依据仍需PAGE/数据合同。
- 原则采用／细节待审：已确认部分见正式分册，JSON候选参数不作为默认。
- 候选待审：只用于讨论与盘点，不代表批准、组件交付或业务能力。

字体角色与完整B字号、字重、行高已确认，Q01/Q02已关闭，Q03原声已统一OPPO Sans；唯一参数源为[typography.json](typography.json)。Q04已按ALIGN-FOCUS-005采用C；Q05已按ALIGN-MATERIAL-006采用M2，唯一参数源[control-material.json](control-material.json)；Q06已按ALIGN-SHELL-007采用A及手动图标折叠；Q07已按ALIGN-LEDGER-008采用L1/44px/名称144px；Q08按ALIGN-COMPOSITION-009采用含搜索的列表B/其他功能C并保留用户问题卡片；Q09已采用S1，Q10四类/七门槛已采用，Q11已采用自动/F2/数值对齐；Q12按ALIGN-MEDIA-013采用E1与白场舞台。
业务代码、字体资产和运行时Token未改；分支文档入库不等于主线或常驻runtime同步。

原审阅注册表 SHA256：`88747e1f374318130056ac051edb5ae35d9a7b0c50302b027da27045acdd0bf5`。
本次代码基线：`9ff50b8135c0617672a426a97d58ec7448a7cbe7`。完整确认摘要见清单userEvidence。

## 已确认的跨模块规则

| 规则 | 正式落点 | 未确认部分 |
| --- | --- | --- |
| 无边框V2，连续白场、黑结构、橙重音；v8仍为粗稿 | [system](system.md) | Q05控件材质已采用M2；E1/白场舞台已采用 |
| OPPO / Albert / Alumni / Manrope及B字级整体采用 | [typography](typography.md) | 原声OPPO已确认／生产接入Q11 |
| 领域保留面包屑位置，位置与范围分责 | [shell-zones](shell-zones.md) | A容量及图标折叠已采用；组件认证Q10 |
| 三级页面Tab统一归Z02面包屑模块，主区不重复 | [shell-zones](shell-zones.md) | A容量／收纳已采用；对象内部Tab与筛选各自就近 |
| 区域可定位、标题去重、列表紧凑计数 | [regions](regions.md) | Q08组合及动作位置已采用；组件认证Q10 |
| A连续账本，一任务一行 | [regions](regions.md) | 44px/名称144px已采用；B外部组合已采用；组件认证Q10 |
| FIND保留装饰结构，输入/FIND聚焦不加外圈 | [primitives](primitives.md) | C焦点、M2材质已确认；组件认证Q10 |
| 按钮、领域、面包屑、侧栏沿真实页面纹理方向 | [materials](materials.md) | M2参数已确认；E1局部例外/白场舞台已采用 |
| Agent入口渐进展开，不自动推广候选或扩大授权 | [治理协议](../../governance/ui-design-execution.md) | 业务接入与主线集成独立 |

## 规则 · 24项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| R01 | 任务先于布局 | 已确认原则 | [regions](../../design/lids/regions.md) | U03, U08 | — |
| R02 | 区域按职责出现 | 已确认原则 | [regions](../../design/lids/regions.md) | U03, U08 | — |
| R03 | 页面身份去重 | 已确认原则 | [regions](../../design/lids/regions.md) | U03, U08 | — |
| R04 | 读数服从页型 | 已确认原则 | [regions](../../design/lids/regions.md) | U03, U08 | — |
| R05 | 位置与范围分责 | 已确认原则 | [shell-zones](../../design/lids/shell-zones.md) | U05, U08, U10 | — |
| R06 | 动作按对象归位 | 已确认原则 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20 | — |
| R07 | 有限的主操作 | 沿用既有规则 | [primitives](../../design/lids/primitives.md) | existing_lids, U08 | — |
| R08 | FIND保留结构 | 已确认原则 | [primitives](../../design/lids/primitives.md) | U03, U05, U08, U12, U13 | — |
| R09 | 事实状态分别表达 | 沿用既有规则 | [data-boundaries](../../design/lids/data-boundaries.md) | existing_lids, U08 | — |
| R10 | 数值不补事实 | 沿用既有规则 | [data-boundaries](../../design/lids/data-boundaries.md) | existing_lids, U08 | — |
| R11 | 原文与判断分开 | 沿用既有规则 | [data-boundaries](../../design/lids/data-boundaries.md) | existing_lids, U08 | — |
| R12 | 反馈靠近影响 | 沿用既有规则 | [interaction-contracts](../../design/lids/interaction-contracts.md) | existing_lids, U08 | — |
| R13 | 关闭行为明确 | 已确认原则 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U17, U18, U20 | — |
| R14 | 复杂任务保留工作面 | 已确认原则 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U20 | — |
| R15 | 本地展示不冒充结果 | 沿用既有规则 | [interaction-contracts](../../design/lids/interaction-contracts.md) | existing_lids, U08 | — |
| R16 | 共享组件与业务所有权 | 沿用既有规则 | [component-promotion](../../design/components/component-promotion.md) | existing_lids, U08 | — |
| R17 | 组件候选不自动晋升 | 沿用既有规则 | [component-promotion](../../design/components/component-promotion.md) | existing_lids, U08 | — |
| R18 | 视觉值通过语义Token | 沿用既有规则 | [tokens](../../design/lids/tokens.md) | existing_lids, U08 | — |
| R19 | 界面保留诚实的未知 | 沿用既有规则 | [ui-execution-contract](../../agents/ui-execution-contract.md) | existing_lids, U08 | — |
| R20 | 验收分层 | 沿用既有规则 | [ui-design-execution](../../governance/ui-design-execution.md) | existing_lids, U08 | — |
| R21 | 全局外壳只有一个所有者 | 沿用既有规则 | [shell-zones](../../design/lids/shell-zones.md) | existing_lids, U08 | — |
| R22 | 展示适配不丢能力 | 沿用既有规则 | [patterns](../../design/lids/patterns.md) | existing_lids, U08 | — |
| R23 | 规范状态不授予行动 | 已确认原则 | [ui-design-execution](../../governance/ui-design-execution.md) | U01, U08, U17, U18, U21 | — |
| R24 | 文案服务业务任务 | 已确认原则 | [language-policy](../../design/lids/language-policy.md) | U03, U08 | — |

## 区域 · 18项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| Z01 | 全局页头 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z02 | 位置与范围栏 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U10, U15, U17, U18, U21 | — |
| Z02-P | 面包屑 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U10, U15, U17, U18, U21 | — |
| Z02-S | 业务范围 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z03 | 侧边导航 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04 | 主工作区 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04-N | 页内分区 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U10, U15, U17, U18, U21 | — |
| Z04-T | 工作工具栏 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04-V | 视图与设置 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04-R | 结果与范围 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04-B | 主体业务内容 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04-F | 局部反馈 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z04-P | 分页与继续读取 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z05 | 详情与浮层 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z05-H | 详情身份 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z05-N | 详情分区 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z05-B | 详情内容 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |
| Z05-A | 详情动作 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U15, U17, U18, U21 | — |

## 页型候选 · 11项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| PT01 | 入口表单 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT02 | 检索与对象集合 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT03 | 概览分析 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT04 | 主题探索 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT05 | 连续材料阅读 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT06 | 编辑工作区 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT07 | 配置与关系管理 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT08 | 任务账本 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT09 | 执行诊断 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT10 | 矩阵核对 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| PT11 | 数据交付 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |

## 组件候选 · 48项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| C01 | 应用外壳 / AppShell | 原则采用／细节待审 | [shell-zones](../../design/lids/shell-zones.md) | U05, U06, U08, U13, U15, U21 | — |
| C02 | 一级导航 / PrimaryNavigation | 候选待审 | [shell-zones](../../design/lids/shell-zones.md) | U08, U21 | — |
| C03 | 二级导航 / SectionNavigation | 原则采用／细节待审 | [shell-zones](../../design/lids/shell-zones.md) | U06, U08, U13, U15, U21 | — |
| C04 | 层级路径 / Breadcrumb | 原则采用／细节待审 | [shell-zones](../../design/lids/shell-zones.md) | U03, U05, U06, U08, U13, U15, U21 | — |
| C05 | 领域或业务选择 / ScopePicker | 原则采用／细节待审 | [shell-zones](../../design/lids/shell-zones.md) | U05, U06, U08, U13, U15, U21 | — |
| C06 | 页面标题与任务说明 / PageHeader | 候选待审 | [shell-zones](../../design/lids/shell-zones.md) | U08, U21 | — |
| C07 | 上下文操作区 / ActionBar | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U05, U08, U13, U17, U18, U21 | — |
| C08 | 范围与读取时间 / ScopeSummary | 候选待审 | [shell-zones](../../design/lids/shell-zones.md) | U08, U21 | — |
| C09 | 动作按钮 / Button | 原则采用／细节待审 | [primitives](../../design/lids/primitives.md) | U05, U08, U12, U13, U21 | — |
| C10 | 图标按钮 / IconButton | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C11 | 导航与来源链接 / TextLink | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C12 | 操作菜单 / ActionMenu | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C13 | 字段容器 / FormField | 原则采用／细节待审 | [primitives](../../design/lids/primitives.md) | U05, U08, U12, U21 | — |
| C14 | 单行输入 / TextInput | 原则采用／细节待审 | [primitives](../../design/lids/primitives.md) | U05, U08, U12, U21 | — |
| C15 | 多行输入 / TextArea | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C16 | 检索输入 / SearchField | 原则采用／细节待审 | [primitives](../../design/lids/primitives.md) | U03, U05, U08, U12, U13, U20, U21 | — |
| C17 | 选项选择 / SelectField | 原则采用／细节待审 | [primitives](../../design/lids/primitives.md) | U05, U08, U12, U13, U21 | — |
| C18 | 勾选与布尔开关 / BooleanControl | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C19 | 互斥选择组 / ChoiceGroup | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C20 | 数值与单位 / NumberField | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C21 | 日期与时间窗口 / DateRangeField | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C22 | 文件选择与上传 / FileField | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C23 | 敏感值输入 / SecretField | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C24 | 表单提交与校验摘要 / FormActions | 候选待审 | [primitives](../../design/lids/primitives.md) | U08, U21 | — |
| C25 | 对象分区标签 / Tabs | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U10, U21 | — |
| C26 | 展示方式切换 / ViewSwitch | 候选待审 | [patterns](../../design/lids/patterns.md) | U08, U21 | — |
| C27 | 分页与继续加载 / Pagination | 候选待审 | [patterns](../../design/lids/patterns.md) | U08, U21 | — |
| C28 | 折叠详情 / Disclosure | 候选待审 | [patterns](../../design/lids/patterns.md) | U08, U21 | — |
| C29 | 层级树导航 / NavigationTree | 候选待审 | [patterns](../../design/lids/patterns.md) | U08, U21 | — |
| C30 | 选择与批量操作 / SelectionToolbar | 候选待审 | [patterns](../../design/lids/patterns.md) | U08, U21 | — |
| C31 | 数据表格 / DataTable | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U04, U08, U09, U16, U21 | — |
| C32 | 连续列表 / DataList | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C33 | 可打开对象卡 / ObjectCard | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C34 | 事实与说明列表 / KeyValueList | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C35 | 读数 / MetricReadout | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U03, U07, U08, U09, U21 | — |
| C36 | 状态表达基座 / StatusText | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C37 | 时间表达 / TimeDisplay | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C38 | 进度与阶段 / ProgressIndicator | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C39 | 媒体展示 / MediaViewer | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C40 | 原文引用 / SourceQuote | 候选待审 | [data-boundaries](../../design/lids/data-boundaries.md) | U08, U21 | — |
| C41 | 读取与刷新状态 / LoadingState | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C42 | 空态 / EmptyState | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C43 | 局部提示与限制 / InlineNotice | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C44 | 动作回执反馈 / ActionFeedback | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C45 | 模态对话框 / Dialog | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C46 | 后果确认 / ConfirmDialog | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C47 | 详情检视抽屉 / Drawer | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |
| C48 | 补充说明与精确值 / Tooltip | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U21 | — |

## 业务构件候选 · 41项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| B01 | 开发入口表单 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B02 | 模型连接与测试记录 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B03 | 作品材料卡 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B04 | 作品检视器 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B05 | 研究底稿与图片分层 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B06 | 评论与父语境 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B07 | 评论检索与选集 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B08 | 创作者行与身份 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B09 | 作者依据档案 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B10 | 判断与人工调整 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B11 | 观察目标档案 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B12 | 巡查规则与人工观察 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B13 | 作品表现复核 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B14 | 任务执行链 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B15 | 交付对账 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B16 | 工位身份与认领 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B17 | 领域与目标用途 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B18 | 主题身份与定义版本 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B19 | 主题地图与材料列表 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B20 | 用户生命旅程 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B21 | 主题研究与人工备选 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B22 | 研究运行及控制 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B23 | 用户问题与依据 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B24 | 方法版本与比较 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B25 | 运营指标与目标 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B26 | 转化漏斗与趋势 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B27 | 发布记录与封面 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B28 | 选题与作品目录 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B29 | 材料纸条与阅读分布 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B30 | 精确摘录与引用锚点 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B31 | 创作画布与使用关系 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B32 | 正文编辑与版本 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B33 | AI起草与采用审阅 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B34 | 封面与内页编辑 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B35 | 冻结成品导出 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B36 | 范围与资源预算预览 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B37 | 工位能力与派发阻碍 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B38 | 运营人员与平台矩阵 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B39 | 运营账号概要 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B40 | 业务日数据矩阵 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |
| B41 | 迁入批次与数据导出 | 原则采用／细节待审 | [component-promotion](../../design/components/component-promotion.md) | U08, U17, U18, U21 | — |

## 交互候选 · 32项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| I01 | 范围、路由与返回 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I02 | 检索提交与中文输入 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I03 | 筛选应用与清除 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I04 | 标签与展示切换 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I05 | 分页与选择范围 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I06 | 详情与上下文恢复 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I07 | 浮层与焦点 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I08 | 输入校验 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I09 | 未保存与离开 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I10 | 写入与异步回执 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I11 | 长任务控制 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I12 | 部分与逐项结果 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I13 | 错误、重试与结果未确认 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I14 | 版本冲突 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I15 | 来源与判断分离 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I16 | 数值与时间 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I17 | 媒体阅读 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I18 | 图表与地图联动 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I19 | 有后果动作确认 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I20 | 长列表与渐进读取 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I21 | 跨视口与辅助操作 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I22 | 编辑、保存与恢复 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I23 | 精确摘录与来源定位 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I24 | 图页顺序与结构编辑 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I25 | AI输入与采用 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I26 | 导出与下载 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I27 | 新鲜度与刷新 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I28 | 来源资格变化 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I29 | 未开放入口 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I30 | 敏感值与展示范围 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I31 | 并排比较与采用 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |
| I32 | 范围计数与覆盖 | 候选待审 | [interaction-contracts](../../design/lids/interaction-contracts.md) | U08, U20, U21 | — |

## 页面与模式 · 17项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| P01 | 开发登录入口 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P02 | 模型与 AI 设置 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P03 | 证据库 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P04 | 评论研究 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U02, U03, U04, U08, U17, U18, U21 | — |
| P05 | 创作者 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U02, U03, U04, U08, U17, U18, U21 | — |
| P06 | 主题图谱 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U10, U17, U18, U21 | — |
| P07 | Topic 动态工作区 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P08 | 观察目标 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P09 | 采集任务 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U02, U03, U04, U08, U16, U17, U18, U20, U21 | — |
| P10 | 领域管理 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P11 | 工位管理 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P12 | 创作工作台 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P13 | 运营总览 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P14 | 发布记录 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P15 | 账号管理 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P16 | 每日数据 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |
| P17 | 导入导出 | 原则采用／细节待审 | [patterns](../../design/lids/patterns.md) | U08, U17, U18, U21 | — |

## 动作预设候选 · 14项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| A-P02-add | 新增连接 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P04-start | 发起研究 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P05-settings | 发现设置 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P05-join | 加入观察 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P08-register | 注册观察目标 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P08-batch | 批量编辑 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P09-create | 单次采集 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P09-control | 任务控制 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P10-save | 保存领域或关系 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P11-manage | 管理工位 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P12-save | 保存作品 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P12-export | 导出成品 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P13-target | 设置目标 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |
| A-P17-export | 下载导出 | 原则采用／细节待审 | [regions](../../design/lids/regions.md) | U08, U17, U18, U20, U21 | — |

## 原审阅决定 · 8项

| ID | 名称 | 状态 | 正式规则落点 | 依据 | 待审 |
| --- | --- | --- | --- | --- | --- |
| D01 | 按11种页型、26个页面模式选择组合；同页可分模式 | 原则采用／细节待审 | [review-items](../../design/lids/review-items.md) | U08, U17, U18, U21 | — |
| D02 | 领域选择保留在面包屑位置；位置与范围分责、同带组合，空区省略 | 原则采用／细节待审 | [shell-zones](../../design/lids/shell-zones.md) | U02, U03, U05, U06, U08, U13, U15, U21 | — |
| D03 | 列表默认紧凑计数；仅概览主体允许大读数 | 原则采用／细节待审 | [review-items](../../design/lids/review-items.md) | U02, U03, U05, U06, U08, U09, U17, U18, U21 | — |
| D04 | FIND统一检索结构，保留按页的提交/防抖策略 | 原则采用／细节待审 | [review-items](../../design/lids/review-items.md) | U02, U03, U05, U06, U08, U12, U13, U20, U21 | — |
| D05 | 白底、黑结构、橙重音；1px表格分隔与2px结构线；有限4px硬阴影 | 原则采用／细节待审 | [review-items](../../design/lids/review-items.md) | U02, U03, U05, U06, U08, U13, U23 | — |
| D06 | 原文用相同sans字族，靠来源与引文结构表明角色；功能字下限11px | 已确认原则 | [typography](../../design/lids/typography.md) | U08, U09, U11 | — |
| D07 | 主操作沿用真实工具栏黑底与右侧点阵；次级墨框窄点阵；危险与文本按钮分责，普通hover不位移 | 原则采用／细节待审 | [review-items](../../design/lids/review-items.md) | U02, U03, U05, U06, U08, U13, U21 | — |
| D08 | 18个区域坐标+48组件合同+41业务块+32交互共用注册表 | 原则采用／细节待审 | [review-items](../../design/lids/review-items.md) | U08, U17, U18, U21 | — |

## 覆盖与边界

共213项，P类含26模式；候选的结构、状态、交互引用保存在清单，可按ID读取。
本表完整性检查不验证真实DOM/CSS、视觉质量、数据来源、行为正确性、权限或业务结果；[待审清单](review-items.md)和[决定台账](decisions.md)分别记录待决与正式采用。
