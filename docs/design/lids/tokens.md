# LIDS-TOK-001 · Token 基线与唯一数值来源

> 状态: 权威当前
> 运行时状态: LOCAL-001A 已获准的唯一运行时 Token 文件存在；本 Markdown 是其版本化规范与校验基线
> 最后核对: 2026-08-25
> 适用范围: Linggan Intelligence 未来主题、Primitive、Component、Pattern、Page、动效和受控场景的颜色、排版、间距、边界、层级与性能数值
> 事实来源: Mog 指定的 LIDS v2.0 `tokens.md`（SHA-256: `97fac0fb590c7f349f5fe7bfc2e2e423c6c8145b03cec9ffdd80d4494f757e77`）、[system.md](system.md)、LOCAL-001A 运行时 Token 源与对应 Rust 校验
> 冲突时以谁为准: 已获准运行时的唯一 Token 文件 `apps/api/src/local_web/lids_tokens.css` 优先；本 Markdown 是该源的版本化规范与校验镜像，不是第二个可独立编辑的运行时主题。产品/数据/权限冲突不由 Token 解决

本文件冻结 LIDS v2.0 的 107 项数值基线。LOCAL-001A 已把同名完整基线迁入 `apps/api/src/local_web/lids_tokens.css`；该 CSS 是当前唯一的**值编辑源**。本 Markdown 中的 CSS 清单是从该源派生的版本化规范与校验镜像，不能作为第二套运行时主题或独立改值入口。Rust 测试逐项核对 107 个 token 的名称和完整值，防止两份文本无声漂移。

## LOCAL-001A 运行时权威与后续 Agent 规则

```text
唯一值编辑源
apps/api/src/local_web/lids_tokens.css
        ↓ 同一提交内同步为镜像
docs/design/lids/tokens.md
        ↓ Rust 逐项名称和值校验
运行时页面与版本化规范
```

- 后续 Agent 要改变某个 LIDS token 值时，只能先修改 `apps/api/src/local_web/lids_tokens.css`；不得先在页面 CSS、组件 CSS 或本 Markdown 中创造不同值。
- 同一提交必须把本文件的基线清单同步为运行时源的精确镜像，并通过 Rust 的 107 项名称→值比对。校验失败即表示变更未完成，不能用“名称一致”掩盖值漂移。
- 任何 token 值的改变仍是跨页面设计变更：必须按 UI Change Manifest、迁移记录、影响页面和回退规则完成治理；LOCAL-001A 不因拥有运行时源而获得任意改值授权。
- 页面 CSS 只消费 `var(--lgi-*)`，不得声明 `--lgi-*`；页面局部例外必须按 PAGE/Manifest/LIDS migration log 另行登记，不能反向写入 Token 真源。
- 如未来需要自动生成 Markdown 镜像，必须另开受控任务；在此之前，本文件的镜像同步与 Rust 精确校验共同构成当前最小、可验证的单向关系。

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
- 运行时 Token 已在 LOCAL-001A 以 `apps/api/src/local_web/lids_tokens.css` 落地；后续不得新建第二个运行时 Token 文件、页面级主题或值编辑入口。新增自动生成工具或改变该单向关系，必须另开受控任务。
