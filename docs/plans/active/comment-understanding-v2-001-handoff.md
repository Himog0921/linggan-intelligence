# COMMENT-UNDERSTANDING-V2-001 · 交接：旧流程退役后续

> 状态: 活跃计划
> 最后核对: 2026-11-19（已接续至切换；PR #621 的最新提交以 `git log` 为准）
> 适用范围: 接手评论研究 V2 在同一 PR、同一分支上的剩余工作（P0、独立审阅及满足进入条件后的 G4/G6）
> 事实来源: [实施计划](comment-understanding-v2-001.md)（附录 A 改动清单、附录 B 的 G1 记录、附录 C 的 G2 记录）、[评论理解合同](../../data-contracts/comment-understanding.md)、分支 `claude/comment-understanding-v2` 的代码与测试、PR #621 的 CI
> 冲突时以谁为准: Mog 最新明确授权、AGENTS.md、真实运行/数据库副作用、当前代码/migration/测试；本文只是入口，不复制计划

本文写给**没有参与前期对话**的接手者。对话里的结论都已写进计划和本文；对话本身不可得。

## 0. 摘要与启动指令

**一句话：** 评论研究 V2 把"一条评论三次模型调用"改成"一个作品的若干评论一次调用"，需求（Problem）是稳定身份，候选/正式由有效作者数实时推导，合并只追加重定向。G0–G3 与 G5 已实现。G3 接通候选整理、撤销和 Opportunity。G5 接通现有评论研究工作台的 HTTP 与前端。切换候选通过迁移 0232 退役旧流程。Mog 已授权 3001 评论研究 V2 沿用证据库 L1 的 DeepSeek 安排启动真实运行。已完成一轮独立审阅，修复与复核见第 3 节。

**可直接粘贴给接手 agent 的启动指令：**

> 接手 COMMENT-UNDERSTANDING-V2-001。仓库 `Himog0921/linggan-intelligence`，分支 `claude/comment-understanding-v2`，PR #621（已合并）。先读 `AGENTS.md`，再读 `docs/plans/active/comment-understanding-v2-001-handoff.md`（从第 0 节开始），按它的读取顺序往下读。工作在 `.worktrees/` 下的独立 worktree 里做，不要在 `main` 的共享 checkout 里改任何东西。确认 PR 当前提交和 CI 后，按第 6 节核对切换证据与授权边界。第 9 节列出的事项必须停下来问 Mog，不要自行决定；已授权的 3001 评论理解外发和运营操作范围见第 9 节。每个阶段完成后更新 PR 正文和相关文档，已实现/已测试/已合并/已部署分开写。

## 1. 取得代码

```bash
git clone https://github.com/Himog0921/linggan-intelligence.git   # 已有检出则 git fetch origin
cd linggan-intelligence
git fetch origin claude/comment-understanding-v2
git worktree add .worktrees/comment-understanding-v2 claude/comment-understanding-v2
cd .worktrees/comment-understanding-v2
```

- 需要对该仓库的推送权限（GitHub 账号 `Himog0921`）。
- 基线 `origin/main@8891d718`；写本文时 `main` 没有前进。合并 `main` 前先 `ls database/migrations` 核对迁移编号。
- 本地需要：Rust（`rust-toolchain.toml`）、Docker（隔离 PostgreSQL，镜像 `linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1`，`scripts/runtime/build-pgvector-image.sh --ensure` 可构建）、Node（版本见 `.nvmrc`）。

**读取顺序：** `AGENTS.md` → [协作协议](../../governance/agent-collaboration.md) → [安全与领域约束](../../agents/safety-invariants.md) → 本文 → 计划第 2、3、5、6、8、9、10 节与附录 B、C → [评论理解合同](../../data-contracts/comment-understanding.md) → `database/migrations/0230_comment_understanding.sql` → `crates/intelligence/src/ai_foundation/comment_understanding/`。做前端时再读 `docs/governance/ui-design-execution.md` 和 LIDS（AGENTS.md 有路由）。

## 2. 当前状态

| 项 | 状态 |
|---|---|
| G0 基线确认、G1 合同与迁移、G2 单元执行 | 已实施，隔离 PostgreSQL 测试通过（计划附录 B、C） |
| 运行库迁移 | `55432/linggan_intelligence_dev` 已应用并校验 `0230–0233` |
| G3 候选整理与下游 | 已实现并通过隔离端到端，见第 6 节 |
| 评论理解 | 3001 已启用。绑定 L1 的 DeepSeek 配置，日预算 10,000,000 tokens。首批真实运行见第 6 节 |
| 候选整理 | 已注册，停用，未配置模型或预算 |
| 旧三阶段评论研究 | 处理器和接口已删除，三个旧任务已退役，三张旧阶段表已删除 |
| G5 HTTP 接口、前端 | V2 四视图和 AI 设置已部署到 3001 |
| 切换 | 已合并并部署，合并提交 `1efd52ab` |
| 独立审阅 | 首轮审到 `c90bae1c`，发现与修复见第 3 节 |
| PR #621 | 已合并，最终 CI 14 项通过 |

**代码地图**（`crates/intelligence/src/ai_foundation/`）：

| 文件 | 内容 |
|---|---|
| `comment_understanding.rs` | 模块根，公开接口一览 |
| `comment_understanding/start.rs` | `preview`、`start`（T0） |
| `comment_understanding/plan.rs` | 纯函数：`split`、`capacity`、`plan_hash` |
| `comment_understanding/target.rs` | 评论读取与资格 `read_in`；`current_source_in`（下游读取入口，V2 部分） |
| `comment_understanding/context.rs` | L1 卡 → 作品语境投影 |
| `comment_understanding/contract.rs`、`accept.rs` | 载荷与响应 Schema；单元接纳 `accept_unit` |
| `comment_understanding/publish.rs`、`unit.rs`、`catalog.rs` | 同事务写入；单元读取与派发记录；需求目录 |
| `comment_understanding/run.rs` | `read_run`、`cancel` |
| `comment_understanding/consolidation.rs` | 整理合同与 `accept_consolidation` |
| `comment_understanding/execution_tests.rs` | 8 个端到端测试（lib 测试） |
| `executor/service/comment_understanding.rs` | 执行器：准备、调用、结算 |
| `comment_understanding/consolidation_queue.rs`、`redirect.rs` | 整理调度、追加重定向和幂等撤销 |
| `executor/service/comment_problem_consolidation.rs` | 整理调用，派发配对与结果结算 |
| `executor/comment_understanding_order.sql` | 研究单元与整理任务共用同领域串行条件 |
| `comment_research.rs` 的 `current_source_in` | 按方法版本分发的下游读取入口 |

## 3. G2 推送后发现的 CI 失败（已修，需确认）

PR #621 在 G2 提交 `3424b542` 上有 3 个失败。修复提交 `396e17ee` 已验证 CI 13 项通过，包含此前失败的三项。三个失败由 G2 同一变更引入：

- `ai / foundation-contracts`：`opportunity_metrics_postgres` 的 `card::*` 整组失败；`web-real-chain / real-chain-core`：`opportunity_page_real_chain_browser_proof`；`Delivery gate` 是它们的汇总。
- 报错：`SourceCoverage(["…linggan_comment_research_result", "…_unit", "…_unit_target"])`。
- 原因：机会卡取快照时对 `opportunity/card/snapshot.rs` 的 `SOURCE_TABLES` 里每张表加锁，并在提交前审计"实际用到的表都在名单里"。G2 给评论目录的状态 SQL（`comment_study_catalog/facts.sql`、`history.sql`）加了 V2 分支，机会卡的计算会读到这三张表，名单里没有。**这组测试 G2 本地没跑**，所以漏了。
- 修复：把这三张表加进 `SOURCE_TABLES`（按字母序）。
- 本地验证（修复后）：`opportunity::card::snapshot::tests::` 与 `groups::tests::` 5/5；`opportunity_metrics_postgres` 47/48，唯一失败的 `card_uses_canonical_metrics_debounces_and_removes_withdrawn_voices` 是 `SourcesBusy`（取快照用 `LOCK TABLE … NOWAIT`，满负载下的锁时序），**单独连跑 3 次都通过**，判断为偶发，但以 CI 为准；`local_web::opportunity_api::tests::enabled_routes_return_cards_and_guard_changed_pages` 通过。浏览器链路 `opportunity_page_real_chain_browser_proof` 本地没跑（需要浏览器），同一根因，靠 CI 确认。

**教训：** 凡是改了评论目录、评论研究表或它们的读取 SQL，推送前必须跑机会卡全套：`scripts/test-opportunity-postgres.sh` 里的 `opportunity::card::snapshot::tests::`、`opportunity::card::groups::tests::`、`opportunity_metrics_postgres`（先 `cargo build -p linggan-worker --bin linggan-opportunity` 并设 `OPPORTUNITY_TEST_WORKER_BIN`）和 `linggan-api` 的 `local_web::opportunity_api::tests::enabled_routes_return_cards_and_guard_changed_pages`。G3 首次为领域建立机会卡时也发现同类缺口，已登记分类版本外键访问的 `observation_domain`。

### 切换检查回归

切换首次提交 `485d9b0e` 的 Core 检查仍请求旧研究路由。修复后验证现行 V2 路由的 JSON 错误响应，以及旧路由的 404。Opportunity Clippy 检查发现迁移后的来源门测试有未使用的导入。删去该导入，保留两项来源门证明。两个修复都只涉及测试与交接记录。失败日志见 [Delivery 44696451925](https://github.com/Himog0921/linggan-intelligence/actions/runs/44696451925)。

### 独立审阅与合并条件

已独立审阅基线 `8891d718` 到候选 `c90bae1ce17ed77b5a4b709b097ccaff87773ed6`。发现定义撤回未传播、跨评论命题检查缺失、在途取消合同冲突、失败重跑遮蔽主题挂靠，以及接纳和单元回执的边界问题。Mog 于 2026-11-20 确认沿用共享取消规则。已经发送的调用仍结算用量，取消后阻止发布。首轮审阅未满足合并条件，修复后复核增量并确认最终提交授权。
修复新增 `0233` 复核需求定义来源，保留历史身份，屏蔽现行需求、合并历史和机会卡中的受限定义。仍有合并可撤销时保留屏蔽后的入口。在途模型使用过刚撤回的目录时只结算用量。接纳器拒绝命题复用他评完整原文或派发清洗文本，重复新键失效，零接纳的单元标为失败。主题挂靠默认读最近成功结果，显式失败请求仍可查看。

## 4. 保护车道 Claim

| 项 | 内容 |
|---|---|
| 工作包 | COMMENT-UNDERSTANDING-V2-001（Mog 2026-11-19 确认，无 Issue） |
| base / 分支 | `8891d718` / `claude/comment-understanding-v2` |
| exclusive | 计划附录 A.2 列出的评论研究文件、迁移 `0230` 及其后续迁移、`comment_understanding` 模块、对应测试与文档 |
| 共享，改前先看并行改动 | `topic_map_research`、`topic_comment_source`、`opportunity`、`comment_study_catalog` 中读取评论研究结果的查询 |
| forbidden | 其他工作包的文件；运行库写入；部署 |
| 升级条件 | 第 9 节 |

## 5. 已确认的决定，不要重开

L1 是评论研究必需上下文，不降级；同一领域串行；候选整理自动执行、合并可撤销；跨模型抽查由 Mog 上线后事后进行；**先完整落地上线，再优化质量**（上线门槛是程序不变量和端到端运行，不是质量指标）；P0 和 V2 沿用 L1 的 `model_config_ref`（`deepseek-flash`，thinking high，输入 64k、输出 40k）；旧流程直接退役，不写兼容层；判定"是不是同一需求"交给 LLM，向量只做宽松排序、默认路径不用向量（计划 §1.3 有实验数字：余弦在"同话题不同问题"上 AUC 仅 0.74）；拿不准就新建需求（重复可合并，误合并难拆）；候选/正式不存储，由视图推导。

## 6. 剩余工作、顺序与完成标准

建议顺序为 **切换 → G4/G6**。3001 已部署，先完成已授权的首批真实运行。

### G3：候选整理与下游（已实现）

- `0231_comment_problem_consolidation` 注册整理任务，允许自动和交互批次。任务初始化为停用、预算未配置、模型未绑定。整理须有 `comment_material` 外发许可。请求只含需求定义和短别名。
- Mog 已确认：每次研究运行全部单元结束后，如果本次新建过候选，就排一次整理。以来源批次唯一约束幂等，不做领域级去抖。同一领域的研究单元和整理共享串行领取。
- 全量整理按创建顺序给出候选和正式需求。上限为 150 个候选、200 个需求。派发前保存 `pairKey → [problem_ref, problem_ref]` 对照。只有 `same` 可写重定向；正式优先、作者多优先、创建早优先。同一命令可重放撤销回执。
- Opportunity 的 V2 来源是 `linggan_comment_problem_support`。提及保留原始成员键和来源调用，重定向、撤销、更新研究结果或来源受限时按有效支持过滤。V1 路径在切换前仍服务原任务。研究单元和整理成功请求刷新，撤销由现有周期刷新重算。
- 隔离端到端 8/8。新增用例验证跨作品候选只整理一次，`narrower` 保持独立，两个作者的同一需求转正式，机会卡作者数为 2，旧卡链接指向存活需求。幂等撤销后两张卡各恢复为 1 位作者。机会卡快照 4/4、分组 1/1、周期索引调度 1/1、指标 48/48、API 证明 1/1。预检通过。较早一轮指标运行有一次 `SourcesBusy`，最终整轮通过。本地未跑机会卡浏览器证明，CI 回执见 PR #621。
- 未合并、未部署、未做生产验证。真实模型、真实数据规模和独立审阅尚未验证。

### G5：HTTP 接口与前端（已实现）

- `/api/local/comment-research/v2` 提供批量预览、启动、运行回执与取消、当前单条结果、概览、需求列表与详情、手动整理和撤销。旧 HTTP 保留至切换阶段。
- 工作台沿用评论目录、概览、研究记录和用户问题四视图。预览列出缺 L1 的作品，复用现有内容理解预览与启动。补齐后由运营重新预览。研究记录区分受理、等待、执行、取消和结果，失败评论可重选预览。需求卡显示候选、正式及支持数，详情保留原声与已接通的主题挂靠。
- 作品作者未知的评论可以研究，确认是创作者本人的评论继续排除。Opportunity 继续按自身的证据资格处理作品作者未知。
- 已用桌面浏览器验证隔离库、Axum、React 与回环提供方的操作链路。覆盖选择、补 L1、启动、取消在途单元、正式需求与原声，以及从主题挂靠回到评论。本地机会卡快照 4/4、分组 1/1、指标 48/48、API 1/1。Web 核查 1,797 项通过。提交 CI 以 PR #621 的回执为准。尚未做真实运行与 Mog 业务验收。

### 切换（候选已实现）

- 新迁移 `0232` 退役旧三个评论任务。旧开关通过命令和控制事件关闭，排队项结算为已取消；调用、配置与控制历史保留。三张旧阶段表必须为空，迁移才会删除。2026-11-19 的历史只读核对结果为 0 行，实际应用时还会由迁移检查。
- 删除旧任务处理器、HTTP 和前端配置入口。评论目录历史查询只连接 V2 研究单元，主题研究只选择 V2 结果，机会卡直接读取 `linggan_comment_problem_support`。历史调用仍可查询。
- 隔离迁移证明覆盖全量建库、重复迁移、三个旧任务各一条排队项、非空阶段表拒绝及此前 L1 升级记录保留。前端类型、Lint、1,796 项测试和构建通过。评论研究与主题挂靠桌面证明 3/3，AI 设置和机会卡桌面证明各 1/1，运营鉴权 11/11。主题执行及 V2 未决调用证明 8/8。
- 评论目录与执行证明 44/44，其中 V2 端到端 9/9。机会卡快照 4/4、分组 1/1、周期索引 1/1、指标 48/48、API 1/1，提及投影 6/6。主题挂靠源读取 1/1，已保存主题读取 5/5。预检通过。
- 独立审阅和最终 CI 均通过，PR #621 已合并。3001 已备份并应用 0230–0233，已核对页面、接口和执行器版本。详见[研究自动化执行合同](../../data-contracts/research-automation.md)。

### P0：提示词冒烟

G2 已能端到端运行，P0 现在可以直接用 `preview`/`start`。首批选择 1 个已有 L1 的作品和 10 条通过研究准入的评论，先验证派发、接纳和用量。 步骤：用任务配置流程把 `comment_understanding` 绑定到 L1 的 `model_config_ref`（迁移里没写，因为那是生产库里的记录）、开启开关、批准外发许可；选 2–3 个有 L1、评论 10–40 条的作品和 1 个超过 100 条的作品；单元大小试 10、20、30；记录截断率、输出长度、费用；把定稿的提示词存为新的提示词版本（**不要改迁移**）；写一份简短记录（模型、调用次数、费用、发现的合同问题）。不是质量门槛，只用来定提示词和 `UNIT_MAX_TARGETS`。

首批真实运行记录（2026-11-20）：1 个作品、10 条评论、1 个单元。首轮提供方返回 HTTP 200 和 JSON，整单元因 `unit_output_invalid` 未接纳，用量已结算。通过设置新增提示词 v2，在指令中明确顶层字段和固定合同标识。重跑批次 `b90c5081-aa94-4bf8-a2c3-74b6aa925ffb` 已接纳 10 条，3 条有信号，6 条需要更多语境，1 条无信号，无新需求候选。重跑用量为输入 2,185、输出 2,729 tokens。多作品及大单元的完整 P0 矩阵仍未执行。

### G4 / G6

向量（只在计划 §8 G4 的进入条件满足时）；上线后的评估（计划 §7.2–7.5，由 Mog 事后抽查）。

### 上线条件（计划 §8）

G0–G3、G5 完成并合并已部署；在一个真实领域上端到端跑通一次且 §7.1 的不变量为 0；旧流程已退役。

## 7. 容易踩的坑

- **新增读取评论研究结果的代码**不要用 `frozen_in`（V1 的冻结输入复核，会静默拒绝 V2 请求），也不要按"同批次条目是否成功"判断成功；用 `comment_understanding::current_source_in` 和"该请求有成功结果行"。G2 已改过 4 处这样的读取。
- **机会卡的表名单**（第 3 节）。任何新增的、会被机会卡计算读到的表都要登记。
- **信号序号**是信号在已存储 `signals[]` 里的下标（0–11），与 `membership.signal_ordinal` 一致。
- **迁移不可改**：`0230` 一旦在任何共享环境应用就不能再改（校验和账本在 `scripts/local-runtime.sh` 和 `apps/api/src/local_web/full_schema_fixture.rs`）；在那之前改 `0230` 要同步夹具里的校验和。新增迁移要同步这两处。提示词是草案，改提示词用新的提示词版本。
- **别名**：模型只看到 `p<n>`、`t<n>`；T2 靠 `unit_dispatch.catalog_problem_refs`（有序）还原，派发时在调用前写入。
- **适配器把长度截断报告为失败的调用（`output_limit`）**，不是较短的回答；判断截断要同时看失败码和 `finish_reason`。
- **L1 作品语境投影**在 `comment_understanding/context.rs`；L1 卡结构变化时先改这里和它的测试。
- **准入**：G5 已获准处理作品作者未知的评论。能确认来自创作者本人的评论仍排除。
- 别在 `main` 的共享 checkout 里工作；那里有他人未提交的改动。

## 8. 验证配方

- 隔离 PostgreSQL：`scripts/test-comment-study-productization-postgres.sh`（CI 会跑；已包含 `comment_understanding_schema_postgres` 和 lib 的 `comment_understanding::execution_tests`）。它启随机容器和卷并清理。**不要对 `linggan-intelligence-postgres-1`（端口 55432，运行库）做写入**，只允许 `SET default_transaction_read_only=on` 的只读查询。
- 端到端测试是 **lib 测试**，复用 `tests/support` 夹具靠 `lib.rs` 里仅测试用的 `extern crate self as linggan_intelligence;`。提供方的回复由评论文本决定（`tests/support/comment_understanding_response.rs`）：写一条评论就写了一种情形。
- 新 worktree 里跑依赖 Node SDK 的测试需要 `apps/pi-adapter/node_modules`：`scripts/runtime/prepare-pi-adapter.sh --install`，或在 `package-lock.json` 与主检出相同时临时符号链接。符号链接用完删除：`.gitignore` 只忽略目录，符号链接会显示为未跟踪文件，别提交。
- 环境变量：`AI_FOUNDATION_PROOF_NODE`（`node` 路径）；`ai_foundation_postgres` 还要 `LINGGAN_CREATION_ASSET_ROOT`；用合成图片的夹具要 `LINGGAN_LOCAL_MEDIA_ROOT`（二者都是自己创建的临时目录）；机会卡测试要 `LINGGAN_SUPPORT_DIR` 下有 `opportunity/commenter-digest.key`（`openssl rand 32`，权限 600）。缺这些时的失败是环境问题。
- **基线上同样失败、与本包无关的测试（不要当成你的回归，也不要顺手改）：** `linggan-api` 的 `creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work`（503）；`content_understanding::topic_tests::saved_core_drives_topic_queue_and_persisted_membership`（`topic_research_automation/tests.rs:29`，期望 4 个阶段、实际 1 个）。
- 提交前：`bash scripts/ci-preflight.sh origin/main`；新增文件用 `rustfmt --edition 2024` 单独格式化（`cargo fmt --check` 会报仓库里原有文件的差异）。
- 推送后看 CI：`gh pr checks 621 --repo Himog0921/linggan-intelligence`，失败的任务用 `gh run view <run> --job <job> --log-failed` 读。CI 会跑本地没跑的套件，**以 CI 为准**。

## 9. 必须停下来问 Mog 的事项

- 更换模型、扩大外发材料范围或变更既定预算。3001 的评论理解已获准沿用 L1 的 DeepSeek 配置，日预算为 10,000,000 tokens。通过研究准入的合规公开评论及所需作品语境按任务登记外发许可后，可通过工作台发起真实研究。
- 向本机运行库应用迁移、重启运行时、任何部署。
- 合并（保护车道的合并授权针对**最终 head**）。
- 创建外部 Issue；安排独立审阅子代理（协作协议要求候选完成后审阅一次，需要授权）。
- 改变第 5 节的决定或产品语义。

**G5 已确认的决定与剩余授权**

- 作品作者未知时允许研究。能与评论者身份确认相同的创作者本人评论继续排除。未知身份仍保留为未知。
- 评论预览为缺 L1 的作品提供补齐入口，调用现有内容理解预览与启动命令。L1 就绪后由运营重新预览评论，再决定是否发起评论研究。
- Mog 已确认首批真实运行及正常评论研究流程。首批仅启用评论理解。

## 10. 本机残留（只与写本文的那台机器有关）

- `~/proma/linggan-intelligence/.worktrees/claude-cu-v2-baseline-check`：为对照基线失败临时建的 worktree，不在 git 里，可 `git worktree remove` 删除（清理被安全检查拦下，没有强行删除）。
- 会话 scratchpad 里的 `baseline-target/`（编译产物）和 `$TMPDIR/linggan-cu-*`（测试用临时目录）：可删。
- 没有残留的 Docker 容器或卷（运行器退出时清理；`docker ps -a | grep linggan-cu-proof` 应为空）。

## 11. 每个阶段的交付要求

候选完成后安排一次独立审阅并关联实际提交（需授权）；更新 PR 正文里的验证、未验证事项和审阅记录；文档按事实变化同步（合同改变更新 `data-contracts/comment-understanding.md`，切换时改写 `research-automation.md`，在 `docs/progress/2026-11.md` 记里程碑）。已实现、已测试、已合并、已部署、生产已验证分开写，不互相推断。
