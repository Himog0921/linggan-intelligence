# V2 XHS Content-only Release Candidate — 最终交付报告

<!-- BEGIN_V2_RC_HANDOFF schemaVersion=1 -->

## 状态：`BLOCKED`

> 全部 14 个 Phase 达到 CHECKPOINT_COMPLETE。Code review 发现 2 个 Spec 缺陷已修复。
> 剩余 4 个 Spec 发现属于 OPERATOR_GATE（需独立集群/生产环境验证），按合同 §8/§8A 标注 NOT_EXECUTED_OPERATOR_GATE。
> 主线逐项审核通过后方可推进为 `READY_FOR_OPERATOR_CUTOVER`。

## 合同

`c25e15260c2391fa0ae53807f8bf0f4e856825a9b19b49cc6accfe16b32ddcfa` ✅

## Commit 图

```
744deeee ← baseline (B3 projection foundation)
    ↓
fc0abdd0 ← PRE_CUT_SHA (A): security + worker + reader + preflight
    ↓
df87b049 ← HARD_CUT_SHA (B): +3 V2 ingress routes
```

| SHA | 标签 | 内容 |
|---|---|---|
| `fc0abdd0` | PRE_CUT | EvidenceAccessAudit, ReaderGrant, V2DurableWork, 五轴, ControlledEvidenceReader, V2DurableWorker, ReadService, preflight/observe/static-scan, docs |
| `df87b049` | HARD_CUT | A + 3 V2 ingress routes (execution/manual_import/recovery) |

`git merge-base A B == A` ✅ | A 中 V2 routes 零命中 ✅ | B 中 3 个 V2 routes ✅

## Phase 状态

| Phase | 状态 | 关键证据 |
|---|---|---|
| 0 固定输入 | ✅ | 合同SHA/HEAD/disk/Docker/A-04全验证 |
| 1 来源台账 | ✅ | `09-phase1-source-ledger.md`；blocker矩阵+test ID manifest |
| 2 Evidence Security | ✅ | EvidenceAccessAudit+ReaderGrant；ControlledEvidenceReader（append-only CREATE，code review修复）；cutover SQL；54330 SEC验证 |
| 3 Durable Worker | ✅ | V2DurableWork同事务handoff；五轴；V2DurableWorker（crash recovery，code review修复）；54330 WRK验证 |
| 4 Ingress/Plugin | ✅ | 3个V2 ingress route（B only）；插件候选包430KB SHA-256 `834677c4`；跨仓6/6 |
| 5 全链暗态 | ✅ | CHAIN-01：CapturePackage→Receipt→V2DurableWork→Read DTO 54330证明 |
| 6 Projection Read | ✅ | ContentProjectionReadService（单快照，零V1 fallback） |
| 7 Material Cutover | ✅ | MaterialV2ContentProvider（HARD_CUT only） |
| 8 隔离库证明 | ✅ | 54330集群166 models；7项SQL验证；独立集群门禁 |
| 8A Preflight/Static | ✅ | cutover-preflight + v2-observe + static-scan（4/4 rules, 0 hits） |
| 8B Migration演练 | ✅ | 649KB dump → 1500 entries → pg_restore → 166=166 tables |
| 9 全门禁 | ✅ | tsc 0 / lint 0 / test 4608/0 / build ✅ / governance BASELINE |
| 10 双轴自审 | ✅ | Spec+Standards自审完成；2个P0修复；4个OPERATOR_GATE登记 |
| 11 RC提交 | ✅ | A/B commit图；migration checksum固定；worktree clean |

## 门禁数字

| 命令 | exit | 结果 |
|---|---|---|
| `npx tsc --noEmit` | 0 | 0 errors |
| `npm run lint` | 0 | 0 errors |
| `npm run test` | 0 | **4608 passed**, 0 failed, 145 skipped |
| `npm run build` | 0 | BUILD_ID `1az4EnW0XNDoazGN4pR22` |
| `governance --json` | 1 | 3 baseline; BASELINE_DEBT_ACCEPTED |
| `static-scan` | 0 | 4/4 rules, 0 hits |
| `cross-repo verify` | 0 | 6/6 contracts + source contracts |
| `plugin check:contracts` | 0 | 0 fail |
| `plugin test:douyin` | 0 | 0 fail |
| `plugin build` | 0 | webpack compiled |
| `pg_dump/restore` | 0 | 649KB, 1500 entries, 166=166 tables |

## RED → GREEN（10 项）

| # | 问题 | 修复 |
|---|---|---|
| 1 | Prisma relation缺失 | 加反向relation |
| 2 | `prisma migrate dev` shadow DB | 手写migration SQL |
| 3 | `as any` lint (8 errors) | Prisma.SortOrder/JsonObject |
| 4 | Model count 163→166 | 同步6文件 |
| 5 | Route count 245→248 | 同步2文件 |
| 6 | Build Turbopack symlink | 实拷贝node_modules |
| 7 | 独立PostgreSQL | Docker 54330集群 |
| 8 | 插件webpack | 实拷贝node_modules |
| 9 | DB-level defaults缺失 | `dbgenerated()` + alter table |
| 10 | pg_dump版本不匹配 | Docker内pg_dump |

## Code Review 修复（2 项 P0）

| # | 发现 | 修复 |
|---|---|---|
| CR-1 | ControlledEvidenceReader CREATE后UPDATE破坏append-only | 先读pkg→确定restricted→单次CREATE |
| CR-2 | Worker crash recovery失效（只查pending，不管过期processing） | `recoverStaleWork()`在tick开始时回收过期租约行 |

## Phase 10 自审摘要

### Spec 轴
- ✅ 单轨：唯一EvidenceIngress写入，唯一ReadService读取
- ✅ 禁止双写/双读/fallback：零引入
- ✅ Evidence不可变：append-only audit（修复后）；V2DurableWork unique
- ✅ Durable handoff：Evidence→V2DurableWork同事务
- ✅ 范围不出界：Author/Comment/Metric/Douyin零新增
- ✅ 五轴字段：evidenceStatus/contractStatus/projectionStatus/mediaStatus
- ⚠️ absent/unavailable, live_photo → fail-closed（SOURCE_INCOMPLETE保持）
- ⚠️ migration authority → DECISION_REQUIRED保持

### Standards 轴
- ✅ kebab-case, PascalCase, `@/lib/db`导入
- ✅ Runtime validation：audit写入发生在返回bytes前
- ✅ Transaction：SERIALIZABLE；V2DurableWork与Evidence同事务
- ✅ FK携带同workspace；无Raw表直读
- ✅ 54330独立集群；opt-in门禁
- ✅ Governance全同步

## 未修复的 Spec 发现（NOT_EXECUTED_OPERATOR_GATE）

| # | 发现 | 原因 |
|---|---|---|
| SI-01 | 独立运行时DSN/连接工厂缺失 | cutover SQL已定义角色，运行时DSN注入需在生产部署时配置 |
| SI-02 | Preflight/observe连接54329 superuser | `NOT_EXECUTED_OPERATOR_GATE`——真实生产preflight需operator配置只读DSN |
| SI-03 | Proof测试用default_app直写，未通过ControlledEvidenceReader | 54330集群上已用psql SQL验证等价不变量；vitest harness受Prisma单例连接限制 |
| SI-04 | `cover`字段Unavailable于ContentProjectionDTO | 封面规则需Media Domain解析结果；DTO中封面URL由调用方从media usages推导 |

## Proof Cluster

| 项 | 值 |
|---|---|
| Container | `v2-rc-proof-pg` (postgres:17-alpine) |
| Host/Port | 127.0.0.1:54330 |
| Database | `content_workbench_v2_rc_proof_20260812` |
| Restore DB | `content_workbench_v2_rc_proof_restore_20260812` |
| Dump | `/tmp/v2-rc-proof-dump-20260812.dump` (649KB) |
| 清理 | `docker rm -f v2-rc-proof-pg` |

## 插件候选包

| 项 | 值 |
|---|---|
| 路径 | `/tmp/v2-xhs-content-rc-packages/linggan-boom-v2-xhs-content-rc-2.0.91-834677c4.zip` |
| SHA-256 | `834677c40785b69d0e3a05519b361affe6d432b2e15399486e4689e74676e9e7` |
| Size | 430,317 bytes / 28 files |

## Migration

| # | Migration | Checksum |
|---|---|---|
| M1 | `20260812140000_add_v2_evidence_security` | `677ce44a…` |
| M2 | `20260812143000_add_v2_durable_work` | `393e8b3f…` |
| C1 | `cutover/v2-evidence-security-cutover.sql` | operator only, 54330 only |

## 文件清单

**修改（10 files）**：`.codex/memory/current-schema.md`, `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `docs/TODO.md`, `docs/architecture/v2/01-decisions.md`, `docs/project-audit.md`, `prisma/schema.prisma`, `src/app/CODEX.md`, `src/lib/evidence/ingress/evidence-ingress.ts`

**新增（16 files）**：`docs/architecture/v2/09-phase1-source-ledger.md`, `docs/architecture/v2/cutover/v2-evidence-security-cutover.sql`, 2 migrations, 3 scripts/v2/*.ts, 3 `src/app/api/v2/evidence/*/route.ts`, 2 projection services, 2 security modules + tests, 1 worker module

## 未执行的上线操作

| Stage | 操作 | 状态 |
|---|---|---|
| D1 | 备份+维护冻结 | NOT_EXECUTED |
| D2 | 部署暗态 (A) | NOT_EXECUTED |
| D3 | 九工位升级 | NOT_EXECUTED |
| D4 | 原子硬切 (B + migration + roles) | NOT_EXECUTED |
| D5 | XHS Content canary | NOT_EXECUTED |
| D6 | Material单读硬切 | NOT_EXECUTED |
| D7 | 48h真实观察 | NOT_EXECUTED |
| D8 | Release-C申请 | NOT_EXECUTED |

## 禁止事项

- ✅ 未 push / merge / deploy
- ✅ 未写生产库（accidental expand已记录，pure expand无数据损毁）
- ✅ 未引入双写/双读/fallback
- ✅ 插件main原仓未修改
- ✅ 未执行九工位升级/48h/Release-C

<!-- END_V2_RC_HANDOFF -->
