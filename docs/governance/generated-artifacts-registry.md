# 生成型文件登记表

> 状态: 权威当前
> 最后核对: 2026-09-14
> 适用范围: 构建、代码生成、导出、日志、测试证据、备份与临时文件
> 事实来源: 当前工具配置、`.gitignore` 与来源校验清单
> 冲突时以谁为准: 生成源、工具配置和安全规则；生成结果不得反向覆盖来源

任何未列入本表的生成型文件，默认不得写入项目目录或提交 Git。确需新增时，先登记再生成。

| 类别 | 固定位置 | 来源/生成方式 | Git 策略 | 手工修改 | 保留与清理 |
|---|---|---|---|---|---|
| Pi Node 固定依赖 | `apps/pi-adapter/node_modules/`（含 `.linggan-lock-sha256`）、`package-lock.json` | 精确 package.json、npm ci、`prepare-pi-adapter.sh` | node_modules 忽略；lock 提交 | 禁止手工改 lock/安装产物 | 只清理当前 checkout 依赖，不处理其它项目 |
| COMMENT-RESEARCH-RESET-001 V1 隔离证明 | 随机 `linggan-comment-proof-*` Docker container/volume；系统临时目录下的 test log | `scripts/test-comment-research-postgres.sh`、`apps/pi-adapter` 的 Node fixture；历史 migration 后显式应用 0068–0070 | 不进入 Git | 禁止伪造 | 脚本 trap 删除它拥有的 container/volume；日志只含合成资料/失败码，不含评论正文或凭据 |
| 模型后端秘密及合成 Keychain 验证 | macOS Keychain 的 `Linggan.Intelligence.Models.<workspace UUID>` service + 随机 account | API 的 Keychain SecretStore；`model_keychain` 测试只操作随机合成条目 | 不进 Git/数据库/前端存储 | 只经配置命令更换 | 正式版本保留以供冻结任务；测试结束立即删除随机项，不枚举已有秘密 |
| CI-20260907-V1 历史隔离证明 | 已清理的临时 Docker/log 位置 | 历史交付记录；对应旧评论研究源码与再生脚本已由 V1 terminal cutover 删除 | 不进 Git | 禁止伪造或重新生成旧结果 | 仅保留文档中的历史事实；不再是可运行验证入口 |
| CI-RUN-002 请求诊断 | PostgreSQL `linggan_comment_request_trace` | 已授权研究的真实请求边界写入；输入已脱敏，返回保结构脱敏 | 不进入 Git | 禁止伪造或回填历史 | 正文最多保留 24 小时；读取独立校验有效期与来源；worker 清理过期或受限正文，元数据与安全校验摘要继续保留；关闭记录仅影响新批次 |
| 产品页面与首页 Three.js 合成原型 | `docs/design/pages/intelligence-product-prototype.html` | `python3 docs/design/pages/intelligence-product-prototype/build.py`；来源为同目录自有 JS/CSS、`docs/pages/intelligence-product-blueprint.md`、`docs/pages/intelligence-home-threejs.md`；Three.js 0.185.1 按固定 npm integrity 校验并保留 MIT 许可 | 可提交的设计参考，非 app/release；依赖 tarball 与截图只放系统临时目录 | 修改自有源码再生成；不直接修改生成 HTML | 稳定保留；设计更新时重新生成并走查；始终标记合成、无真实模型/采集 |
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
| Linggan Intelligence Browser release | `plugins/linggan-intelligence-browser/releases/linggan-intelligence-browser-v<version>.zip`、`plugins/linggan-intelligence-browser/releases/release-manifest.json` | `cd plugins/linggan-intelligence-browser && npm run build && npm run package:release && npm run release:manifest && npm run release:verify && npm run release:reproducibility`；唯一源是该包的当前 source；打包器固定 ZIP 时间与文件顺序 | 提交 | 禁止手工修改 ZIP 或 release manifest；修改 source 后重新 build/package 并核验 | `release-manifest.json` 指向唯一当前受控发行包；历史 ZIP 作为已发布工件保留，不覆盖或删除。当前 `0.8.52` ZIP SHA-256 = `7d9e0bd969b1424d06f9cfb149a81d8371b66d921233b06ce6d75e8bc7d6d270`；不含真实材料、账号、凭据或运行日志 |

## 当前登记结论

- 当前获准提交 Git 的产品生成物包括上表登记的架构沟通 HTML、产品设计合成 HTML 与 Browser Producer release。`0.8.28` 的运行时/真实标准详情证明来自工位、Package/Receipt/Materialization 与 Evidence UI，不是 ZIP 本身自证。
- `references/` 是已导入并校验的历史证据快照，不是产品运行时生成目录。
- SCOPE-001 的 `crates/contracts/tests/fixtures/capture-v1/*` 及其 expected canonical bytes/hash 是人工审查和维护的权威测试输入，不是由 Rust 或 Node checker 生成的产物；`scripts/verify-capture-fixtures.mjs` 只验证、不回写。任何将来的自动生成/回写都必须先增加本表正式登记，不得沿用这一人工输入豁免。
- 未来若引入 SQLx 离线元数据、API schema、前端构建包或机器生成 fixture，必须先补充：负责人、唯一来源、再生命令、一致性检查和是否必须入 Git。
- 临时实验输出使用操作系统临时目录；若结果需要成为项目证据，应转写为 `docs/audits/` 的可读报告，敏感原件仍留在忽略目录或外部安全存储。
- `PLUGIN-MIGRATION-001` 的 Browser Producer release 只包含 extension code、manifest 和由 Linggan runtime token source 打包的视觉 token；不得包含 Cookie、账号、真实页面材料、媒体字节、旧内容工作台运行依赖或运行日志。该发行物可被浏览器加载，不等于已经获得平台访问或实际采集授权。
- `plugins/linggan-intelligence-browser/` 是当前 Linggan-owned 唯一可发布源。发行物不得包含 Cookie、账号、真实页面材料、媒体字节、旧工作台 host/endpoint/fallback 或运行日志；旧 `plugin-retrofit-*` 与历史副本只可只读对照，不得生成当前 release。
