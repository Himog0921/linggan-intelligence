# 数据库交付边界

本目录只承载 Linggan Intelligence 的全新数据库设计、migration 和脱敏 fixture。

## 已实现的 Comment Fact Storage V0

migrations/0001_comment_fact_storage_v0.sql 是一个独立、greenfield 的最小事实链。它只接受已通过 xhs.comment-capture-source.v0 运行时校验的 content_detail + comments package pair，并保存：

- 不可变 source_evidence_v0：完整来源 JSON 和逻辑 payload SHA-256；
- 不可变 source_capture_pair_v0：同一次已验证输入所使用的 detail 与 comments Evidence 关系；
- 稳定 comment_identity_v0：workspace + platform + noteId + commentId；
- 不可变 evidence_comment_record_v0：来源 record 到稳定评论身份的关系；
- 不可变 comment_observation_v0：当前 V0 仅以原始评论正文变化形成新事实；
- 可变 comment_current_v0：严格指向同一身份、按本系统 admission sequence 最新的一条不可变 Observation。

它不保存从当前 fixture 无法可靠证明的评论作者、点赞、评论发表时间、回复链、作品正文或 coverage 语义。特别地，V0 的时间列是 admitted_at，表示本系统接纳来源包的时间；Current 的“最新”只表示 adapter 最后接纳的不同正文 materialization，不能显示为用户评论的发表时间或平台观察时间。

真实 PostgreSQL proof 使用一次性 Docker 容器：

    scripts/prove-comment-fact-storage-v0.sh

它创建随机容器、随机数据库和本机随机端口，运行 migration 与攻击性测试后自动删除容器。脚本不会读取已有数据库 URL，也不会连接旧数据库或 compose 网络。

## 禁止进入 Git

- 旧数据库 dump。
- 生产数据、Cookie、Token、DSN。
- 本地媒体文件。
- 未脱敏的 Evidence payload。

## 旧数据库

旧数据库只恢复到独立的 linggan_legacy_archive，并使用只读账号。新项目代码不得连接或 fallback 到它。

## 新数据库

建议名称：

- 开发：linggan_intelligence_dev
- 测试：每次随机命名 linggan_intelligence_proof_timestamp
- 正式：由部署环境单独配置，不写入仓库
