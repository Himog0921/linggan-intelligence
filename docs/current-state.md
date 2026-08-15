# 当前状态与事项队列

> 状态: 权威当前
> 最后核对: 2026-08-15
> 适用范围: 当前阶段、事项顺序、阻塞与下一步
> 事实来源: 本机实际检查、已确认项目边界和完成计划
> 冲突时以谁为准: 真实运行结果、ACCEPTED ADR 与用户最新确认

## 当前阶段

项目处于“业务实现前的基础准备”。业务数据库模型、大规模 Rust 移植和生产接入均未获准开始。

已确认事实：

- 本轮开始时 HEAD 已核对为 `8e4fc2f49c29165ae2c7cf80c14daaa7a826c27c`；当前工作树已有未提交治理改动，不能把该固定点等同于完整的当前文件状态。
- 当前机器已有 Git、Rust 和 Cargo。
- PostgreSQL 16 的 `psql` 与 `pg_restore` 尚未通过 `./scripts/new-machine-check.sh`。
- Rust workspace 仍是骨架；尚无业务 DDL、migration 或已证明的 Rust 业务链路。
- GOV-001 文件治理基线已通过验证并获用户确认，交付见 [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md)。

## 事项队列

| 顺序 | 编号 | 事项 | 状态 | 退出条件 |
|---|---|---|---|---|
| 1 | GOV-001 | 文件治理、索引、冲突与变更留痕 | 已完成 | 规则、索引、记录与自动检查随治理提交进入 `origin/main` |
| 2 | ENV-001 | Rust 与 PostgreSQL 16 开发环境 | 待讨论 | 工具版本、数据库方式、密钥边界和可重复验证获确认 |
| 3 | SCOPE-001 | 第一条垂直切片边界 | 待讨论 | 输入、输出、非目标和真实验收样本获确认 |
| 4 | AUD-XHS-001 | XHS 真实 producer、字段与合同审计 | 待讨论 | 来源、fixture、缺失字段和证据边界可复查 |

同一时间默认只允许一个事项处于“执行中”。状态流转为：`待讨论 → 需要决定 → 已确认 → 执行中 → 验证中 → 已完成`。来源不足使用 `SOURCE_INCOMPLETE`；必须由用户决定的边界使用 `DECISION_REQUIRED`；外部条件无法继续时使用 `BLOCKED`。

## 当前下一步

下一事项是 ENV-001。先讨论并固定环境边界，再安装或修改配置；在 ENV-001 边界确认前不安装 PostgreSQL、不修改 Rust 工具链版本、不创建业务数据库模型。

ENV-001 首轮只讨论并记录：

1. Rust 版本固定、格式化和静态检查标准。
2. PostgreSQL 16 使用本机服务还是容器。
3. 开发库、证明库、备份与销毁边界。
4. Node 仅用于历史 fixture 验证时是否固定版本。
5. 本地秘密、连接配置和不可入 Git 文件的存放方式。
