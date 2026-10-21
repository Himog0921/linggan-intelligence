# OCR-CONTENT-LAYERING-001 数据合同

> 状态: 权威当前
> 最后核对: 2026-10-21；新增离线规则，生产分层语义未变
> 适用范围: OCR 原始文字、版面行、内容分层、展示标题来源和视觉模型调用
> 事实来源: DEC-0007、媒体生命周期合同、当前处理作业/派生模型
> 冲突时以谁为准: 用户最新确认、AGENTS.md、迁移与运行时校验

## 不变量

`ocr_raw` 记录引擎读到的文字；`ocr_layout` 记录同一版本的行、归一化坐标和置信度；内容分层只引用行 ID。任何 headline 或实质文本均可回溯到其布局行、输入 Blob、处理版本和（如有）模型调用。

每个行分类只有：`PRIMARY_COPY`、`SUBSTANTIVE_TEXT`、`PLATFORM_WATERMARK`、`PLATFORM_UI`、`INCIDENTAL_SCENE_TEXT`、`BACKGROUND_DOCUMENT`、`UNCERTAIN`（数据库枚举以相应小写串存储）。`retained_line_refs` 是 `{lineRef, classification}` 对象数组，而非不可解释的字符串数组；`cover_headline` 必须能按 `PRIMARY_COPY` 行的阅读顺序精确重建。`UNCERTAIN` 不进入展示标题。

展示标题不是原始标题字段：`displayTitle.source` 必须是 `platform_title`、`cover_ocr` 或 `unknown`；只有原始标题为空、存在可回溯的 `PRIMARY_COPY` headline、且媒体槽位是显式 `cover` 或已知顺序的前 3 张图片之一时才能使用 `cover_ocr`。结果可为 `ACCEPTED` 或 `PARTIAL`：后者只表示图片仍有未决文字，不能抹杀已被行级证据支持的封面标题。没有已知顺序的正文图不猜测为“第几张”。

视觉模型输入为原始图片的受控缩放副本、OCR 行和候选 block；模型输出仅允许行 ID 与受限分类枚举。输出出现未知行 ID、生成新文案、无效 JSON 或没有视觉能力的配置时，整次分层结果拒绝并保持 `NEEDS_REVIEW`。

## 语料与读取资格

`ocr_raw` 是原始机器观察，不等于清洗语料。RESEARCH-DOSSIER-001 rules-v2 允许 `ACCEPTED` 或 `PARTIAL` 中有明确行来源的 `SUBSTANTIVE_TEXT` 进入受控读取；未定用途的 `UNCERTAIN` 仍排除。主文案独立按 `PRIMARY_COPY` 回查。旧 rules-v1 结果保持原有整张 ACCEPTED 的资格，不能借新规则把历史未知行全部升级。派生正文与标题必须由各自选定行精确重建。

规则升级仅追加分层结果，不需要重新识字。详见[研究底稿合同](research-dossier.md)及[第一阶段计划](../plans/active/research-dossier-001.md)。

## OCR-TITLE-QUALITY-454：保存结果上的隔离采用规则

2026-10-21 本切片见[执行计划](../plans/active/ocr-title-quality-454.md)。`ocr_adoption::assess` 是无 I/O 的确定性函数；尚未接入现有 worker、表、底稿或正式默认输入。上文旧分层/读取保持当前运行语义，不能把本段当生产已经升级或把新结果塞入 rules-v3。

输入必须带相同的作品、图片槽位、原图SHA与尺寸；布局和逐行观察各有原结果哈希与版本。调用者负责核实哈希所指字节及其来源/访问资格，函数只能校验输入之间的身份一致性、格式与范围，不能证明调用者没有伪造来源。原JSON/原图仍由原存储责任方保存；输出哈希不是替代原文件的证据。

布局保留上游标签、原始块文字、块级原图像素矩形与返回阅读顺序；逐行支持保留原行文字/分数、行级框，原多边形仍在原始JSON。不从矩形或均分文字反推逐字坐标。区域结束回执须绑定 blockRef 与输出文字SHA；缺失保留为未知。整图成功、顺序日志或无绑定的 `finish_reason=length` 均不能伪造为某块的结束回执。

规则版本为 `ocr-adoption-v1`。输入指纹绑定规则版本、完整类型化输入及两路原制品哈希；不是未来生产完整模型配置的自动指纹。区块质量分别为 supported / needs_review / rejected；supported 只表示当前检查及归一化逐行支持成立，不是原图真值或人工确认。

- 行支持在同图、至少60%行框面积重合、confidence≥0.70的范围内按已保存行顺序比较；NFKC/小写/字母数字比较仅检查文字支持，不证明标点、大小写或逐字准确。
- 不完整区域结束、32个连续同字符、足够长的重复短周期等阻止直接采用原VL块；普通image文字、未验证table结构、缺行支持或文字冲突保留待核验。启发式不承诺识别所有幻读；双模型一致也不是事实认证。
- 冲突或拒用时只以confidence≥0.75且无异常长重复的原行片段做显式回退；缺少支撑不补造。失败执行不返回伪成功回退。背景image、元信息等不自动成为正文。表格回退只保留行，不继承VL表格/章节关系。
- `usableFragments` 是有来源的**文字候选**，不等于作者正文、Corpus用途授权或C1准入。评论/截图/练字等即使可识别仍需既有来源/角色/用途判定。没有替调用者授权默认输入。跨块引用同一行时必须按来源识别重叠；跨图相同文字保留各自身份。

作品封面标题独立于 image_headings：仅明确cover或已知原始序号1可采用，内页不能上升为作品标题，顺序未知保持不确定。候选来自所有 text/doc_title/paragraph_title 块；当前最小字形高度4%、相对最大字形70%、连接间距1.5倍行高、竞争区块面积比1.8等均是首版可解释启发式，经固定开发集检验，尚无未见样本泛化结论。较小远区文字记上下文，不仅靠位置断言它必为字幕。

`coverTitle.state` 区分 usable / none / uncertain / not_cover / unavailable。none只说明当前已完成观察未形成合格突出组，不证明原图绝无标题；uncertain保留候选与理由而不填正式标题。展示补全 helper 始终优先返回非空平台原值，只在usable时供 `cover_ocr` 候选；不写回或静默覆盖平台字段，也不宣称已获Mog业务签收。此候选语义可窄适配既有cover_headline，当前无数据库写入适配。

采用文字/回退文字的引用是原块或原行 Unicode scalar 半开区间，附原框和 sourceKind。连接仅加入换行，原片段文字不修改；归一化只用于比对。精确重建只证明引用对应保存的机器观察，不能把机器观察冒充原图绝对真值。

离线入口：`cargo run -p linggan-evidence --example ocr_adoption_replay --locked < input.jsonl > output.jsonl`。每行一个ImageInput/Assessment，无模型、网络、数据库、自动重试或文件路径参数。单行上限8MB，坏记录非零退出且错误不打印原文；此前输出仅证明那些图片，不可当整批成功。真实测试输入/输出仅本地私有制品，CI只用合成用例。
