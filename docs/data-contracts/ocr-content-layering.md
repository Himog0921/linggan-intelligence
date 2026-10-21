# OCR-CONTENT-LAYERING-001 数据合同

> 状态: 权威当前
> 最后核对: 2026-10-21；新增显式版本的隔离 VL 接缝，生产默认仍为 local-v3
> 适用范围: OCR 原始文字、版面行、内容分层、展示标题来源和视觉模型调用
> 事实来源: DEC-0007、媒体生命周期合同、当前处理作业/派生模型
> 冲突时以谁为准: 用户最新确认、AGENTS.md、迁移与运行时校验

## 不变量

下述行引用不变量适用于既有 local-v3/rules-v3。新增 local-vl-v1 的区块/行引用合同见末节，不能伪装为旧规则行。

`ocr_raw` 记录引擎读到的文字；`ocr_layout` 记录同一版本的行、归一化坐标和置信度；内容分层只引用行 ID。任何 headline 或实质文本均可回溯到其布局行、输入 Blob、处理版本和（如有）模型调用。

每个行分类只有：`PRIMARY_COPY`、`SUBSTANTIVE_TEXT`、`PLATFORM_WATERMARK`、`PLATFORM_UI`、`INCIDENTAL_SCENE_TEXT`、`BACKGROUND_DOCUMENT`、`UNCERTAIN`（数据库枚举以相应小写串存储）。`retained_line_refs` 是 `{lineRef, classification}` 对象数组，而非不可解释的字符串数组；`cover_headline` 必须能按 `PRIMARY_COPY` 行的阅读顺序精确重建。`UNCERTAIN` 不进入展示标题。

展示标题不是原始标题字段：`displayTitle.source` 必须是 `platform_title`、`cover_ocr` 或 `unknown`；只有原始标题为空、存在可回溯的 `PRIMARY_COPY` headline、且媒体槽位是显式 `cover` 或已知顺序的前 3 张图片之一时才能使用 `cover_ocr`。结果可为 `ACCEPTED` 或 `PARTIAL`：后者只表示图片仍有未决文字，不能抹杀已被行级证据支持的封面标题。没有已知顺序的正文图不猜测为“第几张”。

视觉模型输入为原始图片的受控缩放副本、OCR 行和候选 block；模型输出仅允许行 ID 与受限分类枚举。输出出现未知行 ID、生成新文案、无效 JSON 或没有视觉能力的配置时，整次分层结果拒绝并保持 `NEEDS_REVIEW`。

## 语料与读取资格

`ocr_raw` 是原始机器观察，不等于清洗语料。RESEARCH-DOSSIER-001 rules-v2 允许 `ACCEPTED` 或 `PARTIAL` 中有明确行来源的 `SUBSTANTIVE_TEXT` 进入受控读取；未定用途的 `UNCERTAIN` 仍排除。主文案独立按 `PRIMARY_COPY` 回查。旧 rules-v1 结果保持原有整张 ACCEPTED 的资格，不能借新规则把历史未知行全部升级。派生正文与标题必须由各自选定行精确重建。

规则升级仅追加分层结果，不需要重新识字。详见[研究底稿合同](research-dossier.md)及[第一阶段计划](../plans/active/research-dossier-001.md)。

## OCR-TITLE-QUALITY-454：显式本地 VL 观察与采用

范围与证据沿[执行计划](../plans/active/ocr-title-quality-454.md)和 #464。新版本 `local-vl-v1` 复用原有媒体作业、租约、derivative、layout、原行和底稿读取；0142 只增加附着既有结果的 append-only 采用表。默认作业仍为 local-v3，不自动重排、安装模型或启用 worker；当前只在自有隔离库验证，未合并部署或接入共享运行。

输入绑定作品、图片槽位、原图 SHA、尺寸与原序号。原图、PP-OCRv4 原始 JSON/多边形、完整 VL 原始响应、Paddle 后处理区块、区域结束原因、用量、错误类别及实际请求图 SHA 分开保留；Paddle 截掉重复后不能冒充原始生成正常。仅有块框时只提供块级定位，不生成逐字坐标。原始平台标题不改；封面候选独立于图片正文，内页标题与段落按图片/区块次序组成图片正文，不全局去重，不据 OCR 确认作者归属。

当前采用规则为 `ocr-adoption-v4`。v1 保存结果回放、v2 本地中间实测、v3 完整链路诊断均保留为历史，不能据最新规则覆写原报告。指纹绑定规则版本、完整类型化观察及两路原制品 SHA；调用者仍须核实原图/原件的真实字节与访问资格。相同模型的再次识别、两路文本一致及引用重建都不等于原图真值。

| 层 | 状态 / 使用方式 |
|---|---|
| 原生成质量 | supported / needs_review / rejected，保留逐块理由与真实 finish_reason；缺少必需结束原因、截断、错误、原生成长重复均不能直接采用 |
| 采用决定 | adopted_block / adopted_line_fallback / pending / excluded；与识别质量分开，识别一致也可能是 UI 计数，不成为正文 |
| 回退文字 | fallback_candidates 单列；旧 OCR confidence 高不自动可信，需绑定同原图/块/原框的有限局部复核后才能进入 usable_fragments |
| 封面标题 | usable / none / uncertain / not_cover / unavailable；仅显式封面或已知序号1可选，允许无标题，不把内页小标题当作品主标题 |

行支持仅在同图、至少60%行框重合、confidence≥0.70的范围比对；同块按视觉行、行内从左到右核对，保留原返回序号。NFKC/字母数字归一化仅比对文字支持，不认证标点或语义。局部行识别可以证明旧行漏字；同 VL 重读只允许有既有文字锚点的有限补充，不允许删除/改写原锚点。不能确认的表格/评论/手写和错字仍留候选。重复头像截图中的孤立短数字保持用途不明，不能因两路读数一致进入正文；普通数字材料没有该截图上下文时仍保留。

每个可用片段绑定原 block/line ID、原图像素框及 Unicode scalar 半开范围。拼接仅加换行，不改原片段；全作品投影保留每图、pending 块和逐段图片回链。`sourceRelation=image_transcription_not_author_claim` 始终表明这是图片机器转录，不是作品作者的亲历/事实证明。derived_text 只写采用投影，raw bundle 独立按 SHA 保存，拒绝结果也不覆盖原件。权限/退休/cutoff 仍由既有媒体读选择约束。

本地 helper 只接受显式本地模型目录和字面 loopback 端点，使用已有完整 PaddleOCR-VL 布局/阅读序/区域识别。固定依赖为 paddleocr 3.7.0、paddlex 3.7.2；进程内钩子捕获原响应，不改安装包。每请求45秒/最多4096 token，整图180秒、stdout上限8MB，每图最多6个疑点区块（每区至多行裁剪及 VL 裁剪各一次），不无限重试；失败诊断只含固定类别，不打印图片内容或请求。实际模型版本/权重、加载与资源消耗还须由部署包核实，M4 4bit 实测不是其他生产环境性能。

隔离回放入口 `ocr_adoption_replay` 默认接 ImageInput JSONL，`--bridge` 校验保存的原始 bridge bundle 后回放；均无模型/网络/数据库动作。`scripts/test-ocr-adoption-postgres.sh` 默认仅合成数据；`--private-local` 才接显式本地材料，CI 拒绝私有模式，不上传真实图片/文字。临时 PostgreSQL 使用自有容器/卷并清理。安装/默认版本切换、共享迁移、正式展示接缝和 Mog/G4 业务签收仍属后续受控范围，不能由这些隔离证明代签。

### v4 公式与截图正文边界

纯数字/变量、数学运算与等式/比较符组成的区域保留原文、坐标、原行与候选，但扁平字符一致不能证明上下标结构；标记 `mathematical_layout_unverified`、`pending`。大号候选标题为 `uncertain`，不补写数学含义；同字符裁剪回退也不能绕过该边界。含自然语言的数字标题不因此一律排除，这不是通用教学板分类器。

重复头像截图中，头像上半部对齐且落在其高度内的短行可依几何排为昵称；轻微重叠到下方正文不再构成排除依据，正文仍经过原有文字支持/结束原因检查。日期、孤立计数、展开回复与翻译/显示原文控件仍单独处理；待定原块、原行候选和不确定状态保留，不因为两段恢复采用而给整图完成标记，也不推断作者归属。
