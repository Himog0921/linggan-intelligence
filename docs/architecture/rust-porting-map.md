# TypeScript V2 → Rust 模块迁移地图

> 状态: 活跃计划
> 最后核对: 2026-08-15
> 适用范围: V2 参考能力到 Rust 模块的迁移顺序
> 事实来源: 固定 V2 源码、fixture、测试与目标架构
> 冲突时以谁为准: 真实 producer 合同、fixture 和目标 Rust 代码

| 现役参考 | Rust 目标 | 迁移方式 |
|---|---|---|
| 插件 `protocol/v2` | `crates/contracts` | 保留真实 JSON fixture，Rust runtime validate |
| `evidence/ingress` | `crates/evidence` | 重建 canonical bytes、hash、identity、append-only |
| `evidence/security` | `crates/storage-postgres` + API capability | 重建最小权限和审计，不复制旧角色 SQL |
| `evidence/worker` | `apps/worker` | 重建 claim/lease/fencing/retry，使用真实并发测试 |
| `evidence/derived` | `crates/observation` | 先实现 XHS Content normalization/evaluation |
| `evidence/projection` | `crates/observation` | 历史 Observation 与 Current view 分离 |
| `evidence/media` | `crates/observation` | Media identity、origin、slot、usage 分离 |
| Material V2 provider | `apps/api` 查询服务 | 版本化 DTO、单一致快照、无 fallback |
| 旧 Prisma schema | 只读参考 | 不移植；从已确认领域合同生成新 migration |

## 迁移单元

每个单元按以下顺序完成：

1. 固定真实 producer fixture。
2. 固定旧实现输出 bytes/hash/数据库结果。
3. 写 Rust 失败测试和 PostgreSQL 攻击测试。
4. 实现最小 Rust 模块。
5. 对照输出完全一致。
6. 才允许下一模块依赖它。

不进行逐文件、逐 class 或逐 Prisma model 翻译。
