# 重建行动方案

> 状态: 活跃计划
> 最后核对: 2026-08-20
> 适用范围: 从 Bootstrap 到情报闭环的阶段路线
> 事实来源: 当前项目边界、目标架构和来源盘点
> 冲突时以谁为准: `docs/current-state.md`、ACCEPTED ADR 和真实验证结果

## Phase 0：Bootstrap（当前交付）

- 建立 Rust workspace、项目合同和来源固定点。
- 归档战略讨论、现役 V2 文档、源码参考与 handoff。
- 建立安全搬迁规则，不复制秘密、旧库和 68GB 媒体。

退出条件：新电脑能够取得同一仓库、验证来源 checksum，并理解当前与目标边界。

## Phase 0.5：项目基础设计（已完成）

- 重新确认首要用户、真实决策、产品价值和明确非目标。
- 梳理端到端用户工作流、功能地图和人工/Agent/插件责任。
- 统一领域语言、对象、状态、不变量和边界。
- 已确认正式领域语言采用 Topic Identity / Definition Version / Domain Definition Release，正式与实验词表、知识关系与导航、定义发布与分类就绪分开。
- 梳理采集、Evidence、Coverage、失败、重试和媒体链路。
- 确认概念数据模型、生命周期、版本、关系、事务和权限原则。
- 用前述结论复核 Rust 模块、API、worker、插件、存储和页面架构。
- 完成跨文档一致性审查，再决定第一条垂直切片。

退出条件：`DISC-001` 七道设计关口全部经用户确认。此阶段允许只读来源审计，不创建业务 DDL、migration 或运行接口。

完成状态：2026-08-20 用户确认 `USER-DEC-01`–`06`，七道 Gate 全部关闭。

## Phase 0.75：SCOPE-001 合成事实内核（当前）

- 只使用合成/脱敏 Content Detail fixture。
- 冻结终态 Package 接入、逐 Record 处理、部分结果 Coverage、最小 Content 身份与 Observation、字段级 Current 来源。
- 通过 API + minimal CLI 证明同一 Content 当前值、历史来源、Coverage、限制和处理状态可解释。
- 以两份追加 migration、真实 PostgreSQL 事务副作用、负向 fixture 和文件规模门禁证明范围。
- 独立对抗审查与用户实施确认完成前不创建 fixture、migration 或业务代码。

退出条件：正式 [`../plans/active/scope-001-content-evidence-vertical-slice.md`](../plans/active/scope-001-content-evidence-vertical-slice.md) 的 P0/P1 风险已裁定，用户明确授权实施，之后才进入代码阶段。

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
- 真实 PostgreSQL 故障注入：Package 接入事务不得留下“半包”；终态 Package 可以如实承载相对于 Work Order 目标的部分结果，合格 Record 不因目标未完成而丢弃。

## Phase 4：Observation Plan 与插件接入

- 新计划合同、调度、租约和安全低频策略。
- 插件保留已验证 runtime，替换提交合同而非重写全部插件。
- 单工位真实 XHS Canary 后再扩大。

## Phase 5：Change、Signal 与 Intelligence

- 先实现有时间与捕获范围边界的近期观察和候选变化；跨时间正式趋势只有在 Gate 4–5 的真实可比性与统计资格通过后，才按已证明范围实现。
- Feature/Cluster 的版本与成员变化。
- Intelligence/Brief 固定当时采用的 Claim revisions、支持、反例、缺口、适用范围与建议；不使用一个全局置信度覆盖不同判断。
- Decision、Action、结果渠道可获得性、Outcome Observation 与 Evaluation 分开；结果通过新判断或有记录修订校准后续，不直接回写旧知识。未接入的深度评论、私信、咨询、销售等保持未知；公开指标只有在真实 Action/平台对象和 producer 来源链通过审计后才能作为自动结果。

## Phase 6：新产品界面

- 每日关注与决策入口（最终页面名待 Gate 7）、Topic 深入研究、判断详情、观察计划、对象档案、证据浏览器、工位、行动结果。
- UI 通过受控查询和服务合同读取记录、定义、判断与视图；页面不成为第二事实源，不访问 legacy archive，不 fallback。
- 外部 Agent/CLI 与工作台使用同一产品合同和委托边界：第一阶段可读、可分析、可建议、可申请；任何改变正式知识、长期观察、真实采集、敏感传播或现实行动的请求必须由人授权，获准后仍走现有责任链，不能建立 Agent 特权旁路。

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
