# Linggan Intelligence

Linggan Intelligence 是一个以可信证据为起点的领域情报系统。

本仓库不是旧“内容工作台”的整体复制，也不是对已验证 V2 内核的盲目推倒重写。它以 Rust 和全新 PostgreSQL 数据库重新实现产品，同时把现役插件与 V2 合同作为可追溯参考资产。

## 从这里开始

1. 阅读 [`docs/context/START-HERE.md`](docs/context/START-HERE.md)。
2. 阅读 [`docs/product/PRD.md`](docs/product/PRD.md) 和 [`docs/architecture/target-architecture.md`](docs/architecture/target-architecture.md)。
3. 按 [`docs/migration/action-plan.md`](docs/migration/action-plan.md) 推进，不直接复制旧数据库结构。
4. 在安装 Rust 工具链后执行 `cargo test --workspace`。

## 已确认边界

- 后端与 worker 使用 Rust。
- 数据库使用全新 PostgreSQL，不带旧表、不回填伪 V2 事实。
- 浏览器插件以生产验证过的 v2.0.95 为迁移起点。
- 旧工作台与旧数据库只作只读参考。
- Evidence、Observation、Signal、Intelligence 必须能回溯到来源。
- 类型、mock 或接口返回值不能单独证明运行成功。

## 当前状态

这是 `bootstrap` 固定点：项目结构、上下文、源码参考快照和迁移路线已建立；业务实现尚未开始。Rust 工具链在创建本固定点的电脑上尚未安装，因此不能把此骨架描述为已编译。
