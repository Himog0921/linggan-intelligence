# 开发环境运行手册

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: macOS 本机 Rust 与 Docker PostgreSQL 16 的日常使用
> 事实来源: `rust-toolchain.toml`、`.nvmrc`、`compose.yaml` 和项目脚本
> 冲突时以谁为准: 实际脚本输出、Compose 配置和真实数据库验证

## 环境结构

```text
macOS
├── Rust 1.95.0：编译、格式化、测试
├── Node 24.13.0：只验证历史合同
└── Docker Desktop
    └── PostgreSQL 16.14（tag 与 digest 固定）
        ├── linggan_intelligence_dev
        ├── psql / pg_restore
        └── 独立持久数据卷
```

旧内容工作台数据库、旧 dump 和旧 migration 不进入这套环境。

## 第一次启动

1. 启动 Docker Desktop，等待其显示运行正常。
2. 在项目目录运行 `./scripts/setup-local-env.sh`。
3. 运行 `./scripts/dev-db.sh up`。
4. 运行 `./scripts/verify-development-environment.sh`。

成功时最后应看到 `development environment verification passed`，以及“proof 数据库已创建、写入、读取和移除”的说明。

## 日常命令

| 目的 | 命令 | 是否影响数据 |
|---|---|---|
| 启动数据库 | `./scripts/dev-db.sh up` | 保留并继续使用现有数据卷 |
| 查看状态 | `./scripts/dev-db.sh status` | 不修改数据 |
| 查看最近日志 | `./scripts/dev-db.sh logs` | 不修改数据 |
| 进入 PostgreSQL | `./scripts/dev-db.sh psql` | 是否修改取决于输入命令 |
| 查看客户端版本 | `./scripts/dev-db.sh versions` | 不修改数据 |
| 停止容器 | `./scripts/dev-db.sh stop` | 数据卷保留 |
| 停止并移除容器 | `./scripts/dev-db.sh down` | 数据卷仍保留 |
| 完整验收 | `./scripts/verify-development-environment.sh` | 创建并清理独立 proof 数据库 |
| F01 数据库 foundation proof | `./scripts/test-scope-001-postgres.sh` | 在独立一次性 PostgreSQL 16 container/volume 中创建、迁移、测试并清理随机 proof 数据库；不连接或复用开发库/container/volume |

## 数据与秘密在哪里

- `.env`：本机秘密，Git 忽略，不得复制进文档、日志或提交。
- `env.example`：无秘密的字段模板。
- Docker 数据卷：`linggan-intelligence-postgres-16-data`。
- 本机端口：`127.0.0.1:55432`，只绑定本机。
- 开发库：`linggan_intelligence_dev`。
- 本地管理账号：`linggan_dev_admin`，不等于未来产品运行账号。

## 明确禁止

- 不运行 `docker compose down -v` 或手工删除项目数据卷。
- 不把旧 dump 恢复到开发库。
- 不把密码或完整 `DATABASE_URL` 发到聊天、Issue、日志或 Git。
- 不因为容器显示“运行中”就宣布环境通过；必须运行完整验收。
- 不在 ENV-001 中创建业务表或 migration。
- 不在开发库直接运行 `database/migrations/0001_scope_001_capture_evidence.sql`；F01 foundation 只能先通过 `./scripts/test-scope-001-postgres.sh` 的随机 proof database 验证。
- 不把 `linggan_dev_admin` 当作正式环境或产品运行账号。

## 常见失败

Docker 已安装但未启动：影响是 PostgreSQL 无法运行；启动 Docker Desktop 后重新验收。

端口 `55432` 被占用：影响是新数据库不能安全绑定；先找出占用者并记录，不静默改用旧项目端口。

`.env` 已存在：脚本不会覆盖。若配置来源不明，先人工核对字段，不直接删除。
