# LIDS v7.0 · 当前规则与审阅增量入口

> 状态: 权威当前
> 最后核对: 2026-10-19
> 适用范围: 全项目设计规则、候选审阅、渐进读取与迁移状态
> 事实来源: Mog 本次 LIDS-ALIGN-001 交付授权、已确认对话决定、现行 LIDS 与 main@9ff50b81；逐项来源见 review-alignment.json；LIDS-ALIGN-001 本次授权与对齐记录
> 冲突时以谁为准: 用户最新明确确认、产品/数据/权限合同与当前 SCOPE；已采纳设计按本文及明确替代关系执行，候选不自动生效


> 标准成熟度: PROPOSED
> 当前增量: LIDS-ALIGN-001；确认原则入库，不宣布完整组件库、业务迁移或整体视觉验收完成。

LIDS继续是全项目唯一设计表达标准：Token → Primitive → Component → Pattern → Page。Motion / Scene / Data Truth横向约束。v7保留规范身份，本次不将v8粗稿或独立审阅0.1.0宣布为完成版。

## 正式前端采用状态

LIDS-UI-001 已由 Mog 授权真实代码实施；唯一色值源 [color.json](color.json) 采用原正式目标色板。共享参数由 sync-lids-runtime.py 生成唯一运行 Token；[本轮验收](../acceptance/lids-ui-implementation-001-acceptance.md)逐项记录已实现、未迁移与未部署。下文及九源的“未改业务/runtimeMigrated=false”只描述原 LIDS-ALIGN-001 规范包，不覆盖本轮授权，也不表示全站已验收。

## 先读什么

先读 [UI Agent 渐进执行标准](../../governance/ui-design-execution.md)；按L1触发条件只展开本任务模块，再进入实际PAGE/CMP、产品与数据合同。根AGENTS.md、CLAUDE.md、codx.md共用该路由。

| 当前规则模块 | 唯一正文职责 |
| --- | --- |
| [system](system.md) | 视觉气质、强度、五层与Data Truth |
| [typography](typography.md) | 已采纳字体与B唯一字级；[参数源](typography.json)和生产接入边界 |
| [tokens](tokens.md) | 语义值、目标阶梯与未改动的运行时镜像 |
| [primitives](primitives.md) | 控件、按钮、输入/FIND的C焦点与基础反馈；参数[focus.json](focus.json) |
| [materials](materials.md) | M2控件材质与既有家族；参数[control-material.json](control-material.json) |
| [shell-zones](shell-zones.md) | 共享壳、领域原位、三级页面Tab统一归面包屑模块与A容量／图标折叠；参数[shell-layout.json](shell-layout.json) |
| [regions](regions.md) / [patterns](patterns.md) | 18个职责坐标、已确认组合原则与现有Pattern |
| [interaction-contracts](interaction-contracts.md) | 触发、反馈、取消、返回及真实回执 |
| [data-boundaries](data-boundaries.md) / [language-policy](language-policy.md) | 数据四态、来源、中文与英文使用边界 |
| [component-promotion](../components/component-promotion.md) | 共享组件查重、第二独立页面与正式CMP准入 |

## 决定、待审与证据

- [逐项对齐表](review-alignment.md)及其[清单源](review-alignment.json)：覆盖审阅稿全部R/Z/PT/C/B/I/P/A/D条目；只查本任务命中ID。它是追踪目录，不复制另一套规范权威。
- [待审项](review-items.md)：Q01/Q02已采用B、Q03原声已统一OPPO Sans；Q04焦点已采用C、Q05材质已采用M2；Q06壳层按ALIGN-SHELL-007采用A及图标折叠，Q07按ALIGN-LEDGER-008采用L1/44px/任务名称144px；Q08组合已按ALIGN-COMPOSITION-009采用含搜索的列表B/其余功能C，用户问题卡片保留；Q09浮层已按ALIGN-OVERLAY-010采用S1；Q10四类已按ALIGN-REUSE-011采用，七门槛保持；Q11字体交付已按ALIGN-FONT-012采用，Q12按ALIGN-MEDIA-013采用E1与白场舞台。
- [决定台账](decisions.md)：明确已确认规则对旧条款的局部替代，规则状态与运行时状态分列。
- [迁移记录](migration-log.md)：本次实际只改文档与治理检查；不宣称业务字体/组件/页面已迁移。
- [UI执行合同](../../agents/ui-execution-contract.md)与[设计治理](../design-governance.md)：授权、产品语义和验收边界继续有效。

新规则取代的部分以各分册ALIGN标记为准；旧规则未涉及部分及合法局部例外继续有效。旧设计快照不能覆盖用户最新确认；审阅稿中的新参数也不能覆盖未确认的正式值。规范采用、实现、运行、视觉和业务验收分别记录。

## 来源回执与不继承清单

### v7.0 来源（2026-09-02，当前）

| 来源文件 | SHA-256 | 本项目吸收的职责 |
|---|---|---|
| `/Users/moglenny/Downloads/linggan-design-system-v7.html` | `1093462bcea81c10584e18119a92ae6b51645d1b480667e161d993e863ca4e36` | 三层 Token 架构与实测对比度、8px 采样点阵材料语法、Mono 预算、壳层区位锁定、控件与图标、状态与空态、证据标本卡、数据边界四态、页面模式、11 条 ADR |

原件不复制进本仓库，SHA 仅证明本次核对的版本。**明确不继承**：文件中的全部演示数值（12,482 内容 / 327.9K 评论 / 1,832 作者 / 328K 证据 / 目标 ID 与博主名 / `09:42` 与各条时间戳）、`LIVE` 与运行状态、模拟事件流、示例作品标题与原声、以及任何被读作"系统已经有这些数据"的内容。它是设计规范，不是本仓库的产品事实来源。

命名冲突提醒：本仓库运行时 CSS 的 `v7-*` 类前缀来自 `REF-V7-001`（Evidence Library 页面 Gold Master，2026-08-24 登记），**与本设计系统 v7.0 无关**。两者同名纯属巧合，迁移时不要混为一谈。

### v2.0 来源（2026-08-21，已被 v7.0 取代）

本标准最初由 Mog 提供的本地包 `Linggan_Intelligence_Design_System_v2.0` 在 2026-08-21 核对后吸收。v7.0 取代了它的数值与表达口径，但它建立的五层架构、状态五轴、`PARTIAL + VALID` 与治理纪律**未被推翻，仍然有效**。来源包主文件校验值保留如下备查。

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


## 标准状态与升级门

LIDS 对协作执行的约束已经是“权威当前”，但其系统成熟度保持 `PROPOSED`。LOCAL-001A 的 Evidence Library 只是一个受限 L1 loopback 页面：它没有材料 read model 或真实数据合同，因此不计入下列成熟度门。转为 `STABLE` 至少需要由不同事项、不同验收层实际证明：一个真实 L3 Topic/Observatory 页、一个真实 L2 Evidence/Topic 页、一个拥有真实数据合同、状态验证、响应式和可访问性证据的 L1 Corpus/Collection 页。若包含 L3 场景，还必须另有通过的 Style Frame、性能、Poster、移动端和 Reduced Motion 证明。

这不是推迟 LIDS 的执行：从今天起，所有获准 UI 工作都遵从 LIDS；只是不能把“设计标准已写好”误报为“产品和完整组件系统已建成”。

- [LIDS-CLOSEOUT-18 · 整体收口核查](../../audits/lids-closeout-audit-2026-10-20.md)：Q01–Q12采用、已修复漂移、色值冲突与未验证层分开。
