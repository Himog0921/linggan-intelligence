# Comment Context Storage V0 · 隔离 PostgreSQL 证明

状态：PROVEN IN ISOLATION
日期：2026-09-13
范围：一组去标识化 XHS `content_detail + comments + replies` 独立来源包的不可变接入和只读上下文读取。它不代表 API、页面、清洗、LLM、研究任务、向量或用户问题功能已经完成。

## 运行命令

    scripts/prove-comment-context-storage-v0.sh

脚本启动一个全新的 `postgres:16-alpine` Docker 容器，使用随机容器名、随机数据库名和 `127.0.0.1` 随机端口。连接串只传给本次集成测试进程，脚本退出时删除容器。

## 数据模型边界

| 表 | 责任 | 可变性 |
| --- | --- | --- |
| `source_evidence_v0` | 通过特定来源合同验证的完整 package JSON 与 hash | append-only |
| `comment_identity_v0` / `comment_observation_v0` / `comment_current_v0` | 当前评论的直接事实与本地 current materialization | 继承 0001 语义 |
| `context_work_record_v0` | 一条 `content_detail` Evidence 的作品标题、正文可用性与可选作者来源事实 | append-only |
| `context_discussion_record_v0` | `comments` / `replies` Evidence 中逐条锚定的讨论事实和独立关系指针 | append-only |
| `source_context_capture_set_v0` | 三份独立 Evidence 的不可变审计关系 | append-only |

Context Storage V0 不建立 `context_current`。一次 context read 只选择一组已完整接纳、同 workspace/platform/noteId/commentId 且 **正文 SHA-256 等于当前 Comment** 的三包来源关系。若没有这种关系，读取结果就是 `None`；不按相同 comment ID 把旧上下文接到新正文上。

作品详情、父评论和回复只用于解释讨论语境。它们各自带 Evidence + record index 引用，不能写成当前评论作者的直接表达。

## 已验证场景

| 场景 | 预期 | 证明 |
| --- | --- | --- |
| 0001 后应用 0002 | 新迁移只依赖新 Comment Fact baseline | 集成测试 |
| replies 包跨作品 | Context Source Contract 在 transaction 前拒绝，九张表均 0 写入 | 集成测试 |
| 首次三包接入 | 三份 Evidence、直接 Comment Current、1 条 work context、3 条讨论记录、1 个 capture set 同事务写入 | 集成测试 |
| 精确 replay | Evidence、context records、capture set 与 Comment Current 均不重复覆盖 | 集成测试 |
| 读取当前上下文 | 返回作品 Evidence 锚点及两个显式相关回复；读操作无写入 | 集成测试 |
| reply 的双关系 | 同一 reply 的 `parentCommentId` 与 `replyToCommentId` 同时保留 | 集成测试 |
| 同 commentId、正文变化 | 旧 context hash 不匹配，新 current 不附着旧上下文 | 集成测试 |
| capture set 跨 workspace | PostgreSQL trigger 以 `55000` 拒绝 | 集成测试 |
| 修改/删除 work、discussion、capture set | PostgreSQL append-only trigger 以 `55000` 拒绝 | 集成测试 |

## 未证明、不得声称的能力

- 平台最新状态、评论发表时间、点赞、完整回复树或采集完整性；
- 作者是否为内容作者，及任何基于作者身份的研究结论；
- OCR、ASR、媒体资源、上下文截断或模型输入；
- 任何语义研究、标签、问题聚类、变化趋势或自动化调度；
- HTTP/API/UI 的上下文展示。

`admitted_at` 仅表示 Linggan 本地接纳时间，不能替代平台发布时间、采集观察时间或真实世界的上下文新鲜度。
