# 生成型文件登记表

> 状态: 权威当前
> 最后核对: 2026-10-18
> 适用范围: 构建、代码生成、导出、日志、测试证据、备份与临时文件
> 事实来源: 当前工具配置、`.gitignore` 与来源校验清单
> 冲突时以谁为准: 生成源、工具配置和安全规则；生成结果不得反向覆盖来源

任何未列入本表的生成型文件，默认不得写入项目目录或提交 Git。确需新增时，先登记再生成。

| 类别 | 固定位置 | 来源/生成方式 | Git 策略 | 手工修改 | 保留与清理 |
|---|---|---|---|---|---|
| WEB-REACT-001 D3 私有证据 | 当前 D3 工作树 `artifacts/private/web-react-001-d3/`：只读实际基线、GET 回执、候选截图／逐像素差异、盘点辅助及验证日志 | root 的 GET/HEAD 限制浏览器采集、同输入重放、隔离库脚本；无共享库写入、无外发 | 忽略，不提交；真实评论／材料不进入代理或 CI | 不得伪造回执或修改原始像素；分析摘要可更新 | 保留至本包审阅；交付后移交主 checkout 同名私有目录再清理本工作树 |
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
| Linggan Browser Producer release | `plugins/linggan-browser-producer/releases/linggan-browser-producer-<version>.zip`、`plugins/linggan-browser-producer/releases/release-manifest.json` | `cd plugins/linggan-browser-producer && npm run build`；唯一源为该包的 `src/` 和 Linggan runtime token source `apps/web/src/styles/lids_tokens.css`，build script 同时生成 ZIP/manifest | 提交 | 禁止手工修改 ZIP 或 release manifest；修改源后重新 build | 当前只保留当前发行版；替代/删除必须经单独发布卡和 hash 核对 |
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

- 维护的测试源：React 评论页测试 `apps/web/src/pages/corpus/comments/*.test.tsx` 与 `crates/intelligence/tests/comment_like_library_postgres.rs`；旧 `scripts/test-comment-like-library-ui.cjs` 随 D3 删除；不是生成代码。合成测试日志、临时 HTTP/浏览器脚本放系统临时目录 `pr401-*`，不入 Git。PG 容器和卷由现有随机名脚本创建并在退出时清理核验。
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

- 历史测试源：`scripts/test-topic-map-ui.cjs`、`scripts/test-topic-map-compact-ui.py` 当时为维护源码、只运行合成 HTTP fixture 与候选 JS/CSS；WEB-REACT-001-F 同包退役，当前替代为 `apps/web/src/pages/topics/*.test.ts*`、`apps/web/test/e2e/topics-real-chain.mjs` 与 `scripts/test-web-app-postgres.sh` 的正式 React/Axum/PG 证明。历史截图仍只证明当时版本。
- 源码传递包、补丁、截图与日志：本任务系统临时目录和 Actions artifact（源码包 1 日、验证结果 7 日）；不入 Git、不含真实原文/凭据、不访问模型/平台/正式库。
- `.github/workflows/topic-map-compact-transfer.yml` 仅用于专属候选分支的版本校验与传递，交付前删除；持续回归另用只读验证 workflow。临时传递仅影响此分支，不强推、不合并或部署。

## COLLECTION-JTBD-001 / #398 隔离验证

- 来源：`scripts/test-collection-single-run-postgres.sh`、正式 API router 与合成 PostgreSQL、`scripts/test-collection-single-run-ui.cjs`；正式插件 fixture。
- 位置：`/tmp/collection-398-*` 日志、浏览器依赖与合成截图；本任务随机 `linggan-collection-single-run-proof-*` 容器和同名 `-data` 卷；Cargo/npm 现有忽略缓存。审阅截图复制到本任务授权的 Codex visualizations 目录，不入 Git。
- 无真实材料、账号、DSN、Cookie 或凭据；交接包六张用户截图只读，不复制进仓库或 CI。
- 再生：上述脚本加 `COLLECTION_RUN_BROWSER_PROOF=1` 与明确临时证据目录；发布候选按正式插件现有脚本生成，不加载插件。
- 清理：脚本退出核验删除本次随机容器/卷；浏览器上下文及测试服务器退出关闭，不清理其他任务或业务资源。责任 #398 当前执行者。

## LOGIN-ANATOMY-001

- WEB-REACT-001 H 后生成位置为 `apps/web/src/pages/login/assets/` 与 `sceneModel.ts`；旧Rust HTML已删除，可经PR #411历史恢复；禁止手改生成物。
- 唯一源：`scripts/login-anatomy/{model.mjs,parts.mjs,order.mjs,build.mjs}及apps/web/src/pages/login/Login.module.css`；表单及生命周期由React/TypeScript维护；Anatomy 技能的 MIT kit 仅在生成时使用，许可随源保留。
- 再生：`node scripts/login-anatomy/build.mjs --audit`，Node 20.10+ 和已安装 Anatomy；`ANATOMY_SKILL_ROOT` 可指定安装位置。页面运行不依赖技能、网络或前端库。
- 责任：本对话 root / LOGIN-ANATOMY-001；批准来源与局部例外见 [登录规格](../design/changes/login-anatomy-001.md)。生成一致性及模型/遮挡检查由相邻 verify-scene.mjs 手动执行，依赖已安装 Anatomy 技能，不在CI中；正式前端构建与登录浏览器生命周期另由Web CI验证。
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
- 固定位置：证据放在 `WEB_APP_PROOF_DIR` 下的 `evidence/` 子目录：`fixture.json`、`report.json`（逐步结果、请求、写请求、失败请求、存储键）、`dr1-dead-controls.json`（失效控件的真实调用记录）、`plugin-entry.json`（插件入口的请求头与 HTTP 状态）、各视口与各分区截图 PNG；某一步失败时另有 `failure-step-N.png`（失败现场截图），`report.json` 同时记下当时打开着的对话框文字；CI 作为 `web-app-real-chain-proof` 产物保留 7 天。合成媒体字节写在支持目录的 `media/` 下，只经本机媒体句柄读取。插件入口证明的最小测试扩展与浏览器配置写在系统临时目录（`linggan-entry-extension-*`、`linggan-entry-profile-*`），脚本结束时删除；它自起的记录服务器只监听 `127.0.0.1` 随机端口。
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

## WEB-REACT-001-F · 主题图谱迁移证据

- 固定位置：本工作树`artifacts/private/web-react-001-f/`，含实际3000基线、只读候选、完整滚动/四视口截图、最小JSON快照、差异/性能/验证日志；忽略、不提交，禁止上传真实材料。
- 来源：本包Playwright、真实Axum+隔离PG的合成proof及同源GET/HEAD视觉桥接；真实写命令只在一次性隔离PG。自动viewed回执若由当前页面触发在视觉捕获中阻断并如实记录，不伪造回执。
- 原图保留不改；候选只读桥接不作正式链路证明，原始API快照最小必要本机保存。合并后按既有私有运行证据规则先保存在canonical相同artifacts/private路径，再移除交付worktree。

### WEB-REACT-001-F · 概览 B 修订证据

位置为同工作树 `artifacts/private/web-react-001-f/overview-b/`，忽略、不提交。来源为正式源码构建、现有隔离 PG 链路及同源 GET 实际候选；保存改版前/后截图、最小 JSON、差异说明和验证日志。原迁移截图不覆盖。截图含真实材料，只在本机使用；复现与清理继承 F 证据规则。供用户查看的选定设计及最终截图可另复制至当前 Codex visualization 任务目录，不入 Git。

### WEB-REACT-001-F · #441 内核修复与本机发布

固定于本工作树 `artifacts/private/web-react-001-f/kernel-release/`：合成PG复现/完整回归/独立审查/CI日志、无秘密发布前后身份/策略/调用计数与实际3000截图。忽略、不入Git，真实原文/凭据不得交模型或CI。一次性runner只调用已有隔离脚本前置并按同一清理规则销毁受控资源。合并后先复制到canonical同名private证据路径，再移除交付worktree。

## 2026-10-15 · V3 还原的私有真实文本验收副本

- 仍使用 `implementation/v3-fidelity/` 私有目录；真实笔记/原声、账号身份、来源映射、作品回执、截图与成品包均忽略，不进 Git、不上传 CI。源副本只来自既有本机读取，不代表重新采集或全库覆盖。
- 维护源为 `content_workbench_private_review.rs` 测试夹具；只有显式 `CW_BROWSER_MANUAL_REVIEW=1` 才接受 `CW_BROWSER_REAL_SAMPLES`，复制到一次性库并保留原笔记与父评论映射。缺失作者/互动信息不补造。
- 再生入口：设置上述变量和 `CW_BROWSER_PROOF_DIR` 后执行 `scripts/test-content-workbench-postgres.sh --browser`；不携带私有样本时仍为 CI 的合成证明。人工模式保留候选最多八小时，在证据目录写入 `review-complete` 后退出并清理隔离资源。人工模式不会冒充自动浏览器证明。
- 保留本包必要的真实样本、来源映射、最终作品及成品证据直至 Mog 验收；不必要的目录抓取和重复截图按本包精确路径清理。正式库、常驻 runtime 与来源原件均不在清理范围内。

## WEB-REACT-001-CLOSEOUT · 全站收尾证据与登录静态素材

- 责任：root，#430完整收尾授权；[验收矩阵](../design/acceptance/web-react-001-closeout.md)。
- 私有证据固定于本工作树 `artifacts/private/web-react-001-closeout/`，包括旧实际GET基线/截图、新版同输入截图/差分、隔离PG/浏览器日志、真实首篇/最小AI回执和部署/离线/回退证据；忽略、不入Git、不交CI或模型。结束前移交主checkout同名目录；原始像素/回执不修改。
- 登录生成素材：`apps/web/src/pages/login/assets/{station.svg,scene-data.json}` 与 `sceneModel.ts` 由 `scripts/login-anatomy/build.mjs` 和原模型/几何唯一源再生，允许入Git；不得手改生成物。`fallback.svg` 为已确认原静态图，从旧运行页面原样提取；旧页面可经历史Git恢复，不保留运行双轨。样式归入Login.module.css，继承LOGIN-ANATOMY-001局部例外。
- 旧页面/动画基线仅存本机私有目录，生成审计使用原LOGIN登记临时目录；后续构建不再生成Rust HTML。


## AI-GEN1 / PR-1A 隔离证据

- 固定证据目录：本工作树 `artifacts/private/ai-gen1-pr1a/`；从实际 Cargo/治理/隔离 runner 输出保存日志与最终收尾审查。忽略、不提交、不伪造；只含合成资料与无凭据元数据。系统临时目录中本次 `ai-gen1-pr1a-*.log` 是同一过程的临时输出，可核对后删除；最终保留到本包审阅结束，移除工作树前按需移交同名私有证据目录。
- 重放入口：`scripts/test-research-dossier-postgres.sh --note-material` 与原完整兼容 runner。随机 Docker PG/卷/数据库只由该脚本创建并由其 trap 清理；不借用共享 DSN、不清理其他容器或 schema。CLI 的 plan/receipt 是操作者显式生成的本机 JSON，不自动写进 Git；无正文。真实范围的 plan/apply 尚未获派。
- `Cargo.lock` 按既有登记由 Cargo 管理；本包加入已解析的 Unicode normalization 依赖，不手改生成锁。没有新前端生成物或发布包。

## 前期规范包生成登记（保留历史生命周期）

以下记录描述 LIDS-ALIGN-001 的规范/审阅阶段；本轮真实代码采用以末尾 LIDS-UI-001 为准。

## LIDS-ALIGN-001 · 逐项对齐表与校验

- 手工维护源：`docs/design/lids/review-alignment.json`，由已核对审阅稿导入，含来源SHA、用户确认摘要、逐项状态和正式落点；候选要求不作为正式规则。规则正文继续由LIDS分册维护。责任为本次文档交付者root。
- 派生文件：`docs/design/lids/review-alignment.md`，允许入Git，禁止手改表格；`python3 scripts/verify-lids-alignment.py --render`再生，`--check --self-test`检查一致性、来源/状态及反例。不需要外部审阅目录即可读取、检查。
- 校验日志、业务源码前后指纹和临时维护脚本放本任务系统临时目录，不入Git；不含真实业务原文、字体字节或秘密。重新运行治理检查及相同tracked文件指纹即可再生，审阅后按本任务精确路径清理。

## LIDS-TYPE-001-B · 唯一排版参数与生成镜像

- 手工权威源：`docs/design/lids/typography.json`；来源为Mog对Q01/Q02明确采用B、Q03原声采用OPPO Sans的决定，维护职责为排版规范维护者。该源允许入Git，业务资产及运行Token未改。
- 项目派生表：`typography.md`中的TYPE-B标记区，由`python3 scripts/verify-lids-alignment.py --render`生成；`--check --self-test`核对表格、采用状态、用户依据及已关闭Q项。
- 隔离审阅镜像：任务visualizations的`intelligence-design-system/spec/registry.json → visual.typographyApplication`，由`python3 scripts/sync-lids-typography.py --review-root <审阅根目录> --write`生成，`--check`校验参数、来源SHA与状态。它是生成缓存，不是可手改的第二参数源，也不进入业务构建。镜像包含本机生成checkout路径；工作树迁移后从新的正式项目源重新生成。审阅predev/prebuild自动校验同源参数，漂移时拒绝构建。参考册再由审阅包`spec/tools/render.py`生成。
- 原A对照参数与采用前审阅记录保留历史身份；审阅入口只应用B，历史URL的variant/custom不成为当前覆盖值。无额外全局设置、数据库、服务或部署。
- 采用前后指纹、浏览器截图、检查回执在同任务`typography-review/adoption-05/`本机保留，不入项目Git；无真实业务原文或秘密。

## Q03-VOICE-06 · 原声对照与三级导航证明

- Q03已由ALIGN-VOICE-004确认OPPO Sans；voice-06保留历史字族对照，当前规则及唯一参数源在LIDS分册。三级页面Tab归Z02的已确认位置按ALIGN-NAV-003执行；不是新CMP推广。
- 隔离审阅代码与合成评论：任务visualizations内intelligence-design-system/reference-study/src/voiceReview；passage.json是同段原文唯一源，不含真实评论或个人数据，评论详情／阅读页及两种字族共用。工具状态在内存，意见手动导出，不写业务。
- Serif样例为官方google/fonts固定revision的NotoSerifSC[wght].ttf，连同原OFL.txt和METADATA.pb仅保存在同审阅工程public/fonts/notoserifsc/；SHA、来源URL与固定revision见本机typography-review/voice-06/font-source.json及对齐清单voiceReview.fontEvidence。禁止后台更新或复制进业务字体资产，Q03已确认Sans，生产接入另按Q11决定。
- 指纹、截图、浏览器实测和构建回执固定在typography-review/voice-06/，不入项目Git，无模型、业务写入或部署；保留至Mog审阅，按精确本机目录清理。

## Q04-FOCUS-07 · 焦点反馈审阅与Q03收口

- Q03由Mog明确采用OPPO Sans；当前审阅入口只显示Sans，旧voice-06对照证据及Noto样例字节保留历史身份，不再作为活跃选项。B字号／行高／字重和ALIGN-NAV-003不变。
- Q04隔离源码固定在同任务intelligence-design-system/reference-study/src/focusReview/；复用Find、Select、Button、PagePreview，不复制业务实现。组件／创作者／新建采集仅为焦点审阅的合成工作面；输入、校验、选择、记录均在内存，无业务请求。
- Q04已按ALIGN-FOCUS-005采用C；旧focus-07保留历史候选对照，当前focus入口仅C，隔离审阅共用已采纳参数。源码与当前入口由同任务README及字体审阅入口索引。
- 指纹、截图、浏览器实测、构建及治理回执固定在同任务typography-review/focus-07/，不入项目Git；无真实原文、秘密、模型或部署。保留至Mog审阅，精确目录清理。

## Q06-SHELL-10 · 壳层容量与窄屏收纳审阅

- 候选源及隔离源码固定于同任务 `intelligence-design-system/reference-study/src/shellReview/`，`presets.json`为三种收纳策略的唯一候选参数源，不入业务构建。沿现有 `?page=system&section=review&review=shell` 审阅路由，复用Select、TertiaryTabs、个人菜单与PagePreview内容；默认页面不应用候选。
- 指纹、源码核对、截图、浏览器实测、构建及治理回执固定于同任务 `typography-review/shell-10/`；不入项目Git，无真实数据或秘密。来源与当前入口从任务README、spec/AI-ENTRY.md及项目shell-zones／review-items索引；保留至Mog审阅，按精确目录清理。
- Q06保持待审；字体／B字级／C焦点／M2材质源与领域、三级Tab的Z02归属固定。预览宽度、长名称、时钟及导航收纳均仅为压力样例；不授予业务代码、路由、权限、全局设置或部署变更。

## ALIGN-MATERIAL-006 · Q05采用M2

- 唯一手工参数源：`docs/design/lids/control-material.json`（入Git），职责与替代关系从materials.md／primitives.md进入。`scripts/sync-lids-material.py`生成同任务审阅`spec/registry.json → visual.materialApplication`；隔离`reference-study/scripts/check-lids-material.mjs`于predev／prebuild核对SHA与内容。镜像不可手改，业务Token不在本次范围。
- 同任务固定证据目录：`intelligence-design-system/typography-review/material-adoption-09/`，保存采用前后指纹、浏览器测量、截图、检查与README，不入项目Git。责任方为当前LIDS-ALIGN-001执行Agent；保留至Mog审阅，清理仅限该精确目录。入口从任务README与spec/AI-ENTRY.md索引。
- 旧material-08截图、参数和回执保留为采用前历史；原presets.json降级历史对照，当前审阅只消费M2生成镜像，不再提供M1/M3或临时微调。M-02E、M-06、V8_workbench范围不变。

## ALIGN-FOCUS-005与Q05-MATERIAL-08 · 焦点采用与材质审阅（历史阶段）

- 已确认Q04参数唯一源：docs/design/lids/focus.json（入Git），primitives.md说明职责；sync-lids-focus.py将其生成到隔离registry.visual.focusApplication，审阅predev／prebuild校验SHA及内容。只迁移隔离审阅样式，不改业务Token。
- Q05采用前候选源：同任务intelligence-design-system/reference-study/src/materialReview/presets.json；已由ALIGN-MATERIAL-006采用M2取代，原参数只保留历史。隔离源码在src/materialReview，入口与回执从同任务README索引。
- 本轮基线、截图、浏览器实测、构建／治理结果及审阅导出试样固定于同任务typography-review/material-08/，不入项目Git；样例与操作仅本地合成及内存，无真实业务、秘密、外部模型或部署。旧focus-07保留历史候选对照，当前focus入口只应用已确认C。保留至Mog审阅后按精确目录清理。


## ALIGN-SHELL-007 / Q07-LEDGER-11 · A壳层采用与单行账本审阅

- 正式壳层参数源：`docs/design/lids/shell-layout.json`；由`scripts/sync-lids-shell.py`生成同任务`intelligence-design-system/spec/registry.json`的`visual.shellApplication`；隔离`reference-study/scripts/check-lids-shell.mjs`守卫同源。
- 隔离源码：同任务`reference-study/src/shellReview/`采用A并增加图标侧栏开关；共享`dsReview/PersonalMenu.jsx`提取原菜单，避免壳层循环依赖。`src/ledgerReview/`仅Q07候选，参数在`presets.json`，复用原任务fixture和详情语义。
- 证据目录：同任务`typography-review/shell-adoption-ledger-11/`，含README（变更清单、表面/状态/依赖/验收矩阵）、before.json、verification.json、浏览器回执、截图、构建/治理日志和审阅意见样例。创建前登记；不入业务构建。
- Q06采用A和用户要求的手动图标折叠，Q07行高/列宽/字段优先级/窄屏三案待审。一任务一行、不缩字号、不多行回退；字体B/C/M2及Z02领域/三级Tab固定。业务、权限、服务、数据库、字体资产、全局配置、合并与部署均不在本次范围。


## ALIGN-LEDGER-008 / Q08-COMPOSITION-12 · L1账本采用与页型组合审阅

- 正式源`docs/design/lids/ledger-layout.json`由用户Q07选择L1/44px及当前任务名称列144px形成；`scripts/sync-lids-ledger.py`生成隔离registry.visual.ledgerApplication，`reference-study/scripts/check-lids-ledger.mjs`守卫同源。其余列宽维持原L1各字段值，不将全表列强制成144px。
- 隔离`src/ledgerReview/TaskLedger.jsx`承载统一账本组件；Q07活跃入口仅L1，旧presets保留历史。常规P09、Q06/Q08与原连续账本的行尺寸和字段定义读已采用源；真实业务代码不改。
- 隔离`reference-study/src/compositionReview/`为Q08候选，沿review=composition路由，读取原17页/26模式、11页型、组件与动作候选。A分带/B合带/C常驻只比较允许区域组合，不覆盖Z02规则或新增产品能力。共享ShellFrame通过可选页面配置复用已采用A。
- 证据固定于同任务`typography-review/ledger-adoption-composition-12/`：README清单/表面状态依赖验收矩阵、before/verification、用户144px字段证据、来源、浏览器矩阵/截图、构建与治理日志。创建前登记，不入业务构建，不作生产/组件认证。Q08保持待审，Q09–Q12不展开；禁止业务API、模型、数据库、服务、字体资产、全局设置、推送/合并/部署变化。

## ALIGN-COMPOSITION-009 · Q08采用与用户问题卡片保留

- 唯一组合规则源为`docs/design/lids/composition-layout.json`，从patterns.md按需进入，登记检索B/功能C与入口表单豁免、逐模式映射、Z02与局部动作归属；`scripts/sync-lids-composition.py`生成同任务`spec/registry.json → visual.compositionApplication`，`reference-study/scripts/check-lids-composition.mjs`核对源/镜像。
- 隔离`reference-study/src/compositionReview/`沿既有入口采用已选组合；旧presets及前轮ledger-adoption-composition-12保留历史。`reference-study/src/dsReview/ProblemCards.jsx`和`problem-cards.css`仅在隔离原型补齐真实用户问题卡片的结构与材质，合成内容、已有B字体/C焦点映射，不改业务来源组件/CSS或真实数据。
- 来源为专属工作树的`apps/web/src/pages/corpus/comments/ProblemCards.tsx`、`Results.module.css`及真实3000只读结构/计算样式核对；保留问题/线索卡、原声切角、硬投影、黑色编号、指标/时间和整卡详情入口，不借通用检索表格替代。卡片保护不扩展为整个业务页面或组件认证。
- 新证据固定于同任务`typography-review/composition-adoption-13/`：README、before/verification、结构/计算样式（不含真实用户原文）、源码指纹、合成截图、宽窄布局和交互/同源/构建/治理记录。创建前登记，不入项目Git；责任为当前LIDS-ALIGN-001执行Agent，保留至Mog审阅，清理仅限该精确目录。业务/运行/字体资产/全局文件、真实命令及合并部署边界不变，Q09–Q12不自动批准。

## Q09-OVERLAY-14 · 浮层、离开与长任务交互审阅

- 候选源码为同任务`reference-study/src/interactionReview/`，沿`?page=system&section=review&review=overlays`；presets.json声明候选策略、形态和场景，model.js为内存状态演练，OverlayPanel.jsx及InteractionReview.jsx渲染可操作对照。复用ShellFrame、PagePreview、TaskLedgerSurface、ProblemCards及既有B/C/M2/C焦点；只添加可选审阅回调，默认已采用页面不变。
- 候选比较模态抽屉/居中弹窗/只读非模态详情、遮罩/Escape/显式关闭、脏稿保护、提交中离开/回执跟踪、暂停/继续/停止/结果未确认/版本冲突。真实合同的原身份核对、停止不可继续、已得与在途材料保留不作为可选项，不提供真实API/Provider/数据库/Storage写入。
- 来源定向只读核对项目`RunDialog.tsx`、`RunCreationDialog.tsx`、`RunDrawer.tsx`、`runRequests.ts`、`StudyDialog.tsx`、`ProblemDrawerShell.tsx`、`WorkbenchPage.tsx`及collection-single-run-001合同。七项浮层合同的候选覆盖不等于32交互全部认证，新增默认仍待Mog审阅。
- 证据目录固定为同任务`typography-review/overlays-14/`，含有界清单、before/verification、来源、浏览器状态/焦点/窄屏/回执与关闭证据、合成截图及构建/治理日志；创建前登记，不入业务Git，不存真实用户原文/秘密。入口由任务README、spec/AI-ENTRY及项目interaction-contracts/review-items路由。责任为当前LIDS-ALIGN-001执行Agent，保留至Mog审阅，仅按精确目录清理；不新增提交、推送、合并、部署或更改全局设置。

## ALIGN-OVERLAY-010 / Q10-COMPONENT-15 · S1收口与组件准入审阅

- 当前用户U20采用Q09 S1，保留未保存保护、原请求跟踪和停止不可恢复；唯一规则源为项目`docs/design/lids/overlay-policy.json`，已确认规则进入interaction-contracts。`scripts/sync-lids-overlay.py`生成隔离`spec/registry.json → visual.overlayApplication`，`reference-study/scripts/check-lids-overlay.mjs`在predev/prebuild拒绝漂移；旧S2只留14证据，不是活跃候选。新增JSON为权威规则，脚本为再生/检查入口，入项目Git；业务和字体资产不动。
- Q10候选源码固定在本任务`reference-study/src/componentReview/`：catalog.json记48分类/边界案与来源，model.js做纯查找/边界判断，ComponentReview.jsx及components.css提供目录、变体/状态、代表页和复用判断。入口`review=components`；复用既有Button/Find/Select/ShellFrame/TaskLedger/ProblemCards与Q09 S1，不另起业务组件库。候选白名单不等于正式CMP或新能力授权。
- 新证据目录固定为同任务`typography-review/overlay-adoption-components-15/`：状态头README/四项有界清单、before/verification、来源/指纹、浏览器交互/布局/截图、构建与治理/镜像反例日志。创建前登记，不入业务Git，不存真实敏感内容。责任为当前LIDS-ALIGN-001执行Agent；保留至Mog审阅，仅按该精确目录清理。任务README/spec AI-ENTRY及项目索引/组件晋升规则均链接此入口。
- Q10仍待Mog审阅；Q11/Q12和真实CMP资格不随S1采用批准。不改apps/crates/database/plugins、正式Token、字体资产、运行服务、全局设置，不执行API/Provider/DB，不新增提交、推送、合并或部署。

Q10渐进查询沿既有隔离`spec/tools/contract.py component Cxx`只返回一项reuseReviewCandidate；self-check附48项分类覆盖/候选边界校验。catalog是该候选分类唯一源，不反向写业务或正式CMP目录。

## ALIGN-REUSE-011 / Q11-FONT-16 · 四类组件边界采用与字体交付审阅

- U21采用Q10四类（基础复用/页面适配/领域专属/先补合同），七项晋升硬门保持。唯一机器规则源为项目`docs/design/lids/component-reuse.json`，`scripts/sync-lids-components.py`生成隔离spec.visual.componentApplication及componentReview/catalog的采用字段，第八个`reference-study/scripts/check-lids-components.mjs`在predev/prebuild校验。48项分类是复用路线，全部certified仍false，正式CMP授予0。JSON为当前规则，脚本为再生入口，入项目Git。
- Q11候选源码固定于本任务`reference-study/src/fontDeliveryReview/`：presets.json候选回退/演练输入；font-inventory.json由未改字体文件元数据/许可/哈希只读生成；model.js和useFontRun.js为测量/FontFace对象管理；FontDeliveryReview.jsx和fonts.css为可操作演练。复用当前组件、壳、账本和卡片，字体只从已存在本机静态路径载入，不复制/改写字体资产，不接业务。
- 新证据目录固定为同任务`typography-review/reuse-adoption-fonts-16/`，含状态头README/表面-状态-依赖-验收清单、before/verification、来源/字体元数据、浏览器加载/回退/字重/数字/窄容器记录与合成截图、镜像/静态注册表/构建/治理日志。创建前登记，不入业务Git，不存真实用户内容。责任为当前LIDS-ALIGN-001执行Agent，保留至Mog审阅，仅按该精确目录清理。
- 任务README/spec AI-ENTRY、项目typography/component-promotion及索引路由；spec/tools/contract.py逐C披露采用路线与未认证状态。Q11保留待审，Q12及生产字体迁移不继承批准。业务apps/crates/database/plugins、字体资产/许可、正式Token、运行服务、数据库、全局设置及合并部署均不改；演练仅本机静态字体请求，不使用CDN/Provider。


## ALIGN-FONT-012 / Q12-EXCEPTIONS-17 · 字体交付采用与材料例外审阅

- U22明确采用Q11自动换入、F2回退、普通数字右对齐/小数列按小数点对齐。唯一规则继续置于项目docs/design/lids/typography.json的families和deliveryDecision；sync-lids-typography.py生成visual.typographyApplication，第一个predev/prebuild守卫扩展校验deliveryDecision，不新造主题或修改原字体/许可。
- 隔离审阅的src/typography/DecimalValue.jsx及delivery.css为保留精度的数值定位辅助，非正式CMP；TypographyScope共享F2和已有swap字脸；fontDeliveryReview由候选改为采用状态/自动加载，旧F1/pnum/手动放行只留16证据。
- Q12候选源码登记于同任务reference-study/src/mediaMaterialReview/：presets.json记录受控例外与来源/待决冲突，MediaMaterialReview.jsx、Surfaces.jsx、materials.css为作品卡/媒体/工位合成代表页，复用A壳、B/C组合、M2控件、S1浮层及B字体。仅本地内存和内联合成比例样张，无真实媒体/平台/Provider/业务API；材料对照不授予新业务操作或CMP。作品卡M-06、媒体contain、工位M-02E与已登记工作台V8边界逐项披露，不能互相外溢。
- 新证据目录固定为同任务typography-review/font-adoption-exceptions-17/，创建前登记：README有界表面/状态/依赖/验收声明，before/source-proof、来源元数据/冲突、宽窄交互/计算样式/截图、镜像反例、构建与治理回执。任务README/spec AI-ENTRY、原型AGENTS及项目typography/materials/review-items/Agent入口按模块链接。责任为当前LIDS执行Agent，保留至Mog审阅，清理仅限该精确目录；不入业务Git，不新增提交推送合并部署，不改业务apps/crates/database/plugins、运行Token、字体资产、服务/数据库或全局。

本轮同步既有spec/tools/render.py的foundations字体交付段落，生成既有foundations.md，不新造手工字体表；ShellFrame.jsx仅修正受控导航已接入项的title，避免实际可点却提示“未接入”。


## ALIGN-MEDIA-013 / LIDS-CLOSEOUT-18 · Q12采用与全层收口核查

- U23采用Q12 E1与媒体舞台白场。唯一局部材料源登记为项目docs/design/lids/media-material.json，sync-lids-media.py生成隔离spec.visual.mediaApplication及第九个predev/prebuild守卫check-lids-media.mjs；原mediaMaterialReview的E0/测量底纹退为17历史，不改业务媒体与字体资产。
- 同任务typography-review/closeout-18/为本轮独立证据目录，创建前登记。含README有界四图、before/source-proof、全目录/样张与代表页核对、冲突/遗漏索引、浏览器状态/截图/控制台、再生/镜像反例/构建及项目治理日志；本任务Agent负责，保留至Mog审阅，仅按该精确目录清理，不存真实用户原文/秘密，不入项目Git。
- 审阅工程只修已确认标准漂移：通用页面复用既有CompositionReview的受控嵌入模式，不再并行维护早期外壳/工具组合；ComponentDemo/Modal按明确合同对齐共享控件与S1，ProblemCards源码保护；旧历史路由明确状态并指向当前入口。新审阅元数据从项目对齐源再生，不另立设计标准。
- 项目一次性报告固定docs/audits/lids-closeout-audit-2026-10-20.md，索引到docs/README及LIDS入口；待决/遗漏只在review-items单列，报告引用状态与证据，不取代规则。原型AGENTS改为当前短路由，旧交付原文移入精确本任务证据历史，不删除历史。正式LIDS与项目AGENTS/CLAUDE/codx通过模块入口渐进展开。
- 原字体/许可、业务apps/crates/database/plugins、运行Token、服务/数据库、全局配置和主线合并部署保持不动；无提交/推送/子Agent派单。规范收口不自动认证48个组件或全部业务状态。


## LIDS-UI-001 · 已采用设计进入正式前端（2026-10-20）

Mog在当前对话明确允许按建议收口色值并开始优化真实项目代码/UI；这是同一已派交付包的实施授权，本Agent单独执行，不新增并发、不改其他checkout、不合并部署。

- 专属工作树 `/Users/moglenny/proma/linggan-intelligence/.worktrees/lids-ui-implementation-001`，分支codex/lids-ui-implementation-001，取已fetch的origin/main@612dcbea。旧治理树保留，规范通过有来源的三方衔接带入，不能覆盖较新业务合同。
- 活跃计划 `docs/plans/active/lids-ui-implementation-001.md`、UI变更清单 `docs/design/changes/lids-ui-implementation-001-ui-change-manifest.md`、验收 `docs/design/acceptance/lids-ui-implementation-001-acceptance.md`；均入Git、索引docs/README，计划完成后按生命周期归档，未验证保持NOT VERIFIED。
- 当前正式颜色单源 `docs/design/lids/color.json`，按已存在tokens目标表裁定，新增color源同步/验证路径沿项目scripts；其余九源保持已确认设计。
- 生成值只进入唯一 `apps/web/src/styles/lids_tokens.css`，禁止第二套Runtime Token；可增加字体声明/许可说明和应用共享控件CSS，值从已确认源生成，按源再生。字体原文件和许可完整复制到 `apps/web/public/fonts/lids/`，来源为独立审阅的public/fonts；保留哈希/完整许可/使用标记，不裁切转换。
- 正式TypeScript/React共享改动在 `apps/web/src/ui/`、app壳/导航与已有页面模块。需要新文件时归对应模块，遵守现有CSS Modules；不创建新业务页面/功能/数据字段。必要校验用 `apps/web/src/test/`及已有测试所在模块。
- 本地证明固定于同任务 `intelligence-design-system/implementation-19/`，不入Git；含before/验证日志/实际页面截图/哈希/只读候选预览工具。预览工具只放行3000的GET/HEAD并拒绝写，不改3000运行。测试、构建和只读预览允许；数据库、后端业务、providers、工位、插件与部署禁止。

LIDS-UI-001 实际复用目录为既有 apps/web/src/shared/（Sidebar/inputMethod 与既有 ActionButton），未创建另一个 ui 组件库；页面新CSS位于 CollectionFrame/tasks。typography.css 为原字体声明与C焦点消费层，几何/值仍由唯一Token映射。字体 manifest.json/README/许可随静态资源发布；共享参数再生入口为 scripts/sync-lids-runtime.py，--check由lidsRuntime.test.ts执行。预览宽窄框架仅位于登记的 implementation-19/serve-preview.mjs，用真实构建iframe，不入业务Git。


### LIDS-UI-001 · 创作者页续修证据

固定位置：`/Users/moglenny/.codex/visualizations/2026/10/16/01a142dc-a14f-7023-b8f7-faa60edbc98b/intelligence-design-system/implementation-20-creators/`。责任当前Agent，来源为本工作树creators真实代码、现有只读4190候选与3000 GET数据；包含before哈希/基线、验证日志、实际宽窄/筛选/抽屉截图和DOM读数及README边界，不入Git。沿用implementation-19的GET/HEAD桥，所有写请求拒绝；保留至Mog验收，仅按此精确目录清理。不导出原始API/秘密；真实页面截图限本机审阅。构建仍沿已登记apps/web/dist，应用新适配器源码/CSS/测试归既有creators目录，未增加业务生成代码或第二Token。

### LIDS-UI-001 · 证据库续修证据

固定位置：`/Users/moglenny/.codex/visualizations/2026/10/16/01a142dc-a14f-7023-b8f7-faa60edbc98b/intelligence-design-system/implementation-21-evidence/`。当前Agent负责；来源本工作树真实evidence代码和4190 GET只读候选/3000 GET数据。含before哈希/基线、核对日志、宽窄/筛选/作品卡/详情/白场媒体截图与DOM几何、README边界；不入Git，不导出原始API/秘密，仅本机验收，保留至Mog验收并仅按精确目录清理。沿implementation-19只读桥，窄屏harness只扩充evidence路由白名单；不转发写请求，不触发真实采集/模型。dist沿现有登记，基本选择源码shared/SelectField不算生成物。

### LIDS-UI-001 · 两页合并与3000发布证明（Mog指定Terra）

固定位置：`/Users/moglenny/.codex/visualizations/2026/10/16/01a142dc-a14f-7023-b8f7-faa60edbc98b/intelligence-design-system/release-22-creators-evidence/`。当前Agent与Mog指定的gpt-5.6-terra子代理负责；含模型启动/执行回执、精确文件清单/commit/PR/CI、治理与测试、迁移授权与无秘密的前后状态、3000身份/浏览器截图及发布回执，不入Git。只保留最小聚合身份，不输出.env/秘密/原始请求；登记不授权迁移或真实采集/provider。仅按本精确目录清理，保留至Mog验收。原implementation工作树保留尚未发布的页面改动，发布若需独立树只放仓库.worktrees内，回执独立区分已合并与已部署。

## COLLECTION-LIDS-UI-001 私有检查产物

来源：本包构建、GET桥与浏览器检查。固定位置：`/tmp/collection-lids-ui-001/`；截图/日志/preview.mjs私有不入Git。桥拒绝非GET/HEAD。再生：前端verify/build及preview.mjs；负责人root。审阅后只清理自有临时目录。dist/node_modules沿既有登记。


## AI-GEN1 / PR-1B

私有证据固定在本工作树 `artifacts/private/ai-gen1-pr1b/`，Rust/Node、合成PG/browser、源码定位与收尾审查日志忽略、不提交；生成来源为本包实际命令，不得伪造。只创建脚本拥有的一次性PG/卷/临时目录并清理，不读取共享DSN或真实秘密。Pi依赖仍按现有精确lock在本工作树 `apps/pi-adapter/node_modules/` 安装，忽略、不手改；不借用runtime-main依赖写入。

`crates/contracts/src/ai_limits.json` 是手写的版本化运行合同源，允许入Git，不是模型能力记录或生成物；Rust/Node读取同源，SQL迁移的冻结副本用测试核对，部署脚本消费合同推导排空上限。更改上限须新合同/迁移与相关检查，不回写旧migration。

AI-GEN1 PR-1B的writing-*日志来自0140、实际SDK写作/迁移与前端类型/组件/lint/build命令，沿上述private路径保留失败与最终结果，不入Git。Web依赖在本树apps/web/node_modules按现有package-lock执行npm ci --ignore-scripts，仍忽略；dist沿既有网页构建登记，不把build当部署。旧写作fake child优先归入runner拥有的资产根，并以Drop回收；首次失败残留的精确临时目录已凭脚本/所有权/时段核验清理，未做泛化临时目录清扫。

AI-GEN1 PR-1B独立进程增量：`artifacts/private/ai-gen1-pr1b/service-*.log`由本包实际编译/owned隔离runner生成，忽略、不入Git。进程证明的ack/身份/adapter输出只放runner拥有的`ledger-assets.*`目录，终止并回收自有进程后由该runner删除；不接入共享运行目录或读取真实凭据。人工维护入口/调度/测试为源文件，按原WP3 Claim入Git。

独立AI进程proof默认使用runner的`ledger-assets.*`作为所有子进程ack/日志根；单独运行无数据库的worker单测时，只允许系统临时目录下由该用例exclusive创建的`linggan-ai-worker-proof-<UUID>`，Drop先回收自己的PID再删除精确自有目录。首次并发锁升级超时日志保留为`service-first-concurrency-failure.log`；修复后的证明覆盖4/8实际并发和轮次结算。禁止为这些proof运行会读取共享`.env`和启动开发库的旧`test-local-runtime.sh`。

AI-GEN1 PR-1B安装接续：人工维护源码为`scripts/runtime/ai_guard.py`、`scripts/runtime/ai_snapshot.py`、`scripts/runtime/drain.zsh`、`scripts/runtime/plist.zsh`、相关install/sync/launch与`scripts/runtime/tests/*.py`，允许入Git。证明日志固定`artifacts/private/ai-gen1-pr1b/install-*.log`；所有fake-repo/fake-runtime/替身命令/plist/收条只在该private目录下exclusive创建的`install-proof.<UUID>`，finally回收测试子进程并只删除自有根。真实运行时的disabled快照、AI收条与更新许可放在既有support/runtime-drain，由安装/进程原子生成，不入Git；不能将替身结果当作本机launchd或共享数据库证明。

安装接续的真实进程/PG证明由`--ai-install`同一owned runner执行；patrol证明目录优先放在runner的ledger-assets下，直接执行对应单测时只允许自建`linggan-patrol-proof-<UUID>`/`linggan-drain-proof-<UUID>`，由持有的Child guard回收。CLI控制用例的四个dummy进程用独立session创建，finally只回收该自有process group。`install-first-compile-failure.log`保留SQLx动态SQL边界适配前的编译失败；`install-local-children-first-observer-failure.log`保留dummy子进程继承observer pipe的超时，fixture重定向后证明通过。这些都是过程证据，不作为产品失败计数。Python入口禁写项目字节码，日志均忽略不提交；不运行共享dev DB的旧test-local-runtime.sh。

AI-GEN1正式设置页候选：人工维护的apps/web/src/pages/settings分区/闭集解码/交互测试及ai_foundation样本读口是入Git源码。实际React/TS/lint/build/owned浏览器输出固定private/ai-gen1-pr1b/settings-*，不入Git；浏览器只连owned loopback，素材/skill包来自自有fixture目录，不读取真实用户目录或运行模型。页面构建仍在apps/web/dist，使用原Web治理生命周期。

AI-GEN1 C0：人工维护离线合同/固定合成fixture/页面与纯合成测试/计划允许入Git；验证输出固定本独立树`artifacts/private/ai-gen1-pr1c0/`，忽略不提交，无真实材料或共享DB。CI同一浏览器证据放自有`$RUNNER_TEMP/web-app-proof/c0-offline/`，随既有Web artifact归档；截图/report/临时fixture.json不入Git。B自有target已移交本C0树target，不碰runtime-main；删除工作树前保留需要的私有日志。
