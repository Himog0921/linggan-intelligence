# COLLECTION-READ-MODEL-CLOSURE-001 · Collection 三包串行收口计划

> 状态: 活跃计划
> 最后核对: 2026-09-04
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
- Review remediation task-id: `collection-read-model-closure-001-p1-review-fixes-1`
- Review remediation 2 task-id: `collection-read-model-closure-001-p1-remediation-2`
- Coordinator: Codex root `/root`
- Execution subagent: `/root/current_drawer_audit`
- Exact base: `d5b78863d8d56ad39664314229db02942fa4fd5d`
- PR #151 review baseline: `e386cce2f6b08b602881a233fbd8b4de6c0595e5`
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

本包确认的测试与公共 seam：

1. Rust: `material_query_sql + WorkResourceCurrent` 是 crate-private typed batch Current owner；列表、单品、详情 Inspector 与 lifecycle 在同一 `as_of` 规则下复用。`creator_lifecycle` 只做 target-scoped 候选、窗口与派生计算；选中作品不是读查询参数。
2. HTTP: `GET /api/local/collection/targets/{target_ref}/lifecycle`，查询参数只接受冻结的窗口与指标枚举；响应是瘦 derived DTO，不公开 title/author/published/engagement Current。
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
| lifecycle JSON API | 提供有界、as-of 的 target/window/规则/coverage/排除/scan receipt 与 percentile/median | 不返回 title、author、published/engagement Current、正文、评论身份、媒体、原始 payload、Evidence fragment |
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

## 8. Reality Matrix（实施前基线）

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

### 8.1 PR #151 review remediation 现况

| Finding | 当前 branch 处置 | 证明 |
|---|---|---|
| Work facts 第二套裁定 | `WorkResourceCurrent` typed batch + 单一 CTE/列合同；列表、单品、Inspector、lifecycle 同 owner、同 transaction/as-of | 同 Work parity，含相同 `observed_at` tie；Inspector/长历史回归 |
| Signal focus / SVG `outline:none` | 全局 `--lgi-focus` 与统一 2px/2px focus；点另有 4px stroke + scale | 真实 Tab 走查与 CSS 合同 |
| HTML 非法 lifecycle query 静默回落 | 缺省与非法分离；非法为 `QUERY_INVALID`，不标记默认项 | render/API 422 回归 |
| all caption 写成 90 日 | caption 由实际 window 输出 | all render 回归 |
| Escape/focus/列表上下文 | drawer 在脚本前挂载，Escape 返回原 filter/既有 `sort=last` 上下文并 focus opener | 390 浏览器回归；`sort` 不进入列表查询 |
| retired/unknown tab 读渲染分叉 | 单一 `TargetDrawerTab` 闭集解析；统一归一 Overview 并真的读取 | HTML 路由回归 |
| 截断总量冒充精确 | exact total 为空，另给 `>=2001` 下限及 probe/scanned/returned | 2001 fixture 与 UI 回归 |

公共 lifecycle route 已瘦身为 derived DTO；内部 server render 的 Work facts 继续来自共享 Current owner，
不会形成可替代 Work Resource 的第二份 API。Evidence 仍只在 Corpus，监控价值仍排除。

### 8.2 PR #151 remediation 2 · 实施前 Reality Matrix

本轮只修复 Issue #148 扩展 Claim 中已经点名的同类别缺口；起始
`HEAD=e3aaa05af6ee44a8b58317608a3f410b54484bfb`，本地与远端分支一致且工作树干净。

| Claim / risk | 实施前状态 | 当前证据 | 本轮可证伪动作 |
|---|---|---|---|
| Published Current 的 exact value、source text、field/kind/precision/parser 与 source package/record 必须来自同一来源行 | **FAIL** | `latest_detail_published_at` 选 exact 行，但 `published_at_source_text` 优先选择独立的更新 text 行；会拼出旧 exact + 新 relative | 先用隔离 PostgreSQL 回归制造“旧 exact、更新 relative”，要求整组 published provenance 仍锁定旧 exact 行，再收敛共享 Current SQL |
| Inspector 的 title/body/creator 字段来源必须跟随各自 typed Current source | **FAIL** | Current 已有逐字段 source ref，`material_item` 仍用聚合 material/package ref 生成多字段 `fieldSources` | 先写 Work Resource list/detail/Inspector parity 回归，再让 Inspector 逐字段消费 typed source，并把真实 package/record refs 去重聚合 |
| Lifecycle SVG point 的可交互命中区至少 24×24 | **FAIL** | 当前唯一 `<circle r="5">` 的几何命中区为 10×10；虽有键盘链接与共享 focus ring，触控目标不足 | 先写 render/CSS 合同要求透明 hit circle + `24px` non-scaling stroke，并保留独立可见点，再做 390/1280/1440 DOM geometry、Tab/ARIA/focus 实测 |
| no-DB / target read error 抽屉必须消费与正常态相同的列表上下文 | **FAIL** | `render_unreadable_target_drawer` 关闭链接硬编码 `/collection/targets`，无 `data-return-url` / `data-return-focus`，会丢 filter/sort/fragment | 先写 failure HTTP/render 回归，再复用 `TargetListContext`；浏览器验证 Escape/close 保留上下文，正常可恢复 opener focus，失败页无 opener 时保持准确 URL 而不伪造焦点成功 |
| Lifecycle 公共 API 仍为 slim derived DTO | VERIFIED；需回归 | 当前 API 不暴露 title/author/published/engagement Current | 负向 JSON/route 测试与源码扫描 |
| Corpus `?work=` 仍精确定位，Evidence 只在 Corpus | VERIFIED；需回归 | 当前首批外 detail seam 与 Collection 负向文案测试 | JS/HTTP focused 回归；抽屉不得新增 Evidence 结果区 |
| 监控价值继续排除 | VERIFIED BY DECISION；需回归 | Issue/Claim、PAGE 与 HTML 负向断言 | HTML/JSON/source 负向扫描 |
| migration/schema/shared DB/runtime/plugin/platform/deploy | FORBIDDEN | 扩展 Claim 明确不授权 | 若实现必须触及任一项则停止；验证只用一次性隔离 PostgreSQL 与 loopback runtime |

本轮依赖仍是同一条责任链：`material_query_sql` 拥有 typed Current 裁定，
`material_projection` 只把已裁定字段及其来源投影到 Work Resource/Inspector，
`creator_lifecycle` 只做 target/window/derived 计算；Target Drawer 只渲染最小摘要并跳转 Corpus。
这不是新增 Package 2/3，也不改变 Evidence 或监控价值边界。

### 8.3 PR #151 remediation 2 · 实施结果

| Finding | branch 处置 | 隔离证明 |
|---|---|---|
| Published Current 来源行可能拼接 | exact 发布时间的 value、source text/state、field/kind/precision、parser、material/package/record ref 与时间戳整组由同一 exact typed row 产生；只有不存在 exact 时才走 discovery/text fallback | PostgreSQL 回归构造“旧 exact + 更新 relative”，Material suite 11/11 通过 |
| Inspector 字段来源使用聚合 ref | title/body/creator 分别消费 typed Current source；Inspector provenance 去重聚合真实展示字段的 package/record refs | Work Resource list/detail/Inspector parity 覆盖独立 package 与 record，Material suite 11/11 通过 |
| SVG point 命中区不足 | 每点拆为透明 hit circle 与可见 circle；hit circle 使用 `24px` non-scaling stroke，交互仍由具名 link 承载，focus 继续使用全局 token | 390 实测两点水平/垂直命中跨度均 29px；首次右端裁切后增加 plot inset，终轮右端余量 16.02px |
| failure drawer 丢列表上下文 | 正常、target-not-found、read-error 与 no-DB 统一消费 `TargetListContext` / data attributes；Escape/close 保留 filter、只作返回上下文的 sort 与 fragment | 正常态 Escape 精确恢复 URL 与 opener focus；no-DB 390 保留 URL/fragment、无横向溢出；no-DB 列表没有 opener，未伪报焦点成功 |
| 生命周期 API / Corpus / Evidence 边界回归 | lifecycle route 继续只返回 derived DTO；选中 Work 仍精确跳 `/corpus/evidence?work=`；Collection 不复制 Evidence，监控价值继续排除 | API full-path 1/1；Corpus DOM 精确选择目标 Work；HTML/DOM 无 Evidence tab 与监控价值模块 |

本轮未修改 migration/schema、共享数据库、共享 runtime、插件、真实平台或部署；浏览器数据来自一次性隔离 fixture，不能充当当前真实数据运行证明。

### 8.4 PR #151 remediation 3 · 实施前 Reality Matrix

本轮起始 `HEAD=1c81393e2c4ddd2e490a87a2a1dcf58918c4c2d8`，本地、远端与
PR #151 head 一致且工作树干净。继续使用 Issue #148 Claim expansion 的文件白名单；
`evidence_library.css` 与 `collection_workspace.css` 明确禁止修改。

| Claim / risk | 实施前状态 | 当前证据 | 本轮可证伪动作 |
|---|---|---|---|
| 390px 直接打开首批外 `/corpus/evidence?work=` 后必须看见指定 Work Inspector | **FAIL** | detail seam 能精确读取 Work，但 URL 恢复调用 `selectItem(..., false)`；移动 drawer 保持 closed，Inspector 被移出视口 | 先用 source contract 与真实 390px direct-URL/refresh 复现；只让 URL 恢复成功打开 drawer，不新增 history、不改 work ref，并验证桌面布局不回归 |
| 全站所有可聚焦控件必须得到同一 computed focus | **FAIL** | `shell.css` 的旧 owner 未覆盖可聚焦作品 `<article>`；第一版 remediation 只以 `.v7-app` 定界，又漏掉作为 sibling 渲染的固定 drawer 与 SVG link；后加载局部 CSS 仍声明 3px Signal 或 2px Ink outline | 在 `shell.css` 建立主题文档级、明确交互元素集合与足够 cascade specificity 的 owner；不用 `!important`，不改两个局部 CSS；浏览器逐项读取 computed outline/offset |
| 非 focus 与 error 状态不得被 focus 修正破坏 | **需回归** | 本轮只应改变 `:focus-visible` 的 outline，不应覆盖 background/border/状态色 | 在 Corpus/Collection 桌面与 390px 记录 focus 前后 background/border；检查失败态仍保留原语义与样式 |
| Claim 外 CSS / schema / runtime /平台动作 | **FORBIDDEN** | 当前 Claim 与 coordinator 指令明确禁止 | 若需要改 `evidence_library.css` / `collection_workspace.css` 或其它未列文件则停止；运行证明只用一次性隔离 DB 与 loopback runtime |

本轮表面地图只有 `/corpus/evidence?work=` 的列表、390px Inspector drawer，以及 Corpus / Collection
共享壳内的代表性 link、button、input 与生命周期 SVG point。状态只覆盖 URL 恢复、drawer open/closed、
focus-visible、非 focus 与既有 error；不新增内容、事实、权限或行动状态。依赖方向固定为
`lids_tokens.css → shell.css themed-document focus owner → 后加载页面 CSS`，以及
`URL restore → selectItem → existing detail seam → Inspector`。

验收矩阵：Rust/source test 固定恢复与全局 owner 合同；JS syntax 与完整 workspace 防回归；隔离浏览器在
desktop/390 证明 `data-drawer=open`、可见 Inspector、精确 selected ref、无新增 history，并读取 Corpus / Collection
代表控件与生命周期 SVG 的 computed focus；治理与 UI handbook 证明文档和 LIDS 边界。

### 8.5 PR #151 remediation 3 · 实施结果

| Finding | branch 处置 | 隔离证明 |
|---|---|---|
| 390px 首批外 Work 恢复后 Inspector 不可见 | `selectItem` 使用 `url / user / auto` 来源；成功 URL 恢复与用户选择都打开移动 drawer，但只用户选择 push history，URL/auto 继续 replace | 63-Work fixture 中首批 50 外 `…0048` direct + refresh 均精确显示「补充作品 48」；Inspector rect `23.41–390px`、document `390/390`；单次 Back 返回 Collection 标记，Forward 精确恢复 |
| 后加载页面 focus 规则覆盖共享 LIDS ring | `shell.css` owner 改为主题文档内明确交互元素集合，specificity `0,3,0`；覆盖 `.v7-app` 与 fixed drawer sibling，不改禁止 CSS、不用 `!important` | Corpus/Collection link/button/input/Work row/lifecycle SVG 在 1440/390 真实键盘下均为 `2px solid rgb(51,94,114)`、offset 2；SVG hit circle 有效外径 36px |
| 非 focus / error 状态回归 | 规则只写 `:focus-visible` outline/offset，不重写 background、border 或语义色 | selected Work 仍为 Signal-soft；390 `QUERY_INVALID` 仍为灰底/原边框、0 point，关闭链接得到统一 ring；console logs `[]` |

本轮 isolated PostgreSQL lifecycle 5/5、Material/Inspector 11/11、API/UI focused 10/10；workspace
155 passed / 0 failed / 86 ignored，check 仅 15 条既存 dead-code warning；fmt、JS 2/2、diff、project governance
与 UI handbook 全通过。`:3116/:56690`、browser tab/viewport、container 与 volume 已清理。本轮不修改
`evidence_library.css` / `collection_workspace.css`，也未触碰 schema/migration、shared runtime/DB、plugin、平台或部署。

## 9. 依赖与文件边界

### 9.1 Exclusive

- `crates/evidence/src/creator_lifecycle.rs`（新增）
- `crates/evidence/src/work_resource_current.rs`（review remediation 新增；crate-private shared owner）
- `crates/evidence/tests/creator_lifecycle_postgres.rs`（新增）
- `apps/api/src/local_web/target_drawer.rs`
- `apps/api/src/local_web/target_drawer.css`（新增）
- 本 Issue 新增 focused module/test files
- `docs/plans/active/collection-three-package-closure-001.md`
- 本 Issue 新增 PAGE/UI manifest/acceptance 文件

### 9.2 Shared，限完成本包的最小改动

- `crates/evidence/src/lib.rs`
- `crates/evidence/src/material_query_sql.rs`、`material_projection.rs`、`material_detail_read.rs`（共享 Current 消费方）
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
| Work Package A | VERIFIED（branch） | RED→GREEN；隔离 PostgreSQL lifecycle 5/5、API full-path 1/1；共享 Current/Inspector parity；无 migration |
| Work Package B | VERIFIED（branch） | 四职责抽屉、默认生命周期、Evidence tab 退役；1440/1280/390 隔离浏览器验证，390 点命中区实测 29×29px |
| Work Package C | VERIFIED（branch） | 首批列表外 Work 精确 Corpus detail；未复制 Evidence |
| focused/workspace/governance | VERIFIED（branch） | Material 11/11、lifecycle 5/5、API full-path 1/1、workspace 155 passed / 0 failed / 86 ignored、JS 2/2；workspace check（15 条既存 dead-code warning）、fmt、diff、project governance 与 UI handbook 通过 |
| commit/push/Draft PR | VERIFIED（delivery） | 修正提交已 push 到既有 OPEN/DRAFT PR #151；PR body 保留 `Refs #148`，未 merge、未改 Draft 状态 |
| independent exact-head review | NOT VERIFIED | 由 Mog/Coordinator 安排 |
| main merge | NOT AUTHORIZED / NOT VERIFIED | 不在 Claim 内 |
| shared DB/runtime/deploy | NOT AUTHORIZED / NOT VERIFIED | 不在 Claim 内 |
| Mog 业务验收 | NOT VERIFIED | PR 不能替代 |
