# 生成型文件登记表

> 状态: 权威当前
> 最后核对: 2026-10-16
> 适用范围: 构建、代码生成、导出、日志、测试证据、备份与临时文件
> 事实来源: 当前工具配置、`.gitignore` 与来源校验清单
> 冲突时以谁为准: 生成源、工具配置和安全规则；生成结果不得反向覆盖来源

任何未列入本表的生成型文件，默认不得写入项目目录或提交 Git。确需新增时，先登记再生成。

| 类别 | 固定位置 | 来源/生成方式 | Git 策略 | 手工修改 | 保留与清理 |
|---|---|---|---|---|---|
| Pi Node 固定依赖 | `apps/pi-adapter/node_modules/`（含 `.linggan-lock-sha256`）、`package-lock.json` | 精确 package.json、npm ci、`prepare-pi-adapter.sh` | node_modules 忽略；lock 提交 | 禁止手工改 lock/安装产物 | 只清理当前 checkout 依赖，不处理其它项目 |
| 网页前端固定依赖（WEB-REACT-001） | `apps/web/node_modules/`（含 `.linggan-lock-sha256`）、`apps/web/package-lock.json` | 精确版本的 `apps/web/package.json`、`npm ci --ignore-scripts`、`scripts/runtime/prepare-web.sh` | node_modules 忽略；lock 提交 | 禁止手工改 lock/安装产物 | 只清理当前 checkout 依赖；可随时 `npm ci` 重建 |
| 网页前端构建产物（WEB-REACT-001） | `apps/web/dist/`（`index.html`、带内容指纹的 JS/CSS、`build-info.json`） | `npm --prefix apps/web run build`；本机运行链由 `prepare-web.sh --build <revision>` 生成，revision 写入 `build-info.json` | 忽略，不提交 | 禁止手工改 | 每个 checkout 一份，按 revision 覆盖重建；可安全删除后重建 |
| WEB-REACT-001 隔离浏览器证明 | 随机 `linggan-web-app-proof-*` Docker container/volume；证据目录 `${TMPDIR}/linggan-web-app-proof-*`（或 `WEB_APP_PROOF_DIR`；阶段 D1 的创作者页证明放在其 `creators/` 子目录：`fixture.json`、`report.json`、截图；阶段 D2 的证据库证明放在 `evidence/` 子目录；该库内随机 schema `creators_*`、`corpus_shell_*` 随容器一并删除）；随机 `linggan-web-app-support.*` 临时目录（D2 另在其下 `media/` 写合成媒体字节）；D2 插件入口证明的 `linggan-entry-extension-*`、`linggan-entry-profile-*` 临时目录 | `scripts/test-web-app-postgres.sh`：合成模型凭据、隔离 schema、本机 Axum 路由与 Chromium | 不进 Git；截图只含合成数据 | 禁止伪造 | 容器、数据卷与临时目录在脚本退出时删除；证据目录保留供审阅，可手工删除；CI 作为 `web-app-real-chain-proof` 产物保留 7 天 |
| COMMENT-RESEARCH-RESET-001 V1 隔离证明 | 随机 `linggan-comment-proof-*` Docker container/volume；系统临时目录下的 test log | `scripts/test-comment-research-postgres.sh`、`apps/pi-adapter` 的 Node fixture；历史 migration 后显式应用 0068–0070 | 不进入 Git | 禁止伪造 | 脚本 trap 删除它拥有的 container/volume；日志只含合成资料/失败码，不含评论正文或凭据 |
| 模型后端秘密及合成文件验证 | `~/Library/Application Support/Linggan Intelligence/model-secrets/<workspace UUID>/<secret UUID>`；测试仅在随机系统临时目录 | API 的唯一 LocalFileModelSecrets；旧 Keychain 条目仅作一次性迁移来源 | 目录 0700、文件 0600；不进 Git/数据库/前端存储 | 只经配置命令更换 | 正式凭据随引用版本保留；迁移校验后清理旧 Keychain 条目；测试结束删除自己创建的合成目录 |
| CI-20260907-V1 历史隔离证明 | 已清理的临时 Docker/log 位置 | 历史交付记录；对应旧评论研究源码与再生脚本已由 V1 terminal cutover 删除 | 不进 Git | 禁止伪造或重新生成旧结果 | 仅保留文档中的历史事实；不再是可运行验证入口 |
| CI-RUN-002 请求诊断 | PostgreSQL `linggan_comment_request_trace` | 已授权研究的真实请求边界写入；输入已脱敏，返回保结构脱敏 | 不进入 Git | 禁止伪造或回填历史 | 正文最多保留 24 小时；读取独立校验有效期与来源；worker 清理过期或受限正文，元数据与安全校验摘要继续保留；关闭记录仅影响新批次 |
| 产品页面与首页 Three.js 合成原型 | `docs/design/pages/intelligence-product-prototype.html` | `python3 docs/design/pages/intelligence-product-prototype/build.py`；来源为同目录自有 JS/CSS、`docs/pages/intelligence-product-blueprint.md`、`docs/pages/intelligence-home-threejs.md`；Three.js 0.185.1 按固定 npm integrity 校验并保留 MIT 许可 | 可提交的设计参考，非 app/release；依赖 tarball 与截图只放系统临时目录 | 修改自有源码再生成；不直接修改生成 HTML | 稳定保留；设计更新时重新生成并走查；始终标记合成、无真实模型/采集 |
| COMMENT-STUDY-READ-PERFORMANCE-358 证明 | 系统临时目录 `comment-study-read-performance-*`；随机专属 Docker proof container/volume | 只读 SQL/HTTP 汇总测量、隔离合成 PostgreSQL 和浏览器回归 | 不进 Git；仅汇总，无真实正文/凭据 | 禁止伪造 | 仅清理本任务创建的 proof 资源；源码、治理文档与候选 PR 保留 |
| Rust 构建缓存 | `target/` | Cargo build/test | 忽略 | 禁止 | 可安全重建，按需清理 |
| CI-AUTO-004 worker退出回执 | Application Support/Linggan Intelligence/runtime-drain/worker-drain-ack、worker-update-permit；开发启动为系统临时目录 `linggan-runtime-drain.*` | worker写PID与终态；install生成绑定起止revision的许可，sync核验消费 | 不进Git；无正文/凭据 | 不得伪造完成回执 | 下一次受控drain替换；开发启动成功退出后仅清理自己目录，失败保留供诊断 |
| Rust 依赖锁 | `Cargo.lock` | `cargo generate-lockfile` | 提交 | 禁止 | Cargo 配置变化后重新生成并验证 `--locked` |
| 本地环境秘密 | `.env`、`.env.*` | 人工从安全凭据源配置 | 忽略；仅 `env.example` 可提交 | 允许本地配置 | 不进入变更记录正文，不复制到仓库 |
| PostgreSQL Docker 镜像缓存 | Docker Desktop 管理空间 | `docker compose pull`，来源由 `compose.yaml` 的 tag + digest 固定 | 不进入 Git | 禁止 | 可重新拉取，清理不等于删除数据卷 |
| PostgreSQL 开发数据卷 | `linggan-intelligence-postgres-16-data` | `docker compose up` | 位于 Docker，不进入 Git | 禁止手工改文件 | 默认保留；删除必须单独批准 |
| 私有运行证据 | `artifacts/private/` | 本地验证或外部系统导出 | 忽略 | 禁止伪造 | 按事项安全保留，禁止提交 |
| 数据库备份 | `database/backups/` | PostgreSQL 备份工具 | 忽略 | 禁止 | 加密、独立保管，按批准策略清理 |
| 数据库恢复工作区 | `database/restores/` | PostgreSQL 恢复工具 | 忽略 | 禁止 | 仅本地临时使用，不连接新项目运行时 |
| 历史来源校验表 | `references/SOURCE-MANIFEST.sha256` | Bootstrap 来源固定流程 | 提交并受保护 | 禁止随意修改 | 与历史快照一起长期保留 |
| 架构、业务流程与项目全景图 | `docs/architecture/system-overview-diagram.html`、`docs/architecture/business-process-diagram.html`、`docs/architecture/project-architecture-atlas.html` | `diagram-design` Skill 根据当前代码、migration、测试、`/health`/本地页面运行快照、`docs/current-state.md`、最新 progress、技术基线及目标/模块/采集/数据/Agent/运行/界面架构生成；用 Skill `self_check.py`、几何检查和浏览器渲染复核 | 提交 | 仅在同步复核来源、状态说明和图内边界后允许修改 | 稳定保留；架构状态、技术基线或获准范围变化时重新生成并复核；图不替代长期运行、真实采集或生产证明 |
| Linggan Browser Producer release | `plugins/linggan-browser-producer/releases/linggan-browser-producer-<version>.zip`、`plugins/linggan-browser-producer/releases/release-manifest.json` | `cd plugins/linggan-browser-producer && npm run build`；唯一源为该包的 `src/` 和 Linggan runtime token source `apps/api/src/local_web/lids_tokens.css`，build script 同时生成 ZIP/manifest | 提交 | 禁止手工修改 ZIP 或 release manifest；修改源后重新 build | 当前只保留当前发行版；替代/删除必须经单独发布卡和 hash 核对 |
| Linggan Intelligence Browser release | `plugins/linggan-intelligence-browser/releases/linggan-intelligence-browser-v<version>.zip`、`plugins/linggan-intelligence-browser/releases/release-manifest.json` | `cd plugins/linggan-intelligence-browser && npm run build && npm run package:release && npm run release:manifest && npm run release:verify && npm run release:reproducibility`；唯一源是该包的当前 source；打包器固定 ZIP 时间与文件顺序 | 提交 | 禁止手工修改 ZIP 或 release manifest；修改 source 后重新 build/package 并核验 | `release-manifest.json` 指向唯一当前候选发行包；历史 ZIP 作为已发布工件保留，不覆盖或删除。`0.8.61` 候选 ZIP SHA-256 = `2aba8db3d2edad87b6b4c5436e751efa5f89b322763c789828b65295a9def2f4`；不含真实材料、账号、凭据或运行日志，未加载到浏览器 |

## COMMENT-STUDY-PRODUCTIZATION-001 · 2026-09-23 局部登记

| 类别 | 固定位置 | 来源/生成方式 | Git 策略 | 手工修改 | 保留与清理 |
|---|---|---|---|---|---|
| 评论研究手册机器合同 | `docs/data-contracts/comment-study-productization-001/*.json` | Mog批准的手册v1.0 `validation/`；仅规范化JSON空白；与数据库/HTTP合同逐字段核对 | 提交；不是运行时代码生成结果 | 变更必须有对应合同版本决定，不独立扩展字段 | 与手册版本长期保留 |
| 手册来源及静态检查 | `docs/audits/comment-study-productization-001/*.json` | 原交付回执；`python3 scripts/verify-comment-study-productization-docs.py` 检查源hash/链接/Schema；实际执行再记录新回执 | 允许无正文/凭据的回执；不提交数据库dump或原始模型内容 | 不伪造PASS；历史回执不覆盖，业务T编号由实际证明更新 | 与exact head关联保留 |
| 已确认评论研究合成原型 | `docs/design/pages/comment-study-productization-prototype-v1.html`（待导入）；当前来源元数据在上述audit目录 | 用户确认的 `linggan-comment-research-product-prototype-v1.html`；原文件SHA-256固定，不重新生成替代原件 | 仅合成设计参考；不得进入运行assets；只有原字节校验通过后才算正文入库 | 冲突时以完整手册和LIDS为准，不从模拟数据推导真实能力 | 来源元数据不代表原型正文已上传；入库状态单独记录 |

## 当前登记结论

- 当前获准提交 Git 的产品生成物包括上表登记的架构沟通 HTML、产品设计合成 HTML 与 Browser Producer release。`0.8.28` 的运行时/真实标准详情证明来自工位、Package/Receipt/Materialization 与 Evidence UI，不是 ZIP 本身自证。
- `references/` 是已导入并校验的历史证据快照，不是产品运行时生成目录。
- SCOPE-001 的 `crates/contracts/tests/fixtures/capture-v1/*` 及其 expected canonical bytes/hash 是人工审查和维护的权威测试输入，不是由 Rust 或 Node checker 生成的产物；`scripts/verify-capture-fixtures.mjs` 只验证、不回写。任何将来的自动生成/回写都必须先增加本表正式登记，不得沿用这一人工输入豁免。
- 未来若引入 SQLx 离线元数据、API schema、前端构建包或机器生成 fixture，必须先补充：负责人、唯一来源、再生命令、一致性检查和是否必须入 Git。
- 临时实验输出使用操作系统临时目录；若结果需要成为项目证据，应转写为 `docs/audits/` 的可读报告，敏感原件仍留在忽略目录或外部安全存储。
- `PLUGIN-MIGRATION-001` 的 Browser Producer release 只包含 extension code、manifest 和由 Linggan runtime token source 打包的视觉 token；不得包含 Cookie、账号、真实页面材料、媒体字节、旧内容工作台运行依赖或运行日志。该发行物可被浏览器加载，不等于已经获得平台访问或实际采集授权。
- `plugins/linggan-intelligence-browser/` 是当前 Linggan-owned 唯一可发布源。发行物不得包含 Cookie、账号、真实页面材料、媒体字节、旧工作台 host/endpoint/fallback 或运行日志；旧 `plugin-retrofit-*` 与历史副本只可只读对照，不得生成当前 release。

## COMMENT-LIKE-LIBRARY-001 / PR #401 验证与本机发布

- 维护的测试源：`scripts/test-comment-like-library-ui.cjs` 与 `crates/intelligence/tests/comment_like_library_postgres.rs`；不是生成代码。合成测试日志、临时 HTTP/浏览器脚本放系统临时目录 `pr401-*`，不入 Git。PG 容器和卷由现有随机名脚本创建并在退出时清理核验。
- 经 Mog 本轮明确授权的共享库迁移前备份放根仓库既有忽略目录 `database/backups/comment-like-library-401-<UTC>/`；只保留加密 dump、校验和与不含正文/凭据的核验回执。密钥在独立用户私有位置，0600，不输出、不入 Git；不生成明文 dump。
- 真实 3000 浏览器截图放本任务已授权 Codex visualizations，命名 `pr401-runtime-comments.png`。截图只证明页面读路径，改帖写入的合成反例与真实业务签收分开报告。

## CREATOR-DISCOVERY-001 隔离验证产物

- 位置：系统临时目录中的 `creator-discovery-*.log`、`creator-discovery-http-check.py`、GitHub body Markdown、隔离 API ledger SQL 与 worker 屏障临时目录（用例自行清理）、本任务 `linggan-creator-discovery-proof-*` 一次性 Docker container（无宿主业务 volume），以及 Cargo 既有忽略目录。
- 来源：`creator_discovery_postgres`、`creator_discovery_worker_postgres` 与专用本地 API 进程；输入均为合成数据。测试源码是维护的测试输入，不是生成产物。
- Git：不提交日志、数据库、截图或构建产物；可复现命令与结果转写至 `docs/audits/creator-discovery-001-acceptance.md`。
- 清理：确认测试和本任务 API 进程结束后停止本任务容器，`--rm` 删除其临时数据；不得清理其他容器、业务库、工作树或共享 runtime。

## CORPUS-PERFORMANCE-089 隔离验证产物

- 来源：`scripts/test-corpus-performance-postgres.sh` 与合成 PostgreSQL fixtures；独立容器启用 pg_stat_statements，仅统计测试库查询次数。
- 位置：`linggan-intelligence-corpus-performance-proof-*` 容器及同名 `-data` volume；系统临时目录中的只读 SQL、EXPLAIN 与 HTTP 比较结果。
- Git：只提交源码及聚合指标，不提交原文、DSN、数据库或运行日志。
- 清理：脚本 EXIT 删除并核实本次 proof database/container/volume；比较完成删除真实 HTTP 临时响应并停止本次 3108 候选 API，不触碰共享 3000 或其他任务资源。

## TOPIC-MAP-V41-001 隔离验证

- 来源：主题图谱 Rust/API/JS/CSS 与 `scripts/test-topic-map-postgres.sh`。
- 固定位置：Cargo缓存在worktree已忽略target；测试输出与临时浏览器证据放 `/tmp/topic-map-v41-*`；供 Mog 审阅的合成截图保存到 Codex 当前任务授权的 visualizations 目录，文件名 `topic-map-demo-corrected-*.jpg`。Docker容器/volume使用上述脚本唯一随机名。
- 不入Git，不包含真实原文/运行库dump/凭据；脚本退出清理容器与volume并验证成功。
- 再生：上述脚本 + API focused tests + 隔离合成数据浏览器走查。责任 TOPIC-MAP-V41-001 root。
- 首屏后续候选：`/tmp/topic-map-v41-first-screen-api` 为当前worktree最终build的冻结二进制副本，防止共享Cargo缓存被其他构建覆盖；不入Git，不含运行凭据或正文。由本分支 `cargo build -p linggan-api --bin linggan-api --locked` 再生，记录sha256/PID/cwd后仅用于只读3001验证，进程停止后删除副本。
- 本机发布追加：经 Mog 单独授权的运行库加密备份、校验和与不含正文/凭据的发布回执保存在既有忽略目录 `database/backups/topic-map-v41-<UTC>/`；密钥继续独立保管。真实3000的浏览器截图归本任务 Codex visualizations，文件名 `topic-map-demo-corrected-runtime-*.jpg`，不入Git。合成截图与运行页截图不得互相替代。


## CREATOR-DISCOVERY-001 · PR #386 发布验证

- 来源：维护的 `scripts/test-creator-approved-ui.py`（2026-10-16 随 WEB-REACT-001 D1 删除：旧创作者页已被 React 页取代，其职责由 `scripts/test-web-app-postgres.sh` 的创作者页浏览器证明承接，历史证据不受影响）、官方Playwright Python v1.57.0源码/既有Node驱动、candidate/current runtime只读GET与受控installer。
- 位置：系统临时目录 `/tmp/creator-ui-386-*` 的证明日志/截图/代理脚本，`/tmp/creator-386-*` 测试依赖及编译日志；Cargo缓存复用 `/tmp/linggan-creator-ui-target`。审阅截图只放本任务已授权Codex visualizations。
- 不入Git，不含凭据或数据库dump；真实GET截图为UI运行证明，不是模型语义或写动作验收。测试源为人工维护代码，非生成运行资料。
- 再生：离线脚本 + 只读候选HTTP浏览器 + 合并后的canonical installer与实际PID/cwd/health/浏览器核验。临时代理拒绝全部非GET且不启动worker。
- 清理：测试结束关闭本任务浏览器/代理，发布后移除干净且已远端持久化的proof worktree；不清共享runtime、其它任务容器/依赖。责任Issue #367 / root当前授权发布。


## TOPIC-MAP-WORKFLOW-382 验证与预览

- 来源：专属分支前端、现有本机 GET 读模型、JS/Rust 回归与一次性 PostgreSQL 测试。
- 位置：`/tmp/topic-map-workflow-*` 临时脚本/日志、`/tmp/linggan-topic-map-workflow-target` 构建缓存；截图在当前任务 Codex visualizations 的 `topic-map-workflow/`，不入 Git。真实材料只读截图与合成保存证明分别标明。
- 3112 为只读预览，使用候选 JS/CSS 与现有 token/shell；拒绝全部 POST，不代理外网，不保存真实 API 响应或凭据。预览不是候选 Rust API 部署或正式库保存证明。
- 隔离 PG 使用既有 test-topic-map-postgres.sh 随机容器和卷，退出时删除核验。3112 仅保留到 Mog 页面审阅，后续关闭该任务的 Python 进程；不触碰共享 3000 服务。

## OPS-MIGRATION-001 D1 / Issue #402 验证与导入

- 维护的测试源：`scripts/test-ops-ledger-postgres.sh`、`crates/intelligence/tests/ops_ledger_import_postgres.rs` 与 `tests/support/ops_export_fixture.rs`（全部是合成数据）；不是生成代码。PG 容器和卷由脚本随机命名，退出时删除并核验。
- 真实导出：只在 Mog 本机「下载」目录原地读取，不复制、不入 Git、不进测试。演练用一次性容器（无宿主 volume），只输出不含账号名、标题、链接和运营人名的聚合 JSON，放执行会话的临时目录，不入 Git；容器结束即删除。
- 正式导入（须 Mog 另行授权）的输出同样只含聚合数，回执写入 `linggan_ops_import_batch`；导入前按既有做法备份共享库，备份不入 Git。


## OPS-MIGRATION-001 D2 / Issue #405 页面走查与封面迁入

- 维护的测试源：`crates/intelligence/tests/ops_ledger_read_postgres.rs`（合成数据）与 `apps/api/src/local_web/ops.test.mjs`，由 `scripts/test-ops-ledger-postgres.sh` 统一运行；不是生成代码。
- 页面走查：共享库的一次性完整副本放在随机命名的 Docker 容器里，执行 0124 后由候选服务在 `127.0.0.1:3100` 读取；走查结束删除容器。截图、页面计量结果、导出样本和走查脚本都放执行会话的临时目录，含真实账号名与运营人名，不入 Git，会话结束删除。
- 走查用的封面是程序生成的色块图，只写入副本和临时媒体目录。
- 正式封面迁入（2026-10-13 已执行，批次 `dc5ce2e0`）：`linggan-ops-history-import cover-preview|cover-apply`，文件按内容哈希放在运行时媒体目录的 `ops-covers/`，命令只输出聚合数。原件留在 Mog 的工作台硬盘上。导入前的本机暂存目录 `~/Library/Application Support/Linggan Intelligence/ops-cover-import-20261013/` 带逐文件哈希清单，核验后已删除。以上都不入 Git。

## OPS-MIGRATION-001 D3 / Issue #409 总览走查与规则反证

- 维护的测试源：`crates/intelligence/tests/ops_overview_postgres.rs` 及其 `ops_overview_postgres/` 子文件（合成导出）与 `apps/api/src/local_web/ops_overview.test.mjs`，由 `scripts/test-ops-ledger-postgres.sh` 统一运行；不是生成代码。
- 页面走查：沿用 D2 的做法，共享库的一次性完整副本放在随机命名的 Docker 容器里，执行 D3 的目标迁移（现编号 0126）后由候选服务在 `127.0.0.1:3100` 读取；走查中在副本上设过的周、月目标只在副本里。截图、计量结果和走查脚本放执行会话的临时目录，含真实账号名与运营人名，不入 Git；走查结束删除容器，会话结束删除临时目录。
- 规则反证：临时改坏 6 处规则（同样天数、未知不当 0、只算已迁入日子、防覆盖、重复请求、按周期加锁），逐一确认证明变红后按备份逐字节还原并核对哈希；用的隔离 PG 容器和卷随机命名，结束即删除。脚本与日志在临时目录，不入 Git。

## TOPIC-MAP-COMPACT-382 隔离 UI 验证

- 权威测试源：`scripts/test-topic-map-ui.cjs`、`scripts/test-topic-map-compact-ui.py`，维护源码，不是生成文件。只运行合成 HTTP fixture 与候选 JS/CSS。
- 源码传递包、补丁、截图与日志：本任务系统临时目录和 Actions artifact（源码包 1 日、验证结果 7 日）；不入 Git、不含真实原文/凭据、不访问模型/平台/正式库。
- `.github/workflows/topic-map-compact-transfer.yml` 仅用于专属候选分支的版本校验与传递，交付前删除；持续回归另用只读验证 workflow。临时传递仅影响此分支，不强推、不合并或部署。

## COLLECTION-JTBD-001 / #398 隔离验证

- 来源：`scripts/test-collection-single-run-postgres.sh`、正式 API router 与合成 PostgreSQL、`scripts/test-collection-single-run-ui.cjs`；正式插件 fixture。
- 位置：`/tmp/collection-398-*` 日志、浏览器依赖与合成截图；本任务随机 `linggan-collection-single-run-proof-*` 容器和同名 `-data` 卷；Cargo/npm 现有忽略缓存。审阅截图复制到本任务授权的 Codex visualizations 目录，不入 Git。
- 无真实材料、账号、DSN、Cookie 或凭据；交接包六张用户截图只读，不复制进仓库或 CI。
- 再生：上述脚本加 `COLLECTION_RUN_BROWSER_PROOF=1` 与明确临时证据目录；发布候选按正式插件现有脚本生成，不加载插件。
- 清理：脚本退出核验删除本次随机容器/卷；浏览器上下文及测试服务器退出关闭，不清理其他任务或业务资源。责任 #398 当前执行者。

## LOGIN-ANATOMY-001

- 固定生成位置：`apps/api/src/local_web/login.html`，允许提交为 Rust include_str 页面资产；禁止手工修改。
- 唯一源：`scripts/login-anatomy/{page.html,model.mjs,parts.mjs,order.mjs,station.css,live.mjs,build.mjs}`；Anatomy 技能的 MIT kit 仅在生成时使用，许可随源保留。
- 再生：`node scripts/login-anatomy/build.mjs --audit`，Node 20.10+ 和已安装 Anatomy；`ANATOMY_SKILL_ROOT` 可指定安装位置。页面运行不依赖技能、网络或前端库。
- 责任：本对话 root / LOGIN-ANATOMY-001；批准来源与局部例外见 [登录规格](../design/changes/login-anatomy-001.md)。生成一致性及模型/遮挡检查由相邻 verify-scene.mjs 执行。
- 证据：构建审计/姿态 JSON、截图与浏览器步骤输出只放系统临时目录或本任务 Codex visualizations，不进 Git、不含密码/真实材料；正式源码长期保留，替换须随来源重建。

## RESEARCH-DOSSIER-001 · 第一阶段隔离验证

- 来源：`scripts/test-research-dossier-postgres.sh`，合成 fixture 与 API 手动预览测试；不连接共享运行库或外部模型。
- 位置：随机 `linggan-intelligence-research-dossier-proof-*` Docker container/volume，`/tmp/linggan-dossier-*` 构建、日志与合成媒体；浏览器审阅截图放当前任务授权的 Codex visualizations 目录，稳定名 `research-dossier-phase1-*.jpg`。
- Git：只提交脚本、fixture 和汇总验证记录；不提交二进制、截图、原文、DSN 或凭据。
- 再生与清理：上述脚本运行必要集成回归；`--http` 只验证接口，`--preview` 只运行合成页面服务直到中断。脚本 EXIT 核实清理自己的 DB/container/volume；页面验证后停止候选服务。临时构建缓存可复用，媒体与截图仅用于本任务审阅。

### 第一阶段真实验收续修（#420）

Mog 授权三篇指定作品在本机核对与重新分层。私有材料快照、几何回放输入、运行日志放 `/tmp/linggan-dossier-acceptance/`（0700），不入 Git、不发送模型；结束后删除原始材料快照。截图保留在本任务 Codex visualizations，命名 `research-dossier-acceptance-*.png`，用于 Mog 验收。候选 API 仅监听本机独立端口，读取同一共享材料并只执行这三篇已授权的追加分层；不替换 3000、不启动 worker。验收结束停止该进程。

## UIUX-CONTENT-WORKBENCH-001 阶段一真实页面走查

- 固定位置：当前专属 worktree 的 `artifacts/private/uiux-content-workbench-001/2026-10-14/`，已受 `/artifacts/private/` 忽略。
- 来源：Codex in-app browser 当前页面截图、只读 DOM 摘要、页面操作和运行/源码核对元数据；只保存完成现场检查的证据，不用历史截图替代。
- Git：原始截图、DOM 和逐文件哈希清单不入库；仓库仅存脱敏问题说明、证据编号、源码定位与复现步骤。平台来源 URL 仅随本机私有证据保留；不读取或保存登录凭据、API 密钥、数据库连接信息。
- 再生：按本专项阶段一审计中的路由和只读操作，重新浏览当前真实页面；新回执必须注明新的 revision 与时间。
- 保留：供 Mog 本地验收与后续设计使用；未经明确清理授权不删除，不上传到外部服务。图片禁止手工改造后冒充原始现场。

## UIUX-CONTENT-WORKBENCH-001 · 阶段二原型

- 维护源：`docs/design/pages/content-workbench-prototype/{index.html,prototype.css,prototype.js,fixture.js}`。维护者为本事项 Codex；唯一输入为用户目标、LIDS 与本地合成演示数据；可入 Git，但不进入运行 assets。
- 再现：进入该目录运行 `python3 -m http.server 4177 --bind 127.0.0.1`，浏览器打开 localhost:4177；无真实 API、生产库或模型依赖。
- 生成截图 / 导出 ZIP / PNG / 校验值进入 `artifacts/private/uiux-content-workbench-001/phase-2/`，不入 Git。验收文档记录路径和结果。
- 浏览器仅使用 `linggan-workbench-prototype-v1` 本地键保存演示草稿，页面提供清除入口，不写真实应用的存储键。
- 检查：JS 语法、文档治理、三页真实点击、窄屏、图片尺寸和导出文件内容核对。用户审美签收单独记录；静态原型不代表产品已实现。

### 阶段二 V2 · 材料片段与改稿画布

- 用户本轮提供本地 15 秒操作视频与改稿对照截图，授权分析并构建独立交互原型。原附件保留原位，不入 Git；提取关键帧与核验截图放 `artifacts/private/uiux-content-workbench-001/phase-2-v2/`，本地 ffmpeg scene 变化检测再生，不上传到外部服务。
- 新原型维护源放 `docs/design/pages/content-workbench-prototype/v2/`，沿用 V1 设计源并以新浏览器存储键隔离；允许源入 Git，不能进入运行 assets。V1 供 Mog 当前体验，保持原样。
- V2 验证用合成多篇笔记、原声与片段；不从私有视频复制内容到公开资产，不把合成类似选题数量说成真实系统统计。截图、导出、视频关键帧与 hash 均留在私有忽略目录，结论进入同一专项提案与验收记录。


### 阶段二 V3 · 空间材料浏览与作品关联

- 维护源：`docs/design/pages/content-workbench-prototype/v3/`，维护者为本事项 Codex；用户本轮明确派定。复用 V1 基础 CSS/fixture 与 V2 研读样本/样式，V3 的脚本与局部 CSS 独立；不进入运行 assets。
- 输入：Mog 的视频、新片段截图、合成来源和 LIDS。参考媒体留在原位，不复制进公开资产。新增书页与纸条以原生 HTML/CSS 呈现。
- 存储：仅 `linggan-workbench-prototype-v3`，不读取或覆盖 V1/V2 草稿。静态服务继续为 loopback 4177，入口 `/v3/`。
- 截图、实际 ZIP/PNG、导出清单、走查回执放 `artifacts/private/uiux-content-workbench-001/phase-2-v3/`，不入 Git。可按同一合成选题与 UI 路径再生，不上传外部服务。
- 验收：筛选 / 纸条 / 分布 / 翻页联动，画布到正文与稳定内页关联，排序 / 撤销 / 恢复，桌面和窄屏，实际导出；业务接入和 Mog 审美签收另行记录。

## UIUX-CONTENT-WORKBENCH-001 · 首包候选验证

脚本、合成fixture与汇总报告入Git；构建缓存使用既有target或任务临时目录，隔离PostgreSQL容器/卷以content-workbench-proof前缀登记清理；截图、实际PNG/ZIP、运行回执与测试日志放本工作树 artifacts/private/uiux-content-workbench-001/implementation/，不入Git。临时Issue/Claim正文可放/tmp，完成后可清理；来源用户原文不提交。候选作品资产/导出放独立测试媒体根，仅本包运行使用。再生由本包测试脚本与验收步骤负责，原始证据不改造。

## CONTENT-WORKBENCH-001 正式资产存储约定（候选，尚未部署）

- 运行文件：`$LINGGAN_CREATION_ASSET_ROOT/works/<UUID>/` 下的自有图片、图页 PNG、ZIP 与内部 manifest；launch 候选配置指向 `~/Library/Application Support/Linggan Intelligence/creation`，不随 checkout 更新删除，不进 Git。
- 文件读取通过作品与导出所有者校验；完成成品不可覆盖。当前只新增候选配置，未创建正式目录、未迁移共享库。
- 隔离测试采用 `/tmp/linggan-content-workbench-assets.*`，由 proof 脚本生成并在退出时清理自己的精确目录；验收截图与已验证样例按前述私有 implementation 目录留存。

## 2026-10-15 · WEB-REACT-001 下的原生候选暂停快照

- 路径：`artifacts/private/uiux-content-workbench-001/implementation/paused-native-ui-2026-10-15/`，已忽略，不提交。
- 来源：本包7个未提交原生前端文件、撤接前集成diff与合成preview fixture；`manifest.json` 记录逐文件SHA256。用途是可逆暂停及阶段C算法核对，不得作为正式页面或另一个设计标准。
- 原生源码从业务目录撤出；V1/V2/V3设计原型保持原位。阶段C确认可复用规则并形成正式实现后，由包负责人按证据保留需要清理快照。
- 上次中断遗留的本包 `content-workbench-proof-20261014172930_90983_e6f5041c` 容器与同名data volume已精确删除；共享PostgreSQL未动。


## WEB-REACT-001-C · 本地中文字体与浏览器证明

- 字体维护源：`apps/web/src/assets/fonts/noto-sans-sc.woff2` 与同目录 `OFL.txt`；由 root 从 Google Fonts 官方 `ofl/notosanssc/NotoSansSC[wght].ttf` 核对许可后无子集转换为 WOFF2，保留全部字形与可变字重，随代码提交；上游 revision、源/产物 SHA256 随本节补录。字体只由本机 Axum `/ui-assets/` 分发，页面不请求外网字体；修改字体后同时更新渲染版本，禁止后台自动下载。
- 本阶段浏览器脚本：`apps/web/test/e2e/workbench-real-chain.mjs`；Rust 启动夹具 `apps/api/src/local_web/content_workbench_browser_tests.rs`，必须 multi_thread，运行一次性隔离 PostgreSQL + Axum + React。截图、报告、实际PNG/ZIP保存在既有私有 implementation 目录；CI仅保留合成证明产物7天。
- 统一入口：`scripts/test-content-workbench-postgres.sh --browser`，默认后端证明继续可独立运行，CI执行后端与浏览器两种模式。合成来源/凭据不代表真实选题或真实模型验收。

字体核对：上游 `2894aab31764f10f29c421bdfd2340d3b382d384`；源 TTF SHA256 `a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`；WOFF2 SHA256 `ed05125987c35ee57be16be7242568f1d516fbf497bc0ef4161499ca9e8d8fce`；产物 7782120 字节。转换工具 fontTools 4.62.1，`TTFont(source).flavor="woff2"`，不裁掉字形；OFL 1.1 文本随文件保留，仅移除上游一处行尾空格以通过仓库空白检查。

## WEB-REACT-001-D1 · 创作者页真实链路证明

- 来源：`apps/web/test/e2e/creators-real-chain.mjs`（人工维护的 Node/Playwright 脚本）与 `apps/api/src/local_web/creators_browser_tests.rs`（Rust 启动夹具，必须 multi_thread），由既有 `scripts/test-web-app-postgres.sh` 在一次性隔离 PostgreSQL（随机 `linggan-web-app-proof-*` 容器与数据卷）上依次运行；播种只用合成作者与作品（`合成领域一/二`、`合成作者 NN`），不调用模型、不访问平台、不连开发库或本机常驻服务。
- 固定位置：证据放在 `WEB_APP_PROOF_DIR`（缺省 `${TMPDIR}/linggan-web-app-proof-*`）下的 `creators/` 子目录：`fixture.json`（本次播种的领域与作者编号）、`report.json`（逐步结果、写请求、失败请求、存储键）、各视口截图 PNG；CI 作为 `web-app-real-chain-proof` 产物保留 7 天。
- 不入 Git；不含凭据、真实作者或真实原文。截图是合成数据的界面证据，不代表真实数据量下的表现。
- 再生：`WEB_APP_PROOF_DIR=<目录> ./scripts/test-web-app-postgres.sh`。容器、数据卷与支持目录由脚本 trap 删除并核验；证据目录可手工删除。责任 Issue #433。

## WEB-REACT-001-D2 · 证据库真实链路证明

- 来源：`apps/web/test/e2e/evidence-real-chain.mjs` 与 `apps/web/test/e2e/evidence-plugin-entry.mjs`（人工维护的 Node/Playwright 脚本）、`apps/api/src/local_web/evidence_browser_tests.rs`（Rust 启动夹具，必须 multi_thread）、`apps/api/src/local_web/evidence_api_proof_tests.rs`（接口证明）与播种模块 `evidence_seed.rs`／`evidence_fixture.rs`，由既有 `scripts/test-web-app-postgres.sh` 在一次性隔离 PostgreSQL 上依次运行；播种只用合成作品、评论与媒体，不调用模型、不访问平台、不连开发库或本机常驻服务。
- 固定位置：证据放在 `WEB_APP_PROOF_DIR` 下的 `evidence/` 子目录：`fixture.json`、`report.json`（逐步结果、请求、写请求、失败请求、存储键）、`dr1-dead-controls.json`（失效控件的真实调用记录）、`plugin-entry.json`（插件入口的请求头与 HTTP 状态）、各视口与各分区截图 PNG；CI 作为 `web-app-real-chain-proof` 产物保留 7 天。合成媒体字节写在支持目录的 `media/` 下，只经本机媒体句柄读取。插件入口证明的最小测试扩展与浏览器配置写在系统临时目录（`linggan-entry-extension-*`、`linggan-entry-profile-*`），脚本结束时删除；它自起的记录服务器只监听 `127.0.0.1` 随机端口。
- 不入 Git；不含凭据、真实作品或真实原文。截图是合成数据的界面证据，不代表真实数据量下的表现。
- 再生：`WEB_APP_PROOF_DIR=<目录> ./scripts/test-web-app-postgres.sh`。容器、数据卷与支持目录由脚本 trap 删除；证据目录可手工删除。责任 Issue #433。

## WEB-REACT-001-G · 实际采集页面与 React 迁移对照

- 固定位置：本工作树 `artifacts/private/web-react-001-g/`，含 `baseline/`（3000 实际页面截图及最小读取快照）、`candidate/`、`*-full*` 最终完整滚动及视口快照、`comparison-final/` 和本机验证日志。受既有 artifacts/private 忽略，不提交。
- 来源：Mog 2026-10-16 明确要求的实际页面截图核对；3000 只允许同源 GET，禁止写入与平台/模型请求；同数据候选 GET 可连接强制 read-only 池且外层拒绝非 GET/HEAD；候选写入只在隔离 PostgreSQL。
- 生成：本阶段维护的 Playwright 基线/候选/对照流程，记录 revision、视口、URL、滚动与展开状态；合成证明可在 CI 运行，真实快照/截图不得上传 CI 或 Git。
- 清理：候选服务/容器/卷仅清理本任务精确名称；本机视觉证据留至 Mog 验收后按事项清理，不改造原图。

## WEB-REACT-001-E · 运营五页迁移证据

- 固定位置：本工作树 `artifacts/private/web-react-001-e/`，baseline/candidate/comparison 与日志不入 Git。由本包 Playwright 及隔离 Axum/PG proof 再生。3000 及真实数据候选只允许同源 GET/HEAD，目标写入仅隔离 PG。
- 截图覆盖实际页面全滚动、表格滚动、对话框与多视口；不修改原图冒充现场，不上传真实业务截图/快照。CI 只用合成 fixture；证明产物保留七天。容器、卷和候选服务按本任务精确名称清理。Mog 验收前保留本地证据。
