# DESIGN-004 · Corpus Rail 面层差异化与选中态重音 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: Evidence Library 左侧 216px Corpus rail 的面层、导航项常态/hover/禁用态与选中态重音
> 事实来源: Mog 于 2026-08-25 在会话中的直接指定与参考图、DESIGN-003 已确认基线、LIDS、当前实现与实际浏览器核对
> 冲突时以谁为准: 用户最新确认、AGENTS.md、UI 执行合同、真实代码/合同；本清单不扩大任何数据或行动授权

## 1. 事项

- Issue / Scope: 无 Issue。Mog 在 DESIGN-003 合并（`d6f402b`）之后于会话中直接指定；`LOCAL-001 / 001A` 页面范围内
- Agent 与 worktree: Claude Code 会话，主工作树 `/Users/moglenny/proma/linggan-intelligence`，base `d6f402b`
- 目标: 让左侧 rail 从与主工作面同色的白板，变成有层次、有质感的次级面；并把「当前所在位置」做成可一眼识别的粗野重音
- 用户可见结果: rail 底色降到 `--lgi-canvas-low` 并承载与上下文行同款双层点阵测量场，两者在垂直方向连成一根立柱；选中项保持实心墨面，新增自右边缘向左离散衰减的像素纹理、实色硬阴影与 `translate(-2px,-2px)` 位移；导航序号由 9px 提升到 11px；hover 改为白实面浮起
- 明确非目标: 数据来源、读投影、查询、状态判定、真实平台、媒体、Agent、部署；不接通任何当前 disabled 的导航路由；不改 HTML 结构、不改文案、不新增导航项

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `current-state.md` | 已读 | 授权层级、代码前语义冻结范围、协作纪律 | 2026-08-25 |
| UI execution contract | 已读 | 前置读取、闭集执行、停止条件与收口要求 | 2026-08-25 |
| `design-governance.md` | 已读 | 变更分类与最小授权、例外记录方式 | 2026-08-25 |
| `PAGE-EVIDENCE-001` | 已读 | rail 的页面职责为「给 Evidence Library 一个稳定的 L1 定位」，非目标不含视觉分层 | 2026-08-25 |
| LIDS `system.md` | 已读 | L1 纹理条款（点阵测量场合法、渐变/光晕/噪点禁止）、粗野重音每屏 8 处上限、边界优先次序 | 2026-08-25 |
| LIDS `primitives.md` | 已读 | 线条六原则第 2 条「面代替线」、粗野位移受限允许清单含「当前选中项」、禁用态与排版下限 | 2026-08-25 |
| LIDS `agent-execution-guide.md` | 已读 | 容器决策树「次级 → soft」「当前对象 → signal-soft/rail」、动效时长档 | 2026-08-25 |
| DESIGN-003 变更清单 | 已读 | 纯白基线、点阵纹理来源、页面 CSS 不 author 色值的纪律 | 2026-08-25 |
| 数据 / 权限 / 行动合同 | 已读 | 本次不触碰；空态与未知表达逐字未动 | 2026-08-25 |
| 当前 Rust 代码与测试 | 已读 | CSS 经 `include_str!` 编译进二进制；测试锁定几何常量与「CSS 不得出现 `#`」 | 2026-08-25 |

## 3. 变更分类

- 分类: 展示
- 最高风险类别: 展示
- 对应来源 ID: `PAGE-EVIDENCE-001`、`LIDS-SYS-001`（L1 纹理、重音上限）、`LIDS-PRI-001`（线条六原则、粗野位移、排版下限）、`LOCAL-001-UI-EX-01 / DESIGN-003 修订`
- 为什么该类别足以覆盖本次风险: 只改变已有导航项的呈现层，不新增/删除任何导航项、状态、字段或动作；所有导航项的 `disabled`、`aria-disabled`、`aria-current` 与文案逐字未动，页面对数据的任何陈述未被触及
- 是否存在 `DECISION_REQUIRED`: 否
- L1 / L2 / L3 与主 Pattern: L1 `Corpus Explorer`，无变化
- 是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth: 不新增/修改任何 `--lgi-*` token；无新 CMP、无 Scene；Motion 仅新增选中态的常驻位移（已在 `prefers-reduced-motion` 下关闭）；Data Truth 不变

## 4. 逐项变更与理由

| 变更 | 变更前 | 变更后 | 规则依据 |
|---|---|---|---|
| rail 面 | `--v7-white`，与主工作面同色，靠 2px 黑线分隔 | `--v7-gray` + 56px/14px 双层点阵 | `primitives.md` 线条原则 2「面代替线」；`system.md` L1 纹理条款；决策树「次级 → soft」 |
| 导航项常态 | 自带白底 | `transparent`，透出测量场 | 同上；避免在已有底色差异处再堆一层面 |
| hover | 底色变 `--v7-gray`（与新面色相同，已失效） | 白实面浮起 + 序号转 signal | 面代替线；hover 需与新底色形成反差 |
| 序号字号 | 9px Mono | 11px Mono `600` | `system.md` §4.3 功能文字不小于 11px；9px 仅限不可交互装饰刻度 |
| 序号常态色 | `--v7-muted` | `--v7-ghost` | `primitives.md` 禁用态：未接通能力的文字降到 ghost，让不可用一眼可辨 |
| 选中态 | 实心墨面 + 白字 + signal 序号 | 追加像素纹理、`--v7-brutal` 实色硬阴影、`translate(-2px,-2px)` | `primitives.md`「粗野重音的位移」明确把「当前选中项」列入受限允许清单，位移量与阴影档位按该节固定值 |
| 选中态纹理 | 无 | 6px 棋盘像素，自右边缘向左四档离散衰减，固定 84px 宽 | `system.md` ASCII/终端语言约 5% 视觉权重；离散分档而非连续渐变，规避 L1 渐变禁令；固定像素宽度使移动端全宽时不等比放大 |
| 栏脚分隔 | `--v7-line` | `--v7-line-strong` | 灰底上 hairline 对比不足；仍属两级线宽中的内容级 |

### 参考图的取舍

Mog 提供的按钮参考图（深色胶囊 + 像素纹理）只吸收像素纹理一层。以下三项不采纳，理由为与 Mog 本人在 DESIGN-003 中的确认直接冲突：

| 参考图元素 | 不采纳理由 |
|---|---|
| 厚胶囊圆角 | `system.md` §4.5 只允许 0/2/4/8px 圆角，并点名「不以厚胶囊制造 AI 感」 |
| 柔和模糊阴影 | `primitives.md` 明确模糊阴影仍在禁止之列，被解除的只是位移本身 |
| 暖棕底色 | DESIGN-003 已把暖色系整体换向为冷黑 `#111315` |

### 重音计数

本次不新增任何重音元素，只强化已有的一处。当前屏承担粗野重音的元素为：品牌标识、rail 选中项、系统视图选中项、FACT LAYER 条、系统边界徽章、搜索框、空态上下粗线，计 7 处，仍在每屏 8 处上限内。

## 5. 数据诚实性核对

| 项 | 改动前 | 改动后 | 结论 |
|---|---|---|---|
| 五个导航项的 `disabled` / `aria-disabled` | 全部禁用 | 逐字不变 | 未把未接通路由伪装成可用 |
| `aria-current="page"` | 仅「01 证据库」 | 逐字不变 | 选中态强化的是真实当前位置，不是新增声明 |
| 栏脚 `ACCEPTED DISCOVERY ONLY` 与说明 | 原文 | 逐字不变 | 未削弱来源边界 |
| 页面其余区域 | — | 未触碰 | 空态、UNKNOWN、Coverage 表达全部原样 |

## 6. 影响边界

- 受影响的页面: 仅 `/corpus/evidence`
- 受影响的组件: 无跨页面组件；改动全部在 `evidence_library.css` 的 rail 区块
- 受影响的状态: 无。选中态是导航位置的呈现，不是数据状态
- 是否影响数据口径、权限、敏感展示或真实行动: 否
- 禁止修改的文件/能力: `lids_tokens.css`（token 真源）、`local_web.rs` 的 HTML 与文案、数据/迁移/插件/平台相关全部范围
- 停止条件: 若需要新增导航项、接通任何路由、改变 `disabled` 语义或新增 token，停止并另立卡

## 7. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 本地 host + `GET /corpus/evidence` | VERIFIED，页面正常渲染，导航语义未变 | 不证明任何路由已接通 |
| 状态诚实 | `cargo test -p linggan-api` + HTML 逐项比对 | VERIFIED，8 passed / 5 ignored；禁用与 aria 属性未变 | 5 项 PostgreSQL 用例仍需隔离证明库 |
| 视觉一致 | 真实 Chrome 在 1440×900、1280×800、390×844 实拍 | VERIFIED，见 `ACC-RAIL-004` | 未覆盖 1536×960、1728×1117、1920×1080、2560×1440、430×932 |
| 真实后果 | 无动作变更 | N/A | 本次不产生任何写入或外部副作用 |

## 8. 交接

- 修改文件: `apps/api/src/local_web/evidence_library.css`、本清单、`ACC-RAIL-004`、`lids/migration-log.md`、`PAGE-EVIDENCE-001`
- 验证命令/走查: `cargo test -p linggan-api`、`cargo fmt --all -- --check`、`cargo clippy --workspace -- -D warnings`、`./scripts/check-project-governance.sh`、真实 Chrome 三视口实拍
- 规则或索引同步: `lids/migration-log.md` 追加 DESIGN-004 段；`PAGE-EVIDENCE-001` 的 `LOCAL-001-UI-EX-01` 条目标注「全白背景」已被 DESIGN-003/004 部分替代
- 例外与替代: rail 与主工作面之间的 2px 实黑线在获得底色差异后予以保留。按线条原则 2 的字面要求可以去除，此处保留的理由是它与页头、上下文行的 2px 黑线构成同一层结构骨架，属于「结构级 2px 实黑」的合法用法而非给小元素套框。复核触发条件：若后续出现第二个使用测量场底的次级面，需统一裁定是否去线
- LIDS migration log / 预览同步: 运行时页面即预览；截图未登记进 Git
- PR / reviewer / integration owner: 本次为主工作树直接提交，未开 PR；reviewer 与 integration owner 留给非实现者
