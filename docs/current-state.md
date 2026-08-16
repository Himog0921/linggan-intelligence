# 当前状态与事项队列

> 状态: 权威当前
> 最后核对: 2026-08-16
> 适用范围: 当前阶段、事项顺序、阻塞与下一步
> 事实来源: 本机实际检查、已确认项目边界和完成计划
> 冲突时以谁为准: 真实运行结果、ACCEPTED ADR 与用户最新确认

## 当前阶段

项目处于“业务实现前的基础准备”。业务数据库模型、大规模 Rust 移植和生产接入均未获准开始。

已确认事实：

- GOV-001 已在 commit `73a6dd928a3be01d642efa9aa4c6c5e293c944cf` 推送至 `origin/main`。
- 当前机器已有 Git、Rust 和 Cargo。
- Docker PostgreSQL 16.14 已完成真实验证；日常是否正在运行以 `./scripts/dev-db.sh status` 为准。
- ENV-001 采用本机 Rust + Docker PostgreSQL 16，容器内 `psql` 与 `pg_restore` 已通过真实验证。
- 开发库 `linggan_intelligence_dev` 当前有 0 张业务表；临时 proof 数据库已确认删除。
- Rust workspace 仍是骨架；尚无业务 DDL、migration 或已证明的 Rust 业务链路。
- GOV-001 文件治理基线已通过验证并获用户确认，交付见 [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md)。

## 事项队列

| 顺序 | 编号 | 事项 | 状态 | 退出条件 |
|---|---|---|---|---|
| 1 | GOV-001 | 文件治理、索引、冲突与变更留痕 | 已完成 | 规则、索引、记录与自动检查随治理提交进入 `origin/main` |
| 2 | ENV-001 | Rust 与 PostgreSQL 16 开发环境 | 已完成 | 固定配置、真实数据库副作用、Rust 检查和用户确认均完成 |
| 3 | SCOPE-001 | 第一条垂直切片边界 | 待讨论 | 输入、输出、非目标和真实验收样本获确认 |
| 4 | AUD-XHS-001 | XHS 真实 producer、字段与合同审计 | 待讨论 | 来源、fixture、缺失字段和证据边界可复查 |

同一时间默认只允许一个事项处于“执行中”。状态流转为：`待讨论 → 需要决定 → 已确认 → 执行中 → 验证中 → 已完成`。来源不足使用 `SOURCE_INCOMPLETE`；必须由用户决定的边界使用 `DECISION_REQUIRED`；外部条件无法继续时使用 `BLOCKED`。

## 当前下一步

ENV-001 已获用户确认，完成计划见 [`plans/completed/env-001-development-environment.md`](plans/completed/env-001-development-environment.md)。下一事项是 SCOPE-001：确认第一条垂直切片；在边界确认前不创建业务数据库模型。

已确认边界：

1. Rust 固定 `1.95.0`，使用本机工具链。
2. PostgreSQL 固定 Docker 官方 `16.14-bookworm`，不安装本机 PostgreSQL。
3. 开发库和 proof 数据库与旧项目隔离，禁止恢复旧 dump。
4. Node 固定 `24.13.0`，只用于历史 fixture 验证。
5. `.env` 自动生成本地随机密码，禁止进入 Git。

仍需后置处理但不阻塞 ENV-001：GitHub 账号对应的提交邮箱尚未确认，记录为 `DECISION_REQUIRED`，不得猜写。
