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

`GET /api/local/ops/publication-syncs/{requestRef}` 保留原始逐条 `decision`，并另行读取共享链的当前执行事实 `execution`。`queued`、`waiting_execution`、`collecting`、`receipt_lost_authority`、`no_qualified_result` 与 `material_projected` 不会改写原始决定；尤其 `material_projected` 只表示不可变的 Receipt→Material 关联已写入，不能表示运营字段已经采用、媒体已经可用或整条同步已完成。

XHS 原生发布可先通过 `POST /api/local/ops/native/publications/{publicationRef}/execution-locators` 追加一条受控的短期执行来源。该命令只接受包含 24 位小写作品 ID 和非空 `xsec_token` 的完整 XHS 地址，并在同一事务冻结账号的当前作者 ID、发布版本、作品 ID、地址摘要和实际提交人。签名地址只保存在受保护的来源表，绝不出现在命令或同步回执中；短链、裸详情地址、抖音地址、无作者身份或无写权限均不被接受。它不是 discovery、CapturePackage、material、授权或任务。

当前路由受现有 Ops active-session 与同源守卫保护；团队成员可读取已持久化回执，写权限仍逐条按管理员、当前责任人或同组组长判定。

## 2. 当前决定词典

| 决定 | 当前含义 | 是否生成采集工作 |
| --- | --- | --- |
| `blocked` | 条目合法但未满足身份、受控 locator、有效精确授权或其他既有链前提；会保留拒绝决定 | 否 |
| `forbidden` | 当前成员没有该账号的同步操作权 | 否 |
| `invalid` | 发布不存在、未分配账号或已失效 | 否 |
| `conflict` | 命令身份或冻结版本不兼容；本切片尚未产生该条目形态 | 否 |
| `accepted` | 同一事务已写入真实 Request、Admission、WorkOrder 与精确发布 scope | 是，写入既有共享队列，尚未执行 |
| `existing` | 同一发布版本和 locator 的兼容 WorkOrder 仍在未认领队列，本次 Request 合并进去 | 是，复用既有共享队列项，尚未执行 |

当前已明确返回的阻断码包括：

- `ops_sync_content_identity_unresolved`：登记发布仍为 `pending_match`，不能把 URL 字符串当作平台作品身份；
- `ops_sync_author_identity_unresolved`：账号没有可核验的平台作者身份；
- `ops_sync_douyin_execution_not_ready`：抖音尚无受控身份、locator、能力与回填闭环；
- `ops_sync_execution_locator_unavailable`：作品/作者身份可知，但还没有与当前发布版本匹配的受控 XHS 执行来源。
- `authorization_missing` / `authorization_scope_mismatch` / `authorization_work_unit_limit_reached`：来源已冻结，但不存在覆盖 `ops_publication_sync`、`batch`、一篇详情读的有效 Collection Authorization；不会写 WorkOrder。

阻断、无权和非法都保留命令条目；它们绝不表示“已提交”“等待工位”或“同步完成”。

## 3. 0151 的物理约束

`linggan_ops_publication_sync_request` 保存命令身份、actor、载荷摘要、选中数量、固定用途和提交时间；`linggan_ops_publication_sync_item` 保存逐条冻结快照和决定。两表均有数据库级 append-only trigger。

`0152_ops_publication_execution_locator` 追加 `linggan_ops_publication_execution_locator`：每行都绑定发布、账号、XHS 作品/作者身份、原生发布版本、原始签名地址及其 SHA-256、提交人和验证状态，且数据库禁止更新或删除。同一发布与同一地址摘要不会产生第二条来源；新 token 是另一条输入事实，后续 resolver 必须按当前发布版本选择，不能回退到已经失效或属于另一作品的地址。

`0155_ops_exact_publication_work_scope` 已新增 `collection_work_order_ops_publication_target`。它是一个 Work Order 的不可变、逐条精确发布范围：只绑定发布、发布版本、平台作品/作者身份与受保护 locator 的引用；它不能容纳 material、discovery finding、package 或原始签名 URL，也不为既有 Authorization 默认追加 `ops_publication_sync` 模板——没有这份显式授权的同步仍必须阻断。0155 时代已形成的 scope 保持详情-only（评论、回复、媒体、OCR、ASR 均为零/false）；`0160_ops_publication_media_projection` 只让**新建**的精确 scope 追加同页 `media_slots`，仍保持评论、回复、OCR 与 ASR 为零。历史 scope 不会因升级被放宽。

`0156_ops_exact_publication_detail_session` 使既有详情页会话在“material / 跨行业样本 / 精确运营发布”三种互斥来源中准确绑定一种。精确发布会话只在租约中的同一发布范围和同一 locator 仍可解析时授权；它不能借 material 的会话唯一键、也不能用相同作品 ID 匹配到别的 Work Order。

`0157_ops_publication_execution_input_eligibility` 将精确运营发布纳入既有执行输入资格台账。若其绑定 locator 此后被判为不可用，系统按 publication 身份记录一次 `input_blocked`，收束同一发布待执行的 lane，并在重新发租时跳过它；不会释放整单并无止境地重新排队。它不把 publication 伪装成 material，也不把 URL 变化自动当作扩大采集范围的授权。

`0158_ops_publication_material_projection` 只保存合格回包与运营发布之间的追加式关联：`publication/version → request/work order → task/attempt → package/receipt → record ordinal → Material`。运行时先写入 Receipt，再在同一事务中检查所有这些边界。只有 scheduled 精确 scope 的 `content_detail`、`COMPLETED_LIVE_STEP`、`ACCEPTED` receipt、未超出该单一详情配额的 `accepted_for_library_content` record、匹配的平台作品 ID 与作者 ID 才能创建该关联；回包缺字段、身份不符、record 被隔离或 `LOST_AUTHORITY` 时均不会投影。详情包没有任何合格 record 时也不会仅凭 package 创建 `linggan_material_content`。

该投影从不更新 `linggan_ops_publication`，所以手工阅读量/CTR、日报、历史发布人、当前负责人、业务归属和原发布字段不在此写入范围。它也不写 `linggan_material_domain_usage`：领域 primary/reference 不是“我方内容”含义，不能为了把运营同步展示在某个领域而重写既有领域用途。投影本身是不可变且以 Receipt 为幂等键；字段采用、媒体状态与受控重放必须消费这条真实链，不得重新访问平台或伪造回执。

`0159_ops_publication_field_adoption` 使登记导入、原生登记和合格 Receipt 都以追加式逐字段决定形成一个统一读取模型，而非修改注册行或在页面回退主表。导入基线有 import batch，原生明确填写的标题/正文有原生命令回执，Receipt 值有 package/record、对应 publication identity version 和观测时刻；每项均保存 `adopted`、`not_provided` 或 `conflict` disposition。当前列表按观测时刻读取身份版本仍匹配的最新决定；未知字段绝不补零，也不会回退旧主表字段。字段值只在 `adopted` 时显示；每个字段的 disposition 同时输出，故页面会把 `conflict` 明确标为“待核实”，不会和 `not_provided` 混为一个空单元格。`published_at` 若为空记为 `not_provided`，若与原生登记的北京业务日冲突则保留 `conflict`，不会静默改变运营报表日期。该阶段不采用阅读量、CTR、日报、历史/当前责任人或业务字段，也不提供人工字段修订命令、冲突裁决或投影-only 重放。

`0160_ops_publication_media_projection` 使已批准同页的 `media_slots` 仍沿既有 Task/Attempt/Package/Receipt 和统一媒体获取链运行：只有 scheduled 精确 scope、`COMPLETED_LIVE_STEP`、`ACCEPTED` Receipt、当前 publication identity version、正确平台/作品 Task target，以及 `accepted_for_media_identity` record 才会追加 `publication/version → request/work order → task/attempt → package/receipt/record → observation/slot` 关联。它既不保存远程 URL，也不另建下载器。列表状态只从该关联再读取统一媒体链：`waiting_for_slots`、`processing`、`partial`、`available`、`failed`、`unavailable` 与 `not_requested` 分开表达；`available` 只在未被处置的本地 materialization 已验证时成立，已处置资源显示为 `unavailable` 而非伪装成处理中。下载失败或处置不会删除历史 observation，也不会清除已经可读的字段或历史封面。

对于 `accepted` / `existing`，数据库约束要求同时具备发布/账号/业务/平台/作品/作者/版本/locator 指纹以及 existing-chain 的 Request/WorkOrder 引用。其他决定禁止携带这两个执行引用。因此无法发生“工单已建但运营关联未保存”的表面成功，也不能把 internal `publication_ref` 塞进平台内容 ID。

## 4. 已接入与未完成边界

同步 API 现会在同一数据库事务内，对具备 XHS 冻结身份与 locator 的条目调用精确发布准入：它创建或复用仅作控制身份的 creator Target（不等于发现、归档或 Material），冻结发布所属 active Domain 的 primary/reference 关系，写入 `Acquisition Request → Admission → WorkOrder → collection_work_order_ops_publication_target`，再写回同步条目的两个 existing-chain 引用。唯一可接受的授权必须明确包含 `ops_publication_sync` 模板、`batch` lane、至少一篇作品和一个工作单元；旧授权不会被自动扩大。新 scope 固定一篇详情与同页 `media_slots`，但评论/回复/OCR/ASR 均为零；WorkOrder 的 `max_works=1`，且 lease 发放会再次检查这份精确授权仍有效。

`0155–0160` 及租约/派发/详情会话/缺输入停止读取面现在可在隔离 PostgreSQL 中走通一条合成的 XHS 控制链：精确 scope 被合格安装领取，真实调度代码依序生成 scheduled `content_detail` 与 `media_slots` task；合格详情 Receipt 创建或复用 Material、追加发布—Material 投影和字段采用决定，合格媒体 Receipt 追加发布—observation/slot 投影，随后只有统一媒体链的本地物化才能将列表状态升为 `available`。此证明不访问平台，也不能证明真实 XHS 内容、浏览器插件、生产迁移、部署或业务验收。抖音仍没有受控 locator、身份、能力与回填闭环。

运营“发布记录”页面现在可从“同步数据”打开一个仅列出当前页记录的显式选择框：未选择时不能提交，提交后逐条显示原始决定与当前 `execution`，并可按原 `requestRef` 刷新读取。列表读取还会为每条发布返回其**最近一次**持久化同步条目的 `requestRef`、原始 `decision` 和共享链实时状态；关闭对话框、刷新页面或重新进入后仍可从该 `requestRef` 重开同一回执，绝不把浏览器瞬态提示当作任务状态。列表与详情另行显示媒体状态及已观察/本地可用/失败数量，且不把历史导入封面、远程 URI 或 `material_projected` 当作“媒体已可用”。当前未提供媒体重试命令、人工字段采用/冲突处理或投影-only 重放；字段冲突虽可见，但页面不能静默裁决或覆盖。页面交互已由前端测试覆盖，但并非真实平台、插件、生产迁移、部署或业务验收。

后续工作仍是媒体失败后的受控重试入口、人工字段采用与冲突处理、受控“只重放投影”动作，并完成两平台端到端证明；不得复用假 discovery、假材料或第二个调度队列。
