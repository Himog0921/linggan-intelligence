# AI-SETTINGS-LEFT-NAV-001 · AI 设置左侧导航与标题回收

> 状态: 草案
> 最后核对: 2026-10-21
> 事实来源: 用户当前要求、当前候选 React 源码、聚焦测试、隔离浏览器截图与 `5997b189` 基线复现
> 适用范围: /settings/models?view=capabilities|models|runs|advanced
> 授权来源: Mog 当前直接要求「AI 设置为个人菜单的二级导航栏，采用左侧导航；删除三级页面的 SYSTEM / AI OPERATIONS、AI 设置、说明与刷新设置」
> 冲突时以谁为准: 用户最新确认、AGENTS.md、当前产品页面合同、真实代码和 LIDS

## 1. 读取回执

| 类别 | 来源 | 读取结论 | 当前实现映射 |
|---|---|---|---|
| 项目约束 | AGENTS.md、docs/README.md、docs/current-state.md | React、CSS Modules、LIDS Token、独立工作树、文档与进度同步 | 仅改 React 设置页、现有测试与本清单 |
| 产品页面 | docs/design/pages/model-ai-settings-page.md、docs/pages/model-ai-settings.md | 设置页管理连接、配置、运行回执；保存与真实研究仍分开 | 不改 API、资格、数据或动作 |
| 壳层与区域 | lids/shell-zones.md、lids/regions.md、lids/patterns.md | 模块入口使用 Z03 侧栏；页面身份已由导航说明时不重复同名 Hero | 复用 AppShell、Sidebar、NavIcon 和 v7-* 壳层 |
| Token、语言与数据 | lids/tokens.md、primitives.md、typography.md、language-policy.md、data-boundaries.md | L1 Settings / Governance，中文为主，未知与失败保持原有真实表达 | 不新增 Token、状态词或文案宣言 |
| 当前代码与测试 | ModelSettingsPage.tsx、设置页单测、settings-real-chain.mjs | 四个 view= 入口当前位于 Context Bar；页面 Hero 与刷新按钮不触发写操作 | 将导航移至共享左栏，保留 URL 与读写合同 |

## 2. 表面、状态、依赖与验收矩阵

### 表面地图

| 表面 | 用户结果 | 本次改动 |
|---|---|---|
| 个人菜单的 AI 设置入口 | 进入当前默认 AI 能力页 | 不改入口地址 |
| AI 设置左栏 | 在 AI 能力、模型连接、运行与用量、高级配置之间切换 | 用共享可折叠侧栏承载四个 view= 链接 |
| AI 设置主区 | 直接查看当前视图内容、错误或读取状态 | 删除可见 Hero、说明和“刷新设置”按钮 |
| 历史 section= 深链接 | 已保存链接仍能读取原旧区 | 保留旧区内容与 URL；只在旧链接时显示其兼容导航 |

### 状态词典

| 状态 | 数据/触发来源 | 用户应理解 | 不得暗示 |
|---|---|---|---|
| 正常读取 | 既有 fetchAiSettings 成功 | 左栏当前项对应当前四视图内容 | 切换导航写入配置 |
| 读取中 | 既有 React Query pending | 设置仍在读取 | 空配置或已刷新 |
| 读取失败且有旧值 | 既有 React Query error + data | 下方是最近成功读取的内容 | 当前读取成功 |
| 无数据的读取失败 | 既有 React Query error | 不能核对设置 | 没有连接或运行已停止 |
| 旧深链接 | URL 含被支持的 section= | 兼容内容仍可读取 | 它成为新的普通视图或改变权限 |
| 写动作回执 | 既有 useAiAction | 只由服务端回执更新 | 移除刷新按钮意味着本地保存自动成功 |

### 依赖地图

- 共享壳：AppShell、Sidebar、NavIcon 和 shell.css；不修改共享壳。
- 页面：ModelSettingsPage.tsx 和 AiSettingsExperience.module.css。
- 数据与动作：fetchAiSettings、useAiAction、已有 API 及现有配置表单；不修改。
- 证明：设置页 Vitest、真实 React→Axum→自有 PostgreSQL 浏览器链路、类型/lint/build 与治理检查。
- 无并行共享文件所有权；当前主目录的无关脏改动不纳入本工作树。

### 验收矩阵

| 层级 | 验收方法 | 目标 | 当前边界 |
|---|---|---|---|
| 任务可用 | 四视图单测与浏览器深链接 | 左栏四项可达、当前项明确、URL 保留 | 不证明真实供应商 |
| 状态诚实 | 现有 success/pending/error/legacy 测试 | 读取、旧值、失败与回执文案不变 | 不新增状态 |
| 视觉一致 | 1440px 与 390px 隔离浏览器截图及无横向溢出断言 | 没有可见 Hero/刷新，主区从内容开始 | 本次通过；需 Mog 最终视觉验收 |
| 真实后果 | 浏览器 POST 计数与现有回执测试 | 仅导航不写入、不启动研究或模型 | 本次页面合同通过；不部署、不调用真实模型 |

## 3. 变更分类与边界

- 分类：展示 + 导航交互；最高风险为导航交互。
- LIDS：L1 Settings / Governance；复用共享 Shell 的 Z03 Sidebar，不新增 Primitive、CMP、Token、Scene 或 Motion。
- 影响页面：仅 /settings/models 的四个能力优先视图与旧深链接的导航呈现。
- 非目标：不改个人菜单入口、API、查询键、数据合同、模型连接、执行器开关、历史 section= 地址、权限、迁移、运行服务或部署。
- 停止条件：若需要新导航层级、变更设置动作含义、改写旧链接或触及共享壳行为，则停止并要求新的产品决定。

## 4. 实施与证明回执

- 实施：四个正常 `view=` 入口复用 `Sidebar`/`NavIcon`；可见 Hero 与刷新按钮移除，`h1` 仅保留给辅助技术；旧 `section=` 链接保留原分区布局和行为。
- 已通过：设置页聚焦测试 17 项、TypeScript、ESLint、LIDS 检查与生产构建；隔离浏览器在 1440×900、390×844 走完四个正常视图且无横向溢出、无导航 POST。
- 已知边界：完整设置实链在做法导入收到 `400` 且等候回执超时；未修改的 `5997b189` 基线复现同一失败。详情见 [ACC-AI-SETTINGS-LEFT-NAV-001](../acceptance/ai-settings-left-nav-001-acceptance.md)。
- 发布边界：未合并、未推送、未部署或刷新 `:3000`，无迁移、共享数据库写入、真实模型或研究调用。
