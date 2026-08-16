# ENV-001 开发环境基线

> 状态: 已完成计划
> 最后核对: 2026-08-16
> 适用范围: Rust、Node、Docker PostgreSQL 16、本地秘密与环境验收
> 事实来源: 本机实际工具状态、官方 PostgreSQL 镜像和用户确认
> 冲突时以谁为准: 实际工具输出、锁定配置和真实数据库验证

## 用户可见结果

Agent 在新机器克隆仓库后，只需启动 Docker 并运行项目脚本，即可获得与旧项目隔离的 PostgreSQL 16 环境。环境成功必须由 Rust 检查和真实数据库写入共同证明。

## 已确认方案

- Rust 在 macOS 本机运行，固定 `1.95.0`，包含 `rustfmt` 和 `clippy`。
- PostgreSQL 使用 Docker 官方 `postgres:16.14-bookworm`，并在 Compose 中固定已验证镜像 digest；不额外安装本机服务端或客户端。
- Docker 内同时提供 `psql`、`pg_restore` 和 `pg_isready`。
- 开发库固定为 `linggan_intelligence_dev`，映射到本机 `127.0.0.1:55432`。
- 数据保存在独立卷 `linggan-intelligence-postgres-16-data`。
- Node 固定 `24.13.0`，仅用于历史 fixture/合同验证，不进入 Rust 产品运行时。
- `.env` 由脚本生成随机密码并被 Git 忽略；`env.example` 只保存占位值。
- Docker 初始化账号固定为 `linggan_dev_admin`，只用于本地环境管理和未来 migration；产品运行账号必须由后续已确认 migration 单独创建。
- 不提供自动删除数据卷或恢复旧数据库的快捷命令。

## 范围

1. 固定工具版本和配置。
2. 提供启动、停止、状态、日志、`psql` 和 `pg_restore` 入口。
3. 提供真实环境验收脚本。
4. 建立可重复运行的开发手册。
5. 生成并登记 `Cargo.lock`。

## 非目标

- 不创建业务表、业务 migration 或 Rust 数据访问模型。
- 不提前创建或伪装产品运行数据库角色。
- 不恢复旧 dump，不连接旧数据库。
- 不设计生产部署、云数据库、备份周期或灾备。
- 不把 Node/TypeScript 作为新产品运行时。
- 不执行数据库数据卷删除。

## 执行与可证伪验收

1. 固定配置 -> 验证：版本文件和 Compose 配置可解析。
2. 启动数据库 -> 验证：容器健康且服务端版本属于 PostgreSQL 16。
3. 检查工具 -> 验证：容器内 `psql`、`pg_restore` 均为 16。
4. 证明真实写入 -> 验证：创建独立 proof 数据库，建表、写入、读取后精确删除。
5. 证明 Rust 环境 -> 验证：格式化、Clippy 和 workspace tests 在 `--locked` 下通过。
6. 证明隔离 -> 验证：开发库不存在旧 Prisma migration ledger，历史 `references/` checksum 不变。

## 风险和停止点

- Docker Desktop 未运行时停止，不改用另一套本机 PostgreSQL。
- `55432` 被占用时停止并记录冲突，不静默换端口。
- 已存在 `.env` 时不覆盖。
- 已存在同名数据卷但来源不明时，先盘点再继续。
- 任何 proof 数据库清理失败都不能报告完整成功。

## 实际验证结果

- Docker Desktop `28.5.1`、Docker Compose `2.40.3` 正常运行。
- Rust `1.95.0`、Cargo `1.95.0`、rustfmt 和 Clippy 可用。
- Node `24.13.0` 与 `.nvmrc` 一致。
- PostgreSQL 服务端 `server_version_num=160014`；`psql`、`pg_restore` 均为 `16.14`。
- 官方镜像固定为 `postgres:16.14-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8`。
- proof 数据库已真实创建、建表、写入、读取并删除；删除后再次查询确认不存在。
- 开发库只剩 `linggan_intelligence_dev`，`public` schema 当前为 0 张表，无 `_prisma_migrations`。
- 本地数据库所有者为 `linggan_dev_admin`，未创建产品运行账号 `linggan_app`。
- `cargo fmt --check`、Clippy `-D warnings`、`cargo test --workspace --locked` 全部通过；现有 2 个 Rust 单元测试通过、0 失败。
- `.env` 权限为仅本机用户可读写，Git 忽略检查通过；`references/` 未修改。
- Bootstrap 秘密检查已从“禁止本机存在 `.env`”校准为“禁止任何私密文件对 Git 可见”，与本地开发秘密策略一致。

用户于 2026-08-16 确认 ENV-001；本计划随环境提交进入 `origin/main` 后完成团队收口。
