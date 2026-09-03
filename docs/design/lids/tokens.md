# LIDS-TOK-001 · Token 基线与唯一数值来源

> 状态: 权威当前
> 运行时状态: 第 3 节的 127 项 `--lgi-*` 是当前**唯一运行时值源**；第 2 节的 v7 三层架构是**目标态与新工作的选值依据**，尚未进入运行时
> 最后核对: 2026-09-02
> 适用范围: Linggan Intelligence 主题、Primitive、Component、Pattern、Page、动效和受控场景的颜色、排版、间距、边界、层级与性能数值
> 事实来源: Mog 于 2026-09-02 指定的 `linggan-design-system-v7.html`（§01 Tokens、ADR-02/03/04）、Mog 指定的 LIDS v2.0 `tokens.md`（SHA-256: `97fac0fb590c7f349f5fe7bfc2e2e423c6c8145b03cec9ffdd80d4494f757e77`）、[system.md](system.md)、LOCAL-001A 运行时 Token 源与对应 Rust 校验
> 冲突时以谁为准: 已获准运行时的唯一 Token 文件 `apps/api/src/local_web/lids_tokens.css` 优先于本文件第 3 节；第 2 节的 v7 架构优先于第 3 节**作为新工作的选值依据**，但不得被写进运行时而不走迁移治理。产品/数据/权限冲突不由 Token 解决

## 0. 本文件现在有两层，必须分开读

DESIGN-010（2026-09-02）把 LIDS 的表达标准升级到 v7.0，但按 Mog 的明确范围裁定，**本次不改运行时 token**（`ADR-P02`，见 [decisions.md](decisions.md)）。因此本文件同时承载两个东西：

| 节 | 是什么 | 谁消费它 |
|---|---|---|
| **§2 v7 三层架构** | 目标态。新页面、新组件的**选值依据**与命名结构 | 设计规格、CMP、PAGE、评审 |
| **§3 `--lgi-*` 基线** | 运行时现状。127 项精确值，受 Rust 逐项校验 | 现有运行页 CSS |

**不要把 §2 的名字写进运行时 CSS，也不要把 §3 的值当成 v7 合规。** 两者的差异是已登记的欠账，不是可以随手抹平的漂移。迁移触发条件见 §4。

## 1. 运行时权威与后续 Agent 规则

```text
唯一值编辑源
apps/api/src/local_web/lids_tokens.css
        ↓ 同一提交内同步为镜像
docs/design/lids/tokens.md §3
        ↓ Rust 逐项名称和值校验（127 项）
运行时页面与版本化规范
```

- 后续 Agent 要改变某个运行时 token 值时，只能先修改 `apps/api/src/local_web/lids_tokens.css`；不得先在页面 CSS、组件 CSS 或本 Markdown 中创造不同值。
- 同一提交必须把 §3 的基线清单同步为运行时源的精确镜像，并通过 Rust 的 127 项名称→值比对。校验失败即表示变更未完成，不能用"名称一致"掩盖值漂移。
- 任何 token 值的改变仍是跨页面设计变更：必须按 UI Change Manifest、迁移记录、影响页面和回退规则完成治理。
- 页面 CSS 只消费 `var(--lgi-*)`，不得声明 `--lgi-*`；页面局部例外必须按 PAGE/Manifest/LIDS migration log 另行登记，不能反向写入 Token 真源。

不得为了静态参考页、单个组件或视觉偏好在任何页面另造颜色、字号、间距、圆角、阴影、动效时长、页面宽度或 z-index 值。

## 2. v7 三层 Token 架构（目标态）

v7 把 token 分成三层，并规定了一条**单向引用**规则：

```text
L1 PRIMITIVE  只在 :root 出现，业务样式不得直接引用
      ↓
L2 SEMANTIC   组件唯一允许引用的层
      ↓
L3 GEOMETRY   间距 / 字号 / 线宽 / 圆角 / 动效 五套阶梯
```

**组件只能引用 L2 与 L3。** 组件直接引用 L1 原色（如 `--ink-950`）即违规——那等于把"这里为什么是这个颜色"这个决定复制了一份，下次换色时会漏掉。

### 2.1 文字与状态色 · 实测对比度

对比度按 WCAG 2.1 在白底 `#ffffff` 上计算。**低于 4.5 的色值不得承载正文或元数据。**

| 语义 Token | 值 | 对比度 | 判定 | 用途 |
|---|---|---|---|---|
| `text-primary` | `#0b0f12` | 19.2 | AAA | 标题、关键数值 |
| `text-body` | `#30373c` | 12.1 | AAA | 正文 |
| `text-secondary` | `#5f6a71` | 5.6 | AA | 说明、次要信息 |
| `text-meta` | `#6b767d` | 4.7 | AA | 机器事实、时间戳 |
| `text-disabled` | `#8c959b` | 3.1 | 仅装饰 | 禁用态、分隔符 |
| `text-signal` | `#c63214` | 5.4 | AA | 信号文字、强调 |
| `signal-mark` | `#f24a23` | 3.6 | **仅色块** | 信号轨、销钉、编号 |
| `status-ok` | `#137557` | 5.7 | AA | 可用、就绪 |
| `status-warn` | `#8a5c00` | 5.8 | AA | 部分完成 |
| `status-bad` | `#b52b20` | 6.3 | AA | 中断、错误 |
| `status-info` | `#335e72` | 7.0 | AA | 提示、焦点环 |

### 2.2 信号色的双角色（`ADR-04`，硬规则）

这是 v7 最容易被误用的一条：

| Token | 值 | 角色 | 绝不 |
|---|---|---|---|
| `signal-fill` | `#c63214` | **承载白字**的填充：主按钮、实心徽章 | — |
| `signal-mark` | `#f24a23` | **只做色块**：信号轨、选中销钉、章节编号 | 承载任何文字 |

原橙 `#f24a23` 承载白字只有 3.6 : 1，不合格。**品牌识别由色相延续，不由具体色值延续**——两个值同色相，视觉上仍是同一个信号色。

### 2.3 几何阶梯

| 阶梯 | 取值 | 约束 |
|---|---|---|
| 间距 | 2 / 4 / 8 / 12 / 16 / 24 / 32 / 48 / 64 | 4pt 基准 9 阶；不新增孤立补丁值 |
| 字号 | 11 / 12 / 13 / 15 / 17 / 21 / 27 + `display` | 7 阶，**下限 11px**（`ADR-03`） |
| 行高 | 1.2 / 1.4 / 1.6 / 1.75 | — |
| 字重 | **400 / 600 / 700** | 3 档（`ADR-02`）；850/950 在 PingFang SC 与 Noto Sans SC 中不存在，只会触发浏览器合成加粗 |
| 线宽 | 1（分隔）/ 2（对象边界）/ 3（模式切换）/ 5（信号条）/ 6（硬投影位移） | **边框中的 1.5px 全部移除**——1x 屏上产生半像素模糊。图标描边 1.5 是 SVG 单位会随尺寸缩放，不受此限 |
| 圆角 | 0 / 4 / 8 / pill | pill 仅用于状态点一类纯圆元素，不用于按钮 |
| 尺寸 | 28 / 34 / 40 / 48 / 56 | 控件高度 |
| 动效 | 90 / 140 / 200 / 320ms | 4 档 + 单一 ease `cubic-bezier(.2,.78,.2,1)` |

### 2.4 字体角色

| 角色 | 承担什么 |
|---|---|
| `font-ui` Sans | 操作与连续阅读。标题靠字号差、字重与结构线形成强度，不靠奇怪字体 |
| `font-mono` | 机器事实、状态枚举、结构编号。预算见 [language-policy.md](language-policy.md) |
| `font-evidence` Serif | **人的原声**。评论原文、访谈引用——保留它不是装饰，是标明"这句话不是系统写的" |

负字距只作用于拉丁标题；**中文正文不收紧字距**。

### 2.5 密度（`ADR-06`）

密度控制**一屏能看多少内容**——内边距、行高与次要元数据的显隐。**它不是纹理浓度。**

| 档 | 内边距 | 行高 | 次要元数据 | 材料可见度 |
|---|---|---|---|---|
| T0 疏 | ×1.35 | ×1.25 | 显示 | 关闭 |
| T1 | ×1.15 | ×1.1 | 显示 | 0.45 |
| T2 默认 | ×1 | ×1 | 显示 | 1 |
| T3 密 | ×0.8 | ×0.88 | **隐藏** | 1 |

材料强度是密度的**附带效果**，不是它的目的。切换密度后如果一屏能看的条数没变，这个实现就是错的。

## 3. 完整运行时基线（127 项 `--lgi-*`）

本节是 `apps/api/src/local_web/lids_tokens.css` 的精确镜像，受 Rust 逐项校验。**它是运行时现状，不是 v7 合规基线。**

```css
[data-theme="linggan-intelligence"] {
  /* Canvas */
  --lgi-canvas: #ffffff;
  --lgi-canvas-hi: #ffffff;
  --lgi-canvas-low: #f7f8f8;
  --lgi-canvas-sunken: #eef0f1;
  --lgi-canvas-overlay: rgba(255, 255, 255, 0.94);
  --lgi-canvas-glass: rgba(255, 255, 255, 0.88);

  /* Ink */
  --lgi-ink: #111315;
  --lgi-body: #4a5057;
  --lgi-muted: #6f747a;
  --lgi-ghost: #9aa0a6;
  --lgi-on-dark: #ffffff;

  /* Signature signal */
  --lgi-signal: #ef4f25;
  --lgi-signal-hover: #c73a15;
  --lgi-signal-ink: #a73317;
  --lgi-signal-soft: rgba(239, 79, 37, 0.10);
  --lgi-signal-faint: rgba(239, 79, 37, 0.05);

  /* Semantic: five axes, each carries text / fill / soft.
   * Fill values stay saturated for solid badges; text values are darkened
   * so they clear 4.5:1 on the white canvas. Never swap the two roles. */
  --lgi-success: #05674a;
  --lgi-success-dot: #0e9e6e;
  --lgi-success-soft: rgba(14, 158, 110, 0.10);
  --lgi-warning: #a67a04;
  --lgi-warning-dot: #eaaa05;
  --lgi-warning-soft: rgba(234, 170, 5, 0.12);
  --lgi-danger: #a42001;
  --lgi-danger-dot: #a42001;
  --lgi-danger-soft: rgba(164, 32, 1, 0.08);
  --lgi-info: #42555a;
  --lgi-info-dot: #8d9a9d;
  --lgi-info-soft: rgba(141, 154, 157, 0.14);
  --lgi-unknown: #6f747a;
  --lgi-unknown-dot: #c3c8cb;
  --lgi-unknown-soft: #e7eaeb;

  /* Borders */
  --lgi-hairline: rgba(17, 19, 21, 0.09);
  --lgi-border: rgba(17, 19, 21, 0.15);
  --lgi-border-strong: rgba(17, 19, 21, 0.22);
  --lgi-border-solid: #111315;

  /* Instrument mesh: a measurement field, not decoration. Dots read lighter than
   * a ruled grid at the same weight, so the surface stays quiet while gaining depth. */
  --lgi-mesh-fine: rgba(17, 19, 21, 0.055);
  --lgi-mesh-major: rgba(17, 19, 21, 0.10);

  /* Observation stream: the one deliberately dark surface in the product. Mog approved
   * keeping the V4 Gold Master's low-luminance instrument terminal on 2026-08-26; it is a
   * recorded long-term exception to the LIDS rule against dark terminal surfaces, and it is
   * confined to Operations/NOW. Never reuse these outside that stream. */
  --lgi-stream-bg: #07110e;
  --lgi-stream-bg-raised: #091712;
  --lgi-stream-ink: #d9f7e8;
  --lgi-stream-muted: #78aa97;
  --lgi-stream-dim: #42685a;
  --lgi-stream-mint: #63d9a5;
  --lgi-stream-cyan: #74b8ad;
  --lgi-stream-amber: #d59a38;
  --lgi-stream-red: #e06a55;
  --lgi-stream-line: rgba(99, 217, 165, 0.2);

  /* Typography */
  --lgi-font-sans: "PingFang SC", "Noto Sans SC", "Microsoft YaHei", system-ui, -apple-system, sans-serif;
  --lgi-font-mono: "SFMono-Regular", "JetBrains Mono", "Roboto Mono", "Noto Sans Mono CJK SC", ui-monospace, monospace;
  --lgi-font-evidence: "Songti SC", "Noto Serif CJK SC", "Source Han Serif SC", "STSong", Georgia, serif;
  --lgi-font-display: "Arial Narrow", "DIN Condensed", "Roboto Condensed", "Helvetica Neue", var(--lgi-font-sans);
  --lgi-mosaic-on-dark: conic-gradient(from 90deg, rgba(255, 255, 255, 0.22) 25%, transparent 0 50%, rgba(255, 255, 255, 0.22) 0 75%, transparent 0);
  --lgi-mosaic-on-light: conic-gradient(from 90deg, rgba(17, 19, 21, 0.13) 25%, transparent 0 50%, rgba(17, 19, 21, 0.13) 0 75%, transparent 0);
  --lgi-text-display: clamp(3rem, 6vw, 5.5rem);
  --lgi-text-hero: 3rem;
  --lgi-text-page-title: 1.75rem;
  --lgi-text-section: 1.125rem;
  --lgi-text-longform: 1rem;
  --lgi-text-body: 0.875rem;
  --lgi-text-data: 0.75rem;
  --lgi-text-label: 0.6875rem;
  --lgi-text-calibration: 0.5625rem;
  --lgi-weight-regular: 400;
  --lgi-weight-medium: 500;
  --lgi-weight-semibold: 600;
  --lgi-weight-bold: 700;
  --lgi-lh-display: 0.96;
  --lgi-lh-title: 1.1;
  --lgi-lh-heading: 1.25;
  --lgi-lh-data: 1.4;
  --lgi-lh-body: 1.55;
  --lgi-lh-longform: 1.65;
  --lgi-ls-tight: -0.04em;
  --lgi-ls-normal: 0;
  --lgi-ls-data: 0.04em;
  --lgi-ls-caps: 0.12em;
  --lgi-ls-calibration: 0.18em;

  /* Spacing */
  --lgi-space-1: 4px;
  --lgi-space-2: 8px;
  --lgi-space-3: 12px;
  --lgi-space-4: 16px;
  --lgi-space-5: 20px;
  --lgi-space-6: 24px;
  --lgi-space-8: 32px;
  --lgi-space-10: 40px;
  --lgi-space-12: 48px;
  --lgi-space-16: 64px;
  --lgi-space-24: 96px;

  /* Radius */
  --lgi-radius-none: 0;
  --lgi-radius-xs: 2px;
  --lgi-radius-sm: 4px;
  --lgi-radius-md: 8px;

  /* Shadows */
  --lgi-shadow-none: none;
  --lgi-shadow-contact: 0 1px 2px rgba(17, 19, 21, 0.07);
  --lgi-shadow-overlay: 0 12px 30px rgba(17, 19, 21, 0.11);
  --lgi-shadow-stage: 0 24px 25px rgba(17, 19, 21, 0.05);
  --lgi-shadow-hard: 8px 8px 0 rgba(17, 19, 21, 0.07);
  --lgi-shadow-brutal: 4px 4px 0 #111315;
  --lgi-shadow-brutal-lg: 7px 7px 0 #111315;

  /* Layout */
  --lgi-page-max-l3: 1720px;
  --lgi-page-max-workbench: 1440px;
  --lgi-header-height: 64px;
  --lgi-header-height-compact: 58px;
  --lgi-command-height: 76px;
  --lgi-grid-major: 180px;
  --lgi-grid-mobile: 120px;
  --lgi-calibration-width: 27px;

  /* Motion */
  --lgi-duration-press: 100ms;
  --lgi-duration-state: 160ms;
  --lgi-duration-overlay: 200ms;
  --lgi-duration-panel: 240ms;
  --lgi-duration-page: 280ms;
  --lgi-duration-scene: 600ms;
  --lgi-duration-ambient: 10s;
  --lgi-ease-ui: cubic-bezier(0.2, 0.75, 0.2, 1);
  --lgi-ease-enter: cubic-bezier(0.16, 1, 0.3, 1);
  --lgi-ease-linear: linear;

  /* Z-index */
  --lgi-z-base: 0;
  --lgi-z-stage: 10;
  --lgi-z-sticky: 30;
  --lgi-z-header: 40;
  --lgi-z-drawer: 80;
  --lgi-z-overlay: 100;
  --lgi-z-popover: 140;
  --lgi-z-toast: 180;

  /* Isometric 2.5D */
  --lgi-iso-axis-angle: 26.565deg;
  --lgi-iso-layer-gap: 32px;
  --lgi-iso-module-depth-sm: 12px;
  --lgi-iso-module-depth-md: 22px;
  --lgi-iso-module-depth-lg: 44px;

  /* WebGL quality */
  --lgi-scene-pixel-ratio-max: 1.75;
}
```

## 4. v2.0 → v7 的差异与迁移欠账

下表是 §2（目标态）与 §3（运行时现状）之间**已知的、未消除的**差异。它不是待办清单的全部，但每一条都必须在迁移前被单独处置。

| 项目 | 运行时现状（§3） | v7 目标（§2） | 影响面 | 对应 ADR |
|---|---|---|---|---|
| 命名结构 | 单层 `--lgi-*` 前缀 | L1/L2/L3 三层，组件只引用 L2/L3 | 全部样式表 | — |
| 主信号色 | `--lgi-signal: #ef4f25`，单值双用 | `signal-fill #c63214`（承字）/ `signal-mark #f24a23`（仅色块） | 主按钮、选中态、徽章 | `ADR-04` |
| 墨色 | `--lgi-ink: #111315` | `#0b0f12` | 全站文字与边框 | — |
| 字重 | 含 500（`--lgi-weight-medium`） | 仅 400/600/700 | 全站排版 | `ADR-02` |
| 字号下限 | 9px（`--lgi-text-calibration`） | 11px | 校准刻度、系统标签 | `ADR-03` |
| 线宽 | [primitives.md](primitives.md) 曾规定"全站只有两级线宽"（1/2px） | 4 档（1/2/3/5）+ 硬投影 6px | 分隔、Tab、信号条 | — |
| 圆角 | 0/2/4/8 | 0/4/8/pill（去掉 2px，新增 pill） | 标签、状态点 | — |
| 材料 | `--lgi-mesh-fine` / `--lgi-mesh-major` 双层点阵 | 8px 采样点阵六态 | 背景与边缘 | `ADR-08` `ADR-09` |
| 密度 | 无 | T0–T3 驱动信息量 | 全部列表与容器 | `ADR-06` |
| 深色面 | `--lgi-stream-*` 记录例外 | `M-05` 低亮传感面 | Operations/NOW | `ADR-07` |

### 迁移触发条件

在下列任一条件成立前，**不启动 token 迁移**：

1. Mog 明确授权一次以 token 迁移为唯一目标的受控事项；
2. 该事项有独立 worktree 与文件所有权，不与并行分支抢 `lids_tokens.css`、`shell.css`、`evidence_library.css`、`collection_workspace.css`；
3. 已列出全部受影响页面，并对每个页面准备了迁移前后的同视口对照证据。

### 分步顺序（授权后）

迁移必须分步，且**每一步单独可回退**。合并成一次大改会让"哪一步导致这里变丑"无法定位：

1. **加不减**：在 `lids_tokens.css` 中新增 v7 的 L1/L2/L3 三层，`--lgi-*` 全部改为指向新层的别名。此时渲染结果**逐像素不变**，Rust 校验按新的值同步更新。
2. **换值**：逐条落实 `ADR-04`（信号双角色）、`ADR-02`（字重）、`ADR-03`（字号下限）。每条一个提交，每条一次视觉对照。
3. **换引用**：把三个样式表的 `var(--lgi-*)` 改为直接引用 L2/L3，别名层保留但不再新增引用。
4. **删别名**：确认全仓无 `--lgi-*` 引用后删除别名层，更新 Rust 校验的项数与名称集合。
5. **材料与密度**：`ADR-08`/`ADR-09`/`ADR-06` 是新增能力而非替换，放在最后，各自独立事项。

任何跳步——尤其是跳过第 1 步直接改值——都会让"这次变化是别名引入的还是换值引入的"无法区分。

## 5. 语义护栏

| 领域 | 必须遵守 |
|---|---|
| Canvas | 纯白主工作台；`canvas-low` 只作结构灰。`canvas-glass` 仅浮层，不建立玻璃拟态体系。 |
| Ink | `muted` 是功能文字最低灰度；`ghost` / `text-disabled` 只作禁用文字与纯装饰，**不能承载关键数据或操作**。 |
| Signal | 品牌活跃/选中/升级；不是 danger/warning/success。承载文字只能用 `signal-fill` 一档（`ADR-04`）。 |
| Semantic | 绿=Valid/Completed，琥珀=Partial/Aging/Retrying，深红=Failed/Invalid/删除，蓝灰=信息/链接；均需文字和图形/定位双通道。 |
| Type | Sans 承担中文阅读；Mono 承担机器语义；**Evidence 只承担逐字引用的来源材料**——正文原文、评论原文、OCR/转录文本，不用于任何界面文字，否则读者无法一眼分辨「谁说的」和「系统说的」。Display 为窄体，只用于页面级强层级。可对比数字要有 `tabular-nums`。**功能文字不得低于 11px（`ADR-03`）——v7 废止了 9px 校准刻度，它在非高分屏上不可读。** |
| Mosaic | `mosaic-on-dark` / `mosaic-on-light` 只贴在实心墨色主动作或激活块的**右缘**，必须配渐隐遮罩，面积约 35%–45%，绝不覆盖文字。不得铺满、不得作页面背景、不得替换为竖条纹或随机噪点。它与 [materials.md](materials.md) 的 `M-02` 采样栅格是同一件事的两代写法：Mosaic 是当前运行时的落地值，`M-02` 是 v7 把它归入八格点阵家族后的规则层。 |
| Spacing | 只用 4px 基数；不得新增 13/17/29px 等孤立对齐补丁。 |
| Radius/Shadow | 默认 0/4/8 和无阴影；pill 只给纯圆元素。硬投影必须是实色偏移（`4px 4px 0`），**不得使用模糊或多层阴影**。 |
| Layout | L3 最大 1720px；L1/L2 最大 1440px；超宽屏增加外白而非拉宽正文。 |
| Motion | 只能用本表时长/缓动；不创建 137ms、430ms 等孤立值。量化位移用 `steps()`（`ADR-09`）。 |

## 6. Token 禁止项

- 禁止组件直接写六位色值，禁止新增 `blue-500` / `red-500` 通用库色。
- 禁止组件直接引用 L1 PRIMITIVE（`--ink-950` 一类）；组件只引用 L2/L3。
- 禁止 `pt`、页面自定义最大宽度、12px 以上通用圆角、Clay/霓虹/玻璃阴影，或场景另造第二套 Signal 色。
- 禁止边框使用 1.5px（1x 屏半像素模糊）；SVG 图标描边的 1.5 不受此限。
- Token 值改变是跨页面设计变更：必须在 UI Change Manifest 中标为最高影响级，列出影响页面，更新预览/静态参考、迁移记录与回退办法。
- 不得新建第二个运行时 Token 文件、页面级主题或值编辑入口。新增自动生成工具或改变单向关系，必须另开受控任务。
