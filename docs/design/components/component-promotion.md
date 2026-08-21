# Intelligence Component 晋升规则

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: Linggan 由 Reference Page 发现、验证、晋升或替代 UI 组件的规则
> 事实来源: Issue #7 的用户确认、`docs/design/design-governance.md`、`docs/agents/ui-execution-contract.md` 与 PAGE-TOPIC-001
> 冲突时以谁为准: 用户最新确认、产品/数据/权限合同、已批准页面规格和真实跨页面使用证据

## 核心规则

组件库不是把常见外形预先命名的仓库，而是反复出现、输入和状态语义稳定的界面责任集合。第一张 Reference Page 可以暴露候选，**不能单凭一次使用宣布它们为跨页面公共组件**。

## 当前候选：仅限 PAGE-TOPIC-001

| 局部块 | 当前职责 | 当前状态 | 还不能声称 |
|---|---|---|---|
| Topic Header | Topic 身份、定义版本、页面任务和模式旗标 | Page-local candidate | 可用于所有 Topic/Domain 页面 |
| Sample Window | 切换合成样本的阅读窗口 | Page-local candidate | 是真实时间筛选或通用日期组件 |
| Observation Panel | 组织有限样本观察 | Page-local candidate | 具备 Evidence 查询/排序能力 |
| Evidence Quote | 呈现带限制的合成短片段 | Page-local candidate | 可安全展示真实原文或已具备来源合同 |
| Narrative Cluster | 组织“内容怎样讲”的示例类别 | Page-local candidate | 是市场分类、统计或 Topic 定义 |
| Candidate / Boundary Panel | 分开候选解释与来源不足 | Page-local candidate | 已产生 Claim、Signal 或正式判断 |
| Local Intent Panel | 演示下一步意图与无副作用回显 | Page-local candidate | 已持久化 Decision/Action 组件 |

基础控件（按钮、标签、分隔、可折叠区域）也只在本页以视觉实现存在，尚未选择代码层的组件库或技术框架。

## 晋升为 CMP 的门槛

一个候选只有同时满足以下条件，才可获得 `CMP-xxx` 并填写 `templates/component-spec-form.md`：

1. 已在第二个独立页面使用，且两个页面承担的是同一种用户可见责任，而非仅外形相似；
2. 每个输入、展示状态、权限/可见性和错误/未知边界都有产品或数据来源；
3. 页面没有为了复用而把领域判断、权限判断、数据请求或真实动作藏进组件；
4. 已列出受影响页面、允许/禁止变体、可访问性和替代计划；
5. 有跨页面走查证据，证明复用没有压平两页不同的事实语义。

未达到门槛时，局部块留在 PAGE 规格中。它可以被复制为研究样本，但不能以“设计系统一致性”为由要求其它页面直接复用。

## 三层稳定性

| 层 | 何时可以稳定 | 例子 | 当前 DESIGN-002 结论 |
|---|---|---|---|
| Primitive | 键盘、视觉和基础互动稳定；不携带产品判断 | button、disclosure、标签 | 可使用视觉约定，未选择代码库 |
| UI Pattern | 相同交互/状态责任在多页成立 | 窗口选择、空/未知提示 | PAT 可先存在；实现仍待第二页验证 |
| Intelligence Component | 输入、资格边界和用户含义在多页保持一致 | evidence、source、coverage、candidate | 当前全部仅为候选，未晋升 CMP |

## 变更与废止

页面局部块的修改跟随 PAGE 规格。已晋升的 CMP 修改必须先列出受影响页面；若改变数据资格、行动后果、权限或用户任务，停止并按最高风险类别处理。新组件替代旧组件时，旧规格保留替代指向，未迁移页面不得被文档升级掩盖。
