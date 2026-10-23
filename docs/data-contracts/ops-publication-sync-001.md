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

XHS 原生发布可先通过 `POST /api/local/ops/native/publications/{publicationRef}/execution-locators` 追加一条受控的短期执行来源。该命令只接受包含 24 位小写作品 ID 和非空 `xsec_token` 的完整 XHS 地址，并在同一事务冻结账号的当前作者 ID、发布版本、作品 ID、地址摘要和实际提交人。签名地址只保存在受保护的来源表，绝不出现在命令或同步回执中；短链、裸详情地址、抖音地址、无作者身份或无写权限均不被接受。它不是 discovery、CapturePackage、material、授权或任务。

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
- `ops_sync_execution_locator_unavailable`：作品/作者身份可知，但还没有与当前发布版本匹配的受控 XHS 执行来源。

阻断、无权和非法都保留命令条目；它们绝不表示“已提交”“等待工位”或“同步完成”。

## 3. 0151 的物理约束

`linggan_ops_publication_sync_request` 保存命令身份、actor、载荷摘要、选中数量、固定用途和提交时间；`linggan_ops_publication_sync_item` 保存逐条冻结快照和决定。两表均有数据库级 append-only trigger。

`0152_ops_publication_execution_locator` 追加 `linggan_ops_publication_execution_locator`：每行都绑定发布、账号、XHS 作品/作者身份、原生发布版本、原始签名地址及其 SHA-256、提交人和验证状态，且数据库禁止更新或删除。同一发布与同一地址摘要不会产生第二条来源；新 token 是另一条输入事实，后续 resolver 必须按当前发布版本选择，不能回退到已经失效或属于另一作品的地址。

`0155_ops_exact_publication_work_scope` 已新增 `collection_work_order_ops_publication_target`。它是一个 Work Order 的不可变、逐条精确发布范围：只绑定发布、发布版本、平台作品/作者身份与受保护 locator 的引用；它明确只允许详情页读取，评论、回复、媒体、OCR 与 ASR 均为零/false。表不能容纳 material、discovery finding、package 或原始签名 URL，也不为既有 Authorization 默认追加 `ops_publication_sync` 模板——没有这份显式授权的同步仍必须阻断。

`0156_ops_exact_publication_detail_session` 使既有详情页会话在“material / 跨行业样本 / 精确运营发布”三种互斥来源中准确绑定一种。精确发布会话只在租约中的同一发布范围和同一 locator 仍可解析时授权；它不能借 material 的会话唯一键、也不能用相同作品 ID 匹配到别的 Work Order。

对于 `accepted` / `existing`，数据库约束要求同时具备发布/账号/业务/平台/作品/作者/版本/locator 指纹以及 existing-chain 的 Request/WorkOrder 引用。其他决定禁止携带这两个执行引用。因此无法发生“工单已建但运营关联未保存”的表面成功，也不能把 internal `publication_ref` 塞进平台内容 ID。

## 4. 未完成边界

本切片仍没有由同步 API 创建 `Acquisition Request`、`Admission`、`Work Order`、`Lease`、`Task`、`Attempt`、`Package` 或 `Receipt`；没有平台访问、媒体处理、回填投影或 React 同步按钮。`0155/0156` 及租约/派发/详情会话读取面已经让未来 Work Order 可把精确范围展开为既有 `content_detail` task，并且只解析该工单绑定的 locator，绝不回退到同名材料的 discovery 链接；它尚未接入 Collection 准入、资格停止或回执投影，且同步 API 还不写此范围。因此 XHS 命令的当前结果仍是 `ops_sync_execution_bridge_not_ready`。抖音仍没有受控 locator、身份、能力与回填闭环。它不证明 XHS 或抖音真实同步，也不授权生产迁移、部署、真实插件、真实账号或平台操作。

后续桥接必须为运营精确发布引入受控 locator/身份输入，并在同一采集链中实现回包绑定、receipt 合格判定、投影重放和两平台端到端证明；不得复用假 discovery、假材料或第二个调度队列。
