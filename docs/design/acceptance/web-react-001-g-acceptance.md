# ACC-WEB-REACT-001-G · 采集整个页族迁移验收记录

> 状态: 一次性报告
> 最后核对: 2026-10-16
> 适用范围: WEB-REACT-001 G 四页、全部抽屉/弹窗、窄 JSON 接缝与旧实现退役
> 事实来源: Mog 本次直接派定、[阶段 G 分册](../../plans/active/web-react-001-g-collection.md)、实际 3000 基线、受控工作树与本机证明
> 冲突时以谁为准: Mog 当前决定、实际代码/运行/回执、DEC-0010；本报告不授予合并、部署、采集或模型调用权限

## 1. 范围与分层

交付基线为 main/runtime `baaf5a51b4a26f6ab54bb4b9eac38ce67b714520`，专属分支 `codex/web-react-001-g-collection`，关联 #430。整个采集模块作为一个包交付。没有改共享运行数据库、插件、调度器或真实平台/模型链路。Mog 最终视觉及业务签收单独记录。

| 表面 | React 落点 | 保留的能力与责任 |
|---|---|---|
| 外壳 | `CollectionFrame` / 原 `AppShell` | 两项日常导航、领域选择与管理、全局工位入口、返回领域、调度/目标状态、未知读数；写回执后重新读取共享状态 |
| 观察目标 | `targets/TargetsPage`、Toolbar/Table/Controls | creator/keyword 两套宽表，筛选/排序/搜索/选择/批量分组、新建、领域用途、暂停恢复、删除资格与确认 |
| 目标详情 | TargetDrawer/Overview/Works/Patrol/LifecyclePanel/Modals | 三个 tab、目录与材料回链、趋势/分布/周期/指标、巡查与榜单版本、规则编辑/停用/回执、建档/补采及异常；动作资格由原 Rust owner 决定 |
| 采集任务 | TasksPage/RunCreationDialog/RunDrawer | 领域/筛选/分页、临时关键词/已有目标/精确已选作品、进展/成果/诊断、暂停继续停止、同身份回执核实、草稿与未知状态 |
| 历史诊断 | ExecutionDiagnostics/LegacyTaskDiagnostics | 完整执行分页、最近 100 条旧时间线、五个 inspector tab、冻结范围与资源、Attempt/Package/Receipt 及交付分层 |
| 领域管理 | management/DomainsPage、DomainManagementDialog | 创建/编辑/暂停恢复、primary/reference 关联/改角色/解除、材料 lane 与未知统计、完整页和弹窗及来源返回 |
| 工位管理 | management/RuntimePage 与子组件 | 登记/改名/接活、窗口开关/认领、账号确认、安装与工位退役、资源/容量/队列/实际回执、来源工位焦点 |

旧 Collection Rust HTML、DOM 脚本、旧 CSS 资源和原表单 HTTP routes 同包退役；保留原业务读取/命令 owner 与非 DOM 合同。`/collection` 默认进入观察目标页；按主计划 D4 删除 Operations/Attention 历史入口，没有第二套实现。

## 2. 请求与副作用边界

- GET 不改变授权、建档、采集、规则、工位或模型状态。所有正常写操作仍由人明确触发。
- Run 的创建/控制沿用既有持久请求身份、版本与数据库回执；新增控制回执 GET 必须同时匹配 run/domain/command。
- 原来没有持久请求回执的管理/目标命令使用有上限的进程 epoch 与同身份缓存：消费身份不淘汰，重启后的旧身份保持未确认，不能据空缓存重放。它是传输核实，不能充当采集成功证明；未新增 schema/migration。目标/管理的未确认意图只保留在同一 document 内存，覆盖路由卸载/重挂和关闭重开；整页刷新、浏览器重启恢复不在该证明内。Run 的持久回执与 sessionStorage 身份另行验证。
- 断网/非 JSON/回执身份或版本不符、数据库无法确认提交结果均保留未知；不可换身份自动重发。明确业务拒绝保留草稿。关闭重开管理弹窗仍恢复同一未确认意图。没有把 receipt、页面可见或 HTTP 200 当作成果完成。
- 真实 3000 与同数据 3107 候选仅允许同源 GET/HEAD。候选测试进程仅启动 router，四个池连接均断言 PostgreSQL read-only，HTTP 外层拒绝所有写入，未启动 main、worker、scheduler、migration 或 provider。

## 3. 视觉证据与样式

沿用原 LIDS Token 与 shell；旧 Collection 样式原值、原顺序移到唯一 `LegacyCollection.module.css`。Vite 只对该冻结文件保留全局类名，并以内容 SHA256 固定；其他 CSS Module 继续逐声明检查 Token。没有重画页面，也没有增加颜色/字号/间距规则。

真实截图/DOM/最小 JSON 快照只保存在工作树 `artifacts/private/web-react-001-g/`，不进入 Git 或 CI；合成证明可由 CI 保留。原 UI 的窄屏内部宽工作区限制保持，不宣称移动端重设计。实际页面四页、creator/keyword 三 tab、图表两视图、规则、单次表单、进展/成果、领域与工位各弹层按相同视口、数据、滚动和焦点对照；最终对照汇总如下；原始图和差分图均未遮盖或修改时间戳。各组重复包含整页/视口/滚动帧，不把图数说成独立业务状态数。

| 表面 | 覆盖 | 剩余实际差异 |
|---|---|---|
| Targets | 四视口 8 态主矩阵、额外 10 态与 6 弹层；245 对已目检，所有滚动末端一致 | asOf 真实时间推进、头像边缘、删除计数顿号 21 像素栅格微差；1024 时间戳小数位数不同带来原有 flex 微移 |
| Tasks | 12 组状态，39 对含完整滚动/整页，进展/成果与五个诊断 tab | 原始非零差分 0–0.0924%，集中于少量文字绘制；成果区及各滚动帧多处完全一致 |
| Domains / Runtime | 10 态 × 四视口、212 对原图（161 组不同像素对）、全部实际滚动末端 | 通道差 >16 的最大差异依视口为 0.2471% / 0.3861% / 0.4524% / 0.7419%，集中领域弹窗说明/关闭符号/分隔线约 1px；元素 bbox、文本、字体与正文一致，底层 paint 归因未证实 |

工位名称修复后，两真实短名的 label/cell/edit/manage 在四视口与 3000 bbox 逐项相同；仅原已截断的合成长名在四视口启用收缩，正常 click 打开管理弹窗。所有真实捕获为 0 写请求、0 站外请求、0 页面异常。没有宣称像素完全相同，也未据此替 Mog 完成审美签收。

当前真实库没有“未分配领域目标”和有资格“材料退役”的 creator，不能制造真实数据补图；这两项实际状态截图保留 NOT VERIFIED，由资格出现后的 Mog/业务验收补齐。

## 4. 自动证明与审核

| 检查 | 最终本机结果 | 证明范围与限制 |
|---|---|---|
| Web verify | 37 文件 / 390 测试通过，类型、ESLint、production build 通过 | 四页及组件、错误/未知、重挂身份、Scope、Token；不等于 Mog 签收 |
| API 默认测试 | 170 通过 / 67 ignored；另一个测试目标 5 通过 | 默认 ignored 不计通过；显式隔离项另列；编译有既有及测试 helper 的 unused warnings |
| 隔离核心 PostgreSQL | collection single run 合同 14/14 通过 | 固定范围、部分交付、版本/幂等、停止/在途结算；未修改其业务 owner |
| 保留的历史动作与读取合同 | 5/5 通过 | 3 条原动作改走正式 JSON；领域 lane 实际回执、同一租约展开两个任务的冻结读取保持原合同 |
| 正式 Router + PostgreSQL + React | 整包通过 | Tasks/Corpus 跨页精确 51 篇选择、27 条列表筛选、真实已提交后丢响应再 GET；Targets 11 项；管理 6 项合同 / 18 次 UI 写操作及拒绝/失联恢复 |
| 副作用边界 | 一次性库和卷清理核验通过 | 所有写入仅合成隔离库；未启动 worker、采集器或模型；真实 3000/3107 仅 GET/HEAD |
| 文件与设计治理 | 已通过，提交前再次运行 | 不替代业务或视觉验收 |

提交前按 `commit-reviewer` 清单进行了两条互不审核自身实现的交叉复核。指定 Sonnet CLI 因未登录返回 `Not logged in`，未进行该模型审核、未登录或切换账号；使用两名已获 Mog 许可的现有子代理执行只读复核，并如实保留该限制。正式 PR 的 current-head 独立审核仍待完成。

| 复核发现 | 修复与验证 |
|---|---|
| JSON false 被旧 presence wire 当作勾选 | 归一化时移除未勾选字段；unit + 隔离实际规则 GET 同时断言 automatic 与 follow-details 均 false |
| Targets 切页后丢失未知请求身份 | 按动作/对象/规则槽保留模块内存中的原意图、草稿和编号；重挂只 GET，跨对象独立；新增 3 个反例 |
| 关闭创建弹窗后迟到成功改当前 URL | 挂载保护；关闭后保留原身份供核实，迟到不导航；真实组件反例通过 |
| 管理明确拒绝被归为未知 | typed owner 拒绝使用 rejected；基础设施故障继续 unconfirmed；分类断言通过 |
| 完整滚动末端缺 34px、年月标签分隔符不同 | 保留原批量容器几何、月/周标签恢复原格式；补图对照 |
| 长工位名称遮住管理按钮 | 独立 CSS Module 仅在原布局已截断的名称上允许收缩；旧冻结 CSS 不变。实际短名保留原几何，合成长名 normal click 单独复验；旧窄屏的原有控件限制不推导成已改善 |

补充实拍修复：新建目标领域弹层恢复原 section id/576px 宽度、首个可用领域默认与批量输入初焦点；删除预览保持原初始焦点及首个 Tab 入弹层；领域弹窗移除全页专用 wrapper、恢复关闭按钮 selector、关闭背后的领域菜单。关键词概览恢复 creator 专属操作边界，近 90 天 wire 枚举转回原 URL 值，规则错误弹层重挂后焦点与原页一致。上述均未改业务 owner 或冻结 CSS。

## 5. 交付层

代码、本机自动证明及可达状态的视觉对照已完成，随 Draft PR 交付。main/runtime 仍为 `baaf5a51`；未合并、未部署、未刷新 3000、未执行真实采集或模型验收。云端 CI 与 Mog 最终视觉、业务签收分别记录，不能由本机绿色推导。
