# XHS 评论来源证据集 V1

本目录的 `comment-evidence-set-v1.json` 是一对来自现有本地运行时、已接纳的 producer CapturePackage 的**去标识化形状证据**。它包含一个 `content_detail` 包和一个 `comments` 包，并保留：

- 外层 CapturePackage 与 coverage 的 JSON 嵌套形状；
- record kind 与 package kind；
- 同一作品与评论之间的来源身份关联；
- 已观察字段的存在性和 JSON 类型。

它不包含、也不证明任何真实用户内容、账户或可识别标识：所有文本、姓名、URL、原始 payload 值、计数和时间均已替换为安全占位值。该文件不应用于模型调用、页面演示、指标计算或来源真实性判断。

## 完整性与隐私检查

- SHA-256：`0eb0dd567bafabb4b2511c5c83531959c544cb8a90261050c17a6e4ca38e6234`
- 去标识化：所有字符串都必须是文档声明、受控枚举、`<redacted…>` 占位值、`example.invalid` URL、统一时间戳，或 `*-fixture-###` 关系占位符。

后续合同测试只能从这个 fixture 读取字段形状；缺失字段必须视为未知，不能补成生产默认值。

## Context V0 evidence set

`comment-context-evidence-set-v0.json` 是第二份去标识化来源形状证据，来自一个真实、已接纳的 `content_detail + comments + replies` 三包组合。它保留一个作品详情、一条一级评论和两条关联回复的字段形状，以及 `noteId`、`commentId`、`rootCommentId`、`parentCommentId`、`replyToCommentId` 的跨包关系。

- SHA-256：`310c9fdad4f5c1634fa686036d36462f5e23471667c54bab6c258c6e3fb68c67`
- 真实关系：其中至少一条 reply 同时有 `parentCommentId` 与 `replyToCommentId`；两者必须保持为独立字段。
- 边界：它只证明一个可解析的上下文样本，并不证明每个评论都有详情、正文、父评论、根评论、作者身份、OCR、ASR 或同一次采集版本的完整上下文。
