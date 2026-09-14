# Linggan Intelligence 全项目梳理：证据与复核附录

> 状态: 一次性报告
> 最后核对: 2026-09-05
> 适用范围: `AUD-INTELLIGENCE-20260905` 主报告的证据目录、检查范围、结果与复核方法
> 事实来源: 固定主线 `4864edd466d379dc7c9d0e7000e4e5cb3ec3d111`、GitHub 只读 API、本机 PostgreSQL 只读快照、CUA 浏览器观察与本轮测试
> 冲突时以谁为准: 更新的真实代码和运行证据；本附录不将测试记录、旧回执或页面读取提升为业务验收

返回[主报告](intelligence-system-review-2026-09-05.md)。本附录不包含账号原始身份、评论原文、凭据、数据库连接串或敏感媒体。

## E01：仓库、分支与调查范围

- 权威开发仓库：`/Users/moglenny/proma/linggan-intelligence`。
- 远端 `main`：`4864edd466d379dc7c9d0e7000e4e5cb3ec3d111`。开始及正文编写前两次 `git ls-remote origin refs/heads/main` 一致。
- 共享 checkout：`8df0b2d3ccbea0653e2a5f779ab308c2ed92d9b6`，落后固定主线 30 个提交。原有修改为 `docs/README.md`、`docs/design/README.md`、`docs/progress/2026-09.md`；8 个历史插件 ZIP 在该 checkout 被删除。审计未修改这些文件或删除状态。
- 审计 branch：`codex/intelligence-system-audit-20260905`；目录位于仓库 `.worktrees/intelligence-system-audit-20260905`，符合当前 worktree 规则。
- 开始时另有 Collection control、五页 UI、工位名称与自动接活等 worktree；没有把这些工作目录直接当成已部署版本，也没有清理它们。
- 核心盘点范围：`apps/` 44 文件、`crates/` 101 文件、现役插件 `src/` 203 文件、35 个 migration、168 个已有文档文件。文件数在报告写入及依赖安装前统计，包含测试与样式，不作为成熟度评分。
- 历史 `references/` 未逐文件复审；只有主线实际连接的实现进入运行能力判断。

复核入口：`git status --short --branch`、`git rev-parse HEAD`、`git worktree list --porcelain`、`git ls-remote origin refs/heads/main`。本报告的代码行号均基于固定主线。

## E02：运行目录、进程与 health

本轮使用 `lsof -nP -iTCP:3000 -sTCP:LISTEN` 找到监听者，再以 `lsof -a -p <PID> -d cwd,txt -Fn` 核对实际进程来源。

| 进程 | PID | cwd / 实际程序 |
|---|---:|---|
| 正式 API | 21964 | `~/Library/Application Support/Linggan Intelligence/runtime-main` / `target/debug/linggan-api` |
| 正式巡检 worker | 21967 | 同一 runtime-main / `target/debug/linggan-worker` |
| 正式媒体 worker | 21971 | 同一 runtime-main / `target/debug/linggan-media-worker` |
| 额外预览 API | 14634 | 开发仓库 `.worktrees/main-preview` / `target/debug/linggan-api` |

runtime-main 的 Git HEAD 为固定主线，工作树干净。预览目录 HEAD 为 `d5b78863d8d56ad39664314229db02942fa4fd5d`。当前 `/health` 返回：

```text
HTTP 200
database.state = READY
database.schema = PLUGIN_RUNTIME_002_SCHEMA_READY
listener = loopback-only
dataState = LINGGAN_BROWSER_PRODUCER_RUNTIME
scheduler.state = running
scheduler.lastOutcome = idle
scheduler.lastError = null
```

代码入口：[local_web.rs](../../apps/api/src/local_web.rs)，`serve` 位于 429 行，health 位于 461 行；[巡检 worker](../../apps/worker/src/main.rs)。health 不携带独立 build SHA，不能只凭运行目录 HEAD 证明单个二进制的全部构建来源；本轮记录到 cwd/txt 和目录版本这一级证据。

## E03：数据库只读快照

主快照时间：`2026-09-05T11:19:43.635654Z`，即北京时间 19:19:43。使用 `BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY`，设置 15 秒 statement timeout；输出仅保留结构、状态、计数和时间。

数据库为运行中的 `linggan-intelligence-postgres-1`，镜像 PostgreSQL 16.14。`information_schema.tables` 的 public 表数量为 80。`linggan_local_schema_migration` 共 35 行；逐份比对 migration 文件 SHA-256，缺失 0、冲突 0。最新一项：

```text
0036_monitor_scheduling_clarity
applied_at = 2026-09-05T09:40:23.196579Z
sha256 = 0b2d3c9ed8525d27ce3cac56d620596210d41512b6acaad0cc98c803c1b84c47
```

| 表/记录类别 | 行数 | 计数的含义 |
|---|---:|---|
| `linggan_runtime_task` | 546 | 任务定义，不等于采到的作品 |
| `linggan_runtime_attempt` | 493 | 尝试，不等于成功 |
| `linggan_runtime_capture_package` | 488 | 包，包含过程包和多轮观察 |
| `linggan_runtime_submission_receipt` | 488 | 回执；18 为 COMPLETED_LIVE_STEP/ACCEPTED，415 为 NOT_APPLICABLE/ACCEPTED，55 为 UNKNOWN/UNKNOWN |
| `linggan_material_content` | 46 | 稳定作品身份 |
| `linggan_material_content_detail` | 19 | 详情观察，覆盖 3 个作品 |
| `linggan_material_author_profile` | 19 | 档案观察，覆盖 3 个作者身份 |
| `linggan_material_comment` | 2,427 | 历史评论观察，覆盖 3 个作品 |
| `linggan_material_comment_current` | 970 | 当前评论投影，覆盖 3 个作品 |
| `linggan_material_engagement_observation` | 280 | 互动观察 |
| `linggan_material_lane_observation` | 115 | lane 观察 |
| `linggan_material_derived_text` | 57 | 非空文本，覆盖 46 个作品；未经准确率评测 |
| `linggan_media_slot` | 68 | 逻辑媒体槽位 |
| `linggan_media_blob` | 60 | 独立字节身份 |
| `linggan_media_materialization` | 106 | 本地物化记录，可多次指向同一 Blob |
| `linggan_media_processing_job` | 118 | 处理 Job 定义 |
| Topic 的六张业务表 | 各 0 | workspace / definition / classification_run / material_pack / material_member / import_receipt |

旧入口 `capture_package`、`capture_record`、`record_processing_work`、`local_discovery_package` 均为 0。它们为空不能推导系统没有采集数据：当前材料在 `linggan_runtime_*` 与 `linggan_material_*`。

目标为 creator 3、keyword 1；lifecycle 为 paused 2、pending_decision 1、archiving 1；4 个 `monitoring_enabled` 全部 false。3 个目标有当前 fixed rule，`automatic_enabled` 全部 false；另一个已暂停目标没有活动规则。

## E04：插件版本、产物与资格

源码：[package.json](../../plugins/linggan-intelligence-browser/package.json)、[release manifest](../../plugins/linggan-intelligence-browser/releases/release-manifest.json)。本轮确认：

```text
current release = 0.8.37
sha256 = 1492f8d9fd454ca52aa5b7e324f83fb791696897fe36604ff97f1d89f91ad3fd
server minimum dispatch version = 0.8.34
```

最低准入版本来自 [collection_control.rs](../../crates/evidence/src/collection_control.rs) 第 18 行，与“最新可下载版本”分开。

服务端最近未被取代的已认领安装上报 0.8.37，最后心跳为 `2026-09-04T15:10:40.752503Z`，即北京时间 9 月 4 日 23:10:40。最近账号资格观察是 usable/authenticated，但已于同日 UTC 15:30:40 过期；50 条历史资格观察均已过期。有效工位 1、已停用工位 2；有效工位接活开关为 true。

这些记录证明版本曾报到，不证明当前 Chrome 实际加载状态。本轮工位页面明确显示 `installation_stale`；没有重载插件或发起平台访问。

## E05：真实链路关联与材料覆盖

Package 分布：author_profile 53、batch_checkpoint 300、comments 33、content_detail 21、discovery_search 6、media_slots 24、profile_discovery 18、replies 33，共 488。

完整性 SELECT 检查了每个 runtime Package 的 Task、Attempt、Receipt：task orphan 0、attempt orphan 0、receipt missing 0。检查没有重算全部 Package hash，也没有把关联存在视为来源内容正确。

正式控制链使用以下既有关系，不按标题或目标名猜测关联：

```sql
SELECT p.package_kind,
       count(*) AS packages,
       count(DISTINCT w.work_order_ref) AS work_orders,
       count(DISTINCT l.lease_ref) AS leases,
       count(DISTINCT p.task_id) AS tasks,
       count(DISTINCT p.attempt_id) AS attempts
FROM linggan_runtime_capture_package p
JOIN collection_work_order_lease_task lt ON lt.task_id = p.task_id
JOIN collection_work_order_lease l ON l.lease_ref = lt.lease_ref
JOIN collection_work_order w ON w.work_order_ref = l.work_order_ref
GROUP BY p.package_kind;
```

结果只有 author_profile 3 和 profile_discovery 16；相应 tasks/attempts/leases/work_orders 各为 3 和 16。这些是历史证明；0036 应用时间之后，该关联查询没有新 Package。

WorkOrder 共 44：deep_archive 7、patrol 37，dispatch_lane 与 queue_state 均为 legacy。Lease 共 45，released/completed 16、released/expired 27、released/revoked 2；无活租约。LeaseTask 为 completed 19、in_progress 9、pending 30。生命周期不做跨表压缩。

历史标准评论样本（省略外部作品身份）：`scope=detail_window`、`state=complete`、`uniqueCollectedCount=30`、`requestedLimit=30`、`pageCommentCount=233`。详情 Coverage 同时为 acquired 1、unknown 3。未把 30/30 当作 233 条全部完成。

关键实现：[producer_runtime.rs](../../crates/evidence/src/producer_runtime.rs) 第 864 行的 `submit_producer_package` 及事务提交；[work_order_lease.rs](../../crates/evidence/src/work_order_lease.rs)；[collection_tasks_view.rs](../../apps/api/src/local_web/collection_tasks_view.rs) 第 275–298 行的租约失效展示。

## E06：本轮自动与隔离 PostgreSQL 验证

环境：rustc 1.95.0、cargo 1.95.0、Node 24.13.0、npm 11.6.2。测试数据库使用项目已固定的 PostgreSQL 16.14 镜像及 digest。没有向共享数据库设置测试 URL。

| 检查 | 结果 | 实际覆盖 |
|---|---|---|
| `cargo test --workspace --locked` | 197 passed / 0 failed / 118 ignored | 常规测试；单独统计被忽略的 PG 用例 |
| `scripts/test-local-001-discovery-postgres.sh` | 83 项 PG passed，另有 2 项 JS passed | discovery 9、local producer 6、reobservation 3、material projection 11、social 7、media 8、creator lifecycle 5、dossier 9、Topic 6、API 19 |
| Collection 隔离 proof | 30 项 passed | control 10、control runtime 11、dispatch sequence 9 |
| `scripts/test-scope-001-postgres.sh` | 5 项 passed | 原子 ingress、回滚、幂等与跨父对象限制；不证明旧 synthetic Observation/Current 完成 |
| `npm run test:linggan` | 252 passed / 0 failed / 0 skipped | 当前 package.json 列出的现役插件测试集 |
| `npm run check:contracts` | passed | 插件 TypeScript 合同检查 |
| `npm run build` | passed；3 个 Webpack performance warning | content entry 833 KiB，content.js 648 KiB |
| `verify:content-runtime` | passed，101 个活动模块 | 当前 content graph 的连接关系 |
| `verify:linggan-isolation` | passed | 活动 Service Worker 拥有回传，活动模块没有旧 Workbench host/transport |
| `release:verify` / `release:reproducibility` | passed | 0.8.37 ZIP 与固定源可复现；SHA 见 E04 |

83 + 30 + 5 = 118，覆盖本轮 workspace 默认跳过的全部 PG 用例。没有将同一个测试的两次调用累计为不同场景。

Collection proof 的启动与清理由 [test-collection-dispatch-sequence-postgres.sh](../../scripts/test-collection-dispatch-sequence-postgres.sh) 的临时副本承担；只把 project_root 固定到审计目录，并在同一隔离数据库中增加现有两个测试 target：

```bash
cargo test -p linggan-evidence \
  --test collection_dispatch_sequence_postgres \
  --test collection_control_postgres \
  --test collection_control_runtime_postgres \
  --locked -- --ignored --test-threads=1
```

三个 proof runner 都报告容器/数据库/volume 清理成功。结束后 Docker 中仅剩原有正式 PostgreSQL 容器，没有本轮 proof 名称的容器或数据卷。未测试意外断电后的清理能力。

临时日志保存在 `/tmp/linggan-system-audit-20260905.5tgSnF/`，可被操作系统清理；上述结果已转写为本附录。主要日志为 `cargo-test.log`、`postgres-proof.log`、`control-proof.log`、`scope-proof.log`、`plugin-test.log`、`reproducibility.log`，没有将原始运行材料提交入库。

## E07：媒体实物与处理状态

读取 Blob 的 storage_key、sha256、byte_size 后，在受控媒体根目录中逐项解析路径、核对文件大小和 SHA-256：

```text
60 total / 60 exist / 60 hash_and_size_match
0 missing / 0 mismatch / 0 outside_root
total bytes = 4,230,368
```

一次从真实 Work Resource 返回值取得的本地媒体句柄 GET：HTTP 200，Content-Type image/webp，32,742 字节。没有下载远程媒体。

媒体取得 work：completed 106、terminal 32。DownloadAttempt：acquired 106、network_error 11。ProcessingWork：image_ocr completed 57、thumbnail completed 58、video_frame_ocr completed 1、audio_extract terminal 1、asr terminal 1。Derivative：thumbnail 58、ocr_text 57。非空派生文本 57，长度 5–2,160 字符，平均约 175；未人工抽样评价准确率。

ASR 历史错误包含模型缓存 SHA 检查警告，音频提取记录 ffmpeg 失败；本轮未重新执行处理器，不能单凭截断错误确定今天的根因或修复有效性。

代码：[material_asset_read.rs](../../crates/evidence/src/material_asset_read.rs)（处置资格）、[local_asset_delivery.rs](../../apps/api/src/local_web/local_asset_delivery.rs)（根目录约束与交付）、[media_worker.rs](../../apps/worker/src/bin/media_worker.rs)、[media_worker_support](../../apps/worker/src/bin/media_worker_support/mod.rs)（子进程超时）、[material_evidence_fragment.rs](../../crates/evidence/src/material_evidence_fragment.rs) 第 236 行（默认片段排除封面 OCR）。

## E08：领域与当前架构证据

当前权威输入：[共同语言](../context/domain-language.md)、[领域不变量](../product/domain-invariants.md)、[ADR-0001](../decisions/0001-greenfield-rust-clean-db.md)、[技术架构基线](../architecture/technical-architecture-baseline.md)。模块架构文档自身仍标“草案”，其目标图不作为当前运行事实。

代码核对：[Cargo.toml](../../Cargo.toml)、[API Cargo.toml](../../apps/api/Cargo.toml)、[worker Cargo.toml](../../apps/worker/Cargo.toml)、[evidence 导出](../../crates/evidence/src/lib.rs)、[intelligence 导出](../../crates/intelligence/src/lib.rs)、[domain](../../crates/domain/src/lib.rs)、[observation](../../crates/observation/src/lib.rs)、[storage-postgres](../../crates/storage-postgres/src/lib.rs)。

业务实际使用的 Current 位于 [work_resource_current.rs](../../crates/evidence/src/work_resource_current.rs)、[material_query_sql.rs](../../crates/evidence/src/material_query_sql.rs)、[material_detail_read.rs](../../crates/evidence/src/material_detail_read.rs)。单作品读取使用 repeatable-read/read-only 事务；列表和跨页面通过 [work_resource_read.rs](../../crates/evidence/src/work_resource_read.rs) 复用。字段的“最新已知值”和最新完整对象记录分开选择，不把某一轮未知抹成 0。

## E09：Topic 已实现与未使用的边界

来源：[0031 migration](../../database/migrations/0031_topic_workspace.sql)、[Topic 合同](../architecture/topic-workspace-contract.md)、[Topic 实现](../../crates/intelligence/src/topic_workspace.rs)、[Topic API/页面](../../apps/api/src/local_web/topic_workspace.rs)。

- `import_topic_workspace` 第 166 行：validate、hash、transaction、scope lock、replay、资源存在性、expected version、insert、commit。
- `read_topic_workspace` 第 199 行：读取最新定义及对应 Run/Pack；成员引用不复制原文。
- Topic route 第 32 行：`/topics` 固定跳转 `/topics/task-initiation-difficulty`。
- 实际 GET `/api/local/topic-workspaces/task-initiation-difficulty`：404，`operation=topic_workspace / outcome=not_found / code=topic_workspace_not_found`。
- 页面显示“当前未读取 Topic”和来源/材料未知；没有用合成材料填充，没有可见创建或导入研究的操作入口。

六张表全空是本次真实数据事实，不能以 API/PG 测试通过改写为“Topic 研究已经在使用”。

## E10：Agent 与后续情报能力

主线 intelligence crate 只导出 Topic Workspace；没有 Agent runtime 模块或 migration。一级导航中雷达/洞察均为 disabled，来自 [shell.rs](../../apps/api/src/local_web/shell.rs) 第 26 行。

[Draft PR #116](https://github.com/Himog0921/linggan-intelligence/pull/116)：head `a01e78c4d47bca0cb473f5ba733f1aa65fb0a39f`，base `codex/topic-workspace-real-001`，15 个变更文件、1 个提交。只读检查了该分支的 `agent_candidate.rs`、`agent_runtime.rs`、测试文件名与 migration 位置。

可确认其存在 `ModelToolLoopPort`、`DeterministicCandidateAdapter`、`PiLoopTransport`、`PiCandidateAdapter`、候选校验和持久状态。分支固定 Pi package version 0.84.2，表示当时合同，不是本轮外部版本选型结论。没有运行该分支、安装 provider 或调用模型。

未来范围依据 [agent-architecture.md](../architecture/agent-architecture.md) 和 [page-map.md](../pages/page-map.md)。本轮不推翻尚未实现的已确认领域规则，也不将候选设计记作产品能力。

## E11：七个真实页面的只读检查

使用 CUA 的本机浏览器，1440 × 1000 桌面窗口。仅导航、读取和截图观察；没有执行采集/重试/启停/规则保存。结束后恢复 viewport 并关闭审计标签页。

| 页面 | 实际观察 | 未测试的部分 |
|---|---|---|
| `/collection/targets` | creator 与 keyword 分表；4 个目标；两份已有目录 31/13 篇，详情 0/31 与 0/13，均暂停 | 建档按钮的真实后果、移动端 |
| `/collection/runtime` | 当前三种控制资格均 installation_stale；一个有效工位；无活租约；未开启巡检 | 插件重新上线、账号切换/轮换 |
| `/collection/attention` | “需要处理 103”“你负责 100”；100 条同目标 rule_missing + 3 个当前工位资格阻断 | 恢复动作实际执行 |
| `/collection/operations` | 最新 tick idle、考虑目标 0；历史决定区显示最近 100 条 rule_missing | 新调度轮从入队到完成 |
| `/collection/tasks` | 最近 100 条有界读取；Package/Receipt 分列；失效租约明确无执行权 | 全历史分页、实时任务动作 |
| `/corpus/evidence` | 46 行真实材料，本地封面/头像，Inspector；无页面级横向溢出；独立评论研究、创作者与已存查询入口未接通 | 全部筛选组合、每个媒体打开、完整无障碍/移动端 |
| `/topics/...` | not_found 空状态、暂定定义和材料边界 | 真实材料包、写入、实际研究验收 |

图片有懒加载，首屏加载数量不能用作 92 个 img 全部成功或失败的结论。本轮未保留包含个人材料的页面截图到 Git。

**F01 根因证据：** [collection_control_surface_view.rs](../../apps/api/src/local_web/collection_control_surface_view.rs) 第 182 行按最近时间读取 decision；第 722 行对每个有 recovery_for 映射的历史 decision 直接生成 AttentionEntry；第 713 行对 entries 计数作为“当前需要人工处理”。没有按目标合并，也没有用目标当前生命周期排除历史阻断。

交叉 SQL 显示最近 100 条 decision 全属于同一个 `paused / monitoring_enabled=false / active rule=null` 的目标。历史缺规则记录是真的；把它们当成 100 件当前工作是本轮确认的问题。

## E12：工程门禁和代码集中度

`scripts/check-project-governance.sh <fixed-base>` 在报告写入前通过。它检查结构和索引，不验证声明是否真实。

`scripts/check-rust-boundaries.sh`：exit 1，35 errors / 13 warnings。典型生产文件：

| 文件 | 行数 |
|---|---:|
| `apps/api/src/local_web.rs` | 3,747 |
| `crates/evidence/src/collection_control.rs` | 2,391 |
| `crates/evidence/src/acquisition_chain.rs` | 1,792 |
| `apps/api/src/local_web/collection_control_surface_view.rs` | 1,686 |
| `apps/api/src/local_web/target_drawer.rs` | 1,466 |
| `crates/evidence/src/producer_runtime.rs` | 1,138 |
| `crates/evidence/src/work_order_lease.rs` | 933 |
| `crates/evidence/src/patrol_scheduler.rs` | 924 |

脚本按路径区分测试，导致 `src/tests.rs` 和 `src/creator_lifecycle_tests.rs` 被当成生产文件；其 SQL 检查也包含测试 SQL。不能把全部报错机械视为生产违规。实际业务 SQL 位于 API 的 control surface 等文件，仍需按责任审查。

`cargo clippy --workspace --all-targets --locked -- -D warnings`：exit 101，首先在 [producer_runtime.rs](../../crates/contracts/src/producer_runtime.rs) 第 408 行 `validate_comment_collection_receipt` 报 `too_many_lines (103/100)`。构建在该处停止，不能声称它是全仓唯一 Clippy 问题。本轮没有修复或降低 lint。

## E13：文档与 GitHub 工作台账

GitHub 只读查询返回 14 个 open Issue：#10、#66、#86、#89、#90、#103、#110、#112、#113、#130、#148、#149、#150、#158。

| 开放 Draft PR | base | 审计处置 |
|---|---|---|
| #116 | Topic 历史分支 | 主线无 Agent，对其独有成果单独标识 |
| #91 | material projection 分支 | 不以 open/draft 状态否定主线已有 Evidence 页面 |
| #88、#87 | media reconciliation 分支 | 与主线当前材料/设计能力逐项核对后才能处理 |
| #84 | main | 历史媒体合同候选，未自动关闭 |
| #49 | main | 历史搜索发现切片，未自动关闭 |
| #8 | main | 历史 synthetic 处理模型，未作为当前产品缺失的自动施工清单 |

可复核链接：[Issues](https://github.com/Himog0921/linggan-intelligence/issues)、[PRs](https://github.com/Himog0921/linggan-intelligence/pulls)。

确定漂移：`current-state.md` 顶部仍写 #158 唯一实施中且没有 merge/runtime 授权；实际 [PR #159](https://github.com/Himog0921/linggan-intelligence/pull/159) 在 `2026-09-05T07:05:04Z` 已合并，主线还包含 #160/#161，运行目录与主线一致。#158 Issue 仍 open；这本身不证明应关闭，因为业务验收与剩余范围需要分别核对。

同一 current-state 的“0.8.28 当前权威快照”及“main 之上只有文档提交”也已不符合本次代码事实。`development-stage-tracker.md` 仍混有早期“未部署”的示例与旧 synthetic 阶段描述。报告将其作为历史/设计来源，不继续引用为当前状态；本次没有整篇重写权威文档。

主线没有 `.github/workflows` 目录；GitHub Actions API 的 `total_count=0`。这只能证明本仓库没有可见 Actions 运行记录，不能推断所有外部 CI 都不存在。本轮验证是本地执行。

## E14：读取性能与规模边界

`GET /api/local/work-resources` 返回 46 项，`scanLimited=false`、`truncated=false`、`scannedCount=46`。初次读取约 645ms；随后同 URL 的三次串行只读请求为 630.3ms、607.4ms、529.0ms。工具环境存在其他进程和缓存影响；该数据是小库现象，不是 p95、SLA 或容量基准。

实现：[material_projection.rs](../../crates/evidence/src/material_projection.rs) 第 115–127 行逐行 enrich discovery/social/media；[material_query_sql.rs](../../crates/evidence/src/material_query_sql.rs) 第 244 行起为标题、正文、评论、派生文本、作者等子串匹配；[Topic 页面组合](../../apps/api/src/local_web/topic_workspace.rs) 也逐成员读取 Work Resource。

因此确认存在随返回作品数增加的往返结构，尚未量化每次请求的 SQL 数量或证明它是所有延迟的根因。[Issue #89](https://github.com/Himog0921/linggan-intelligence/issues/89) 可作为后续核对入口。没有大数据集 EXPLAIN ANALYZE、并发压测或向量检索选型结论。

## E15：部署、访问和恢复边界

部署事实来源：[local-runtime-deployment.md](../runbooks/local-runtime-deployment.md)、[sync.sh](../../scripts/runtime/sync.sh)、[launch.sh](../../scripts/runtime/launch.sh)。runtime-main 是已确认的固定位置，本轮没有建议恢复多份历史快照模式。

已观察的脚本行为：sync 第 43 行 fetch/reset，读取 migration 名称台账；台账不可读时第 79 行记录并跳过检查；第 85 行 `cargo build` 没有 `--locked`。三个启动流程共享同步锁，但只重启一个服务不会自动替换其他存活进程。这是代码推导出的更新一致性风险，本轮没有触发重启或观察到当前 split revision。

访问方面：API 只绑定 loopback；插件执行有安装凭据、账号绑定、资格时效和服务端准入；本地媒体有路径、哈希、大小与处置资格保护。普通评论列表不暴露评论正文和外部身份的 PG 测试通过。

研究评论通道位于 [material_projection.rs](../../apps/api/src/local_web/material_projection.rs) 第 79 行和 [material_social_read.rs](../../crates/evidence/src/material_social_read.rs) 第 501 行。该路径按本机研究入口返回有界正文，读取函数没有调用者/用途委托参数。本轮只读源码，没有调用它导出真实评论。现有本机信任假设不能直接扩展为多用户或外部 Agent 权限模型。

未执行备份恢复、角色权限穿透测试、网络渗透、来源撤回跨全文索引/模型输出传播或长时间负载验证。媒体处置测试通过不能提升为完整隐私治理完成。

## E16：向外看的产品边界与报告修订

来源为 [Issue #130：CORPUS-CROSS-INDUSTRY-001](https://github.com/Himog0921/linggan-intelligence/issues/130) 及其 [v1.2 完整设计评论](https://github.com/Himog0921/linggan-intelligence/issues/130#issuecomment-5489282219)。2026-09-05 本轮补充只读核对时 Issue 为 open，最后更新 `2026-09-01T05:19:46Z`；远端 main 仍为 `4864edd`。本轮未发现 Issue timeline 中关联的实现 PR；固定主线没有指定的 `docs/design/cross-industry-corpus.md`，在应用、crate、插件源码中未检出 `cross_industry_sample` / `observation_domain` 模块实现。不能将评论中的确认设计当成已交付功能。

确认的产品意图是观察外部内容的标题、封面、结构和文案逻辑，改善内容创作中的打开、阅读和选题问题。初始外部领域为考研自习、自闭症干预；外部内容为表达参照，不作为 ADHD 事实判断的 Evidence。UI 复用并不允许混读：本领域调用既有 Evidence 只读接口，跨行业调用新接口，样本归属由 CHECK 和复合外键约束。

首版冻结范围包含领域配置、列表/详情两级采集、常规扫描与新鲜捕捉、实际采样口径、实际观测时刻、同形卡片和散点图、随手记及搜索/筛选/导出。实际取得不足 20 条不发“爆”徽章；有效观测点不足 3 个时不分类增长形状。#103 合并并部署是开工前置条件；本轮未替此卡进行依赖验收。

首版明确排除跨领域问题对齐、自动手法归纳、Topic、市场洞察、每日关注和选题行动；累计 50 条随手记时才启动下一版归纳设计。第一阶段本机小团队阅读不做脱敏的既有决定，不包含外部 Agent 原文读取或公开引用。

设计评论还包含判断性或历史表述，报告没有自动采纳为经验事实：设计时“本领域 12 条”不是本轮作品数；“补详情后发布时间必有”不能覆盖当前字段资格规则；点赞/收藏榜单不直接证明情绪/实用价值；固定条数和观测点门槛不是因果或统计有效性的证明。未来智能化应保留这些区别，而不是把文档中的解释当成来源已经证明的结论。

主报告 §3 补回两条观察线，§7 撤回先补齐底座再串行进入全部智能化的表述，增加首轮研究判断的讨论建议。这些建议没有修改 #130、Agent A1–A2、Domain 语言或任何运行合同，也未执行模型调用或新增采集。

## 证据使用规则

重新使用本报告时，先刷新 E01/E02/E03/E04 的版本、运行与时间；不要把本轮计数当作持续不变的库存。复核某项问题时可从对应文件和测试进入，不需要重放全部平台采集。已确认的报告缺陷只进入后续任务建议，本轮没有自动修复、关闭 Issue、创建 PR 或发布任何产品变更。
