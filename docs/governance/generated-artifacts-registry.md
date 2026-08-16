# 生成型文件登记表

> 状态: 权威当前
> 最后核对: 2026-08-16
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

## 当前登记结论

- 当前没有获准提交 Git 的产品生成代码或构建产物。
- `references/` 是已导入并校验的历史证据快照，不是产品运行时生成目录。
- 未来若引入 SQLx 离线元数据、API schema、前端构建包或机器生成 fixture，必须先补充：负责人、唯一来源、再生命令、一致性检查和是否必须入 Git。
- 临时实验输出使用操作系统临时目录；若结果需要成为项目证据，应转写为 `docs/audits/` 的可读报告，敏感原件仍留在忽略目录或外部安全存储。
