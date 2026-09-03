# COLLECTION-READ-MODEL-CLOSURE-001 · Collection 三包串行收口计划

> 状态: 活跃计划
> 最后核对: 2026-09-03
> 适用范围: Issue #148 及其完成后才可开始的 Package 2 / Package 3
> 事实来源: Mog 2026-09-03 最新决定、Issue #148 正文与 Claim、当前 `origin/main@d5b7886`、现有 PostgreSQL migration / Rust read model / Collection 与 Corpus 运行代码、LIDS v7
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实代码/数据库/测试、ACCEPTED 决定、权威当前产品与设计合同；历史内容工作台只作行为证据，不作实现指令

## 1. 交付包与串行关系

Mog 要得到的不是三个互相独立的技术补丁，而是一条按事实层级收口的 Collection 用户结果：先让观察目标拥有可信的只读决策面，再收口采集控制，最后才建立 Creator Dossier。三个宏包必须串行：

```text
Package 1 · Collection Read Model Closure（Issue #148）
  目标级生命周期读模型 → 抽屉默认核心 → 精确跳转语料
                          ↓ 合并并核验最新 main
Package 2 · Collection Control Closure
  五个 Collection 工作面的控制与回执收口
                          ↓ 合并并核验最新 main
Package 3 · Creator Dossier
  在前两包稳定事实与控制面上建立创作者档案
```

- 当前只授权 Package 1 的实现、测试、提交、push 与 Draft PR。
- Package 2、Package 3 不在本 Claim 内；不得预建它们的 schema、API、页面或动作。
- 本包未经 Mog 对 exact head 的后续授权不 merge，不应用共享 migration，不切换 runtime，不部署，不关闭 Issue。

## 2. Claim 与固定工作区

- Issue: `#148 COLLECTION-READ-MODEL-CLOSURE-001`
- Stable task-id: `collection-read-model-closure-001-p1`
- Coordinator: Codex root `/root`
- Execution subagent: `/root/current_drawer_audit`
- Exact base: `d5b78863d8d56ad39664314229db02942fa4fd5d`
- Branch: `codex/collection-read-model-closure-001`
- Worktree: `/Users/moglenny/proma/linggan-intelligence/.worktrees/collection-read-model-closure-001`
- 当前 preflight: branch/HEAD/目录与 Claim 一致；工作树起始干净。共享 checkout、`runtime-main` 和 `main-preview` 均不作为写入位置。
- 并行所有权: 当前本机无其它活跃开发 worktree；历史 Draft PR #91/#88 涉及 Evidence/文档，但不在本机活跃 worktree。本包只以 exact base 为准，不吸收或重写它们。

## 3. 用户结果与最新决定

### 3.1 必须得到的结果

1. Collection 继续只有五个工作面；Evidence 只在 `/corpus/evidence` 呈现。
2. 点击 creator Observation Target 后，右侧抽屉默认概览直接呈现“创作者生命周期”。
3. 生命周期按稳定 Work 的合格真实发布时间呈现作品表现，不代表粉丝增长、执行账号状态或市场趋势。
4. 默认近 90 天，可切全部周期；近 90 天固定为 `Asia/Shanghai` 日历日期口径（当天与第 90 个日历日均纳入），不是 `as_of - 2160 小时`；多年日期不截断。
5. 支持点赞、评论、收藏、转发；`composite-v1 = likes + 2*collects + 3*comments + 4*shares` 只有四项均 `KNOWN` 时可计算。
6. 当前稀疏数据、作者未确认、发布时间不合格、指标未知和扫描截断必须成为可见真相，不得用 fallback 补成好看的曲线。
7. 选中作品点只显示理解图表所需的最小摘要，并准确跳转 `/corpus/evidence?work=<public-ref>`。

### 3.2 明确排除

- 不在 Collection 复制正文、评论、媒体、Evidence fragment 或完整 Corpus Inspector。
- 不保留目标抽屉的 Evidence tab，也不建立第六个 Collection 页面。
- 不做监控价值、机会评分、产出分、稀缺分、趋势、预测、自动淘汰或价值驱动频率。
- 不迁移旧 Next/React/Prisma/API/Outbox，不复制旧 AEDS 外观。
- 不访问真实平台、账号/Cookie，不重载插件，不运行共享 migration/runtime，不部署。
- 不改旧内容工作台、`references/` 原件、共享 checkout 或用户未提交 prototype。

## 4. 语义冻结与读模型 seam

历史标题“账号生命周期曲线”在本包内规范为 **Creator Work Lifecycle / 创作者生命周期**：

- 横轴只接受 Work Resource 已资格确认的精确 `published_at`；相对时间、source text、first seen、observed at 和当前时间都不得替代。
- 一个稳定 Work 最多一个点；多次 Observation 只更新 as-of 读模型选择，不复制点。
- 默认口径只允许详情中的稳定 `author_external_id` 与 creator target `identity_key` 精确相等。`OBSERVED_ON_TARGET_SURFACE`、目标显示名或作品作者显示名不能替代作者归属。
- 每个互动字段在同一 as-of 之前选择最新 `KNOWN`。更新的 `UNKNOWN` 不擦除旧 `KNOWN`；真实 `KNOWN 0` 保持 0。
- 创作者自身分位、滚动中位线和 composite 都是版本化、可重建的描述性投影，不是监控价值或趋势 Claim。
- 滚动中位线冻结旧服务端领域口径 `trailing-5-work-median-v1` / `window=5`；前端只渲染服务端返回值，不得用旧前端的 15 点或新造窗口重复计算。
- `life_work` 只属于页面选择状态；Rust/HTTP lifecycle read query 不接受、不消费 `selected_work`，避免把未消费字段伪装成查询能力。

本包确认的公共测试 seam：

1. Rust: `creator_lifecycle` 暴露 target-scoped window/metric query、result 类型及只读 PostgreSQL read function；选中作品不是读查询参数。
2. HTTP: `GET /api/local/collection/targets/{target_ref}/lifecycle`，查询参数只接受冻结的窗口与指标枚举。
3. Page: `/collection/targets?drawer=<target-ref>&life_window=<recent_90_days|all>&life_metric=<metric>&life_work=<public-ref>`；地址拥有抽屉、筛选与选中点状态。
4. Corpus: `/corpus/evidence?work=<public-ref>` 必须在目标 Work 不在首批列表时仍精确打开该 Work。

## 5. UI Change Manifest 与 Design Direction Lock

### 5.1 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md`、`docs/README.md`、`docs/current-state.md`、Issue #148 Claim | 权威/真实 | 授权、文件、完成层与停止线 | 是 |
| `docs/agents/ui-execution-contract.md`、`docs/design/design-governance.md` | 权威当前 | UI 闭集、表面/状态/依赖/验收责任 | 是 |
| `PAGE-COLLECTION-001` | 权威当前；将由本卡有界修订 | 五工作面、L1 + 受限 L2 抽屉、URL 状态 | 是 |
| LIDS v7 system/tokens/primitives/patterns/materials/shell/data/language/decisions/agent guide | 权威当前；多数运行时待迁移 | 白场、Token、图表、四态、中文、390px、a11y | 是 |
| Work Resource Read / Material Projection / migrations `0015`–`0026` | 代码事实优先 | stable Work、作者身份、时间资格、KNOWN Current | 是 |
| 当前 Collection target/drawer 与 Corpus deep-link 代码/测试 | 代码事实优先 | 当前 seam、缺口与回归边界 | 是 |
| 旧内容工作台 author lifetime service/tests | 历史行为证据 | 90 天/全部、作品点、排除与中位线的对标 | 是；只读且不移植外观/实现 |

### 5.2 变更分类

- 分类: 展示 + 交互 + 状态/语义的混合变更。
- 最高风险: 状态/语义。
- LIDS 强度: Collection 为 L1 Operations；目标抽屉为受限 L2。
- 主 Pattern: `Collection Control`，抽屉仍是同一工作面，不成为第二首页或第二 Evidence 工作台。
- Data Truth: Truth / Coverage / Validity / Freshness / Operation 分责；生命周期投影重点表达 Truth、Coverage、Freshness 与 query receipt，不制造 Operation 成功。
- DECISION_REQUIRED: 当前无。用户、Issue 和 Claim 已冻结产品语义、核心视觉与禁止项。

### 5.3 五问方向锁

1. **使用者与上下文**：产品负责人及小团队在 Collection app shell 中判断某个长期观察对象已经积累了什么可用历史、缺口在哪里，以及是否要去语料研读具体 Work。
2. **审美方向**：LIDS v7 的白场“研究仪器面板”——连续工作面、硬结构线、克制 signal、无渐变面积、无旧工作台皮肤。
3. **第一记忆点**：以作品真实发布时间为横轴、对数表现为纵轴的稀疏散点与滚动中位线；空缺和排除回执与点本身同等重要。
4. **硬约束**：Rust server-rendered HTML + page-local vanilla CSS/JS；只消费 `lids_tokens.css`；功能文字 ≥11px；字重只用 400/600/700；圆角仅 0/4/8；390px 无横向滚动；键盘/读屏/reduced motion；UNKNOWN 与 0 分开。
5. **签名微交互**：点击或键盘选择点后，同一抽屉内更新最小作品摘要并同步 `life_work`；按压/选择只允许 80–160ms 的 transform/opacity 反馈，无弹跳、永久动画或 hover-only 动作。

### 5.4 三行设计 thesis

- **Visual thesis**：白场、精密、克制的研究仪器面板；生命周期图是单一视觉核心，真实缺口不被装饰掩盖。
- **Content plan**：先定向目标与状态 → 生命周期摘要/图表 → 排除回执与最小作品摘要 → 基线/巡检/追踪辅助职责。
- **Interaction thesis**：URL 拥有窗口、指标与选中点；键盘和点击等价；选中反馈短促且 reduced-motion 可关闭。

### 5.5 CSS 与视觉资源策略

- 单一策略: 原生 page-local CSS，只消费唯一 `--lgi-*` / 现行兼容 token；不引入 Tailwind、CSS-in-JS、字体、图标库或新 token。
- 新增 `target_drawer.css` 隔离本卡抽屉新增视觉，不扩张已经过大的 `collection_workspace.css`。
- 图表使用可访问 SVG 几何，颜色、文字、hairline、surface 全部来自 LIDS token；无 gradient、glass、纹理面积或第二套组件系统。

## 6. 用户表面地图

| 表面 | 本包责任 | 非责任 |
|---|---|---|
| `/collection/targets` 列表 | 保持目标列表与点击入口；不把生命周期数字塞满列表 | 不展示 Evidence 结果、趋势或监控价值 |
| creator target drawer overview | 默认直接显示生命周期核心、摘要、图表、排除回执与最小选中摘要 | 不复制 Corpus Inspector，不做作者 Dossier |
| keyword target drawer | 明确“创作者生命周期不适用”，保留其它既有职责 | 不画空的伪曲线 |
| drawer baseline / patrol / trace | 保留基线、巡检、追踪四职责；Evidence tab 退役 | 不新增控制动作或价值频率 |
| lifecycle JSON API | 提供有界、as-of、只读 target projection | 不返回正文、评论身份、媒体、原始 payload、Evidence fragment |
| `/corpus/evidence?work=` | 精确定位生命周期选中的 Work | 不改变 Evidence 产品含义或复制事实 |
| `/collection/attention` | 只在既有可证明异常/限制 seam 能直接复用时同步 | 不凭生命周期分数制造待处理事项 |

## 7. 状态词典

| 状态 | 触发/来源 | 用户应理解 | 禁止暗示 |
|---|---|---|---|
| `READY` | creator target 有至少一个符合当前窗口/指标的点 | 当前投影可画；仅代表已接纳范围 | 平台全量、趋势或价值 |
| `INSUFFICIENT_OBSERVATION` | 有关联/确认作品，但时间或指标不足 | 数据不足以形成当前曲线 | 没有作品、表现为 0 |
| `AUTHOR_NOT_VERIFIED` | target surface 有 Work，但无稳定作者 ID | 归属未证实 | 默认算作该作者作品 |
| `AUTHOR_MISMATCH` | 详情作者 ID 与 target 不同 | 该 Work 不属于默认曲线 | 数据损坏或目标无效 |
| `PUBLISHED_AT_NOT_QUALIFIED` | 无合格精确来源发布时间 | 不进入 x 轴 | 用观察时间补齐 |
| `METRIC_UNKNOWN` | 当前指标 as-of 前无 KNOWN | 不进入该指标曲线 | 指标为 0 |
| `SCAN_LIMITED` / `TRUNCATED` | 有界扫描达到预算 | 当前点集不是完整扫描 | 已覆盖目标全部历史 |
| `NOT_APPLICABLE` | keyword target | 该视图只适用于 creator | keyword 没有结果 |
| `TARGET_NOT_FOUND` | target ref 不存在 | 本机读模型找不到标识 | 平台对象不存在 |
| `READ_UNAVAILABLE` | 数据库/投影读取失败 | 现在无法读取 | 空集或成功 |

## 8. Reality Matrix（实施前）

| Claim | 状态 | 当前证据 | 本包动作 |
|---|---|---|---|
| 稳定 Observation Target 与唯一 creator identity | VERIFIED | `0005_collection_observation_target.sql`、`collection_target.rs` | 复用，不新建身份表 |
| Target transition 与 WorkOrder/Task/Attempt/Package/Receipt 历史 | VERIFIED | 现有 migrations/read modules | 只在追踪投影中使用既有事实 |
| stable Work identity | VERIFIED | `0015_material_projection.sql` | 每 Work 最多一个点 |
| append-only detail/discovery/engagement observations | VERIFIED | `0015`、`0018`、`0020`、`0023` | 在统一 as-of 内读取 |
| qualified exact `published_at` | VERIFIED；live sparse | `0026` 与 PostgreSQL tests | 只接受已资格确认时间 |
| field-wise latest KNOWN | VERIFIED | `material_query_sql.rs`、`material_detail_read.rs` | 复用选择语义；测试 UNKNOWN 不擦除 |
| target surface relationship | VERIFIED | Work Resource collection context | 只计关联总数，不替代 authorship |
| 所有关联 Work 的作者归属 | NOT VERIFIED | discovery 无稳定 author id；当前可靠匹配稀疏 | 默认曲线只纳入 exact match，分项排除 |
| target-scoped lifecycle read model/API | ABSENT | 当前 crate/API 无 seam | Work Package A 新增 |
| drawer 默认生命周期核心 | ABSENT | 当前 overview 只有 archive counts，Evidence tab 尚在 | Work Package B 新增/退役 |
| robust Corpus `?work=` deep link | PARTIAL | URL 可恢复；首批列表外会回退第一项 | Work Package C 修复 |
| 当前 live 丰富曲线 | NOT READY | 约 44 个关联 Work 仅约 1 个可靠点 | 正常呈现稀疏与排除；不采集补图 |
| 无新事实表可完成首版 | CONDITIONALLY VERIFIED | 现有事实列齐；规模性能尚需 focused proof | 无 migration；若索引不可避免则停止回报 |
| Evidence 只在 Corpus | 用户决定已冻结；代码 PARTIAL | drawer 仍有 Evidence tab | 删除 tab，只保留跳转 |
| 监控价值不在本包 | VERIFIED BY DECISION | Issue/Claim 明确排除 | 源码与响应负向断言 |
| 自动、浏览器、部署、Mog 验收 | NOT VERIFIED | 尚未实现/运行 | 分层验证；未发生者保持 NOT VERIFIED |

## 9. 依赖与文件边界

### 9.1 Exclusive

- `crates/evidence/src/creator_lifecycle.rs`（新增）
- `crates/evidence/tests/creator_lifecycle_postgres.rs`（新增）
- `apps/api/src/local_web/target_drawer.rs`
- `apps/api/src/local_web/target_drawer.css`（新增）
- 本 Issue 新增 focused module/test files
- `docs/plans/active/collection-three-package-closure-001.md`
- 本 Issue 新增 PAGE/UI manifest/acceptance 文件

### 9.2 Shared，限完成本包的最小改动

- `crates/evidence/src/lib.rs`
- `apps/api/src/local_web.rs`
- `apps/api/src/local_web/tests.rs`
- `apps/api/src/local_web/evidence_library.js`
- `docs/README.md`
- `docs/design/README.md`
- `docs/current-state.md`
- `docs/design/lids/migration-log.md`
- `docs/progress/2026-09.md`

### 9.3 Forbidden

- `database/migrations/**`，除非先证明索引不可避免并由 Mog 扩权。
- Browser Producer、manifest、release、旧内容工作台、`references/` 原件。
- 共享本机数据库、runtime snapshot、launchd、Chrome 已加载插件。
- Claim 未列文件默认 forbidden；确有直接依赖时先停止该文件并报告 coordinator。

## 10. Work Package A → B → C

### A. 目标级生命周期读模型

按垂直 TDD slice 推进：

1. RED: 隔离 PostgreSQL test 先证明两 target 同名隔离、稳定 author exact match、qualified time 和 KNOWN metric。
2. GREEN: 新增最小 target-scoped read function，单一 repeatable-read/read-only/as-of 事务返回 summary + points + receipt。
3. RED/GREEN: NOT_VERIFIED/MISMATCH、相对时间、UNKNOWN/KNOWN 0、重复 Observation、一点/空集、`Asia/Shanghai` 90 个含首尾日历日边界、跨年、并列、极值、scan limit。
4. RED/GREEN: HTTP enum validation、response 最小必要字段与负向敏感字段断言。
5. 性能: focused `EXPLAIN`/bounded scan；如只能靠新索引达标，命中停止条件，不自行写 migration。

退出条件: crate + HTTP public seams 全部由失败测试先行并在隔离 PostgreSQL 通过；未修改 schema。

### B. Target Drawer + LIDS v7

1. RED: render tests 固定四 tab、Evidence tab 退役、creator overview 默认生命周期、keyword NOT_APPLICABLE、无监控价值词。
2. GREEN: drawer 接入生命周期 projection；overview 以图表为唯一视觉核心，辅助职责退后。
3. RED/GREEN: 90 天/全部、五指标、选中点 URL；SVG 点可键盘选择并有 `aria-label` 与文本摘要；中位线只消费 `trailing-5-work-median-v1`，前端不二次计算。
4. CSS: 新 page-local stylesheet，只消费 token；390/1280/1440 几何、无渐变、字号/字重/圆角/触控区/reduced-motion 检查。
5. 状态: loading 不适用于 server render；READ_UNAVAILABLE、INSUFFICIENT、SCAN_LIMITED、NOT_APPLICABLE、1 点/0 点分别渲染。

退出条件: 默认 creator 抽屉无需进入二级 tab 即看见真实生命周期；所有非目标词与 Evidence 结果区均不存在。

### C. Corpus 深链、待处理与集成

1. RED: `?work=` 指定 Work 不在首批列表时测试先失败。
2. GREEN: 使用既有 Work Resource detail seam 精确定位，不扩大 Evidence 数据或产品语义。
3. 核对 `/collection/attention`: 只有现成读模型能直接证明的异常/限制可投影；否则保持现有诚实状态，不新造事项。
4. 同步 PAGE-COLLECTION-001、UI change manifest、LIDS migration log、索引、current-state、月度 progress 与验收记录。
5. 浏览器只在隔离只读实例验证 1440、1280、390；不触碰 `:3000`、共享库、插件或采集。

退出条件: 生命周期点准确进入唯一 Corpus Evidence 面；Collection 没有 Evidence 复制品。

## 11. 验收矩阵

| 用户结果/风险 | 自动 seam | 隔离运行证明 | 人工/浏览器 |
|---|---|---|---|
| target 严格隔离与作者资格 | `creator_lifecycle_postgres` | 随机隔离 PostgreSQL schema/database | 不依赖人工 |
| qualified time、KNOWN/0/UNKNOWN | PostgreSQL worked examples | 单一 as-of transaction | 图表状态文本走查 |
| 90 天/全部/跨年/排序/极值 | Rust query tests | 固定 clock/as-of | 轴标签与 summary |
| 敏感字段与监控价值不泄漏 | response/render negative tests | JSON/HTML source scan | 页面首屏扫描 |
| creator/keyword/失败/稀疏/截断 | render tests | synthetic/de-identified fixtures | 1440/1280/390 |
| 键盘、ARIA、reduced motion | source/DOM tests | 隔离页面 | Tab/方向键/Enter/Escape |
| Corpus precise deep link | JS/Rust integration tests | 首批列表外 Work | 刷新/返回/窄屏 |
| LIDS v7 | token/style/governance checks | page-local asset served | 白场、无横滚、无渐变 |
| 完整 workspace | `cargo test --workspace --locked`、fmt/check | 无共享副作用 | 不等于部署/业务验收 |

## 12. 停止条件与回退

命中以下任一项，停止受影响 Work Package 并向 coordinator 报告：

- 只能用展示名、surface relationship 或模糊匹配认定作者；
- 只能把 UNKNOWN 当 0、把 first seen/observed at 当发布时间才能画图；
- 需要改变 Evidence 事实/UI 产品语义才能让 Collection 成立；
- target-scoped 查询只能通过未授权 schema/migration 才能可用；
- 需要真实平台、账号/Cookie、插件发布、共享 migration/runtime/deploy；
- Claim shared 文件出现新的并行所有权冲突；
- 出现两个会改变业务含义的实现路径而现有决定无法裁定。

回退方式是撤销本 branch 的有界代码/文档提交；本包不接触共享数据库、runtime 或外部平台，因此不需要现实世界数据回滚。

## 13. 分层完成状态

| 层 | 当前状态 | 完成证明 |
|---|---|---|
| Plan / Claim / Reality Matrix | VERIFIED | 本计划与 `docs/README.md` 索引 |
| Work Package A | VERIFIED（branch） | RED→GREEN；隔离 PostgreSQL lifecycle 4/4、API 1/1；无 migration |
| Work Package B | VERIFIED（branch） | 四职责抽屉、默认生命周期、Evidence tab 退役；1440/1280 与 CDP 390 验证 |
| Work Package C | VERIFIED（branch） | 首批列表外 Work 精确 Corpus detail；未复制 Evidence |
| focused/workspace/governance | VERIFIED | workspace test/check、fmt、JS syntax、diff 与两项 governance 全通过 |
| commit/push/Draft PR | PENDING | Draft PR body 必须 `Refs #148` |
| independent exact-head review | NOT VERIFIED | 由 Mog/Coordinator 安排 |
| main merge | NOT AUTHORIZED / NOT VERIFIED | 不在 Claim 内 |
| shared DB/runtime/deploy | NOT AUTHORIZED / NOT VERIFIED | 不在 Claim 内 |
| Mog 业务验收 | NOT VERIFIED | PR 不能替代 |
