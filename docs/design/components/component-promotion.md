# LIDS-CMP-GATE-001 · Intelligence Component 晋升规则

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: Linggan 从 PAGE 局部块、LIDS Primitive 或受控候选名称，晋升为正式跨页面 CMP 的规则
> 事实来源: [LIDS-SYS-001](../lids/system.md)、[LIDS-PRI-001](../lids/primitives.md)、[LIDS-AGENT-001](../lids/agent-execution-guide.md)、UI 执行合同与 PAGE-TOPIC-001
> 冲突时以谁为准: 用户最新确认、产品/数据/权限合同、当前 SCOPE、已批准 PAGE 和真实跨页面使用证据；本文件不授权组件代码

LIDS 给出的是稳定的责任语言，不是“看起来像组件的东西都已经有了组件库”。同类正式组件全站只能有一个实现；但一次静态参考页也不能把局部结构提前宣布为正式实现。

## 当前 DESIGN-002 候选

| 局部块 / LIDS 名称 | 当前状态 | 现在只负责 | 仍不能声称 |
|---|---|---|---|
| Topic Identity / Topic Header | Page-local candidate | 合成 Topic 身份、任务与边界 | 所有 Topic 的正式 Header |
| Sample Window | Page-local candidate | 切换本地演示文本 | 真实时间筛选或日期控件 |
| Raw Voice 样式块 | Page-local candidate | 标识的合成短片段与限制 | 已能安全呈现真实原声 |
| Observation / Candidate / Boundary | Page-local candidate | 分开样本、推测与未知 | Claim、Signal 或正式 Intelligence |
| Local Intent | Page-local candidate | 无副作用界面回显 | 已持久化 Decision/Action |
| Button、Status、Surface、Readout | LIDS Primitive contract | 未来基础语法 | 已有代码库/正式组件实现 |
| `RawVoiceStrip`、`EvidenceChain` 等 LIDS 名称 | Controlled candidate name | 未来规格讨论的统一词汇 | 本仓库已经拥有正式 CMP |

## 进入 CMP 的硬门

一个候选只有同时满足下列条件，才可以分配 `CMP-xxx` 并使用 [组件规格表单](../templates/component-spec-form.md)：

1. 已在**第二个独立页面**被验证，且两页承担同一用户可见责任，不只是外形相似；
2. 唯一职责、明确不负责什么、L1/L2/L3 适用范围和禁止场景已经稳定；
3. 每个输入、空值、Truth/Coverage/Validity/Freshness/Operation、权限/可见性、错误和真实行动边界都有产品或数据来源；
4. 已查重，确认没有正式同类组件；若有，必须升级原组件或形成替代/废止计划，不能创建第二实现；
5. 已有交互、键盘/Focus、屏幕阅读器、触屏、Reduced Motion、响应式和性能要求；
6. 已列出受影响页面、允许/禁止变体、预览和自动/视觉验收；
7. 跨页面走查证明复用不会压平两页不同的事实、权限或行动语义。

达不到任一条件时，结构留在 PAGE 规格。可以被复制为研究样本，但不能借“一致性”之名要求其他页面复用，不能把数据请求、领域判断、权限判断或真实操作隐藏到组件中。

## 生命周期和变更

生命周期为 `EXPERIMENTAL → PROPOSED → STABLE → DEPRECATED → REMOVED`。新的 CMP/变体/替代/删除同一事项必须更新 CMP Spec、受影响 PAGE、预览、LIDS migration log 和验收记录。改变数据资格、行动后果、权限、用户任务或视觉 Token 时按最高风险类别停下并升级，不能伪装为普通重构。
