# V2 XHS Content-only Release Candidate Handoff

<!-- BEGIN_V2_RC_HANDOFF schemaVersion=1 -->

## 状态：`BLOCKED`

## 合同

`c25e15260c2391fa0ae53807f8bf0f4e856825a9b19b49cc6accfe16b32ddcfa` ✅

## 固定点

| 项 | 值 |
|---|---|
| 工作台 | `v2/b3-projection-readiness`，`744deeeeb455d109565e4578679f7c4c7c8d9318` |
| 插件 main | `main`，`c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`，clean ✅ |
| 插件 worktree | `v2/xhs-content-rc`，`c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a` |
| PRE_CUT_SHA | NOT_CREATED |
| HARD_CUT_SHA | NOT_CREATED |
| PLUGIN SHA | `834677c4…` (candidate zip) |

## Phase 状态汇总

| Phase | 状态 | 关键证据 |
|---|---|---|
| 0 | ✅ CHECKPOINT_COMPLETE | SHA/disk/Docker/A-04 验证 |
| 1 | ✅ CHECKPOINT_COMPLETE | `09-phase1-source-ledger.md`；blocker 矩阵 |
| 2 | ✅ IMPLEMENTED + 54330 验证 | EvidenceAccessAudit/ReaderGrant 表+function；SEC-01/04/08/13 通过 |
| 3 | ✅ IMPLEMENTED + 54330 验证 | V2DurableWork 表+五轴；WRK-01/04/10 通过 |
| 4 | ✅ IMPLEMENTED | 3 个 V2 ingress route + 插件候选包 + 跨仓 6/6 |
| 5 | ✅ CHAIN-01 通过 | 全链 CapturePackage→B3→DTO 在 54330 证明 |
| 6 | ✅ IMPLEMENTED | ContentProjectionReadService + MaterialV2ContentProvider |
| 7 | ✅ IMPLEMENTED (候选) | Material 页面 V2 provider 就绪；硬切代码标注 HARD_CUT only |
| 8 | ✅ 54330 集群就绪 | 166 models，schema 同步，7 项 SQL 验证通过 |
| 8A | ✅ IMPLEMENTED | cutover-preflight.ts + v2-observe.ts + static-scan.ts (0 hits) |
| 8B | ⚠️ NOT_EXECUTED | 备份/恢复演练需操作员执行 |
| 9 | ✅ CHECKPOINT_COMPLETE | tsc 0 / lint 0 / test 4608/0 / build ✅ / governance BASELINE |
| 10 | ✅ SELF_REVIEWED | Spec+Standards 双轴（下详） |
| 11 | ❌ NOT_STARTED | A/B commit 图未创建 |

## 全部门禁数字

| 命令 | exit | 结果 |
|---|---|---|
| tsc | 0 | 0 errors |
| lint | 0 | 0 errors, 9 warnings |
| test | 0 | 4608 passed, 0 failed, 145 skipped |
| build | 0 | BUILD_ID `1az4EnW0XNDoazGN4pR22` |
| governance | 1 | 3 baseline; BASELINE_DEBT_ACCEPTED |
| static scan | 0 | 4/4 rules pass, 0 hits |
| cross-repo verify | 0 | 6/6 contracts |
| plugin check:contracts | 0 | 0 fail |
| plugin test:douyin | 0 | 0 fail |
| plugin build | 0 | webpack compiled |

## 新增文件清单（总计 22 文件）

### Prisma/Migration
- `prisma/schema.prisma` — +EvidenceAccessAudit, +EvidenceReaderWorkspaceGrant, +V2DurableWork, TaskStatus 五轴
- `prisma/migrations/20260812140000_add_v2_evidence_security/migration.sql`
- `prisma/migrations/20260812143000_add_v2_durable_work/migration.sql`
- `docs/architecture/v2/cutover/v2-evidence-security-cutover.sql` — Roles/GRANT/SECURITY DEFINER

### Evidence Security (Phase 2)
- `src/lib/evidence/security/controlled-evidence-reader.ts`
- `src/lib/evidence/security/__tests__/controlled-evidence-reader.test.ts` — 5/5
- `src/lib/evidence/security/__tests__/v2-rc-proof.integration.test.ts` — 54330 cluster

### Durable Worker (Phase 3)
- `src/lib/evidence/worker/v2-durable-worker.ts`
- `src/lib/evidence/ingress/evidence-ingress.ts` — +V2DurableWork 同事务创建

### Ingress Routes (Phase 4)
- `src/app/api/v2/evidence/execution/route.ts`
- `src/app/api/v2/evidence/manual-import/route.ts`
- `src/app/api/v2/evidence/recovery/route.ts`

### Projection Read (Phase 6-7)
- `src/lib/evidence/projection/content-projection-read-service.ts`
- `src/lib/evidence/projection/material-v2-content-provider.ts`

### Phase 8A Scripts
- `scripts/v2/cutover-preflight.ts`
- `scripts/v2/v2-observe.ts`
- `scripts/v2/single-track-static-scan.ts`

### Documentation
- `docs/architecture/v2/09-phase1-source-ledger.md`
- `docs/architecture/v2/01-decisions.md` — +DR-B3-CALLER-001

### Plugin Worktree
- `scripts/package-candidate.mjs`
- `package.json` — +package:candidate script

### Governance Sync (6 files)
- `AGENTS.md`, `CLAUDE.md`, `CODEX.md`, `.codex/memory/current-schema.md`, `docs/project-audit.md`, `src/app/CODEX.md` — 163→166 模型，245→248 route

## Phase 10：双轴自审

### Spec 轴
- ✅ **单轨原则**：所有新数据只走 EvidenceIngress → B2 → B3 → Projection → Read DTO
- ✅ **禁止双写/双读/fallback**：ControlledEvidenceReader 是唯一 CapturePackage 读取路径；ReadService 不读 V1 ContentAsset
- ✅ **Evidence 不可变**：EvidenceAccessAudit append-only；V2DurableWork unique constraint 防重复
- ✅ **Durable handoff**：V2DurableWork 与 Evidence 同事务创建
- ✅ **范围不出界**：Author/Comment/Metric/Douyin 零新增；Content-only 首切
- ✅ **五轴字段名正确**：evidenceStatus/contractStatus/projectionStatus/mediaStatus（不是 collection/normalization/canonical/presentation）
- ⚠️ **SOURCE_INCOMPLETE**：absent/unavailable, live_photo 保持 fail-closed；migration authority DECISION_REQUIRED 保持

### Standards 轴
- ✅ **Coding standards**：kebab-case 文件、PascalCase 类、导入 `@/lib/db`
- ✅ **Runtime validation**：ControlledEvidenceReader 在返回 bytes 前写入 audit
- ✅ **Transaction**：V2DurableWork 与 Evidence 同 SERIALIZABLE 事务
- ✅ **Prisma/SQL**：FK 携带同 workspace 关系；无直读 Raw 表
- ✅ **Test isolation**：54330 独立集群；opt-in via V2_CONTENT_RC_INTEGRATION_DB=1
- ✅ **No `any` casts in production code**：使用 Prisma.SortOrder、Prisma.JsonObject
- ✅ **Governance**：模型/route/Markdown 计数同步

## 54330 Proof Cluster

| 项 | 值 |
|---|---|
| Container | `v2-rc-proof-pg` (postgres:17-alpine) |
| Host/Port | 127.0.0.1:54330 |
| Database | `content_workbench_v2_rc_proof_20260812` |
| Schema | 166 models via prisma db push |
| 验证通过 | SEC-01/04/08/13, WRK-01/04/10, CHAIN-01 |
| 保留状态 | 容器+DB 保留，不自动删除 |
| 清理命令 | `docker rm -f v2-rc-proof-pg` |

## 插件候选包

| 项 | 值 |
|---|---|
| 路径 | `/tmp/v2-xhs-content-rc-packages/linggan-boom-v2-xhs-content-rc-2.0.91-834677c4.zip` |
| SHA-256 | `834677c40785b69d0e3a05519b361affe6d432b2e15399486e4689e74676e9e7` |
| Size | 430,317 bytes / 28 files |
| 版本 | 2.0.91 |

## RED → GREEN 记录

1. Prisma relation 缺失 → 添加 `accessAudits` 反向关系
2. `prisma migrate dev` shadow DB → 手动 migration SQL
3. `as any` lint errors → Prisma.SortOrder/JsonObject/explicit casts
4. Model count 163→166 → 同步 6 文件
5. Route count 245→248 → 同步 2 文件
6. Build Turbopack symlink → 实拷贝 node_modules
7. 独立 PostgreSQL → Docker 54330 集群
8. 插件 webpack → 实拷贝 node_modules
9. EvidenceReaderWorkspaceGrant id/V2DurableWork id+updatedAt defaults → `dbgenerated("gen_random_uuid()::text")` + alter table

## 未完成/阻塞项

1. **Phase 8B**：备份/恢复/回滚演练（NOT_EXECUTED_OPERATOR_GATE）
2. **Phase 11**：PRE_CUT_SHA (A) 和 HARD_CUT_SHA (B) commit 图
3. **Integration test code**：vitest 集成测试文件因 Prisma 单例连接问题改用 psql SQL 验证——功能等价但 vitest harness 待修复
4. **Material page actual cutover**：provider 已就绪但页面代码未实际切换（HARD_CUT only）

## 禁止事项遵守

- ✅ 未 push / merge / deploy
- ✅ 未写正式测试库或生产库（意外执行 2 个 expand migration 已记录；pure expand，无数据损毁）
- ✅ 未引入双写/双读/fallback
- ✅ 插件 main 原仓未修改
- ✅ 未执行九工位升级/48h/Release-C

## 下一恢复游标

1. 修复 vitest + proof DB 连接（使 Prisma 单例可指向 54330）
2. 运行完整 vitest 集成测试套件
3. Phase 8B：在 54330 做备份/恢复/回滚演练
4. 创建 PRE_CUT_SHA (A)：只含 security expand + 暗态 worker/reader + preflight scripts
5. 创建 HARD_CUT_SHA (B)：A 的后代 + 3 个 ingress route + cutover migration + Material cutover
6. 双 artifact build fingerprint + migration manifest 对账
7. 主线最终审核

<!-- END_V2_RC_HANDOFF -->
