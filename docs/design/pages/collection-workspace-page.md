# PAGE-COLLECTION-001 · Collection Workspace 本地页面族

> 状态: 权威当前
> 最后核对: 2026-09-07
> 适用范围: `http://localhost:3000/collection/*` 的五个子面
> 事实来源: Mog 于 2026-08-26 的直接指定与逐项裁定、`REF-V4-001`、LIDS、领域不变量、当前 SCOPE 与真实 Rust 实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；V4 为本页族受限的视觉与骨架 Gold Master

## 1. 身份与授权

- 页面规格 ID: `PAGE-COLLECTION-001`
- 关联事项: `DESIGN-005`、`OBSERVATION-TARGET-DOSSIER-UI-001` / Issue #158
- 当前状态: 观察目标首层已收口为 creator / keyword 两套宽表语义；creator 对象工作区只保留「概览｜档案｜巡查」。共享运行时、真实数据密度和 Mog 业务验收仍须分层证明。
- LIDS 视觉强度: L1 Operations，Targets 抽屉为受限 L2
- LIDS 主 Pattern: Collection Control；抽屉复用同一工作面，不建立第二首页
- 用户任务: 找到我正在观察的创作者，判断档案建到什么程度、巡查是否有效、最近发生了什么，并通过作品生命周期分布进入唯一 Corpus 页研读。
- 三秒答案: creator 宽表单行同时回答对象、档案、目录/详情、巡查、最近变化、上次/下次巡查与下一步。
- 五秒主动作: 建立档案 / 建立标准目录 / 继续完善，或打开宽幅档案工作区看真实建档状态、最近变化、散点分布和具体缺口。
- 明确非目标: Evidence 结果复制、监控价值/机会评分/趋势预测、真实平台访问、插件改动、新事实表或 migration、Agent、部署。
- 当前 Package 支持合同: 验收基线为 **1440 CSS px 桌面全屏**；1280/390 不属于本 Package 验收。“不在手机或小于 13 寸屏幕运行”只描述使用场景，不定义 CSS breakpoint。

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
| `DIRECTORY_LINKED` | 作品从该目标创作者主页直接发现，但详情作者尚未确认 | 灰色空心点 | 已完成详情确认 |
| `AUTHOR_CONFIRMED` | 详情作者 ID 与目标精确一致 | 黑色实心点 | 网红价值或作品质量判定 |
| `INSUFFICIENT_OBSERVATION` | 作者、时间或指标资格不足 | 观察不足以成图 | 没有作品或表现为 0 |
| `NOT_APPLICABLE` | keyword target | 创作者生命周期不适用 | 空曲线或 keyword 表现为 0 |
| `READ_UNAVAILABLE` | 生命周期查询失败 | 当前读取状态未知 | 目标不存在或作品为空 |
| `QUERY_INVALID` | `life_window` 或 `life_metric` 不是闭集值 | 地址查询无效，需选择受支持口径 | 已按 90 日/点赞读取成功 |
| `SCAN_LIMITED` | 扫描命中 2000 + 1 探针 | 当前历史可能不完整 | 已扫描目标全部历史 |
| `TARGET_NOT_FOUND` | 抽屉标识无法解析 | 这个标识没有对应的观察目标 | 该对象在平台上不存在；它曾被删除 |
| `UNKNOWN` | 所有读数 | 尚未获得或无法验证 | 数值为零；运行正常；运行失败 |

本页族只在字段为 `KNOWN` 时显示真实 `0` 或互动数。`UNKNOWN` 不画为 0；明确作者冲突、发布时间不合格、当前指标未知均不入图，但必须进入档案缺口。不再对用户暴露复合指标、创作者内分位、滚动中位线或算法版本。

## 4. 已批准的设计组合

- 全局页头由 `apps/api/src/local_web/shell.rs` 统一生成，Corpus 与 Collection 共用同一实现；`shell.css` 承载页头、上下文行与 216px 导轨，两页共用。
- 页面局部样式在 `collection_workspace.css`；Issue #148 的生命周期与抽屉增量隔离在 `target_drawer.css`，只消费 `--lgi-*`，不写字面色值或渐变。review remediation 在唯一 Token 真源新增共享 `--lgi-focus`，由主题文档级交互元素 selector 统一提供 2px/2px focus；它同时覆盖 `.v7-app` 与作为 sibling 的固定 drawer，不使用 `!important`。后加载页面仍保留局部 focus 声明；已验证表面的 computed focus 正确，Mog 接受该声明为本包外技术债，原 reviewer FAIL 不改写为 PASS。
- 深色实时观察流是全产品唯一的深色面，色值以 `--lgi-stream-*` 十项 token 进入唯一色值源（当时 117 → 127 项；当前真源随后演进为 132 项）。
- 目标工作区与生命周期全部由服务端和 URL 状态渲染。散点固定点大小；x 轴是合格发布时间，y 轴压缩高低互动量的视觉差距，悬浮/选中始终显示原始数值。默认全部周期 + 点赞，可切近 90 天以及点赞/评论/收藏/转发。

### 页面级例外 `DESIGN-005-UI-EX-01`

Mog 于 2026-08-26 确认保留 V4 的深色终端配色。它与 `system.md` §4.10「不使用黑底荧光绿终端」直接冲突，作为长期例外记录：仅限运行态 / NOW 右栏，禁止扩散到任何其它界面；功能文字提到 11px 下限（V4 原型大量使用 7–9px，低于 DESIGN-003 已确认的可读下限）。

## 5. 交互与真实后果

| 用户动作 | 当前状态 | 真实后果 |
|---|---|---|
| 在五个子面之间导航 | 可用 | 仅页面跳转 |
| 切换运行态三模式 | 可用 | 仅页面跳转 |
| 用地址打开目标工作区 | 可用 | creator 仅「概览｜档案｜巡查」；keyword 仅「概览｜巡查」 |
| 点「建立档案 / 建立标准目录 / 继续完善」 | 受控写入 | 通过 Request → Authorization → Admission → Work Order → Lease 发起 200 篇上限的目录扫描；只有证明达到 200 或主页表面末端的当前根能作为标准目录。旧根不合格时保留历史并建立标准目录；有效授权上限低于 200 时明确拒绝，不静默降级 |
| 渐进建档 worker tick | 受控写入 | 只对有版本 marker 的目标，每批 3 篇，使用原 deep-archive 授权与 purpose，任务严格冻结具体 Work；已成功且完整的巡查发现并入当前目录和详情分母 |
| 切换生命周期 `all/recent_90_days` 与四指标 | creator 概览可用 | target-scoped bounded read；不访问平台 |
| 选择散点并进入语料 | 可用 | `life_work` 仅为 UI 状态；Corpus 精确读取该 Work，不复制 Evidence；既有 390px 诊断证明直接 URL/刷新可打开 Inspector 且不新增 history，但窄屏不属于当前验收合同 |
| Escape 或关闭抽屉 | 可用 | 保留列表 filter 与既有 `sort=last` 上下文；焦点返回原 target opener |
| creator 档案/巡查或 keyword 巡查 | 可用 | 不执行 lifecycle 大查询；keyword 不渲染 creator 档案空壳 |

**自动恢复开关未实现是有意的**：它在 Gold Master 里是一个 toolbar 开关，但它意味着系统在无人确认的情况下自动重新访问平台。在采集授权模型存在之前，本页族不提供这个开关。

## 6. 验收与未证明边界

- Issue #158 自动检查覆盖 creator/keyword 独立列、默认无批量选择、状态驱动单一动作、成功巡查时间、stable Work 去重档案、live deep-archive Lease、授权 200 下限、版本 marker、渐进批次、两级关联、KNOWN 0/UNKNOWN、可访问散点与 Corpus Work 深链。
- 当前 creator 只保留三 Tab，不再渲染复合指标、5 点中位线、分位数、算法版本或扫描回执；keyword 只保留概览和巡查，不生成“不适用”的生命周期空壳。
- 视觉验收记录见 `ACC-OBSERVATION-TARGET-DOSSIER-UI-001`；Issue #148 的 `ACC-COLLECTION-LIFECYCLE-001` 只保留历史演进证据。
- 本次证明: branch 源码、隔离 PostgreSQL、API/HTML/CSS/JS seam 与 1440 CSS px 桌面全屏受控视口走查；不新增或验收 1280/390/手机适配。
- 已接受风险: 一次隔离 1280 测量中 Inspector 右缘约超出 viewport 49.83 CSS px，移出当前 Package；页面局部 focus 声明保留为技术债，已验证表面的 computed focus 仍正确。
- 本次未证明且未授权: `origin/main` 合并、本地 `:3000` runtime 刷新、shared DB/migration、外部部署、插件或真实平台访问；完整辅助技术组合及 Mog 业务验收。branch 通过不等于这些层已完成。
