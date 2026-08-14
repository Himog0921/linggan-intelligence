# 05 — 执行计划

> 本文件只允许写已完成的协作步骤和下一步方向。
> Phase 0 已启动并只允许文档契约收口；Phase 1/B1-E 的代码、schema 与数据库实施仍须先通过其直接 blocker 的隔离证明。

---

## 已完成

| 步骤 | 内容 | commit |
|---|---|---|
| G0 | V2 协作包初始化（本批 6 个文件） | `3d68ebc4` |
| G0-1 | 协作状态机、决策归属、验证矩阵与治理 blocker 收口；未触及冻结规范或运行实现 | `cf2f208b` |
| G0-2 | 固化 DEC-E01～E07 Evidence 原则；只更新协作包，未决定物理模型、DDL 或实施方案 | 本轮决策固化 |
| G0-3 | 完成 BLK-001、BLK-002 的 Evidence 边界事实审计与最小决策准备；提出 D-E08、D-E09，未确认、未变更 blocker 状态 | [决策准备包](../../progress/v2-evidence-decision-preparation-2026-08-05.md) |
| G0-4 | 用户确认 DEC-E08（RawSnapshot 唯一登记）与 DEC-E09（包级幂等、观察级追加）；BLK-001、BLK-002 进入 OPEN，未写 DDL 或实施 | 本轮确认 |
| G0-5 | 完成 BLK-001～016 的代码实施前全量来源收口：可转录合同已写入 02，全部余项合并为 DC-001～DC-009；无 blocker 获得验证状态 | [代码实施前准备审计](../../code-review/v2-precode-readiness-audit-2026-08-05.md) |
| Phase 0 | 用户确认 B1 最终业务边界：Evidence→Derived→Canonical→Projection、媒体独立、保留打开原文、本机媒体存储、Douyin 非首期、统一封面两级规则、分阶段门禁；新增运行契约 | [V2 运行契约](06-v2-operating-contract.md) |
| B2-B-01 | 七个 B2 模型 pure expand、四条三列同源 FK 与隔离库正反例完成；无 caller | DEC-B2-001 |
| B2-B-02 | XHS Normalization/ContractEvaluation 暗态单事务服务完成；受控 reader 独立审计/绑定包字节、六合同固定 hash 与新隔离库 9 项原子性/重放/漂移/撤销/并发证明通过；无 caller | DEC-B2-002 |

## 下一步

1. B2-B-02 提交后进入 B3 来源审计；在 B3 的 Canonical→Domain Projection 身份、关系和撤销语义确认前，不实现 Projection writer。
2. 先收口 Phase 1 / B1-E 的 BLK-001、002、003、010、015、016。只有每条的全部模型来源、写入边界、权限与删除规则均可逐项转录后，才能变为 `READY_FOR_PROOF`。
3. 获得单独授权后，在隔离数据库执行 `READY_FOR_PROOF` 的正反例。执行者最多推进至 `PROVED`；审核方复核实际证据后才可 `CLOSED`。
4. B1-E 证明通过后，进入 Phase 2 的 BLK-004～009、012～014，再进入 Phase 3 的 Projection 收口；封面只由中央两级规则生成，BLK-011 仅在明确的发布、审核、强一致运营页面使用。
5. Phase 4 只迁移可证明历史关系；Phase 5 等待所有 Release-B blocker `CLOSED` 与线上连续 48 小时零旧读、零旧写、零页面绕过、零非统一封面逻辑后，才删除旧结构。UND-001 仍须在 Release-B 前由用户完成九工位版本实测核验。

## 禁止

- 未通过 Phase 1 直接 blocker 的隔离证明前，不排入任何代码、schema 或数据库实施任务。
- 不自行解决 blocker。
- 不以通过 Markdown 检查、补写术语或更改状态代替 DDL、权限与反例验证。
