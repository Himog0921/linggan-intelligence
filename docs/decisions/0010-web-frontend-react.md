# DEC-0010 · 网页前端统一为 React + TypeScript

> 状态: 权威当前
> 最后核对: 2026-10-15
> 适用范围: Intelligence 全部正式网页的前端技术、迁移终态与「不留双轨」规则；被本决定取代的旧条款
> 事实来源: Mog 2026-10-15 在本机 Claude 会话中的明确决定（[Issue #430](https://github.com/Himog0921/linggan-intelligence/issues/430)）；同日对 `origin/main@473980cb` 代码、文档与提交历史的只读核对
> 冲突时以谁为准: Mog 最新确认、AGENTS.md、真实代码与运行证据

## 背景

立项讨论稿 [`采集与数据底座战略讨论稿 v1.0`](../../references/strategy-originals/Linggan_Intelligence_采集与数据底座战略讨论稿_v1.0.md) 第十五节「技术路线判断」推荐的组合是 Rust Backend + PostgreSQL + Object Storage + React + Browser Extension。

转为正式决定时，[ADR-0001](0001-greenfield-rust-clean-db.md)、`USER-DEC-05`（见 [discussion-decisions](../context/discussion-decisions.md)）和 [技术架构基线](../architecture/technical-architecture-baseline.md) §7 决策表只确认了后端、数据库、进程、插件与 Agent，**前端技术没有对应条目**。[ARC-001](../plans/active/arc-001-architecture-closure-decision-map.md) 要求收口的「前端框架/路由/状态边界」尚未决定，LOCAL-001（2026-08-24）就开启了本地 Web。

LOCAL-001A 以「最小 Axum host、不引入第二前端框架」交付首个页面。此后各交付包把「page-local vanilla CSS/JS、不引入新框架」当作范围护栏沿用；LIDS 又把来源包的 React 路线列为不继承。[Topic 参考页验收](../design/pages/topic-intelligence-reference-acceptance.md)曾把「运行时前端技术」标为 `DECISION_REQUIRED`，但一直没有形成决定。每个交付包的护栏单独看都合理，累积起来却成了没人拍板的事实标准。

2026-10-15 核对的结果：`apps/api/src/local_web/` 有 15 个原生页面脚本，约 1 万行；非测试 Rust 约 3.2 万行，其中采集与工位页面由 Rust 直接拼接 HTML（`target_drawer.rs`、`station_view.rs`、`collection_targets_view.rs` 三个文件合计一万余行）。最大的页面脚本 `topic_map.js` 单行最长 4339 个字符。2026-09-15 以来，改动该目录的 194 次提交中有 92 次是修复（含部分后端修复），其中多见几类界面状态问题：晚到的响应覆盖新数据、切换领域后丢失范围、浏览器历史状态错误、窄屏布局错乱、各页外壳不一致。

## 决定

1. **技术栈**：所有正式网页由一个 React + TypeScript（`strict`）应用提供，Vite 构建，产物由现有 Rust/Axum 服务同源提供。不使用 Next.js 或其他服务端渲染框架，不新增 Node 服务、BFF、微前端或 iframe。
2. **业务权威不变**：业务判断、资格、授权、幂等与操作回执仍由 Rust/PostgreSQL 负责。前端不重新计算业务结论，不直连数据库；操作只在服务器回执之后显示成功。
3. **范围**：现有 16 个正式页面全部迁移，包括采集、工位、运营台账、设置和登录；新页面一律直接用 React。浏览器插件（已经是独立的 React 应用）与 Pi Adapter 不在范围内。
4. **不留双轨**：主线上同一页面任何时刻只有一套实现。某页的 React 版合入时，同一 PR 删除它的旧实现；不设新旧切换开关，也不保留旧页面作为回退路径（回退以整个版本为单位）。迁移期间旧页面冻结，只修阻断使用的缺陷。终态仓库不再含 Rust 拼接的页面 HTML 与原生页面脚本，并由 CI 检查防止回流。这与 AGENTS.md「禁止 V1/V2 双写」同一原则，只是适用于界面代码。
5. **开发期数据不构成迁移约束**：可以按页面需要调整接口与读模型，不承担新旧行为逐项等价证明，也不承担旧书签兼容。迁移本身不删除数据；清库须另行明确授权。
6. **样式**：只消费 LIDS Token（`--lgi-*` 唯一数值源）；页面样式使用 CSS Modules；不引入 Tailwind 或第二套 Token。
7. **执行计划**：唯一实施计划为 [WEB-REACT-001](../plans/active/web-react-001.md)。它合并了 UIUX-CONTENT-WORKBENCH-001（[Issue #425](https://github.com/Himog0921/linggan-intelligence/issues/425)）的推进安排，内容工作台的前端改在 React 上实现。

## 取代与待裁定

以下条款与本决定冲突，按本决定执行。替代标记由 WEB-REACT-001 阶段 B 首项任务统一补到原文件：

| 原文件 | 被取代的表述 |
|---|---|
| [comment-study-productization-001 架构](../architecture/comment-study-productization-001.md) | 「JS 采用原生 ES modules……不引入前端构建框架或全局状态管理库」 |
| [comment-study-productization-001 计划](../plans/active/comment-study-productization-001.md) | D01 中以「现有 Axum 本地 Web」作为前端形态，以及「暂不做……更换前端框架」 |
| [collection-control-closure-001](../plans/active/collection-control-closure-001.md) 及其 [UI 变更清单](../design/changes/collection-control-closure-001-ui-change-manifest.md) | 「Rust SSR + page-local vanilla CSS/JS」硬约束 |
| [collection-three-package-closure-001](../plans/active/collection-three-package-closure-001.md) | 「Rust server-rendered HTML + page-local vanilla CSS/JS」硬约束 |
| [collection-single-run-398](../plans/active/collection-single-run-398.md) 及其 [UI 核对](../design/changes/collection-single-run-398.md) | 「不引入第二套前端框架」「不新增框架」 |
| [domain-unification-001](../plans/active/domain-unification-001.md) | 「保持当前 local_web SSR + POST/Redirect，不引入 SPA」 |
| [creator-discovery-001](../plans/active/creator-discovery-001.md) | 「不全局换 UI 框架」 |
| [intelligence-home-threejs](../pages/intelligence-home-threejs.md) | 「原生 JS 可以与 Rust 输出的 HTML 共存，不要求全站换 React」 |
| [Topic 参考页验收](../design/pages/topic-intelligence-reference-acceptance.md)、[ARC-001](../plans/active/arc-001-architecture-closure-decision-map.md) | 「运行时前端技术」待决定：现由本决定关闭 |
| UIUX-CONTENT-WORKBENCH-001 计划 §10.6（尚未进入主线） | 「沿现有 Rust/Axum + 原生 JS/CSS 页面……无需新增前端框架」 |

保持有效、不被取代的条款：

- LIDS 来源包的具体代码、目录、模拟数据与 React Three Fiber/Blender 资产路线仍不继承（[LIDS README](../design/lids/README.md)、[参考登记](../design/reference-register.md)、[UI 执行合同](../agents/ui-execution-contract.md)）；本决定只规定前端技术本身。
- 不迁回旧内容工作台的 Next.js/Prisma 技术栈与数据。
- 插件继续独立构建与发布。

由 Mog 在计划相应阶段裁定的事项：仍用旧写法修改页面的在途分支如何处理（见 [Issue #430](https://github.com/Himog0921/linggan-intelligence/issues/430)）；共享外壳在阶段 B 是否直接采用 V3 紧凑外壳。

## 后果与未证明边界

- **代价**：一次性迁移工作量；迁移期间旧页面只修不改；本机运行链新增 Node 构建步骤，需要在 launchd 实际环境验证；仓库增加 TypeScript 工程与依赖锁；页面迁移时，针对旧 DOM 的测试要随旧代码一起改写。
- **预期收益（未证明）**：界面状态类缺陷减少，组件与 LIDS 真正统一，AI 代理的改动受类型检查约束。这些要用迁移后的缺陷数和改动成本实际观察，不能预先宣称。
- 本决定不证明任何页面已经迁移、构建链已经可用或体验已经改善。
