# LIDS-AGENT-001 · 按任务展开的执行入口

> 状态: 权威当前
> 最后核对: 2026-10-19
> 适用范围: 所有 UI Agent 的开始、模块读取、组合与交付；正文规则由对应分册维护
> 事实来源: Mog 本次 LIDS-ALIGN-001 交付授权、已确认对话决定、现行 LIDS 与 main@9ff50b81；逐项来源见 review-alignment.json；LIDS-ALIGN-001 本次授权与对齐记录
> 冲突时以谁为准: 用户最新明确确认、产品/数据/权限合同与当前 SCOPE；已采纳设计按本文及明确替代关系执行，候选不自动生效


## 开始

先读 [UI Agent 渐进执行标准](../../governance/ui-design-execution.md) L0；再按其中 L1 触发表选择分册，L2 进入当前 PAGE/CMP/产品与数据合同，L3 取必要证据。不一次加载全部页面、48个候选或全部历史参考。

执行链保持 `Token → Primitive → Component → Pattern → Page`，Motion / Scene / Data Truth 横向约束。LIDS 成熟度仍为 PROPOSED，已确认规则可以生效，未确认候选、真实实现和用户验收状态独立。

## 任务内决策

1. 明确页面任务、对象与强度；按 [regions](regions.md)安排最小职责区域，按 [patterns](patterns.md)选择已有模式。
2. 查询既有共享实现及正式CMP，核对 [primitives](primitives.md)的输入/按钮/FIND/[焦点](primitives.md#焦点环是一个全局-token)规则；候选名称不能代替可复用代码。
3. 需要字体时先读 [typography](typography.md)；需要值时读 [tokens](tokens.md)。字体族和B字级已确认；角色值只读typography.json，运行时镜像不随文档自动变化。
4. 需要壳层/材料时读取 [shell-zones](shell-zones.md)/[materials](materials.md)；保留领域位置、文字避让、正文白场，候选纹理数值不自动推广。
5. 按 [interaction-contracts](interaction-contracts.md)、[data-boundaries](data-boundaries.md)、[language-policy](language-policy.md)核对状态、键盘、返回、来源、中文和真实回执。
6. 冲突按 [review-items](review-items.md)记录 Q编号；已确认部分继续执行，不重问已有决定；产品、权限和未决视觉默认不补造。

## 交付前

- 当前页面/模式、区域与作用对象可定位；没有无必要标题、统计或空白装饰带。
- 关键常态/空/错/部分/受限/未确认有明确来源；单行任务不压缩成小字或丢失关键状态。
- 鼠标、键盘、窄屏与需要的缩放有等价路径；表格局部横向滚动与整页溢出分开验收。
- 实现、运行、视觉与业务验收分别给证据；已选字体不等于正式字体已加载，合成确认不等于真实命令成功。
- 按渐进执行标准的三项检查验证文档；业务变更另运行当次适用检查。未执行写 NOT_RUN，不以构建替代用户验收。
