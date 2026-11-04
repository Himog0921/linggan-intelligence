# 单篇结果的字段职责与标签范围

> 状态: 权威当前
> 最后核对: 2026-11-04
> 适用范围: 单篇分析 method.v0.3 / result.v1，未部署
> 事实来源: 两层合并决策、切片 A 实现与 N03 正反例验证
> 冲突时以谁为准: 用户当前要求、原始材料及当前接收代码

## 同一份结果

| 字段 | 职责 |
|---|---|
| understanding | 讨论、对象、情境、作者回答；offeredValue只写内容提供物，如规则、步骤、例子 |
| features | 材料中具体的表达与组织做法，category为answer_form、expression或participation |
| feedback | 评论者和作者表达了什么，保留角色、自述与解读归属 |
| explanations | 由特征提出读者可能的理解、兴趣或参与作用 |
| summary / tags | 压缩和索引已有内容理解、特征与反馈，不产生新判断 |
| methodCandidates | 可迁移的内容表达、比较、证据判断或选题方法及条件 |
| evidence / limitations | 精确原句；影响本次判断的材料缺口 |

observation表示可观察的材料或说话行为；author_claim表示作者提出的观点、归因或效果；interpretation表示本次有依据的归纳或推测。例如“标题使用训练称谓”是观察，“作者认为训练有效”是作者主张，“该称谓可能引起兴趣”是推测。其他评论者的说法可用observation记录“评论者说了X”，真实性只到说话行为为止。

audience回答内容面向谁；评论者的姿态或行为归反馈与解释。有依据的人群推断可保留interpretation，未取得依据时填null。回复记录保留已知身份范围：身份未确认时使用“一条/另一条回复”，不据评论ID判断同人或不同人；记录自述、数字和尝试意向时保留其原本含义。正文、OCR、ASR依来源说明，不从碎片补出原媒体结构。

## 解释如何写

explanations.text给出具体特征与读者可能作用之间的联系；support写已确认的前提及该联系的理由；uncertainty写同一假设还缺什么支持。三个字段围绕同一个判断，不用通用的“材料有限”替代关键未知。

解释可以产生新理解，但前提不能借“可能”变成事实。例如“参数可能帮助读者理解任务”是读者作用假设；“可能帮助读者看懂这些数字是实际成绩”已经偷偷确认了成绩。作者策略的现实效果留在作者回答/主张中，方法候选研究其内容写法。

## 摘要与标签

summaryBasisIds和tags[].judgmentIds仅可指向understanding、features、feedback中的id。标签不直接引用explanations或methodCandidates。解释假设仍可阅读并成为方法候选依据；首版检索标签不承担机制假设检索。

label和retrievalUse与依据保持相同主体、对象、程度、说话者归属及未知。retrievalUse只说明筛选用途，例如“查找回复者为数字补充含义的讨论”，数字用途未知时按“出现数字文本的评论”筛选。已观察回应归response，笔记明确的互动设计归participation。

| dimension | 内容 |
|---|---|
| discussion | 具体讨论 |
| audience | 有依据的面向对象，推断保留性质 |
| situation | 具体情境 |
| offered_value | 内容提供物 |
| answer_form | 回答或展示形式 |
| expression | 表达特点 |
| participation | 材料可见的互动设计 |
| response | 已观察回应或明确归属的评论说法 |

维度不要求填满。标签是本篇候选，不创建正式Topic或虚构词表ID；未知状态不作为语义标签。

## 执行结构

按调用提供的JSON Schema输出note-analysis.result.v1。discussion、answer和summary成功时必有；audience、situation、offeredValue可null；其余集合可为空。所有判断id唯一。

- Statement为{id,text,kind,evidenceRefs}；Feature增加label、category。evidenceRefs非空。
- Explanation的kind固定interpretation；非空featureIds关联相关特征，feedbackIds可空，evidenceRefs非空，另给support和uncertainty。纯反馈缺口留在feedback或limitations。
- Tag为{label,dimension,judgmentIds,retrievalUse}；非空judgmentIds只取上述三类基础判断。基础判断仍可能是interpretation，引用范围不是事实真伪白名单。
- Method的kind固定reuse_idea；basisIds可引用基础判断或explanations，非空；text写内容/研究做法，conditions写迁移条件，openQuestion可null。
- summaryBasisIds非空，只取基础判断；摘要是其压缩。
- Evidence仅输出{id,inputCitationId,quote}。quote逐字复制对应sources[].text中唯一匹配的短句，包括错字、拼音替字与标点；相邻片段分别引用。程序解析正式身份、版本和字符位置。

## 验收与下游

程序检查结构、引用范围和精确原句；人工沿证据到判断再到摘要/标签检查含义是否保留。结构通过不代表语义通过；原句存在不代表所有解释都成立。保留有用分析，不以删除全部解释或重复免责声明替代工作。

单篇分析供后续按用途取字段。主题图谱继续负责跨篇归属，评论研究回到目标原声；本方法不执行消费者、归并或采集，也不将上游解释变成独立证据。具体投影遵循切片A结果与接口合同。
