# TOPIC-MAP-CORE-001：主题图谱业务内核

> 状态: 活跃计划
> 最后核对: 2026-10-11
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

| 当前候选检查 | 实际结果 | 证明边界 |
|---|---|---|
| `cargo test -p linggan-intelligence topic_map --lib --locked` | 82/82 通过 | 身份、边界、来源窗口、状态与投影逻辑，含比较双侧可见性及原方法版本 |
| `cargo check -p linggan-intelligence -p linggan-api --locked` | 通过 | 当前组合入口及类型编译；已有 warning 未消除 |
| `node scripts/test-topic-map-ui.cjs` | 14 组通过 | 界面状态、来源定位、比较去重、实际保存参数及失败进度与终态操作 |
| `node --test apps/pi-adapter/test/structured-output.test.mjs` | 18/18 通过 | 真实 SDK 的合成传输合同，无 provider 外发 |
| Chrome 合成页面 | 15 状态，1440/390，30 GET；无页面错误或整页横向溢出 | 浏览器布局与交互，不证明真实数据库内容；后续失败进度修正由 JS 回归覆盖，未重复浏览器验收 |
| 原生 PostgreSQL + pgvector | 34/34 通过，CI `38041566274` / `32887e24` | 全部选定原生目标实际执行；隔离合成证明，不代表真实模型质量 |
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

## 隔离 PostgreSQL 就绪检查

第 5 次 CI [38041040402](https://github.com/Himog0921/linggan-intelligence/actions/runs/38041040402) / `c56ab3b` 已通过 API/core 编译、82 项 Rust、14 组 UI 和 18 项适配器测试；原生入口在第二次 `pg_isready`（当时脚本第 38 行）退出，34 项原生用例均未启动，治理步骤跳过。隔离容器和数据卷清理已核验。本轮没有容器启动日志，因此不能把具体启动竞态记为已确认根因。

两处原探针都未指定 host，会检查 Unix socket。[官方 PostgreSQL 镜像入口](https://github.com/docker-library/postgres/blob/master/docker-entrypoint.sh) 的 `docker_temp_server_start` 明确以 `listen_addresses=''` 启动初始化服务；探针可能把临时服务当作最终服务。修复仅在两处探针显式指定 `127.0.0.1:5432`，等待实际承接 proof 连接的 TCP 服务。保留 30 次有界等待、最终严格检查、全部选定原生目标、非零退出汇总和清理，不修改业务代码、配置预算或测试断言。完整原生结果由后续新提交 CI 另记。

## 最终开发候选回执

[Draft PR #383](https://github.com/Himog0921/linggan-intelligence/pull/383) 的代码提交 `32887e2404c814023c3fcd37e9441a5af44ea8b2` 对应独立已审树 `010f172dc295f80b22a393801b7ad9e07c2c1473`。第 6 次 [CI 38041566274](https://github.com/Himog0921/linggan-intelligence/actions/runs/38041566274) 于 2026-10-10 09:34 UTC 完成 success，根与独立协作者分别读取完整日志核验。API/core 编译、82 项 Rust 单元、14 组实际 JS、18 项结构化适配器、34 项原生 PostgreSQL 及治理/差异检查全部通过；09:34:26 UTC 明确记录隔离容器和数据卷清理成功。

| 原生目标 | 实际通过 |
|---|---:|
| core legacy | 2 |
| core lifecycle（含比较快照竞争） | 4 |
| core 归属与资格 | 6 |
| core unknown | 3 |
| topic map 与结构确认 | 3 |
| research（含两项新进度用例） | 8 |
| saved sources | 5 |
| 临时 search | 1 |
| evidence 评论预算 | 1 |
| API 回执/重放/来源守门 | 1 |
| **合计** | **34，通过；0 失败** |

前序失败的比较评论引用、正文尾段和完整 OCR/ASR 尾段保存均已实际通过；两项新进度用例也通过，分别证明真实输入超限后的失败明细与明确新 Start，以及 25 个失败/未知任务的完整总数和 20 项明细边界。保持原 8192 配置及严格引用、保存、撤回断言，未以提高预算或重试相同代码取得回执。

召回说明：BM25/CJK 与合格本地 WeMM 512 维向量以 RRF `k=60` 融合，常规召回每讨论取前 6，再形成调用批次的候选并集；当前归属准备实际逐条处理讨论，使新候选立即对下一条可见。定义重评还会补入必须复核的精确定义，因此“前 6”不是每次模型输入定义数量的永久上限。窗口、比较和重评均有范围上限，保留部分覆盖及未知状态。

| 完成层 | 本轮状态 |
|---|---|
| Design / Code | 已实现并独立复审 |
| 本包自动化与原生证明 | 上述代码提交全部通过 |
| 全库 Clippy / 旧函数边界 | 基线失败仍在，未宣称完整 gate 通过 |
| 真实模型、中文语料质量及大规模时延 | 未验证 |
| 生产迁移 / Deploy | 未执行 |
| Mog 业务验收 | 待实际验收 |

后续文档回执只描述本次已验证代码，不改变其业务实现、迁移、测试或预算。合并、部署和真实模型评测依据各自授权及实际回执，不能由本次合成 CI 推定。

## 2026-10-10 研究阻塞修复（Issue #382）

Mog 已要求修复实际研究进度核查发现的全部问题。基线为 main `d214cdc57f81a377f6161c6e6b2eaa03f5165787`；执行身份 `topic-map-research-recovery-382`，专属分支 `codex/topic-map-research-recovery-382`。已通过既有 pause 命令暂停 ADHD 自动研究，保留真实任务、预算和调用账本。

已确认：每 tick 新增批次与“最少调用次数”优先互相作用，使已提炼材料的归属任务饥饿；SSE 总字节 256 KiB 限制误拦截正常流；业务接纳丢弃具体拒绝原因；进度仅显示最近 30 批且混淆有效结果、无信号和材料不足；旧方法任务无法收尾。输入 hash 去重正常，不按重复入队缺陷处理。

实施与所有权：根负责 worker/queue 的背压、公平阶段调度、旧方法可审计收尾、接纳拒绝码及合成 PostgreSQL 回归、计划和最终集成。沿用 Mog 原交付包的并行授权，适配器协作者仅负责 Pi SSE 有界处理、Pi diagnostics 与适配器测试；进度协作者仅负责 research 读取投影、既有 JS 研究弹窗、UI 测试和 HTTP 进度合同；来源协作者仅负责逐任务来源准备的有限范围读取及其来源资格测试。协作者共享此专属工作树，不另开分支或提交。根最终提交前按 AGENTS 执行只读 commit-reviewer。

回归源码入口：根新增手写 `crates/intelligence/tests/topic_map_core_recovery_postgres.rs`，接入既有原生脚本/CI；来源协作者扩充既有 `crates/evidence/tests/topic_map_comment_budget_postgres.rs` 并收窄 `restoration.rs` 的结果来源复核读取，保持旧方法与比较的原始范围资格。生成物沿用既有 worktree target 和 `/tmp/topic-map-v41-*` 登记。

来源复核进一步收窄 `topic_map_core/source_qualification.rs` 至每个领域/配置实际概念规则创建来源的并集，仍传播完整定义依赖撤回；`comparison/queue.rs` 仅读已验证的请求作品范围。两处归来源协作者独占，本次原生撤回/未知保护证明继续覆盖。

验收必须覆盖：持续新来源下已有归属仍被调度；有 backlog 时不继续创建自动批次；旧方法 queued 安全停止且历史失败/未知不重发；合成完整提炼→归属→接纳→完成；正常大 SSE 与确实超限的负例；输出拒绝原因有界且不泄露原文；领域总进度与最近批次、阶段/无信号/材料不足/旧方法真实表达；缩小来源读取不放宽资格及窗口身份。保持 16,000 输入上限、2,000 输出上限、预算和补采授权，无新增真实模型调用、schema 迁移或插件变更。新 head 的合并、刷新 3000 和恢复真实研究依据 Mog 对具体结果的最终授权。

最终业务代码本机验证：API/core `cargo check --locked` 通过；主题 Rust 88/88、Pi 新诊断单测 1/1、实际 JS 16 组、完整 Pi 适配器 41/41、隔离 PostgreSQL + pgvector 39/39 全部通过，合成容器/卷清理已验证。最后来源资格并集读取落地后已完整重跑原生目标；不是以前树的回执。治理及差异检查通过。提交前 `recovery_commit_review` 独立实际重跑 88 项主题单测、16 组 UI、41 项适配器并复核全部业务差异，未发现阻塞缺陷；其报告时最终 native 尚在执行，最终 39 项与清理回执由根逐项核对。此前意外全库 rustfmt 的无关差异在独占工作树恢复，没有带入提交。

此验证只证明合成业务闭环及来源/并发边界；真实 provider 修复效果、中文主题质量、大规模时延和新部署尚未验证。此前已发生的 `invalid_output` 记录不回填猜测原因，不删除未知调用或零记费用。当前运行仍为 `d214cdc5`，策略仍暂停；本次修复交付不自动恢复外发。

远端首次 CI `38065677965`：编译、88+1 Rust、16 组 JS 和 40/41 适配器通过；超大 wire 负例先返回 `provider_timeout` 而非预期 `response_too_large`，未执行后续 native gate。该 fixture 原来发送一条多 MiB SSE 注释行，本机虽通过，却受不同 SDK/runner 行解析时延影响。改为有限长度的合法注释行，总 wire 字节仍严格超过授权上限，拒绝原因和 diagnostics 断言保持；不提高运行时/测试超时或弱化字节/文本/token 守门。业务源码及已验证的 native 树不变，补丁定向复审后再推送新 head。


## 主题质量与规模修复 / 2026-10-11

Mog 在运行审计后认可本包推进方案；原包 subagent 并行授权继续用于同一 Issue #382。新分支 `codex/topic-map-quality-382` 基于 `6feaaf60`。已通过产品 pause 命令暂停研究；保留全部材料、结果、费用与任务历史。#385 已部署，前文未部署表述仅为旧阶段回执。Claim 本地记录于此；公开 Issue 工程摘要等待披露授权，不阻塞本地实现。

目标：稳定概念和有边界的主题树取代单条现象顶层罗列；短来源成组并携带必要作品/父文上下文；有界合并重评与新材料公平推进；覆盖和维护分别计数。禁止硬编码 ADHD 树、相似度直接合并、删除账本/原声、次数门槛隐藏低频、提高预算替代效率修复。

业务术语：讨论单元保存具体陈述与原声；主题概念表示可持续研究的方向；父主题以范围包含子主题；重评重新判断既有讨论，不是初次覆盖。新主题和父节点保持机器候选；不自动发布正式定义、改写旧定义或移动旧正式主题。既有合并/拆分保留精确版本影响预览和确认。

重新核查 LLooM §3.1 的 distill / cluster / synthesize：综合相关样例形成概念、迭代抽象、回到文本判断；当前逐陈述升级主题不等于该机制。采用成组来源、多讨论归纳、明确领域相关性与可复用边界、候选比较与父子包含判断；复用本地 WeMM，不宣称复现论文指标。

同一 worktree 内互斥并行所有权：输入协作者持有 research/windows.rs 与 source/restoration 专用 helper、来源入队资格 research/admission.rs 及 research/tests/grouped.rs 成组回归（从既有大文件按职责拆出）和对应测试；调度协作者持有 core/backfill.rs、research_worker.rs/lifecycle、research read/summary 和对应原生测试；概念协作者持有 research_analysis.rs/子模块、core/acceptance.rs、topic_map/store.rs、新父子 helper、topic 专用 adapter schema 与对应测试；根负责文档、方法版本协调、API/前端整合、core/read 必要整合与验证入口。遇到依赖先相互报告，禁止改他人持有文件。源码/手写测试进入现有模块；生成物按 TOPIC-MAP-V41-001 临时路径，不提交敏感原文。

表面地图：既有 /topics 树、概览、主题边界与讨论、研究进度弹窗；共享页头/面包屑保持。状态词典：候选不是正式；领域/层级不足为未确定；旧方法候选明示边界；初次窗口、已覆盖作品、重评、比较分列；重评不增加覆盖；排队/等待/失败/未知依据回执。依赖地图：既有 LIDS、冻结窗口、精确定义版本、policy/budget/request ledger；无新模型、采集、插件或预算修改。分类为状态/语义+既有交互；根和 UI 实现者按 UI execution contract、design/LIDS/data-boundaries/language-policy/primitives/patterns 读取后实施。

验收矩阵：诊断同义询问复用路径主题；三年级实例不硬编码年级主题；启动/持续区分；相反体验同主题保留；泛泛评论/报名不强成领域主题；低频可见；有依据的父节点自动挂新候选，跨域/撤回/循环/旧版本拒绝；旧正式定义不变。100短评论成有界组、引用/父文正确；长文尾段保留；多定义重评合并且新材料实际获得派发；唯一初次覆盖不因重评上涨。使用手写业务样例、合成 adapter、隔离原生 PG、UI/桌面窄屏验证；真实模型语义质量单独报告。

停止点：不重发旧 unknown，不删除收费/结果历史。新方法旧 queued 替代策略有界、可审；最终 exact head 合并/部署和真实模型新实验按明确授权核对。合成通过不能写成专业主题树业务验收。

模块收口：新增 `topic_map_research/admission.rs` 将增量范围/未知排除从 queue 拆出，成组测试进入 `tests/grouped.rs`；概念质量测试进入 `topic_map_research_analysis/tests/concept_quality.rs`，新父绑定进入 `topic_map/store/parent_binding.rs`。均为上述对应协作者的同一职责，不新增数据库对象或文件长度豁免。

跨面复审：`comparison/tests/context.rs` 手写回归归输入协作者所有，验证上下文在范围归并、重建、冻结与恢复链保留角色，不充当独立作者回应或增加初次覆盖。父精确定义依赖进入比较冻结清单及通用依赖提取；不复制父说明或提议原文。原 36 评论单任务断言改为 6 个有界组及 36 个精确主评论身份全集，原保存重开/撤回判断仍执行。

### 本次开发候选最终验证

最终主题 Rust 单元 107/107、实际 JS 回归 18 组、完整 Pi 适配器 41/41、API/core `cargo check --locked` 均通过。最后完整原生执行为 `topic-map-v41-quality-postgres-freeze-5`，52/52 通过、0 失败，最终明确核验隔离 container/volume 清理；不是沿用前次失败树的回执。

| 原生目标 | 实际通过 |
|---|---:|
| 新成组输入、增量、未知保护 | 4 |
| core legacy / lifecycle / 概念及层级 / 调度质量 / unknown | 2 / 4 / 9 / 6 / 3 |
| core recovery / topic map | 3 / 3 |
| research / saved sources / search | 9 / 5 / 1 |
| evidence 评论预算 / API 回执与来源守门 | 2 / 1 |
| 合计 | 52 |

前序原生失败保留在本地回执：六评论组请求超限，修正为仅向模型发送必要原文与引用信息，完整审计映射仍冻结于服务器；合成预算下限与不可变时间条件按真实 schema 修正；保存测试从单评论任务转为六组后，指定非首 child 的角度引用必须显式选择。最后使用不可变的合成模型/config selector 给出精确 child 与父文引用，原 36 身份全集、非空 child 引用、保存重开及来源撤回断言均保留。未以提高实际输入/输出/预算或放宽生产资格取得通过。

独立 `quality_commit_review` 提交前复审 PASS。确认的两项跨面缺陷已修复：比较范围重建丢失作者上下文角色，及比较冻结遗漏父精确定义依赖。分别补充实际 build→restore→validate 反例与父定义变化/撤回后的拒绝；审核员还实际重跑指定非首 child 的合成 adapter 引用反例。最终 50 个非文档变化文件按排序路径与完整内容的 SHA-256 为 `583747deb88cb45f272f5fc49a482a49fed7a2f39a0159677c5e22733df1485b`。治理、差异和所属文件格式通过；全库 Clippy/既有函数边界基线失败仍在，无新豁免。

浏览器补充证明使用实际候选 JS 的树、讨论依据与研究计数渲染函数及现有 LIDS/CSS，数据全部为手写合成样例。桌面实际视口 1280px，document `clientWidth=scrollWidth=1280`；390×844 iframe 内 `clientWidth=scrollWidth=390`，历史候选 details 实际展开后可读，窄屏选择器可见。没有把未生效的视口设置称作 1440px，也不把静态合成控件称作真实 API/运行页交互验收。

交付状态：代码及上述合成/原生闭环已验证，可以形成本地提交；尚未推送、合并、部署或恢复真实研究。公开 GitHub 工程摘要待 Mog 披露授权；新 head 发布与恢复按确切结果取得授权。复用 schema 0120，不新增迁移，保留原 16,000 输入/2,000 输出、模型和每日/每轮预算。真实中文语义质量、专业主题树业务效果及规模吞吐尚未验证，不能由合成通过推定。
