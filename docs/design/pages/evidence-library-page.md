# PAGE-EVIDENCE-001 · Evidence Library 本地空态页

> 状态: 权威当前
> 最后核对: 2026-08-24
> 适用范围: `http://localhost:3000/corpus/evidence` 的第一个 Linggan 本地产品页面
> 事实来源: Mog 的 Local-001 产品确认、[LOCAL-001 活跃计划](../../plans/active/local-001-local-product-evidence-library.md)、Issue #25、LIDS 与当前 Rust 实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；V7 只作为已登记的本页 Gold Master

## 1. 身份与授权

- 页面规格 ID: `PAGE-EVIDENCE-001`
- 关联 Issue / Scope: `LOCAL-001 / 001A`，Issue #25
- 当前状态: 本地页面与 loopback host 获准；材料读投影、数据库读取和插件 ingress 未接通
- LIDS 视觉强度: L1 Corpus Explorer，嵌入受限 L2 Split Evidence Inspector
- LIDS 主 Pattern: `LIDS-PAT-001 / Corpus Explorer`，右侧使用同一工作面的 Inspector，不建立第二首页
- 产品页面来源: LOCAL-001 的 Evidence Library 用户任务；`REF-V7-001` 的页面 Gold Master
- 用户任务: 在此找到、看懂、验证、追溯 Linggan 已接纳的本地材料；本卡只诚实呈现“尚无可展示材料”的前置状态
- 三秒答案: “这是本地 Evidence Library；当前材料读投影尚未接通。”
- 五秒主动作: 本卡无动作。001B/001C 的后续能力未接通，不能以假按钮代替。
- 明确非目标: 采集、读数据库、显示材料、搜索、保存查询、Topic/Insight/Agent 行动、媒体/OCR/ASR、真实状态或线上部署
- 当前可用数据/权限合同: 仅 local host route 合同；没有 Materials read contract 或材料访问授权
- 决策 owner: Mog；实施范围由 LOCAL-001 活跃计划和 Issue #25 限定

## 2. 页面边界

- 入口: `GET /corpus/evidence`；同一 local host 的唯一页面路由
- 退出与返回: 当前无可用的相邻产品路由；顶层和二级导航是工作空间方位标记，不是未接通能力的假链接
- 本页负责的核心任务: 在材料存在以前，清楚说明 Evidence Library 将承接什么、当前哪些链路未接通、以及这些缺口不能证明什么
- 本页明确不负责: 形成或接纳 Evidence、判断 Observation、计算 Coverage、读取原文、启动采集、修改设置或建立任何第二事实源
- 第二事实源边界: HTML 只表达 host 的静态 `SOURCE_INCOMPLETE` 状态；它不保存数据、计数、材料、趋势或动作回执
- Inspector 边界: 右栏只在选中真实 `ContentItem` 后读取来源/观察/Capture/Coverage；001A 没有选中对象，所以仅显示 `UNKNOWN`

## 3. 信息、状态与行动

| 状态 ID | 触发/数据来源 | 用户应理解什么 | 禁止暗示什么 | 可用下一步 | 对应合同 |
|---|---|---|---|---|---|
| `SOURCE_INCOMPLETE` | 001A host 尚未连接 001B 材料读投影 | 此页没有足以显示的来源输入 | 世界里没有内容；Linggan 库为 0；采集失败 | 等待 001B 受控读投影 | LOCAL-001 / 001A |
| `NOT_CONNECTED` | 001A 明确没有数据库/API read contract | 当前页面没有读取任何材料 | 本地数据库、历史工作台或插件已被读取 | 无；不能在本页触发连接 | LOCAL-001 / 001A 禁止项 |
| `UNKNOWN` | 没有选择 ContentItem，且没有可用 Observation/Capture 输入 | 来源、观察、Capture 和 Coverage 当前未知 | unknown 等于 0、正常、失败或完整 | 无 | AGENTS.md 领域不变量；LIDS-PRI-001 |
| `NO_ACCEPTED_MATERIAL_AVAILABLE` | 页面范围内没有可展示的本地已接纳材料 | 页面现在不能展示材料 | 系统永久没有材料或材料不合格 | 001B/001C 的独立卡 | LOCAL-001 / 001A |

本卡没有用户动作，也没有 `success`、`live`、`fresh`、`loading`、`partial` 或 `failed` 的运行态展示；这些状态需要未来的真实合同，不由页面编造。

## 4. 已批准的设计组合

| 页面区域/场景 | PAGE / DS / PAT / CMP 来源 ID | 使用目的 | 明确不允许的替代 |
|---|---|---|---|
| 顶层产品方位 | `REF-V7-001` + `LOCAL-001-UI-EX-01` + LIDS Token | 延续 V7 两层导航的工作空间方向 | 未接通页面路由、假链接或伪造 active task |
| 左侧语料 rail | `LIDS-PAT-001 / Corpus Explorer` | 给 Evidence Library 一个稳定的 L1 定位 | 卡片瀑布流、全局第二侧栏 |
| 中央 Evidence Workspace | `LIDS-PAT-001 / Corpus Explorer` | 连续材料工作面；001A 只显示边界空态 | 示例材料、KPI 卡、趋势图或人工造数 |
| 右侧 Provenance Inspector | `LIDS-PAT-001 / Split Evidence Inspector` | 为真实对象预留来源/观察/Capture 边界位置 | 第二套筛选器、第二主页或 AI 结论 |
| Truth / Read Model / Coverage readout | `LIDS-PRI-001` 五轴状态原则 | 分开表达来源不足、读模型未接通和未知 Coverage | 一个万能“正常”或“失败”标签 |

### LIDS 采用清单

- Token 基线与唯一数值来源: `LIDS-TOK-001`。运行时 CSS 只使用 `--lgi-*` 名称；值逐项抄自 `docs/design/lids/tokens.md`。
- Primitive / 正式 CMP 查重结果: 没有新增跨页面 CMP；使用 `LIDS-PRI-001` 的 Sans/Mono、连续工作面、五轴状态分责与 hairline 原则。
- 页面唯一视觉核心: 中央空材料边界说明；不是营销 Hero，也不是假数据仪表盘。
- 原声、来源、样本、窗口、Coverage/Validity/冲突/复核边界: 001A 没有材料、原声或样本；页面直接说明对应字段未知而非显示占位数字。
- ASCII / 场景 / 动效: 不使用。`prefers-reduced-motion` 不触发额外动效，因为本页没有运动。
- 页面级例外 `LOCAL-001-UI-EX-01`: 为满足 V7 三栏工作空间的检视比例，本页用 216px 左栏、440px 右栏以及 2px 中央分界线。这些仅限本页布局，不是全局 Token；其余颜色、字体、间距、半径和动效值均映射 LIDS。
- 明确禁止: 第二套色彩/字体/状态/按钮语言，玻璃/渐变/悬浮卡，模拟数据、模拟 live/fresh、假 Primary 与任何前端事实写入。

## 5. 交互与真实后果

| 用户动作 | 前置条件 | 请求/回执来源 | 页面如何区分接纳、处理中、完成、部分、失败 | 不得宣称 |
|---|---|---|---|---|
| 无 | 001A 无获准操作 | 不适用 | 不适用；只显示当前静态 source/read-model 边界 | 后续功能、数据写入、采集或真实系统回执 |

## 6. 验收与未证明边界

- 任务验收场景: 打开页面后，能在首屏区分“本地 host 已运行”与“材料读投影未接通”。
- 状态/语义验收场景: `SOURCE_INCOMPLETE`、`NOT_CONNECTED`、`UNKNOWN` 和“页面没有可展示材料”分别出现；页面不出现 0、LIVE、FRESH、已采集或完成语言。
- 视觉验收场景与声明的工作条件: 1280×800、1440×900、1920×1080、390×844 的真实 local route 检查，见 `ACC-EVIDENCE-001`。
- 真实后果验收场景: 仅证明 loopback HTTP 路由；没有真实数据、数据库、插件、平台或部署副作用。
- 自动检查: API route tests、workspace format/clippy/test、governance check。
- 本次证明: 由实际命令和本地浏览器/HTTP 检查记录。
- 本次未证明: 001B read projection、001C discovery、任何 Evidence/Observation/Capture/Material、访问权限、媒体和业务验收。
