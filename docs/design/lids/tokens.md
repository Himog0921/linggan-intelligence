# LIDS-TOK-001 · Token 基线与唯一数值来源

> 状态: 权威当前
> 运行时状态: Token 文件尚未获准创建
> 最后核对: 2026-08-21
> 适用范围: Linggan Intelligence 未来主题、Primitive、Component、Pattern、Page、动效和受控场景的颜色、排版、间距、边界、层级与性能数值
> 事实来源: Mog 指定的 LIDS v2.0 `tokens.md`（SHA-256: `97fac0fb590c7f349f5fe7bfc2e2e423c6c8145b03cec9ffdd80d4494f757e77`）、[system.md](system.md)、当前无 Web 运行时的项目状态
> 冲突时以谁为准: 已获准运行时的唯一 Token 文件优先；在它尚不存在时以本基线为准。产品/数据/权限冲突不由 Token 解决

本文件冻结 LIDS v2.0 的唯一数值基线。现在它是设计阶段的数值真源；未来只有在用户和当前 SCOPE 授权运行时主题后，才能以完全相同的 token 名称迁入一个唯一的代码文件。届时数值只可在该文件编辑，本文件必须由脚本生成或校验，避免两套数值漂移。

不得为了静态参考页、单个组件或视觉偏好在任何页面另造颜色、字号、间距、圆角、阴影、动效时长、页面宽度或 z-index 值。

## 完整 LIDS v2.0 基线

```css
[data-theme="linggan-intelligence"] {
  /* Canvas */
  --lgi-canvas: #ecebe6;
  --lgi-canvas-hi: #f6f5f1;
  --lgi-canvas-low: #deddd7;
  --lgi-canvas-overlay: rgba(236, 235, 230, 0.92);
  --lgi-canvas-glass: rgba(246, 245, 241, 0.86);

  /* Ink */
  --lgi-ink: #121211;
  --lgi-body: #2d2d2a;
  --lgi-muted: #64635e;
  --lgi-ghost: #8c8a84;
  --lgi-on-dark: #faf9f5;

  /* Signature signal */
  --lgi-signal: #ef4f25;
  --lgi-signal-hover: #dd431e;
  --lgi-signal-ink: #a73317;
  --lgi-signal-soft: rgba(239, 79, 37, 0.12);
  --lgi-signal-faint: rgba(239, 79, 37, 0.06);

  /* Semantic */
  --lgi-success: #4b6525;
  --lgi-success-dot: #9fcb54;
  --lgi-success-soft: rgba(159, 203, 84, 0.14);
  --lgi-warning: #805500;
  --lgi-warning-dot: #c18a1a;
  --lgi-warning-soft: rgba(193, 138, 26, 0.14);
  --lgi-danger: #9e2517;
  --lgi-danger-soft: rgba(158, 37, 23, 0.10);
  --lgi-info: #345a6f;
  --lgi-info-soft: rgba(52, 90, 111, 0.10);

  /* Borders */
  --lgi-hairline: rgba(18, 18, 17, 0.14);
  --lgi-border: rgba(18, 18, 17, 0.20);
  --lgi-border-strong: rgba(18, 18, 17, 0.28);
  --lgi-border-solid: #121211;

  /* Typography */
  --lgi-font-sans: "PingFang SC", "Noto Sans SC", "Microsoft YaHei", system-ui, -apple-system, sans-serif;
  --lgi-font-mono: "SFMono-Regular", "JetBrains Mono", "Roboto Mono", "Noto Sans Mono CJK SC", ui-monospace, monospace;

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
  --lgi-shadow-contact: 0 1px 2px rgba(18, 18, 17, 0.08);
  --lgi-shadow-overlay: 0 12px 30px rgba(18, 18, 17, 0.12);
  --lgi-shadow-stage: 0 24px 25px rgba(18, 18, 17, 0.055);
  --lgi-shadow-hard: 8px 8px 0 rgba(18, 18, 17, 0.08);

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

## 语义护栏

| 领域 | 必须遵守 |
|---|---|
| Canvas | `canvas` 为暖灰主工作台，`canvas-hi` 为内容面，`canvas-low` 只作结构灰。`canvas-glass` 仅浮层，不建立玻璃拟态体系。 |
| Ink | `muted` 是功能文字最低灰度；`ghost` 只作不可交互刻度/禁用文字，不能承载关键数据或操作。 |
| Signal | 品牌活跃/选中/升级；不是 danger/warning/success。浅底小字号只能用 `signal-ink`。 |
| Semantic | 绿=Valid/Completed，琥珀=Partial/Aging/Retrying，深红=Failed/Invalid/删除，蓝灰=信息/链接；均需文字和图形/定位双通道。 |
| Type | Sans 承担中文阅读；Mono 承担机器语义。可对比数字要有 tabular nums；不得把 9px 校准刻度用在交互或事实。 |
| Spacing | 只用 4px 基数；不得新增 13/17/29px 等孤立对齐补丁。 |
| Radius/Shadow | 默认 0/2/4/8px 和无阴影。`shadow-hard` 只可用于 Tooltip/微型浮层；普通卡、行、表无阴影。 |
| Layout | L3 最大 1720px；L1/L2 最大 1440px；超宽屏增加外白而非拉宽正文。 |
| Motion | 只能用本表时长/缓动；不创建 137ms、430ms 等孤立值。 |

## Token 禁止项与迁移规则

- 禁止组件直接写已有的 `#ecebe6` 等数值，禁止新增 `blue-500` / `red-500` 通用库色。
- 禁止 `pt`、页面自定义最大宽度、12px 以上通用圆角、Clay/霓虹/玻璃阴影，或场景另造第二套 Signal 色。
- Token 值改变是跨页面设计变更：必须在 UI Change Manifest 中标为最高影响级，列出影响页面，更新预览/静态参考、迁移记录与回退办法。
- 引入运行时主题前先冻结迁移目标与单一文件位置；没有这一决定，不能提前添加 Web 目录或样式框架。
