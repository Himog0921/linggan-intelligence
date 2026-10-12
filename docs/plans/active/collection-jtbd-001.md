# COLLECTION-JTBD-001 · 采集日常两页与管理入口收敛

> 状态: 活跃计划
> 最后核对: 2026-10-12
> 适用范围: Collection 导航、原观察目标/采集任务页面及领域/工位管理返回链
> 事实来源: Mog 2026-10-12 对 Demo 的确认和真实 UI 保留要求、main d94e72c7、当前 SSR/采集控制合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/测试与既有数据/行动合同；Demo 不取代生产字段或扩大采集权限

## 1. 事项、Claim 与代码前 UI 变更清单

- Issue: [#398](https://github.com/Himog0921/linggan-intelligence/issues/398)。用户直接授权真实项目实施，2026-10-12 已登记 Claim。
- 基线: `d94e72c7a53d56527acedf7863042c74babb8b9a`；branch `codex/collection-jtbd-20261012`；worktree `<repo>/.worktrees/collection-jtbd-20261012/`。
- 用户结果: 日常侧栏只保留观察目标与采集任务；管理领域放在面包屑领域下拉中，工位管理放在面包屑右侧。两个保留页面的现有内容与 UI 不删减。
- 分类: 混合（导航交互、既有受控命令入口），最高风险为权限/行动；不新增后端权限或采集引擎。
- 视觉: L1 Collection Control；既有目标抽屉为受限 L2。继续消费现有 `--lgi-*`、按钮、表格、弹窗、shell；不升级 Token，不改主题、列密度、字号或截图字段。
- 非目标: 新 schema/migration、临时关键词采集协议、插件发布、全局领域过滤合同变更、真实平台采集、模型调用、生产数据库操作、部署/合并。
- 例外裁定: 当前用户明确要求取代 PAGE-COLLECTION-001 的旧六页顺序；context bar 工位入口为本包明确授权，不向其它模块推广。

## 2. 读取回执

| 来源 | 本次用途 | 核对 |
|---|---|---|
| AGENTS、docs/README、current-state、governance | 先文档后代码、独立工作树、文件归属与交付证明 | 已读 |
| UI execution contract、design-governance、LIDS 入口/Token/shell/data-boundaries/language | 复用既有 UI、保护未知、上下文入口与反馈 | 已读 |
| PAGE-COLLECTION-001、PAGE-DOMAIN-MANAGEMENT-001 | 被最新决定替代的导航与必须保留的内容 | 已读 |
| collection/targets/tasks/station 的 SSR、JS、CSS 与测试 | 真实字段、管理能力、装配槽、依赖及返回路径 | 已读 |
| collection_control 人工观察命令、Task 时间线、station_read | 幂等/版本、提交与执行区别、全局工位与额度 | 已读 |

## 3. 表面地图与保留清单

| 表面 | 本包变化 | 必须保留 |
|---|---|---|
| 全局 shell / Collection rail | 仅 01 观察目标、02 采集任务；默认观察目标 | 全局一级导航和原 UI |
| 观察目标 | 管理入口位置变化；无通用单次采集按钮 | 全部来源/创作者/关键词/建档中/巡逻中、计数、最近观察、类型/身份输入、批量编辑/新建；创作者与关键词 13 列、全部行操作与抽屉 |
| 采集任务 | 唯一通用单次采集入口 | 7 列账本、概览/Attempt/Package/Receipt/冻结资源、详情交付对账、真实失败与全局读取范围 |
| 领域管理 | 领域下拉底部入口；完整管理页与返回来源 | 名称/说明/研究目标、状态、材料 lanes、统计、Target 关系与 primary/reference、表单和错误草稿 |
| 工位管理 | 面包屑右侧入口；完整管理页与返回来源 | 十列表格、六项运行读数、名称编辑、200 篇/日真实额度、接活、账号、能力/版本自检、安装认领、窗口、退役、在途/排程、运行概览 |
| 原待处理/生产流 | 删除独立页面，旧 GET 地址跳转任务页 | 原任务回执/冻结事实与工位诊断，不删除底层事实 |

管理入口使用原 SSR 页面，不复制 Demo 简表或动态注入嵌套弹层。返回地址仅允许两个本地日常页面，保留领域及原列表上下文；表单成功/失败继续显示原反馈。

## 4. 状态词典与行动合同

| 状态/动作 | 用户含义 | 禁止替代 |
|---|---|---|
| 读取失败/UNKNOWN | 当前事实无法读取 | 0、无工位/无任务、已完成 |
| 成功空读 | 当前有界读取没有记录 | 全历史不存在 |
| 工位暂停接活 | 不接未来新工作 | 修复登录、取消在途、停用工位 |
| 工位管理领域上下文 | 仅用于返回工作页；工位是全局资源 | 按领域分配/暂停工位 |
| 单次采集 | 对已有目标人工观察一次，复用既有命令 | 任意新链接/临时关键词已获得下发能力 |
| 命令提交成功 | 新建或复用 WorkOrder，显示耐久回执和引用 | 已执行、已采回、已生成 Task |
| 准入/版本/幂等拒绝 | 本次申请被拒，显示真实原因 | 本地 toast 成功、乐观状态 |

人工观察继续经过目标版本、幂等身份、生命周期、关键词建档、Domain/授权/Admission 守门。不修改自动规则，不在点击时指定工位。目标上下文的既有「人工观察一次」保留，通用入口只在任务页。任务账本保持现有全局最多 100 条口径，不显示无效领域筛选器。

## 5. 依赖与文件所有权

- `/root`: `local_web.rs`、`collection.rs`、必要管理导航模块及治理文档；负责集成。
- `frontend_inventory`: `collection_task_actions.rs` 及专属 `collection_task_actions_tests.rs`、`crates/evidence/src/collection_task_action_read.rs` 及 `lib.rs` 的精确导出；只读目标用途/命令回执放 evidence 层，原任务表/目标表/抽屉/schema 不改。
- `backend_flow`: 工位能力与八条 POST 核对；`collection_management.rs`、专属 `collection_management_tests.rs`、collection JS/CSS 的管理导航增量。
- `jtbd_challenge`: 只读验证准备与独立 commit-reviewer，不承担实现。
- 必要依赖: collection 局部 JS/CSS、既有路由/渲染测试、限定本分支 GitHub Actions 验证；共享 shell/其它模块只读。
- 禁改: 目标正文/列定义、工位业务控制真源、插件、迁移、运行凭据、其它并行 PR 功能。
- 停止条件: 新平台权限、扩大临时目标语义、删除保留字段、覆盖他人分支或生产变更时停止该部分并说明；不影响已授权入口收敛。

## 6. 验收矩阵

| 层 | 验收方法 | 当前结果 | 未证明边界 |
|---|---|---|---|
| 导航/返回 | SSR 路由；恰好两项；管理入口；旧址迁移；安全返回白名单 | 待实施 | 部署 |
| 字段/能力 | 原目标/任务/工位渲染回归与差异核对；表单字段/动作不丢 | 待实施 | 用户视觉签收 |
| 单次命令 | 表单/版本/幂等/拒绝/回执与既有后端测试 | 待实施 | 真实插件领取与平台访问 |
| 视觉 | 1440px 桌面原样式、入口位置、管理弹窗/抽屉与焦点 | 待实施 | 无浏览器时不得冒充视觉通过 |
| 工程 | Rust 格式/构建/相关测试、JS 语法、设计/项目治理、独立审查 | 工具链调查中 | 本机目前无 Rust，不以静态检查代替编译 |

## 7. 交接记录

### 2026-10-12 实施与提交前审查

- 原 `collection_targets_view.rs`、`collection_tasks_view.rs`、`target_drawer.rs/css`、`station_view.rs`、`shell.rs/css`、`lids_tokens.css` 与基线逐字节一致；仅由原壳/装配链增添导航、完整管理返回与单次入口。
- 管理页所有成功、失败、不可读分支统一携带返回上下文；八条工位写路由及领域错误草稿保留。返回只承诺 path/query/hash，任务行/Inspector Tab 仍沿用原 DOM 临时状态，不宣称恢复该临时选择。
- 审查发现人工观察 replay 的 URL 提示可能与耐久工单回执冲突，已接受并修正：有回执时由精确 target/receipt/manual_observe 查询表达结果。新增回归覆盖成功/拒绝 replay。
- 新 evidence 两项只读投影已核对当前 schema/命令字段；本机没有 PostgreSQL/Rust，本包尚未运行其 SQL，不以零匹配测试冒充查询验证。
- 本机已通过 JS 语法、diff 空白、项目治理与设计手册检查。Rust/浏览器将在只针对本分支的 CI 中运行；页面 smoke 使用无数据库的真实服务，不访问平台、不执行 POST。

最终命令结果、独立审查结论与 PR 链接完成后追加。合成/隔离验证不等于生产采集或用户业务验收；合并按 AGENTS 的 exact-head 要求单独授权。


提交前独立只读审查通过（仅静态范围），无未修复确定阻断项；原 replay 矛盾已接受修正。根重算 12 源码/浏览器脚本文件聚合 SHA-256 与 reviewer 一致：`eddbbe068d9d53b6bb079d643b3a9c15de7ba4fdcc51b51e17afa8719b249234`；workflow SHA-256 `a7c9812cde9bccc015fa9fc5b45d3f51eb16ebbf983ac06c38ace769304aa300`。允许提交 Draft PR 进入 CI，不等于编译、SQL、浏览器或合并通过。专用 `~/.claude/agents/commit-reviewer.md` 在本环境缺失，已由独立只读代理按当前合同审查，未假称运行不存在的定义。
