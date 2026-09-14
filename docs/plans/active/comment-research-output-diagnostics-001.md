# COMMENT-RESEARCH-OUTPUT-DIAGNOSTICS-001 · V6 分支结构合同、候选绑定与安全诊断

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #258/#260/#262 的 V5 诊断与 V6 评论研究输出合同、安全调用账本与受控真实运行
> 事实来源: Run `7025f656-8ecc-48bc-9bbb-c8b33f66ff22`、DeepSeek 官方 Responses / JSON Output 文档、当前 worker 与 Pi adapter
> 冲突时以谁为准: 真实运行回执、当前代码、数据库账本与自动化证明

## 用户结果

评论研究运行失败时，用户可以知道失败发生在 JSON 传输、结构合同还是问题归并接纳阶段；系统不把原始评论、模型回复或模型思考写进常规账本，也不让部分失败 Run 生成研究页面版本。

## 已确认事实

- 最新 20 条 Run `7025f656-8ecc-48bc-9bbb-c8b33f66ff22` 完成为 `completed_with_failures`，不包含 ResultRevision：5 个 RunItem 是 `semantic_json_schema_rejected`，10 个 Problem Resolution 是 `problem_resolution_json_schema_rejected`。
- 两类失败调用都已正常结束，安全账本均为 `textShape.shape=direct_json`，而非空输出、截断或 Markdown 包装；账本不保存模型文本，因此不能也不应从历史账本还原模型原文。
- 现有 V5 provider Schema 只在根对象要求 `outcome`/`decision`，没有将 Rust tagged variant 的分支字段写为互斥且必填。这是可由当前代码和安全形态裁定的合同缺口，不是降低发布门槛或放宽解析的理由。
- DeepSeek 官方 Responses API 支持 `text.format=json_schema`；JSON Output 指引要求 prompt 中有 JSON 与合法 JSON 示例，也说明输出可能为空。

## 实施决定

1. V6 输出合同的 extraction/membership rule hash 进入保存策略与研究指纹：同一评论不会把此前 V5 的失败或结论误认作 V6 的有效完成；部署后必须重新保存策略才可开始新的 Run。策略表既有 `contract_version` 保持其稳定数据库家族语义，不被误作 packet revision。
2. 保持严格传输边界：只接受完整 JSON 或唯一完整的 `json` fenced block。不能从说明文本或多段内容截取对象。
3. 安全账本记录 `textShape`（缺失、空、直接 JSON、json fence、非 JSON）和有限长度区间；不保存正文、提示词或思考文本。
4. 语义输出分为 `semantic_json_unparseable`、`semantic_json_schema_rejected`、既有证据/接纳失败；问题归并分为 JSON、schema、admission 三类。
5. 语义与归并 packet 保留最小合法 JSON 示例，并以 `oneOf`/`const`/必填字段将 provider Schema 收束为与 Rust tagged variant 相同的两条互斥分支，继续使用已合格官方 endpoint 的 `json_schema` 请求。
6. `same_problem` 示例仅使用本次候选集实际给出的 `problemRef` 与 `definitionRevision`；不存在候选时只展示 `new_problem` 示例，禁止固定演示 UUID。
7. 语义 atom 的 evidence 改为研究正文中唯一可精确匹配的短句；程序将它映射为不可变 source offsets。模型不再提交 Unicode 字符坐标。
8. 部署后，按开发期授权重置**仅评论研究派生层**，再运行最多 20 条 V6 受控首批；只有全成功、发布 ResultRevision、五个读视图均可读时才扩大范围。

## 非目标

- 不保留或展示模型链式思考、完整模型回答、提示词或评论正文。
- 不放宽失败即不发布的门槛。
- 不恢复 V1/V2 的派生数据，也不变更原始评论、作品或模型配置。

## 验收

- Rust：解析与 schema 分离、失败码 / 阶段映射、安全 textShape 不泄露文本。
- Node：V6 packet 对 DeepSeek / OpenAI 已合格协议仍请求独立 named json schema。
- PostgreSQL：既有 Run、派生 reset、读取与未发布边界保持通过。
- Runtime：V6 受控批次用运行回执、发布状态、读取端点分别核验。
