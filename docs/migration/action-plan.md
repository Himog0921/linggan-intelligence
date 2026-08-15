# 重建行动方案

> 状态: 活跃计划
> 最后核对: 2026-08-15
> 适用范围: 从 Bootstrap 到情报闭环的阶段路线
> 事实来源: 当前项目边界、目标架构和来源盘点
> 冲突时以谁为准: `docs/current-state.md`、ACCEPTED ADR 和真实验证结果

## Phase 0：Bootstrap（当前交付）

- 建立 Rust workspace、项目合同和来源固定点。
- 归档战略讨论、现役 V2 文档、源码参考与 handoff。
- 建立安全搬迁规则，不复制秘密、旧库和 68GB 媒体。

退出条件：新电脑能够取得同一仓库、验证来源 checksum，并理解当前与目标边界。

## Phase 1：事实审计

- 插件六种 XHS profile 的实际字段、终态、coverage、媒体来源审计。
- 现役 V2 contract/fixture 逐项转录。
- 情报能力到字段的反向映射。
- 输出 `Capture Contract v1`，禁止凭旧模型猜字段。

## Phase 2：Rust Evidence Kernel

- 运行时 validator、canonical JSON、hash、幂等 identity。
- CapturePackage、Evidence、Coverage、Artifact。
- PostgreSQL 角色、append-only、访问审计。
- 与 TypeScript fixture 字节级对照。

## Phase 3：Observation Kernel

- Content、Author、Comment、Media Observation。
- Current pointer 与历史版本分离。
- accepted/rejected、并发、replay、撤销和媒体多 slot。
- 真实 PostgreSQL 故障注入，禁止部分成功。

## Phase 4：Observation Plan 与插件接入

- 新计划合同、调度、租约和安全低频策略。
- 插件保留已验证 runtime，替换提交合同而非重写全部插件。
- 单工位真实 XHS Canary 后再扩大。

## Phase 5：Change、Signal 与 Intelligence

- 跨时间变化计算。
- Feature/Cluster 的版本与成员变化。
- Intelligence Event 包含证据、反例、缺口、置信度、建议。
- Outcome 回写形成判断校准。

## Phase 6：新产品界面

- 情报收件箱、情报详情、观察计划、对象档案、证据浏览器、工位、行动结果。
- UI 只读新 Projection，不访问 legacy archive，不 fallback。

## Phase 7：真实数据运行与旧系统退役

- 新数据只进入新系统，不双写。
- 高价值历史对象重新采集。
- 旧数据库和媒体保持只读归档。
- 当新项目覆盖真实工作流后，再独立决定旧系统关闭与删除。

## 每阶段强制证明

- 真实 producer fixture。
- 正常路径和攻击性负例。
- 新建隔离 PostgreSQL 数据库。
- 运行结果、数据库副作用、输入输出 hash。
- 不允许用 mock、类型或接口 `ok:true` 替代完整链路证明。
