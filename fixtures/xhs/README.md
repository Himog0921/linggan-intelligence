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
