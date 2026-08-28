# 生成型文件登记表

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: 构建、代码生成、导出、日志、测试证据、备份与临时文件
> 事实来源: 当前工具配置、`.gitignore` 与来源校验清单
> 冲突时以谁为准: 生成源、工具配置和安全规则；生成结果不得反向覆盖来源

任何未列入本表的生成型文件，默认不得写入项目目录或提交 Git。确需新增时，先登记再生成。

| 类别 | 固定位置 | 来源/生成方式 | Git 策略 | 手工修改 | 保留与清理 |
|---|---|---|---|---|---|
| Rust 构建缓存 | `target/` | Cargo build/test | 忽略 | 禁止 | 可安全重建，按需清理 |
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
| Linggan Intelligence Browser migration release | `plugins/linggan-intelligence-browser/releases/linggan-intelligence-browser-v<version>.zip`、`plugins/linggan-intelligence-browser/releases/release-manifest.json` | `cd plugins/linggan-intelligence-browser && npm run build && npm run package:release && npm run release:manifest`；唯一源是该包的迁入 source；打包器固定 ZIP 时间与文件顺序 | 提交 | 禁止手工修改 ZIP 或 release manifest；修改 source 后重新 build/package 并核验 | `PLUGIN-RETROFIT-LOCAL-TRUSTED-001` Draft adapter release；当前只保留本次可复现发行包，不含真实材料、账号或运行日志 |

## 当前登记结论

- 当前获准提交 Git 的产品生成物只有上表中的架构沟通 HTML 与 `PLUGIN-MIGRATION-001` 的 Browser Producer release；它们均不证明运行时接通、真实采集或产品业务完成。
- `references/` 是已导入并校验的历史证据快照，不是产品运行时生成目录。
- SCOPE-001 的 `crates/contracts/tests/fixtures/capture-v1/*` 及其 expected canonical bytes/hash 是人工审查和维护的权威测试输入，不是由 Rust 或 Node checker 生成的产物；`scripts/verify-capture-fixtures.mjs` 只验证、不回写。任何将来的自动生成/回写都必须先增加本表正式登记，不得沿用这一人工输入豁免。
- 未来若引入 SQLx 离线元数据、API schema、前端构建包或机器生成 fixture，必须先补充：负责人、唯一来源、再生命令、一致性检查和是否必须入 Git。
- 临时实验输出使用操作系统临时目录；若结果需要成为项目证据，应转写为 `docs/audits/` 的可读报告，敏感原件仍留在忽略目录或外部安全存储。
- `PLUGIN-MIGRATION-001` 的 Browser Producer release 只包含 extension code、manifest 和由 Linggan runtime token source 打包的视觉 token；不得包含 Cookie、账号、真实页面材料、媒体字节、旧内容工作台运行依赖或运行日志。该发行物可被浏览器加载，不等于已经获得平台访问或实际采集授权。
- `PLUGIN-REHOME-001` 的 Linggan Intelligence Browser release 是完整旧 UX/source 的 Linggan-owned 迁入基线。它同样不得包含 Cookie、账号、真实页面材料、媒体字节、旧工作台 host/endpoint/fallback 或运行日志；其 build/release 证明不等于真实平台采集、媒体本地化或 Evidence Library 数据互通。
