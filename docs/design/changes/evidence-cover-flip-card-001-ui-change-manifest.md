# EVIDENCE-COVER-FLIP-CARD-001 · 证据库封面作品卡

> 状态: 权威当前
> 交付状态: 已合并 `main@521002f`，本机 `:3000` 已刷新至同一 revision
> 最后核对: 2026-09-15
> 适用范围: `/corpus/evidence?layout=cover` 中既有作品卡的主视觉与其局部翻转交互
> 事实来源: Mog 2026-09-15 的明确交付、`PAGE-EVIDENCE-001`、`EVIDENCE-V9-001`、LIDS、现行 `evidence_library.js/css`
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/API 合同；本清单不扩大 Material Projection 或媒体资格语义

## 1. 事项与边界

- Work Package：`EVIDENCE-COVER-FLIP-CARD-001`；专属分支 `codex/corpus-cover-flip-cards`，专属 worktree `.worktrees/corpus-cover-flip-cards`。
- 目标：只将封面排版的现有作品卡重组成「可翻转主视觉」和「固定元数据」两块；正面是标题与有限 SVG 几何模板，背面是同一 3:4 Stage 内的受控本地真实封面。
- 明确非目标：页面信息架构、查询/筛选/排序/分页、Inspector、详情选择、Work Resource API、数据模型、媒体 URL 资格、coverage/completeness、观察逻辑、采集、插件、数据库、全局 Token、Shell、运行服务与部署。

## 2. 读取回执与方向锁

| 来源 | 解决的问题 | 结论 |
|---|---|---|
| `PAGE-EVIDENCE-001` §4–§5 | 作品为列表主对象；三种 layout 共用同一读取集；小红书预览为 3:4 | 保留同一 `rowFor`、同一选择/Inspector 链路，不增加卡片体系 |
| `EVIDENCE-V9-001`、当前 JS/CSS | 现有 cover 只复用 `.ev-preview`，行 click 进入当前详情 | 仅在 `layout=cover` 替换 DOM 表达；研究/表格不变 |
| LIDS Token / Data Boundaries / Agent Guide | Token-only、真实数据边界、焦点、reduced motion、未知不造 0 | 复用 `--lgi-*`，封面只使用受控同源句柄，标题两行截断 |
| 用户提供的 V5 原型 | 主视觉与元数据分离、同一 Stage、档案卡翻面节奏 | 取交互和局部结构，不把原型的页面/模拟数据写入运行页 |

- Visual thesis：米白主视觉像一张可翻阅的技术目录档案；细墨线、极淡点阵、SVG 线稿和 Signal 橙红只承担当前选择，不使用渐变、玻璃或大阴影。
- Content plan：主视觉内固定标题区 → 共享 3:4 Stage → 类型/翻面提示；下方固定作者/日期 → 互动数 → materials 状态与最近观察。
- Interaction thesis：有 hover 的设备只翻主视觉；无 hover 的设备点主视觉翻/复原并阻止该次详情选择；其余卡面点击仍沿现有行选择链路打开 Inspector；键盘 Enter/Space 可翻面。
- 分类：展示 + 局部交互；不改变状态、权限或任何写动作。

## 3. 表面、状态与依赖

| 表面 | 责任 | 复用事实 | 禁止替代 |
|---|---|---|---|
| 主视觉正面 | 标题、作品类型、抽象几何 | `title`、本地已知媒体形态 | AI 生成图、远程封面、状态臆测 |
| 主视觉背面 | 同尺寸 Stage 中展示原始封面 | 已有 `cover.localAssetUrl` + same-origin gate | 远程 CDN / 不安全或不可内联字节 |
| 固定元数据 | 作者、发布时间、互动、lane 状态和最近观察 | 既有 author/published/engagement/material summary | 总完整度、伪造 0、第二状态模型 |
| 作品行 | 选择和 Inspector 深链 | 既有 `selectItem` / `selectCrossIndustrySample` | 新详情路由或平行点击链路 |

## 4. 验收矩阵与停止条件

| 验收层 | 方法 | 通过条件 |
|---|---|---|
| DOM/交互 | 聚焦 Rust 文本合同与浏览器 | 前后同一 Stage；仅 visual 翻；行选择仍可达；触屏 toggle 不冒泡 |
| 数据边界 | 代码走读与聚焦测试 | 继续只用 controlled local handle；未知、部分、受限、清理不改写 |
| 视觉/a11y | 1280/1440/1920 与 390px、focus/reduced-motion | 两行标题不推移 Stage；无横向滚动；无 motion 时立即切换 |
| 真实后果 | diff/网络核对 | 无 API/数据库/平台/部署写入 |

停止条件：实现需要新字段、远程媒体、修改 API/媒体资格/状态含义、改变详情点击契约或全局视觉 Token 时，停止并交由 Mog 决定。
