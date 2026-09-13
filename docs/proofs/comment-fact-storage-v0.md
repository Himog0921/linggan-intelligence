# Comment Fact Storage V0 · 隔离 PostgreSQL 证明

状态：PROVEN IN ISOLATION
日期：2026-09-13
范围：XHS content_detail + comments 去标识化 source pair 的事实接入。不是 HTTP、页面、LLM、清洗、向量、调度、问题归并或变化观察的完成声明。

## 运行命令

    scripts/prove-comment-fact-storage-v0.sh

脚本使用一个全新的 postgres:16-alpine Docker 容器、随机容器名、随机数据库名和 127.0.0.1 随机端口。它只将连接串通过当前进程环境变量传递给集成测试，退出时自动删除容器。

## 数据模型边界

| 表 | 责任 | 可变性 |
| --- | --- | --- |
| source_evidence_v0 | 通过来源合同的完整 package JSON、SHA-256 与 admission 时间 | append-only |
| source_capture_pair_v0 | 一次已验证输入中 detail Evidence 与 comments Evidence 的不可变配对关系 | append-only |
| comment_identity_v0 | workspace + platform + noteId + commentId 稳定身份 | immutable |
| evidence_comment_record_v0 | package record 与评论身份的来源关系 | append-only |
| comment_observation_v0 | 正文变化的不可变事实版本 | append-only |
| comment_current_v0 | 该稳定身份当前采用的 Observation | 只允许推进到同一身份、更晚 admission sequence 的 Observation |

source_evidence_v0 的重复 package 由 workspace、platform、source_contract、package_kind 和 payload_sha256 唯一约束消重，重复写入只返回原 Evidence，不更新已有 JSON。

V0 中 Current 的“最新”严格表示 **adapter 最后接纳的不同正文 materialization**。它不表达 source package 的平台观察时间、评论发表时间或真实世界最新状态；这些时间语义尚无可用的版本化来源合同。

## 已验证场景

| 场景 | 预期 | 证明 |
| --- | --- | --- |
| 来源文本空白 | runtime contract 先拒绝，数据库 0 写入 | 集成测试 |
| JSON 任意扩展包含 U+0000 | evidence prepare 在 transaction 前拒绝，六张表均 0 写入 | 集成测试 |
| 首次接入 fixture | 2 条 source Evidence、1 个身份、1 个 Observation、1 个 Current | 集成测试 |
| 同一 comments Evidence 配不同 detail Evidence | 新增独立 pair 关系，保留两次输入的可追溯关系 | 集成测试 |
| 完全相同 package replay | 复用 Evidence 与已有 source Observation，不覆盖、不推进 Current | 集成测试 |
| 不同 package、相同正文 | 新 Evidence 与来源 record 入库，但不重复 Observation 或 Current | 集成测试 |
| 不同 package、正文变化 | 新 Observation 追加，Current 在同一事务推进 | 集成测试 |
| 旧来源在正文变化后重放 | 不允许将 Current 回拨到旧 Observation | 集成测试 |
| 不同作品使用同一 commentId | 因 noteId 不同形成两个独立身份 | 集成测试 |
| 修改 Evidence 或删除 Observation | PostgreSQL trigger 以 55000 拒绝 | 集成测试 |
| Current 的 hash 与引用 Observation 不一致 | PostgreSQL trigger 拒绝伪造 projection | 集成测试 |
| Current 回指同身份较早 Observation | PostgreSQL trigger 拒绝 | 集成测试 |
| 两个 client 并发写入不同正文 | identity row lock 串行化；最终 Current 指向该身份最高 admission sequence | 集成测试 |

## 未证明、不得声称的能力

- 外部 package 的发布时间、平台观察时间、点赞、回复链或作者字段的完整性；
- 决定一条评论有无研究价值；
- 清洗派生文本、模型输入、LLM 结论、问题簇或趋势；
- 并发 writer 的负载、性能与故障恢复基线；
- 任何 source time 或迟到来源事件的排序语义；
- 插件到新 Rust 项目的真实生产传输。

本 proof 的 PostgreSQL trigger 是当前应用服务角色的完整性边界。数据库 owner、superuser 或拥有禁用 trigger 权限的运维操作可绕过它们；这类权限隔离、迁移执行角色和审计策略不属于 V0 的实现声明。

下一条切片应在此 current projection 上建立受控只读 DTO 和原声浏览器合同；不能在 UI 中把 V0 未验证字段渲染为 0 或已知事实。
