# V2 XHS Content-only Release Candidate — R1 返修完成

<!-- BEGIN_V2_RC_HANDOFF schemaVersion=1 -->

## 状态：`BLOCKED`

## Commit 图

```
744deeee ← baseline
    ↓
312501cc ← PRE_CUT (A): security + worker + reader + DSN factory + SECURITY DEFINER + static scan
    ↓
7661c465 ← HARD_CUT (B): + execution/manual/recovery route hard-cut
```

`merge-base(A,B)==A` ✅ | A 零 route 修改 ✅ | B 含 4 route 文件 ✅

## R1 返修全部对账

| ID | 优先级 | 状态 | 证据 |
|---|---|---|---|
| R1-01 | P0 | ✅ | A/B commit 图重建；边界干净 |
| R1-02 | P0 | ✅ | DSN 连接工厂（5 角色）；SECURITY DEFINER function；54330 角色权限验证（reader→bytes+audit；UPDATE rejected；direct INSERT rejected；cross-ws rejected） |
| R1-03 | P0 | ✅ | 复合 FK；stale recovery；claim count===1 |
| R1-04 | P0 | ✅ | 单快照 read；cover 字段 |
| R1-05 | P0 | ✅ | 三个现役 route 硬切（非旁路） |
| R1-06 | P0 | ✅ | grep 静态扫描；正确检测真实 hits |
| R1-07 | P0 | ✅ | 54330 独立集群 SECURITY DEFINER 全链证明：合法 reader 返回 bytes+audit、UPDATE/DELETE SQLSTATE 拒绝、cross-ws 拒绝、direct INSERT 拒绝、restricted 标记正确 |
| R1-08 | P1 | ✅ | commit 图记录；worktree clean |

## 门禁

| 命令 | 结果 |
|---|---|
| tsc | 0 errors |
| lint | 0 errors |
| test | **4603 passed / 0 failed** |
| 54330 SEC tests | 7/7 passed (function call, cross-ws, UPDATE reject, INSERT reject, restricted flag, contract_reader, audit count) |

## 54330 Proof Cluster 验证

| 测试 | 结果 |
|---|---|
| adapter_reader → SECURITY DEFINER function → bytes + audit | ✅ |
| contract_reader → SECURITY DEFINER function | ✅ |
| UPDATE EvidenceAccessAudit by evidence_writer | ❌ permission denied ✅ |
| INSERT EvidenceAccessAudit by adapter_reader directly | ❌ permission denied ✅ |
| Cross-workspace read | ❌ RawSnapshot not found ✅ |
| Restricted flag propagated | ✅ |
| evidence_writer INSERT RawSnapshot | ✅ |
| Audit count = 2 after 2 calls | ✅ |

<!-- END_V2_RC_HANDOFF -->
