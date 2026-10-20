# PAGE-COLLECTION-001 · Collection Workspace 本地页面族

> 状态: 权威当前
> 最后核对: 2026-10-20
> 适用范围: `http://localhost:3000/collection/*` 的两个日常页面与上下文管理入口
> 事实来源: Mog 于 2026-08-26 的直接指定与逐项裁定、`REF-V4-001`、LIDS、领域不变量、当前 SCOPE 与真实 Rust 实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；V4 为本页族受限的视觉与骨架 Gold Master

## COLLECTION-LIDS-UI-001 · 当前三页呈现增量

Mog于2026-10-20直接派定观察目标、采集任务与工位管理UI更新，按已确认Q01–Q12和#457的设计基础实施。旧V4的固定1040px页面宽、工具栏内新建表单、多行任务混合字段、重看板几何由本节局部替代；以下产品、数据、权限、命令、材料和历史验收边界继续有效。

目标P08/B与任务P09/B使用紧凑工具/结果/主体，工位P11/C工具常驻。目标creator/keyword列语义及全部抽屉保留，字段宽度不足用表内横滚；任务按ledger-layout.json的L1/44px/名称144px，17字段、四核心固定、显隐与窄屏横滚保持，一任务一行。工位管理入口仍在Z02右侧、领域管理仍在领域下拉，不自行增加日常导航页。新建/批量/对象/表单动作分别归其对象；停止先确认后果，工位恢复接收不等于继续停止任务。

当前React样式来源为`apps/web/src/pages/collection/CollectionFrame.module.css`、目标工具/任务账本/任务页/管理CSS Modules，并消费唯一运行Token。下文`collection_workspace.css`等原Rust路径只保留历史沿革，不是本次新增实现位置。目标门户抽屉的既有结构保持，由当前全局字体/Token继承；未宣称其所有历史CSS均已清偿。

变更依据与表面/状态/依赖/验收见[本包清单](../changes/collection-lids-ui-001-ui-change-manifest.md)，实际结果见[本包验收](../acceptance/collection-lids-ui-001-acceptance.md)。1280/820/390只是本次前端布局验证，不改旧Package业务签收，不宣布Mog验收、真实采集、main或3000已更新。

## 1. 身份与授权

- 页面规格 ID: `PAGE-COLLECTION-001`
- 关联事项: `DESIGN-005`、`OBSERVATION-TARGET-DOSSIER-UI-001` / Issue #158
- 当前状态: 观察目标首层已收口为 creator / keyword 两套宽表语义；creator 对象工作区只保留「概览｜作品｜巡查」，keyword 只呈现其搜索命中与巡查。共享运行时、真实数据密度和 Mog 业务验收仍须分层证明。
- LIDS 视觉强度: L1 Operations，Targets 抽屉为受限 L2
- LIDS 主 Pattern: Collection Control；抽屉复用同一工作面，不建立第二首页
- 用户任务: 找到我正在观察的对象，判断目录或命中掌握多少、详情是否补齐、巡查是否有效、最近发生了什么，并从逐篇可查证列表进入唯一 Corpus 页研读。
- 三秒答案: creator 宽表单行同时回答对象、档案、目录/详情、巡查、最近变化、上次/下次巡查与下一步。
- 五秒主动作: 管理领域和 Target 用途 / 建立档案 / 查看任务 / 查看档案 / 补采缺口 / 处理异常；不把“重建目录”“继续完善”“查看进度”作为日常产品概念。
- 明确非目标: Evidence 结果复制、监控价值/机会评分/趋势预测、真实平台访问、插件改动、新事实表或 migration、Agent、部署。
- 当前 Package 支持合同: 验收基线为 **1440 CSS px 桌面全屏**；1280/390 不属于本 Package 验收。“不在手机或小于 13 寸屏幕运行”只描述使用场景，不定义 CSS breakpoint。

## 2. 信息架构（2026-10-12 用户确认）

| 序 | 路由 | 中文名 | 唯一主问题 |
|---|---|---|---|
| 01 | `/collection/targets` | 观察目标 | 长期观察谁 / 什么，目录、详情与巡查如何？ |
| 02 | `/collection/tasks` | 采集任务 | 本次执行到了哪一步，获得什么、缺少什么？ |
| 领域下拉 | `/collection/domains` | 管理领域 | 正式研究哪些领域、各自观察什么、是否接受新工作？ |
| 面包屑右侧 | `/collection/runtime` | 工位管理 | 执行资源、账号、安装、队列与回执是否健康？ |

日常侧栏仅显示前两项，`/collection` 默认进入观察目标。原 `/collection/attention`、`/collection/operations` 不再渲染独立页面，旧地址转到采集任务；底层历史事实保留。

本次确认取代旧六子页顺序。管理面整体复用原页面与全部能力，保留安全返回来源页/领域的路径，不压缩成 Demo 子集。目标两张宽表、顶部筛选和全部抽屉内容、任务账本及对账完整保留。通用「单次采集」只放任务页，目标上下文仍可人工观察一次。实施/证明边界见 [COLLECTION-JTBD-001](../../plans/active/collection-jtbd-001.md)。

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
| `DOMAIN_UNASSIGNED` | 候选 Observation Target 尚未关联任何 Domain | 目标身份已保存；需先由人把它关联到一个或多个 active Domain 并分别选择 role | 已有材料用途；可以直接建立档案；系统替人决定研究领域 |
| `UNKNOWN` | 所有读数 | 尚未获得或无法验证 | 数值为零；运行正常；运行失败 |

本页族只在字段为 `KNOWN` 时显示真实 `0` 或互动数。`UNKNOWN` 不画为 0；明确作者冲突、发布时间不合格、当前指标未知均不入图，但必须进入档案缺口。不向用户暴露复合指标、滚动中位线或算法版本；作品表现分布可显示当前纳入作品的 P25–P75 典型区间，但必须明确它不是行业常态、质量判断或预测。高讨论率（评论／点赞 `>20%`）仅是逐点复核外圈，点赞为 `0` 或任一分子／分母未知时写为未可判，不推断低讨论。

## 4. 已批准的设计组合

- 全局页头由 `apps/web/src/app/AppShell.tsx` 统一生成，Corpus 与 Collection 共用同一实现；`shell.css` 承载页头、上下文行与 216px 导轨，两页共用。
- 页面局部样式在 `collection_workspace.css`；生命周期与抽屉增量隔离在 `target_drawer.css`，只消费 `--lgi-*`，不写字面色值。趋势图允许唯一的 SVG 面积渐变：现有 `signal-soft → canvas` 只辅助读取同一条 Ink 中位线，不能承载第二指标、时间方向、状态或营销装饰；分布图不输出渐变。review remediation 在唯一 Token 真源新增共享 `--lgi-focus`，由主题文档级交互元素 selector 统一提供 2px/2px focus；它同时覆盖 `.v7-app` 与作为 sibling 的固定 drawer，不使用 `!important`。后加载页面仍保留局部 focus 声明；已验证表面的 computed focus 正确，Mog 接受该声明为本包外技术债，原 reviewer FAIL 不改写为 PASS。
- 深色实时观察流及其专用 token 为原生产流历史设计；该独立表面随 COLLECTION-JTBD-001 移除，本包不清理共享 token。
- 目标工作区由服务端和 URL 状态渲染；“作品”Tab 是目标范围内已接纳记录的可核验表，支持标题或作品 ID 筛选，不用主表数字拼装空表。

### 页面级例外 `DESIGN-005-UI-EX-01`

Mog 于 2026-08-26 确认的 V4 深色终端配色例外随原生产流表面退役，仅保留历史出处，不授权新页面使用。2026-10-12 的 context bar 工位管理入口由本次用户明确指定，其它共享 shell 不变。

## 5. 交互与真实后果

| 用户动作 | 当前状态 | 真实后果 |
|---|---|---|
| 在两个日常页面与上下文管理之间导航 | COLLECTION-JTBD-001 实施中 | 仅页面跳转；管理保留来源上下文 |
| 在任务页发起单次采集 | COLLECTION-JTBD-001 实施中 | 对已有目标复用人工观察一次；命令/工单回执不等于执行或采回 |
| 用地址打开目标工作区 | 可用 | creator 仅「概览｜作品｜巡查」；keyword 仅「概览｜作品｜巡查」，作品对关键词即命中结果 |
| 管理 Domain 与 Target 用途 | 受控写入 + 明确选择 | 同一 Target 可关联多个 Domain、每个 Domain 单独选择 primary/reference；只影响今后的准入，不重采作者、不复制 Evidence、不改写冻结用途 |
| 点「建立档案 / 补采缺口 / 处理异常」 | 受控写入 | 通过 Request → Authorization → Admission → Work Order → Lease 发起不超过 200 条的主页链接目录；达到主页末端或 200 才是合格初始边界。旧事实保留且标明边界未知，不冒充平台总数 |
| 渐进建档 worker tick | 受控写入 | 只对有版本 marker 的目标，每批 3 篇，使用原 deep-archive 授权与 purpose，任务严格冻结具体 Work；已成功且完整的巡查发现并入当前目录和详情分母 |
| 切换作品表现的 `trend/distribution`、趋势 `month/week`、生命周期 `all/recent_90_days` 与四指标 | creator 作品表现可用 | target-scoped bounded read；图表／粒度仅为 URL-owned 阅读状态，不访问平台 |
| 选择散点并进入语料 | 可用 | `life_work` 仅为 UI 状态；Corpus 精确读取该 Work，不复制 Evidence；既有 390px 诊断证明直接 URL/刷新可打开 Inspector 且不新增 history，但窄屏不属于当前验收合同 |
| Escape 或关闭抽屉 | 可用 | 保留列表 filter 与既有 `sort=last` 上下文；焦点返回原 target opener |
| creator 档案/巡查或 keyword 巡查 | 可用 | 不执行 lifecycle 大查询；keyword 不渲染 creator 档案空壳 |
| 停止 / 恢复观察 | 受控写入 | 只切换今后的自动巡查；历史材料与已在途工作不被删除，停止后行内提供恢复。 |
| 彻底删除观察目标 | 受控写入 + 确认 | 先读取真实预览并要求输入确认名；删除目标控制面（含详情会话、授权尝试、lane preparation、执行资格）及只对该目标目录生效的人工失效结论，并逐项披露数量。任何仍被有效 Domain 用途或安装级明确风险页信号引用的控制关系都必须安全处理；作品、详情、评论、Package 和 Receipt 始终保留。 |

**自动恢复开关未实现是有意的**：它在 Gold Master 里是一个 toolbar 开关，但它意味着系统在无人确认的情况下自动重新访问平台。在采集授权模型存在之前，本页族不提供这个开关。

## 6. 验收与未证明边界

- Issue #158 自动检查覆盖 creator/keyword 独立列、默认无批量选择、状态驱动单一动作、成功巡查时间、stable Work 去重档案、live deep-archive Lease、授权 200 下限、版本 marker、渐进批次、两级关联、KNOWN 0/UNKNOWN、可访问散点与 Corpus Work 深链。
- 当前 creator 保留「概览｜作品｜巡查」三 Tab；作品内的“表现”在同一画布切换时间桶趋势与逐篇分布，并保留精确作品链接、Known zero／Unknown 边界和当前纳入作品的 P25–P75 阅读辅助。它不渲染复合指标、滚动中位线、算法版本或扫描工程回执；keyword 同样提供命中作品表，不生成“不适用”的创作者档案空壳。
- 视觉验收记录见 `ACC-OBSERVATION-TARGET-DOSSIER-UI-001`；Issue #148 的 `ACC-COLLECTION-LIFECYCLE-001` 只保留历史演进证据。
- 本次证明: branch 源码、隔离 PostgreSQL、API/HTML/CSS/JS seam 与 1440 CSS px 桌面全屏受控视口走查；不新增或验收 1280/390/手机适配。
- 已接受风险: 一次隔离 1280 测量中 Inspector 右缘约超出 viewport 49.83 CSS px，移出当前 Package；页面局部 focus 声明保留为技术债，已验证表面的 computed focus 仍正确。
- 本次未证明且未授权: `origin/main` 合并、本地 `:3000` runtime 刷新、shared DB/migration、外部部署、插件或真实平台访问；完整辅助技术组合及 Mog 业务验收。branch 通过不等于这些层已完成。
