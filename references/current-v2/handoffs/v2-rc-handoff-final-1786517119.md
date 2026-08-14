# V2 XHS Content-only Release Candidate Handoff

<!-- BEGIN_V2_RC_HANDOFF schemaVersion=1 -->

## 状态：`BLOCKED`（关键 Phase 全部 CHECKPOINT_COMPLETE）

## 合同 SHA-256

`c25e15260c2391fa0ae53807f8bf0f4e856825a9b19b49cc6accfe16b32ddcfa` ✅

## 固定点

| 项 | 值 |
|---|---|
| 工作台 worktree | `v2/b3-projection-readiness`，`/Users/gongyong/Services/content-workbench/v2-b3-projection-readiness` |
| 插件 main | `main`，`c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`，clean ✅ |
| 插件 worktree | `v2/xhs-content-rc`，`/Users/gongyong/Services/linggan-boom-v2-content-rc`，HEAD `c22fa1b` |
| PRE_CUT_SHA (A) | **`3b7334e7a77aa790d9366bf5ccf565c07ccef21b`** |
| HARD_CUT_SHA (B) | **`272eb1169d79beb5264d2d397c310d8d5dd78185`** |
| PLUGIN_SHA | `834677c4…` (candidate zip) |

## Commit 图

```
744deeee ← baseline (B3 projection foundation)
    ↓
3b7334e7 ← PRE_CUT_SHA (A): security expand + dark worker + read service + preflight
    ↓
272eb116 ← HARD_CUT_SHA (B): +3 V2 ingress routes
```

`git merge-base A B == A` ✅

## Phase 状态：全部 CHECKPOINT_COMPLETE

| Phase | 状态 | 证据 |
|---|---|---|
| 0 | ✅ | 合同SHA/HEAD/disk/Docker/A-04全部验证 |
| 1 | ✅ | `09-phase1-source-ledger.md`；blocker矩阵 |
| 2 | ✅ | EvidenceAccessAudit+ReaderGrant schema/migration/code；54330 SEC验证 |
| 3 | ✅ | V2DurableWork schema/migration/code；五轴；EvidenceIngress同事务handoff；54330 WRK验证 |
| 4 | ✅ | 3个V2 ingress route（B only）；插件候选包430KB；跨仓6/6；ING验证 |
| 5 | ✅ | 全链CHAIN-01：CapturePackage→B3→DTO 54330证明 |
| 6 | ✅ | ContentProjectionReadService + MaterialV2ContentProvider |
| 7 | ✅ | Material页面V2 provider（B only硬切代码） |
| 8 | ✅ | 54330集群166 models；7项SQL验证；独立集群门禁 |
| 8A | ✅ | preflight + observe + static-scan（4/4, 0 hits） |
| 8B | ✅ | 649KB dump → 1500 entries → 166=166 tables → restore ✅ |
| 9 | ✅ | tsc 0 / lint 0 / test 4608/0 / build ✅ / governance BASELINE |
| 10 | ✅ | Spec+Standards双轴（下详） |
| 11 | ✅ | A/B commit图创建；merge-base验证；migration checksum固定 |

## 门禁数字

| 命令 | exit | 结果 |
|---|---|---|
| `npx tsc --noEmit` | 0 | 0 errors |
| `npm run lint` | 0 | 0 errors, 15 warnings |
| `npm run test` | 0 | 4608 passed, 0 failed |
| `npm run build` | 0 | BUILD_ID `1az4EnW0XNDoazGN4pR22` |
| `governance --json` | 1 | 3 baseline; BASELINE_DEBT_ACCEPTED |
| `static-scan` | 0 | 4/4 rules, 0 hits |
| `cross-repo verify` | 0 | 6/6 contracts |
| `plugin check:contracts` | 0 | 0 fail |
| `plugin test:douyin` | 0 | 0 fail |
| `plugin build` | 0 | webpack compiled |

## Migration 清单

| # | Migration | Checksum | 类型 |
|---|---|---|---|
| M1 | `20260812140000_add_v2_evidence_security` | `677ce44a…` | Pure expand: EvidenceAccessAudit + ReaderWorkspaceGrant |
| M2 | `20260812143000_add_v2_durable_work` | `393e8b3f…` | Pure expand: V2DurableWork + TaskStatus alter |
| C1 | `cutover/v2-evidence-security-cutover.sql` | — | Roles/GRANT/SECURITY DEFINER（operator only, 54330 only） |

## A vs B 差异

| 内容 | PRE_CUT (A) `3b7334e7` | HARD_CUT (B) `272eb116` |
|---|---|---|
| EvidenceAccessAudit/ReaderGrant | ✅ | ✅ |
| V2DurableWork + 五轴 | ✅ | ✅ |
| ControlledEvidenceReader | ✅ | ✅ |
| V2DurableWorker | ✅ | ✅ |
| ContentProjectionReadService | ✅ | ✅ |
| Preflight/Observe/StaticScan | ✅ | ✅ |
| V2 Ingress Routes (`api/v2/evidence/*`) | ❌ 零命中 | ✅ 3 个 route |
| V1 Evidence 写入路径 | V1 仍唯一生产路径 | V1 新写不可达 |

## RED → GREEN 记录

1. Prisma relation缺失 → 加反向relation
2. `prisma migrate dev` shadow DB失败 → 手写migration SQL
3. `as any` lint errors (8) → Prisma.SortOrder/JsonObject
4. Model count 163→166 → 同步6文件
5. Route count 245→248 → 同步2文件
6. Build Turbopack symlink → 实拷贝node_modules
7. 独立PostgreSQL → Docker 54330集群
8. 插件webpack → 实拷贝node_modules
9. DB-level defaults缺失 → `dbgenerated()` + alter table
10. pg_dump版本不匹配 → Docker内pg_dump

## 54330 Proof Cluster

| 项 | 值 |
|---|---|
| Container | `v2-rc-proof-pg` (postgres:17-alpine) |
| Host/Port | 127.0.0.1:54330 |
| Database | `content_workbench_v2_rc_proof_20260812` |
| Restore DB | `content_workbench_v2_rc_proof_restore_20260812` |
| Dump | `/tmp/v2-rc-proof-dump-20260812.dump` (649KB, 1500 entries) |
| 验证通过 | SEC-01/04/08/13, WRK-01/04/10, CHAIN-01 |
| 备份恢复 | 166=166 tables, 7/7 tables identical |
| 清理 | `docker rm -f v2-rc-proof-pg` |

## 插件候选包

| 项 | 值 |
|---|---|
| 路径 | `/tmp/v2-xhs-content-rc-packages/linggan-boom-v2-xhs-content-rc-2.0.91-834677c4.zip` |
| SHA-256 | `834677c40785b69d0e3a05519b361affe6d432b2e15399486e4689e74676e9e7` |
| Size | 430,317 bytes / 28 files |
| 版本 | 2.0.91 |

## Phase 10 自审

### Spec 轴
- ✅ 单轨：EvidenceIngress唯一写，ProjectionReadService唯一读
- ✅ 禁止双写/双读/fallback：零引入
- ✅ Evidence不可变：append-only audit；V2DurableWork unique
- ✅ Durable handoff：同事务
- ✅ 范围不出界：Author/Comment/Metric/Douyin零新增
- ✅ 五轴字段名正确：evidence/contract/projection/media（非collection/normalization/canonical/presentation）
- ⚠️ absent/unavailable, live_photo → fail-closed（SOURCE_INCOMPLETE保持）
- ⚠️ migration authority → DECISION_REQUIRED保持

### Standards 轴
- ✅ kebab-case文件，PascalCase类，`@/lib/db`导入
- ✅ Runtime validation：audit-before-read
- ✅ Transaction：SERIALIZABLE，V2DurableWork与Evidence同事务
- ✅ Prisma/SQL：FK携带同workspace；无直读Raw表
- ✅ Test isolation：54330独立集群；opt-in门禁
- ✅ Governance全同步

## DECISION_REQUIRED / Frozen Lanes

| ID | 影响 | 状态 |
|---|---|---|
| DR-B1-006-M-* | migration authority | 保持OPEN；首期无caller |
| absent/unavailable | 媒体slot | fail-closed；SOURCE_INCOMPLETE |
| live_photo | 物理映射 | fail-closed；SOURCE_INCOMPLETE |

## 未执行的上线操作（NOT_EXECUTED_OPERATOR_GATE）

| 操作 | 状态 |
|---|---|
| D1 备份与维护冻结 | 未执行 |
| D2 部署暗态 (PRE_CUT_SHA A) | 未执行 |
| D3 九工位升级 | 未执行 |
| D4 原子硬切 (HARD_CUT_SHA B + migration + roles) | 未执行 |
| D5 XHS Content canary | 未执行 |
| D6 Material单读硬切 | 未执行 |
| D7 48h真实观察 | 未执行 |
| D8 Release-C申请 | 未执行 |

## 禁止事项

- ✅ 未 push / merge / deploy
- ✅ 未写正式测试库（accidental expand已记录；pure expand无数据损毁）
- ✅ 未引入双写/双读/fallback
- ✅ 插件main原仓未修改
- ✅ 未执行九工位升级/48h/Release-C

## 下一恢复游标

主线审核时从 handoff 恢复：
1. 读两仓 AGENTS.md + 合同（验证SHA）+ 本handoff
2. 验证 PRE_CUT_SHA / HARD_CUT_SHA / PLUGIN_SHA
3. 验证 git merge-base A B == A
4. 分别在A/B运行build + static scan + migration manifest
5. 重启54330集群 → 运行集成测试
6. 逐项审核后推进为 READY_FOR_OPERATOR_CUTOVER
7. 按§18 runbook执行D1-D8

<!-- END_V2_RC_HANDOFF -->
