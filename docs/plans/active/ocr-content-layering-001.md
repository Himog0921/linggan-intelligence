# OCR-CONTENT-LAYERING-001

> 状态: 活跃计划
> 最后核对: 2026-09-20
> 适用范围: Issue #296 的图片 OCR 引擎替换、内容分层、证据库标题补位与历史 Tesseract OCR 处置
> 事实来源: DEC-0007、当前媒体处理代码/迁移、50 张本地对照结果和 Mog 最新授权
> 冲突时以谁为准: 用户最新确认、AGENTS.md、当前代码/迁移与真实验证

## 用户结果

证据库在平台标题为空时，能谨慎显示从封面中找回的作者主文案；图片 OCR 可用于语料，但不会把水印、截图 UI、背景教材或衣服 Logo 混入主要内容。用户可查看原始识别、分层结果及其来源。

## 范围与非目标

- 范围：PaddleOCR 本地 bridge；坐标化 OCR；本地规则分层；视觉分层结果的可审计数据合同；标题回退；处理版本升级；历史 Tesseract OCR 的可审计失效与新作业追加；对应 API/UI 读取、合同、测试和运行说明。
- 非目标：平台采集、浏览器插件、ASR/Whisper、原始图片/Evidence 删除、真实 provider 调用、共享数据库迁移、运行时部署、自动批量重跑和业务验收。当前 Pi adapter 的模型描述仍只声明 `text` 输入；在它获得受控图像输入、视觉能力探测和调用账本的端到端实现前，复杂图片一律停在 `PARTIAL`/`NEEDS_REVIEW`，不会假装已完成视觉语义判断。

## 表面、状态、依赖与验收

| 表面 | 正常 | 未完成/失败 | 验收 |
|---|---|---|---|
| 媒体 worker | Paddle 产出 raw+layout，随后排入分层 | 本地模型缺失或图像失败保留明确处理失败；视觉配置不可用不外发 | bridge fixture、Rust unit 与隔离 PostgreSQL |
| 分层派生 | headline/实质文本只引用已识别行，记录保留/排除原因 | `NEEDS_REVIEW` 不产出标题，不冒充空内容 | 结构与攻击性负例测试 |
| Evidence Library | 原始标题优先；空标题显示封面文案并标记“封面 OCR” | 无合格文案仍显示标题未知 | API projection 与浏览器/JS 验证 |
| 历史处置 | Tesseract 结果失效、不可读、不可检索，新 Paddle 作业追加 | 不删除原图、Evidence、处理事件或 ASR | migration fixture 与 requeue proof |
| 视觉调用 | 结果表已预留 `vision` 与调用账本关联 | 当前 adapter 不具备受控图像输入，复杂图停在 `PARTIAL`/`NEEDS_REVIEW` 且不外发 | 合同/迁移约束；真实视觉 runner 是下一张需选定 provider 的卡 |

## 停止条件与回退

- 若图片输入协议无法保持调用账本、最小数据与显式模型能力，停止视觉调用部分，保留本地规则与 `NEEDS_REVIEW`。
- 若 migration 会删除非 OCR 派生、原始媒体或 Evidence，停止并修正处置谓词。
- 回退是停止新版本 worker/不执行 migration；已追加的新派生通过其版本和失效状态隔离，不回写原始材料。
