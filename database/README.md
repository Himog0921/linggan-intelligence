# 数据库交付边界

> 状态: 权威当前
> 最后核对: 2026-08-15
> 适用范围: 新 PostgreSQL 数据库、migration、fixture 与秘密边界
> 事实来源: ACCEPTED ADR、当前数据库目录和实际 migration
> 冲突时以谁为准: ACCEPTED ADR 与实际新项目 migration

本目录只承载 Linggan Intelligence 的全新数据库设计、migration 和脱敏 fixture。

当前没有创建业务 DDL，因为 Capture Contract、Coverage、Author/Comment/Media Observation 仍需事实审计。提前建表会把讨论稿中的设想伪装为物理事实。

## 禁止进入 Git

- 旧数据库 dump。
- 生产数据、Cookie、Token、DSN。
- 本地媒体文件。
- 未脱敏的 Evidence payload。

## 旧数据库

旧数据库只恢复到独立的 `linggan_legacy_archive`，并使用只读账号。新项目代码不得连接或 fallback 到它。

## 新数据库

建议名称：

- 开发：`linggan_intelligence_dev`
- 测试：每次随机命名 `linggan_intelligence_proof_<timestamp>`
- 正式：由部署环境单独配置，不写入仓库
