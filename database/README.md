# 数据库交付边界

> 状态: 权威当前
> 最后核对: 2026-09-07
> 适用范围: 新 PostgreSQL 数据库、migration、fixture 与秘密边界
> 事实来源: ACCEPTED ADR、当前数据库目录和实际 migration
> 冲突时以谁为准: ACCEPTED ADR 与实际新项目 migration

本目录只承载 Linggan Intelligence 的全新数据库设计、migration 和脱敏 fixture。

`0001_scope_001_capture_evidence.sql` 已建立 F01 主链的最小接入侧 foundation：合成 Work/Attempt/冻结 target、Delivery、Package、Record、target result、Coverage 与 typed processing work。它不创建 Source、Content、Observation 或 Current；这些事实只由后续 F01 Record processing 切片创建。

`0002_local_001_discovery.sql` 是独立的 `LOCAL-001 / 001B-001C1` 发现面接纳切片：只追加保存已接受的 discovery Package、可见卡片的稳定平台内容身份、DiscoveryOccurrence 与 visible-card Coverage，并以数据库约束防止静默覆盖。它不把 discovery 升级为详情 Evidence、Source/Author Profile、Observation、Comment、媒体 Blob、OCR/ASR、Topic、Research、Insight 或市场结论。它必须与 `0001` 一起在独立 proof database 中验证；它不授权直接在开发库迁移或写入真实平台材料。

`./scripts/test-local-001-discovery-postgres.sh` 是 #34 的唯一 PostgreSQL proof 入口。它会先检查 Docker daemon 是否可用；不可用时明确退出，并保证没有创建 proof database/container/volume。可用时才建立随机、一次性的数据库、container 与 volume，运行 Package admission、partial/replay/rejection/append-only 与 localhost ingress → storage → Evidence Library read projection 测试，然后显式清理全部资源。它不启动 Docker、不接触开发库、旧库或真实平台。`cargo test` 的非 ignored 结果只证明编译和无数据库单元测试，不能代替这项 proof。

`./scripts/test-scope-001-postgres.sh` 每次创建随机 `linggan_intelligence_proof_<suffix>` proof database，并在独立、一次性的 PostgreSQL 16 container 与 test-only named volume 中从零执行 `0001` 和真实约束测试。成功或失败后都必须显式验证并删除该 proof database，再删除该 container 与 test-only volume；它不调用 `dev-db.sh`、不复用开发 container/volume，也不得用于开发库或旧库。它不替代后续 ingress 原子回滚、worker、Observation/Current、API/CLI 或生产证明。

该授权不证明真实 XHS producer、Raw Artifact、作者/评论/媒体、平台穷尽、趋势、AI Agent 或生产数据库设计。SCOPE-001 不创建通用 `evidence` 表、对象存储或真实平台 payload；需要这些能力时必须由后续 SCOPE 依据真实来源合同重新授权。

Gate 5 的长期数据分类、身份、版本、Current、隐私传播、统计资格和事务边界候选模型维护在 [`docs/architecture/data-architecture.md`](../docs/architecture/data-architecture.md)；候选基数与外键责任按渐进披露进入 [`docs/architecture/data-relations.md`](../docs/architecture/data-relations.md)，并发、重放、隐私传播与 PostgreSQL 验收进入 [`docs/architecture/data-consistency.md`](../docs/architecture/data-consistency.md)。这些概念文档不直接授权物理表；当前物理范围只由 [`../docs/plans/active/scope-001-content-evidence-vertical-slice.md`](../docs/plans/active/scope-001-content-evidence-vertical-slice.md) 控制。

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

## 本地开发环境

- PostgreSQL 运行在 Docker 官方 `postgres:16.14-bookworm` 中。
- 本机只绑定 `127.0.0.1:55432`，不复用旧项目端口。
- 开发库固定为 `linggan_intelligence_dev`。
- 本地初始化与 migration 管理账号为 `linggan_dev_admin`；产品运行账号以后由已确认 migration 创建。
- 数据卷固定为 `linggan-intelligence-postgres-16-data`。
- `psql` 与 `pg_restore` 通过 `./scripts/dev-db.sh` 使用，不要求额外安装本机 PostgreSQL。
- 完整步骤见 [`docs/runbooks/development-environment.md`](../docs/runbooks/development-environment.md)。

ENV-001 的 proof 数据库只用于环境验证，验证后必须删除；它不包含业务 DDL，也不是 migration baseline。

## COMMENT-DAILY-001

`0043_comment_daily.sql` 依赖 0039/0040。它新增带版本清洗、单日计划、冻结来源批次、逐条状态、分包调用引用及幂等重试命令，复用既有模型配置和 invocation 账本。数据库 trigger 禁止修改已冻结范围或删除研究执行审计；原始 Evidence 不覆盖。迁移暂停旧即时 automatic 并清除自动指针，不启用新每日计划。

当前只在 `test-model-pi-postgres.sh` 的随机隔离 PostgreSQL 验证。已登记 `local-runtime.sh migrate`，未应用共享库。新包边界与证明见 [COMMENT-DAILY-001](../docs/plans/active/comment-daily-001.md)。
