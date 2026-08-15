# Linggan Intelligence

> 状态: 权威当前
> 最后核对: 2026-08-15
> 适用范围: 项目总入口
> 事实来源: 当前仓库、ACCEPTED ADR 与实际验证结果
> 冲突时以谁为准: `AGENTS.md`、真实代码与运行证据

Linggan Intelligence 是一个以可信证据为起点的领域情报系统。

本仓库不是旧“内容工作台”的整体复制，也不是对已验证 V2 内核的盲目推倒重写。它以 Rust 和全新 PostgreSQL 数据库重新实现产品，同时把现役插件与 V2 合同作为可追溯参考资产。

## 从这里开始

1. 先阅读 [`AGENTS.md`](AGENTS.md)，确认最高约束。
2. 通过 [`docs/README.md`](docs/README.md) 进入文档地图。
3. 查看 [`docs/current-state.md`](docs/current-state.md)，只处理当前已确认事项。
4. 根据任务按需阅读产品、架构、运行手册或历史证据，不一次性加载整个 `references/`。
5. 提交变更前运行 `./scripts/check-project-governance.sh`。

## 已确认边界

- 后端与 worker 使用 Rust。
- 数据库使用全新 PostgreSQL，不带旧表、不回填伪 V2 事实。
- 浏览器插件以生产验证过的 v2.0.95 为迁移起点。
- 旧工作台与旧数据库只作只读参考。
- Evidence、Observation、Signal、Intelligence 必须能回溯到来源。
- 类型、mock 或接口返回值不能单独证明运行成功。

## 当前状态

项目仍处于业务实现前的准备阶段。当前机器的 Rust 工具链可用，但 PostgreSQL 16 客户端尚未就绪；具体以 [`docs/current-state.md`](docs/current-state.md) 和 `./scripts/new-machine-check.sh` 的实际结果为准。
