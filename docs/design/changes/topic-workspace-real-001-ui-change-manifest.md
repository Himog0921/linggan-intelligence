# TOPIC-WORKSPACE-REAL-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: Issue #112 `/topics/{canonical_key}` / PAGE-TOPIC-WORKSPACE-001 的 UI 变更
> 事实来源: 用户授权、Topic 合同、Work Resource Read、LIDS、当前 Rust/CSS/JS/tests
> 冲突时以谁为准: 用户最新确认、AGENTS.md、PAGE、Topic 合同、LIDS 与真实运行证据
> 页面: `/topics/{canonical_key}` / PAGE-TOPIC-WORKSPACE-001

## 来源回执

| 来源 | 状态 | 本次用途 |
|---|---|---|
| AGENTS/current-state/Issue #112 Claim | 权威当前 | 范围、工作区、停工与交付门 |
| DISC-001 Topic 语言与不变量 | 权威当前 | Identity/Version/Run/Adjudication/Release 分责 |
| PAGE-TOPIC-001 | 静态 reference | 只吸收 L2 阅读节奏；不复用合成事实或假动作 |
| LIDS system/tokens/primitives/patterns/language | 权威当前 | Token → Primitive → Component → Pattern → Page |
| Work Resource Read | 当前代码合同 | 作品显示字段的唯一共享入口 |

## 变更分类与表面

- 混合变更：新页面 + 新交互 + 新状态语义；最高风险为把暂定研究冒充正式 Topic/Trend。
- 新一级入口：共享 Header 的“主题图谱”从 disabled 变为 `/topics/task-initiation-difficulty`，状态为“暂定研究”。
- 新 L2 页面：定义/版本、人工裁定、冻结材料、当前 Work Resource、来源边界。
- 新 loopback 读取和导入：页面只有 GET；POST 不在 UI 暴露。

## LIDS 影响

| 层 | 影响 |
|---|---|
| Token | 无新增；CSS 只消费现有 `--lgi-*` 与共享 `--v7-*` alias |
| Primitive | 复用共享 Focus、按钮、技术键、硬阴影、状态 soft fill |
| Component | page-local Topic role lens、material row、boundary strip；不晋升 CMP |
| Pattern | 新的 PAGE-TOPIC-WORKSPACE-001 L2 组合；不改 LIDS 全局 pattern |
| Page | 新 runtime route；旧 static PAGE-TOPIC-001 保持独立 |
| Data Truth | 增加 `PROVISIONAL + HUMAN_ADJUDICATED + FROZEN_PACK` 的真实表达 |

## 诚实状态和敏感边界

- 未读取、not found、schema unavailable、Work Resource unavailable 分开处理。
- 页面不显示 raw body/comment；前端只用 `textContent` 渲染来源字段。
- 角色和 rationale 是人工裁定，不是模型置信度；材料数只指 pack 内引用数。
- 没有正式发布、趋势、市场事实、Agent 命令、采集或成功回执。

## 响应式与动效

- ≥1080px 三列研究面；≤1080px Inspector 下移；≤720px 单栏。
- 选择、筛选与链接反馈使用 100–160ms LIDS motion；Reduced Motion 关闭全部过渡。
- 无 WebGL、图表、外部字体/脚本或远程资产。

## Shared Shell 窄屏可达性收口（2026-09-02）

- 触发事实：current-head 隔离浏览器在 `390×844` 发现 Topic 主工作区符合单栏要求、document 无横向 overflow，但 shared 一级导航的五个控制项仍各有至少 `86px` 宽度；最后的「采集」为 `344–430px`，超出 390px 视口。横向滚动容器不能替代所有一级职责在初始窄屏视图中可见、可触达的要求。
- 用户授权与分类：Mog 已授权在本 Issue 内根治该功能缺口；分类为 shared Shell 的展示 + 导航交互/无障碍修复。路由、当前项、禁用项、状态词、数据/权限/操作含义均不得改变。
- 表面与依赖：唯一实现 owner 是 `shell.css`；`shell.rs` 继续渲染同一语义化 `<nav aria-label="一级导航">`、真实 `<a>` 和 disabled `<button>`。受影响的是所有使用 `global_header` 的 Topic、Corpus 与 Collection 表面，而不是 Topic 页面局部 CSS。
- 目标行为：`≤640px` 一级导航使用可用宽度的自动分列网格，五项在 390px 同屏；若可用列数不足或将来新增职责，控制项自动换到下一行。不得保留横向滚动作为一级入口可达性的前提；每一项仍保留至少 24×24px 命中区、可见 focus、中文主语义和状态文字。窄屏可隐藏英文技术旁注，但不得隐藏中文职责、当前项或状态事实。
- 验收结果：source regression test 锁定窄屏 grid/wrap、取消 parent scroller 与无横滑依赖；隔离浏览器在 `390×844`、`430×932`、`1440×900` 核对所有五个一级入口均在可见可点击区域、当前项/禁用态/角色筛选保持。额外 `320px` 触发第二行而不裁断；最后 exact-stylesheet loopback 复验确认 parent global row 与 nav 均为 `overflow-x:visible`。390px 真实点击「采集」抵达 `/collection/attention`；启用原生链接保留 visible focus。console 和外部资源边界均为零；外部辅助技术覆盖仍为 NOT VERIFIED。

## 停止与交接

- 需要正式 Topic Release、自动分类、Claim、趋势、原文或采集时停止。
- 共享 Shell 的一级导航变更需要 integrator 注意与其它页面分支的行级冲突。
- 验收见 `ACC-TOPIC-WORKSPACE-REAL-001`；合并、迁移、部署和业务验收不由本 manifest 宣布。
