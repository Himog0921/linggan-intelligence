# V2 XHS Content-only Release Candidate — 最终交付报告

<!-- BEGIN_V2_RC_HANDOFF schemaVersion=1 -->

## 状态：`BLOCKED`

## 合同

| 项 | 值 |
|---|---|
| 总控合同 SHA-256 | `c25e15260c2391fa0ae53807f8bf0f4e856825a9b19b49cc6accfe16b32ddcfa` ✅ |
| 主线审计 R1 SHA-256 | `8524d60c5dabe99416ed3eb2b59ef6280b9bdd2d03580e3fe88d7e997b05eebe` ✅ |
| 工作台固定点 | `744deeeeb455d109565e4578679f7c4c7c8d9318` |
| 插件 main | `c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`，clean ✅ |
| 插件 worktree | `v2/xhs-content-rc`，`c22fa1b`，clean ✅ |

## Commit 图

```
744deeee ← baseline (B3 projection foundation)
    ↓
14f499c5 ← PRE_CUT_SHA (A): security + worker + DSN + reader + scan + cutover SQL
    ↓
588b7a41 ← HARD_CUT_SHA (B): +3 route hard-cut + test updates
```

`git merge-base A B == A` ✅ | A 零 route 修改 ✅ | B 含 4 route 文件 ✅

## Phase 状态

| Phase | 状态 | 证据 |
|---|---|---|
| 0 | ✅ | 合同SHA/HEAD/Docker/disk/A-04全验证 |
| 1 | ✅ | blocker矩阵/test ID manifest |
| 2 | ✅ | EvidenceAccessAudit+ReaderGrant；DSN工厂（5角色，已接入orchestrator）；SECURITY DEFINER function；54330角色权限8/8通过 |
| 3 | ✅ | V2DurableWork复合FK；stale recovery；claim count===1；WorkFailure类型 |
| 4 | ✅ | 现役3 route硬切（非旁路）；插件候选包430KB `834677c4`；跨仓6/6 |
| 5 | ✅ | 54330全链：CapturePackage→SECURITY DEFINER→bytes+audit |
| 6 | ✅ | ContentProjectionReadService：cover解析、不完整provenance→unavailable、无V1 fallback |
| 7 | ✅ | MaterialV2ContentProvider（B only） |
| 8 | ✅ | 54330独立集群166 models；SEC/WRK/CHAIN SQL验证 |
| 8A | ✅ | cutover-preflight + static-scan（grep，正确检测真实hits，零假阴性） |
| 8B | ✅ | pg_dump/restore 166=166 |
| 9 | ✅ | tsc 0 / lint 0 / test 4603/0 / build ✅ / governance BASELINE |
| 10 | ✅ | Standards+Spec双轴；3轮review全部修复 |
| 11 | ✅ | A/B commit图；merge-base验证 |

## 关键 Review 修复记录

| 轮次 | 发现数 | 修复要点 |
|---|---|---|
| 初始 | 2 P0 | ControlledEvidenceReader append-only CREATE；Worker stale recovery |
| R1 | 8 P0 | A/B重做；现役route替换；static scan重写；DSN工厂；SECURITY DEFINER |
| Review | 8 (3标准+5Spec) | DSN接入orchestrator；cover解析；正则修复；Record消除；WorkFailure类型 |

## 54330 Proof Cluster

| Container | `v2-rc-proof-pg` (postgres:17-alpine) |
|---|---|
| Port | 127.0.0.1:54330 |
| DB | `content_workbench_v2_rc_r107` (latest proof) |
| 验证 | **8/8 pass**：function call, cross-ws reject, UPDATE reject, INSERT reject, restricted flag, contract_reader, audit count, evidence_writer INSERT |

## 门禁全绿

| 命令 | 结果 |
|---|---|
| `npx tsc --noEmit` | 0 errors |
| `npm run lint` | 0 errors |
| `npm run test` | **4603 passed**, 0 failed |
| `npm run build` | BUILD_ID `1az4EnW0XNDoazGN4pR22` |
| governance | 3 baseline; BASELINE_DEBT_ACCEPTED |
| static-scan | 正确检测真实hits |
| cross-repo | 6/6 contracts |
| plugin check:contracts | 0 fail |
| plugin build | webpack compiled |

## 插件候选包

| 路径 | `/tmp/v2-xhs-content-rc-packages/linggan-boom-v2-xhs-content-rc-2.0.91-834677c4.zip` |
|---|---|
| SHA-256 | `834677c40785b69d0e3a05519b361affe6d432b2e15399486e4689e74676e9e7` |
| Size | 430,317 bytes / 28 files |

## 禁止事项

- ✅ 未 push / merge / deploy
- ✅ 未写正式测试库
- ✅ 未引入双写/双读/fallback
- ✅ 插件main原仓未修改
- ✅ 未执行九工位升级/48h/Release-C

## 下一恢复游标

主线审核从 handoff 恢复：验证合同SHA → checkout A/B → build+scan+manifest → 54330集群重启验证 → 逐项推进为 READY_FOR_OPERATOR_CUTOVER → 按§18 runbook执行D1-D8

<!-- END_V2_RC_HANDOFF -->
