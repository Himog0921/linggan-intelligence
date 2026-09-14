# COMMENT-RESEARCH-OUTPUT-DIAGNOSTICS-001 · V3 结构输出诊断与示例

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #258 的 V3 评论研究输出合同、安全调用账本与受控真实运行
> 事实来源: Run `e4e6d635-7f63-434d-8852-7c6ee91086ff`、DeepSeek 官方 Responses / JSON Output 文档、当前 worker 与 Pi adapter
> 冲突时以谁为准: 真实运行回执、当前代码、数据库账本与自动化证明

## 用户结果

评论研究运行失败时，用户可以知道失败发生在 JSON 传输、结构合同还是问题归并接纳阶段；系统不把原始评论、模型回复或模型思考写进常规账本，也不让部分失败 Run 生成研究页面版本。

## 已确认事实

- V2 20 条受控 Run 完成为 `completed_with_failures`，不包含 ResultRevision。
- RunItem 有 1 次 `semantic_json_unparseable`；归并有 7 次 `invalid_problem_resolution`。
- 失败语义调用已收到正常结束事件、存在输出 token；当前账本没有保存文本，因此不能从历史账本还原模型原文。
- DeepSeek 官方 Responses API 支持 `text.format=json_schema`；JSON Output 指引要求 prompt 中有 JSON 与合法 JSON 示例，也说明输出可能为空。

## 实施决定

1. V3 输出合同进入研究指纹：同一评论不会把 V2 失败或结论误认作 V3 的有效完成。
2. 保持严格传输边界：只接受完整 JSON 或唯一完整的 `json` fenced block。不能从说明文本或多段内容截取对象。
3. 安全账本记录 `textShape`（缺失、空、直接 JSON、json fence、非 JSON）和有限长度区间；不保存正文、提示词或思考文本。
4. 语义输出分为 `semantic_json_unparseable`、`semantic_json_schema_rejected`、既有证据/接纳失败；问题归并分为 JSON、schema、admission 三类。
5. 语义与归并 packet 增加最小合法 JSON 示例，并继续使用已合格官方 endpoint 的 `json_schema` 请求。
6. 部署后，按开发期授权重置**仅评论研究派生层**，再运行最多 20 条 V3 受控首批；只有全成功、发布 ResultRevision、五个读视图均可读时才扩大范围。

## 非目标

- 不保留或展示模型链式思考、完整模型回答、提示词或评论正文。
- 不放宽失败即不发布的门槛。
- 不恢复 V1/V2 的派生数据，也不变更原始评论、作品或模型配置。

## 验收

- Rust：解析与 schema 分离、失败码 / 阶段映射、安全 textShape 不泄露文本。
- Node：V3 packet 对 DeepSeek / OpenAI 已合格协议仍请求 named json schema。
- PostgreSQL：既有 Run、派生 reset、读取与未发布边界保持通过。
- Runtime：V3 受控批次用运行回执、发布状态、读取端点分别核验。
