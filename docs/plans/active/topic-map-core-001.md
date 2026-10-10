# TOPIC-MAP-CORE-001：主题图谱业务内核

> 状态: 活跃计划
> 最后核对: 2026-10-10
> 适用范围: Issue #382，主题提炼、语义归属、持久化及已有图谱前端
> 事实来源: Mog 当前对话、main@a88ff387、主题图谱手册 D01–D39、LLooM 与 TopicGPT 官方论文/源码
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码及验证回执；论文不是运行授权

## 交付包与研究依据

用户要求实际构建模块核心，算法、数据库、前端一次打通。保留完整概览、五阶段旅程、我方/外部/评论对照、自动增量及少量证据可见；不新增创作发布或第二个研究平台。代码在 `codex/topic-map-core-001` 独立 worktree，Issue #382 留有 Claim。根执行者负责持久化、worker、读取整合、文档与验证；协作者在同一交付包按互斥文件处理输入、模型契约及 UI。不自行合并或部署。

主要借鉴 [LLooM (CHI 2024)](https://arxiv.org/abs/2404.12259) 的多讨论提炼、明确概念标准及回到原文判断；[TopicGPT (NAACL 2024)](https://arxiv.org/abs/2311.01449) 的语义候选比较及解释归属。核查源码：LLooM `8252533ab6018bea89c1e33e31c0f8fb6a07a707`（BSD-3-Clause），TopicGPT `e6499ef7f3db8bba2bcff30e4d636b11d2acc97d`（MIT）。本次自主实现机制，不复制研究 notebook 运行时。

不采用低频删除、相似即合并、全量材料×全部主题逐项调用。两篇论文的指标不能证明本项目中文领域质量；真实模型质量需用业务样例另行测量。

## 算法与数据合同

1. 复用 canonical 作品、合格 OCR/ASR、评论与已有 Signal；完整来源分片，稳定 Unicode 区间；父评论仅作为明确上下文，不新建 Problem 或重复原文库。
2. 按模型输入预算建立可重建的有界窗口，每个窗口单独排队。记录来源总量、窗口范围与覆盖，不把前缀截断当全文处理。
3. 第一阶段提炼多个独立讨论，保存陈述、证据、来源角色、支持/反驳/背景及建议边界。持久化中间状态后才能进入第二阶段。
4. 使用已资格化的本地 WeMM 文档向量及文字检索召回已有主题；相似度只提供候选。没有向量时明确召回受限，保持可用结果。
5. 第二阶段逐讨论按主题定义、纳入/排除条件和原文判断，多主题归属、新主题、未确定及领域外分开。新主题接纳必须解释与召回候选的区别；等价复用已有 ID，不按显示名称合并。
6. 新增讨论、精确定义版本与归属记录；主题关系保留 broader/narrower/related 的研究依据。正式/人工定义不被机器静默改写。既有合并/拆分仍按 D12 显式预览确认。
7. 来源变更替换当前研究，保留历史账本；无关主题变化不使提炼重跑。预算、暂停、停止、源资格和迟到响应检查沿用已有执行约束。

讨论计数、去重作品数、评论证据数与需求人数不可混用。归入一个主题不等于支持其观点；否定、反例、作者宣传与用户经历分开。

## 表面、状态、依赖与验收矩阵

| 表面 | 实际变化 | 状态/来源 | 验收 |
|---|---|---|---|
| `/topics` 概览及主题身份 | 显示定义纳入/排除边界、来源规模 | 无边界时明确尚未形成；已有统计/树/作品继续可用 | 现有 UI 回归和新字段渲染 |
| 判断弹窗具体讨论 | 独立讨论、归属主题、简短依据和观点差异 | 作者/评论/引用/未知；支持/反例/背景；未确定不冒充已归类 | 合成同义、相邻主题、多讨论、否定及原文定位 |
| 原文阅读器 | 可回到窗口中具体 Unicode 区间 | 受限来源不显示原文或衍生结论 | 既有资格/乱序/关闭测试和新证据测试 |
| 研究进度 | 提炼与归属阶段、已完成窗口及覆盖 | 排队/处理中/部分/暂停/失败/未知分别显示；已有结果继续可读 | PostgreSQL 两阶段/预算/失败/幂等测试 |
| 旅程与我方对照 | 复用当前主题 ID 和讨论 | 旅程不替代主题；未知不强配；我方做过不排除 | 原有旅程/备选/比较回归 |

UI 变更分类：展示+交互+状态/语义，最高为状态/语义。读取 AGENTS、docs 总索引/current-state、UI execution contract、design governance、LIDS README/tokens/data-boundaries/language-policy 及相应 primitives/patterns 后实施。使用现有 Token → Evidence Fragment/Readout/Status → 主题详情/原文阅读 Pattern → `/topics`；不创建新全站组件或修改公共 Header。

文件所有权：输入协作者持有 `creator_discovery.rs`、`topic_map_research.rs`、`comment_study_source.rs` 的 topic 专用读取 seam；契约协作者持有 `topic_map_research_analysis.rs` 及同名子目录、pi-adapter topic 专用结构化输出与测试；UI 协作者持有 `local_web/topic_map.js/.css` 与 `test-topic-map-ui.cjs`；根持有新 migration 0120、topic core 模块、worker/read/store/lib 整合、PostgreSQL 测试、合同和治理文档。

## 验证与完成边界

先做可证伪业务样例：同义表达复用身份、开始与持续的边界、多标签、同名不同边界不误合、父评论上下文、作者与评论反例、低频可见、重复运行不增计数、长文尾段、Unicode 引用、来源撤回、定义变化和预算中断。适用 Rust/适配器/UI 测试，隔离合成 PostgreSQL 证明，独立复审与治理检查。

Design / Code / Automated checks 随真实结果更新；Real-chain proof（真实模型及中文语料质量）、Deploy、Mog/business acceptance 未发生前保持 NOT VERIFIED。任何真实敏感材料外发、生产写入、采集扩大或不可逆操作都超出当前实施授权。


## 闭环补齐与原生验证入口

独立复审覆盖同作作者/评论比较、跨作品比较、保存重开、来源限制传播、同文重新观察、部分结果和筛选计数。相应修复属于本包算法到前端闭环：新增 `topic_map_core/`、`topic_map_research/`、`topic_map_research_worker/`、`topic_map/core_read/` 子模块；topic 专用 comment/media/full frozen source seam；saved reader 同步绝对 Unicode 和精确当前资格。普通页面、采集额度与正式定义权限不扩展。

原生验证入口使用 `.github/workflows/topic-map-core.yml`，在该交付分支 PR 上运行固定 Rust 1.95.0、Node 适配器/UI，以及现有 `scripts/test-topic-map-postgres.sh` 的隔离 Docker PostgreSQL + pgvector proof。新 `topic_map_core_postgres.rs` 与 `tests/support/topic_map_core_fixture.rs` 是手写、纯合成测试源码，后者由原测试公共准备函数迁入；没有真实模型、平台或生产库访问。自动构建不表示合并、部署或业务验收。

本机的 PGlite WASM 仅补充证明完整 DDL 与单会话恢复/资格拒绝。其多个 SQLx 客户端共享 prepared statement 命名空间，不能替代原生事务/并发 gate；不修改生产连接池来绕过这一限制。

最后一轮复审增补 `topic_map_saved_sources_postgres.rs` 与 `topic_map_core_lifecycle_postgres.rs`：前者负责保存原文/定义依赖撤回和跨配置同文身份，后者负责有限重评授权、维护队列完成竞争及发送等待期间的来源撤回。Worker 完成逻辑与主题接纳分别拆入 `lifecycle.rs`、`acceptance.rs`，所有新增测试为手写合成源码，纳入同一隔离 PostgreSQL gate。

补充 `topic_map_core_unknown_postgres.rs` 与 `topic_map_core_legacy_postgres.rs`：前者以合成传输未知响应验证跨配置等待、暂停及来源改变均不允许盲目重发；后者保留真实旧 schema 的 machine_proposed 来源缺口，验证不可复核的机器定义不继续进入展示、prompt 或保存，人工裁定定义仍可用。不回填猜测的历史血缘。比较任务还在实际 policy/run 锁后重核候选快照，防止等待期间新讨论/定义被旧授权包接纳。

## 复审裁定与验证记录

已成立的复审发现均落到生产代码及相应负例：来源窗口与定义依赖的发送/接纳复核，已发 unknown 状态优先，跨配置语义范围防重复，原 run 有限授权与维护完成竞争，当前来源窗口最新版本计数，以及旧版机器定义来源缺口。相同来源的新表述只替换当前投影，历史保存仍按原引用读取。

复审曾提出“qualified catalog 与 raw catalog fingerprint 必然不一致”，核对发现准备阶段本来就使用 raw 版本，故该推断不成立；随后发现的实际问题是候选读取后的新时间戳可能给旧候选盖章，已改成读取候选前冻结版本，发送/接纳再核验。复审结论与修复均以实际代码和可证伪测试裁定。

本机全库 Clippy/函数边界检查存在基线违规，未通过完整 gate；诊断命令的 `--cap-lints warn` 只用于定位，不能作为通过声明。本包新增函数行数/参数数问题已拆分 helper，没有新增 `allow` 或放宽阈值，也没有顺带重构无关大文件。原生 PostgreSQL、远端 CI、合并、部署和真实模型质量分别记录实际回执。

| 提交前检查 | 实际结果 | 证明边界 |
|---|---|---|
| `cargo test -p linggan-intelligence topic_map --lib --locked` | 82/82 通过 | 身份、边界、来源窗口、状态与投影逻辑，含比较双侧可见性及原方法版本 |
| `cargo check -p linggan-intelligence -p linggan-api --locked` | 通过 | 当前组合入口及类型编译；已有 warning 未消除 |
| `node scripts/test-topic-map-ui.cjs` | 12 组通过 | 界面状态、来源定位、比较去重及实际保存参数 |
| `node --test apps/pi-adapter/test/structured-output.test.mjs` | 18/18 通过 | 真实 SDK 的合成传输合同，无 provider 外发 |
| Chrome 合成页面 | 15 状态，1440/390，30 GET；无页面错误或整页横向溢出 | 浏览器布局与交互，不证明真实数据库内容 |
| 新 native PostgreSQL 目标 `--no-run` | 编译通过 | 原生断言仍须由隔离 Docker CI 实际执行 |
| PGlite 全 schema | DDL 及 vector 检查通过 | WASM 单会话辅助证据，不替代原生事务/并发 |
| `check-project-governance.sh` / `git diff --check` | 通过 | 文件归属、索引与差异卫生 |

比较快照的手写回归子模块随既有 lifecycle target 执行，不增加新的运行入口或依赖。完整原生运行回执、独立 reviewer 的精确提交判断与 Draft PR 引用在 Issue #382 及本月进度继续登记。

## 原生 CI 比较结果可见性修正

Draft PR #383 首轮原生 CI `38037431631` 在提交 `a6af2aaa` 实际通过 23 项 PostgreSQL 测试，跨作品比较读取用例失败，后续 saved/search/budget/API 目标尚未执行；隔离容器与卷清理成功。静态复核确认比较结果只挂在按 UUID 排序的物理主作品，另一实际参与作品及筛掉主作品的视图会遗漏同一比较；旧测试同时依赖变量 `a` 和合并层结果引用，无法稳定验证该闭环。

本轮继续完成已有对照/保存合同：数据库保留单份比较结果；读取在所有实际 `selectedWorkRefs` 上投影，仍复核完整冻结来源与定义资格，不把仅请求但未入选的作品算作参与者。既有判断/产品机会列表按原始结果及条目索引去重，保存继续使用条目自己的结果与索引。范围为 `core_read` 读取、`topic_map.js` 的列表/保存引用和对应手写回归；页面布局、Token、Header、研究授权及采集范围不变。验收覆盖两参与作品、筛掉物理主作品、未入选作品、聚合去重、零索引保存以及评论限制后两侧派生结果撤回。

第二轮原生 CI `38038741350` 在 `08f48c20` 再次通过编译、82 项 Rust 单元、12 组 UI、18 项适配器和前 18 项 PostgreSQL 测试；research target 为 5 通过、1 失败。新增断言显示双作品新 run 的 accepted compare result 为 0，尚未进入双侧读取/保存断言，不能把静态修复或单元测试记为完整原生通过。隔离资源已清理。

继续诊断保持结果唯一性断言与 8192 的合成模型输入上限，失败时仅补 run/task 状态、拒绝原因和调用计数，不输出来源正文或 prompt。隔离脚本对既有选定目标逐一尝试并汇总非零退出码，以取得尚未执行的 saved/search/budget/API 回执；任何失败仍使 CI 失败，不增加目标、重试或外部调用权限。

同结构的实际 builder/restoration 纯探针确认，双作品三讨论即使只有 78 字原文，完整请求仍估计 8773 token；其中重复审计映射占 920。准备层仅在发送给模型的 coverage 副本中移除 `currentSources`、`sourceHashes`、`fragmentOrigins`，保留实际原文、Unicode 引用、片段所属作品、父评论与 Signal 血缘、范围及所有讨论。完整输入、身份 hash、冻结 request manifest 和读取/保存资格仍用原对象；探针对照为 7853，不变更模型、run 或 daily 预算。原生用例继续验证 8192 配置下的比较派发、双方保存与撤回，并核对账本中的三项审计映射完整存在。探针不能代替失败 CI 的 task 原因，诊断回执另记。

诊断 CI `38039550197` / `bb13f129` 已在真实原生账本确认比较任务 `failed / model_input_limit`，尝试、invocation、result 均为 0，输入上限仍为 8192。全部既有目标共 28 通过、4 失败：原比较及 saved_sources 的长文尾段、31+ 评论/父文、后页媒体三项均在来源处理阶段失败；search、comment budget、API 各 1 项通过。后者旧诊断没有保存 last_reason，不把同因推测写成事实；仅追加 phase/reason/attempt 安全诊断，保留原文范围和全部保存/撤回断言。三项审计映射精简同时作用于提炼与比较；尤其正文窗口仍须完整保留自己的片段，不能把完整来源的审计 map 反复塞入每个模型窗口。首轮发现的单侧读取漏洞是独立的已确认生产缺陷，不能以首轮 Null 推断当时已经有比较结果。容器和数据卷清理均成功；修复后的完整原生回执另记。

## 研究进度的真实失败与可用操作

`994adb8` 的原生 CI `38040007733` 实际完成 32 项，其中 29 通过、3 失败；输入上限修复后比较结果已接纳，31+ 评论/父文保存已通过，余下比较评论引用和正文/转录尾段引用断言继续定位。所有选定目标均已尝试，隔离资源清理成功；不能据此宣告完整通过。

这一回执同时暴露已获准研究进度的显示缺口：run 已结束、task 已失败时，页面可能仍显示历史 `comparison_queued` 原因；比较阶段虽在 API 中却未渲染；终态仍显示后端会拒绝的恢复操作。修正属于现有进度状态合同，不改变调度检查点、自动研究权限、重试策略或预算。

| 表面与依赖 | 状态词典与修正 | 验收 |
|---|---|---|
| 研究弹窗 → 既有 GET research → run/task 账本 | 新增有界 `taskIssues`，最多显示 20 项真实 failed/unknown_dispatch 的任务、阶段、状态及原因；总失败数仍完整；有失败时不拿历史排队原因解释 | 原生 completed + queued checkpoint + failed/model_input_limit 回归，实际失败计数及原因一致 |
| 既有研究阶段区 | 提炼、归属、比较各自显示真实待处理/进行中数量，未知仍显示未知 | 运行实际 JS 的阶段、失败原因、未知原因及转义回归 |
| 既有暂停/恢复/停止操作 | queued/running 允许暂停/停止，paused 允许恢复/停止，daily_budget_paused 保持等待额度语义并允许暂停/停止；completed/stopped/run_budget_exhausted 无无效操作 | 实际 JS 渲染按各状态检查动作，不改后端允许状态或发起请求 |

沿用前述 UI 读取回执、LIDS 组件及页面结构；这是状态表达与现有动作可用性的修正，不修改 Token、CSS、公共 Header 或新建页面。当前互斥所有权：进度协作者处理 `topic_map_research.rs` 的只读进度、`topic_map.js`、UI 测试、research native 回归及 HTTP 进度合同；根负责本计划、当月进度、最终集成及验证，其他复审只读分析来源失败。
