# PAGE-PLUGIN-001 · Linggan Browser Producer Popup

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `PLUGIN-001` 自有 MV3 Browser Producer 的最小 popup；只表达身份、loopback health 与未接通的 Discovery 边界
> 事实来源: Issue #33、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、`LIDS-PAT-001`、`LIDS-PRI-001` 与 `PLUGIN-001` 活跃计划
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前 Discovery 合同和实际 plugin/runtime receipt；本页不授予采集动作

## 用户任务与 3 秒答案

用户打开 popup 的唯一任务是确认：这是不是 Linggan 自有 Producer、它只连接哪个本机地址、当前本机 host 是否可达、以及能否开始 Discovery。

首屏必须在 3 秒内回答：

```text
Linggan-owned package / version
→ localhost:3000
→ host reachable or unreachable
→ Discovery remains NOT_CONNECTED and NOT_AUTHORIZED
```

## LIDS 组合

- 强度与 Pattern：`L1 / Settings / Governance`；不构成完整产品页面、导航或 Collection Control。
- Token：构建时从 `apps/api/src/local_web/lids_tokens.css` 复制唯一运行时 `--lgi-*` 值源到安装包；popup CSS 只消费这些 token，不能独立定义第二套数值主题。
- Primitive：`InstrumentSurface` 的 canvas-hi 分组、Mono readout、禁用 Quiet/Secondary 边界按钮、带文字的 Operation/Validity 状态；没有通用 CMP 晋升。
- 状态：`LOCAL_HOST_REACHABLE` / `LOOPBACK_UNREACHABLE` 仅表示 loopback health 结果；`NOT_AUTHORIZED` 表示没有 Work Order；`NOT_CONNECTED` 表示 Discovery ingress 尚未实现。它们不能合并为“采集成功”或“系统异常”。

## 信息与动作边界

| 区域 | 可见内容 | 明确不能表示 |
|---|---|---|
| Package | `LINGGAN / version` | 浏览器已经加载当前版本或真实 Producer 已验证 |
| Loopback target | 固定 `localhost:3000` | ingress、数据库或页面读模型已经接通 |
| Local host | 仅 `/health` 的最小可达性结果 | Discovery Package 被接受、Evidence 已入库 |
| Authorization | `NOT_AUTHORIZED` | 用户没有账号、平台无法访问或数据不存在 |
| Discovery | 固定 Canary 意图与 disabled 状态 | 点击可开启采集、详情/评论/媒体已经获准 |

唯一 Discovery 按钮必须 disabled；禁用原因以同一区域常驻文字提供，不依赖 Tooltip。Popup 不显示平台封面、卡片、正文、评论、Cookie、账号或任何原始材料。

## 响应式、可访问性与非目标

- popup 采用单列、无动画的 L1 Surface；没有第二套 Header、导航、场景、悬浮卡或未经批准的视觉资产。
- button 保持禁用且不可点击；当未来获得真实 ingress/授权时必须通过新 Issue 重定义其动作、回执与状态，而不是移除 `disabled`。
- `LOCAL_HOST` readout 的 title 可提供非敏感说明；关键状态同时由文字和颜色表达。
- 本卡不验证真实浏览器加载、视口截图、实际 health 请求或屏幕阅读器；这些是视觉验收中的 `NOT VERIFIED`，不以静态 HTML 冒充。
