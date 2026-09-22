# PAGE-COLLECTION-001 · Collection Workspace 本地页面族

> 状态: 权威当前
> 最后核对: 2026-09-22
> 适用范围: `http://localhost:3000/collection/*` 的五个子面
> 事实来源: Mog 于 2026-08-26 的直接指定与逐项裁定、`REF-V4-001`、LIDS、领域不变量、当前 SCOPE 与真实 Rust 实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；V4 为本页族受限的视觉与骨架 Gold Master

## 1. 身份与授权

- 页面规格 ID: `PAGE-COLLECTION-001`
- 关联事项: `DESIGN-005`、`OBSERVATION-TARGET-DOSSIER-UI-001` / Issue #158
- 当前状态: 观察目标首层已收口为 creator / keyword 两套宽表语义；creator 对象工作区只保留「概览｜作品｜巡查」，keyword 只呈现其搜索命中与巡查。共享运行时、真实数据密度和 Mog 业务验收仍须分层证明。
- LIDS 视觉强度: L1 Operations，Targets 抽屉为受限 L2
- LIDS 主 Pattern: Collection Control；抽屉复用同一工作面，不建立第二首页
- 用户任务: 找到我正在观察的对象，判断目录或命中掌握多少、详情是否补齐、巡查是否有效、最近发生了什么，并从逐篇可查证列表进入唯一 Corpus 页研读。
- 三秒答案: creator 宽表单行同时回答对象、档案、目录/详情、巡查、最近变化、上次/下次巡查与下一步。
- 五秒主动作: 分配领域 / 建立档案 / 查看任务 / 查看档案 / 补采缺口 / 处理异常；不把“重建目录”“继续完善”“查看进度”作为日常产品概念。
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
| `DIRECTORY_LINKED` | 作品从该目标创作者主页直接发现，但详情作者尚未确认 | 作品目录已列入；在作品表可查归属，分布图不以归属样式冒充确认程度 | 已完成详情确认 |
| `AUTHOR_CONFIRMED` | 详情作者 ID 与目标精确一致 | 作者归属已确认；在作品表可查，不把墨点颜色当作质量或确认评分 | 网红价值或作品质量判定 |
| `INSUFFICIENT_OBSERVATION` | 作者、时间或指标资格不足 | 观察不足以成图 | 没有作品或表现为 0 |
| `NOT_APPLICABLE` | keyword target | 创作者目录不适用，显示关键词命中 | 空曲线或 keyword 表现为 0 |
| `READ_UNAVAILABLE` | 生命周期查询失败 | 当前读取状态未知 | 目标不存在或作品为空 |
| `QUERY_INVALID` | `life_window` 或 `life_metric` 不是闭集值 | 地址查询无效，需选择受支持口径 | 已按 90 日/点赞读取成功 |
| `SCAN_LIMITED` | 扫描命中 2000 + 1 探针 | 当前历史可能不完整 | 已扫描目标全部历史 |
| `TARGET_NOT_FOUND` | 抽屉标识无法解析 | 这个标识没有对应的观察目标 | 该对象在平台上不存在；它曾被删除 |
| `DOMAIN_UNASSIGNED` | 插件接纳的作者资料建立了目标，但 `domain_ref` 仍为空 | 目标身份已保存；要先由人决定材料归入本行业证据库还是跨行业参照语料 | 已归入本领域；可以直接建立档案；插件替人判断了领域 |
| `UNKNOWN` | 所有读数 | 尚未获得或无法验证 | 数值为零；运行正常；运行失败 |

本页族只在字段为 `KNOWN` 时显示真实 `0` 或互动数。`UNKNOWN` 不画为 0；明确作者冲突、发布时间不合格、当前指标未知均不入图，但必须进入档案缺口。不向用户暴露复合指标、滚动中位线或算法版本；作品表现分布可显示当前纳入作品的 P25–P75 典型区间，但必须明确它不是行业常态、质量判断或预测。高讨论率（评论／点赞 `>20%`）仅是逐点复核外圈，点赞为 `0` 或任一分子／分母未知时写为未可判，不推断低讨论。

## 4. 已批准的设计组合

- 全局页头由 `apps/api/src/local_web/shell.rs` 统一生成，Corpus 与 Collection 共用同一实现；`shell.css` 承载页头、上下文行与 216px 导轨，两页共用。
- 页面局部样式在 `collection_workspace.css`；生命周期与抽屉增量隔离在 `target_drawer.css`，只消费 `--lgi-*`，不写字面色值。趋势图允许唯一的 SVG 面积渐变：现有 `signal-soft → canvas` 只辅助读取同一条 Ink 中位线，不能承载第二指标、时间方向、状态或营销装饰；分布图不输出渐变。review remediation 在唯一 Token 真源新增共享 `--lgi-focus`，由主题文档级交互元素 selector 统一提供 2px/2px focus；它同时覆盖 `.v7-app` 与作为 sibling 的固定 drawer，不使用 `!important`。后加载页面仍保留局部 focus 声明；已验证表面的 computed focus 正确，Mog 接受该声明为本包外技术债，原 reviewer FAIL 不改写为 PASS。
- 深色实时观察流是全产品唯一的深色面，色值以 `--lgi-stream-*` 十项 token 进入唯一色值源（当时 117 → 127 项；当前真源随后演进为 132 项）。
- 目标工作区由服务端和 URL 状态渲染；“作品”Tab 是目标范围内已接纳记录的可核验表，支持标题或作品 ID 筛选，不用主表数字拼装空表。

### 页面级例外 `DESIGN-005-UI-EX-01`

Mog 于 2026-08-26 确认保留 V4 的深色终端配色。它与 `system.md` §4.10「不使用黑底荧光绿终端」直接冲突，作为长期例外记录：仅限运行态 / NOW 右栏，禁止扩散到任何其它界面；功能文字提到 11px 下限（V4 原型大量使用 7–9px，低于 DESIGN-003 已确认的可读下限）。

## 5. 交互与真实后果

| 用户动作 | 当前状态 | 真实后果 |
|---|---|---|
| 在五个子面之间导航 | 可用 | 仅页面跳转 |
| 切换运行态三模式 | 可用 | 仅页面跳转 |
| 用地址打开目标工作区 | 可用 | creator 仅「概览｜作品｜巡查」；keyword 仅「概览｜作品｜巡查」，作品对关键词即命中结果 |
| 为插件发现的候选目标分配领域 | 受控写入 + 明确选择 | 只给同一目标写入一个当前有效领域；重复选择同一领域安全返回，已有不同领域时拒绝覆盖。不重采作者、不复制 Evidence，也不把空领域回落成本领域 |
| 点「建立档案 / 补采缺口 / 处理异常」 | 受控写入 | 通过 Request → Authorization → Admission → Work Order → Lease 发起不超过 200 条的主页链接目录；达到主页末端或 200 才是合格初始边界。旧事实保留且标明边界未知，不冒充平台总数 |
| 渐进建档 worker tick | 受控写入 | 只对有版本 marker 的目标，每批 3 篇，使用原 deep-archive 授权与 purpose，任务严格冻结具体 Work；已成功且完整的巡查发现并入当前目录和详情分母 |
| 切换作品表现的 `trend/distribution`、趋势 `month/week`、生命周期 `all/recent_90_days` 与四指标 | creator 作品表现可用 | target-scoped bounded read；图表／粒度仅为 URL-owned 阅读状态，不访问平台 |
| 选择散点并进入语料 | 可用 | `life_work` 仅为 UI 状态；Corpus 精确读取该 Work，不复制 Evidence；既有 390px 诊断证明直接 URL/刷新可打开 Inspector 且不新增 history，但窄屏不属于当前验收合同 |
| Escape 或关闭抽屉 | 可用 | 保留列表 filter 与既有 `sort=last` 上下文；焦点返回原 target opener |
| creator 档案/巡查或 keyword 巡查 | 可用 | 不执行 lifecycle 大查询；keyword 不渲染 creator 档案空壳 |
| 停止 / 恢复观察 | 受控写入 | 只切换今后的自动巡查；历史材料与已在途工作不被删除，停止后行内提供恢复。 |
| 彻底删除观察目标 | 受控写入 + 确认 | 先读取真实预览并要求输入确认名；删除目标控制面（含详情会话、授权尝试、lane preparation、执行资格）及只对该目标目录生效的人工失效结论，并逐项披露数量。跨行业样本或安装级明确风险页信号存在时整体拒绝；作品、详情、评论、Package 和 Receipt 始终保留。 |

**自动恢复开关未实现是有意的**：它在 Gold Master 里是一个 toolbar 开关，但它意味着系统在无人确认的情况下自动重新访问平台。在采集授权模型存在之前，本页族不提供这个开关。

## 6. 验收与未证明边界

- Issue #158 自动检查覆盖 creator/keyword 独立列、默认无批量选择、状态驱动单一动作、成功巡查时间、stable Work 去重档案、live deep-archive Lease、授权 200 下限、版本 marker、渐进批次、两级关联、KNOWN 0/UNKNOWN、可访问散点与 Corpus Work 深链。
- 当前 creator 保留「概览｜作品｜巡查」三 Tab；作品内的“表现”在同一画布切换时间桶趋势与逐篇分布，并保留精确作品链接、Known zero／Unknown 边界和当前纳入作品的 P25–P75 阅读辅助。它不渲染复合指标、滚动中位线、算法版本或扫描工程回执；keyword 同样提供命中作品表，不生成“不适用”的创作者档案空壳。
- 视觉验收记录见 `ACC-OBSERVATION-TARGET-DOSSIER-UI-001`；Issue #148 的 `ACC-COLLECTION-LIFECYCLE-001` 只保留历史演进证据。
- 本次证明: branch 源码、隔离 PostgreSQL、API/HTML/CSS/JS seam 与 1440 CSS px 桌面全屏受控视口走查；不新增或验收 1280/390/手机适配。
- 已接受风险: 一次隔离 1280 测量中 Inspector 右缘约超出 viewport 49.83 CSS px，移出当前 Package；页面局部 focus 声明保留为技术债，已验证表面的 computed focus 仍正确。
- 本次未证明且未授权: `origin/main` 合并、本地 `:3000` runtime 刷新、shared DB/migration、外部部署、插件或真实平台访问；完整辅助技术组合及 Mog 业务验收。branch 通过不等于这些层已完成。
