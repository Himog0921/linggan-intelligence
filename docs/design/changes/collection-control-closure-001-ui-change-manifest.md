# COLLECTION-CONTROL-CLOSURE-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-04
> 适用范围: Issue #149 的 Collection Targets 规则 modal 与 Operations/Attention/Tasks/Runtime 控制事实表达
> 事实来源: Issue #149、PAGE-COLLECTION-001、COLLECTION-CONTROL-CLOSURE-001 active plan、LIDS v7 与当前 Rust/server-rendered UI
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实服务端合同/数据库回执、PAGE-COLLECTION-001 与 LIDS；本清单不增加权限

## 读取回执

已核对 UI execution contract、design governance、LIDS README/tokens/materials/data-boundaries/language/primitives/patterns/decisions/agent guide、PAGE-COLLECTION-001、collection-monitoring-rules、Issue #149 和 exact-base 代码。历史内容工作台只提供“按钮+弹窗、暂停未来调度、人工重观察、失败分层”的行为对标，不提供实现或视觉权威。

## 范围与分类

- 变更分类: 权限/行动混合变更。
- 页面强度: Collection L1；规则 modal 是当前目标的受限 L2 overlay。
- 唯一 Pattern: Collection Control。
- 新增: 目标行“监控规则”入口、服务端渲染 modal、版本/模式/窗口/fallback/receipt、五个 Collection 面的统一 reason/frozen refs。
- 不新增: 独立规则页、Evidence/Corpus 内容、监控价值、Opportunity、Dossier、Agent、移动/窄屏适配。
- 支持条件: 1440 CSS px desktop fullscreen only。

## 表面与状态

| 表面 | 状态来源 | 呈现责任 |
|---|---|---|
| 规则入口 | target + active rule pointer | 入口永久可发现；dismissed 目标显示不可编辑理由 |
| modal form | rule revision + command receipt | 中文独立表达；expected revision/idempotency 隐藏但真实提交 |
| modal feedback | durable receipt outcome/reason | success/replay/stale/conflict/rejected 分开；失败保留输入 |
| dynamic | comparable-round qualification | `DYNAMIC_UNAVAILABLE` 与 24h fixed fallback 同时可见，不渲染假计算 |
| Runtime | capacity evaluator | 每个阻断 reason、freshness、来源和恢复责任可解释 |
| Operations/Attention/Tasks | scheduler/decision/work/lease/task/receipt | 只显示 durable fact；UNKNOWN 不是 0/失败，PARTIAL+VALID 不进失败区 |

## 交互与 a11y

入口记录 opener；打开后首焦点进入标题/首控件；Tab/Shift+Tab 在 modal 内循环；Esc 与关闭按钮均返回原 opener。提交失败仍渲染相同字段值与字段级错误。保存规则只返回“规则已保存/重放/拒绝”，绝不显示“采集成功”。所有功能文字 ≥11px，Focus 使用全局 `--lgi-focus`，不加渐变、字符图标、原生 alert/confirm/prompt 或 hover-only action。

## 验收边界

自动检查覆盖表单闭集、revision/idempotency、错误保留、敏感字段负向、规则不创建执行事实、五面 reason 一致。隔离 1440 浏览器覆盖首焦点、Tab trap、Esc、focus return、刷新后 receipt/state、无横向溢出和 console error。真实平台、Chrome loaded extension、共享 runtime/DB、部署与业务验收保持 NOT VERIFIED。

## V4 五页面视觉复刻扩展（2026-09-04）

Mog 以本地 `linggan-collection-field-workspace-v4-ia-cn.html` 作为五个 Collection 页面新的视觉与交互对标。该文件是 synthetic prototype，只提供几何、层级、密度和交互关系，不提供事实、权限或产品语义；其中模拟数字、Evidence tab/卡片、监控价值式健康分和未接通动作均不进入实现。

- 使用者与上下文: 产品负责人和小团队在 1440 CSS px 桌面全屏中处理采集运营。
- 审美方向: LIDS 白场研究仪器工作站；硬结构线、高密度 ledger、克制 signal、单一 dark observation instrument。
- 第一记忆点: 五页一致的“标题 → 五格事实读数 → 控制条 → 主工作区”，以及对象选择后右侧事实区。
- 硬约束: Rust SSR + page-local vanilla CSS/JS；只消费现有 token；中文独立成立；UNKNOWN 与 0 分开；不做窄屏。
- 签名交互: ledger 行选择更新 inspector；目标行继续由 URL 打开宽幅 lifecycle drawer；Esc 返回 opener。
- CSS 策略: 复用现有 `collection_workspace.css`、`target_drawer.css` 与 `collection_workspace.js`，不新增 framework、字体、图标库、全局 header 或第二套 token。

用户表面、Reality Matrix、依赖、停止条件和 1440 验收矩阵登记在 `docs/plans/active/collection-control-closure-001.md` 的 `COLLECTION-FIVE-PAGE-V4-UI-001` 扩展中。

## V4 实施回执

- 共享壳层仍由 `shell.rs` 所有；V4 只进入 Collection page scope，没有复制 header/context/rail 或新建 token。
- `collection.rs` 提供五页可见标题、五格读数与稳定 body/readout slot；各真实 projection 在读取成功后替换相应 slot。
- Attention、Tasks 的行选择只更新右侧事实区，不提交写入；Operations dark instrument 只显示持久 scheduler decision；Targets drawer 仍由 URL 持有，默认展示 creator lifecycle；Runtime 继续以服务端 capacity 为唯一准入结论。
- 页面可见文案不包含 Evidence 或“监控价值”模块；语料内容与价值判断继续归属 Corpus。合法 lifecycle 状态均有中文标签。
- 隔离 1440×900 浏览器对五页逐一检查，得到 `scrollWidth = clientWidth = 1440`、每页 5 个 readout、0 条 console warning/error；Attention 选择、Task tab、creator lifecycle drawer 均实测成立。fixture 为 synthetic，不能代表共享数据库或真实平台。
