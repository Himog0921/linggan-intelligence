# LIDS-PRI-001 · Primitive 与基础交互契约

> 状态: 权威当前
> 运行时状态: 定义未来唯一基础语法，不声明现有代码实现
> 最后核对: 2026-08-21
> 适用范围: Linggan Intelligence 未来前端的文本、按钮、状态、Surface、Readout、分割、输入、列表、提示、反馈和焦点行为
> 事实来源: Mog 指定的 LIDS v2.0 `primitives.md`（SHA-256: `e7d67dba088fea500d05ecfa45a62a290e4b3d07f121830f010a9e38cecf5b73`）、[tokens.md](tokens.md)、[system.md](system.md)
> 冲突时以谁为准: 用户最新确认、产品/数据/权限合同、当前 SCOPE 和已获准的正式 CMP；本文件不授予实现或数据状态

Primitive 是全站可复用的基础语法，不承载具体 Topic、真实请求、权限判断或业务结论。未来运行时的唯一前缀为 `.lgi-*`；禁止新建原型私有类或第二套视觉前缀。

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
| Primary | 当前区域唯一真实主动作；Ink 实底，hover 才进入 Signal | 用于未接通能力、同区第二主动作、渐变/浮起/多层阴影 |
| Secondary | 返回、取消、查看证据；浅面+强边框 | 伪装成第二种 Primary |
| Quiet | 筛选、展开、复制、切换；无常态边框 | 用于危险/不可逆动作 |
| Danger | 删除、停止、驳回；深红文字/边框，确认后才可实底 | 无对象、数量与后果的操作 |
| Command | 深色 Command Dock 的上下文命令 | 代替页面普通 Primary |

按钮最小高度 36px，纯图标必须有可读名称，禁用必须不可点击并说明原因；loading 保持宽度并显示真实阶段。禁止原生弹窗、假按钮、视觉禁用仍可点击和 hover 上浮。

## 状态：五轴分别表达

未来正式组件分为 `TruthTag`、`CoverageTag`、`ValidityTag`、`FreshnessTag`、`OperationTag`，不做一个万能 `StatusTag`。标签高度 22–24px、具有文字、方形或 2px 角；状态不只依赖颜色。`PARTIAL` 使用 Warning，且绝不等同 `INVALID`；Signal 不作为普通状态色。

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
