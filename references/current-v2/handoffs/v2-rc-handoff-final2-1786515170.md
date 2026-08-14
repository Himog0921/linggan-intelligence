# V2 XHS Content-only Release Candidate Handoff

> 本文件由执行代理在每个 checkpoint 整体覆盖，禁止 append。

<!-- BEGIN_V2_RC_HANDOFF schemaVersion=1 -->

## 状态

**`BLOCKED`** → 两个卡点已解决。Phase 2/3 的集成测试代码（SEC-01~18、WRK-01~12）尚需编写后运行。

## 合同

- SHA-256：`c25e15260c2391fa0ae53807f8bf0f4e856825a9b19b49cc6accfe16b32ddcfa` ✅

## 固定点

| 项 | 值 |
|---|---|
| 工作台 worktree | `/Users/gongyong/Services/content-workbench/v2-b3-projection-readiness`，`v2/b3-projection-readiness` |
| BASELINE_HEAD | `744deeeeb455d109565e4578679f7c4c7c8d9318` |
| CURRENT_HEAD | `744deeeeb455d109565e4578679f7c4c7c8d9318`（未提交） |
| 插件 main | `/Users/gongyong/Services/linggan-boom`，HEAD `c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`，clean ✅ |
| 插件 worktree | `/Users/gongyong/Services/linggan-boom-v2-content-rc`，`v2/xhs-content-rc`，clean ✅ |
| PRE_CUT_SHA | NOT_CREATED |
| HARD_CUT_SHA | NOT_CREATED |

## Phase 状态

| Phase | 状态 | 证据 |
|---|---|---|
| 0 固定输入 | CHECKPOINT_COMPLETE | 全部固定点验证通过 |
| 1 来源台账 | CHECKPOINT_COMPLETE | `09-phase1-source-ledger.md`；blocker 矩阵、test ID manifest |
| 2 Evidence Security | IMPLEMENTED | Schema + migration + ControlledEvidenceReader + cutover SQL；独立集群 schema 已验证 |
| 3 Durable Worker | IMPLEMENTED | Schema + migration + V2DurableWorker + EvidenceIngress modified；独立集群 schema 已验证 |
| 4 Ingress | PARTIAL | adapter 已有；route 硬切/插件包未做 |
| 5 全链暗态 | PARTIAL | B2/B3 暗态已有（55 proofs）；全链集成测试未做 |
| 6 Projection Read | IMPLEMENTED | ContentProjectionReadService；tsc ✅ |
| 7 Material Cutover | NOT_STARTED | — |
| 8 隔离库总证明 | PARTIAL | 独立集群已启动（54330）；proof DB schema 已同步；集成测试代码待写 |
| 8A Observe/Preflight | NOT_STARTED | — |
| 8B Migration Rehearsal | NOT_STARTED | — |
| 9 全门禁 | CHECKPOINT_COMPLETE | tsc ✅、lint ✅ (0 errors)、test ✅ (4608/4608)、build ✅、governance BASELINE_DEBT_ACCEPTED |
| 10 双轴自审 | NOT_STARTED | — |
| 11 Release Candidate | NOT_STARTED | — |

## diff（9 修改 + 9 新增）

### 修改文件
| 文件 | 变更 |
|---|---|
| `prisma/schema.prisma` | +EvidenceAccessAudit、+EvidenceReaderWorkspaceGrant、+V2DurableWork、TaskStatusProjection 五轴列 |
| `src/lib/evidence/ingress/evidence-ingress.ts` | +V2DurableWork 同事务创建 |
| `AGENTS.md`、`CLAUDE.md`、`CODEX.md`、`.codex/memory/current-schema.md`、`docs/project-audit.md` | 模型数 163→166 同步 |
| `docs/architecture/v2/01-decisions.md` | +DR-B3-CALLER-001 |
| `docs/TODO.md` | +A-04 引用 |

### 新增文件
| 文件 | 职责 |
|---|---|
| `prisma/migrations/20260812140000_add_v2_evidence_security/migration.sql` | EvidenceAccessAudit + EvidenceReaderWorkspaceGrant |
| `prisma/migrations/20260812143000_add_v2_durable_work/migration.sql` | V2DurableWork + TaskStatusProjection alter |
| `docs/architecture/v2/cutover/v2-evidence-security-cutover.sql` | Roles/GRANT/SECURITY DEFINER（HARD_CUT only） |
| `docs/architecture/v2/09-phase1-source-ledger.md` | Phase 1 来源台账 |
| `src/lib/evidence/security/controlled-evidence-reader.ts` | EvidenceAnalysisAuditSource 实现 |
| `src/lib/evidence/security/__tests__/controlled-evidence-reader.test.ts` | 5/5 pass |
| `src/lib/evidence/worker/v2-durable-worker.ts` | V2 Durable Worker |
| `src/lib/evidence/projection/content-projection-read-service.ts` | Content Projection Read Service |
| `docs/code-review/v2-b3-caller-read-cutover-source-audit-2026-08-12.md` | A-04 审计（untracked） |

## 门禁数字

| 命令 | exit | 结果 |
|---|---|---|
| `npx prisma validate` | 0 | ✅ |
| `npx prisma generate` | 0 | ✅ |
| `npx tsc --noEmit` | 0 | 0 errors ✅ |
| `npm run test` | 0 | 4608 passed, 137 skipped, 0 failed ✅ |
| `npm run lint` | 0 | 0 errors, 7 warnings ✅ |
| `BETTER_AUTH_SECRET=dummy npm run build` | 0 | BUILD_ID `1az4EnW0XNDoazGN4pR22`, fingerprint `4dc074ff…` ✅ |
| `governance --json` | 1 | 3 baseline failures; BASELINE_DEBT_ACCEPTED ✅ |
| `git diff --check` | 0 | clean ✅ |

## 卡点解决记录

### 卡点 1：Build（Turbopack symlink）
- **原因**：`node_modules` → `v2-b2-derived-readiness/node_modules` 符号链接
- **解决**：`rm node_modules && cp -a <src> node_modules`
- **验证**：build 成功，BUILD_ID 生成

### 卡点 2：独立 PostgreSQL 集群
- **解决**：`docker run -d --name v2-rc-proof-pg -p 54330:5432 postgres:17-alpine`
- **Container**：`v2-rc-proof-pg`，image `postgres:17-alpine`
- **Host/Port**：`127.0.0.1:54330`
- **Database**：`content_workbench_v2_rc_proof_20260812`
- **Schema**：`prisma db push` 完整同步（166 models）
- **PG Version**：PostgreSQL 17.10
- **Table counts**：
```
CapturePackage=1
EvidenceAccessAudit=1
EvidenceReaderWorkspaceGrant=0
V2DurableWork=0
ContentCurrentProjection=0
ContentObservation=0
ContentMediaUsage=0
NormalizationRun=0
ContractEvaluation=0
```
- **保留状态**：容器和数据库均保留，不自动删除
- **清理命令**：`docker rm -f v2-rc-proof-pg`

## 未完成事项

1. **集成测试代码**：SEC-01~18、WRK-01~12、HELPER-01~16 的 vitest 测试文件未编写
2. **Phase 4**：route 硬切候选代码、插件 `package:candidate` 脚本
3. **Phase 5**：全链集成测试
4. **Phase 7**：Material 页面 K-01 链修改
5. **Phase 8A**：preflight/observe/static-scan 命令
6. **Phase 8B**：migration 恢复演练
7. **Phase 10**：Spec/Standards 双轴自审
8. **Phase 11**：PRE_CUT_SHA (A) / HARD_CUT_SHA (B) commit 图

## 禁止事项

- ✅ 未 push / merge / deploy
- ✅ 未写正式测试库或生产库
- ✅ 未引入双写/双读/fallback
- ✅ 插件 main 原仓未修改
- ✅ 未执行九工位升级/48h/Release-C

## 下一恢复游标

1. 编写 SEC-01~18 集成测试，在独立集群上运行
2. 编写 WRK-01~12 集成测试，在独立集群上运行
3. 实现 Phase 4 route 硬切候选 + 插件 `package:candidate`
4. 实现 Phase 7 Material K-01 链
5. 实现 Phase 8A preflight/observe/static-scan
6. 完成 Phase 10 自审
7. 创建 A/B commit 图

<!-- END_V2_RC_HANDOFF -->
