# 新电脑启动入口

> 状态: 权威当前
> 最后核对: 2026-08-16
> 适用范围: 新机器与新 Agent 的项目背景入口
> 事实来源: Bootstrap 固定点、当前仓库和本机验证
> 冲突时以谁为准: `docs/current-state.md`、真实工具输出与 ACCEPTED ADR

## 这是什么

这是 Linggan Intelligence 的新项目固定点。它保存目标、历史来源、可复用资产和重建顺序，但不携带现役系统的秘密、生产数据或历史表债务。

## 当前事实

- 新项目技术方向：Rust + PostgreSQL。
- 数据库：全新建立，不复制旧表，不执行旧系统 153 个 migration。
- 插件迁移起点：v2.0.95，来源 commit `60a896c1def5062dbb8098e05b030c5a0871203b`。
- 工作台 V2 参考固定点：commit `2dca6cb9b08cd217d851aef845ea462c19289ef7`。
- 现役 V2 已验证 XHS Content-only 的 Evidence → B2 → Canonical → B3 Projection 链；Author、Comment、Metric、Signal、Intelligence 并未因此自动完成。
- 旧工作台约 166 个模型，历史数据库和本地媒体不进入新项目运行面。

## 在另一台电脑上的顺序

1. 安装 Git、Rustup、Node 24 和 Docker Desktop；PostgreSQL 16 由 Docker 提供。
2. 克隆或复制本仓库。
3. 核对 `references/SOURCE-MANIFEST.sha256`。
4. 阅读 `docs/decisions/0001-greenfield-rust-clean-db.md`。
5. 运行 `./scripts/setup-local-env.sh` 和 `./scripts/dev-db.sh up`。
6. 运行 `./scripts/verify-development-environment.sh`，用真实数据库副作用和 `cargo test --workspace --locked` 验收。
7. 先完成合同与字段审计，再创建第一份数据库 migration。

## 不要做

- 不要恢复旧 dump 到新项目数据库。
- 不要把 `references/` 的 TypeScript 代码当作 Rust 项目运行依赖。
- 不要把历史交付报告中的“完成”解释为新项目功能已完成。
- 不要为了快速展示页面而跳过 Evidence 和 Observation。
