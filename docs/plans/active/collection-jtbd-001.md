# COLLECTION-JTBD-001 · 采集日常两页与管理入口收敛

> 状态: 活跃计划
> 最后核对: 2026-10-12
> 适用范围: Collection 导航、原观察目标/采集任务页面及领域/工位管理返回链
> 事实来源: Mog 2026-10-12 对 Demo 的确认和真实 UI 保留要求、main d94e72c7、当前 SSR/采集控制合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/测试与既有数据/行动合同；Demo 不取代生产字段或扩大采集权限

本文件仅保留 #400 的导航历史范围；当前完整交付由 [collection-single-run-398.md](collection-single-run-398.md) 替代，不再执行旧限制。

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
- `frontend_inventory`: `collection_task_actions.rs` 及专属 `collection_task_actions_tests.rs`、`crates/evidence/src/collection_task_action_read.rs`、专属投影集成测试及 `lib.rs` 的精确导出；root 在格式化后将固定错误说明原样拆入 `collection_task_action_feedback.rs`，使新增生产文件保持 500 行以内。只读目标用途/命令回执放 evidence 层，原任务表/目标表/抽屉/schema 不改。
- `backend_flow`: 工位能力与八条 POST 核对；`collection_management.rs`、专属 `collection_management_tests.rs`、collection JS/CSS 的管理导航增量。
- `jtbd_challenge`: 只读验证准备与独立 commit-reviewer，不承担实现。
- 必要依赖: collection 局部 JS/CSS、既有路由/渲染测试、限定本分支 GitHub Actions 验证；新增 `scripts/test-collection-jtbd-postgres.sh` 复用已锁定的测试镜像，创建并清理专属临时库/容器/卷，只运行本包两项投影证明。共享 shell/其它模块只读。
- 禁改: 目标正文/列定义、工位业务控制真源、插件、迁移、运行凭据、其它并行 PR 功能。
- 停止条件: 新平台权限、扩大临时目标语义、删除保留字段、覆盖他人分支或生产变更时停止该部分并说明；不影响已授权入口收敛。

## 6. 验收矩阵

| 层 | 验收方法 | 当前结果 | 未证明边界 |
|---|---|---|---|
| 导航/返回 | SSR 路由；恰好两项；管理入口；旧址迁移；安全返回白名单 | 0b18c96e 的 Rust 与真实浏览器检查通过 | 部署 |
| 字段/能力 | 原目标/任务/工位渲染回归与差异核对；表单字段/动作不丢 | 8 份原渲染/样式文件字节不变；118 Collection、49 Station、66 API 测试通过 | 用户视觉签收；13 项原 ignored 不计通过 |
| 单次命令 | 表单/版本/幂等/拒绝/回执边界；精确读取用途与回执 | 渲染/路由回归与 2 项真实隔离 PostgreSQL 投影证明通过 | 未重新证明真实平台执行；fixture 不等于命令端到端 |
| 视觉 | 1440px 桌面原样式、入口位置、管理往返与弹窗焦点 | 1440×1000、5 条实际服务浏览器场景通过，0 JS 异常；5 张截图已核对 | 无生产数据，未代替用户对有数据页面的签收 |
| 工程 | Rust 格式/构建/相关测试、JS 语法、设计/项目治理、独立审查 | CI 构建与治理通过；最后格式/局部截图收尾后的精确 head 以 PR #400 检查为准 | 本机无 Rust，明确使用 CI 回执 |

## 7. 交接记录

### 2026-10-12 实施与提交前审查

- 原 `collection_targets_view.rs`、`collection_tasks_view.rs`、`target_drawer.rs/css`、`station_view.rs`、`shell.rs/css`、`lids_tokens.css` 与基线逐字节一致；仅由原壳/装配链增添导航、完整管理返回与单次入口。
- 管理页所有成功、失败、不可读分支统一携带返回上下文；八条工位写路由及领域错误草稿保留。返回只承诺 path/query/hash，任务行/Inspector Tab 仍沿用原 DOM 临时状态，不宣称恢复该临时选择。
- 审查发现人工观察 replay 的 URL 提示可能与耐久工单回执冲突，已接受并修正：有回执时由精确 target/receipt/manual_observe 查询表达结果。新增回归覆盖成功/拒绝 replay。
- 新 evidence 两项只读投影已核对当前 schema/命令字段；本机没有 PostgreSQL/Rust，本包尚未运行其 SQL，不以零匹配测试冒充查询验证。
- 本机已通过 JS 语法、diff 空白、项目治理与设计手册检查。Rust/浏览器将在只针对本分支的 CI 中运行；页面 smoke 使用无数据库的真实服务，不访问平台、不执行 POST。

最终命令结果、独立审查结论与 PR 链接完成后追加。合成/隔离验证不等于生产采集或用户业务验收；合并按 AGENTS 的 exact-head 要求单独授权。

验证补充：调查并复用现有临时 PostgreSQL 测试夹具，为新增两项只读投影验证 primary 用途边界和精确命令回执。只允许隔离测试数据库与合成 fixture，不触碰生产或外部平台；若可复用则纳入本分支 CI，实际运行前仍标记为未证明。


提交前独立只读审查通过（仅静态范围），无未修复确定阻断项；原 replay 矛盾已接受修正。根重算 12 源码/浏览器脚本文件聚合 SHA-256 与 reviewer 一致：`eddbbe068d9d53b6bb079d643b3a9c15de7ba4fdcc51b51e17afa8719b249234`；workflow SHA-256 `a7c9812cde9bccc015fa9fc5b45d3f51eb16ebbf983ac06c38ace769304aa300`。允许提交 Draft PR 进入 CI，不等于编译、SQL、浏览器或合并通过。专用 `~/.claude/agents/commit-reviewer.md` 在本环境缺失，已由独立只读代理按当前合同审查，未假称运行不存在的定义。

### 首轮 CI 与最新主线对齐

- 候选 PR: [#400](https://github.com/Himog0921/linggan-intelligence/pull/400)。验证 workflow 的 runner 临时目录初始化已移至 runner step，避免 job env 使用不可用 context。
- 同步主线 `be4b485d`（#399）；独立审查逐字节核对双方代码保真，只将三份共同文档的新增记录并存，没有改写主题图谱功能。
- [CI 38239219017](https://github.com/Himog0921/linggan-intelligence/actions/runs/38239219017) 在 `d6cfeffcb32a5f6b9af3a97bdb9af84bfe0d10c8` 的 API 构建通过。首个 collection 测试组 117 通过、1 失败、1 ignored；后续工位/导航与浏览器步骤未运行，不记通过。
- 失败来自旧测试对整页禁止 `targets?domain=`，包含了新增领域菜单明确的 `domain=all` 项。修复为检查实际侧栏两条导航的裸路径，同时正向保护领域菜单与管理入口，不改变运行代码或删除断言保护目标。
- 应用该 CI 导出的 Rust 格式补丁；固定错误说明原样拆为小模块，使格式化后的新增生产文件仍少于 500 行。新增 2 个 PostgreSQL 投影证明及独占临时资源 harness 已通过独立静态审查，等待新提交实际执行。

修复/格式化/投影证明提交前独立增量审查通过；root 重算 11 个代码/配置文件聚合 SHA-256 与 reviewer 一致：`bea53edf4e7860e2d96abcd65379f9610fcc601e8a26b188bda87240d1fe993f`。此项只批准进入新提交 CI，未把旧 head 的构建或未执行的 PG/浏览器记为本次通过。

### 第二轮回归发现与修复

- [CI 38239986950](https://github.com/Himog0921/linggan-intelligence/actions/runs/38239986950) 在 `6847df4090a69c21facc6e61b96291d99b650358` 构建通过，Collection 118 通过/1 ignored，Station 49 通过。API/local_web 组 63 通过/3 失败/12 ignored；PG 与浏览器因前置失败未执行。
- 接受真实错误恢复缺陷：无数据库时原新建领域失败页没有 `domain-create` 表单，草稿替换分支无落点。现在在原工作区内以既有 `domain-management` 样式保留草稿与错误，仍携带返回上下文；正常新建/编辑替换路径不变。
- 接受样式所有权问题：新增领域管理链接只由自己的 class/data 属性选中，不以 shell 类起头定义，保留同样的局部间距并使用既有线宽 Token。公共 shell CSS 仍未修改。
- 更新过时的“context meta 必须全空”断言为只允许已获批准的工位管理链接，继续禁止重复目标读数；草稿测试增加正文位置与原样式容器验证。
- 该提交 CI 的 rustfmt 补丁为 0 字节（下载 ZIP SHA-256 `76f3ec37339a5bd2f1f7944276313d256f5264c77884a538f46d7b435c6759bc`）。后续 workflow 让编译成功后的 PG/浏览器/治理检查独立执行，任一失败仍使 job 失败，不用 continue-on-error 掩盖失败。

第二轮失败修复经独立增量审查通过，root 重算 4 文件代码/配置 SHA-256 与 reviewer 一致：`85b620bf472311581251c456544e19d7fa7662135b3edd48c71aa88b781a2425`。待下一提交实际 CI 验证。

### 完整隔离验证与截图收尾

- [CI 38240768798](https://github.com/Himog0921/linggan-intelligence/actions/runs/38240768798) 在 `0b18c96ef2de6ea4c04233d6235867d0a97e9520` 构建、三组页面回归、两项 PostgreSQL 投影证明、浏览器与治理全部通过。三组分别为 118 / 49 / 66，共 233 个非忽略测试；13 项原有 ignored 没有冒充通过。
- PG 独立 target 实际 2/2 通过、0 filtered（2.57 秒）；创建的数据库/容器/卷清理成功。该 fixture 只证明新读取逻辑，不把手工写入的合成 Work Order 当作真实准入/平台执行证明。
- 1440×1000 无数据库真实服务通过五条导航/弹窗场景：默认目标与仅两项侧栏、领域菜单管理往返、右侧工位往返、唯一通用单次入口与不可读禁提交/Escape 焦点、旧址跳转。0 未捕获浏览器错误，未发送 POST 或外部请求。
- 已下载并核验浏览器 artifact（ZIP SHA-256 `3739a73aa50eee96991d83a285551b6216fb956e7e6a748d9a1b3e2eace459d1`），检查全部五张截图。发现新增任务按钮作为链接仍有默认下划线且文字未垂直居中；仅在 `data-task-actions` 内补按钮布局与文本装饰，不改变颜色、字体、尺寸或原控件。工位不可读说明的旧“待处理”去向改为采集任务/观察目标。
- 应用同次 artifact 中唯一一处 Rust 换行补丁（领域草稿恢复的链式调用，937 字节）；workflow 现在要求格式补丁为空才通过。该最终收尾快照提交后的精确 head 重跑与独立审查回执记录在 PR #400，不将前一 head 的通过代替它。
- 交付状态：真实项目候选已实现；不合并、不部署、不连接生产数据库、不触发真实采集/插件。用户认可既有 Demo 不等于代替其对真实有数据页面的视觉签收；合并仍等待 Mog 明确确认 PR 的具体 head。

最终收尾四个代码/配置文件经独立只读增量复审通过；root 重算聚合 SHA-256 与 reviewer 一致：`5e0281b6bc19702d5c8bd91c2fc50395b447ffff6ef39c01226c201160de76f9`。格式 diff 与已下载 937 字节 patch 一致，局部按钮修复不改变原视觉 Token，最终提交必须再通过完整 CI；该次精确 head 与结果记录在 PR #400。
