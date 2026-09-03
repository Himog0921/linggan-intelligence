# PAGE-COLLECTION-001 · Collection Workspace 本地页面族

> 状态: 权威当前
> 最后核对: 2026-09-04
> 适用范围: `http://localhost:3000/collection/*` 的五个子面
> 事实来源: Mog 于 2026-08-26 的直接指定与逐项裁定、`REF-V4-001`、LIDS、领域不变量、当前 SCOPE 与真实 Rust 实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；V4 为本页族受限的视觉与骨架 Gold Master

## 1. 身份与授权

- 页面规格 ID: `PAGE-COLLECTION-001`
- 关联事项: `DESIGN-005`（无 GitHub Issue，Mog 直接指定）
- 当前状态: 五个子面及 Observation Target、调度/工单/任务/尝试/包/回执与执行工位均已有真实读写 seam。Issue #148 新增 creator target-scoped 生命周期读模型与抽屉概览；共享运行时、真实数据密度和 Mog 业务验收仍须分层证明。
- LIDS 视觉强度: L1 Operations，Targets 抽屉为受限 L2
- LIDS 主 Pattern: Collection Control；抽屉复用同一工作面，不建立第二首页
- 用户任务: 判断「我关心的对象系统在看吗」「哪里断了」「这个创作者已接纳作品在真实发布时间轴上表现怎样」，并从选中作品准确进入唯一的 Corpus Evidence 面研读。
- 三秒答案: 目标列表回答观察对象；creator 抽屉默认概览回答已接纳作品的生命周期、当前可绘制范围和被排除原因。
- 五秒主动作: 选择时间窗或互动指标、选择作品点、跳到 `/corpus/evidence?work=<public-ref>`；页面选择不触发平台访问。
- 明确非目标: Evidence 结果复制、监控价值/机会评分/趋势预测、真实平台访问、插件改动、新事实表或 migration、Agent、部署。

## 2. 信息架构（顺序与名称冻结）

| 序 | 路由 | 中文名 | 唯一主问题 |
|---|---|---|---|
| 01 | `/collection/targets` | 观察目标 | 我们长期正在观察谁 / 什么？ |
| 02 | `/collection/operations` | 运行态 | 系统此刻怎样运行、刚刚观察到了什么？ |
| 03 | `/collection/attention` | 待处理 | 哪些事情真正需要人介入？ |
| 04 | `/collection/tasks` | 执行任务 | 具体有哪些执行任务、到哪一步？ |
| 05 | `/collection/runtime` | 执行运行时 | 执行工位、队列、租约与回执是否健康？ |

运行态内含 `?mode=now|trace|review` 三个模式，均为真实地址。

**已记录的产品保留意见**：`DESIGN-005` 的第一性审核认定，执行任务与执行运行时两个子面对本阶段唯一用户（产品负责人本人）没有决策价值——它们服务的是排查故障的场景。本次按 Gold Master 冻结的信息架构如实实现五个子面，该意见留待 Mog 决定是否重组，不由实现者自行合并。

## 3. 状态与诚实性

| 状态 ID | 触发 | 用户应理解什么 | 禁止暗示什么 |
|---|---|---|---|
| `READY` | creator 生命周期至少有一个合格点 | 当前窗口/指标可绘制 | 平台全量、趋势或价值 |
| `INSUFFICIENT_OBSERVATION` | 作者、时间或指标资格不足 | 观察不足以成图 | 没有作品或表现为 0 |
| `NOT_APPLICABLE` | keyword target | 创作者生命周期不适用 | 空曲线或 keyword 表现为 0 |
| `READ_UNAVAILABLE` | 生命周期查询失败 | 当前读取状态未知 | 目标不存在或作品为空 |
| `QUERY_INVALID` | `life_window` 或 `life_metric` 不是闭集值 | 地址查询无效，需选择受支持口径 | 已按 90 日/点赞读取成功 |
| `SCAN_LIMITED` | 扫描命中 2000 + 1 探针 | 当前历史可能不完整 | 已扫描目标全部历史 |
| `TARGET_NOT_FOUND` | 抽屉标识无法解析 | 这个标识没有对应的观察目标 | 该对象在平台上不存在；它曾被删除 |
| `UNKNOWN` | 所有读数 | 尚未获得或无法验证 | 数值为零；运行正常；运行失败 |

本页族只在字段为 `KNOWN` 时显示真实 `0` 或互动数；创作者内分位只描述已纳入当前投影的作品，不是外部市场排名。Gold Master 示例数字仍不得进入真实路由。

## 4. 已批准的设计组合

- 全局页头由 `apps/api/src/local_web/shell.rs` 统一生成，Corpus 与 Collection 共用同一实现；`shell.css` 承载页头、上下文行与 216px 导轨，两页共用。
- 页面局部样式在 `collection_workspace.css`；Issue #148 的生命周期与抽屉增量隔离在 `target_drawer.css`，只消费 `--lgi-*`，不写字面色值或渐变。review remediation 在唯一 Token 真源新增共享 `--lgi-focus`，所有 LIDS 页面由同一全局 2px/2px focus 规则消费。
- 深色实时观察流是全产品唯一的深色面，色值以 `--lgi-stream-*` 十项 token 进入唯一色值源（当时 117 → 127 项；当前真源随后演进为 132 项）。
- 目标抽屉与生命周期全部由服务端和 URL 状态渲染。散点、对数 y 轴、创作者内分位与 `trailing-5-work-median-v1` 均由服务端投影提供或计算；前端不二次计算滚动中位线。

### 页面级例外 `DESIGN-005-UI-EX-01`

Mog 于 2026-08-26 确认保留 V4 的深色终端配色。它与 `system.md` §4.10「不使用黑底荧光绿终端」直接冲突，作为长期例外记录：仅限运行态 / NOW 右栏，禁止扩散到任何其它界面；功能文字提到 11px 下限（V4 原型大量使用 7–9px，低于 DESIGN-003 已确认的可读下限）。

## 5. 交互与真实后果

| 用户动作 | 当前状态 | 真实后果 |
|---|---|---|
| 在五个子面之间导航 | 可用 | 仅页面跳转 |
| 切换运行态三模式 | 可用 | 仅页面跳转 |
| 用地址打开目标抽屉、切四个职责 tab | 可用 | 仅本机读取与地址变化；Evidence tab 不存在 |
| 切换生命周期 `recent_90_days/all` 与五指标 | creator 概览可用 | target-scoped bounded read；不访问平台 |
| 选择散点并进入语料 | 可用 | `life_work` 仅为 UI 状态；Corpus 精确读取该 Work，不复制 Evidence |
| Escape 或关闭抽屉 | 可用 | 保留列表 filter 与既有 `sort=last` 上下文；焦点返回原 target opener |
| 基线/巡检/追踪或 keyword drawer | 可用 | 不执行 lifecycle 大查询；keyword 明示不适用 |

**自动恢复开关未实现是有意的**：它在 Gold Master 里是一个 toolbar 开关，但它意味着系统在无人确认的情况下自动重新访问平台。在采集授权模型存在之前，本页族不提供这个开关。

## 6. 验收与未证明边界

- Issue #148 自动检查覆盖稳定作者匹配、qualified `published_at`、field-wise latest KNOWN、KNOWN 0/UNKNOWN、上海日历 90 日边界、5 点中位线、scan receipt、API 闭集、四 tab、可访问 SVG、负向敏感字段与 Corpus 首批外 Work 深链。
- 视觉验收记录见 `ACC-COLLECTION-LIFECYCLE-001`；只登记实际完成的隔离视口与源码检查。
- 本次证明: branch 源码、隔离 PostgreSQL、API/HTML/CSS/JS seam 与受控视口走查（以验收记录最终结果为准）。
- 本次未证明: shared migration/runtime/deploy、真实平台访问、完整辅助技术组合、Mog 业务验收或 `origin/main` 合并。
