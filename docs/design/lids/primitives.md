# LIDS-PRI-001 · Primitive 与基础交互契约

> 状态: 权威当前
> 运行时状态: 定义跨页面基础语法；LOCAL-001A 页面存在，但本文件不把页面局部 HTML/CSS 提升为通用 Primitive 实现
> 最后核对: 2026-08-26
> 适用范围: Linggan Intelligence 未来前端的文本、按钮、状态、Surface、Readout、分割、输入、列表、提示、反馈和焦点行为
> 事实来源: Mog 指定的 LIDS v2.0 `primitives.md`（SHA-256: `e7d67dba088fea500d05ecfa45a62a290e4b3d07f121830f010a9e38cecf5b73`）、[tokens.md](tokens.md)、[system.md](system.md)
> 冲突时以谁为准: 用户最新确认、产品/数据/权限合同、当前 SCOPE 和已获准的正式 CMP；本文件不授予实现或数据状态

Primitive 是全站可复用的基础语法，不承载具体 Topic、真实请求、权限判断或业务结论。未来运行时的唯一前缀为 `.lgi-*`；禁止新建原型私有类或第二套视觉前缀。

## 线条：先问能不能不画（DESIGN-003）

线是这套视觉语言里最容易失控的元素。Evidence Library 在 DESIGN-003 之前一屏有 194 条可见边框，其中六成来自给小元素套框。定六条规则，按顺序适用：

1. **分组用间距，不用线。** 靠近即成组。只有当间距不足以表达分组时才画线。列表行、导航项、栏目项之间一律不画。
2. **面代替线。** 区分两块区域时给其中一块换底色，好过在中间画线。已经有底色差异的地方不再补线。
3. **实心底已是分隔。** 有填充的元素不再加边框。
4. **边框只留给可交互元素。** 有框即可点：输入、下拉、按钮、复选框保留；标签、封面、指标、矩阵行一律去框。这让边框本身成为可供性信号。
5. **全站只有两级线宽。** 结构级 2px 实黑，内容级 1px `--lgi-hairline`。中间档一律并入这两级。
6. **对齐能替代线。** 元素本身对齐准确时，用于提示对齐的线是冗余的。

纹理不是线：需要背景层次时用 `--lgi-mesh-fine` / `--lgi-mesh-major` 点阵，不用划线网格。

## 排版

| 类别 | 字体/规格 | 只能承担 |
|---|---|---|
| `lgi-text-display` | Sans / display / 0.96 / 700 | L3 品牌级 Topic 标题 |
| `lgi-text-page-title` | Sans / 28px / 1.1 / 600 | L1/L2 页面标题 |
| `lgi-text-section` | Sans / 18px / 1.25 / 600 | 区块标题 |
| `lgi-text-longform` | Sans / 16px / 1.65 / 400 | 原声、研究结论 |
| `lgi-text-body` | Sans / 14px / 1.55 / 400 | 普通正文 |
| `lgi-text-data` | Mono / 12px / 1.4 / 500 | 数据、时间、ID |
| `lgi-text-label` | Mono / 11px / 1.2 / 600 | 状态/字段标签 |
| `lgi-text-calibration` | Mono / 9px / 1.2 / 500 | 纯装饰刻度 |
| `lgi-caps` / `lgi-tabular` | Mono caps / tabular nums | 英文系统标签 / 对齐数字 |

禁止任意字号和长中文 Mono；`calibration` 不能进入按钮、Tooltip、表格值或关键状态。

## 按钮：一个动作区域最多一个 Primary

| 类型 | 使用 | 禁止 |
|---|---|---|
| Primary | 当前区域唯一真实主动作；Signal 实底 + Ink 边框 + 实色硬阴影，hover 加深并整块位移 | 用于未接通能力、同区第二主动作、渐变、模糊阴影、多层阴影 |
| Secondary | 返回、取消、查看证据；浅面+强边框 | 伪装成第二种 Primary |
| Quiet | 筛选、展开、复制、切换；无常态边框 | 用于危险/不可逆动作 |
| Danger | 删除、停止、驳回；深红文字/边框，确认后才可实底 | 无对象、数量与后果的操作 |
| Command | 深色 Command Dock 的上下文命令 | 代替页面普通 Primary |

按钮最小高度 40px，纯图标必须有可读名称，禁用必须不可点击并说明原因；loading 保持宽度并显示真实阶段。禁止原生弹窗、假按钮、视觉禁用仍可点击。

### 粗野重音的位移（DESIGN-003 修订）

本节此前禁止一切 hover 上浮。Mog 于 2026-08-25 确认新粗野主义作为重音语言，位移是它的定义动作，因此改为**受限允许**：

- 只有承担粗野重音的元素可以位移：品牌标识、Primary、Secondary、当前选中项。普通列表行、筛选、标签、卡片一律不得位移。
- 位移量固定 `translate(-2px, -2px)`，同时把 `--lgi-shadow-brutal` 换成 `--lgi-shadow-brutal-lg`；按下状态回落到 `translate(1px, 1px)`。
- 阴影必须是实色偏移（`4px 4px 0`），不得使用模糊或多层阴影。模糊阴影仍在禁止之列，被解除的只是位移本身。
- `prefers-reduced-motion: reduce` 下位移与过渡全部关闭。

判断一处位移是否合规，只问一句：它是不是那一屏 8 处重音之一。不是就不给。

### 悬停不得改写当前选中项（2026-08-26）

导航项的 hover 是「你可以离开这里」的邀请，选中态是「你现在在这里」的事实。两者语义相反，因此 **hover 的底色与文字色一律不得覆盖当前选中项**：鼠标经过时，选中项保持它的 Ink 实底与反白文字不变。

- hover 规则必须在选择器里排除当前项（`:not([aria-current="page"]):hover`），而不是在其后再把选中态的颜色重写一遍——一个状态只能有一个归属规则。
- 这条约束是硬性的，因为 CSS 权重会站在错误的一边：`.x:not([disabled]):hover` 的权重高于 `.x[aria-current="page"]`，先写选中态并不能保住它。
- 选中项仍可保留位移与阴影重音（上一节允许），被禁止的只是底色与文字色被 hover 改写。
- 未选中项的 hover 反馈必须原样保留：修这条问题时把整个 hover 反馈删掉同样不合格。

### 禁用态

未接通的能力不得以完整 Primary 形态出现：禁用时去掉硬阴影，边框降到 `--lgi-border`，文字降到 `--lgi-ghost`，Signal 实底降为 `--lgi-signal-soft`。用户必须一眼看出「这个动作现在不存在」，而不是「点了没反应」。

## 状态：五轴分别表达

未来正式组件分为 `TruthTag`、`CoverageTag`、`ValidityTag`、`FreshnessTag`、`OperationTag`，不做一个万能 `StatusTag`。标签高度 22–24px、具有文字、方形或 2px 角；状态不只依赖颜色。`PARTIAL` 使用 Warning，且绝不等同 `INVALID`；Signal 不作为普通状态色。

### 实心填充与文字色分工（DESIGN-003）

状态标签使用**实心填充 + Ink 文字**，不使用浅底淡字：

| 角色 | 填充（标签底） | 文字（正文内引用状态时） |
|---|---|---|
| 已验证 / 覆盖完整 | `--lgi-success-dot` | `--lgi-success` |
| 部分 / 配额未满 | `--lgi-warning-dot` | `--lgi-warning` |
| 冲突 / 失败 | `--lgi-danger-dot`（配 `--lgi-on-dark` 文字） | `--lgi-danger` |
| 仅发现层 / 处理说明 | `--lgi-info-dot` | `--lgi-info` |
| 未知 | `--lgi-unknown-soft` | `--lgi-unknown` |

两条硬规则：

1. **填充色与文字色不得互换。** `--lgi-warning-dot`（`#eaaa05`）在白底上的对比度只有 2.1:1，作正文色不可读；`--lgi-warning`（`#a67a04`）作大面积填充又失去金黄的辨识度。每个角色的两个值各司其职。
2. **实心底本身就是分隔，不再叠加边框。** 给实心标签加描边会压低饱和度，让颜色变脏，同时把页面线条数量成倍推高。

未知是唯一不给饱和色块的角色：它不是错误，只是尚未知道，视觉重量应低于任何已知状态。

没有真实合同的静态参考只能使用显式合成/来源不足边界，不得假装这些组件已接收真实状态。

## InstrumentSurface 与 InstrumentReadout

`InstrumentSurface` 是一个视觉组的唯一外壳：`canvas`（主面）、`soft`（次级）、`ink`（命令/关键判断）、`signal-soft`（当前对象）、`warning-soft`（缺口）、`danger-soft`（失败）。边缘只有 `plain`、`rail`、`cut`、`ticket`；页面不可同时堆五种边缘。非交互 Surface 没有 hover，Ink 面不承载大段普通说明。

`InstrumentReadout` 固定为 `LABEL / VALUE UNIT / FRESHNESS or SOURCE`：数字 tabular，单位不换行，必须有口径或可展开入口；0 中性，未知写 `UNKNOWN`，只在可交互时具 hover/focus。超过 20 行同构数据使用表格；连续列表不为每行套圆角卡。

## 输入、提示与反馈

- Input/Search：浅面、1px 边框、4px 圆角、双层焦点；真实搜索才有防抖、清空、结果数、关键词高亮和可恢复状态。
- Command：只用于已获批准的研究工作区/Command Dock，必须显示上下文，`>` 不是可访问标签的替代。
- Tooltip：只解释缩写、图标或截断；Ink 背景+Signal 左轨，可 hover/focus/touch 替代，不能承载必读内容。
- Popover：可交互、有焦点管理和 Esc；不得和 Tooltip 混用。
- Loading：区块骨架、按钮内阶段；超过 3 秒显示真实步骤。Empty 必答“现在没有什么、为什么、下一步”；Error 必答“出了什么事、影响什么、下一步如何救援”，技术详情可折叠，不能把原始错误抛给用户。

## Focus、列表和硬性禁止

所有交互元素命中区至少 24×24px，双层 Focus Ring 可见，不用 `tabindex > 0`。表头使用 11px Mono；行高 40–44px；数字右对齐；行内操作仅 hover/focus 出现；长文本可展开。

禁止第二套按钮/状态/Surface、任意 Pill、硬编码视觉值、非交互 hover、普通卡浮起、动画掩盖加载、Tooltip 承载错误处置、Toast 承载必须处理的错误。
