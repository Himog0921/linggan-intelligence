# RUNTIME-STATION-TABLE-001 · 执行工位页以工位为单位重排

> 状态: 权威当前
> 最后核对: 2026-09-13
> 适用范围: `/collection/runtime`（执行工位）的页面结构、工位表、语言收敛与页头计数
> 事实来源: Mog 于 2026-09-13 的四项确认（11 列完整表 / 历史安装折叠且不计数 / 能力矩阵收进明细 / 底部读数压成一组）、[design-governance.md](../design-governance.md)、[../agents/ui-execution-contract.md](../../agents/ui-execution-contract.md)、[../lids/patterns.md](../lids/patterns.md) Collection Control、[../lids/language-policy.md](../lids/language-policy.md) LANG-05、[../lids/data-boundaries.md](../lids/data-boundaries.md)、[../lids/primitives.md](../lids/primitives.md)
> 冲突时以谁为准: 用户最新确认、真实数据合同与运行事实；本清单不改写任何准入判定、权限或账号资格口径

## 1. 事项

- Issue / SCOPE：Mog 于 2026-09-13 直接指派的执行工位页 UI 审查与重排（无 Issue 号）
- Agent 与 worktree：主工作树 `main`
- 目标：把「执行工位」从四层系统叙述改写为**以工位为单位的管理台**，并把用户读不懂的后端语言按 LANG-05 清掉
- 用户可见结果：
  1. 一张工位总表，一行一台机器，一眼读完「这台机器现在能不能干活、今天干了多少、要不要处理」；
  2. 页面上不再出现「准入第 5 问」「有界控制资格」「同一评估器」「租约」「lane 上限」「WorkOrder」这类内部术语；
  3. 页头「未归位安装」不再把历史插件残留算成待处理项。
- 明确非目标：
  - 不改 `establish_capacity` 的判定口径、不改账号资格语义、不改任何写动作的后端合同；
  - 不改观察目标页、生产流页、待处理页、采集任务页；
  - 不新增页面、导航项、字段、权限或采集动作；
  - 不删除任何已有真实动作（更名 / 停用 / 认领 / 登记 / 暂停接活 / 确认观察账号全部保留）。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / docs/current-state.md | 已读 | 确认执行工位是唯一允许出现工程执行细节的页面——允许出现，不等于允许用内部命名 | 是 |
| UI execution contract | 已读 | 本次为混合变更（展示 + 交互 + 状态语义），按最高风险类别收集依据 | 是 |
| 产品页面文档 `pages/collection-workspace-page.md` | 已读 | 执行工位子面的职责与事实边界 | 是 |
| `changes/design-009-*`、`changes/design-011-*` | 已读 | 前两轮把这一页定为「产能判定面」；本轮在其之上补「工位管理」这一层，不推翻判定 | 是 |
| LIDS patterns（Collection Control）、primitives、tokens、data-boundaries、language-policy | 已读 | 采集运行域主 Pattern = Collection Control（L1），L1 表格为主体；LANG-05 三类白名单；四态渲染契约 | 是 |
| 数据合同 / 真实代码 | 已读 | `StationOverview`、`StationCapability`、`RuntimeCapacityOverview`、`RuntimeResourceView`、`RuntimeLaneControlView` 的实际字段与可空性 | 是 |
| 运行页实测 | 已读 | 2026-09-13 17:5x 抓取 `/collection/runtime` 渲染产物，逐条核对可见文字 | 是 |

## 3. 变更分类

- 分类：**混合**（展示 + 交互 + 状态语义）
- 最高风险类别：状态语义
- 对应来源 ID：`LIDS-PAT-001`（Collection Control）、`LIDS-LANG-001` LANG-05、`LIDS-BOUND-001` 四态、`LIDS-PRI-001` 排版与按钮
- 为什么该类别足以覆盖本次风险：本次不新增事实来源，只改变同一批事实的**归位与命名**。每一处改名都对着数据合同核对过：合同字面取值（`capacity_available`、`account_unknown`、`UNKNOWN`）保留为 Mono 旁注，描述性标签一律删英文/删内部术语。
- 是否存在 DECISION_REQUIRED：**否**。四个产品决定已由 Mog 于 2026-09-13 当面拍板。
- L1 / L2 / L3 与主 Pattern：L1 · Collection Control
- 是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth：不新增 Token / CMP；复用 `.c-tg-table-head` / `.c-tg-item` 宽表栅格（`TARGET-INSPECTOR-PERFORMANCE-001` 已落地），只新增一个栅格变体 `.c-tg-station-grid`。不引入动效与场景。

## 4. 影响边界

- 受影响的页面：仅 `/collection/runtime`
- 受影响的组件：
  - `station_view.rs` 全页工作区（判断区 / 工位表 / 运行边界 / 排程 / 登记与认领）
  - `collection_control_surface_view.rs` 的 `render_runtime_control`：工位级事实（接活开关、安装、凭据、观察账号、账号资格）从独立区块**移入工位表**；该模块在这一页只继续提供通道判定数据
  - `collection.rs` 上下文行「未归位安装」计数口径
  - `collection_workspace.css` / `target_drawer.css` 的采集域样式
- 受影响的状态：
  - 新增用户可读表达：在岗 / 空缺、自动接活 / 已暂停、账号未绑定 / 待确认 / 已确认、能力 N 就绪·N 未验证·N 不支持·读不到
  - 保留不可互换的四组区分：**读不到 ≠ 没有**、**未验证 ≠ 降级**、**执行失败 ≠ 能力缺陷**、**从未问过活 ≠ 问了但没活**
- 是否影响数据口径、权限、敏感展示或真实行动：**不影响**。账号身份摘要仍不上报、凭据仍只显示有效性、所有 POST 目标地址与字段不变。
- 禁止修改的文件/能力：`crates/` 下的判定逻辑（仅允许修 `runtime_capacity.rs` 中 `deep_archive` 的**中文显示名**以消除同屏双命名）、任何 migration、任何写路径 handler
- 停止条件：若表格某一列在真实数据里找不到可靠来源，该列写「读不到」而不是留空或补默认值；若需要新的视觉规则，停工并升级。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 本机 3010 端口接真实库走查（2 台工位 / 7 条历史安装 / 5 条排程） | **VERIFIED**：一台工位一行，行内读得出状态、插件、最后报到、今日采集、接活、账号、能力、上次问活；展开面板含更名 / 停用 / 接活开关 / 认领窗口 / 账号明细 / 能力 9 格 / 上次问活处置。页头「未归位安装」由 7 变 0 | 只在 2 台工位的规模上走查；未在其它机器验证 |
| 状态诚实 | `station_view.rs` 36 条单元测试 | **VERIFIED**：四组不可互换区分（读不到≠没有、未验证≠降级、执行失败≠能力缺陷、从未问过活≠问了但没活）各有断言；`the_page_never_speaks_in_internal_engineering_names` 钉住 12 个内部术语；`a_lane_is_never_named_twice_on_the_same_page` 钉住双命名 | 降级、风险暂停、活跃许可三态用合成夹具验证，非真实故障复现 |
| 视觉一致 | `./scripts/check-project-governance.sh`、linggan-api 207/207、1440 / 1100 / 860 三档浏览器走查 | **VERIFIED**：治理检查通过；1440 与 1100 宽均无横向滚动；860 宽的外壳堆叠与已验收的观察目标页逐项一致（两页 sidebar 均 1040px、文档均不溢出），属外壳既有行为。控制台无错误 | 未做跨浏览器与高分屏验收；860 宽以下是外壳的既有欠账，不在本轮范围 |
| 真实后果 | 所有写动作的 action 地址与字段与改写前逐字一致，仅位置改变 | **NOT VERIFIED**：本轮未实际点击提交任何写动作（会改动 Mog 的真实工位记录）。已核对六个表单的 method / action / 隐藏字段名与改写前完全相同，失败态文案与 `error` 码映射未改 | 未实点更名 / 停用 / 暂停接活 / 认领 / 开关窗口；不验证平台侧行为——本页不访问任何平台 |

## 5b. 后续修订：状态列新增「失联」（PATROL-ALARM-RECOVERY-001，2026-09-13）

本清单交付当天，一次真实的插件失联暴露出工位表的一处表达缺口：页面首屏已经正确地说
「接不了活 · installation_stale」，而工位表的「状态」列仍写「在岗」——同一屏上两个互相
矛盾的答案，且让人放心的那个离眼睛更近。

- 「状态」列由两态（在岗 / 空缺）改为三态：**在岗 / 失联 / 空缺**；
- 判据是插件最后报到时间越过 `CONTROL_FRESHNESS_MINUTES`（20 分钟），**直接引用准入判定
  用的同一个常量**，页面不另写一个数；
- 读不到报到时间时两边都不编：仍显示「在岗」但用中性语气，由「最后报到」列说「尚未报到」；
- 「失联」不等于「坏了」，也不等于「空缺」——工位仍在册，只是此刻连不上，因此用警告色而
  不是危险色，且不影响更名 / 停用 / 认领等动作的可用性。

由 `a_station_that_stopped_reporting_is_not_still_called_on_duty`、
`the_page_uses_the_same_freshness_threshold_as_the_admission_check`、
`an_unreadable_heartbeat_is_not_rendered_as_either_answer` 三条断言自动执行；三条均已做
变异验证（把判定改回「有插件即在岗」确认转红）。

## 6. 交接

- 修改文件：`apps/api/src/local_web/station_view.rs`（重写）、`collection_control_surface_view.rs`（删除 runtime 渲染）、`local_web.rs`（handler 与页头计数）、`collection_targets_view.rs`（时间辅助函数提升可见性）、`tests.rs`（版式治理前缀表）、`collection_workspace.css`、`target_drawer.css`、`crates/evidence/src/runtime_capacity.rs`（通道显示名）
- 验证命令/走查：`cargo test -p linggan-api --bin linggan-api`（207/207）、`./scripts/check-project-governance.sh`、3010 端口真实库浏览器走查
- 规则或索引同步：本文件登记进 `docs/design/README.md` 权威地图
- 例外与替代：本轮替代 `DESIGN-009` / `DESIGN-011` 中「工位以卡片块呈现」的局部实现，两份清单的判定口径部分继续有效
- LIDS migration log / 预览同步：LANG-05 在本页的欠账由本轮清偿，登记进 `lids/migration-log.md`
- PR / reviewer / integration owner：待 Mog 指定
