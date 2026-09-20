# RUNTIME-STATION-V7-2-001 · 执行工位页内容区按 v7.2 稿复刻 + 运行概览抽屉

> 状态: 权威当前
> 最后核对: 2026-09-20
> 适用范围: `/collection/runtime`（执行工位）页面的内容区结构、仪器面表达、按钮区与右侧运行概览抽屉
> 事实来源: Mog 于 2026-09-20 提供的 `linggan-execution-station-v7-2-runtime-drawer.html`、Mog 同日的两项范围裁定与一项数据裁定（见 §1）、[design-governance.md](../design-governance.md)、[../../agents/ui-execution-contract.md](../../agents/ui-execution-contract.md)、[../lids/patterns.md](../lids/patterns.md) Collection Control、[../lids/language-policy.md](../lids/language-policy.md) LANG-05、[../lids/data-boundaries.md](../lids/data-boundaries.md)、[../lids/materials.md](../lids/materials.md)、[../lids/decisions.md](../lids/decisions.md)
> 冲突时以谁为准: 用户最新确认、真实数据合同与运行事实；本清单不改写任何准入判定、权限、账号资格或工单口径

## 1. 事项

- Issue / SCOPE：[Issue #309](https://github.com/Himog0921/linggan-intelligence/issues/309)（Mog 于 2026-09-20 直接指派）
- Agent 与 worktree：`feat/runtime-station-v7-2-001` @ `.worktrees/runtime-station-v7-2-001`，基线 `origin/main@5371311`
- 目标：把 v7.2 设计稿的**内容区**复刻到线上执行工位页，并新增右侧「运行概览」抽屉

### 用户可见结果

1. 内容区顶部出现一条**控制条**：实时标记 + 工位控制 + 四个读数（接单状态 / 在岗工位 / 正在执行 / 自动排程）+ 三个按钮；
2. 「能不能接活」由一段散文改写为**接单仪器面**：深色仪器表头（阻塞数徽章 + 判定 + 通道原因码）+ 左侧逐条通道状态 + 右侧四个读数（在岗工位 / 今日预算 / 风险余量 / 观察账号）；
3. 工位表、今日运行、自动观察排程三块按 v7.2 的仪器语言重绘（区段徽章与序号、量化额度轨、能力行内签、读数格、排程状态列）；
4. 右侧新增**运行概览抽屉**：贴边标签页，点开从右侧滑入、Esc 关闭、窄屏默认收起；抽屉内四块内容全部来自真实读模型。

### Mog 于 2026-09-20 的三项裁定

| 裁定 | 内容 |
|---|---|
| 改动范围 | **只复刻内容区**。左侧导航与顶栏（共享外壳）保持现状，其它页面完全不受影响 |
| 英文标签 | **按现有规则只留中文**。稿中的 `WORKSTATION LIST`、`TODAY'S OPERATIONS`、`OBSERVATION SCHEDULE`、`RUNTIME OVERVIEW`、`LIVE`、`SUCCESS`、`FAIL`、`TOP 3 · 24H`、`ALL ONLINE` 等描述性英文一律不加（LIDS-LANG-05） |
| 抽屉内容 | 稿中三块内容在本系统**没有真实来源** → **换成真读得到的四块**，稿中数值一个都不出现 |

### 明确非目标

- 不改左侧导航与顶栏。二者由 `shell.rs` / `shell.css` 为所有页面渲染，ADR-10（shell 区域冻结）与 `patterns.md` 的所有权规则都要求页面样式表不得声明它们；测试 `no_page_stylesheet_restyles_a_component_the_shell_owns` 已在强制这一点；
- 不改观察目标页、采集任务页、待处理页、生产流页、证据库；
- 不新增任何写动作、字段、权限、数据口径或后端查询。所有既有 POST 地址、字段名与错误码逐字不变；
- 不引入 v7 token 值到运行时（ADR-P02）。本页只消费既有 `--lgi-*` 与页内已登记的 `--c-*` / `--v7-*` 别名；
- 不新增材料、场景、动效或深色主题（ADR-07）。深色仪器面只有两处：接单仪器表头与抽屉执行结果卡，且各自只使用已批准的深色面语法。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `CLAUDE.md` / `docs/current-state.md` | 已读 | 确认交付分级、worktree 唯一位置、提交前独立审核与「文档先于实现」四条硬要求 | 是 |
| UI execution contract | 已读 | 本次为**混合**变更（展示 + 交互 + 状态语义），按最高风险类别收集依据；实施前建立表面地图、状态词典、依赖地图与验收矩阵 | 是 |
| `docs/design/lids/README.md` | 已读 | 确认 LIDS 权威边界与「已采纳 ≠ 现有代码已这样」的两栏读法；确认「没有权威依据的部分必须停止并标记 DECISION_REQUIRED」 | 是 |
| `docs/design/lids/tokens.md` | 已读 | §2 v7 三层目标值与稿中调色板一致，但 §3 运行时基线仍是 `--lgi-*`；§4 迁移触发条件未满足 → 本次不新增 token、不改 token 值 | 是 |
| `docs/design/lids/decisions.md` | 已读 | ADR-02 字重三档、ADR-03 字号下限 11px、ADR-04 信号拆分、ADR-07 无暗色模式、ADR-09 量化淡出、ADR-10 shell 冻结、ADR-11 文本标签页、ADR-P02 运行时 token | 是 |
| `docs/design/lids/patterns.md` | 已读 | 采集运行域主 Pattern = `Collection Control`；shell 所有权规则；页头收回规则（无可见标题块、`h1` 仅 `.v7-sr-only`） | 是 |
| `docs/design/lids/materials.md` | 已读 | 材料预算 70/20/10；M-05 只用于局部深色面；一个容器最多一种材料；纹理不替代状态 | 是 |
| `docs/design/lids/language-policy.md` | 已读 | LANG-05 Mono 预算三类白名单：机器事实、封闭集合的系统状态枚举、结构编号；描述性标签禁止英文 | 是 |
| `changes/runtime-station-table-001-ui-change-manifest.md` | 已读 | 上一轮（2026-09-13）把这一页定为「以工位为单位的管理台」，本轮在其之上**只改表达、不改判定与结构语义** | 是 |
| `changes/design-011-collection-surfaces-v7-ui-change-manifest.md` | 已读 | 前一轮的 v7 落地范围与欠账（共享 shell 未动、ADR-04 信号拆分受阻） | 是 |
| 数据合同：`RuntimeCapacityOverview` / `StationOverview` / `StationCapability` / `UnclaimedInstallation` / `RuntimeResourceView` / `RuntimeLaneControlView` | 已读 | 逐字段核对抽屉四块内容的真实来源与可空性（见 §4 依赖地图） | 是 |
| 真实代码：`station_view.rs`（生产段 1–1525 行）、`collection_workspace.css`、`collection_workspace.js`、`shell.rs`、`lids_tokens.css` | 已读 | 现有 6 个渲染函数、40 条断言、可复用的 `--lgi-*` / `--c-*` / `--v7-*` 取值、shell 侧不得触碰的类名清单 | 是 |
| 运行页实测 | 已读 | 2026-09-20 抓取 `http://localhost:3000/collection/runtime` 渲染产物，逐条核对现有可见文字与结构 | 是 |

### 2b. 稿中数值的真实性核查（本清单的关键证据）

在写任何代码前，逐块核查了 v7.2 稿抽屉中四块内容在本系统是否有真实来源：

| 稿中内容 | 核查结论 | 证据 |
|---|---|---|
| 规则策略 3/3 正常 | **有真实来源** | `RuntimeLaneControlView` → `RuntimeControl.lanes`，三条通道各有中文名与可接活判定 |
| 回传成功率 98.6% / `+2.1` / 12 根柱 / `1,842 SUCCESS` / `26 FAIL` | **无此读模型** | 全仓检索 `回传`/`成功率`/`receipt_rate`/`success_rate` 无运行时读模型；仅 `StationCapability.successes` 与 `capability_failures`/`execution_failures` 存在，且口径是**近 7 天、按能力分**，不是 24 小时全局，也没有逐时序列可画柱 |
| 补采入口「失败任务列表 26 条」「手动触发补采」 | **运行页无此动作** | `apps/api/src/local_web.rs` 的 `/collection/runtime` 8 条 POST 全为本地登记类动作，无补采；`补采` 的真实入口在 `/collection/targets`（目标级），不在本页 |
| 失败原因 `插件无响应 62%` / `目标页面结构变化 24%` / `账号未绑定 14%` | **无聚合读模型** | 全仓 `failure_code` 只有三个取值（`page_read_failed`、`capability_not_executable_here`、`lease_expired`），且没有任何按原因汇总的查询或读模型 |

**结论**：稿中这三块数值在本系统中不存在对应事实。照搬即为伪造运行事实，直接违反 `data-boundaries.md` 与 `CLAUDE.md`「mock 与编译通过不能证明真实链路成功」。Mog 于 2026-09-20 裁定「换成真读得到的四块」，§4 的依赖地图给出替代来源。

## 3. 变更分类

- 分类：**混合**（展示 + 交互 + 状态语义）
- 最高风险类别：**状态语义**
- 对应来源 ID：`LIDS-PAT-001`（Collection Control）、`LIDS-LANG-001` LANG-05、`LIDS-MAT-001` 材料预算、`LIDS-BOUND-001` 四态、`LIDS-PRI-001` 排版与按钮、`LIDS-ADR-010` shell 冻结
- 为什么该类别足以覆盖本次风险：本次**不新增任何事实来源**，只改变同一批已有的真实事实的**表达与归位**。四块新内容全部来自本页已经在读的读模型（见 §4），没有新增 SQL、没有新增字段、没有新增写路径。三处易被"顺手改语义"的地方已单独设卡：
  1. **「接不了活」的判定口径不变**——仍由 `lane_rows` 决定，本轮只换它的外壳；
  2. **量化额度轨不替代数字**——轨只是 55/200 的量级读法，精确数字始终在它上方；
  3. **深色面只承载真实读数**——不出现没有数据来源的柱、趋势或百分比。
- 是否存在 DECISION_REQUIRED：**否**。三项方向性裁定已由 Mog 于 2026-09-20 当面拍板（§1）。
- L1 / L2 / L3 与主 Pattern：L1 · `Collection Control`（`Header / Active Workstations / Risk / Next Window` → 状态矩阵 → 队列与详情）
- 是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth：
  - **Token**：不新增、不改值。新增样式只消费既有 `--lgi-*` 与页内已登记的 `--c-*`（`DESIGN-010-UI-EX-01` 例外块）；
  - **Primitive / CMP**：复用既有按钮语法（`.c-btn-primary` / `.c-btn-quiet`）、宽表栅格（`.c-tg-*`）与区段语法，不新建组件规格；
  - **Scene**：不涉及；
  - **Motion**：只新增抽屉的开合位移（260ms）与标签页淡出，两者均受 `prefers-reduced-motion: reduce` 关闭；不使用循环动画，不新增扫描带动效；
  - **Data Truth**：不改变任何一轴。四态（读得到 / 读不到 / 没有 / 未知）的既有区分全部原样保留。

## 4. 影响边界

### 4a. 表面地图（改动落在哪些表面）

| 表面 | 现状 | 本轮变化 |
|---|---|---|
| 共享外壳（顶栏 / 上下文行 / 左侧导航） | 由 `shell.rs` + `shell.css` 拥有 | **不动**。页面样式表不得声明 `.v7-global-*` / `.v7-primary-nav` / `.v7-nav-*` / `.v7-context-*` / `.v7-side*` / `.v7-app` / `.v7-shell` |
| 内容区 · 控制条 | **不存在** | **新增**（`.c-deck`）：实时标记 + 名称 + 4 读数 + 3 按钮 |
| 内容区 · 接单判定 | `.c-verdict` 散文块 + 通道行 + 因子格 | 重绘为 `.c-instr` 仪器面：深色表头 + 左侧通道状态列 + 右侧 4 读数格。判定文案与通道数不变 |
| 内容区 · 工位表 | `.c-stn` + `<details>` 11 列宽表 | 重绘为区段语法（徽章 + `h2` + 右侧 meta）+ 行内量化额度轨 + 能力签。**可展开的管理面板与全部动作原样保留** |
| 内容区 · 今日运行 | `.c-run` + `dl.c-run-readouts` 6 格 | 重绘为 `.c-ops` 六格读数条（标题 + 编号 + 值 + 说明）。**去掉稿中的装饰性柱状图**（见 4d） |
| 内容区 · 自动观察排程 | `.c-run` + 6 列宽表 | 重绘为区段语法 + 9 列（新增「序」「状态」，最后列由稿中的 `⋮` 改为真实可点的入口） |
| 内容区 · 登记与未归位安装 | `.c-stn-footer` | 保留，仅套用新的区段语法与按钮语法 |
| 右侧 · 运行概览抽屉 | **不存在** | **新增**：贴边标签页 + 固定抽屉 + 4 个内容块 |
| 共享脚本 | `collection_workspace.js` | 新增抽屉开合（含 Esc 与窄屏默认收起）；不改动既有行为 |

### 4b. 状态词典（本轮出现或保留的用户可读状态）

| 状态词 | 来源 | 语气 | 不可与什么互换 |
|---|---|---|---|
| 能接活 / 部分接不了活 / 接不了活 / 读不到 | `lane_rows` 汇总 | 成功 / 警告 / 危险 / 中性 | 「读不到」不等于「接不了」 |
| 可接活 / 可排队，等工位 / 接不了 | 单条通道 | 成功 / 警告 / 危险 | 「可排队」既不是「可接活」也不是「坏了」 |
| 在岗 / 失联 / 空缺 | `station_state`，阈值直接引用 `CONTROL_FRESHNESS_MINUTES` | 成功 / 警告 / 中性 | 「失联」不等于「空缺」，也不等于「坏了」 |
| 自动 / 已暂停 / 读不到 | `accepting_cell` | 成功 / 警告 / 中性 | 「读不到」不写成「已暂停」 |
| 已确认 / 待确认 / 已变化 / 缺资格信号 / 未绑定 | `account_binding_label` | 成功 / 警告 / 警告 / 警告 / 中性 | 「未绑定」不是警告，是尚未指派 |
| N 就绪 · N 降级 · N 未验证 | `capability_counts` | 降级决定语气 | 「未验证」不等于「降级」 |
| 从未 / — | `moment_or_never` / `short_ref` | 中性 | 「从未」不是 0，也不是未知 |
| 已完成 / 执行中 / 等待领取 / 已取消 / 历史状态 / 还没排过 | `work_order_state_label` | 中性 | 封闭集合的系统状态枚举，原样保留 |

### 4c. 依赖地图（抽屉四块内容的真实来源）

| 抽屉内容块 | 真实来源字段 | 无数据时的表达 |
|---|---|---|
| **规则策略** | `RuntimeControl.lanes`（三通道中文名 + `available`/`queueable` + 原因码），读不到时退回 `RuntimeCapacityOverview.lanes` | 整块显示「读不到」，不写「正常运行」 |
| **执行结果（近 7 天）** | `StationCapability.successes` / `capability_failures` / `execution_failures` 跨台求和；最近一次拿回取 `PatrolOutlook.last_succeeded_at` | 分母为 0 时显示「尚无记录」，不显示 0% |
| **补采与失败入口** | 真实路由 `/collection/tasks`、`/collection/targets` | 不显示任何读不到的计数；两个入口是导航，不是动作 |
| **积压与重试** | `DispatchLaneBacklog` 的 `queued_work_orders` / `leased_work_orders` / `retry_cooling_work_orders`；`ActiveRiskPause` 列表 | 整块读不到时写整句「这一轮没有读到队列事实，因此答不出积压与重试的现状」，不写 0；队列读得到、但没有在等的单时写「没有在等的一单」。两种情形不合并 |

页脚：`PatrolOutlook.last_dispatched_at`（上次派出时间），空值显示「从未」。

### 4d. 对稿的有意偏离（逐条留痕）

| 稿中做法 | 本轮做法 | 理由 |
|---|---|---|
| 抽屉「回传成功率 98.6%」及 12 根柱状图 | 改为「执行结果 · 近 7 天」的真实成功与失败计数，**不画柱** | 无 24 小时全局读模型，也无逐时序列；柱状图会读作真实趋势 |
| 抽屉「补采入口」两个动作按钮 | 改为两个真实导航入口，**不显示「26 条待检查」** | 该动作与计数在本页都不存在；不造不存在的动作 |
| 抽屉「失败原因 TOP 3」 | 整块换成「积压与重试」 | 全库只有三个 `failure_code` 且无聚合读模型 |
| 控制条第 3 个按钮「查看观察轨迹」 | 改链真实存在的观察目标页 | 一级导航中没有「观察轨迹」这一项，原按钮在本系统没有落点，不能做成死链 |
| 今日运行六格下的装饰性柱状图 | **不做** | 六格读数没有任何一格带序列数据；无来源的柱违反了「纹理不替代状态」与「不得营造假运行感」 |
| 工位行末的 `⋮` 菜单 | 保留为可点开该行的「明细」（`<summary>`） | 该行的管理面板里是真实动作（更名 / 停用 / 接活开关 / 认领窗口 / 账号确认），不能替换成一个只弹提示的菜单 |
| 排程表末列 `⋮` | 改为链到观察目标页的真实入口 | 本页没有逐规则动作；链接指向真正能操作规则的那一页 |
| 深色「传感器」斜纹与点阵 | 只用 `--lgi-mosaic-on-dark` 已登记的深色面语言，覆盖率受限 | `materials.md`：M-05 只用于局部深色面，且不得成为页面主视觉 |
| 抽屉默认画成**打开** | **默认关闭**，关闭时带 `inert` | 稿子画的是"打开时是什么样"，不是一个默认状态。覆盖层默认打开会盖住正文，且屏幕外的链接会留在 Tab 序列里 |
| 抽屉面板的柔和光晕 `-18px 0 42px rgba(11,15,18,.12)` | 硬边阴影 `-4px 4px 0 var(--v7-black)` | 这套语言里所有浮起的面都是硬边的；柔和的半透明 rgba 光晕在 token 里没有对应值 |
| 把手 42px 高、`top:112px` | 34px 高、顶端贴固定头下沿 | 稿子的把手落在一片 388px 宽的页边空白上（`.control-deck` 只到 1484/1872）。这一页的正文铺满，42px 会压到控制条的动作按钮上——实测交叠约 70×13px |
| 把手自带 `-4px 4px 0 rgba(11,15,18,.18)` | 同款硬边阴影，用 `--v7-black` 实色 | 同上「硬边阴影」一条 |
| 控制条四个读数的值恒为墨色 | 「部分接不了活」用橙、「接不了活」用红，「读不到」保持墨色 | 四态区分比稿中的二值信号轨承载更多；只有确认为坏的两档上色，答不出不等于出了问题 |
| `@media(max-width:900px)` 把控制条折成一列、抽屉铺满 `100vw` | **不做** | 本页正文有 1040px 的最小宽度（`.c-page` 既有规则），再窄浏览器只会显示左侧一段。为不会出现的排版写折叠规则没有验收路径；把抽屉撑到 `100vw` 还会盖住左侧导航 |

### 4e. 其余边界

- **受影响的页面**：仅 `/collection/runtime`
- **受影响的组件**：`apps/api/src/local_web/station_view.rs`（生产段全部渲染函数）、`collection_workspace.css`（执行工位样式段）、`collection_workspace.js`（新增抽屉行为）
- **是否影响数据口径、权限、敏感展示或真实行动**：**不影响**。账号身份摘要仍不上报、凭据仍只显示有效性、8 条 POST 的 method / action / 隐藏字段名 / 错误码映射逐字不变
- **禁止修改的文件/能力**：
  - `crates/` 下的任何判定、读模型与 SQL；
  - `shell.rs` / `shell.css`（共享外壳）；
  - 任何 migration；
  - 任何其它页面的渲染函数与样式段
- **停止条件**：若某一块内容在真实数据里找不到可靠来源，该块显示「读不到」并**整块移除**，不得留空、补默认值或退缩成装饰；若需要新的视觉规则或新 token，停工并升级。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 见 §5b 实拍走查 | 见 §5b | 只在桌面宽度走查 |
| 状态诚实 | `cargo test -p linggan-api --bin linggan-api` 全量 | 见 §5b | 降级、风险暂停、读不到三态用合成夹具验证，非真实故障复现 |
| 视觉一致 | `./scripts/check-project-governance.sh` + 浏览器多档宽度走查 | 见 §5b | 未做跨浏览器与高分屏验收 |
| 真实后果 | 8 条 POST 的 method / action / 隐藏字段名 / 错误码映射与改写前逐字比对 | 见 §5b | 未实际点击提交任何写动作（会改动真实工位记录） |

### 5b. 实施后回填（2026-09-20）

**实施范围**（逐行对应 §4a）

- `station_view.rs`：内容区六个渲染函数改写 + 新增抽屉渲染。`render_runtime` 仍是唯一入口，路由 `/collection/runtime` 与 `local_web.rs` 的任何路由、表单字段、错误码映射均未触碰；
- `collection_workspace.css`：新增 `.c-deck*` / `.c-instr*` / `.c-sect*` / `.c-ops` / `.c-op` / `.c-usage*` / `.c-rdrawer*` 各段，删除被替代的 `.c-verdict*` / `.c-factors*` / `.c-lanes*` 段；
- `collection_workspace.js`：新增抽屉开合 IIFE。

**逐层验收**

| 层级 | 方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 浏览器实拍走查，1600 / 1230 / 1150 三档宽度 | 通过 | 只在本机 Chrome 1600×900 窗口；未跨浏览器、未做高分屏与真实触摸。本行与下面的几何实测由实施者在本会话内用脚本读出，**收尾审核的子代理无浏览器，未独立复现**，只做了算式核对 |
| 状态诚实 | `cargo test -p linggan-api --bin linggan-api` | **240 passed; 0 failed; 21 ignored**，与本轮改动前的基线逐字相同 | 降级 / 风险暂停 / 读不到三态由合成夹具构造，不是真实故障复现 |
| 视觉一致 | `./scripts/check-project-governance.sh` + `.c-runtime` 子树全量溢出扫描 | 见下 | 面板的 12 格额度轨是量级读法，不是精确刻度；跨 4 个以上工位的真实数据规模未走查 |
| 真实后果 | 8 条 POST 的 method / action / 隐藏字段名 / 错误码与改写前逐字比对 | `local_web.rs` 与 `station_view.rs` 的表单渲染段本轮无 diff，结构未变 | 未实际点击提交任何写动作（会改动真实工位记录） |

**抽屉开合契约（脚本实测，非目测）**

- 静止：`data-open` 无、`inert` 有、`body` 无类、两个触发器的 `aria-expanded="false"`；
- 点击把手：`data-open` 有、`inert` 无、`body.c-rdrawer-open` 有、两者 `aria-expanded="true"`，焦点移到关闭按钮；
- 点击关闭：以上全部复原，焦点回到把手。

**几何实测（1600×900）**

- 把手 112×34 @ (1488, 128)；控制条顶端 168 —— 把手完整落在它上方那条留白里；
- 「去观察目标页」按钮 (1462,185)–(1558,219) 与把手**零交叠**（修前交叠约 70×13px）；
- `.c-runtime` 子树内 `scrollWidth > clientWidth` 的元素：**0 个**。

**窄屏实测（1150×900：正文恰好到达它既有的 1040px 最小宽度）**

- 控制条仍是单行三列 `132px 548px 308px`；仪器面二列；四格读数二乘二；六格读数条六列；
- `.c-runtime` 子树内溢出元素：**0 个**。

**关于窄屏的一段既有行为（本轮不修，留证以免下次误判）**：视口窄于约 1230px 时，页面右侧会被共享外壳裁掉一段。来源是 `.c-page { min-width:calc(var(--c-cell) * 130) }`（=1040px）配 `.v7-app { overflow:hidden }`，两者都是既有规则（`git log -S` 指向 `9188cc8`），不是本轮引入，也不在本轮范围内（ADR-10 冻结共享外壳）。

**本轮修掉的两个缺陷，都出在本轮自己的新代码上**

1. **折叠规则写错了位置**。折叠规则写进了文件早段的 `@media` 块，而它们要覆盖的基础规则排在 300 行之后 —— 媒体查询不改变优先级，只按源码顺序决胜，于是那些规则一条都没生效，1120px 下控制条塌成三行。**定论后没有补写折叠**：实测基础排布在本页最小宽度（992px 内容宽）下零溢出，「塌成三行」完全是失效规则自己造成的。补一套折叠等于为不存在的排版加机制，所以最终处理是把这些规则**整块删除**。
2. **额度轨顶出格子**。`.c-usage-track` 的 `width:calc(var(--c-cell) * 11)` 是定值，装它的格子却随工位卡宽度缩；轨道顶出约 20px，被裁掉的正是读数右边那个数（上限）。加 `max-width:100%` 让轨道跟着格子收 —— 宽屏取值不变，窄屏从裁切改为收缩。

**未做**：未部署、未推送、未合并。`~/Library/Application Support/Linggan Intelligence/runtime-main` 上跑的仍是旧代码，本节描述的效果在本机 `:3000` 上**尚不可见**。

## 6. 交接

- **修改文件**：`apps/api/src/local_web/station_view.rs`、`collection_workspace.css`、`collection_workspace.js`（+ 本清单、进度记录、LIDS migration log）
- **验证命令/走查**：`cargo test -p linggan-api --bin linggan-api`、`./scripts/check-project-governance.sh`、本机浏览器走查
- **规则或索引同步**：本文件登记进 `docs/design/README.md` 权威地图；`lids/migration-log.md` 补一条本页 v7.2 落地记录
- **例外与替代**：本轮**不替代** `RUNTIME-STATION-TABLE-001` 的任何判定口径；那份清单的判定、四组不可互换区分与语言收敛全部继续有效，本轮只换表达层
- **LIDS migration log / 预览同步**：见上
- **PR / reviewer / integration owner**：待 Mog 指定；按仓库规则，收尾提交前跑一次 `commit-reviewer`，push 与部署由 Mog 决定
