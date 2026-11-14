# 内容回溯看板：勘察记录

> 状态: 代码事实优先
> 最后核对: 2026-11-14
> 适用范围: [内容回溯看板实施手册](06-content-lens-playbook.md)里全部 `<<RECON:…>>` 占位符的真实值，以及开工前必须知道的代码事实；对应任务卡 T0.1
> 事实来源: 2026-11-14 的只读走查，行号已在合入 `origin/main@954bcaa9` 之后重新核对；本机运行库的只读结构查询
> 冲突时以谁为准: 实施当时的真实代码与迁移。行号会漂移，使用前按符号名重新定位；找不到时停下并记入 `QUESTIONS.md`

本文件只记录事实。没有在这里出现的现有符号，一律视为不存在；需要用时先补勘察。

## 1. 占位符取值

| 占位符 | 取值 | 出处 |
|---|---|---|
| `ops_publication_table` | `linggan_ops_publication`；标题列 `title`（可空），平台列 `platform`，小红书取值 `'xhs'`；主键 `publication_ref`；账号 `account_ref`；发布日期 `published_business_date`（按北京时间生成） | `database/migrations/0123_ops_ledger.sql:104` |
| `ops_view_report_table` | `linggan_ops_view_report`；只追加（有禁止修改的触发器）；`report_kind` 为 `value` 或 `cleared`，`cleared` 时 `view_count` 为空 | `database/migrations/0123_ops_ledger.sql:183` |
| `latest_views` | 现成的公共表达式 `LATEST_VIEWS_CTE`：每个 `publication_ref` 取 `report_sequence` 最大的一条 | `crates/intelligence/src/ops_ledger/read.rs:34` |
| `ops_daily_table` | `linggan_ops_daily_report`；每人每天一行；`links_opened_count`、`followers_gained`（加微） | `database/migrations/0123_ops_ledger.sql:207` |
| `ops_read_module` | `crates/intelligence/src/ops_ledger/read/`；概览读模型在 `read/overview.rs`（`read_ops_overview`，153 行处） | 同左 |
| `task_key_enum` | `TaskKey`，以及它的 `as_str` | `crates/intelligence/src/ai_foundation.rs:55`、`:83` |
| `input_ref_enum` | `InputRef`：每个任务的输入引用是它的一个变体 | `crates/intelligence/src/ai_foundation/batch_store.rs:44` |
| `executor_dispatch` | 按 `TaskKey` 分派到各任务服务的 `match` | `crates/intelligence/src/ai_foundation/executor/service.rs:189` 起 |
| `task_service_example` | L1 的服务实现，是新任务最接近的样板（准备、调用、结算三段） | `crates/intelligence/src/ai_foundation/executor/service/content_understanding.rs` |
| `task_seed_example` | 新任务的种子迁移样板：依次写入 `linggan_ai_task`、`linggan_ai_control`、`linggan_ai_task_round_state`、`linggan_ai_prompt_version`、`linggan_ai_task_config`、`linggan_ai_task_setting`、`linggan_skill_binding` | `database/migrations/0199_content_understanding_model_deep.sql:1` |
| `ai_foundation_tables` | 批次与条目：`linggan_ai_batch`、`linggan_ai_batch_item`（及各自的 `_control`）；调用账本：`linggan_model_invocation`；任务配置：`linggan_ai_task_config`；外发记录：`linggan_model_egress_event` | 本机库结构查询 |
| `material_class` | 枚举 `MaterialClass`，现有七个取值，没有“我方笔记标题”。`NoteMaterial` 指外部笔记材料，其 `permits_task` 明确列出允许的任务。外发许可按“端点 × 任务 × 材料类别”记录 | `crates/intelligence/src/ai_foundation/provider_store.rs:17`、`:38`、`:119` |
| `local_read_json_error` | 函数 `local_read_json_error(status, code)`，响应体 `{"operation":"local_read","outcome":"unavailable","code":<code>}` | `apps/api/src/local_web.rs:2379` |
| `ops_api_routes` | 运营路由在 `routes()` 里逐个 `merge`；新增一组接口就是新增一个文件并在这里 `merge` | `apps/api/src/local_web/ops.rs:7` |
| `ops_admin_check` | `require_admin(state, headers)`；概览接口每个处理函数开头调用 | `apps/api/src/local_web/ops_overview_api.rs:151` |
| `ops_not_connected` | 数据库未连接时的统一响应 `ops_api::not_connected()` | `apps/api/src/local_web/ops_overview_api.rs` 的 `overview` 处理函数 |
| `web_ops_paths` | `OPS_PATHS` 常量列出运营的二级路径 | `apps/web/src/app/pages.ts:21` |
| `web_page_registry_test` | 页面登记测试 | `apps/web/src/app/pageRegistry.test.ts` |
| `web_ops_nav` | 运营左栏的数据页清单（概览、发布记录、账号管理、每日数据）；受限成员默认落在发布记录 | `apps/web/src/pages/ops/OpsFrame.tsx:7`、`:57` |
| `web_ops_overview` | 概览页与区块 | `apps/web/src/pages/ops/OverviewPage.tsx`、`OverviewBlocks.tsx`（各 442 行） |
| `web_ops_charts` | 现有图表实现 | `apps/web/src/pages/ops/Charts.tsx` |
| `web_ops_api_client` | 运营页的接口封装 | `apps/web/src/pages/ops/api.ts` |
| `wilson_interval` | `wilson_interval(k, n, z) -> Result<Interval, StatsError>`；同文件另有 `rate_per_1k` | `crates/intelligence/src/opportunity/stats.rs:17`、`:48` |
| `feature_flag_pattern` | 机会卡的开关：环境变量取值严格等于 `enabled` 才开启。目前实现在命令行与执行器两处；接口层还没有机会卡路由，所以没有“启动时读取并存入状态”的现成实现 | `apps/worker/src/bin/opportunity.rs:206`；`crates/intelligence/src/ai_foundation/executor/service/opportunity_sync.rs:25`；`env.example:24` |
| `cli_pattern` | 机会卡的命令行二进制 `linggan-opportunity`，子命令形式 | `apps/worker/Cargo.toml:66`；`apps/worker/src/bin/opportunity.rs` |
| `migration_assembly` | 新迁移要登记两处：本机运行脚本的 `apply_migration_once` 列表；接口测试用的全量结构夹具（`include_str!` 与校验和各一行） | `scripts/local-runtime.sh:529` 附近；`apps/api/src/local_web/full_schema_fixture.rs:339`、`:494` 附近 |
| `next_migration` | 勘察时主线最新为 `0215_opportunity_creator_problem_density`；一天之内主线新增了 0214、0215 两个迁移。编号在合入时取主线下一个，不提前占用 | `database/migrations/` |
| `proof_scripts` | 隔离库证明脚本的样板 | `scripts/test-ops-ledger-postgres.sh`、`scripts/test-opportunity-postgres.sh`、`scripts/test-local-runtime-schema.sh`；空选择保护在 `scripts/cargo-proof.sh` |
| `preflight` | 提交前预检 | `bash scripts/ci-preflight.sh origin/main` |
| `ci_selector` | 按改动路径选择证明；新增共享依赖要同步它的路由测试 | `scripts/ci-select.mjs`、`scripts/ci-select.test.mjs` |

## 2. 开工前必须知道的五件事

### 2.1 我方笔记标题需要一个新的材料类别

现有 `MaterialClass` 里没有适合“我方已发布笔记的标题”的取值。`NoteMaterial` 的语义是外部笔记材料，而且它允许的任务是写死的清单。按 Mog 对 DR-04 的裁定（只授权标题、限定任务），应当新增一个材料类别，只允许 `ops_title_label` 这一个任务。外发许可由 Mog 在 AI 设置里对具体端点授予，代码与迁移都不能替他授予。

### 2.2 新增一个任务要动的共享位置

| 位置 | 改什么 |
|---|---|
| `TaskKey` 及 `as_str` | 新增取值 |
| `InputRef` | 新增变体，承载一个标注批次分片的引用 |
| 执行器分派的 `match` | 新增分支 |
| `MaterialClass` 及 `permits_task` | 新增取值与许可 |
| 种子迁移 | 按 0199 的样板写入七张配置表 |
| 迁移装配两处 | 登记新迁移 |
| `scripts/ci-select.mjs` | 新增证明脚本时登记路由 |

这些都是共享装配文件。机会卡工作包也在改其中的迁移装配，合入前必须先同步主线，见[任务编排](ORCHESTRATION.md)第 5 节。

### 2.3 统计函数暂不搬家

`wilson_interval` 在机会卡模块里，而机会卡阶段 1 正在开发。为避免两个工作包同时改同一个文件，看板直接调用现有函数，新增的置换检验与 kappa 写在看板自己的模块里。等机会卡阶段 1 结束，再由一个单独的小改动把三者提取到共享位置。

### 2.4 阅读量“清空”是一种合法状态

`linggan_ops_view_report` 的最新一条可能是 `cleared`，此时 `view_count` 为空。它与“从未填过”一样，都不进分母，也都不显示为 0。直接复用 `LATEST_VIEWS_CTE`，不要自己写取最新的逻辑。

### 2.5 运营左栏的现有标签是“概览”

手册早先写作“总览”，以代码里的“概览”为准。新页面排在“概览”之后。受限成员默认落在“发布记录”，看板在圈 A 只对管理员可见（DR-09），所以不需要改这段默认逻辑。

### 2.6 L1 的失败恢复已在主线推进

2026-11-13 至 14 日主线合入了两组改动（PR #582、#584）：在不改变来源锚点的前提下恢复无歧义的 L1 草稿；从已保存的完整回复离线恢复 Core（新增迁移 0214 与一个恢复用的命令行）；恢复带多余换行逗号的无歧义转写引句。因此“29 次失败”是 2026-11-13 的时点数字，其中结构不合格与引句不逐字两类已有一部分被处理。工作包 U1 应当先核对这些改动之后剩余的失败，再做分析。

## 3. 数据现状（2026-11-13 本机库）

| 项 | 数字 |
|---|---|
| 小红书发布记录 | 1,546 |
| 标题非空 | 956 |
| 不同标题键 | 886 |
| 有阅读量 | 442 |
| 全部记录的来源 | `content_workbench` 导入；原生登记 0 |
| 最新发布日期 | 2026-10-12 |

## 4. 没有找到的东西

| 找什么 | 结果 |
|---|---|
| 通用的特性开关机制 | 不存在；只有机会卡的环境变量约定 |
| 接口层“启动时读取开关并存入状态”的实现 | 不存在；看板要自己实现，并在机会卡接口落地时与之对齐 |
| 现成的 kappa 或置换检验实现 | 不存在 |
| 任何人工判定或评测结果的表 | 不存在 |
| 现有测试的失败基线清单 | 本次未运行全量测试；首个改代码的工作包开工时先记录基线 |

## 5. 未核对

- 前端是否已有可复用的区间条或格子组件。只确认了 `Charts.tsx` 存在，没有读它的内容。
- `linggan_ai_task` 等配置表是否对 `task_key` 有额外的取值约束。按 0199 的样板写入即可，但首次迁移务必在隔离库的真实有序链上跑通。
- 成员角色除“管理员”与“受限”之外的细分。
