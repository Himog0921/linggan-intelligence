# COMMENT-STUDY-REBUILD-001 · UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-17
> 适用范围: `/corpus/comments` 新评论研究读取面及其本地 API 替换准备
> 事实来源: DEC-0006、COMMENT-RESEARCH-REBUILD-001、当前旧 V1 路由/页面代码、LIDS
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/数据合同和当前代码

## 1. 事项

- Issue / SCOPE: Issue #295 / COMMENT-RESEARCH-REBUILD-001。
- Agent 与 worktree: `/root` / `codex/comment-research-rebuild-001`。
- 目标: 用新 `linggan_comment_study_*` 事实替换评论研究旧 V1 的 API、页面和 worker 依赖；先完成只读投影，再接入经用户确认的本机 reset 与真实模型 adapter。
- 用户可见结果: 用户能看见本次所选作品、冻结的评论目标、每个目标的真实处理状态、已接纳 Signal、待归并 Signal 和已建立的稳定 Problem；所有未开始、处理中、部分完成、来源受限和未配置模型状态各自如实表达。
- 明确非目标: 不保留 V1 ResultRevision/向量/聚类/自动排程显示或兼容入口；不在 UI 调用模型、重置本机数据、创建 Problem、改变 raw Evidence、发起外部访问、刷新 3000、合并或部署。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `current-state.md` | 已读 | 隔离 worktree、开发期 reset 和真实副作用边界 | 2026-09-17 |
| UI execution contract | 已读 | 先建立表面/状态/依赖/验收矩阵，后修改用户界面 | 2026-09-17 |
| DEC-0006 / rebuild plan | 已读 | 评论是唯一 Signal 证据；旧路径不可兼容 | 2026-09-17 |
| `PAGE-COMMENT-RESEARCH-V1-001` | 已读，仅作被替代范围 | 确认现页的 V1 ResultRevision、向量与自动运行语义不能复用 | 2026-09-17 |
| LIDS Primitive / Pattern / Data Boundary / Language | 已读 | L1 Corpus Explorer、文字 Tab、中文状态、Known(0) 与 Unknown 分开 | 2026-09-17 |
| 当前 API / HTML / JS / worker | 已读 | 当前 `/corpus/comments` 及 `/api/local/comment-research/*` 全部绑定旧 V1 | 2026-09-17 |

## 3. 变更分类

- 分类: 混合；最高风险为状态/语义。
- 对应来源: DEC-0006、COMMENT-RESEARCH-REBUILD-001 §当前合同、LIDS-BOUND-001、LIDS-LANG-001。
- 风险覆盖: 页面读取的是研究事实，不得将未处理、没有 Signal、待归并、模型失败、来源受限或未配置模型压成同一种空态或成功态。
- DECISION_REQUIRED: 无。新合同已明确用户任务和允许的行动；若需要新增人工归并、自动排程或真实模型按钮，必须另立决定。
- L1 / L2 / L3 与主 Pattern: L1 / Corpus Explorer。原声、目标与 Problem 是连续表格；不建第二套工作台、卡片瀑布流或 Agent 对话面。
- Token / Primitive / CMP / Scene / Motion / Data Truth: 复用 shell 与 LIDS 基线，只使用既有表格、状态和反馈语法；不新增 token、CMP、场景或动效。

## 4. 表面、状态、依赖与边界

| 表面 | 数据事实 | 必须显示 | 明确不显示 |
|---|---|---|---|
| 研究概览 | 最新 StudyRun 与累计 StudyProblem | 所选作品数、目标数、各真实状态数、已建 Problem 数 | 占比、趋势、旧 ResultRevision、模型“成功”暗示 |
| 评论目标 | StudyTarget + source/current qualification | 评论原声、冻结上下文状态、处理状态、Signal 数 | 评论作者身份、模型原文、向量/embedding 术语 |
| 待归并信号 | eligible Signal + Resolution | `deferred_novel` / `deferred_ambiguous` / `deferred_context` 的具体含义和下一步条件 | 把等待写成失败或已建 Problem |
| 用户问题 | StudyProblem + membership | 稳定定义、include/exclude、关联 Signal 数 | 仅凭相似度的合并、伪造问题规模 |
| 执行记录 | StudyBatch + semantic attempt + invocation receipt | batch/target 生命周期、可读失败原因、是否有真实模型调用收据 | 评论正文、prompt、密钥、原始 provider 输出 |

| 状态 | 用户应理解什么 | 禁止暗示 | 下一步 |
|---|---|---|---|
| 尚未开始 | 没有 StudyRun，未创建研究工作 | 没有评论或研究失败 | 选择作品并开始研究（未来受控 action） |
| 未配置模型 | Run/batch 可以存在，但不能外发模型请求 | 模型正在执行或研究已完成 | 在模型设置中完成配置 |
| 处理中 | batch 已 lease 或 invocation 仍 running | 已得到 Signal / Problem | 等待真实回执；不自动刷新为成功 |
| 无信号 | 模型已显式返回 no_signal | 模型缺输出或调用失败 | 查看原声和本次合同 |
| 等待语境 | problem/need Frame 缺必要字段，或目标缺父语境 | 失败或无用户问题 | 保留信号，等待后续可读语境或人工判断 |
| 待归并 | Signal 已通过语义接纳但未关联 Problem | 已创建新 Problem | 等待候选/第二独立证据 |
| 部分完成 | 同 batch 有接纳和可重试/失败 target | 全部完成或无结果 | 查看运行记录 |
| 来源受限 | 冻结后来源不可再研究 | 已安全处理或评论不存在 | 不外发；查看限制原因 |

- 受影响页面: `/corpus/comments`、其 `comment-research` 静态资源和 `/api/local/comment-research/*` 本地读取 API。
- 受影响组件: 页面自有表格、tabs、空态、运行反馈；不改 shell、一级导航或其它页面。
- 数据/权限/行动: 只读 API 仅投影新 schema；未来开始研究/reset/action 必须以服务端回执为准。
- 禁止修改的能力: raw Evidence、平台采集、模型密钥、外部 adapter 调用、共享 migration、runtime 与旧系统的兼容读取。
- 停止条件: 若当前新 schema 无法提供某个用户状态的真实来源，页面必须显示该状态为未知/不可读，不得在前端补造。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | API fixture + 页面路由测试 | 待新投影实现 | 尚未浏览器验收 |
| 状态诚实 | 对尚未开始、无信号、待归并、部分、来源受限、未配置模型分别构造 fixture | 待新投影实现 | 不证明真实模型语义质量 |
| 视觉一致 | L1 shell / Corpus Explorer 字符串与截图走查 | 尚未实施 | 现有 V1 页面不作为新设计合格证据 |
| 真实后果 | HTTP 只读断言调用账本与 Evidence 均不变化 | 待 API 测试 | 不调用模型、不 reset、不部署 |

## 6. 交接

- 计划修改: 新 comment-study read module、local API route/read tests、页面 HTML/CSS/JS；待上述投影稳定后删除旧模块。
- 验证: Rust unit、isolated PostgreSQL、local HTTP、页面静态/浏览器走查、project governance；按阶段运行，不将旧 V1 套件当作新合同证明。
- LIDS migration log / 预览同步: 新页面可运行并完成视觉验收时再登记；本清单不是运行时迁移完成声明。
- PR / reviewer / integration owner: 尚未授权提交、PR、审查或合并。

## COMMENT-STUDY-LAYOUT-001 补充（2026-09-17）

### 1. 事项与读取回执

- Issue / SCOPE: Issue #295 / COMMENT-STUDY-LAYOUT-001；属于既有 COMMENT-RESEARCH-REBUILD-001。
- Agent / branch / worktree: `/root` / `codex/comment-study-layout-001` /
  `/Users/moglenny/proma/linggan-intelligence/.worktrees/comment-study-layout-001`。
- exact base: `origin/main@0a7c94057c1fb6cfac0e545a23995e783b84e9e0`。
- 用户可见目标: 移除评论研究页的无效 Hero 占位，把已有的 ADHD 作品选择改为可筛选、可滚动的
  连续表格，使研究启动与现有读数都可在同一 L1 工作区高密度阅读。
- 明确非目标: 不改 shell、Token、路由、API、StudyRun/policy 语义、数据库、worker、模型调用、
  runtime 或部署；不新增服务端搜索、批量动作或数据字段。

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS / current-state / UI execution contract | 已读 | 受保护 UI 包、worktree、表面/状态/依赖/验收矩阵 | 2026-09-17 |
| PAGE-COMMENT-STUDY-REBUILD-001 | 已读并补充 | L1 读取责任及既有受控启动边界 | 2026-09-17 |
| LIDS Token / Primitive / Pattern / Materials / Shell / Data Boundary / Language | 已读 | L1 Corpus Explorer、连续表格、中文、白场与壳层所有权 | 2026-09-17 |
| 当前 `/corpus/comments` HTML / CSS / JS 与 :3000 | 已读 | Hero + 卡片选择挤出数据区；live runtime 的确仍是该页面 | 2026-09-17 |
| `comment-study.setup.v1` | 已读 | 只消费现有 `eligibleWorks`，不发明搜索或数据合同 | 2026-09-17 |

### 2. 表面、状态、依赖与验收

- 分类: 展示 + 既有交互呈现；最高风险为状态语义。
- Pattern: L1 Corpus Explorer；不新建 CMP、Scene、Motion 或 Token。默认表面为 M-00 白场；不在正文、
  表格或读数区增加纹理。
- exclusive files: `comment_study.html`、`comment_study.css`、`comment_study.js`、相关页面静态测试，
  本页规格、清单、验收、LIDS log、索引与 2026-09 progress。
- shared files: 无。forbidden: `shell.rs`、`shell.css`、LIDS token、API/data contract、migration、worker、
  runtime/deployment 脚本。
- 停止条件: 若实现需要新的真实状态、服务端检索、改变已选作品的 Run 输入、改 shell 几何、或改变
  policy/run 回执，即停止并报告。

| 表面 / 状态 | 必须保持的用户含义 | 自动或人工验收 |
|---|---|---|
| 初始加载 | 候选尚未读取，不能假装为 0 条 | 表格加载行、状态文字与静态断言 |
| 有候选 / 空候选 | 已加载作品数与当前筛选命中数分开；空候选不等于读取失败 | JS fixture / 浏览器走查 |
| 筛选与选择 | 筛选只缩小可见集合；隐藏项仍属已选择输入 | JS 行为断言 / 浏览器走查 |
| 保存策略 / 创建 Run | 继续调用既有 endpoint；按钮状态不伪造回执 | 页面脚本静态断言与现有 HTTP 合同 |
| 运行读数空态 | 无 Run、无 Signal、无 Problem 各自可读 | 页面渲染与浏览器走查 |
| 窄屏 | 可自然滚动，不用桌面嵌套滚动困住表格 | 390px 浏览器走查 |

本补充不把浏览器截图、构建、提交、合并、3000 刷新或 Mog 验收预先写成完成事实。

## COMMENT-STUDY-TABS-001 补充（2026-09-17）

### 1. 事项与读取回执

- Issue / SCOPE: Issue #295 / COMMENT-STUDY-TABS-001；属于既有 COMMENT-RESEARCH-REBUILD-001。
- Agent / branch / worktree: `codex/comment-study-tabs-001` /
  `/Users/moglenny/proma/linggan-intelligence/.worktrees/comment-study-tabs-001`。
- exact base: `origin/main@8b3efb9d60ed61cb54ed1bdc90cefacb916dbd35`（含 COMMENT-STUDY-LAYOUT-001）。
- 触发: Mog 合并部署 COMMENT-STUDY-LAYOUT-001 后发现历史用户原声、运行情况和运行结果均不可见。
  核查确认这是 COMMENT-STUDY-REBUILD-001 本身遗留的实现缺口——本节 §1「已批准组合」列出的 5 个
  Tab（概览、评论目标、待归并、用户问题、运行记录）当时只完成只读 API，从未在页面上实现。
- 用户可见目标: 把已批准的 5 个 Tab 实现出来；「评论目标」Tab 新增对原始评论正文的读取（此前只读
  API 完全没有暴露 `commentText`）。
- 明确非目标: 不改 shell、Token、StudyRun/Policy/Batch 写入路径、迁移、worker、模型调用、runtime
  或部署；不新增分页控件；不重新设计已批准的信息架构本身。

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| PAGE-COMMENT-STUDY-REBUILD-001 §4 已批准组合 | 已读 | 5 个 Tab 与原声列的既定设计 | 2026-09-17 |
| 本文件 §1「已批准组合」/§4「表面、状态、依赖与边界」 | 已读 | 每个 Tab 必须显示/禁止显示的具体字段 | 2026-09-17 |
| `comment_study_read.rs`（既有只读 API） | 已读 | `overview`/`runs`/`targets`/`signals`/`problems` 已有字段与 `runRef` 依赖 | 2026-09-17 |
| `comment_study_source.rs` 的来源资格判定 | 已读 | 读取原文时如何安全核对当前限制状态的既有写法 | 2026-09-17 |
| 旧 `comment_research_read_v1.rs::read_voices`（已退役，仅作只读参考） | 已读 | 旧版如何把原文与限制状态绑定；不复用其表或路由 | 2026-09-17 |
| LIDS `ADR-11` 文字 Tab primitive | 已读 | 选中态只用字重 + 3px 信号线，不用分段控件 | 2026-09-17 |

### 2. 表面、状态、依赖与验收

- 分类: 混合；最高风险为读取新字段时的敏感数据边界（原始评论正文）。
- Pattern: L1 Corpus Explorer；复用既有 `.study-table` 家族与文字 Tab primitive，不新建 CMP、Scene
  或 Token。原声使用既有 `--lgi-font-evidence` Serif 呈现。
- exclusive files: `comment_study.html`、`comment_study.css`、`comment_study.js`、
  `comment_study.rs`（页面静态测试）、`comment_study_read.rs`、
  `comment_study_rebuild_postgres.rs`、本页验收记录、LIDS log、索引与 2026-09 progress。
- shared files: 无新增；`comment_study_read.rs` 是既有只读投影文件的追加字段，不改变既有字段语义。
  forbidden: `shell.rs`、`shell.css`、LIDS token、StudyRun/Policy/Batch 的写入合同、migration、
  worker、runtime/deployment 脚本。
- 停止条件: 若某个 Tab 需要的事实在当前 `linggan_comment_study_*` schema 中不存在，该 Tab 必须显示
  为未知/不可读，不得在前端补造；若读取原文需要新的迁移或改变现有限制判定的语义，即停止并报告。

| 表面 / 状态 | 必须保持的用户含义 | 明确不显示 | 自动或人工验收 |
|---|---|---|---|
| 评论目标 · 原声 | 当前仍可读的原始评论文字 | 已被限制或未知来源的原文（即使冻结时曾经可读） | Rust 页面静态断言 + 隔离 PostgreSQL 限制前后对比 |
| 评论目标 · 处理状态 | `target.state`/`contextState`/Signal 数的真实取值与中文含义 | 把 `excluded` 说成失败，或把 `no_signal` 说成缺输出 | 页面静态断言 |
| 待归并 | 仅 `resolutionState` 为空或 `pending`/`deferred_context`/`deferred_ambiguous`/`deferred_novel` 的 Signal | 已 `assigned`、`not_user_problem`、`protocol_rejected`、`failed` 的 Signal 混入；来源被限制后继续引用其 `evidence`（逐字子串）/`proposition`（摘要） | 页面静态断言（口径断言，非渲染像素）+ 隔离 PostgreSQL 限制前后对比（提交前审核发现原漏做此项核对，已修） |
| 用户问题 | 既有 Problem 定义、纳入/排除条件、关联 Signal 数、`active`/`retired` | 伪造规模或跨 Problem 的相似度合并 | 页面静态断言 |
| 运行记录 | 每次运行的作品/目标/逐状态计数完整表格 | 把「已产出」暗示成语义质量已验证 | 页面静态断言 |
| 窄屏 | 复核表格保留横向滚动（数据列多，不同于顶部 3 列作品表） | 强制纵向嵌套滚动困住整页 | 待浏览器走查（本轮尚未完成，见验收记录） |

本补充不把静态测试、隔离 PostgreSQL 证明、编译通过、构建、提交、合并、3000 刷新或 Mog 验收预先
写成完成事实；实际证据边界见 [验收记录](../acceptance/comment-study-tabs-001-acceptance.md)。

## COMMENT-STUDY-LAYOUT-002 补充（2026-09-17）

### 1. 事项与触发

- Issue / SCOPE: Issue #295 / COMMENT-STUDY-LAYOUT-002；属于既有 COMMENT-RESEARCH-REBUILD-001。
- 触发: Mog 部署验收 COMMENT-STUDY-TABS-001 后反馈两处布局问题（5 个 Tab 被巨大表单挤到页面
  最下面；研究策略与作品选择不该常驻页面）与一处数据疑问（"运行记录"全 0）。
- 用户可见目标: 页面改为「工具栏 → 5 个 Tab → Tab 内容」三段式，Tab 紧跟工具栏；「受控启动」
  与「研究输入」整体移入按钮触发的 `<dialog>`，不再占用主页面空间；创建 Run 成功后自动切到
  运行记录 Tab。
- 明确非目标: 不改变 5 个 Tab 各自的读取内容与状态语义（那是 COMMENT-STUDY-TABS-001 的范围）；
  不接通批次准备或模型调用（见下方根因排查，为独立决定，不在本包内擅自处理）。

### 2. 表面、依赖与验收

- 分类: 纯布局与交互容器调整；不改 API、schema、状态词典。
- exclusive files: `comment_study.html`、`comment_study.css`、`comment_study.js`、页面静态测试、
  本节文档、progress。
- forbidden: `shell.rs`/`shell.css`、LIDS token、API/data contract、migration、worker、
  runtime/deployment 脚本、`comment_study_read.rs`。

| 表面 | 必须保持的用户含义 | 自动/人工验收 |
|---|---|---|
| 工具栏 → Tab 顺序 | Tab 紧跟工具栏，中间不夹巨大表单 | 页面静态断言（工具栏与 Tab 之间字符距离上限） |
| 研究策略 + 作品选择 | 移入 `<dialog id="study-dialog">`，按钮触发，不常驻主页面 | 页面静态断言：`<main>` 不含 `policy-form`/`works`，dialog 内含两者 |
| 创建 Run 后 | 自动关闭弹窗、切到运行记录 Tab | JS 静态断言（顺序断言） |

### 3. "运行记录全是 0" 根因排查（只报告，不在本包内处理）

`prepare_study_run` 冻结 Target 为 `queued`，本身不调用模型；`prepare_study_batch`（把 Target
打包成可被 `linggan-worker` 常驻模型循环 `claim_next_study_batch` 认领的 Batch）在生产代码里
**零调用方**（`git grep` 全仓库确认，仅测试文件引用），也没有对应 API 路由或 CLI。因此任何
StudyRun 创建后，其 Target 会永久停在 `queued`，运行记录的产出/无信号/等待语境/失败/来源受限
计数会一直是 0——这不是本次或上次 UI 改动引入的回归，是"批次准备"这一环从合同层面就没有被
任何生产入口调用。是否要在此接通、如何接通（人工审批每次真实模型调用、额度控制、真实敏感
数据处理边界）需要 Mog 决定，属另一件事，本包不擅自处理。

本补充不把静态测试、编译通过、临时预览浏览器走查、提交、合并、3000 刷新或 Mog 验收预先写成
完成事实；实际证据边界见验收记录。
