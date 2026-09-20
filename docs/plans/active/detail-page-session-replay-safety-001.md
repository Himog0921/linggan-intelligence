# DETAIL-PAGE-SESSION-REPLAY-SAFETY-001

> 状态: 活跃计划
> 最后核对: 2026-09-21（签名详情 URL 明确失效收口候选）
> 适用范围: XHS 详情页的派发、浏览器导航、页面结果暂存与本机交付重传
> 事实来源: 2026-09-20 的真实运行审计、当前 `dispatch.rs`/Browser Producer 实现，以及 Mog 确认的“最终执行策略”
> 冲突时以谁为准: Mog 的最新确认、运行时事实、受保护交付规则

## 要解决什么

同一 Work Order 的 `in_progress` task 会被派发端重放，以处理“claim 响应已提交但客户端没有收到”的网络窗口。现有 Browser Producer 仅在详情页缓存存在时避免再次开页；缓存缺失、后台重启或原页面尚未回传时会再次执行 `openTaskWindow`。这会造成同一篇笔记被重复打开，触发平台风控。

本包只收束“同一工作单、同一篇内容的自动重开页”问题，并在已观察到连续平台风险拦截页时保护报告该事实的插件安装；不扩大真实采集范围，不改变巡检频率、账号策略或历史 Evidence。

## 已确认的合同

1. 停止的是不确定状态下的**新增平台导航**，不是已取得数据的保存、分 lane 分包、outbox 重传或服务端接纳。
2. 正文、媒体、评论、回复继续是各自的 Task → Attempt → Capture Package → Receipt；不新增第二套任务账本，也不让同一 Attempt 接受可变的多次终态包。
3. 新增唯一 `collection_detail_page_session`：证据材料以 `(work_order_ref, content_public_ref)` 唯一；跨行业样本仍隔离在自己的 `sample_ref`，以同构的 `(work_order_ref, cross_industry_sample_ref)` 唯一。两侧都绑定首个 owner installation 与持久 `grant_request_id`。同一申请 ID 重传返回同一授权；换申请 ID、换安装或本地状态不明时不再次授权导航。
4. 浏览器在 IndexedDB 的同一读写事务内，把“未消费”转换成“导航许可已消费”，等待事务完成后才允许 `chrome.windows.create`。崩溃窗口宁可停止并标记不确定，也不自动重开。
5. 缓存和 outbox 是已得数据的本机可靠交付层，不随着 Lease 到期删除；但旧 Attempt 的未接纳材料也不得静默改挂到新 Attempt。
6. 若本地已记录由本插件创建的页面 tab，后台重启后只确认/接管该 tab；找不到已消费许可对应的页面时停止自动导航。
7. `prepared` 表示本地已持久化同一份 `grant_request_id`，不是开页事实。授权接口的暂时不可达只允许同 ID 重试或按退避回队；不得换 ID 后再次导航。
8. 显式风险拦截页只在已经打开的 claimed XHS 详情页受限状态面被观察。相同 installation 在 30 分钟内的 2 个独立风险观察会形成 12 小时安装级 cooldown；它不改写人的 `accepting_tasks` 意图，不暂停其他未被确认关联的插件安装。
9. 页面 collector 的 epoch 毫秒时间必须在本机持久缓存边界转换为 RFC3339。服务端明确拒绝 immutable Package 时，使用该 Package 的 submission UUID 追加 `capture_delivery_rejected`；只把当前 Task 结束为 `unavailable`，同页其它冻结 lane 仍可从已保存缓存分包交付，绝不因此新增导航。
10. 最终 URL 明确落到 XHS 的 404/失效页时，追加 `detail_page_url_invalid`：停止并只记住这条已验证执行 URL 的 SHA-256，不把作品身份标成删除。此 URL 不会再派发；后续发现链带回不同签名 URL 后，可由新 WorkOrder 正常执行。

## 实施面

- `0089_detail_page_session_replay_safety.sql`：会话唯一性、owner、授权申请、计划快照/hash、Chrome 导航观测、进度和安全停止原因。导航观测是单独上报的事实，不从授权推断。
- Rust/loopback：只为已 claim 的详情 task 签发或按同一 request id 重放会话授权；不向非 owner 安装泄露或转让该会话。
- `0090_detail_page_grant_recovery_and_risk_cooldown.sql`：追加 grant outcome 审计、安装级风险信号与有截止时间的 cooldown；它们不是 Attempt、Package、Receipt 或 Evidence。
- `0093_capture_delivery_rejection.sql`：把 `capture_delivery_rejected` 纳入闭集失败码。它不形成新的证据或页面事实；它只终结服务端已拒绝的一个 Task，并保留其它 frozen lane。
- `0095_detail_page_url_rejection.sql`：会话只保存服务端核验后签名 URL 的 SHA-256；`detail_page_url_invalid` 终结当前冻结 lane 并在后续派发前拒绝同一 URL。它不引入 URL 生命周期表、TTL 猜测或永久作品失效状态。
- Browser Producer：持久 grant ledger、原子消费、已消费后抑制重开、可接管已知 tab；正文在评论前进入 outbox。页面 payload 仅以原 Lease 寻址，且在全部冻结 lane 均已进 durable outbox 前永不因 TTL/容量被删除。
- 测试：重复领取/并发消费只允许一次导航；授权响应丢失复用同一 grant；已消费但页面不明时 fail closed；正文先于评论；原 Lease 过期仍保留 payload、新 Lease 不得接管，全部 lane 入 outbox 后才可清理。

## 非目标与验收边界

- 不应用共享 migration、不刷新 localhost runtime、不重新加载 Chrome 扩展、不触发平台页面，也不合并或推送。
- 隔离 PostgreSQL 只证明数据库原子性和合同；插件自动测试只证明本地导航闸门和 outbox 边界。真实平台打开次数、运行时切换和业务验收另行核验。
