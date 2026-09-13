# Comment Research · User Voice Context Read API V0

状态：PROVEN IN ISOLATION
日期：2026-09-13

范围：为「用户原声」详情抽屉读取**已采到且可与当前原声正文匹配**的作品标题/正文和相关回复。它不创建研究任务、不执行清洗、不调用模型，也不把已采到的局部来源伪装为完整的平台作品或评论树。

## 用户路径

用户先在「用户原声」看到当前评论事实。打开任意一条后，浏览器带回列表已经公开的 `workspace_id + Evidence ID + record index`，异步读取：

```text
当前原声
→ 已采到的相关讨论
→ 作品上下文
→ 来源证据
```

浏览器不发送 commentId、作者身份或平台链接。服务端先验证该 Evidence 定位符仍然是这个 workspace 的 **Current Comment** 来源；只有通过后，才在存储层内部解析 note/comment 身份和读取上下文。旧 Evidence、伪造 Evidence 或正文更新后不再当前的定位符都返回 `404 comment_voice_not_found`，不会把旧上下文挂到新原声上。

## 接口

```text
GET /api/v0/comment-research/voices/context
  ?workspace_id=<required>
  &evidence_id=<current-list Evidence UUID>
  &record_index=<current-list non-negative integer>
```

成功且存在完整、正文 SHA-256 匹配的三包来源上下文时：

- `availability: available`；
- `work_context`：作品 title / body_text 各自保留 `observed`、`blank`、`unavailable` 三态，均带自己的 Evidence ID 与记录序号；
- `related_discussion`：最多 100 条已采到、以根评论/父评论/被回复对象指向当前原声的回复。每条只有文本、三种关系布尔值和 Evidence 引用。

当前原声有效、但尚未接入完整匹配的上下文时，只返回：

```json
{ "availability": "unavailable" }
```

这表示本系统尚未取得匹配上下文，**不表示平台没有作品、正文或其他讨论**。缺失参数、非法 UUID、非法记录序号返回 `400 invalid_request`；定位符不再当前或不存在返回 `404 comment_voice_not_found`。

响应不含：commentId、作者、URL、点赞、平台发表时间、OCR、ASR、媒体资源、模型/研究状态或任何完整性声明。

## 运行证明

```text
scripts/prove-comment-voice-context-read-api-v0.sh
```

脚本启动随机命名的 `postgres:16-alpine` 容器、随机数据库和随机 loopback 端口，运行真实 Axum Router 集成测试；结束时删除容器。它不读取旧库、共享库、环境既有数据库 URL，也不启动 3000 端口。

| 场景 | 已验证的事实 |
| --- | --- |
| 可读上下文 | 当前列表 Evidence 可读出 source-backed 标题、正文和 2 条相关回复；一条回复的 parent 与 reply-to 关系独立为真 |
| 三态文本 | `title=blank` 与 `body_text=unavailable` 在 HTTP DTO 中不被压为同一个空字符串 |
| 无匹配上下文 | 当前原声仍返回 200，但明确为 `availability=unavailable` |
| 定位符校验 | workspace、Evidence UUID、记录序号缺失或非法为 400；伪造 UUID 为 404 |
| 正文更新 | 同一 note/comment 接纳新正文后，旧 context Evidence locator 变为 404；新当前 locator 返回 unavailable，不附着旧上下文 |
| 纯读 | 所有 context GET（包括错误输入）前后九张 Evidence/Comment/Context 表行数不变 |
| DTO 边界 | 递归断言响应不泄露 commentId、作者、URL、点赞、发布时间、OCR、ASR 或媒体字段 |

## 未证明、不得声称的能力

- 平台最新状态、完整评论树、所有父/根评论、上下文采集完整性或时间新鲜度；
- 当前评论作者、作品作者的身份判断或任何作者维度结论；
- 文本相关片段截断、OCR/ASR、模型 Context Pack、清洗、研究结果、问题聚类或变化趋势；
- 生产/共享数据库迁移、已有 3000 端口切换、真实浏览器验收或用户业务验收。
