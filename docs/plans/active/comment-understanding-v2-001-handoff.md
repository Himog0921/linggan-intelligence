# COMMENT-UNDERSTANDING-V2-001 · 交接：从 G3 继续

> 状态: 活跃计划
> 最后核对: 2026-11-19（本文写于 G2 之后；PR #621 的最新提交以 `git log` 为准）
> 适用范围: 接手评论研究 V2 在同一 PR、同一分支上的剩余代码工作（P0、G3、G5、切换、G4/G6）
> 事实来源: [实施计划](comment-understanding-v2-001.md)（附录 A 改动清单、附录 B 的 G1 记录、附录 C 的 G2 记录）、[评论理解合同](../../data-contracts/comment-understanding.md)、分支 `claude/comment-understanding-v2` 的代码与测试、PR #621 的 CI
> 冲突时以谁为准: Mog 最新明确授权、AGENTS.md、真实运行/数据库副作用、当前代码/migration/测试；本文只是入口，不复制计划

本文写给**没有参与前期对话**的接手者。对话里的结论都已写进计划和本文；对话本身不可得。

## 0. 摘要与启动指令

**一句话：** 评论研究 V2 把"一条评论三次模型调用"改成"一个作品的若干评论一次调用"，需求（Problem）是稳定身份，候选/正式由有效作者数实时推导，合并只追加重定向。G0–G2 已完成（合同、迁移、规划、执行、读取），现在**没有任何入口调用它**，运行中的产品行为没有变化。剩下：候选整理与下游（G3）、接口与前端（G5）、旧流程退役（切换）、真实模型冒烟（P0，需 Mog 授权）。

**可直接粘贴给接手 agent 的启动指令：**

> 接手 COMMENT-UNDERSTANDING-V2-001。仓库 `Himog0921/linggan-intelligence`，分支 `claude/comment-understanding-v2`，PR #621（Draft）。先读 `AGENTS.md`，再读 `docs/plans/active/comment-understanding-v2-001-handoff.md`（从第 0 节开始），按它的读取顺序往下读。工作在 `.worktrees/` 下的独立 worktree 里做，不要在 `main` 的共享 checkout 里改任何东西。先做第 3 节（确认 CI 修复已生效），再按第 6 节顺序推进 G3。第 9 节列出的事项必须停下来问 Mog，不要自行决定；尤其不要调用真实模型、不要对运行库写入、不要合并或部署。每个阶段完成后更新 PR 正文和相关文档，已实现/已测试/已合并/已部署分开写。

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
| 迁移 `0230` | 只在隔离测试库应用过；**未应用到本机运行库**（`55432/linggan_intelligence_dev`，迁移头 `0229`），未部署 |
| 新任务 `comment_understanding` | 已注册，停用，未配置模型和预算；**从未调用过真实模型** |
| 旧三阶段评论研究 | 仍是运行中的流程，代码和测试都在 |
| HTTP 接口、前端 | 没有任何入口调用 V2 |
| 独立审阅 | 尚未进行 |
| PR #621 | Draft；CI 见第 3 节 |

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
| `comment_understanding/consolidation.rs` | 整理合同与 `accept_consolidation`（**尚无调用方**） |
| `comment_understanding/execution_tests.rs` | 8 个端到端测试（lib 测试） |
| `executor/service/comment_understanding.rs` | 执行器：准备、调用、结算 |
| `executor/comment_understanding_order.sql` | 同领域串行的领取条件 |
| `comment_research.rs` 的 `current_source_in` | 按方法版本分发的下游读取入口 |

## 3. G2 推送后发现的 CI 失败（已修，需确认）

PR #621 在 G2 提交 `3424b542` 上有 3 个失败，**根因相同，是 G2 引入的**：

- `ai / foundation-contracts`：`opportunity_metrics_postgres` 的 `card::*` 整组失败；`web-real-chain / real-chain-core`：`opportunity_page_real_chain_browser_proof`；`Delivery gate` 是它们的汇总。
- 报错：`SourceCoverage(["…linggan_comment_research_result", "…_unit", "…_unit_target"])`。
- 原因：机会卡取快照时对 `opportunity/card/snapshot.rs` 的 `SOURCE_TABLES` 里每张表加锁，并在提交前审计"实际用到的表都在名单里"。G2 给评论目录的状态 SQL（`comment_study_catalog/facts.sql`、`history.sql`）加了 V2 分支，机会卡的计算会读到这三张表，名单里没有。**这组测试 G2 本地没跑**，所以漏了。
- 修复：把这三张表加进 `SOURCE_TABLES`（按字母序）。**接手后第一件事：确认该修复已在分支上并已推送，重新看 CI。**
- 本地验证（修复后）：`opportunity::card::snapshot::tests::` 与 `groups::tests::` 5/5；`opportunity_metrics_postgres` 47/48，唯一失败的 `card_uses_canonical_metrics_debounces_and_removes_withdrawn_voices` 是 `SourcesBusy`（取快照用 `LOCK TABLE … NOWAIT`，满负载下的锁时序），**单独连跑 3 次都通过**，判断为偶发，但以 CI 为准；`local_web::opportunity_api::tests::enabled_routes_return_cards_and_guard_changed_pages` 通过。浏览器链路 `opportunity_page_real_chain_browser_proof` 本地没跑（需要浏览器），同一根因，靠 CI 确认。

**教训：** 凡是改了评论目录、评论研究表或它们的读取 SQL，推送前必须跑机会卡全套：`scripts/test-opportunity-postgres.sh` 里的 `opportunity::card::snapshot::tests::`、`opportunity::card::groups::tests::`、`opportunity_metrics_postgres`（先 `cargo build -p linggan-worker --bin linggan-opportunity` 并设 `OPPORTUNITY_TEST_WORKER_BIN`）和 `linggan-api` 的 `local_web::opportunity_api::tests::enabled_routes_return_cards_and_guard_changed_pages`。G3 要给机会卡改读视图，会再次碰这里。

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

建议顺序：**确认第 3 节 → G3 → G5 → 切换 →（P0 随 Mog 授权并行）→ G4/G6**。每一步都保持提交可验证。

### G3：候选整理与下游

- 整理任务：注册 `comment_problem_consolidation`（照 `0230` 里 `comment_understanding` 的写法，新迁移编号以届时 `database/migrations` 为准）；`InputRef` 增加整理输入；`BatchKind` 允许 `interactive` 和 `automatic`；派发表记录被问的配对（`pairKey` → 两个 `problem_ref`）；执行器分发。合同和接纳函数已有（`consolidation_schema()`、`accept_consolidation()`），重定向写入由 `0230` 的触发器校验。
- 配对生成：G3 用全量模式——候选总数不超过 `CONSOLIDATE_FULL_MAX`（150）时把全部需求交给模型找重复；向量排序属于 G4。
- 触发：一次运行的所有单元结束后，若产生了新候选就排一个整理任务（`read_run` 的单元状态可判断运行结束）。
- 存活方：正式优先、作者多优先、创建早优先，由程序决定。**只有 `same` 写重定向**，`narrower` 不合并。
- 撤销：`0230` 有撤销表但**还没有撤销函数**，要补（命令幂等，写 `linggan_comment_problem_redirect_revocation`）。
- Opportunity：`opportunity/mentions.rs` 约 137 行用 `membership.review_invocation_ref` 做内连接，**V2 的支持行被静默漏掉**，改读 `linggan_comment_problem_support`；核对 `linggan_opportunity_comment_mention` 对 `membership(request_ref, signal_ordinal)` 的外键、`linggan_opportunity_sync_problems` 函数和 `opportunity/card/snapshot.rs` 的表名单。
- T4 刷新：`executor/service.rs` 的 `fill_slots` 只在 `CommentProblemPairReview` 成功后调用 `opportunity_sync.request()`；V2 要在单元成功和整理成功后触发。先读 `executor/service/opportunity_sync.rs`。
- **完成标准：** 跨单元的重复候选被合并后，作者合计达到 2 位的自动成为正式需求；撤销后状态恢复；机会卡的作者数与 `linggan_comment_problem_current` 一致；第 3 节列的机会卡全套测试通过；有对应的端到端测试（仿 `execution_tests.rs`，提供方回复由评论文本决定）。

### G5：HTTP 接口与前端

- 现有 V1 路由在 `apps/api/src/local_web/comment_study_command_api.rs`（`/api/local/comment-research/*`）。V2 接口草案（可调整，但函数都已存在）：`POST …/preview`（选择列表较长，用 POST）→ `preview`；`POST …/runs` → `start`；`GET …/runs/{batch}` → `read_run`；`POST …/cancel` → `cancel`。
- **缺的读取：** 需求列表与详情（读 `linggan_comment_problem_current`、`linggan_comment_problem_support`，含代表性原声、合并历史、撤销入口）还没有读取函数，V1 的 `comment_problem::read_problems` 读的是旧表，要新写。
- 前端文件见计划附录 A.2（`apps/web/src/pages/corpus/comments/` 下 10 个文件，加设置页的任务列表）。UI 工作必须按 `AGENTS.md` 的路由读 UI 执行标准与 LIDS，不要凭感觉设计。
- 九类标签沿用中文名；执行状态、研究结果、需求状态分开显示；"候选等待第二位作者"是正常状态，不是失败（计划 §7）。
- **完成标准：** 运营能在评论目录选评论、看预览（含"L1 未就绪"并能触发补 L1）、启动、看进度与失败原因、取消；需求列表能看候选/正式、作者数、原声；桌面端对照现有页面不退化；前端检查与浏览器场景通过。

### 切换

- 一个迁移：把 `comment_study`、`comment_problem_resolution`、`comment_problem_pair_review` 设为 `retired`（照 `0229` 的做法：关开关、取消排队项），删空的旧 stage 表（`linggan_comment_research_stage`、`linggan_comment_problem_dispatch`、`linggan_comment_problem_stage_result`）；同一提交删旧代码（`comment_research.rs` 的 V1 部分、`comment_problem.rs` 及其子模块、`executor/service/comment_research.rs`、`executor/service/comment_problem.rs`、旧路由与前端入口、旧测试）。**生产库的旧评论研究表为 0 行**（2026-11-19 只读核对），不写兼容层。
- 改写 `docs/data-contracts/research-automation.md` 的评论部分，把旧文档标为历史；更新 `docs/README.md`、`docs/progress/2026-11.md`。
- `current_source_in` 的 V1 分支、`frozen_in` 随旧代码删除。
- **完成标准：** 没有代码引用旧三个任务和旧表；全套测试通过；`scripts/ci-preflight.sh` 通过。

### P0：提示词冒烟（需要 Mog 授权）

G2 已能端到端运行，P0 现在可以直接用 `preview`/`start`。**前提见第 9 节。** 步骤：用任务配置流程把 `comment_understanding` 绑定到 L1 的 `model_config_ref`（迁移里没写，因为那是生产库里的记录）、开启开关、批准外发许可；选 2–3 个有 L1、评论 10–40 条的作品和 1 个超过 100 条的作品；单元大小试 10、20、30；记录截断率、输出长度、费用；把定稿的提示词存为新的提示词版本（**不要改迁移**）；写一份简短记录（模型、调用次数、费用、发现的合同问题）。不是质量门槛，只用来定提示词和 `UNIT_MAX_TARGETS`。

### G4 / G6

向量（只在计划 §8 G4 的进入条件满足时）；上线后的评估（计划 §7.2–7.5，由 Mog 事后抽查）。

### 上线条件（计划 §8）

G0–G3、G5 完成并合并已部署；在一个真实领域上端到端跑通一次且 §7.1 的不变量为 0；旧流程已退役。

## 7. 容易踩的坑

- **新增读取评论研究结果的代码**不要用 `frozen_in`（V1 的冻结输入复核，会静默拒绝 V2 请求），也不要按"同批次条目是否成功"判断成功；用 `comment_research::current_source_in` 和"该请求有成功结果行"。G2 已改过 4 处这样的读取。
- **机会卡的表名单**（第 3 节）。任何新增的、会被机会卡计算读到的表都要登记。
- **信号序号**是信号在已存储 `signals[]` 里的下标（0–11），与 `membership.signal_ordinal` 一致。
- **迁移不可改**：`0230` 一旦在任何共享环境应用就不能再改（校验和账本在 `scripts/local-runtime.sh` 和 `apps/api/src/local_web/full_schema_fixture.rs`）；在那之前改 `0230` 要同步夹具里的校验和。新增迁移要同步这两处。提示词是草案，改提示词用新的提示词版本。
- **别名**：模型只看到 `p<n>`、`t<n>`；T2 靠 `unit_dispatch.catalog_problem_refs`（有序）还原，派发时在调用前写入。
- **适配器把长度截断报告为失败的调用（`output_limit`）**，不是较短的回答；判断截断要同时看失败码和 `finish_reason`。
- **L1 作品语境投影**在 `comment_understanding/context.rs`；L1 卡结构变化时先改这里和它的测试。
- **准入沿用 V1 来源门**：创作者本人的评论排除，作品作者未知的评论也排除；只放开了"评论者作者未知"。
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

- 真实模型调用（P0 以及任何会外发评论的运行），以及发给第三方模型前的目的、最小样本、输出边界和处置计划（评论含 ADHD、儿童内容，见 safety-invariants 和 DEC-0006）。
- 启用 `comment_understanding`、配置预算、绑定模型。
- 向本机运行库应用迁移、重启运行时、任何部署。
- 合并（保护车道的合并授权针对**最终 head**）。
- 创建外部 Issue；安排独立审阅子代理（协作协议要求候选完成后审阅一次，需要授权）。
- 改变第 5 节的决定或产品语义。

**当前开放的问题（需要 Mog 回答）：**

1. 作品作者未知的评论要不要放开准入？（现在排除；影响可研究评论的数量。）
2. 整理任务按"每次运行结束后一次"触发是否足够，还是需要按领域去抖？（计划写的是每次运行后。）
3. 评论目录里"补 L1"入口要复用现有 L1 自动化还是单独做？（本机只有约 41% 的评论所属作品有 L1。）
4. P0 的授权、样本和外发处置计划。

## 10. 本机残留（只与写本文的那台机器有关）

- `~/proma/linggan-intelligence/.worktrees/claude-cu-v2-baseline-check`：为对照基线失败临时建的 worktree，不在 git 里，可 `git worktree remove` 删除（清理被安全检查拦下，没有强行删除）。
- 会话 scratchpad 里的 `baseline-target/`（编译产物）和 `$TMPDIR/linggan-cu-*`（测试用临时目录）：可删。
- 没有残留的 Docker 容器或卷（运行器退出时清理；`docker ps -a | grep linggan-cu-proof` 应为空）。

## 11. 每个阶段的交付要求

候选完成后安排一次独立审阅并关联实际提交（需授权）；更新 PR 正文里的验证、未验证事项和审阅记录；文档按事实变化同步（合同改变更新 `data-contracts/comment-understanding.md`，切换时改写 `research-automation.md`，在 `docs/progress/2026-11.md` 记里程碑）。已实现、已测试、已合并、已部署、生产已验证分开写，不互相推断。
