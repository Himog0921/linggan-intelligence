# DESIGN-009 · 执行工位改为产能判定面 · UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: `/collection/*` 的共享运行状态、`/collection/runtime` 与 Corpus 一级导航
> 事实来源: Mog 于 2026-08-29 的直接指定与三项裁定、真实运行结果、`PAGE-COLLECTION-001`、LIDS、`linggan-contracts` 的 `Capacity` 合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定

## 0. 流程偏离（先说清楚）

治理协议要求「定义」先于「实施」。**本清单是在代码写完之后补的，顺序反了。**

原因不是遗忘：Mog 在 2026-08-29 明确指示「不用重写简报，你可以直接按照这个逻辑来设计，我来看实际的效果再给你反馈」。按来源优先级，用户最新确认高于本协议，因此实施先行成立；但清单本身仍是必需品，不能因为流程被压缩就不留。**这条偏离记在这里，不埋在别处。**

## 1. 事项

- 事项 ID: `DESIGN-009`。原始变更由 Mog 直接指定、当时没有 GitHub Issue；2026-08-31 的共享状态硬化续作已在重新打开并指派的 [Issue #94](https://github.com/Himog0921/linggan-intelligence/issues/94) 下实施。首轮 Claim 的候选文件范围不足，独立审查后已补充 [Claim amendment](https://github.com/Himog0921/linggan-intelligence/issues/94#issuecomment-5470459730)，逐项列明 exclusive/shared/forbidden 文件。
- 关联: `PAGE-COLLECTION-001`、`LIDS-SYS-001`、`LIDS-PAT-001`、`LIDS-PRI-001`、`DECISION-04`、`INV-36`
- 触发: Mog 要求规划 `/collection/runtime` 的 UX/UI。调研中发现的不是排版问题，而是**这一页在系统性地说假话**。

## 2. 读取回执

`AGENTS.md` → `docs/design/README.md` → `design-governance.md` → `lids/system.md` + `lids/patterns.md` → `pages/collection-workspace-page.md` → `acceptance/design-005-...md` → `docs/product/domain-invariants.md` → `docs/progress/2026-08.md`（COLLECTION-001 全段）→ 真实代码 `station_view.rs` / `collection.rs` / `shell.rs` / `acquisition_chain.rs` / `station_read.rs` / `collection_workspace.css` → **真实运行页面**（`curl localhost:3000`）与 **真实进程状态**（`launchctl list`）。

### 读取查出的核心事实

页面上五处接通状态是**写死的字符串**，写下时都成立，之后系统接通而字符串一个字没变：

| 页面写着 | 真实情况 | 证据 |
|---|---|---|
| 调度器未接通 | 常驻运行中 | `com.linggan-intelligence.patrol-worker` PID 33666 |
| 准入第 5 问尚未接上工位 | 2026-08-27 已接通，第一张工单已诞生 | 进度记录、`0009_work_order_station.sql` |
| 队列、租约、回执尚不存在 | 租约表已存在并在用 | `0010_work_order_lease.sql`、`complete_lease_for_task` |
| 暂无观察目标 | 2 个，其中 1 个开着巡检 | 真实读库 |
| 离线工位 UNKNOWN | 页面正下方同时写着这台工位「在岗」 | `collection.rs` 旧 `head_readout` |

**一个不会随系统状态改变的状态区块，等于一个永远不会响的警报器。** 而且这次说谎的方向是「低报」——把已经跑起来的系统说成没跑，比吹牛更难被发现，因为没人会去质疑一句谦虚的话。

## 3. 变更分类

按风险最高的一类处理：**状态/语义**。

| 类型 | 本次内容 | 依据 |
|---|---|---|
| 状态/语义 | 五处接通状态改为实读；上下文行两个 UNKNOWN 改为真实值 | Mog 2026-08-29 裁定「改成真实数（推荐）」；`Capacity` 合同 |
| 展示 | 页面重排为四层：能不能接活 → 缺哪一样 → 是谁 → 按什么边界跑 | Mog 裁定「每天扫一眼」+「重排」 |
| 交互 | 开发期工具收进默认折叠区 | 展示层降级，未改动任何后端动作 |
| 权限/行动 | **无。** 没有新增、修改或删除任何动作，没有任何新的平台访问 | — |

### 页面主问题的变更（需要 Mog 确认）

`PAGE-COLLECTION-001` §2 冻结的主问题是「执行工位、队列、租约与回执是否健康？」——那是**排障视角**，写于工位对象还不存在、第 5 问恒为 `false` 的时候。本次按 Mog 的裁定实现为「**系统现在有没有能力接活？缺哪一样？正在按什么边界跑？**」。

**页面规格尚未同步修改**：`PAGE-COLLECTION-001` 是「权威当前」的冻结文件，改它需要 Mog 确认。在确认之前，规格与实现之间存在一处已知的、有记录的分歧。

## 4. 影响边界

### 改了什么

| 文件 | 改动 |
|---|---|
| `crates/evidence/src/runtime_capacity.rs` | 新增。执行工位页的只读投影 |
| `crates/evidence/src/acquisition_chain.rs` | 新增 `read_capacity`：**页面与准入读同一个 `establish_capacity`** |
| `apps/api/src/local_web/station_view.rs` | 重写为四层结构 |
| `apps/api/src/local_web/collection.rs` | 新增 `SurfaceState`；读数、系统词、导轨底部改实读 |
| `apps/api/src/local_web/shell.rs` | `global_header` 新增 `collection_state` 入参；码表提为常量 |
| `apps/api/src/local_web.rs` | runtime 路由接三份读物 |
| `apps/api/src/local_web/collection_workspace.css` | 工位面板样式重写 |

### 明确的非目标

不新增功能、不碰数据库结构、不碰采集能力、不碰插件、不改任何后端动作、不引入调度心跳机制。

### 例外与偏离

- **`DESIGN-009-UI-EX-01`**：其余四个采集子面（`targets` / `operations` / `attention` / `tasks`）继续显示静态的「调度器未接通 · 暂无观察目标」。它们读不到这些事实，本次不为它们建读投影。**读不到时保留原话，绝不因为读不到就宣布已接通**——这是有意的保守，不是遗漏。复核触发条件：任一子面接入真实读投影时一并修正。

### 外壳纪律

`global_header` 采用增加入参的方式（`PrimarySurface` 之后的第二个参数化点），而不是让采集页在自己的样式表里覆盖外壳——符合 `LIDS-PAT-001`「外壳缺一个参数就在 `shell.rs` 增加入参」。页面样式表字面色值计数保持 **0**。

## 5. 验收与证明边界

见 `ACC-RUNTIME-001`。

**本次不证明**：调度进程是否活着（系统无心跳记录，页面如实说读不到）、真实采集能力、账号资格、1440/1536/1728/2560/移动视口、完整键盘可达性。

## 6. 交接

三项待 Mog 裁定，见 `ACC-RUNTIME-001` §5。

## 7. 2026-08-31 · 真实运行状态硬化与 `DESIGN-009-UI-EX-01` 关闭

Mog 已要求在同一交付中把观察目标、工位管理和语料展示地基做成可靠运行面。真实
`main@b8bf0f0` 运行时已经有 `collection_scheduler_heartbeat`，`/health` 可读
`scheduler.state=running`，观察目标页也实际读取到 2 个目标；因此 §4 当时保留的
`DESIGN-009-UI-EX-01` 已经满足复核触发条件。继续显示“调度器未接通 / 暂无观察目标”
不再是保守，而是与同一服务的真实读数冲突。

### 表面地图

| 表面 | 本次只修什么 | 不改什么 |
|---|---|---|
| `/collection/targets` | Header、上下文读数和导轨底部使用现有目标/调度事实 | 目标创建、筛选、巡检和建档动作 |
| `/collection/runtime` | 心跳状态与 `/health` 使用同一 `read_scheduler_heartbeat` | 产能判定、工位认领、租约与准入规则 |
| `/collection/attention`、`operations`、`tasks` | 共享 Header 不再显示过时的全局未接通状态；缺少业务读模型的正文统一明确为“当前未知” | 各子面尚未实现的业务列表与操作 |
| `/corpus/evidence` | 共享一级导航中的“采集”状态读取同一事实 | Work Resource、列表、Inspector、材料状态和隐私边界 |

### 状态词典

| 事实 | 用户表达 | 禁止替代 |
|---|---|---|
| heartbeat `running` | 调度运行中 | 心跳读不到、系统全部正常 |
| heartbeat `stale` | 调度心跳已过期 | 调度运行中、调度器未接通 |
| 无 heartbeat / 读取失败 | 调度心跳读不到 | 运行中、停止 |
| 目标/巡检/建档计数已读 | 显示真实计数 | UNKNOWN、0 的推测 |
| 数据库或对应读模型失败 | 未知 / 读不到 | 暂无、未接通、已接通 |

数据库整体不可用时，目标正文、导轨、生产流、执行工位、上下文状态、系统边界与一级导航全部使用“未知 / 读不到”；任何一个表面都不得单独退回历史静态词。

### 依赖地图

只复用 `count_targets`、`read_runtime_capacity`、`read_station_overview` 与
`read_scheduler_heartbeat`；四者均为当前 Rust/PostgreSQL 只读事实。共享 Header 继续由
`shell.rs` 唯一渲染，不新增 CSS、Token、组件或前端状态。任何一项读取失败只降级该项，
不能把其它已读事实一起降成“未接通”。

### 验收矩阵

| 验收 | 自动证据 | 运行证据 |
|---|---|---|
| running / stale / unreadable 不互相冒充 | `collection.rs` 聚焦单测 | `/health` 与三个页面 DOM 文案对账 |
| 2 个目标不再显示“暂无观察目标” | API 页面测试 | 本机 `/collection/targets` |
| 工位页与 health 心跳一致 | API 页面测试 | 本机 `/collection/runtime` + `/health` |
| 语料列表不受影响 | 既有 Work Resource / UI tests | Chrome 仍显示 12 个作品集合及逐 lane 状态 |

变更分类仍是**状态/语义**；不新增权限、动作、页面、数据字段、数据库 migration 或平台访问。
本节关闭 `DESIGN-009-UI-EX-01`，但不把待处理/生产流/采集任务的业务数据面写成已实现。
