# ACC-RAIL-004 · Corpus Rail 面层与选中态视觉验收

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: DESIGN-004 对 Evidence Library 左侧 rail 的呈现层变更
> 事实来源: 真实 loopback 运行结果、真实 Chrome 实拍、`cargo test -p linggan-api`
> 冲突时以谁为准: 真实运行/代码/合同、用户最新确认；截图不得单独覆盖任何一层

## 1. 验收对象

- Issue / SCOPE: 无 Issue，Mog 于 2026-08-25 会话直接指定；`LOCAL-001 / 001A`
- 页面/组件/状态: `/corpus/evidence` 左侧 216px Corpus rail；导航项常态、禁用态、选中态
- 关联 ID: `PAGE-EVIDENCE-001`、`LIDS-SYS-001`、`LIDS-PRI-001`、`DESIGN-004`
- 验收日期与环境: 2026-08-25，macOS Chrome 经 DevTools 协议驱动，本机 Rust host + Docker PostgreSQL 16
- 适用数据/权限前提: 数据库已连接（`LOCAL_TRUSTED_PRODUCER` / 库内无窗口内卡片），rail 全部导航项仍为未接通

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 当前位置识别 | 一眼看出自己在「证据库」 | 选中项表达导航位置，不表达数据状态 | 实心墨面 + signal 序号 + 像素纹理 + 硬阴影 + 位移；纹理不覆盖任何文字 | 无动作 | 通过 |
| 未接通路由 | 分辨 02–05 尚不可用 | 路由未接通，不是「点了没反应」 | 序号降到 ghost、无 hover 响应、光标 `not-allowed` | 无动作 | 通过 |
| 面层分层 | 分辨 rail 与主工作面是两个层 | rail 与主工作面同为白面，靠 2px 结构线分隔 | 纹理只出现在上下文行，rail 不复用 | 无动作 | 通过（2026-08-26 修订后）|
| 移动折叠 | 390 宽下 rail 转为全宽区块 | 结构降级但语义不变 | 选中项纹理固定 84px 不随块宽等比放大；重音不失控 | 无动作 | 通过 |

本页当前没有可用的 hover / focus 实景：五个导航项全部 `disabled`，因此 hover 与 `:focus-visible` 规则**已写入但未在真实交互下验证**，记为 NOT VERIFIED。

## 3. 视觉工作条件

- 声明的视口: 1440×900、1280×800、390×844（真实 Chrome 实拍）
- 输入内容长度与数据密度: 五个固定导航项，最长「已存查询」四字；无真实材料
- 已批准的设计规则: DESIGN-003 纯白基线、线条六原则、粗野重音位移受限允许。L1 点阵测量场经 2026-08-26 Mog 判定不适用于本页 rail，见 DESIGN-004 §9
- LIDS 强度 / Pattern / Token 依据: L1 `Corpus Explorer`；全部视觉值经 `--v7-*` 别名解析到 `--lgi-*`，页面 CSS 内 `#` 计数为 0
- 状态五轴或合成边界依据: 本次不涉及状态轴；rail 选中态属导航位置，未借用任何真实状态标签
- Reduced Motion 降级: `prefers-reduced-motion:reduce` 下关闭全部过渡，并把选中项的常驻位移置为 `none`；硬阴影保留（非运动属性）
- 已检查的响应式/可访问性条件: 三视口无页面级横向滚动；序号字号升至 11px 满足功能文字下限；`aria-current`、`aria-disabled` 未变
- 截图证据位置: 会话内实拍，未登记进 Git

## 4. 分层结论

| 完成层 | 结论 | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | 逐条对照 `system.md` L1 纹理条款、`primitives.md` 线条与位移条款 | 2px 分隔线的保留属显式例外，已记入 DESIGN-004 §8 |
| 前端实现 | VERIFIED | 真实 Chrome 三视口实拍（08-25 灰底版）+ 1440×900 复核（08-26 白底版）| 白底版未在 1280×800 与 390×844 复拍；面层回滚不改变几何，风险低但未实测 |
| 自动检查 | VERIFIED | `cargo test -p linggan-api` 8 passed / 5 ignored | 5 项需隔离 PostgreSQL 证明库 |
| 交互态（hover / focus） | NOT VERIFIED | 全部导航项当前 `disabled`，无法触发 | 待任一路由接通后补验 |
| 真实链路/回执 | N/A | 本次无动作变更 | — |
| 部署 | N/A | 仅 loopback | — |
| Mog / 业务验收 | NOT VERIFIED | 待 Mog 在浏览器实际确认 | — |

## 5. 发现与后续

- 发现的规格冲突: 线条原则 2「面代替线」在本页被实测推翻——当页面已有一条以纹理承担语义的横向带时，纵向面复用同款纹理会消掉那一层的唯一性，减少的线条不抵消失的层次。rail 面层已按 Mog 判定回滚为纯白，两块面之间继续由 2px 结构线分隔
- 是否需要 DECISION_REQUIRED: 否
- 是否需要设计例外或长期决定: 若未来出现第二个测量场次级面，2px 分隔线的去留应升级为跨页面裁定
- 不得因此推断的结论: 不得据此认为 rail 任一路由已接通、hover/focus 已验证、键盘可达性已补全，或 LIDS 已升至 STABLE
