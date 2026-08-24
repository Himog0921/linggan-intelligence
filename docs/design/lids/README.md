# LIDS v2.0 · Linggan Intelligence 设计标准

> 状态: 权威当前
> 标准成熟度: PROPOSED
> 最后核对: 2026-08-25
> 适用范围: Linggan Intelligence 全部未来用户可见 Web UI、设计 Token、基础语法、组件、页面模式、动效、场景、状态表达与 UI Agent 协作产物
> 事实来源: Mog 于 2026-08-21 明确指定的 `Linggan_Intelligence_Design_System_v2.0`、AGENTS.md、docs/governance/、docs/current-state.md、已确认产品/数据/权限合同与当前 SCOPE
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决策与当前 SCOPE；LIDS 只约束设计表达，不能越权改写这些来源

LIDS（Linggan Intelligence Design System）是本项目唯一的设计表达标准。它不是“暖灰、ASCII 或未来感”的灵感集，也不是让 Agent 根据截图补全产品的页面模板；它将可用的设计决策固定为一条可复核的生产链：

```text
Token → Primitive → Component → Pattern → Page
        ↘ Motion / Scene / Data Truth（横向约束） ↗
```

**它现在已经生效的含义**：未来 UI 任务必须按照这条链选择规则、记录来源、复用已正式化的构件并通过检查；不能再自建第二套颜色、按钮、状态标签、容器语言或页面骨架。

**它尚不证明的事情**：LOCAL-001A 已有受限的 loopback Rust Web host、一个 Evidence Library 页面和唯一运行时 Token 源；这不证明真实数据或 Materials read model、通用组件库、L2/L3 页面、L3 3D 场景、外部 Agent 动作、部署或完整产品 Web 已成立。LIDS 的建议目录、React/Three/Blender 路线、V3 原型演示值及任何运行态文字均不是这些能力的授权。

## 先读什么

所有 UI Agent 先完成 [UI 协作 Agent 执行合同](../../agents/ui-execution-contract.md) 的读取，再按工作类型进入下列材料。没有被页面规格、数据合同或当前 SCOPE 支撑的内容，一律是 `DECISION_REQUIRED`，而不是设计发挥空间。

| 工作 | 必读材料 | 本次事项还必须补齐 |
|---|---|---|
| 新页面 | [system.md](system.md)、[patterns.md](patterns.md)、对应 PAGE 规格 | 产品页面、数据/权限/行动合同、UI Change Manifest |
| 新基础视觉或 Token 调整 | [tokens.md](tokens.md)、[system.md](system.md)、[migration-log.md](migration-log.md) | 唯一 Token 真源、影响面、验证与迁移记录 |
| 新/改组件 | [primitives.md](primitives.md)、[agent-execution-guide.md](agent-execution-guide.md)、CMP 规格 | 查重、数据与状态合同、预览、替代/废止计划 |
| L3 场景或动效 | [system.md](system.md) 的场景条款、[prototype-audit.md](prototype-audit.md) | 另行批准的技术、资产、性能、降级、真实状态与等价 HTML |
| 静态/合成参考 | 对应 PAGE、[agent-execution-guide.md](agent-execution-guide.md) | 首屏合成边界；不得伪造运行、来源或动作回执 |

## LIDS 的项目内权威边界

1. [system.md](system.md) 是 LIDS 在 Linggan 的主标准：定义产品气质、五层架构、L1/L2/L3、状态表达、动效、响应式、可访问性与治理门。
2. [tokens.md](tokens.md) 是 `LIDS-TOK-001` 的版本化规范与校验镜像；LOCAL-001A 的 `apps/api/src/local_web/lids_tokens.css` 已是唯一可编辑的运行时 Token 值源。两者的单向同步与 107 项名称→值校验规则由 `tokens.md` 明确规定；页面 CSS 只能消费 Token，不得另行声明主题。
3. [primitives.md](primitives.md) 和 [patterns.md](patterns.md) 是基础语法与页面组合的权威约束；它们不是现有代码实现的声明。
4. [agent-execution-guide.md](agent-execution-guide.md) 是给协作 Agent 的简明强制清单；它从主标准派生，不能与主标准冲突。
5. [prototype-audit.md](prototype-audit.md) 把下载包中的 Observatory V3 降级为 L3 视觉母题参考，防止旧单页原型、模拟数据、永久动画或假运行状态被误带入产品。
6. [migration-log.md](migration-log.md) 记录每次 LIDS 规则、实现、替代或例外的实际变化与验证边界；改动五层中的任一层必须同一事项更新它。

## 来源回执与不继承清单

本标准由 Mog 提供的本地包 `Linggan_Intelligence_Design_System_v2.0` 在 2026-08-21 核对后吸收。为便于后续复核，来源包的主文件校验值如下；原件不复制进本仓库，也不因本回执成为本仓库的产品事实来源。

| 来源文件 | SHA-256 | 本项目吸收的职责 |
|---|---|---|
| `design-system.md` | `8650c7b2a50634fa87e2031e06078584b4a6ea442fbfc44e0bf4afedfa67024c` | 设计表达、层级、状态、场景与治理原则 |
| `tokens.md` | `97fac0fb590c7f349f5fe7bfc2e2e423c6c8145b03cec9ffdd80d4494f757e77` | `--lgi-*` 数值与语义基线 |
| `primitives.md` | `e7d67dba088fea500d05ecfa45a62a290e4b3d07f121830f010a9e38cecf5b73` | 基础语法、控件与状态约束 |
| `patterns.md` | `94fe15b47e7d38b1be15b0369c143f51859a1aa79a1ca6c2dba0db9dfa2a2719` | L1/L2/L3 页面骨架 |
| `ai-agent-guide.md` | `4585d68ef800bb231c9135029cf0b486ca6d42860c43d7e21f5a7daefe5d6351` | Agent 决策树与自检门 |
| `prototype-audit.md` | `dc4bc89daee5ff5120bb35019851c9b1acb477a24f7d91416a9ed6a5a9733439` | 原型保留、删除、重做边界 |
| 组件/页面模板与迁移记录 | 来源包内已核对 | 统一纳入现有 LIDS 模板和变更流程 |

**明确不继承**：来源包任何模拟 Topic、样本数、覆盖数、时间、`LIVE`/运行状态、产品路径、旧原型 HTML、技术栈、目录、第三方资产、Blender/GLB/React Three Fiber/Three.js 选择或部署假设。它们只有在未来独立产品、技术、数据和资产授权全部成立时，才可以作为候选重新评估。

## 与现有手册的关系

| 现有位置 | 现在的作用 |
|---|---|
| [../design-governance.md](../design-governance.md) | 规定设计来源优先级、闭集执行、例外与交接；本目录受其约束 |
| [../templates/page-spec-form.md](../templates/page-spec-form.md) | 采用 LIDS Page 规则的唯一 PAGE 规格表单 |
| [../templates/component-spec-form.md](../templates/component-spec-form.md) | 成为跨页面 CMP 前的唯一规格表单 |
| [../components/component-promotion.md](../components/component-promotion.md) | 防止一次参考页把局部块误报为正式 LIDS 组件 |
| [../pages/topic-intelligence-reference-page.md](../pages/topic-intelligence-reference-page.md) | 首个 L2 合成参考页；只验证 LIDS 的局部设计表达，不代表真实产品页 |

## 标准状态与升级门

LIDS 对协作执行的约束已经是“权威当前”，但其系统成熟度保持 `PROPOSED`。LOCAL-001A 的 Evidence Library 只是一个受限 L1 loopback 页面：它没有材料 read model 或真实数据合同，因此不计入下列成熟度门。转为 `STABLE` 至少需要由不同事项、不同验收层实际证明：一个真实 L3 Topic/Observatory 页、一个真实 L2 Evidence/Topic 页、一个拥有真实数据合同、状态验证、响应式和可访问性证据的 L1 Corpus/Collection 页。若包含 L3 场景，还必须另有通过的 Style Frame、性能、Poster、移动端和 Reduced Motion 证明。

这不是推迟 LIDS 的执行：从今天起，所有获准 UI 工作都遵从 LIDS；只是不能把“设计标准已写好”误报为“产品和完整组件系统已建成”。
