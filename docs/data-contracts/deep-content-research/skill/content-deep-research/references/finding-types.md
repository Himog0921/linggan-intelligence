# Finding Type：定义与例子

> 状态: 草案
> 最后核对: 2026-11-16
> 适用范围: L2 Skill content-deep-research 首个版本（content-understanding.l2.model.v2），待 Mog 评审，未导入运行库
> 事实来源: Deep Content Research V0.3 附录 A
> 冲突时以谁为准: Mog 评审意见；payload 字段名以 deep_v2.rs 为准

以下例子都是合成的。

## hook：标题、封面、开头的切入设计

合格（observation）：标题写「孩子写作业磨蹭到十点」，正文第一句「昨晚又是十点」接着同一个时间点展开。
不合格：标题很吸引人，抓住了家长的痛点。（没有具体内容；“抓住痛点”是效果判断）

## structure：组织方式及各部分的作用

合格（observation，带 order）：正文依次给出「先把书包放到桌上」「再定一个十分钟」「最后只写第一题」三个动作。
不合格：文章结构清晰，层层递进。

## value_delivery：实际交付的内容

合格（observation）：正文给出三个可以直接照做的启动动作，并写明时长「十分钟」。标题承诺的「不吼不催」在正文中没有对应的做法。
不合格：这篇提供了很有价值的育儿方法。

## expression_mechanism：具体表达手法

合格（observation）：作者用「像热车一样」比喻开始写作业前的准备动作。
合格（interpretation，basisFindingIds 指向上一条）：这个比喻把“准备动作不是浪费时间”说成了一个日常经验。
不合格：语言生动形象。

## engagement_hypothesis：对读者反应的假设（性质只能是 hypothesis）

合格：结尾「你家孩子卡在哪一步？」向读者要具体信息。
mechanism：要求读者描述自家的具体情况，比泛泛的提问更容易得到回答。
observableSignal：同一账号中以具体提问结尾的笔记，评论率高于其他结尾方式。
不合格：这种结尾会带来很多互动。（因果断言，无法检验）

## transferable_method：可迁移的方法

见 modes/creation-method.md 中的合格例子。
