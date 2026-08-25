# PLUGIN-RUNTIME-001 · 灵感爆爆爆全量 Browser Producer Runtime Retrofit

> 状态: 活跃计划  
> 最后核对: 2026-08-25  
> 适用范围: GitHub Issue #50 的灵感爆爆爆全量 Browser Producer Runtime Retrofit、媒体本地化和 Evidence Library 受控读取；不授权真实平台访问。  
> 事实来源: 用户最新确认、`AGENTS.md`、Issue #50、现行 Browser Producer 合同、当前代码与可复现验证。  
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前活跃 SCOPE/数据合同以及真实代码、数据库和运行证据；历史内容工作台仅是迁入参考。  
> 事项: GitHub Issue #50

## 用户结果

Linggan Intelligence 自身携带并使用原“灵感爆爆爆”浏览器插件的成熟采集与交互能力。插件不再连接或回退到内容工作台；它是 Linggan 的唯一 Browser Producer Runtime。用户既可从原有 Popup、页面按钮、批量控制和恢复交互手动采集，也可在未来接收已获授权的 Linggan `TaskSpec`，但两条路径都进入同一个 Attempt、不可变 Capture Package、Coverage、receipt 与本地 Evidence 入口。

## 已冻结边界

- Linggan 决定采什么和是否有资格继续；插件只保留浏览器内执行、暂停/恢复/停止、页面读取、重试、下载与进度。
- 当前是 `LOCAL_TRUSTED`：无旧工作台登录、授权码、工位、租约、心跳或轮询器；仍保留 producer/task/attempt/package/submission 身份，防止重放和混淆。
- Partial 不丢弃：已取得记录可以接纳；Coverage 记录已观察、已尝试、已取得、已验证、失败、未尝试和未知，不能把缺口改写成零或完成。
- 原插件的页面读取、错误恢复与 UI/UX 是待复用资产；旧 Workbench 的 URL、队列、同步回退、身份授权和数据事实模型不是活跃运行时。
- 未连接的外部处理器不得伪装为 OCR/ASR 完成。原始媒体接纳与后续处理独立。

## 媒体不可谈判设计

```text
MediaSlot（内容中的角色与序号）
→ MediaObservation（一次观察到的候选 URL）
→ MediaDownloadAttempt
→ MediaBlob（SHA-256 / MIME / size）
→ Linggan-owned /api/local/media/<sha256>
→ ProcessingJob event → derivative（OCR / ASR / thumbnail 等）
```

URL 既不是媒体身份，也不是 Evidence Library 的展示地址。一个 blob 可以为多个 Slot 服务；一个 Slot 可以随着后续观察拥有新的 URL，而不重写历史。下载失败不否定已成功的文字/发现；媒体上传在独立 outbox lane 中运行，不阻塞文字 Capture Package 的接纳。

本机文件保存在 `LINGGAN_LOCAL_MEDIA_ROOT`（默认 `.linggan-local/media`）下，以临时文件写入、SHA-256 验证、原子重命名方式 materialize。数据库只保存 metadata、lineage 和 Linggan 本地 asset path；旧 Dexie/download 临时路径只能作为采集过程暂存，receipt 后按本机策略清理，永不作为 Evidence Library 展示来源。

## 实施面与可证伪验收

| 面 | 本轮实现 | 验收 |
|---|---|---|
| Runtime contract | 一个版本化 TaskSpec/Attempt/Capture Package，覆盖 discovery、profile、detail、comments、replies、author、media slots/bytes 与 checkpoint | 合同负例与插件 contract test；未将 Topic/Claim 放入插件 |
| 旧能力继承 | XHS / Douyin 现有页面 collector、批量、恢复与 UI 经过统一 receipt seam 接入 | 编译包中存在原 collector；手动与 batch 不再走旧远端回传 |
| Text delivery | durable browser outbox、重放 receipt、终态 Attempt 不追加 package | outbox timeout/retry test；隔离、自动清理的 PostgreSQL proof 已通过；持久本机运行库仍待凭据修复授权 |
| Media | Slot/URL/blob/job/event DDL；本地 asset URL；独立 bounded media outbox | 同 bytes/不同 URL、重复 blob/多 Slot、partial Coverage、远端 URL 不作为显示 URL 的 proof |
| Processing | OCR/ASR/thumbnail job 仅以 pending event 入队；未启用 provider 不产出 derivative | job lineage test；不得宣称实际识别完成 |
| Read boundary | Evidence Library 只读 Linggan 本地事实与本地 asset URL；不触发平台读取 | UI fixture / API proof；尚未接入的数据必须显示 SOURCE_INCOMPLETE |

## 明确不做

- 不在开发中访问真实 XHS/Douyin、Cookie、账号或真实原文。
- 不恢复内容工作台接口或双写/fallback。
- 不创建 Topic、Claim、趋势、研究结论或 OCR/ASR 外部调用。
- 不在本事项自动合并、关闭 Issue 或声称用户验收。

## 当前阻塞与停止点

1. 隔离且自动清理的 PostgreSQL proof 已通过；本机持久运行库的 `local-runtime.sh migrate` 仍发现应用角色密码与 `.env` 不一致。该脚本要求显式 `repair-password`，本事项不自行执行。它阻止的是持久本机运行库迁移，不否定已通过的隔离 proof。
2. 外部 OCR/ASR provider 尚未授权。队列与血缘可以实现，但任何“已转录/已 OCR”都必须保持 `NOT_ENABLED`。
3. 真实浏览器/平台验收只能在 Draft PR 经独立审查、合并并由 Mog 明确开始 Canary 后进行。
