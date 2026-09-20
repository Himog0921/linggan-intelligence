# OCR-CONTENT-LAYERING-001 数据合同

> 状态: 权威当前
> 最后核对: 2026-09-20
> 适用范围: OCR 原始文字、版面行、内容分层、展示标题来源和视觉模型调用
> 事实来源: DEC-0007、媒体生命周期合同、当前处理作业/派生模型
> 冲突时以谁为准: 用户最新确认、AGENTS.md、迁移与运行时校验

## 不变量

`ocr_raw` 记录引擎读到的文字；`ocr_layout` 记录同一版本的行、归一化坐标和置信度；内容分层只引用行 ID。任何 headline 或实质文本均可回溯到其布局行、输入 Blob、处理版本和（如有）模型调用。

每个行分类只有：`PRIMARY_COPY`、`SUBSTANTIVE_TEXT`、`PLATFORM_WATERMARK`、`PLATFORM_UI`、`INCIDENTAL_SCENE_TEXT`、`BACKGROUND_DOCUMENT`、`UNCERTAIN`（数据库枚举以相应小写串存储）。`retained_line_refs` 是 `{lineRef, classification}` 对象数组，而非不可解释的字符串数组；`cover_headline` 必须能按 `PRIMARY_COPY` 行的阅读顺序精确重建。`UNCERTAIN` 不进入展示标题。

展示标题不是原始标题字段：`displayTitle.source` 必须是 `platform_title`、`cover_ocr` 或 `unknown`；只有原始标题为空、存在可回溯的 `PRIMARY_COPY` headline、且媒体槽位是显式 `cover` 或已知顺序的前 3 张图片之一时才能使用 `cover_ocr`。结果可为 `ACCEPTED` 或 `PARTIAL`：后者只表示图片仍有未决文字，不能抹杀已被行级证据支持的封面标题。没有已知顺序的正文图不猜测为“第几张”。

视觉模型输入为原始图片的受控缩放副本、OCR 行和候选 block；模型输出仅允许行 ID 与受限分类枚举。输出出现未知行 ID、生成新文案、无效 JSON 或没有视觉能力的配置时，整次分层结果拒绝并保持 `NEEDS_REVIEW`。

## 语料与读取资格

`ocr_raw` 是原始机器观察，可用于审计和原文检索，但不是清洗后的语料。只有 `state='ACCEPTED'` 且非空的 `image_substantive_text` 才进入资料库的受控图片有效文本读取与检索；返回的证据片段标为 `image_substantive_text`，并携带对应派生物与媒体槽位。`PARTIAL` 在媒体 Inspector 的 `ocrLayering` 中返回状态、布局引用和候选 headline，但不返回 `imageSubstantiveText`、不进入 clean corpus；其中已逐行证明的 `cover_headline` 仍可按标题回退规则使用。
