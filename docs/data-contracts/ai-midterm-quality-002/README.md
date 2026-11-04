# AI-MIDTERM-QUALITY-002 单篇分析 Skill 与离线试验

> 状态: 草案
> 最后核对: 2026-11-04
> 适用范围: 单篇分析候选方法、切片 A 合同及离线验证；未部署
> 事实来源: 当前源码 f34dc08a、N02/N04/N06 本地阅读分析与本包离线回执
> 冲突时以谁为准: 用户当前要求与既有来源/权限合同；本草案不改变正式领域身份

## 已冻结版本

用户已确认最后收口后固定版本。当前基线为[note-analysis-baseline-20261104](frozen-baseline.md)，Skill、提示词与结果职责按此冻结；已知自然语言缺口保留，不主动重开迭代。当前仍是未部署的本地候选。

## 当前入口：切片 A 两层合并结果

[结果与接口规范](slice-a-result-and-api.md)及[候选 Skill](skill/linggan-note-analysis/SKILL.md)是当前入口，方法标识 v0.3。2026-11-04 已完成 N02/N03/N05/N08 四篇 DeepSeek 首次试跑；质量问题促成本次边界、确定性和解释关联规则修订。原始输出与离线人工校订分开保留，后者不是再次模型生成，也不代表质量已由用户验收。实施与验证状态见[实施计划](../../plans/active/single-note-analysis-minimal-implementation.md)。下文 v0.1/v0.2 为历史演进记录。 当前候选已落实字段职责重写：offeredValue记录内容提供物，解释记录读者作用假设，摘要与标签压缩基础判断；Rust标签引用范围已收窄，方法仍可引用解释。N03正反例仅验证合同与检查边界，不代表真实模型效果。

## 2026-11-03 用户纠偏后的当前主线

笔记入库后先调用Skill做系统分析与打标，形成供后续模块使用的单篇结果。主题图谱归并属于下游；已完成的容量/配对复现只保留为后续风险证据，不作为本阶段单篇分析的前置任务。

下文candidate.v0.1是已验证结构边界的实验草案，不是已确认的完整业务合同。下一轮先补清具体议题、情境、回答、提供的价值、内容特征与标签用途，再修订Schema。当前四个标签维度不是最终词表；逐条评论覆盖只作为本批实验的校验条件，不自动成为单篇研究的生产必经步骤。已有评论可形成回应概览与追问线索，深度提取/问题归并由后续模块负责。没有评论不阻塞单篇分析。

## 历史可评阅产物：单篇分析方法 v0.2

[Skill 方法](skill/linggan-note-analysis/SKILL.md)与[输出内容、标签规则](skill/linggan-note-analysis/references/output-guide.md)现已更新至 v0.3；以下记录的是 v0.2 业务样稿。N02、N04、N06 的完整人读分析、结构化同义结果与来源检查存于本工作树 `artifacts/private/ai-midterm-quality-002/single-note-v02/`；README.md 是评阅入口。真实材料不进入 Git。

三篇样稿由 Agent 基于已有材料编写，另以 N04 做了一次未见前稿的独立 Skill 试读。它们不是项目供应商调用结果，也不是用户已验收的标准答案。引用校验仅验证来源和引句存在；业务价值由实际内容评阅判断。

v0.2 补足内容理解、回答、价值、表达、已有反馈、检索标签与使用判断；旧 v0.1 结构和 B0/B1 请求保留为历史实验，不能直接声称覆盖 v0.2。下一次对照前需同步修订，不在本轮执行。归并修复保持后置。

## 工作包与范围

用户要求基于三篇解读收敛首版输出合同、B0/B1，对主题64候选及评论配对做离线复现。此包只新增候选合同、离线工具、合成验证和报告，不修生产逻辑，不新增线上状态机或迁移，不接真实供应商。root负责合同/实验；只读架构协作者承担独占的Rust复现脚本。独立工作树分支 codex/ai-midterm-quality-002，基底 f34dc08a。所有真实材料、冻结输入、Agent参考输出和测试回执只存忽略的 artifacts/private/ai-midterm-quality-002/，不进入Git。

## 1. 首版输出合同

唯一可执行结构定义为 [contract.cjs](../../../scripts/ai-midterm-offline/contract.cjs)，导出 JSON Schema draft-07；[validate.cjs](../../../scripts/ai-midterm-offline/validate.cjs)补充身份、引用和角色关系校验。这是实验用的候选，不作为生产验证器直接部署。

| 字段 | 责任 | 约束和实际用途 |
|---|---|---|
| contract/sampleId/inputFingerprint | 锁定合同与输入身份 | 由程序提供，模型回显；本地分享ID不等于真实work_ref |
| status | 表达当前输出可读范围 | analyzed / partial / no_readable_material；不是任务执行状态或可信度分数 |
| claims | 保存可定位的具体判断 | observation / author_claim / hypothesis / inspiration 四类；每项有原文短引句和来源ID |
| reading | 选出概览和特色对应的claim IDs | 页面可以复用同一主张，不另生成无依据摘要 |
| comments | 逐条覆盖本次输入评论 | 每条一次；作者回复强制author_response；目标引用不得借父文补证据 |
| tags | 少量候选检索特征 | situation / answer_form / interaction / material_form；含claimIds与retrievalUse，不产生正式Topic身份 |
| uses | 能支持什么研究/创作动作 | 引用claimIds并说明条件，不能凭空编经历或授权发布 |
| limitations | 缺口具体影响什么判断 | reason + affects，不以笼统“材料不足”否定所有可用部分 |

claims最多24项、tags最多8项、uses最多5项。仅为本次小样本的输出控制，不是最终生产容量。comments最多100条；输入超出时本轮实验明确拒绝，不截取前100，不把它当未来全库策略。没有机制增量时hypothesis可以为零；存在hypothesis必须给替代解释，inspiration必须给条件。

comment responseType 采用 author_response / intention / reported_result / question / evaluation / social / ambiguous / other。可选 measurement 区分 elapsed_seconds、remaining_seconds、progress、unknown；unknown的值必须null，其余值不得null。它服务本批数字回应反例，不据此自动判断临床能力。类型与数值含义仍需语义审阅，程序不会靠关键词替模型做通用判断。

计数、作者角色、平台指标、采样范围、来源版本、方法版本、调用账与生成时间由程序持有。模型不重新生成评论总数、独立人数、费用或平台快照。本轮先不提供指标，以减少事后合理化；后续表现观察直接组合真实快照，不先让模型再猜一遍。

生产接入时 envelope 复用现有 task config、Prompt/Skill 版本、invocation 与 canonical 来源资格。本地inputFingerprint是实验快照字节身份，不能直接取代正式材料指纹和权限检查。旧分享编号也不能用于写回来源坐标。

## 2. 校验能保证什么

离线结构与关系校验能拒绝：跨样本／错输入指纹、未知来源、伪造引句、重复评论、评论遗漏、父文冒充目标证据、作者冒充用户、unknown写0、标签无主张依据、假设无替代解释、启发无条件、未批准的额外正式对象字段。

它不能确定：真实引句是否足以支持结论、作者主张是否被偷换成事实、报进度是否被说成报秒数、标签是否真的有检索价值。三个故意错误的合成答案仍能结构通过，已记录为“结构能力边界”，不会记为语义PASS。

所有正常fixture与反例都必须报告实际结果。3篇Agent参考答案属于手工式本地方法样稿，既不是旧线上结果、也不是B0/B1模型结果，更不是人工Gold。引用存在率不替代用户有用性验收。

## 3. 固定输入

[freeze-inputs.cjs](../../../scripts/ai-midterm-offline/freeze-inputs.cjs)校验原分享manifest后，冻结N02/N04/N06。保留已分享的平台标题、全部可读非gap区块、阅读结构、作者/父评论角色和每条已采评论；gap作为缺口记录，不混作原文。不带平台指标、不带原图片字节，不新增材料获取。

| 样本 | 来源单元（含1个标题） | 本次评论记录 | 剔出的诊断gap | 条件 |
|---|---:|---:|---:|---|
| N02 | 5 | 6 | 0 | 正文＋人工校对封面文字；未提供原封面图 |
| N04 | 32 | 30 | 0 | 正文＋题面OCR＋ASR；本次模型条件不看原图/视频 |
| N06 | 74 | 8 | 3 | 多图文字；保留页序未知和部分低可靠内容边界 |

标题投影自分享JSON的note.platformTitle，来源标记为platform；新增T001只在本地实验中引用，不冒充canonical区块。三篇该note对象均无平台标签字段，本轮不补造标签。

没有把上一轮Agent看原图后的解释塞入方法输入。因此纯文本模型不能被要求复述它没有看过的空间布局。将来添加图片必须单设实验条件。

## 4. B0/B1对照

两个方案共享 [common.txt](../../../scripts/ai-midterm-offline/common.txt)、同一Schema、相同输入字节、模型配置、输出额度、媒体能力与评论范围，只改变方法指令。

- [B0](../../../scripts/ai-midterm-offline/B0.txt)：一次清晰阅读，说明内容、回答、特点、用途与限制。这是本次新基线。
- [B1](../../../scripts/ai-midterm-offline/B1.txt)：先具体处境和回答，再按需解释；逐条评论辨角色和回应类型；让评论修正判断，最后筛选有用途的标签。

共同约束同样保护B0，不能故意让基线没有来源/安全要求来制造B1优势。不加固定十镜头、不做模型投票、不重试挑最好答案。B1没有实际增益就保留B0。

### 待执行真实对照的预注册参数

三篇×两个方法＝6次初始调用；单次最多8192输出tokens、输入上限24000 tokens、一次尝试，不自动重试。按真实供应商分词/能力预检，超额先停并报告，不截断。总上界是144000输入＋49152输出tokens（不是已用量、不是费用授权）；共同系统文本、Schema也包含在输入额度内。相同思考等级、超时和供应商配置冻结，顺序以可复现随机种子交错；任何增补、审核模型或图像调用单独计，不计入“免费步骤”。

本轮未选定可真实调用的模型，也未承诺费用。之前数据库中的模型名称、手填能力与unlimited不等于已通过当前供应商可用性/价格验证。需要发起真实试验时，再提出精确模型/endpoint、三篇最终脱敏输入哈希、允许数据类别、六次调用和货币费用封顶；未知外发停止，不自动重发。

本轮冻结包是内部研究用，含家庭/健康相关评论；它不是默认获准的供应商请求。尤其N06课程性质评论仍含健康背景，外发前先决定必要性与最小化，变更后同时重冻B0/B1及引用映射，不悄悄删句继续沿用旧指纹。N07和留出正文未纳入。

### 比较怎么判

评审输出隐藏B0/B1名，保留单独映射。当前Agent已参与方法与样稿设计，不能自称真正盲评；由未见方法的独立评审或用户校准补足。逐项使用0/1/2＋理由，不汇总成隐藏严重错误的总分。

| 维度 | 必查内容 |
|---|---|
| 忠实与角色 | 事实、作者自述、尝试意向、实际自报结果分开 |
| 具体性 | 是否说清本篇回答方式，换另一篇后是否仍能原样套用 |
| 解释边界 | 有依据及替代解释，不强造机制，不确认疗效或资质 |
| 评论增量 | 是否改变或补足正文理解，是否保留反例与未知 |
| 工作价值 | 能否通过标签找到对应材料，并支持一次选择或后续研究 |
| 可追溯 | 引用存在＋语义支持分别评审，不以结构通过替代后者 |

本批硬性语义反例：N02“准备尝试”不能变成有效反馈；启动困难与家长介入边界不能自动并成同一问题。N04“才数到18”不是18秒、裸数字单位未知、作者回复不作独立需求。N06标题称谓不证明专业身份，年龄回应含混必须保留，材料正文为空不等于没有可读内容。

选择B1的条件：至少在一个预注册错误或明确检索任务上较B0有可说明的改善，同时不引入新增严重错误，并报告长度、调用、tokens与耗时成本。只更长、分段更多、措辞更漂亮不算改善。若两者都严重错误，均不采用；修订后新开轮次，保留原失败。

先不加B2。N09–N12维持未读，方法冻结与作者/近重复分组完成后才开启留出检验。

## 5. 重跑与证据入口

```bash
node scripts/ai-midterm-offline/freeze-inputs.cjs /path/to/share artifacts/private/ai-midterm-quality-002/offline
node scripts/ai-midterm-offline/check-inputs.cjs artifacts/private/ai-midterm-quality-002/offline
node scripts/ai-midterm-offline/check.cjs /path/to/ajv artifacts/private/ai-midterm-quality-002/offline/contract-checks.json
python3 scripts/ai-midterm-repro/run.py
```

本次使用现存Ajv 6.15.0只读模块，没有安装网络依赖；重跑需显式提供兼容模块路径。Rust复现使用本机cargo offline与自有target，不触碰运行树或其他编译目录。复现边界见 [说明](../../../scripts/ai-midterm-repro/README.md)。

当前结果与问题处置见 [离线回执](../../audits/ai-midterm-offline-validation-2026-11-03.md)。本合同仍待模型试验、生产接入与用户业务校准，不宣称已上线。
