# Intelligence 全站 UI 共享组件标准化 · 准备审查

> 状态：准备阶段完成，待用户审阅后进入实施
> 类型：只读代码盘点与实施方案；本文件不改变运行时代码
> 核对基线：GitHub main，2026-11-04

## 1. 目标与范围

不是把所有页面做成同一个模板，而是让它们使用同一套基础组件、视觉规则和交互规则。主题图谱保留图谱/主题树/旅程，运营保留概览/台账/创作工作区，语料保留研读/表格/封面/评论研究；它们不需要同一内容布局。

统一的是：按钮、下拉、页签、搜索、状态、表格基础行为、弹窗/抽屉、导航层级和交互状态。页面通过配置/适配器提供数据、查询、权限、路由和动作。共享组件不承担领域资格、业务判断、权限或真实动作回执。

默认范围：
- 纳入所有标准 AppShell 业务页面：语料、采集、主题图谱、运营、设置、Topic 工作区。
- 登录、邀请等独立入口不强制改成 AppShell。
- 创作工作台的沉浸式工作面、采集运行控制面保留自己的页面 Pattern，但复用可共用基础组件。
- 全站共享页头/Context Bar 统一；不把所有页面主内容区改成黑底。

## 2. main 代码盘点

### 已有基础
- apps/web/src/app/AppShell.tsx 是标准全局页头入口。
- apps/web/src/styles/shell.css 承担共享页头、Context Bar、三级导航、领域选择器和侧栏样式。
- apps/web/src/shared/ 已有 ActionButton、FindField、SelectField、Sidebar、IntelligenceIcon 等共享代码。
- LIDS 已有 Token → Primitive → Component → Pattern → Page 五层方法，以及 shared / adapter / business / gap 四种复用路线。

### 尚未完成
- AppShell 的 crumb、contextNav、meta 都接受任意 ReactNode。外壳共享，但页面仍可传入任意 HTML、按钮和 CSS，没有强制使用统一 Breadcrumb、Domain Picker、Tertiary Tabs、Context Meta。
- LIDS 复用目录登记 48 项：shared 29、adapter 13、business 1、gap 5；正式 CMP 认证数为 0。
- component-contracts.json 当前只有 C09 ActionButton、C16 FindField、ICON-B-001 图标资产三份实现合同。
- component-reuse.json 的 runtimeMigrated=false，表示分类已登记，不代表 48 项都已迁移或认证。

### 已发现的页面差异
| 页面 | 当前实现 | 主要问题 |
|---|---|---|
| 主题图谱 /topics | AppShell + Topic Engine + Topics.module.css | 领域选择、概览/旅程 Tab、操作区由页面自定义；Context Bar 部分样式写在页面 CSS |
| Topic 工作区 | AppShell + 专属工作区 | 页面布局可专属，基础控件仍需统一 |
| 离线预演 | AppShell + 独立页面 | “离线预演”链接放在页面内容最前面，成为页头下方的普通蓝色链接 |
| 运营数据 | OpsFrame → AppShell + Ops.module.css | 自有按钮、输入、Tabs、工具栏、表格样式 |
| 运营创作 | WorkbenchPage → OpsFrame（immersive） | 保留专属工作流；基础控件仍应复用 |
| 语料证据库/创作者 | CorpusFrame → AppShell | Frame 已共享，但 Search、Quick Tabs、Select、表格、抽屉仍有页面级实现 |
| 语料评论研究 | AppShell + CorpusSideNav | 有自己的 DomainPicker 和 Context Tabs |
| 采集 | CollectionFrame → AppShell | 领域菜单以内联 details 实现，工具栏和表格采用 Collection 专属 CSS |
| 设置 | AppShell + 各设置页 | 外壳共享，表单、反馈、菜单仍有页面级实现 |

源码证据：
- Topics.module.css 约 3,210 行/92.7KB；Ops.module.css 约 1,654 行/43.8KB。
- Topic engine.tsx 约 76KB，采用独立状态订阅和事件委托，并返回 React 内容。不要为了 UI 统一一次性重写其业务状态机。
- topicElements.tsx 自己定义 btn/tab/badge/empty。
- TopicsPage、CollectionFrame、Corpus DomainPicker、OpsScopePicker 分别承载不同领域选择实现。

领域选择尤其要区分：观察领域、语料领域、采集领域、运营业务范围的数据来源、默认值、资格和 URL/状态保留规则不同。应统一呈现和基础交互，用适配器保留业务语义，不能合并成万能选择器。

## 3. 目标架构

继续遵循 LIDS 五层，不新建第二套设计系统：

Token → Primitive → Shared Components → Page Patterns / Adapters → Pages

### 共享外壳
- BreadcrumbTrail：路径与当前页面/对象。
- DomainScopePicker：统一外观、键盘/鼠标行为和菜单；页面适配器提供业务范围数据与切换动作。
- ContextTabs：三级页面导航，支持路由链接和原地视图切换两种适配，外观/选中态/键盘行为统一。
- ContextMeta：真实状态、范围读数、更新时间等适合放在 Z02 的信息。

逐步将 AppShell 的任意 ReactNode 插槽改为结构化配置；页面声明配置，AppShell/Frame 管布局和响应式。

### 基础交互组件
- ActionButton：主/次/安静/危险/加载/禁用。
- FindField/TextField：结构共享，防抖、手动提交、IME 行为由页面适配器声明。
- SelectField / DomainScopePicker：普通选项与业务范围选择分开。
- Tabs / NavigationTabs：页面分区、对象内部 Tab、内容视图切换保留各自语义。
- Feedback：加载、空、错误、部分、未知、真实回执；五轴状态不合并成万能 Status。
- Dialog / Drawer：共享焦点、键盘、关闭和遮罩容器；稿件、请求和回执仍由页面控制。

### 页面组合 Pattern
逐步抽出 Search + Filter + Sort Toolbar、Result Summary、Table/List Frame、Split Workspace、Detail Drawer、Empty/Loading/Error/Partial 状态区。组合是可复用模式，不是强迫所有页面采用同一布局。

## 4. 实施顺序

**阶段 0：准备与基线（本次）**
已核对 main、路由、AppShell/Frame、共享组件目录、LIDS 复用目录和主要页面样式；只读，不改业务运行代码。

**阶段 1：共享壳层**
统一 Breadcrumb、Domain Scope Picker、Context Tabs、Context Meta；修复主题图谱、评论研究和运营的导航差异；调整离线预演入口位置。保持路由、领域切换、URL、返回历史和页头高度不变。

**阶段 2：基础组件**
收敛按钮、搜索、选择器、页签、状态、反馈和浮层。每项先查重、定义责任/状态/禁止场景/无障碍要求，再由第二个独立页面验证，达到七项门槛后才申请正式 CMP。

**阶段 3：页面组合**
建立搜索筛选工具栏、结果摘要、表格/列表框架、分栏工作区、详情抽屉等组合。页面仍拥有列、筛选、权限和真实数据状态。

**阶段 4：模块迁移**
1. 主题图谱 + 运营：用户已明确指出的主要不一致页面。
2. 语料：证据库、创作者、评论研究，作为跨页验证组。
3. 采集：观察目标、采集任务、领域管理、工位管理。
4. 设置和其他标准 AppShell 页面。
5. 创作工作台：保留沉浸式布局，只统一可复用基础控件。

**阶段 5：防止再次漂移**
共享外壳样式只由 shell.css/AppShell 管理；正式共享控件只有一个 shared 入口；页面 CSS 不覆盖共享组件；CI 增加重复实现、外壳样式越权、Token 漂移检查；建立代表页面视觉回归与键盘/窄屏测试。

## 5. Pattern Waves 与 LIDS 规则

用户已确认黑色方格 Silk Context Bar 的视觉方向和参数，但 main 当前包含 React Bits 源码。考虑到用户明确希望 Intelligence 不依赖第三方组件源码，正式共享组件应独立实现并保留已批准视觉效果，不继续以复制的第三方实现作为长期基础。

当前 LIDS materials.md / system.md 对深色面和动态材质仍有既定限制。正式实施前应将本次确认记录为精确的共享壳例外：
- 仅限标准 AppShell 的 Context Bar。
- 不扩展为全站深色主题；正文、表格、详情阅读区保持白场。
- 导航文字、白底黑字领域按钮和紧凑棋盘格尾部按用户终稿。
- Reduced Motion、WebGL 不可用、窄屏和 GPU 性能有明确处理。
- 同步更新 LIDS、组件合同、参数源、示例和验收，避免用户批准的效果与规范冲突。

## 6. 验收标准

视觉：
- 同职责组件在主题图谱、运营、语料、采集、设置中尺寸、字级、边框、颜色、间距和状态一致。
- hover/focus/active/disabled/loading 表达一致。
- 黑色 Context Bar 只改变导航带，不改变主体白场。

交互：
- 按钮、下拉、Tab、搜索、弹窗、抽屉的鼠标/键盘/触屏行为一致。
- 领域切换保留各页面现有 URL、筛选、返回历史和业务资格。
- 未保存、处理中、结果未确认、错误、空值和未知状态保留原语义。

工程：
- 同一职责只有一个共享实现，业务差异由 adapter 提供。
- 页面 CSS 不覆盖共享壳层和正式共享组件。
- LIDS 源、生成目录、组件合同、实现和测试同步。
- TypeScript strict、lint、单测、build、LIDS checks 通过。
- 代表页面完成桌面/窄屏、键盘、Reduced Motion 和真实浏览器验收。

不得发生：
- 不改变业务 API、数据库、权限、采集、研究、发布或真实动作合同。
- 不把不同页面的事实、资格、状态和操作后果压成同一种语义。
- 不一次性重写 Topic Engine、运营业务状态或创作工作流。
- 不以静态 Demo 代替真实页面验收。

## 7. 默认执行范围

目前已有信息足以进入下一阶段，不需要用户提供技术细节。默认：
1. 先做主题图谱与运营，再用语料页面验证共享组件。
2. 所有标准 AppShell 页面共享壳层；登录/邀请不强制套用；沉浸式工作台保留独特布局。
3. Pattern Waves 保留视觉效果，但正式实现独立开发，并同步 LIDS 例外。
4. 第一轮只统一视觉与交互，不改业务流程、数据口径、权限或接口。

本文件来自 main 源码与规范的静态核对，不代表已完成实际浏览器视觉审查。真实数据状态、不同角色、键盘/读屏、Reduced Motion、窄屏、GPU 性能和生产运行仍需在各实施阶段验证。
