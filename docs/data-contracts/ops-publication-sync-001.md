# 运营精确发布同步数据合同（OPS-PUBLICATION-SYNC-001）

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: OPS-LAUNCH-001 的已登记发布同步命令、幂等、服务端冻结和与既有采集链的关联边界
> 事实来源: 迁移 `0151_ops_publication_sync_request.sql`、`ops_sync_api.rs`、隔离 Axum/PostgreSQL 证明；同步实施包仅为需求证据
> 冲突时以谁为准: Mog 最新确认、OPS-LAUNCH-001、真实 schema/API、采集链当前合同

## 1. 命令不是队列

`POST /api/local/ops/publication-syncs` 接收调用方的 `idempotencyKey` 和 1–200 个去重后的 `publicationRefs`。它只建立一条不可变的运营同步命令和逐条决定：

- 发起人从 active session 取得；浏览器不能提供成员、角色、业务、账号、平台、作者或链接。
- 同一个 key 只能被同一成员以同一规范化发布集合重放；不同成员或不同集合得到 `ops_sync_request_conflict`，不追加条目。
- 每条发布由服务端在同一事务中冻结当时的发布、账号、业务、平台、作品身份、账号作者身份、原生版本和 locator 指纹（可知时）。
- `accepted` / `existing` 只有同时关联真实 `collection_acquisition_request` 与 `collection_work_order` 才可能写入；运营表没有独立的排队、租约、尝试或重试状态。

当前路由受现有 Ops active-session 与同源守卫保护；团队成员可读取已持久化回执，写权限仍逐条按管理员、当前责任人或同组组长判定。

## 2. 当前决定词典

| 决定 | 当前含义 | 是否生成采集工作 |
| --- | --- | --- |
| `blocked` | 条目合法，但没有进入现有采集链所必需的身份、执行能力或桥接 | 否 |
| `forbidden` | 当前成员没有该账号的同步操作权 | 否 |
| `invalid` | 发布不存在、未分配账号或已失效 | 否 |
| `conflict` | 命令身份或冻结版本不兼容；本切片尚未产生该条目形态 | 否 |
| `accepted` | 保留给已写入既有 Request/Admission/WorkOrder 关联的条目 | 仅未来桥接实现后 |
| `existing` | 保留给同用途、同身份、同授权范围的真实在途链复用 | 仅未来桥接实现后 |

当前已明确返回的阻断码包括：

- `ops_sync_content_identity_unresolved`：登记发布仍为 `pending_match`，不能把 URL 字符串当作平台作品身份；
- `ops_sync_author_identity_unresolved`：账号没有可核验的平台作者身份；
- `ops_sync_douyin_execution_not_ready`：抖音尚无受控身份、locator、能力与回填闭环；
- `ops_sync_execution_bridge_not_ready`：XHS 身份均已冻结，但“运营精确发布”尚未接入同一 Work Order/locator/dispatch 合同。

阻断、无权和非法都保留命令条目；它们绝不表示“已提交”“等待工位”或“同步完成”。

## 3. 0151 的物理约束

`linggan_ops_publication_sync_request` 保存命令身份、actor、载荷摘要、选中数量、固定用途和提交时间；`linggan_ops_publication_sync_item` 保存逐条冻结快照和决定。两表均有数据库级 append-only trigger。

对于 `accepted` / `existing`，数据库约束要求同时具备发布/账号/业务/平台/作品/作者/版本/locator 指纹以及 existing-chain 的 Request/WorkOrder 引用。其他决定禁止携带这两个执行引用。因此无法发生“工单已建但运营关联未保存”的表面成功，也不能把 internal `publication_ref` 塞进平台内容 ID。

## 4. 未完成边界

本切片没有创建 `Acquisition Request`、`Admission`、`Work Order`、`Lease`、`Task`、`Attempt`、`Package` 或 `Receipt`；没有平台访问、媒体处理、回填投影或 React 同步按钮。它不证明 XHS 或抖音真实同步，也不授权生产迁移、部署、真实插件、真实账号或平台操作。

后续桥接必须为运营精确发布引入受控 locator/身份输入，并在同一采集链中实现回包绑定、receipt 合格判定、投影重放和两平台端到端证明；不得复用假 discovery、假材料或第二个调度队列。
