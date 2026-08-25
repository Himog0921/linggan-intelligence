# ACC-COLLECTION-001 · Collection Workspace 视觉与交互验收

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: `DESIGN-005` 交付的 `/collection/*` 五个子面
> 事实来源: 真实 loopback 运行结果、真实 Chrome 实拍、`cargo test -p linggan-api`
> 冲突时以谁为准: 真实运行/代码/合同、用户最新确认

## 1. 验收对象

- 事项: `DESIGN-005`（无 Issue）
- 页面: `/collection/targets`、`/collection/operations?mode=now|trace|review`、`/collection/attention`、`/collection/tasks`、`/collection/runtime`
- 关联 ID: `PAGE-COLLECTION-001`、`REF-V4-001`、`LIDS-SYS-001`、`LIDS-PRI-001`、`DESIGN-005-UI-EX-01`
- 环境: 2026-08-26，macOS Chrome 经 DevTools 协议驱动，本机 Rust host + Docker PostgreSQL 16
- 数据前提: 无观察目标、无任务、无事件、无执行工位；调度器未接通

## 2. 场景矩阵

| 场景 | 用户任务 | 预期含义 | 结果 |
|---|---|---|---|
| 五个子面导航 | 在采集内部切换 | 每个子面回答各自唯一主问题，导轨选中态明确 | 通过 |
| 运行态三模式 | 切换并分享地址 | 三个模式都是真实地址，刷新回到同一视图 | 通过 |
| 目标抽屉打开 | 用地址直接打开某个对象 | 抽屉自右侧进入、锚定全局页头下方、左侧列表保留上下文 | 通过 |
| 抽屉 tab 与宽度 | 切 tab、切宽度 | 状态写入地址栏；刷新后 tab 与宽度都恢复 | 通过（实测 `?drawer=T-CR-019&tab=patrol&wide=1`）|
| 标识无法解析 | 打开一个不存在的对象 | 明确说「这个标识没有对应的观察目标」，不伪造对象 | 通过 |
| 空态诚实 | 判断系统现在能做什么 | 每个子面说明现在没有什么、为什么、不代表什么 | 通过 |
| 会消耗平台访问的动作 | 尝试新建观察目标 | 显式不可用并标注「需要采集授权」，不是点了没反应 | 通过 |
| 实时流未接通 | 判断系统是否在观察 | 显示 NOT CONNECTED，明确不会用计时器伪造事件 | 通过 |

## 3. 视觉工作条件

- 视口: 1920×1080 真实 Chrome 实拍（含抽屉常规宽度与 WIDE 两态）
- 几何: 128px 全局页头（78 + 50）、216px 导轨、抽屉 72vw / 86vw、运行态右栏 450px
- Token 依据: 页面样式表内 `#` 计数为 0；`--lgi-stream-*` 十项进入唯一色值源，运行时与镜像文档逐项校验通过（127 项）
- 深色面: 扫描线与环境亮按 Gold Master 的 2.5% / 7% 强度实现；首版误用 20% 结构线强度导致整片泛绿，已修正后复拍
- 排版: 深色面功能文字提到 11px 下限；仅时间戳与刻度类允许 9px
- Reduced Motion: 抽屉过渡在 `prefers-reduced-motion:reduce` 下关闭

## 4. 分层结论

| 完成层 | 结论 | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | 逐条对照 PAGE 规格、LIDS 与 `REF-V4-001` 可借鉴范围 | 深色面为已记录例外 |
| 前端实现 | VERIFIED | 1920×1080 实拍五个子面 + 抽屉四种状态 | 未覆盖 1440×900、1280×800、390×844 |
| 自动检查 | VERIFIED | `cargo test -p linggan-api` 14 passed / 5 ignored；fmt、clippy、governance 全绿 | 5 项需隔离 PostgreSQL 证明库 |
| 共享页头无回归 | VERIFIED | 抽取前后 `/corpus/evidence` HTML **逐字节一致** | — |
| 真实链路 | N/A | 本次无任何读写或外部请求 | — |
| Mog / 业务验收 | NOT VERIFIED | 待 Mog 在浏览器确认 | — |

## 5. 明确未证明

- 目标行的六列布局、创作者作品生命周期图、关键词结果景观图**没有渲染对象**，因此其几何未经验证。它们随数据接通时一并落地并复验。
- 抽屉的「切换到另一个对象」行为无法验证：列表中没有可点的行。
- 键盘可达性只验证了 Escape；完整 Tab 顺序与焦点环覆盖未系统检查。
- 本次不证明任何采集能力、调度、实时推送、目标创建或平台访问。
