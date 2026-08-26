# PAGE-PLUGIN-001 · Linggan Browser Producer Popup

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `PLUGIN-MIGRATION-001` 自有 MV3 Browser Producer popup；只表达身份、本机接纳准备度和用户手势触发的受限 Discovery
> 事实来源: Issue #37、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、`LIDS-PAT-001`、`LIDS-PRI-001` 与 `PLUGIN-MIGRATION-001` 活跃计划
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前 Discovery 合同和实际 plugin/runtime receipt；本页不扩大采集权限

## Issue #53 · v0.4.2 启动失败补充

`POPUP_STARTUP_FAILURE` 只表示 toolbar popup 的 React 渲染发生错误；它不是本机 host、Discovery、接纳、Evidence 或平台状态。它保持 `L1 / Settings / Governance`：使用既有 `InstrumentSurface` 的 canvas-hi、mono readout、文字状态和 LIDS 批准的 2px 结构线，不晋升 CMP、不新增 Scene 或 Motion。`popup.html` 从唯一值源 `apps/api/src/local_web/lids_tokens.css` 的构建副本加载 `themes/lids-tokens.css`；此局部失败 Surface 只消费 `--lgi-*` token，不创建第二套 token 值。

| 失败状态 | 触发与可见内容 | 可以诚实说明 | 明确不能说明 | 用户下一步 / receipt |
|---|---|---|---|---|
| `POPUP_STARTUP_FAILURE` | `App` 初始渲染或后续渲染错误；显示版本、启动失败、`本机状态目前未知` 和本提示未发起新的采集或传输 | 此提示自身没有新 collector 或交付动作；本机状态未知 | 失败前 `App` 未读取 tab/context/storage；host 已就绪；任务成功；任何材料已接纳或入库 | 在 Chrome 扩展程序页面重新加载明确版本后重新打开；无 receipt、无按钮行动 |

此路由不是 Dashboard fallback，也不得改变已有 popup、内容页浮条、Collector、TaskSpec、回传、权限、host permission、API、数据库或真实浏览器配置。若实现需要任一上述变化，停止并另立事项。

## 用户任务与 3 秒答案

用户打开 popup 的唯一任务是确认：这是不是 Linggan 自有 Producer、它只连接哪个本机地址、当前本机 host 是否可达、以及能否开始 Discovery。

首屏必须在 3 秒内回答：

```text
Linggan-owned package / version
→ localhost:3000
→ local ingress ready, host only reachable, or unreachable
→ current search page can or cannot run the one allowed Discovery
```

## LIDS 组合

- 强度与 Pattern：`L1 / Settings / Governance`；不构成完整产品页面、导航或 Collection Control。
- Token：构建时从 `apps/api/src/local_web/lids_tokens.css` 复制唯一运行时 `--lgi-*` 值源到安装包；popup CSS 只消费这些 token，不能独立定义第二套数值主题。
- Primitive：`InstrumentSurface` 的 canvas-hi 分组、Mono readout、带文字的 Operation/Validity 状态与单一用户动作按钮；没有通用 CMP 晋升。
- 状态：`LOCAL_INGRESS_READY` 只表示 `/health` 报告已知 Discovery 接纳路径；`LOCAL_HOST_REACHABLE` 只表示 host 可达但不可提交；`LOOPBACK_UNREACHABLE` 表示本机不可达；`ACCEPTED` / `REPLAY` / `NOT_ACCEPTED` 只表示这一次提交的 receipt。它们都不能表示 Evidence 已入库或页面已更新。

## 信息与动作边界

| 区域 | 可见内容 | 明确不能表示 |
|---|---|---|
| Package | `LINGGAN / version` | 浏览器已经加载当前版本或真实 Producer 已验证 |
| Loopback target | 固定 `localhost:3000` | ingress、数据库或页面读模型已经接通 |
| Local host | `/health` 的最小准备度结果 | Discovery Package 被接受、Evidence 已入库 |
| Discovery intent | 仅 `ADHD` 搜索页、可见“综合”、当前面、最多 20 张卡片 | 所有搜索页面、隐藏内容或历史内容都可采 |
| Discovery action | 用户明确点击后的一次 visible-card 提交 | 详情、评论、作者、媒体、OCR/ASR、后台采集已经获准 |
| Receipt | `ACCEPTED` / `REPLAY` / `NOT_ACCEPTED` / `NOT_READY` 与本次 `visibleCards` / `stoppedReason` | Evidence Library 已出现材料或平台采集完整 |

唯一 Discovery 按钮只在 `LOCAL_INGRESS_READY` 时可点击；每次点击重新检查本机状态，并只能读取用户当前激活的匹配搜索页。任何不满足条件的结果必须显示 `NOT_READY` 与原因，且不得提交。Popup 不显示平台封面、卡片、正文、评论、Cookie、账号或任何原始材料。

## 响应式、可访问性与非目标

- popup 采用单列、无动画的 L1 Surface；没有第二套 Header、导航、场景、悬浮卡或未经批准的视觉资产。
- button 只代表当前一次有限 Discovery；不可据此扩为自动、详情、评论或媒体动作。任何新增能力必须通过新 Issue 重定义其动作、回执与状态。
- `LOCAL_HOST` readout 的 title 可提供非敏感说明；关键状态同时由文字和颜色表达。
- 本卡不验证真实浏览器加载、视口截图、实际 health 请求或屏幕阅读器；这些是视觉验收中的 `NOT VERIFIED`，不以静态 HTML 冒充。
