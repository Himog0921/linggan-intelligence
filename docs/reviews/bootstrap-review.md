# Bootstrap 双轴审查

审查固定点：`ce4feb8a5a82eed4350ab2573224ec80190f7cf8`
审查范围：root commit 相对空树；历史 `references/` 只检查隔离与来源，不把旧代码标准当成新 Rust 标准。

## Standards

PASS。

- 新运行代码没有依赖旧 Prisma/ContentAsset/RawSnapshot。
- `references/` 与运行模块物理隔离，并有来源 commit 与 SHA-256 清单。
- 未提交 `.env`、数据库 dump、密钥、Cookie、runtime DSN 或本地媒体。
- 文档明确禁止假成功、fallback、类型代替运行时验证和 Evidence 覆盖。
- 脚本语法、JSON、Git whitespace 和 bootstrap checksum 验证通过。

已知验证边界：创建机器没有安装 `rustc`/`cargo`，因此 Cargo workspace 未编译。项目已经明确标注此事实，没有把骨架描述为可运行实现。

## Spec

PASS。

- 后端方向固定为 Rust。
- 新数据库边界固定为干净 PostgreSQL，不复制旧表和旧 migration。
- 六份 Linggan Intelligence 战略/能力/案例资料原件已归档。
- v2.0.95 插件完整 tracked source 已归档。
- 工作台 Evidence/B2/B3/Media/Security/Worker 相关代码、schema 参考、migration 和证明已归档。
- 相关 handoff 与用户附件已归档，并标记为历史而非现行合同。
- 已交付现役 V2 全景、目标架构、Rust 迁移地图、PRD、页面地图、行动方案和新电脑搬迁清单。

刻意未做：没有复制旧数据库 dump 和 68GB 媒体；没有在领域事实审计前猜造新数据库 DDL。

## 结论

可以作为另一台电脑的项目启动固定点。下一阶段应先安装 Rust 工具链并完成 Capture/字段/情报需求三项审计，再创建第一份 clean database baseline。
