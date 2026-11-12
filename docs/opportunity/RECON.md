# RECON（代码库勘察）

勘察日期：2026-11-12
勘察人：Claude Code（执行 agent）
仓库：`/Users/moglenny/proma/linggan-intelligence`

> 状态: 一次性报告
> 最后核对: 2026-11-12
> 适用范围: 机会卡 T0.1 与 T0.1b 的代码库勘察结论。
> 事实来源: 指定基线上的实际文件读取、`rg` 搜索与只读聚合查询。
> 冲突时以谁为准: 当前代码、迁移、合同与实际命令输出。

## T0.1b 基线同步（2026-11-12）

- 新基线：`origin/main` @ `4d65e506`（`Merge pull request #573 ...`），通过 `git fetch origin` 确认 `HEAD..origin/main = 53` 已归零。
- 同步方式：主工作树的 `docs/README.md` 同时有他人未提交改动和上游改动，`git pull --ff-only` 会覆盖本地改动。按负责人裁定改用独立 worktree，主工作树及其未提交/未跟踪文件保持原样。
- 工作 worktree：`.worktrees/opportunity-card-20261112`，分支 `claude/opportunity-card-20261112`，跟踪 `origin/main`，HEAD `4d65e506`。
- 迁移链头：`0202_topic_core_merge_prompts`；迁移文件 203 个（数字前缀 0001–0202，`0064`、`0065` 各有两个文件）。
- 数据库实际 ledger 最新为 `0202_topic_core_merge_prompts`（只读查询 `linggan_local_schema_migration`）。
- 原 T0.1 基线 `ad19a4d7` 的勘察结论凡未被下文覆盖的，仍然有效；受影响的条目已就地更新，补充勘察见 §7。

### 原 T0.1 基线

基线：`main` @ `ad19a4d7`（`git rev-parse --abbrev-ref HEAD`、`git rev-parse --short HEAD`）。

工作树状态（`git status -sb`）：`main...origin/main [behind 53]`；已有他人改动 `M docs/README.md`，未跟踪 `.artifacts-private/`、`docs/audits/douyin-comments-reference-audit-2026-11-02.md`、`docs/audits/evidence-image-ocr-product-audit-2026-11-08.md`。T0.1 **未修改任何代码，也未提交**；只新增 `docs/opportunity/` 下的文档。T0.1b 同样未修改代码。

> 说明：本文件只记录本次实际读到的符号和实际命令输出。未列出的现有符号按手册 §3.1 视为不存在；需要时先补勘察。

## 1. 基础

### 1.1 仓库结构概览

| 顶层路径 | 用途 | 证据 |
|---|---|---|
| `apps/api` | Axum 本地 API，同源提供正式 React 页面 | `Cargo.toml:4`；`apps/api/src/main.rs:1-8` |
| `apps/worker` | 巡检、媒体、AI 等 worker 二进制 | `Cargo.toml:5`；`apps/worker/src/bin/ai_worker.rs:39-70` |
| `crates/contracts` | 采集/传输合同与 fixture | `Cargo.toml:6`；`crates/contracts/tests/` |
| `crates/domain` | 领域类型 | `Cargo.toml:7` |
| `crates/evidence` | 采集控制、材料读取、研究 dossier | `Cargo.toml:8` |
| `crates/observation` | 观察侧 | `Cargo.toml:9` |
| `crates/intelligence` | 评论研究、AI foundation、整篇理解 | `Cargo.toml:10` |
| `crates/storage-postgres` | PostgreSQL 存储 | `Cargo.toml:11` |
| `apps/web` | React + TypeScript 正式前端 | `apps/web/package.json:1-54` |
| `database/migrations` | 203 个 SQL migration（数字前缀 0001–0202；0064、0065 各有两个文件） | `ls database/migrations/*.sql \| wc -l` = `203`；`ls database/migrations/*.sql \| sed 's#.*/##' \| cut -d_ -f1 \| sort \| uniq -d` = `0064`、`0065` |
| `plugins/linggan-intelligence-browser` | 浏览器采集插件 | `README.md` 插件说明；`AGENTS.md:33` |
| `scripts` | 本地运行时、CI preflight、隔离 PostgreSQL proof | `scripts/local-runtime.sh:272-509`；`scripts/ci-preflight.sh` |
| `docs` | 合同、治理、设计、页面、决策 | `docs/` |

`ls -1` 顶层实际输出（节选）：`AGENTS.md apps artifacts Cargo.lock Cargo.toml CLAUDE.md codx.md compose.yaml crates database docker docs env.example plugins PROJECT-MANIFEST.json README.md references rust-toolchain.toml scripts target`。

### 1.2 后端语言、框架与入口

- 语言/工具链：Rust edition 2024（`Cargo.toml:16`）；`rustc --version` = `rustc 1.95.0 (59807616e 2026-04-14)`；`rust-toolchain.toml` 存在。
- Web 框架：Axum `0.8.8`（`Cargo.toml:21`）；异步运行时 Tokio `1.48.0`（`Cargo.toml:23`）；数据库 sqlx `0.9.0`（`Cargo.toml:22`）。
- API 入口：`apps/api/src/main.rs:1-8`，`#[tokio::main]` 调用 `local_web::serve()`。
- 路由总装：`apps/api/src/local_web.rs:343-435` 的 `router_with_public_access`；各功能模块在自己的 `routes()` 中返回 `Router<LocalWebState>`，例如 `apps/api/src/local_web/comment_study_api.rs:27-36`、`apps/api/src/local_web/note_analysis_api.rs:16-25`；新基线新增 `content_understanding_api::routes()`（`apps/api/src/local_web.rs:418`）。
- Worker 入口：`apps/worker/src/main.rs:24-28` 读取 `LINGGAN_LOCAL_DATABASE_URL` 后启动；AI worker 在 `apps/worker/src/bin/ai_worker.rs:39-70`。

### 1.3 前端框架、页面目录、路由方式

- React `19.3.1`、React DOM `19.3.1`、react-router `8.4.0`（`apps/web/package.json:30-32`）；TanStack Query `5.104.2`（`apps/web/package.json:26`）。
- TypeScript `6.0.3`、Vite `8.3.4`、Vitest `5.0.4`、Playwright `1.57.0`（`apps/web/package.json:49-53`）。
- 路由：`createBrowserRouter`，页面路径集中在 `apps/web/src/app/App.tsx:35-51`；路径常量在 `apps/web/src/app/pages.ts:5-23`。
- 页面目录：`apps/web/src/pages/`（如 `apps/web/src/pages/corpus/comments/`、`apps/web/src/pages/corpus/evidence/`）。
- 页面登记约束：每个前端路径必须同时出现在 `apps/api/src/local_web/web_app.rs` 的 `REACT_PAGES`，由 `pageRegistry.test.ts` 逐项对照（`apps/web/src/app/pages.ts:1-4`）。

### 1.4 构建、测试、Lint、类型检查命令与一次实际运行结果

命令来源：

- Rust：`cargo test --workspace`（`README.md:21` 要求提交前跑受影响模块验证）；`cargo fmt --check`；Clippy 配置 `too_many_lines = "deny"`（`Cargo.toml:32-35`）。
- 前端：`npm run typecheck` = `tsc --noEmit -p tsconfig.json`；`npm run lint` = `eslint .`；`npm run test` = `vitest run`；`npm run build` = `tsc --noEmit -p tsconfig.json && vite build`；`npm run verify` 串联四者（`apps/web/package.json:10-16`）。
- 提交前：`bash scripts/ci-preflight.sh origin/main`（`README.md:21`）。
- 隔离 PostgreSQL proof：`scripts/test-*-postgres.sh`，例如 `scripts/test-comment-research-postgres.sh`、`scripts/test-comment-like-library-postgres.sh`、`scripts/test-collection-single-run-postgres.sh`。

实际运行（2026-11-12，新基线 `4d65e506`；T0.1b 重新记录）：

```text
$ cargo test --workspace --no-fail-fast
failures:
---- local_web::creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work stdout ----
thread 'local_web::creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work'
  panicked at apps/api/src/local_web/creator_lifecycle_tests.rs:226:5:
  assertion `left == right` failed
    left: 503
   right: 200
test result: FAILED. 111 passed; 1 failed; 127 ignored; 0 measured; 0 filtered out; finished in 4.83s
error: 1 target failed:
    `-p linggan-api --bin linggan-api`

其余 test binary 均为 ok（示例：193 passed / 1 ignored；332 passed / 10 ignored；50 passed；
大量需要隔离 PostgreSQL 的测试显示 ignored）。
```

结论：新基线上仍是**同一个**既有失败，失败集合与 T0.1 基线一致（D-002）；
失败测试位于 `apps/api/src/local_web/creator_lifecycle_tests.rs:214-232`，期望 `/collection/targets` 返回 `200`，实际 `503`。
本次环境同时缺少 `apps/web/node_modules` 与 `apps/web/dist`（`ls` 均不存在），但未进一步断言失败原因，也未修复。

```text
$ cargo fmt --check
exit=1
Diff in .../crates/evidence/tests/support/standard_dossier_fixture.rs:31:
-        slots.push((if video && images == 1 { "cover" } else { "image" }, page));
```

`cargo fmt --check` 仍在既有测试支持文件上失败，与 T0.1 基线一致；T0.1b 未修改。

```text
$ npm run typecheck / lint / test / build
未运行：apps/web/node_modules 不存在（node --version = v24.13.0；apps/web/node_modules absent）
```

`bash scripts/ci-preflight.sh origin/main` 本次未运行；它是 `README.md:21` 规定的提交前检查，不是 T0.1 的必需测试运行。

## 2. 数据库

- 类型与版本：PostgreSQL `16.14` + pgvector `0.8.0`（`database/README.md:44-48`：固定 Docker 官方 `postgres:16.14-bookworm` digest，镜像 `linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1`，pgvector `v0.8.0`）。
- 实际运行中的容器（`docker ps --format '{{.Names}} {{.Image}} {{.Ports}}'` 输出节选）：
  `linggan-intelligence-postgres-1 linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1 127.0.0.1:55432->5432/tcp`。
- 本机端口与数据库：`127.0.0.1:55432`，库名 `linggan_intelligence_dev`（`env.example:2-5`；`database/README.md:48`）。
- 访问方式：**原生 SQL + sqlx，无 ORM/查询构建器**。`Cargo.toml:22` 只启用 `runtime-tokio`、`postgres`、`uuid`、`json`；示例查询 `crates/intelligence/src/ai_foundation/comment_problem.rs:38`。
- 迁移工具：自建 shell runner，不是 sqlx migrate。`scripts/local-runtime.sh:272-305`：
  - ledger 表 `linggan_local_schema_migration(migration_id text PRIMARY KEY, migration_sha256 text, applied_at)`（`:274-278`）；
  - `apply_migration_once` 先用 `shasum -a 256` 校验文件，再读取 ledger；已应用但 checksum 不同会拒绝继续（`:286-292`）；
  - 未应用时把 migration 文件包在 `BEGIN; ... INSERT ledger; COMMIT;` 中执行（`:298-303`）。
- 迁移目录：`database/migrations/`；命名规则：`<4 位数字前缀>_<snake_case 名称>.sql`；**0001–0202 共 203 个文件**，0064/0065 各有两份（`database/README.md:59` 明确说明 `0064_account_observation_normalization` 是另一项独立迁移，ledger 用完整 migration ID 校验；T0.1b 重数：`ls database/migrations/*.sql | wc -l` → `203`）。
- 迁移链装配（新基线）：`scripts/local-runtime.sh:307-516` 从 `0001_scope_001_capture_evidence` 顺序调用到 `0202_topic_core_merge_prompts`（`:313`、`:516`）。T0.1b 新增的 `0196..0202` 在 `:507-516` 一段注册。
- 本地库 ledger 头：`SELECT max(migration_id) FROM linggan_local_schema_migration` → `0202_topic_core_merge_prompts`（2026-11-12，只读查询）。
- 回滚方式：**未找到 down migration / rollback 脚本**。`find database/migrations -maxdepth 1 -iname '*down*' -o -iname '*rollback*'` 没有 down migration（唯一命中是名称含 `cooldown` 的 `0090_detail_page_grant_recovery_and_risk_cooldown.sql`）。恢复方式只能是新增前向 migration；开发库重置脚本（如 `scripts/maintenance/unified-research-reset.sql`）不等于迁移回滚。
- 本次 T0.1 **未执行任何 migration，未写数据库**。

## 3. 现有数据

### 3.1 comments_table / comment_pk / comment_work_fk

- `comments_table` = `linggan_material_comment`（`database/migrations/0016_material_social_lanes.sql:26`）。
- `comment_pk` = `material_ref uuid PRIMARY KEY`（`0016_material_social_lanes.sql:27`）。
  - 注意：`material_ref` 是**每一次观察**的主键，不是稳定评论主键。
  - 稳定评论身份 = `(content_public_ref, comment_external_id)`，当前投影按它 `DISTINCT ON`（`database/migrations/0025_comment_current_projection.sql:6-24`）。
  - 因此新表的 `comment_id` 若用于去重/提及人数，应使用稳定身份；`material_ref` 只适合指向某一次观察。见 `QUESTIONS.md` Q-06。
- `comment_work_fk` = `content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref)`（`0016_material_social_lanes.sql:28`）。

### 3.2 comment_published_col / comment_observed_col

- `comment_published_col` = **无**。
  - `linggan_material_comment` 的列里没有发布时间（`0016_material_social_lanes.sql:26-56`）。
  - 全库搜索 `ALTER TABLE linggan_material_comment` 无结果；搜索 `material_comment.*published` 无结果。
  - `0026_work_resource_read.sql:8-15` 只给 `linggan_material_content_detail` 加 `published_at`（作品），不是评论。
- `comment_observed_col` = `observed_at text NOT NULL`（`0016_material_social_lanes.sql:40`）。
  - 它是**该次观察**的时间文本；`linggan_material_comment_current` 取 `observed_at::timestamptz DESC` 的最新观察（`0025_comment_current_projection.sql:24`）。
  - 首次采集时间没有独立列（未找到）。可从 `linggan_material_comment_first_admission(content_public_ref, comment_external_id, first_package_ref)`（`0125_collection_single_run.sql:136-145`）关联 `linggan_runtime_capture_package.accepted_at`（`0004_plugin_runtime_all_capabilities.sql:42`）推导；trigger 在首次插入时写入（`0125_collection_single_run.sql:146-153`）。
- 作品侧双时间：`linggan_material_content_detail.observed_at text`（`0015_material_projection.sql:21`）与 `published_at timestamptz`（`0026_work_resource_read.sql:8-15`）；`published_at` 只在 `platform_epoch` + 精确度合格时允许非空（`0026_work_resource_read.sql:17-25`）。

**T0.1b：采集包里到底有没有评论发布时间（D-008 用）**

原始采集包里**有**发布时间字段，但**没有**写进 `linggan_material_comment`，也没有稳定映射到 API：

- 插件采集：`plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:93-114` 用 `create_time / createTime / time` 解析（秒级或毫秒级 epoch），`:458-492` 同时生成 `publishedAt` 与 `publishedAtText`（相对时间/展示文本）。
- 入库规范化：`plugins/linggan-intelligence-browser/src/db/recordNormalization.js:162`：`publishedAt = normalizeNumber(record.publishedAt || record.createdAt, 0)`，即**缺少发布时间时回退到采集时间**（`createdAt`），无法与真实发布时间区分。`:185` 的 `publishedAtText` 只取 `record.publishedAtText || record.time`。
- 实际数据（只读聚合，库 `linggan_intelligence_dev`）：comments + replies 包共 31809 条记录，`records[].payload.publishedAt` 为 13 位毫秒 epoch 的只有 **45 条（0.14%）**，其余 31764 条为 `0`；`publishedAtText` 31809 条全部有值，但 76.8% 是带 `-` 的展示日期、16.6% 是“N 天前”相对文本（逐项输出见 §7.3）。
- API 侧：`crates/intelligence/src/comment_study_catalog/comments.sql:183` 直接写 `'publishedAt', NULL`，所以评论研究目录接口**恒为 null**，即使包里存了值。
- 结论：D-008 v2 要求的“发布时间可用率 ≥90%”在当前链路上**远未达到**（原始精确 epoch 仅 0.14%）。这是一条新阻塞，已记入 `QUESTIONS.md` Q-11。

### 3.3 commenter_col（平台评论者标识，是否稳定）

- `commenter_col` = `author_external_id text`（`0016_material_social_lanes.sql:38`），可为 `NULL`；显示名单独存 `author_display_name`（`:39`）。
- “未知作者”规则：未知身份的原声仍可查阅，但研究目标计入 `commentAuthorUnknown`，且**未知作者不能作为新 Problem 的独立作者支持**（`docs/data-contracts/comment-study-http-001.md:43`）。
- 未找到单独的“评论者身份稳定性”合同字段。现有稳定身份是 `comment_external_id`；作者身份用 `author_external_id`，未知时为 NULL。作者本人声音的判断用评论作者与作品作者比较，例如 `crates/intelligence/src/comment_study_source.rs:73` 同时选出 `c.author_external_id` 与 `author.author_external_id AS work_author_external_id`。

### 3.4 works_table / work_pk / work_domain_col / work_creator_col

- `works_table` = `linggan_material_content`（作品身份）+ `linggan_material_content_detail`（逐次观察的标题/正文/互动/发布时间）。
  - `linggan_material_content`：`platform`、`content_external_id`、`public_ref`（`0015_material_projection.sql:7-14`）。
  - `linggan_material_content_detail`：`material_ref`、`content_public_ref`、`observed_at`、`title`、`body_text`、`creator_display_name`、`published_at_source_text` 等（`0015_material_projection.sql:16-39`）。
- `work_pk` = `linggan_material_content.public_ref uuid`（`0015_material_projection.sql:10`）。
- `work_domain_col` = **无直接列**。
  - 作品与领域是多对多：`linggan_material_domain_usage(content_public_ref, domain_ref, role, basis_kind)`（`0103_domain_membership_and_usage.sql:125-149`）。
  - 后续又加 `use_kind`：`domain_research` / `creator_context`（`0180_creator_preview_discovery_only.sql:8-23`）；生产读取按 `use_kind='domain_research'` 过滤，例如 `crates/evidence/src/material_social_read.rs:528`。
  - 观察目标与领域的绑定另有 `observation_domain_target(domain_ref, target_ref, role)`（`0103_domain_membership_and_usage.sql:15-22`）。
- `work_creator_col` = **无直接列**。
  - 当前作者投影是视图 `linggan_material_content_author`，`DISTINCT ON (content_public_ref)` 选择一条作者归属，字段 `author_external_id`、`attribution_source`、`observed_at`（`0063_content_author_attribution.sql:28-35`）。
  - 需要 `as_of` 一致时用函数 `linggan_material_content_author_at(content_public_ref, as_of)`（`0115_creator_discovery.sql:4`）；例如整篇理解读取使用它（`crates/intelligence/src/ai_foundation/note_analysis/source.rs:53`）。
  - `author_external_id` 可为 NULL；详情自带与主页扫描推得的归属有不同的 `attribution_source`（`0063_content_author_attribution.sql:33-34`）。

### 3.5 点赞数字段（作品、评论）

- 作品：`linggan_material_content_detail.like_count bigint` + `like_count_state`（`0023_material_engagement_and_media_components.sql:3-6`）；另有 `comment_count`、`collect_count`、`share_count` 及各自 state（`:7-15`）。state 闭集为 `KNOWN/UNKNOWN/NOT_VISIBLE/NOT_OBSERVED/PARSE_FAILED`（`:5-15`），`KNOWN` 才允许非空（`:16-25`）。
- 评论：`linggan_material_comment_engagement.like_count` + `like_count_state`（`0122_comment_like_library.sql:8-24`），`KNOWN/UNKNOWN`（`:15-18`），append-only trigger（`:27-29`）；规则版本 `comment-like.v1`（`:24`）。
- 评论点赞口径：`赞` 记为已知 0；正整数 `likes` 记为已知值；空/缺省 `"0"`/非整数为未知，不写 0；读取取 `asOf` 之前最近一次已知观测（`docs/data-contracts/comment-study-http-001.md:325`）。

### 3.6 来源资格：eligibility_predicate 与 is_withdrawn

- 评论“当前、已接受、未受限”的资格：
  - 已接受：`linggan_runtime_capture_package.accepted_at <= scope_001_now()`。
  - 未受限：不存在 `linggan_material_comment_restriction` 行。
  - 该判断在生产代码中的实际写法：`crates/intelligence/src/comment_study_source.rs:73`（`JOIN linggan_runtime_capture_package ... accepted_at<=...` + `NOT EXISTS (SELECT 1 FROM linggan_material_comment_restriction ...)`）；整篇理解读取同样如此（`crates/intelligence/src/ai_foundation/note_analysis/source.rs:53`）。
  - 0064 还定义了视图 `linggan_comment_research_source_current`（`0064_comment_research_kernel.sql:11-25`），但当前生产读取主要直接查 `linggan_material_comment_current` + `linggan_material_comment_restriction`。
- 评论撤回：`linggan_material_comment_restriction(content_public_ref, comment_external_id, restricted_at, reason)`（`0094_corpus_evidence_read_recovery.sql:7-13`）。`is_withdrawn` 可判定为“存在 restriction 行”。
- 作品撤回/失效：`collection_material_retirement(target_ref, content_public_ref, reason_code, decided_by)`（`0061_material_retirement.sql:22-38`）；它针对**某个观察目标的作品目录**，不是全局作品删除（`:24-26`）；`reason_code` 只有 `page_gone/page_unreadable`，`decided_by` 必须为 `person`（`:28-31`）。
- 受限来源的读取处理：当前 Signal/Problem 读取会屏蔽可能复述受限来源的 proposition、evidence、problemFrame；来源受限后从当前列表退出（`docs/data-contracts/comment-study-http-001.md:201`、`:276`）。

### 3.7 评论研究中的“用户问题”实体、与评论的关联、“已核验”判定位置

- 当前生产使用的实体：
  - `linggan_comment_problem_definition`：`definition_ref`、`problem_ref`、`domain_ref`、`identity_hash`、`label`、`actor`、`goal`、`barrier`、`context`、`source_request_ref`、`source_signal_ordinal`、`review_invocation_ref`（`0177_comment_problem_workflow.sql:39-49`）。
  - `linggan_comment_problem_membership`：`request_ref`、`signal_ordinal`、`definition_ref`、`review_invocation_ref`（`0177_comment_problem_workflow.sql:51-55`）。
- 与评论的关联链：
  `linggan_comment_problem_membership.request_ref` → `linggan_comment_research_request.source_ref` → `linggan_material_comment.material_ref`。
  证据：`0174_comment_research_execution.sql:26-52`（request 的 `source_ref` 引用 `linggan_material_comment(material_ref)`）；`crates/intelligence/src/ai_foundation/comment_problem/read.rs:13`（membership → request → source 的 join）。
- “已核验”判定：
  - 问题归并的 pair review prompt 要求“新共性问题必须由 pairedEvidence 中独立作者的两条原声分别支持同一个主体、目标、障碍与场景；缺少独立支持不能通过”（`0177_comment_problem_workflow.sql:11`）。
  - 发布 gate 在 Rust 中强制“支持来源作者不能与当前作者相同，且 identity 必须一致”，否则 `comment_publication_withheld`（`crates/intelligence/src/ai_foundation/comment_problem/publish.rs:39-50`）。
  - 读取面 `supportState` 三态：`admitted_by_later_review` / `awaiting_independent_support` / `not_pending`（`crates/intelligence/src/ai_foundation/comment_research.rs:263-272`）。
- 旧内核仍存在：`linggan_comment_research_problem`（`0064_comment_research_kernel.sql:205`）、`linggan_comment_research_problem_definition`（`:212`）、`linggan_comment_research_atom_problem_membership`（`:228`）。当前生产 problem 读取使用 0177 表（`crates/intelligence/src/ai_foundation/comment_problem.rs:38`；`apps/api/src/local_web/comment_study_command_api.rs:141-149`）。两者关系需负责人确认，见 `QUESTIONS.md` Q-07。

**T0.1b：0064 是否仍被写入、0177 是否有合并/取代/撤回语义**

- 0064 旧 problem 表：**当前代码不再写**。`rg -n 'INSERT INTO linggan_comment_research_problem|linggan_comment_research_problem\b' --glob '!database/migrations/**' -g '!docs/**'` 只命中 `database/bootstrap/comment-study-reset.sql:37,98`（开发重置脚本的计数与 DROP），没有任何生产 `INSERT`；生产读取走 0177 表。
- 0177 problem 表**没有** merge / supersede / withdraw / valid_from / valid_to 之类的列（`0177_comment_problem_workflow.sql:39-55` 只有定义字段和 membership 字段）；并且 `problem_ref uuid NOT NULL UNIQUE`（`:41`），即**一个问题只能有一条 definition**，所以 D-014 预期的“定义版本变化”在当前 schema 下不会发生。若需要“新版本取代旧版本”，必须新增前向 migration。
- 归属变化不改写 definition：membership 只追加 `(request_ref, signal_ordinal, definition_ref, review_invocation_ref)`（`:51-55`），不记录“本次归属取代上次归属”。

### 3.8 “用户提到的办法与经历”的存储位置

- 评论研究 Atom 的 `kind` 闭集包含 `problem|need|solution|experience`（`0064_comment_research_kernel.sql:147-152`）。
- 当前有效 Signal 的分页读取：`GET /current-signals`，`kind` 取 `solution` 或 `experience`（`docs/data-contracts/comment-study-http-001.md:276`）。
- 整篇理解也有 `methodCandidates`（方法候选，`crates/intelligence/src/note_analysis.rs:154-161`、`:190`）和 `features`/`feedback`（`:100-107`、`:186-187`）。

### 3.9 整篇理解结果：存储位置、understanding_fields 实际字段

- 请求表：`linggan_note_analysis_request`，含 `domain_ref`、`work_ref`（引用 `linggan_material_content(public_ref)`）、`scope note|note_with_comments`、`knowledge_cutoff`、`input_hash`、`input_manifest`、`content_fingerprint`、`task_config_ref`、`task_revision`、`method_version='note-analysis.method.v0.3'`、`batch_ref`、`item_ref`（`0191_note_analysis.sql:26-42`）。
- 结果表：`linggan_note_analysis_result.result jsonb` + `failure_code`，二选一（`0191_note_analysis.sql:44-52`）；不可变 trigger（`:53-54`）。
- 合同/方法版本：`note-analysis.result.v1`、`note-analysis.method.v0.3`（`crates/intelligence/src/note_analysis.rs:8-9`）。
- `understanding_fields` 实际字段（`crates/intelligence/src/note_analysis.rs:181-193`）：
  - 顶层：`contract`、`summary`、`summaryBasisIds`、`understanding`、`features`、`feedback`、`explanations`、`tags`、`methodCandidates`、`limitations`、`evidence`。
  - `understanding`：`discussion`、`audience`、`situation`、`answer`、`offeredValue`（`:91-97`）；每项是 `Statement { id, text, kind, evidenceRefs }`（`:83-88`），`audience/situation/offeredValue` 可为 `null`。
  - `features[]`：`id`、`text`、`kind`、`evidenceRefs`、`label`、`category`（`:100-107`）。
  - `methodCandidates[]`：`id`、`text`、`kind`、`basisIds`、`conditions`、`openQuestion`（`:154-161`）。
  - `evidence[]`：`id`、`kind`、`sourceRef`、`sourceVersion`、`citationId`、`start`、`end`、`quote`（`:169-178`）。
  - `scope` 取 `note` 或 `note_with_comments`（`:12-22`）。

### 3.10 观察关系的存储位置

- 观察目标：`collection_observation_target`（`0005_collection_observation_target.sql:12-50`）：
  - `platform` 当前只允许 `xhs`（`:17`）；
  - `target_kind` 为 `creator|keyword`（`:20`）；
  - `identity_key` 是平台稳定身份（creator 用平台稳定 id，keyword 用词+排序面，`:22-26`）；
  - `lifecycle_state` 为 `pending_decision|archiving|archived|monitoring|paused|dismissed`（`:41-42`）。
- 目标与领域：`observation_domain_target(domain_ref, target_ref, role primary|reference)`（`0103_domain_membership_and_usage.sql:15-22`）。
- 作品与领域：`linggan_material_domain_usage`（`0103_domain_membership_and_usage.sql:125-149`）+ `use_kind`（`0180_creator_preview_discovery_only.sql:8-23`）。
- 作品作者归属：`linggan_material_content_author` 视图（`0063_content_author_attribution.sql:28-35`）与 `linggan_material_content_author_at(...)` 函数（`0115_creator_discovery.sql:4`）。

## 4. 能力

### 4.1 AI 任务队列：提交、回执、预算、许可的接口与示例调用位置

- 任务注册：`linggan_ai_task(task_key, label, lifecycle, automatic_brake)`（`0131_ai_gen1_provider.sql:2-20`）；评论问题工作流新增 `comment_problem_resolution`、`comment_problem_pair_review`（`0177_comment_problem_workflow.sql:2`、`:7`）。
- 提交：
  - 业务入口示例 `crates/intelligence/src/ai_foundation/note_analysis.rs:111-173`（`start`，最后调用 `batch_store::insert_batch` 于 `:164`）；
  - 评论研究 `crates/intelligence/src/ai_foundation/comment_research.rs:90`，`insert_batch` 于 `:135`；
  - 通用 `batch_store::insert_batch`（`crates/intelligence/src/ai_foundation/batch_store.rs:276-313`，函数定义在 `:276`）；
  - HTTP 示例：`POST /api/local/note-analysis`（`apps/api/src/local_web/note_analysis_api.rs:20`）、`POST /api/local/comment-research`（`apps/api/src/local_web/comment_study_command_api.rs:32`）、`POST /api/local/ai-settings/batches`（`apps/api/src/local_web/ai_settings.rs:73`）。
- 回执：
  - batch item 控制表 `linggan_ai_batch_item_control`（`0135_ai_gen1_batch.sql:37-68`）；
  - 成功回执必须关联模型 invocation（`0135_ai_gen1_batch.sql:123`，`ai_item_success_receipt_missing`）；
  - 迟到 usage 回执 `linggan_ai_late_usage_receipt`（`0139_ai_gen1_executor.sql:113-129`）；
  - 单条 item 的 attempt 回执 trigger（`0139_ai_gen1_executor.sql:108`）。
- 预算：
  - `linggan_ai_task_config.budget_mode`（`unconfigured|unlimited|capped`）+ `daily_token_limit`（`0134_ai_gen1_task_config.sql:61-67`）；
  - 执行前预算检查 `crates/intelligence/src/ai_foundation/executor/budget.rs:19-37`（`daily_budget_exhausted`）。
- 许可：
  - `linggan_model_egress_event` + 当前投影 `linggan_ai_current_egress`（`0131_ai_gen1_provider.sql:176-191`）；
  - 修改许可 `provider_store::change_egress`（`crates/intelligence/src/ai_foundation/provider_store.rs:112-148`）；
  - HTTP `POST /api/local/ai-settings/egress`（`apps/api/src/local_web/ai_settings.rs:45`）；
  - 入队/派发时按 `task_key` + `material_class` 检查 `allowed`（`crates/intelligence/src/ai_foundation/admission.rs:44-80`；`admission/base.rs:175-179`）。
- 注意：新任务若要外发评论材料，必须同时注册 `linggan_ai_task`、加入 batch 的 task/kind 约束、并在 egress 约束中允许对应 material_class；现有 0131/0177 的约束不是通用放行。
- 约束是**逐版本重建**的：`0196_content_understanding.sql:18-34`、`0199_content_understanding_model_deep.sql:18-34`、`0200_topic_research_automation.sql:9-15` 都先 `DROP CONSTRAINT` 再 `ADD CONSTRAINT`。当前最新版本：batch 的 task/kind 约束以 **0200** 为准（它只改了 batch 约束）；egress 的 task/material_class 约束以 **0199** 为准（0200 未改 egress）。新增任务时必须基于这两份清单重建，不能只抄 0131/0177。`comment_study` / `comment_problem_resolution` / `comment_problem_pair_review` 在 0200 中仍只允许 `kind='interactive'`，egress 允许 `note_material|comment_material`（`0199_content_understanding_model_deep.sql:28`）。

### 4.2 提示词与 Skill 版本注册方式（prompt_registry）

- 提示词版本表 `linggan_ai_prompt_version(version_ref, task_key, version_no, system_text, instruction_text, created_at)`，唯一 `(task_key, version_no)`，不可变（`0170_ai_prompt_versions.sql:11-21`）。
- `linggan_ai_task_config.prompt_version_ref` 外键绑定到同 task 的 prompt version（`0170_ai_prompt_versions.sql:22-24`）。
- HTTP 读取/保存/激活：`GET /api/local/ai-settings/prompts/{task}`、`POST /prompts/save`、`POST /prompts/activate`（`apps/api/src/local_web/ai_settings.rs:51-56`）。
- Skill 版本另有 `linggan_skill_binding` / `linggan_skill_version` 体系（`0133_ai_gen1_skill.sql`、`0136_ai_gen1_skill_binding.sql`；API 路由 `apps/api/src/local_web/ai_settings.rs:61-65`）。

### 4.3 采集侧：可用任务类型；是否支持按关键词定向搜索

- Runtime package 的 `package_kind` 闭集（`0004_plugin_runtime_all_capabilities.sql:31-34`）：
  `discovery_search`、`profile_discovery`、`content_detail`、`comments`、`replies`、`author_profile`、`media_slots`、`media_bytes`、`batch_checkpoint`。
- 观察目标支持 `target_kind='keyword'`（`0005_collection_observation_target.sql:20`）；keyword 的 `identity_key` 是“词 + 排序”，同一词不同排序算不同观察面（`:22-26`）。
- 搜索能力现状（`docs/platforms/xiaohongshu/capture-capability-registry.md:44-58`）：
  - 搜索输入 `VERIFIED`（`:46`）；
  - 下拉联想 `VERIFIED`（`:47`）；
  - 筛选组 `VERIFIED`（`:48`）；
  - 结果刷新 `PARTIAL`（`:49`）；
  - 结果卡片身份/内容 `VERIFIED`（`:55-56`）；
  - 顺序与筛选 `PARTIAL`（`:57`）；
  - 加载与停止 `SOURCE_INCOMPLETE`，真实搜索滚动/自然结束/跨筛选结果仍未验证（`:58`）。
- 详情/评论窗口：标准详情 `VERIFIED`（`:66-70`）；评论目标为 `min(公开评论数, 30)`，不可见时上限 30 且总数未知（`:68`）；同一 Attempt 取得 30 条评论/回复（`:69`）。

### 4.4 向量检索 / 嵌入能力：有 / 无，位置

- 有 pgvector：`CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA public`（`0075_local_embedding_001.sql:7`）。
- 有本地嵌入 profile：`Tencent/WeMM-Embedding-2B`，512 维，document 模式，`comment-research.atom-canonical-text.v1`（`0075_local_embedding_001.sql:36-46`）。
- 有评论 Atom embedding 表：`linggan_comment_research_atom_embedding`，`vector jsonb` + `dimensions`，state `pending/running/succeeded/failed/incompatible`（`0064_comment_research_kernel.sql:186-201`）。
- **未找到当前生产代码中的相似度检索查询**：`rg -n "atom_embedding|<=>|cosine" crates apps --glob '!*test*'` 只命中 `apps/pi-adapter/node_modules` 与 `apps/pi-adapter/src/wemm_runtime.py` 的注释；没有 Rust/SQL 的 `<=>`/余弦 Top-K 读取。`docs/current-state.md:405` 声称 V1 曾有 exact cosine Top-K，但当前源码未找到对应调用。
- 结论：向量**存储/嵌入能力存在**；向量**检索调用未确认**。T2.2 若需要“问题表述与作品理解摘要相似度前 20”，现有 atom embedding 不覆盖作品理解摘要，需先确认是否新建/复用检索路径。

### 4.5 密钥与配置读取方式

- 环境变量模板 `env.example:1-21`：`POSTGRES_*`、`LINGGAN_LOCAL_PORT=3001`、`DATABASE_ADMIN_URL`、`RUST_LOG`、`LINGGAN_COLLECTION_UPGRADE_PHASE=recovery` 等。
- 模型密钥：`crates/intelligence/src/model_secrets.rs:33-46` 的 `LocalFileModelSecrets::from_runtime()` 从 `LINGGAN_SUPPORT_DIR` 读取；缺省回退到 `$HOME/Library/Application Support/Linggan Intelligence`（`:39-43`），再拼接 `model-secrets/`；`MAX_SECRET_BYTES = 16 KiB`（`:13`）。路径必须绝对（`:52-54`），目录/文件强制私有权限。
- Worker 配置：`LINGGAN_LOCAL_DATABASE_URL`（`apps/worker/src/bin/ai_worker.rs:39`）、`LINGGAN_AI_CONCURRENCY`（`:40`）。
- 本地 API 端口：`println!("Linggan local host listening on http://localhost:{local_port}")`（`apps/api/src/local_web.rs:541`）。
- 采集账号 digest key：`LocalWebState.account_digest_key: Option<Arc<Vec<u8>>>`（`apps/api/src/local_web.rs:186`）；公开访问构造时为 `None`（`apps/api/src/local_web/public_access.rs:269`）；缺失时返回 `MissingDigestKey`（`crates/evidence/src/collection_control.rs:579-581`）。

### 4.6 特性开关机制

- **未找到通用 feature flag 机制**。搜索命令：
  `rg -n "FEATURE_|feature_flag|featureFlag|ENABLE_" --glob '!*.lock' --glob '!node_modules' apps crates database docs scripts env.example` → 无输出。
- 现有最接近的机制是环境变量阶段开关：`LINGGAN_COLLECTION_UPGRADE_PHASE`（`env.example:19-21`，缺省/未知保持新采集暂停）；以及 `LINGGAN_MODEL_SYNTHETIC_PREVIEW`（`crates/intelligence/src/model_secrets.rs:158`）。
- `opportunity_card` 默认关闭的实现方式需要负责人裁定，见 `QUESTIONS.md` Q-02。

### 4.7 日期时间库、加密库（Cargo.toml / package.json 中已有的）

- 日期时间：**Cargo.toml 中没有 `chrono` / `time` 直接依赖**（`rg -n "chrono|time =" Cargo.toml apps/*/Cargo.toml crates/*/Cargo.toml` 无命中）。现有时间处理以 PostgreSQL `timestamptz` 和 SQL 时区换算为主，例如 `linggan_ai_budget_day` 使用 `AT TIME ZONE 'Asia/Shanghai'`（`0131_ai_gen1_provider.sql:192-193`）；Rust 侧常以 `String` 传递时间（如 `note_analysis.rs:42` 的 `as_of`、`0016:40` 的 `observed_at text`）。
- 加密：
  - `sha2 = "0.11.0"`（`crates/contracts/Cargo.toml:15`、`crates/evidence/Cargo.toml:16`、`crates/intelligence/Cargo.toml:18`、`apps/api/Cargo.toml:22`、`apps/worker/Cargo.toml:14`）；
  - `argon2 = "0.5.3"`、`scrypt = "0.11"`（`apps/api/Cargo.toml:12`、`:19`；密码验证代码 `apps/api/src/local_web/ops_auth.rs:671-735`）；
  - 已有手写 HMAC-SHA256：`crates/evidence/src/collection_control.rs:1432-1454`，digest version `hmac-sha256-v1`（`database/migrations/0034_collection_control_closure.sql:64`）。
- 若 T0.6 评论者哈希复用该 HMAC 实现，不需要新增 `hmac` crate；但需要确定密钥来源与轮换，见 `QUESTIONS.md` Q-08。

### 4.8 性质测试库（如有）

- **未找到** `proptest` / `quickcheck`（`rg -n "proptest|quickcheck" Cargo.toml apps/*/Cargo.toml crates/*/Cargo.toml` 无命中）。
- 按手册 T0.4 的替代方案：固定种子生成 10,000 组随机输入。

## 5. 约定

### 5.1 后端模块组织与命名

- Workspace 成员：`apps/api`、`apps/worker`、`crates/contracts`、`crates/domain`、`crates/evidence`、`crates/observation`、`crates/intelligence`、`crates/storage-postgres`（`Cargo.toml:3-12`）。
- API 侧：`apps/api/src/local_web.rs` 顶部用 `mod <feature>;` 挂载 81 个模块声明（`:1-116`）；每个功能模块导出 `routes() -> Router<LocalWebState>`，由总路由合并。示例：`comment_study_api.rs:27-36`、`note_analysis_api.rs:16-25`。
- 业务逻辑侧：`crates/intelligence/src/<feature>.rs` 或 `<feature>/` 目录，例如 `comment_problem.rs`、`comment_problem/`、`note_analysis.rs`、`comment_research.rs`。
- 新增 `opportunity` 后端模块时，按现有约定放 `apps/api/src/local_web/opportunity*.rs` 与 `crates/intelligence/src/opportunity*.rs`，并在 `local_web.rs` 挂载。

### 5.2 API 路由风格、错误格式、分页方式

- 路由统一 `/api/local/...`，由 `local_web.rs:343-435` 总装；写接口复用本地同源/loopback guard（`docs/data-contracts/comment-study-http-001.md:21`；`apps/api/src/local_web/comment_study.rs` 的 guard middleware）。
- 请求/响应：serde `rename_all = "camelCase"` + `deny_unknown_fields`，例如 `apps/api/src/local_web/comment_study_command_api.rs:43-45`、`crates/intelligence/src/note_analysis.rs:24-25`。
- 错误：JSON `{code}` 风格，实际 helper：
  - `ingress_json_error` → `{ "admission": "not_accepted", "code": ... }`（`apps/api/src/local_web.rs:2359-2365`）；
  - `local_producer_error` → `{ "delivery": "not_acknowledged", "code": ... }`（`:2367-2373`）；
  - `local_read_json_error` → `{ "operation": "local_read", "outcome": "unavailable", "code": ... }`（`:2375-2380`）。
- 分页：
  - 评论研究 HTTP 合同使用 `cursor` + `limit`，响应 `page { limit, hasMore, nextCursor, asOf }`，`nextCursor` 无下一页为 NULL；默认 `limit=50`，允许 1–100（`docs/data-contracts/comment-study-http-001.md:25`、`:29`）。
  - 评论研究页面另有 `limit`/`offset` 只读接口（`docs/design/pages/comment-research-v1-page.md:59-61`）。
  - 新机会卡接口应按现有合同优先使用 `cursor`/`limit`/`asOf`。

### 5.3 日志方式

- 未发现 `tracing` / `log` crate 的结构化日志；当前是 `println!` / `eprintln!`，例如 `apps/api/src/local_web.rs:541`、`:756`、`:1979`，`apps/worker/src/bin/ai_worker.rs:25`、`:31`。
- `RUST_LOG=info` 在 `env.example:17`，但没有看到对应的 tracing subscriber 初始化。

### 5.4 现有测试的组织方式

- 单元测试：内联 `#[cfg(test)] mod tests`（例如 `crates/evidence/src/collection_control.rs:3235` 的 HMAC RFC 4231 向量）。
- API 测试：`apps/api/src/local_web/*_tests.rs`（例如 `creator_lifecycle_tests.rs`、`note_analysis_api_tests.rs`）。
- 集成测试：`crates/*/tests/*.rs`（例如 `crates/evidence/tests/collection_control_postgres.rs`、`crates/contracts/tests/capture_contract.rs`）。
- 隔离 PostgreSQL proof：`scripts/test-*-postgres.sh`（列表见 §1.4），通常创建一次性数据库、应用完整 migration fixture、运行 proof 后清理（`database/README.md:65`）。
- 提交前：`bash scripts/ci-preflight.sh origin/main`（`README.md:21`）。

## 6. 未找到的项

| # | 项目 | 查找方法 | 影响 | 处理 |
|---|---|---|---|---|
| 1 | 评论发布时间 `comment_published_col` | `rg -n "published_at" database/migrations/0016_material_social_lanes.sql`；`rg -n "ALTER TABLE linggan_material_comment" database/migrations`；`rg -n "material_comment.*published" database/migrations` → 均无结果。T0.1b 补查：采集包有 `publishedAt`/`publishedAtText`，但 API 恒 `NULL` | T0.2 双时间字段；§6.2.1 按 `published_at` 切周；所有周指标 | 无独立列；精确 epoch 覆盖仅 0.14%（45/31809）；见 §3.2 与 `QUESTIONS.md` Q-11 |
| 2 | 通用特性开关机制 | `rg -n "FEATURE_\|feature_flag\|featureFlag\|ENABLE_" --glob '!*.lock' --glob '!node_modules' apps crates database docs scripts env.example` → 无输出 | 硬规则 R14 `opportunity_card` 默认关闭 | 询问，见 Q-02 |
| 3 | Rust 日期时间库 | `rg -n "chrono\|time =" Cargo.toml apps/*/Cargo.toml crates/*/Cargo.toml` → 无命中 | T0.3 周期切分；手册 §3.3 第 8 条 | 询问，见 Q-03 |
| 4 | 性质测试库 | `rg -n "proptest\|quickcheck" Cargo.toml apps/*/Cargo.toml crates/*/Cargo.toml` → 无命中 | T0.4 三条性质测试 | 按 T0.4 替代方案固定种子 10,000 组，不需询问 |
| 5 | 作品领域单列 `work_domain_col` | 读取 `0015`、`0103`、`0180`；`rg -n "linggan_material_domain_usage" database/migrations` | §6.2.1 领域过滤与分母 | 使用 `linggan_material_domain_usage` + `use_kind`/`role`；语义见 Q-04 |
| 6 | 作品创作者单列 `work_creator_col` | 读取 `0015`、`0063`、`0115`；`rg -n "linggan_material_content_author_at"` | §6.2.1 排除作品作者；作者问题集中度 | 使用 `linggan_material_content_author_at(..., as_of)`；NULL 语义见 Q-05 |
| 7 | 首次采集时间独立列 | 读取 `0016`、`0125`、`0004`；搜索 `first_admission` | T0.2 `observed_at` 回填 | 可从 `first_admission` + `package.accepted_at` 推导；是否新增列见 Q-09 |
| 8 | 向量相似度检索调用 | `rg -n "atom_embedding\|<=>\|cosine" crates apps --glob '!*test*'` → 仅 node_modules/注释 | T2.2 向量来源 | 向量存储有、检索调用未确认；见 Q-10 |
| 9 | 评论者身份稳定性合同 | 读取 `0016:38`、`docs/data-contracts/comment-study-http-001.md:43`；搜索 `author_external_id` 的稳定性说明 | T0.6 评论者哈希 | `author_external_id` 可空；未知作者不能独立支持；密钥/语义见 Q-08 |
| 10 | 稳定 `comment_id` 类型 | 读取 `0016:27`、`0025:6-24`；对比 `material_ref` 与 `(content_public_ref, comment_external_id)` | T0.5 `comment_mention`；§6.2 去重 | 推荐稳定复合身份；见 Q-06 |
| 11 | 唯一“用户问题”实体 | 读取 `0064:205-240`、`0177:39-55`；`rg -n "linggan_comment_problem_definition\|linggan_comment_research_problem_definition" crates apps --glob '!*test*'` | T0.5 `source_ref` | 生产读 0177；旧 0064 仍存在；见 Q-07 |
| 12 | 新 AI 任务的通用外发许可 | 读取 `0131:176-191`、`0177:12-26`；`rg -n "ai_batch_task_kind_valid\|ai_egress_material_task_valid" database/migrations` | T1.3/T2.4 机会卡 AI 任务 | 需显式注册 task/batch/material_class；不是现成放行 |

## 7. 补充勘察（T0.1b，2026-11-12）

> 只读声明：本节数据库查询均为 `SELECT`，只记录聚合数字，未读取或写入评论正文；未修改代码、未执行 migration、未写数据库。

### 7.1 基线同步与差异勘察（D-001、D-002）

- 新基线：`origin/main` @ `4d65e506`。主工作树的 `docs/README.md` 同时有他人未提交改动和上游改动，`git pull --ff-only` 会覆盖本地改动；按负责人裁定改用独立 worktree `.worktrees/opportunity-card-20261112`，分支 `claude/opportunity-card-20261112`。主工作树未改动。
- 迁移链头：本地 ledger `SELECT max(migration_id) FROM linggan_local_schema_migration` → `0202_topic_core_merge_prompts`；迁移文件 203 个。
- `ad19a4d7..HEAD` 在勘察路径中的变更文件数（`git diff --name-only ... | wc -l`）：

| 路径 | 变更文件数 |
|---|---|
| `database/migrations/` | 7（全部为新增 0196–0202） |
| `crates/intelligence/` | 97 |
| `crates/evidence/` | 14 |
| `apps/api/src/local_web/` | 16 |
| `apps/web/src/app/` | 2（`navigation.ts`、`shellOwnership.test.ts`） |
| `docs/data-contracts/` | 7（新增 content-card 合同/ schema，修改 ops 与 research-dossier） |
| `env.example` | 0 |

- 新增 migration 的 DDL 目标：0196 内容理解请求/结果 + AI 约束；0197 ops 交接；0198 深化理解表；0199 深化请求 + AI 约束；0200 topic automation + batch 约束；0201 topic angle handoff；0202 仅 prompt 数据。**没有**修改 `linggan_material_comment`、`linggan_material_content`、`linggan_material_domain_usage`、`linggan_comment_problem_*` 的列或约束（对 0196–0202 的 `rg` 只命中 `domain_ref`/`work_ref` 外键与 AI 任务/外发约束）。
- 受影响并已更新的 RECON 项：§1.2/§5.2 路由行号、§5.1 模块数、§2 迁移数量与链头、§3.2 评论时间、§3.7 0064/0177、§4.1 `insert_batch` 与 AI 约束、§4.5 `model_secrets` 默认路径。`env.example`、`Cargo.toml`、`apps/web/package.json` 无变更。
- 既有失败重记（新基线）：`cargo test --workspace --no-fail-fast` → `test result: FAILED. 111 passed; 1 failed; 127 ignored`，失败仍为 `local_web::creator_lifecycle_tests::target_route_never_reflects_an_invalid_selected_work`（`apps/api/src/local_web/creator_lifecycle_tests.rs:226:5`，期望 200 实际 503）；`cargo fmt --check` 仍 exit 1，首处 diff 在 `crates/evidence/tests/support/standard_dossier_fixture.rs:31`。与 T0.1 基线属同一失败集合。

### 7.2 first_admission 覆盖率（D-007）

```text
$ docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -Atc "..."
identities_in_first_admission|29017
distinct_identities_in_current|29017
observations|31771
invalid_timestamptz|0
null_observed_at|0
```

- 没有 `linggan_material_comment_first_admission` 记录的稳定评论身份：**0**（29017/29017，100%）。
- 表定义：PK `(content_public_ref, comment_external_id)` + `first_package_ref`（`database/migrations/0125_collection_single_run.sql:136-145`）；首次插入时 `ON CONFLICT DO NOTHING` 写入（`:146-153`）；表本身不可变（`:154`）。
- 首次采集时间由 `first_package_ref → linggan_runtime_capture_package.accepted_at` 取得（`0004_plugin_runtime_all_capabilities.sql:42`）；`linggan_material_comment.created_at` 是入库时间，不能替代。
- `observed_at` 全部非空且 `pg_input_is_valid(observed_at,'timestamptz')` 为真（31771/31771），因此 D-012 的 `observed_at_unparseable` 在当前库为 0。

### 7.3 评论时间来源（D-008）

- 原始采集包有字段：`linggan_runtime_capture_package.payload.records[].payload.publishedAt`（毫秒 epoch）与 `.publishedAtText`。插件解析在 `plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:93-114`（`create_time / createTime / time`），展示文本生成在 `:458-492`。
- 规范化会回退到采集时间：`plugins/linggan-intelligence-browser/src/db/recordNormalization.js:162` 为 `publishedAt = normalizeNumber(record.publishedAt || record.createdAt, 0)`。因此 `0` 不能当“无时间”，只能当“未知/采集时间回退”。
- 合同 fixture 没有评论时间：`crates/contracts/tests/producer_execution_validation.rs:69-73` 的 comments package fixture 是 `"records":[]`；唯一 `publishedAt` fixture 是作品级 `publishedAtSourceText`（`crates/contracts/tests/discovery_boundary_contract.rs:31`）。
- 数据库聚合（`comments` + `replies` 包，只读）：

```text
records|31809
nonzero_epoch_13|45
zero|31764
has_text|31809
text_relative(前)|5294
text_dash(-)|24426
text_colon(:)|2119
text_other|15
```

- 精确平台时间覆盖 **45/31809 = 0.14%**；其余 31764 条 `publishedAt=0`。`publishedAtText` 100% 有值，但 76.8% 是带 `-` 的展示日期（最多日精度），16.6% 是“N 天前”相对文本，6.6% 带时间冒号；均不是 UTC 时间戳。
- 观察表 31771 条与包记录 31809 条相差 38 条，T0.1b 未逐条核对差异原因。
- API 侧恒为 null：`crates/intelligence/src/comment_study_catalog/comments.sql:183` 写死 `'publishedAt', NULL`。派生视图 `linggan_ci_source` 会从 payload 提取（`database/migrations/0048_comment_intelligence.sql:40-42`），但没有落到评论表或机会卡可用的列。
- 结论：远未达到 D-008 v2 的“覆盖率 ≥90%”。新阻塞见 `QUESTIONS.md` Q-11。

### 7.4 `linggan_material_domain_usage` 的 as_of 时间列（D-009）

- 有：`created_at timestamptz NOT NULL DEFAULT scope_001_now()`（`database/migrations/0103_domain_membership_and_usage.sql:137`）。行上还有 `role`、`basis_kind`、`request_ref`/`work_order_ref`/`package_ref`、`record_ordinal`（`:128-136`），后续增加 `use_kind`（`database/migrations/0180_creator_preview_discovery_only.sql:9`）。
- 过滤口径：`domain_ref = :d AND use_kind='domain_research' AND role='primary' AND created_at <= :as_of`；同一作品可能有多行（不同包/请求），统计前需按 `content_public_ref` 去重。
- 局限：`created_at` 是入库时间，不是平台观察时间。`legacy_domain_migration` 回填行的 `created_at` 是迁移执行时间（`0103:163-169`），不能证明“该作品在 as_of 时已属于该领域”。按 D-009 记为已知局限。

### 7.5 当前代码是否仍写 0064 的 problem 表（D-014）

```text
$ rg -n "INSERT INTO linggan_comment_research_problem|linggan_comment_research_problem\b" \
    --glob '!database/migrations/**' -g '!docs/**'
database/bootstrap/comment-study-reset.sql:37: ... linggan_comment_study_reset_count('linggan_comment_research_problem'),
database/bootstrap/comment-study-reset.sql:98:DROP TABLE IF EXISTS linggan_comment_research_problem;
```

- 没有生产 `INSERT`；唯一命中是开发重置脚本的计数和 `DROP`。当前生产读取使用 0177 的 `linggan_comment_problem_definition` / `_membership`（`crates/intelligence/src/ai_foundation/comment_problem.rs:38`；`apps/api/src/local_web/comment_study_command_api.rs:141-149`）。
- 结论：0064 旧 problem 表**不再被写入**，可只保留为历史；与 D-014 一致。

### 7.6 0177 是否有合并、取代或撤销语义（D-014）

- 没有合并/取代列：`linggan_comment_problem_definition` 只有 `definition_ref`、`problem_ref`、`domain_ref`、`identity_hash`、`label`、`actor`、`goal`、`barrier`、`context`、`source_request_ref`、`source_signal_ordinal`、`review_invocation_ref`、`created_at`（`database/migrations/0177_comment_problem_workflow.sql:39-49`）；没有 `merged_into`、`superseded_by`、`valid_from/valid_to`、`withdrawn_at`。
- `problem_ref uuid NOT NULL UNIQUE`（`:41`）：一个问题只能有一条 definition。因此 D-014 的“同一 `problem_ref` 的 `definition_ref` 变了，`def_version` 加 1”在当前 schema 下**不会发生**；同步任务应走“无变化、不写入”的路径。
- 归属只追加：`linggan_comment_problem_membership` 主键 `(request_ref, signal_ordinal)`（`:51-55`），不记录“本次归属取代上次归属”。两张表都不可变（`:56-58`）。
- 撤销只存在于请求级：`linggan_comment_research_cancellation(request_ref, command_ref)`（`0177_comment_problem_workflow.sql:65-68`）取消一次研究请求，不是撤销某个问题或定义。
- 结论：0177 没有合并/取代/问题级撤销语义。按手册 T1.1 的映射规则，`problem_lineage` 只建表、不写谱系（`docs/opportunity/playbook.md:1078`）。

### 7.7 每篇作品评论分布与多次采集（D-016）

```text
works|1735  p50|14  p90|30  max|731
works_total|1735  multi_pkg_works|1450  single_pkg_works|285  max_pkgs_per_work|28
identities|29017  in_2plus_packages|1478  max_packages_for_one_identity|12
multi_identities|28423  admitted_after_work_first_package|11643
```

- 每篇作品已采评论数（按稳定身份去重）：P50 = 14，P90 = 30，最大 731；共 1735 篇作品。
- 1450/1735 篇作品有 ≥2 个评论采集包，单作品最多 28 个包；说明重复采集是常态。
- 重复采集**会补到新评论**：在多包作品的 28423 个身份中，11643 个（41.0%）的首次入档包晚于该作品最早的评论包。
- 重复采集也会重复观察旧评论：1478 个身份（5.1%）在 ≥2 个包中出现，单身份最多 12 个包。
- 因此“本期新评论”不能按观察行计数；必须按稳定身份 + `first_admission` 判新（D-007、D-011）。

### 7.8 图片识别文字与口播转写的存储位置（D-017）

- 处理任务：`linggan_media_processing_job.processor_kind IN ('thumbnail','image_ocr','audio_extract','asr','video_frame_ocr')`（`database/migrations/0004_plugin_runtime_all_capabilities.sql:141-150`）；生命周期是追加事件（`:154-160`）。
- 产物：`linggan_media_derivative.derivative_kind IN ('thumbnail','ocr_text','audio','asr_text','frame_ocr_text')`（`0004:162-171`）。
- 文字：`linggan_material_derived_text.kind IN ('ocr_text','asr_text','frame_ocr_text')`，含 `text_content`、`display_text`、`source_location`（`database/migrations/0024_media_processing_runtime.sql:42-58`）。
- 研究阅读层：`linggan_material_unit.kind IN ('body','image','audio','video_frame')`（`database/migrations/0146_research_dossier_v2.sql:4-24`）；`linggan_material_reading.producer_kind IN ('platform','ordinary_ocr','local_vision','cloud_vision','asr','frame_ocr','human_edit')` + `blocks jsonb`（`0146:27-44`）。
- 整篇材料快照：`linggan_note_material_fragment.field IN ('title','cover_headline','body','tags','ocr','transcript')`（`database/migrations/0130_note_material_snapshot.sql:27`）。
- 结论：OCR 文字与口播转写都有存储位置，且有多层（derived_text / reading / fragment）。T1.2 的“图片识别文字”和“口播转写”不需要新建存储。

### 7.9 HMAC、`model_secrets` 与前端只读状态（D-013、D-006）

- HMAC：`fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String`（`crates/evidence/src/collection_control.rs:1432`）**不是 `pub`**；模块声明是 `mod collection_control;`（`crates/evidence/src/lib.rs:7`），也未公开。现有调用只在同文件 `:686`。RFC 4231 测试在 `:3235-3243`，继续有效。按 D-013，可把函数改为 `pub`（行为不变），不能复制实现。
- digest version：现有账号 digest 写 `'hmac-sha256-v1'`（`collection_control.rs:690`）；D-013 的评论者版本另定为 `commenter-hmac-sha256-v1`。
- `model_secrets` 复用性：`LocalFileModelSecrets::from_runtime()` 是私有函数（`crates/intelligence/src/model_secrets.rs:33`），且读取的是 `model-secrets/<workspace>/<reference>` 固定布局，**没有通用的“读取任意密钥文件”函数**。可复用的是路径解析与私有权限校验的写法，不是函数本身。
- **默认路径不一致**：代码的缺省是 `$HOME/Library/Application Support/Linggan Intelligence`（`model_secrets.rs:39-43`；同样的缺省见 `apps/worker/src/bin/media_worker_support/mod.rs:104-110`、`paddle_ocr.py:19-23`、`docs/runbooks/paddle-ocr-runtime.md:15`），而 D-013 写的是 `$HOME/.linggan-support`。新阻塞见 `QUESTIONS.md` Q-12。
- 前端只读状态写法（可参照）：`apps/web/src/pages/topics/TopicAutomation.tsx:17-26` 用 `AbortController` + `requestJson` 读取，`useState` 保存，卸载时 abort；`:27-35` 提供重试；`:36-47` 写操作带 `commandRef` 与 `expectedRevision`，成功后用响应替换本地状态；`:48-50` 用 `disabled` / `aria-busy` / `role="status"` 表达未就绪与错误。`requestJson` 在 `apps/web/src/shared/api.ts:52`。机会卡只读页可按同一模式，不需要新的状态库。

### 7.10 新阻塞

- `QUESTIONS.md` Q-11：评论平台时间精确覆盖率仅 0.14%，达不到 D-008 v2 的 90% 门槛。
- `QUESTIONS.md` Q-12：D-013 写的 `LINGGAN_SUPPORT_DIR` 缺省路径与代码实际缺省不一致。

