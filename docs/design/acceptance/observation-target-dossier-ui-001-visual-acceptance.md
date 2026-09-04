# ACC-OBSERVATION-TARGET-DOSSIER-UI-001 · 观察目标档案桌面验收

> 状态: 一次性报告
> 验收状态: 集中整改已完成一次最终验证；不启动第二轮代码 review
> 最后核对: 2026-09-05
> 适用范围: Issue #158；`codex/observation-target-dossier-ui-001`，exact base `origin/main@bfe5d7e623b70e314cc6697269e291c3d5f5b3ef`
> 事实来源: 首版自动测试与隔离 PostgreSQL/1440 浏览器证明、exact head `fd6cb533e4fb5872d708bf0e73d3ec1298c1de90` 的唯一一次联合审查、集中整改后的 workspace/隔离 PostgreSQL/1440 浏览器最终证明
> 冲突时以谁为准: 用户最新确认、Issue #158 Claim、真实代码/数据库/浏览器结果、PAGE-COLLECTION-001 与 LIDS；本报告不授权 merge、共享运行或真实平台行动

## 1. 当前结论

exact head `fd6cb533e4fb5872d708bf0e73d3ec1298c1de90` 已建立结构、数据链和首次 1440 浏览器证明，但唯一一次 exact-head 联合审查拒绝了该版本，因为它仍存在下列产品/UX 失真：

- creator 抽屉的固定身份区没有承载完整身份、分层进度、巡查时间和唯一主动作；
- 概览正文在“最近变化”前插入了创作者简介；
- 目录 Coverage 与作者关联问题不足时，页面可能把已取得详情误说成档案已建好；
- 档案 Tab 没有交付真实“当前可分析”数量，并把页面名称放进统计值位置；
- 失败反馈、头像占位和 keyword 空态仍含工程语言；
- 生命周期中的作者归属确认被写成了详情本身已确认。

上述问题已按类别合并为唯一一次集中整改，并完成一次最终自动、隔离 PostgreSQL 与 1440 浏览器验证；没有再开启第二轮代码 review。整改源代码和合同层结论为 `VERIFIED`，但这不是对首次审查结果的改写，也不是独立复审通过声明。

## 2. 冻结的 1440 验收矩阵

| 表面 | 常用态必须看到 | 空/部分/失败/受限/处理中 | 验收方法 |
|---|---|---|---|
| Creator 首层 | 12 列：`编号｜创作者｜平台｜分组｜档案状态｜作品目录｜详情进度｜巡查状态｜最近变化｜上次巡查｜下次巡查｜操作` | 未建立、基线受限、详情部分、读取失败、初始目录在途、详情小批次在途必须分开 | DOM 表头顺序、普通格换行数 0、每行主动作数 1、文档/表格横向溢出 0 |
| Keyword 首层 | 10 列：`编号｜关键词｜平台｜分组｜巡查状态｜最近命中｜数据更新｜上次巡查｜下次巡查｜操作` | 无差分投影时显示“尚未取得”，不写 `0`；不出现 creator 档案列或散点 | DOM 表头顺序、横向溢出 0、每行主动作数 1，抽屉只有“概览｜巡查” |
| Creator 固定身份区 | 头像、昵称、平台/账号、分组/标签、简介、已取得的公开数据、目录/详情/可分析三层进度、巡查状态、上次/下次巡查、唯一主动作 | 头像/简介/公开数据未取得时使用业务空态；读取失败不把数量写成 `0` 或“—” | 1440 首屏实际文本/元素顺序、主动作数量、身份与列表状态一致性 |
| Creator 概览 | 正文顺序严格为“最近变化 → 作品生命周期 → 档案缺口” | 无差分读模型显示“尚未取得”；无可绘点说明是时间/指标/关联资格不足，不声称没有作品 | DOM 标题顺序、图容器尺寸、空态与有点态 |
| Creator 档案 | 只显示三项同口径实数：作品目录、已取得详情、当前可分析 | 读取不可用时三项均显示状态而不写数；关联/隔离问题用缺口文案单独呈现 | 三项读数的 API/HTML 一致性；不存在页面名称或材料名称冒充统计值 |
| Creator 巡查 | 启停状态、上次成功、下次时间、最近结果、真实规则入口 | 未开启、无成功历史、无差分结果均使用准确空态 | 巡查成功时间与生命周期最新外圈的同一投影身份 |
| 行动后果 | 首次“建立档案”只建立 `maximumQuota=200` 的 canonical root；“继续完善”每次只冻结≤3篇已知缺详情作品 | 基线读取失败时禁止写入；在途时只查看进度；基线受限时只查看原因；继续完善不重扫主页 | 隔离 PostgreSQL 断言根身份、任务数、冻结 Works、无重复主页任务与读失败零写入 |
| Corpus 边界 | 散点选中只以 stable Work public ref 进入 Corpus | 无目标级评论/OCR/ASR/媒体计数时不创造数字或假入口 | 精确 Work 深链；目标抽屉不复制正文、评论身份或媒体内容 |

## 3. 建档完成与单一动作口径

- “建立档案”扫描的是主页当前可见的作品链接，200 是上限，不是平台总量或必须凑满的目标。
- 只有 `maximum_quota=200` 的清洁 Coverage 已完成，或 producer 明确到达当前可见表面末端，初始基线才能标记为可用。
- 风险、时间、配额、读取或执行故障造成的提前停止属于“基线受限”，不能因当前已发现作品恰好都有详情就标记为档案已建好。
- 首次基线之后，“继续完善”只深化已知作品，复用 canonical root；旧授权仍有效时继续使用，旧授权到期后只接受新的同目的、同目标类型且上限不小于 200 的有效授权。一次不超过 3 篇，不再派发作者资料或主页目录扫描。
- 任何行在一个时刻只有一个主动作。“查看进度”“查看受限原因”“查看读取状态”都是只读导航，不会隐式触发建档、续采或巡查。

## 4. 生命周期图的数字与图点

- 作品目录：该 target 已接纳、未隔离、按 stable Work 去重的目录作品数。
- 已取得详情：上述作品中已有合格详情的 stable Work 数；这不等于作者归属已确认。
- 当前可分析：上述作品中对当前窗口和指标具备合格发布时间、关联资格和 KNOWN 指标的 stable Work 数。
- 空心点：直接从该 target 主页发现，详情作者尚未确认。
- 实心点：详情中的作者 ID 与 target 精确匹配；用户文案统一为“作者已确认”。
- 橙色外圈：该作品首次由当前“上次成功巡查”关联。成功时间和外圈使用同一 canonical latest-successful-patrol 投影；手工接纳不使用该外圈。
- 一个 stable Work 只有一个点；新巡查对同一作品的最新 KNOWN 互动观察更新原点，不增加重复点。
- 作者冲突、时间不合格或当前指标 UNKNOWN 的作品不入图；KNOWN `0` 正常入图。

## 5. 首版已有证明的保留边界

下列是 `fd6cb533e4fb5872d708bf0e73d3ec1298c1de90` 在审查前已取得的事实证明，只能作为整改后回归基线，不构成当前通过：

- 隔离 `:3318` API 与 synthetic PostgreSQL fixture 在 CSS viewport `1440×813`、DPR 1 下，creator 12 列、keyword 10 列的 `documentOverflow=0 / tableOverflow=0`，creator 普通格换行数 0、每行主动作 1。
- 抽屉宽 `1036.8px / 72%`、横向溢出 0，creator 三 Tab 与 keyword 两 Tab 存在。
- synthetic 三点投影含 2 个目录空心点、1 个作者确认实心点与 1 个最新巡查外圈；应用 console warning/error/exception 为 0。
- 首版 `cargo check/test`、Node syntax、UI handbook 与 diff 检查通过；隔离 PostgreSQL/Rust/API proof 共 78 项，Node controller proof 2 项。
- 上述 fixture 为 `SYNTHETIC / NOT LIVE`；隔离 tab、API、Chrome CDP profile、database、container 和 volume 已删除或停止。

## 6. 集中整改的最终验证结果

本节是对集中整改结果的一次性证明，不是第二轮代码 review。浏览器使用一次性 PostgreSQL 16 数据库、一次性 API `127.0.0.1:3318` 和 synthetic fixture，未连接共享数据库或真实平台。

- 自动检查：`cargo check --workspace --all-targets --locked`、`cargo test --workspace --all-targets --locked`、`cargo fmt --all -- --check`、Node 语法检查、UI handbook、governance 与 `git diff --check` 通过。编译只保留仓库既有的 3 条 Collection dead-code warning。
- 隔离 PostgreSQL：完整 `./scripts/test-local-001-discovery-postgres.sh` 通过 82 项 Rust/PostgreSQL/API proof 与 2 项 Node controller proof；含本 Package 的 observation dossier `8/8`、creator lifecycle `5/5`、Collection Control `8 + 11`。临时数据库、container 与 volume 在该套件后清理验证通过。
- 1440 目录：CSS viewport `1440×900`、DPR 1；document `1440/1440`，creator/keyword 表均 `tableOverflow=0`；表头分别为 12/10 列，普通数据格换行数 0，两行主动作数均为 1。
- 1440 creator 抽屉：宽 `1036.8px`，占 viewport `72%`，document `1440/1440`；固定身份区只有 1 个主动作，三 Tab 为“概览｜档案｜巡查”，标题顺序严格为“最近变化｜作品生命周期｜档案缺口”。
- 生命周期：synthetic fixture 渲染 3 个 stable Work 点，其中 2 个目录关联空心点、1 个作者确认实心点、1 个最近巡查橙色外圈；SVG 使用 `role=group`，点的可访问名称包含作品、日期、指标和关联状态。选中新增作品后出现原始发布时间/点赞数和精确 stable Work Corpus 深链。
- 档案与 keyword 分型：creator 档案 Tab 显示作品目录 `2`、详情进度 `0 / 2`、当前可分析 `3`，并明确评论/OCR/ASR/媒体回到语料页；keyword 抽屉只有“概览｜巡查”，散点数为 0。
- focus 与 console：无 fragment 打开 creator 抽屉时焦点进入 `#c-drawer-title`；带 `#creator-lifecycle` 的作品深链尊重目标锚点。最终 creator 页面 console warning/error/exception 为 0。独立 Chrome 截图进程出现 macOS headless `CVDisplayLink` 与 GCM 环境日志，它们不是页面 console 事件。
- 清理：最终证据形成后停止一次性 `:3318` API、Chrome CDP/profile、PostgreSQL database/container/volume；清理状态记录在本 Package 的 Issue/PR 回执中。

最终提交在本节形成后产生；浏览器验证对应同一源代码树，exact correction source head 由随后一笔文档回执记录。

## 7. 未证明与不支持

- `NOT VERIFIED / NOT AUTHORIZED`：merge、`origin/main`、共享数据库或 migration、共享 `:3000`、worker 切换、插件 reload、真实平台、真实账号/Cookie、真实 200 篇目录扫描、部署。
- `NOT VERIFIED`：Mog 对真实数据和日常使用的业务验收。
- 当前没有 target-level“新增 N / 更新 N / 补齐 N”差分读模型；UI 必须诚实显示“尚未取得”。
- 目录作品没有合格精确发布时间或所选指标为 UNKNOWN 时不伪造散点；“目录里有作品”不保证“每篇都立即能画”。
- 手机、小于 13 寸设备、1280 与 390 CSS px 已由 Mog 明确移出项目支持范围。本报告只验收 1440 CSS px 桌面，不再把窄屏问题带回修复循环。
