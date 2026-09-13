# 评论研究 · 来源支撑的研究输入预览 V1

状态：PROVEN IN ISOLATION
日期：2026-09-13

范围：为一个当前、完成 Cleaning Contract V1 的用户原声展示**未来研究可能读取的已采到文本**。它只是内容装配预览：不创建任务、不冻结样本、不保存输入快照、不判断上下文是否充分、不调用模型，也不生成评论分析、问题、聚类或趋势。

## 用户路径

用户在「用户原声」中打开原声详情，先阅读原始采集原声、已采到的相关讨论和作品上下文；再按需点击「查看研究输入预览」。浏览器仍只携带列表已公开的：

```text
workspace_id + Evidence ID + record index
```

服务端仅在该定位符仍是一个**当前且有 `comment-cleaning.v1` 可研究派生**的评论来源时组装预览。正文推进后，旧定位符返回 `404 comment_voice_not_found`；不会把旧语境附着到新正文。

接口：

```text
GET /api/v0/comment-research/voices/context-pack
  ?workspace_id=<required>
  &evidence_id=<current-list Evidence UUID>
  &record_index=<current-list non-negative integer>
```

## 预览的内容边界

响应明确分成三类文字：

| 区域 | 可以包含 | 证据含义 |
| --- | --- | --- |
| 当前评论的直接证据 | 仅 Cleaning Contract V1 的 `research_expression` | 唯一可归因给当前评论者的文字 |
| 讨论语境 | 已采到且 root / parent / reply-to 指向当前原声的回复 | 帮助理解讨论，不是当前评论者的证据 |
| 作品语境 | 已采到、正文 hash 匹配的 `content_detail` title / body | 帮助理解作品，不是当前评论者的证据 |

原始采集原声只保留在已有的来源详情读取中；这个 endpoint 不返回原始正文、Evidence package 或任何最终 Prompt。V1 来源合同没有 OCR、ASR、媒体、作者、点赞或发表时间，因此这些字段不会进入预览。

`needs_context` 仍可查看预览，但会明确说明：页面只显示当前已采到的文本，未来实际研究必须另行检查语境是否足够。没有匹配 Context Capture 时，讨论与作品语境都是 `unavailable`；这不表示平台没有作品或讨论。

## 固定预算与截断合同

V1 使用 Rust 的 Unicode `chars()` 截断，不做 byte slicing，因此不会把 UTF-8 scalar value 切断。

| 字段 | 上限 |
| --- | ---: |
| 当前评论的直接证据 | 480 字符 |
| 已采到的相关讨论 | 最多 3 条、每条 220 字符 |
| 作品标题 | 160 字符 |
| 作品正文 | 500 字符 |
| 预览全部文字 | 1,800 字符 |

响应返回已纳入字符数与各项固定上限。任何直接评论、讨论、标题或正文被截断，或已读取的讨论超过 3 条，都会在 `omissions` 中给出可读说明。`omissions` 只说明本预览的来源/预算边界，不是模型错误、清洗原因或研究结论。

## 隔离 PostgreSQL 运行证明

```text
scripts/prove-comment-voice-context-pack-preview-v1.sh
```

脚本启动随机命名的 `postgres:16-alpine` 容器、随机数据库与随机 loopback 端口，通过真实 Axum Router 运行 HTTP 集成测试。结束时容器删除。它不读取共享数据库、不使用 3000 端口、不调用 provider、向量服务、worker 或外部来源。

已验证：

| 场景 | 结果 |
| --- | --- |
| 已采到的文本 | 当前清洗表达、已关联回复和作品 title/body 分区返回，回复关系保持 root/parent/reply-to 三个独立布尔值 |
| 直接证据边界 | 当前直接证据不等于讨论语境文字；作品/回复不会进入直接证据字段 |
| 三态作品字段 | `title=blank`、`body_text=unavailable` 不被压成同一个空字符串 |
| 无匹配上下文 | `needs_context` 的「同问」仍可预览直接证据，讨论/作品明确为 unavailable，并提示未来执行仍要检查充分性 |
| Unicode 与预算 | 长中文/emoji fixture 的文本按 char 上限截断、直接证据为 480 字符、讨论最多 3 条、标题/正文均标明截断、总数不超过 1,800 |
| 确定性 | 相同 Current 和来源上下文连续两次 GET 返回相同 JSON |
| 定位符与正文推进 | 缺少/非法参数为 400；伪造 locator 为 404；同一 identity 的正文推进后旧 locator 为 404，新 current locator 不附着旧上下文 |
| 纯读与隐私 | 成功、无上下文、错误参数与伪造 locator 的 GET 前后 10 张 Evidence/Comment/Derivation/Context 表行数不变；DTO 递归拒绝作者、评论/作品 ID、URL、点赞、时间、OCR/ASR、媒体、原始文本、Evidence package、清洗原因、派生/指纹、Prompt、模型、Token 与费用字段 |

## 未证明、不得声称

- 实际模型输入快照、系统指令、Prompt/Skill、provider/model 配置、Token、成本或任何模型调用；
- Context Pack 的执行时充分性判断、任务/队列/Run、失败重试或研究结果；
- 结构化输出校验、Evidence span、问题归并、向量、聚类、趋势或自动排程；
- 共享数据库 migration、main 合并、3000 端口切换、真实浏览器验收或业务验收。
