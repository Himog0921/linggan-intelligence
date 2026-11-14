# RECON（代码库勘察）

勘察日期：2026-11-12
勘察人：Claude Code（执行 agent）
仓库：`/Users/moglenny/proma/linggan-intelligence`

> 状态: 一次性报告
> 最后核对: 2026-11-12
> 适用范围: 机会卡 T0.1、T0.1b 与 C1 采集侧勘察的代码库结论。
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
- 结论：D-008 v2 要求的“发布时间可用率 ≥90%”在当前链路上**远未达到**（原始精确 epoch 仅 0.14%）。这是一条新阻塞，已记入 `QUESTIONS.md` Q-11，并由 D-021、D-028 答复。

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
- 结论：远未达到 D-008 v2 的“覆盖率 ≥90%”。已记入 `QUESTIONS.md` Q-11，并由 D-021、D-028 答复。

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
- **默认路径不一致**：代码的缺省是 `$HOME/Library/Application Support/Linggan Intelligence`（`model_secrets.rs:39-43`；同样的缺省见 `apps/worker/src/bin/media_worker_support/mod.rs:104-110`、`paddle_ocr.py:19-23`、`docs/runbooks/paddle-ocr-runtime.md:15`），而 D-013 写的是 `$HOME/.linggan-support`。已记入 `QUESTIONS.md` Q-12，并由 D-022 答复。
- 前端只读状态写法（可参照）：`apps/web/src/pages/topics/TopicAutomation.tsx:17-26` 用 `AbortController` + `requestJson` 读取，`useState` 保存，卸载时 abort；`:27-35` 提供重试；`:36-47` 写操作带 `commandRef` 与 `expectedRevision`，成功后用响应替换本地状态；`:48-50` 用 `disabled` / `aria-busy` / `role="status"` 表达未就绪与错误。`requestJson` 在 `apps/web/src/shared/api.ts:52`。机会卡只读页可按同一模式，不需要新的状态库。

### 7.10 新阻塞（均已答复）

- `QUESTIONS.md` Q-11：评论平台时间精确覆盖率仅 0.14%，达不到 D-008 v2 的 90% 门槛。已由 D-021 答复（v1 保持 `work_published_cohort`，不解析展示文本，采集侧修复移交），并受 D-028 修订（`comment_published` 成为阶段 C 的目标口径）。
- `QUESTIONS.md` Q-12：D-013 写的 `LINGGAN_SUPPORT_DIR` 缺省路径与代码实际缺省不一致。已由 D-022 答复（以代码实际缺省为准，提取共享 `pub` 解析函数）。

## 8. 采集侧补充勘察（C1，2026-11-12）

范围：只读代码与文档，未访问平台、未修改代码、未写数据库。目标是确认评论时间与采集协议的现状，为 C2–C4 提供事实。需要负责人裁定的排序协议见 `QUESTIONS.md` Q-13。

### 8.1 评论请求的排序方式

- **当前请求没有排序参数**。小红书顶层评论请求由 `buildXhsCommentPageRequestUrls` 构造，查询参数只有 `note_id`、`cursor`、`top_comment_id`、`image_formats` 和可选的 `xsec_token`（`plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:356-370`）。子评论请求同理，另有固定 `num: 10`（`:372-390`，`:376`）。
- **代码把排序标成未知**：小红书评论记录的 `sortMode` 固定写 `'unknown'`（`plugins/linggan-intelligence-browser/src/platforms/xhs/commentCollector.js:708`、`:1014`、`:1055`；默认参数见 `commentApi.js:450`、`:519`）。对比抖音有 `hot`/`latest` 两档并写入请求（`plugins/linggan-intelligence-browser/src/platforms/douyin/commentApi.js:222-223`），小红书没有对应逻辑。
- **能力登记只证明“默认顺序”**：`docs/platforms/xiaohongshu/capture-capability-registry.md:69` 记录当前默认顺序已 `VERIFIED`；同一文件 `:116` 明确列出“默认排序”尚未验证。仓库中没有平台支持按时间排序的证据。
- **结论**：现状是平台默认顺序，不是显式的时间排序。平台是否支持 `latest`/时间排序：**未验证**，不得推断。机会卡统计应采用哪种排序协议见 `QUESTIONS.md` Q-13。

### 8.2 单次评论条数上限 30 的来源

- 标准详情窗口的硬上限是 `MAX_ATTACHED_COMMENTS = 30`（`plugins/linggan-intelligence-browser/src/linggan/localExecutionSupport.js:23`），实际取值是 `min(页面公开数, 请求值, 30)`（`:177-181`）。
- 详情页会话计划也拒绝大于 30 的 `commentLimit`（`plugins/linggan-intelligence-browser/src/linggan/detailPageSessionStore.js:105-109`）。
- 批量控制器默认 `settings.commentLimit ?? 30`（`plugins/linggan-intelligence-browser/src/platforms/xhs/batchController.js:427`），该值可配置；深采 `all_public` 是独立的采集范围，不受标准 30 条窗口限制（`docs/platforms/xiaohongshu/capture-capability-registry.md:68`、`:96`、`:122`、`:130`）。
- 分页层面，主评论最多补 6 页、每个根评论最多补 6 页（`commentApi.js:605-606`），子评论每页固定 10 条（`:376`）；这是浏览器内补页上限，不是服务端配额。
- **结论**：30 是标准详情窗口的固定上限，批量/深采可通过 `commentLimit` 配置；机会卡若依赖“最新 30 条”，协议必须写明取的是默认顺序下的 30 条。

### 8.3 包与记录上的版本字段

- 采集包 `contractVersion` 固定为 `linggan.producer.capture-package.v1`（`plugins/linggan-intelligence-browser/src/linggan/producerRuntime.js:22`、`:81`；Rust 侧常量 `crates/contracts/src/producer_runtime.rs:14`）。包内的 `records` 是不透明 JSON 数组（`producer_runtime.rs:183`、`:208-210`）。
- 任务/尝试/提交的信封版本分别是 `linggan.producer.task-spec.v2`、`linggan.producer.attempt.v1`、`linggan.producer.capture-package.v1`（`plugins/linggan-intelligence-browser/src/linggan/adapter.js:2-4`；Rust 常量 `producer_runtime.rs:12-14`）。
- 评论采集回执有一个独立的协议字段 `commentCollection.version`，当前固定为 `1`（`plugins/linggan-intelligence-browser/src/platforms/xhs/captureReceipt.js:113`；`producerRuntime.js:250`）；Rust 校验要求它**恰好等于 1**（`crates/contracts/src/producer_runtime.rs:539`）。
- 插件版本来自 manifest（`plugins/linggan-intelligence-browser/manifest.json:4`、`package.json:3`，当前 `0.8.66`），只在工位报到时发送（`plugins/linggan-intelligence-browser/src/linggan/background.js:358`、`adapter.js:104-137`），**不写入采集包或记录**。
- **能否区分改造前后的包**：现有 `contractVersion` 与 `commentCollection.version` 都能作为分界，但两者都是当前固定值，直接升级会同时影响所有评论包。更细的“采集协议版本”（排序方式、单次上限、触发来源）目前不存在；C2 需要新增字段或升 `commentCollection.version`，并由 Rust 校验与 fixture 同步。

### 8.4 评论记录 `publishedAt` 的类型与校验

- **包合同不校验单条评论字段**：`CapturePackageWire.records` 是 `Vec<Value>`，Rust 只校验包级字段、`coverage` 和 JSON 可存储性（`crates/contracts/src/producer_runtime.rs:183`、`:409-427`、`:443-452`）；注释明确 `records` 保持不透明（`:208-210`）。因此 `publishedAt` 是否允许 `null` **不是合同层限制**。
- 插件规范化当前把缺失时间写成 `0`：`record.publishedAt || record.createdAt` 后 `normalizeNumber(..., 0)`（`plugins/linggan-intelligence-browser/src/db/recordNormalization.js:162`、`:184`）；解析函数对缺失/非法值也返回 `0`（`plugins/linggan-intelligence-browser/src/platforms/xhs/commentApi.js:93-102`）。
- 现有 comments fixture 的 `records` 都是空数组（`crates/contracts/src/producer_runtime.rs:912`、`:965`；`crates/contracts/tests/producer_execution_validation.rs:69-73`），没有评论时间样本。
- **改为允许 `null` 要改哪里**：
  1. 插件规范化：`recordNormalization.js:162` 不再回退 `createdAt`，缺失时保留 `null`；同步评论相关测试。
  2. 合同与 fixture：合同层无需改类型，但应新增一条带 `"publishedAt": null` 与非空毫秒 epoch 的 comments fixture，锁定两种取值（现有 fixture 为空，新增不会破坏旧断言）。
  3. 下游读取：`0048` 的提取逻辑本身对 `null`/`0` 都返回 `NULL`（见 8.5），但任何把 `publishedAt` 当数字直接相加/排序的代码都要检查；本仓库现有读取点主要是 `0048` 与评论研究 API（后者恒写 `NULL`，`crates/intelligence/src/comment_study_catalog/comments.sql:183`）。

### 8.5 `0048` 的时间提取能否复用

- `linggan_ci_source` 的提取条件是：`payload.data->>'publishedAt'` 匹配 `^[0-9]{12,15}$` 且数值小于 `253402300800000`，才 `to_timestamp(ms/1000)`，否则 `NULL`（`database/migrations/0048_comment_intelligence.sql:40-41`）。`publishedAtText` 截断到 100 字符另存（`:42`）。
- 该表达式可直接复用为 C3 投影器的提取语义：`null`、`0`、非数字、超范围值都落 `NULL`；接受 12–15 位毫秒 epoch。
- 限制：它目前写在视图里，依赖 `linggan_comment_research_current`、`linggan_runtime_capture_package` 和 `linggan_ci_comment_identity` 的连接及限制过滤（`0048:46-52`），**不是独立函数**。C3 若要按稳定身份全量投影，需把这段判定提取为共享 SQL 函数或复用同一表达式；不要另写第二份不同的判定。

### 8.6 重新采集的现有入口与控制

- **入口一（按作品补评论）**：主题页 `deepen_comments`（`apps/web/src/pages/topics/engine.tsx:1644`）→ `POST /api/local/topic-map/collection/commands`（`apps/api/src/local_web/topic_map.rs:96`）→ `CollectionCommand::DeepenComments`（`crates/intelligence/src/topic_map_research_collection.rs:35-45`；合同说明见 `docs/data-contracts/topic-map-http.md:96`）。
  - 每次只允许 1 个作品（`:151-166`），领域采集策略必须已启用（`:168-181`）。
  - 同一 `scope_hash` 已有活动轮次，或未传 `newRound` 时复用轮次（`:185-190`）。
  - 本轮冻结 `comment_limit = 30`、`reply_expand_limit = 2`，不取媒体、不做 OCR/ASR（`:197-201`）。
  - 评论预算：`knownCommentIds` 快照最多 5000 条（`:229-232`），`new_unique_limit = 30`、`max_scroll_rounds = 20`、`max_duration_seconds = 180`（`:233`）。
- **入口二（通用材料深采）**：`POST /api/local/collection/material-deepening`（`apps/api/src/local_web.rs:325`，处理函数 `:1185`），接受明确的公开材料身份集合；只排队，不分配工位或 Lease（`:1179-1184`）。
- **授权与频率控制**（两条入口共用）：
  - 采集授权表 `collection_acquisition_authorization`（`database/migrations/0006_collection_acquisition_chain.sql:16`）；准入时 `granted_by = 'person'`（`topic_map_research_collection.rs:203-212`）。
  - 授权失败码包括 `authorization_missing`、`authorization_scope_mismatch`、`authorization_purpose_mismatch`、`authorization_target_limit_reached`、`authorization_work_unit_limit_reached`、`authorization_expired_or_revoked`（`crates/evidence/src/collection_control.rs:2346-2353`）。
  - 风险暂停与插件冷却：`collection_risk_pause`、`collection_installation_risk_cooldown`（`collection_control.rs:1069-1091`）；账号冷却 `account_cooling`（`:1348`）；另有平台并发上限与 Lease 抢占。
- **重新采集一篇作品的评论需要走**：启用该领域的采集策略 → 确认目标/授权覆盖 `material_deepening` → 提交 `deepen_comments`（或材料深采接口）→ 服务端冻结轮次与评论预算并创建 Work Order → 插件以现有工位/账号/Lease 领取执行 → 实际取得结果按稳定评论 ID 接纳；重复 ID 不占新额度，超过 30 条新评论需要下一轮。
- **对 C3/C4 的直接含义**：现有入口一次最多新增 30 条，历史回填需要按周/按轮多次执行并保留协议版本；风险暂停、冷却、授权过期都会中断回填，必须记录停止原因，不能把“未采到”当成“没有评论”。

### 8.7 负责人裁定（已答复）

- `QUESTIONS.md` Q-13：负责人选 A，固定平台默认顺序，`collection_protocol.sort_mode = "platform_default"`，不实现显式时间排序。见 D-032。C1 只能确认现状是平台默认顺序；平台是否支持时间排序**未验证**，C2 也不依赖该能力。

## 9. T1 补充勘察（2026-11-13，`843e75cc`）

- 问题登记：`database/migrations/0205_opportunity_problem_taxonomy.sql:10-20` 以 `problem_ref UUID` 为主键；`source_ref`、`source_definition_ref` 是已有问题和定义引用。同步入口为同文件的 `linggan_opportunity_sync_problems()`（`:78-111`），谱系解析入口为 `linggan_opportunity_resolve_problem_id(uuid)`（`:113`）。单一 UUID 身份的裁定见 D-035。
- 分类版本：同文件 `:4-8` 中 `linggan_opportunity_taxonomy_version` 每领域只有一个当前版本和 `updated_at`；`:40-76` 的谱系触发器在 merge/split 时递增它。本表本身不能提供已被覆盖的版本时间。
- 0177 的归属键为 `(request_ref, signal_ordinal)`（`database/migrations/0177_comment_problem_workflow.sql:51-55`），并非单列 membership ID。`crates/intelligence/src/ai_foundation/comment_problem/publish.rs:106-118` 负责写入；原始评论观察经 `linggan_comment_research_request.source_ref` 关联（`database/migrations/0174_comment_research_execution.sql:26-39`）。T1.1 的 `source_membership_ref` 使用 `<request_ref>:<signal_ordinal>` 可逆编码，关联事实仍由 0177 保存。
- 评论归属发布发生在 `crates/intelligence/src/ai_foundation/executor/service/comment_problem.rs:156-174` 的已有事务中；该事务随后保存回执并提交（`:176-202`）。新功能不能让关闭开关时的评论研究行为发生变化。
- `CommenterDigest` 提供 `load()`、`from_key()`、`hash()` 和 `digest_version()`（`crates/intelligence/src/opportunity/commenter_hash.rs`）；生产写入使用 `load()`，测试可用明确的合成密钥。当前配置读取入口为 `load_current_metric_definition()`（`crates/intelligence/src/opportunity/metric_definition.rs:82`）。
- 基线 `843e75cc` 未发现 `LINGGAN_OPPORTUNITY_CARD` 的代码或环境模板引用（`rg -n 'LINGGAN_OPPORTUNITY_CARD' apps crates env.example database`，无输出）；T1 的写入入口须按 D-006 新增并遵守默认关闭。
- 逐问题人数：`apps/api/src/local_web/comment_study_command_api.rs:141-149` 的问题接口调用 `comment_problem/read.rs:4-40`；后者每问题最多读取 100 条来源合格依据，不输出去重评论者人数。发布资格在 `comment_problem.rs:38-68` 中检查初始核验的独立作者，不等同于机会卡全部有效提及人数。手册 T1.1 对账口径的冲突见 Q-16。
- 评论平台时间时点：`0208_comment_platform_time_day_precision.sql:31-47` 的当前视图先按精度选择，`:411-413` 的资格函数直接连接它，未限制时间行创建或其来源包接纳时刻不晚于 `as_of`。Q-17 已由 D-037 裁定，需前向修复并共享一套选择逻辑。
- 提及投影依赖的现有版本字段：`linggan_opportunity_comment_fact` 的稳定身份和首入档评论者取值见 `database/migrations/0204_opportunity_comment_eligibility.sql:12-68`；评论研究请求的输入指纹见 `0174_comment_research_execution.sql:26-39`；复核调用及其模型引用见 `0040_model_pi.sql:30-36,66-86` 和 `0138_ai_gen1_invocation.sql:34-36`；冻结任务配置的提示词版本见 `0170_ai_prompt_versions.sql:11-24`。所有缺失来源信息必须报错，不以占位值冒充。
- T1.2 时点依赖：`linggan_material_comment_first_admission` 的键和首次包在 `0125_collection_single_run.sql:136-153`，该表没有创建时间；评论观察的 `created_at` 在 `0016_material_social_lanes.sql:26-45`，来源包的 `accepted_at` 来自运行包。共享作者函数及其详情、主页发现来源在 `0115_creator_discovery.sql:4-31`，其他调用方包括 `crates/evidence/src/material_query_sql.rs:208`、`crates/evidence/src/collection_run.rs:221` 与 `crates/intelligence/src/ai_foundation/note_analysis/source.rs:53`。时点修复按 D-037、D-039、D-040 前向实施。
- T1.2 当前候选：统一时点选择与共享作者修复位于 `database/migrations/0212_opportunity_platform_time_as_of.sql`；协议分组的指标表和影子定义位于 `0213_opportunity_problem_metric_period.sql`。计算入口 `crates/intelligence/src/opportunity/problem_metrics.rs`，命令入口 `apps/worker/src/bin/opportunity.rs`。以上是工作分支的候选，尚未表示运行环境已迁移。
- 后续切换冲突：`0203_opportunity_metric_definition.sql` 的当前指针是全局 singleton，而 D-028 的覆盖率门是逐领域的。T1.2 只同时计算当前口径及影子口径，不移动指针；在实施按领域切换前须依 §3.3 将全局指针与逐领域切换的冲突提交负责人裁定，不能直接切换全部领域。

## 10. T1.3 补充勘察（2026-11-13，`263435f3`）

- `linggan_opportunity_eligible_comments(domain, as_of, tz, basis)` 返回稳定评论身份、评论者、`work_author_external_id` 与 `period_anchor_at`，并统一筛除无资格评论（`database/migrations/0204_opportunity_comment_eligibility.sql:170-195`）。`0212_opportunity_platform_time_as_of.sql:135-205` 采用截至时点的最新合格作品详情与共享作者函数；作品发布周口径的锚点就是作品发布时间，评论发布时间口径的锚点是评论时间。
- 作者窗口固定按作品发布时间，无论周期指标的口径是什么（手册 §6.2.4）。T1.3 对评论发布时间口径复用同一作品发布周资格函数获取作品发布时间，不另写最新详情或作者的选择规则；仍没有合格作品时间时单独计数，不从旧详情回退（D-041）。
- 现有问题指标的输入、协议归属和来源提及由 `crates/intelligence/src/opportunity/problem_metrics/inputs.rs` 读取，计算事务与增量指纹由 `problem_metrics.rs` 的 `rebuild_problem_metrics` 协调；线上后台入口复用该函数（`crates/intelligence/src/ai_foundation/executor/service/opportunity_sync.rs`），手工重建入口为 `apps/worker/src/bin/opportunity.rs`。
- Wilson 区间由 `crates/intelligence/src/opportunity/stats.rs:17` 的共享 `wilson_interval` 计算；I7 现有定义在 `crates/intelligence/src/opportunity/invariants.rs` 的 `sufficient_creator_meets_minimum`，T1.3 建表后须补按领域及 `as_of` 的检查。
- Q-22 已答复：作者集中度沿用首次入档协议，未知单独成组并按组排序；后续卡片标明组别，见 D-042。协议是同一派生指标的维度，不引入平行评论事实。

## 11. T1.4 前置勘察（2026-11-13，`b9a8f324`）

- 问题周期指标表以 `protocol_version` 为同一指标的分组维度，未知为 NULL（`database/migrations/0213_opportunity_problem_metric_period.sql:9-28`）；作者指标和组内排序见 `crates/intelligence/src/opportunity/problem_metrics/creator_density.rs`。手册 §6.7 的需求栏未提供对应协议组，见 Q-23。
- `linggan_comment_problem_membership` 的归属对象来自当前评论研究结果；问题归属阶段只选择合格的 `problem` / `need`（`crates/intelligence/src/ai_foundation/comment_problem.rs:22-31`）。现有 `solution` / `experience` 信号没有问题框架（`crates/intelligence/src/comment_study_semantic.rs:51-62`），不能据此证明办法与指定问题的关系，见 Q-24。
- 阶段 1 不执行供给搜索与判定，但 §6.7 的 `supply.counts` 不允许空值；R5 要求未知不冒充 0，见 Q-25。
- 作者资料的 `display_name_state` 允许 `UNKNOWN`（`database/migrations/0016_material_social_lanes.sql:63-82`）；作者归属只保证作者身份，不保证资料记录或姓名。§6.7 的 `CreatorDensity.name` 必填，见 Q-26。
- 代表原声可能因来源受限而撤回；来源限制记录位于 `database/migrations/0094_corpus_evidence_read_recovery.sql:7-13`，既有 I9 位于 `crates/intelligence/src/opportunity/invariants.rs:131-147`。后续实现必须同时验证防抖窗口内的撤回安全和重建后补位，不能只证明重建前的旧副本仍可读。
- 点赞投影 `linggan_material_comment_engagement.created_at` 可晚于评论和包的时间（`database/migrations/0122_comment_like_library.sql:4-29`）；原评论研究 `like_heads` 只按评论和包筛选时点。Q-27 已由 D-047 答复，当前工作树把选择逻辑抽至 `crates/intelligence/src/comment_study_catalog/like_heads.sql` 并增加投影创建时间限制；隔离评论目录回归已通过，T1.4 整体尚未验收。
- 已确认的 D-040 需要共同的按时点作者选择；原评论研究的 `selected_source_in`、目录 `facts.sql` 及候选 SQL 连接当前作者视图。当前工作树已将它们收敛至 `linggan_material_content_author_at`（`crates/intelligence/src/comment_study_source.rs:73`、`crates/intelligence/src/comment_study_catalog/facts.sql:108`、`crates/intelligence/src/comment_study_source/candidates.sql:49`）；专项跨模块回归仍待完成。
- §8.2 原称 11 月 2 日为当前周，但 `as_of=2026-11-10T00:00:00Z` 的上海日历周从 11 月 9 日开始。Q-28 已由 D-048 裁定按真实日历周，原 W0 的 11 月 2 日单周标准值保持不变；卡片 `current` 应表示 11 月 9 日开始的周。
- 作品互动统一来源为 `linggan_material_engagement_observation`（`database/migrations/0023_material_engagement_and_media_components.sql:29-50`），覆盖详情与发现两条来源；证据库只从 `KNOWN` 观察中选最新值（`crates/evidence/src/material_query_sql.rs:77-83`）。T1.4 的互动参考可用性复用该视图，限制观察创建和来源包接纳时间；作品发布时间锚点仍按 D-041 选截至时点的最新详情。
- 作者资料为 `linggan_material_author_profile`（`database/migrations/0016_material_social_lanes.sql:63-87`），姓名和粉丝数分别有 `KNOWN/UNKNOWN` 状态；目标与领域绑定见本文件 §3.4。T1.4 作者组排序复用 `sort_creator_density_group`（`crates/intelligence/src/opportunity/problem_metrics/creator_density.rs:135`），不另造排序口径。
- 评论目录与机会卡共同使用 `comment_study_catalog::like_heads_sql`；时点作者共用 `linggan_material_content_author_at`。共享入口回归覆盖点赞、作者和研究资格，不依赖真实采集。
