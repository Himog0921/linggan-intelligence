# 评论研究产品化 · 机器可读合同

> 状态: 权威当前（已批准目标的伴随合同，非已部署状态）
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001
> 事实来源: 用户交付的 v1.0 手册 validation 目录
> 冲突时以谁为准: 对应数据库和HTTP合同；实际语义由Rust接受器证明

此目录是原手册 `validation/` 中四个JSON Schema及开始命令示例的仓库书架。入库仅压缩JSON空白，不改变解析后的数据。完整入口见 [手册入口](../../runbooks/comment-study-productization-package.md)。

- `start-run.schema.json` 与 `start-run.example.json`：开始命令与合成示例。
- `semantic-output.schema.json`：按targetRef归属的模型提取输出。
- `resolution-output.schema.json`：冻结候选闭集比较。
- `pair-output.schema.json`：独立证据配对与Problem定义形状。

JSON Schema只规定传输形状；身份、权限、UTF-8字节数、确切候选集合、证据逐字定位、预算和并发必须由Rust/数据库验证。禁止把Schema通过等同业务正确。原始文档验证回执及T01–T54状态分别归档，不在这里预先改成PASS。
