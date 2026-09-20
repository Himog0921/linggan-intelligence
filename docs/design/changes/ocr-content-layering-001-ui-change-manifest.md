# OCR-CONTENT-LAYERING-001 · Evidence Library UI 变更清单

> 状态: 活跃计划
> 最后核对: 2026-09-20
> 适用范围: Evidence Library 列表/Inspector 的标题来源与图片文字可追溯表达
> 事实来源: PAGE-EVIDENCE-001、DEC-0007、OCR-CONTENT-LAYERING-001 数据合同、LIDS v7.0
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实 API/数据合同

## 分类与用户结果

这是状态/语义变更：列表仍呈现同一 Work Resource；当原始标题未知且已验证的封面主文案存在时，标题位显示该文案，并以轻量“封面 OCR”来源说明区分。它不创建标题、不改变平台原始标题、不触发 OCR/模型调用。

## 表面与状态

- `EV-S05` 列表：`platform_title` 不显示额外标记；`cover_ocr` 显示“封面 OCR”；`unknown` 继续“标题当前未知”。
- `EV-S06` Inspector：标题事实与来源分别显示；材料区展示 raw/layout/分层的处理版本、状态和限制。
- 不新增页面、导航、按钮或全局 Token；使用既有 LIDS 文本/状态 Primitive 与数据边界规则。

## 依赖与验证

依赖共享 Work Resource Read Interface 的 display title/source 字段；不另写浏览器 SQL。验证包括 API serialization、未知与 OCR 回退负例、既有布局的 DOM/JS fixture。真实图像、真实 provider、部署和人工视觉验收均不由本改动自动证明。

