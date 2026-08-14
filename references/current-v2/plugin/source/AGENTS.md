# 灵感爆爆爆 — 智能体导航

> Chrome MV3 扩展，当前覆盖小红书与抖音两条采集链路，纯 JS + Dexie。  
> 本文件职责：提供导航、约束和事实源入口，不单独承担产品或架构的最高权威。

本仓库对应的不是一个独立产品，而是统一产品“内容工作台”中的浏览器执行端。

统一产品分工固定如下：

- 内容工作台：主系统，负责判断、组织、沉淀
- 灵感爆爆爆插件：执行端，负责网页内采集、页面交互、结果回传

无论在本仓库还是工作台仓库协作，都应把两者理解成同一个产品的两个运行面，而不是两个彼此独立的产品。

## 本地工作台联调规则（2026-06-25 起）

Mog 当前一周内主要在本机打磨内容工作台和插件。默认联调目标是本地工作台，除非用户明确说“线上 / 生产 / 正式站”：

1. 插件要写入本地数据时，工作台地址必须设为 `http://localhost:3000`。
2. 插件如果指向 `https://lingganboom.fun`，采集结果、任务回传、媒体上传和工位同步都会进入线上正式站；本地打磨时不要这样做。
3. 本地和线上是两套授权关系；切到本地后，需要在本地工作台重新生成授权码、重新绑定执行设备。
4. 本地联调前先确认内容工作台本地服务可打开，并在插件里执行“测试连接”。
5. 插件已内置“本地 3000”快捷入口，优先使用它，不要手写临时地址。

## Source of Truth

### 权威级别

| 级别 | 文件/目录 | 用途 |
|------|-----------|------|
| 权威 | `docs/product/*.md` | 当前产品能力、用户路径、验收标准 |
| 权威 | `docs/technical/*.md` | 当前技术栈、数据模型、消息协议、平台调研事实 |
| 权威 | `docs/plans/active/README.md` | active 目录真实状态标注；物理位置在 active 不等于仍在做 |
| 权威 | `docs/decisions/index.md` | 关键架构决策与转向记录 |
| 权威 | `progress.txt` | 真实时间线与阶段进展 |
| 权威 | `docs/governance/*.md` | 文件落位、命名和文档维护规则 |
| 导航/约束 | `AGENTS.md`、`CLAUDE.md` | 阅读入口、协作铁律、工作方式约束 |
| 导航 | `docs/README.md` | 文档地图:按层分类 + 状态标注(✅权威/⚠️滞后/🗄️归档),进 docs 先看它 |
| 历史资料 | 外层 `01_*.md`、`02_*.md`、`03_*.md` | 立项期研究与方案草稿，仅供背景参考 |

### 冲突时怎么判断

1. 产品问题优先看 `docs/product/*.md`
2. 技术事实优先看 `docs/technical/*.md` 和真实代码
3. 当前执行顺序优先看 `progress.txt`；旧计划是否仍活跃先看 `docs/plans/active/README.md`
4. 关键历史转向优先看 `docs/decisions/index.md`
5. 若外层 `01/02/03` 与内层 `docs/**` 冲突，一律以内层 `docs/**` 为准

## 架构速览

```text
用户操作层（Popup / Dashboard / 页内注入）
  → Chrome 容器（Background Service Worker + Content Script）
    → 平台层（XHS / Douyin 采集器、批量控制、页面检测）
      → 页面上下文桥接（Injected Scripts / 页面侧 fetch / API capture）
        → 本地数据层（Dexie / IndexedDB）
```

当前构建入口仍是 4 个：`content`、`background`、`popup`、`dashboard`。

## 文档导航

| 你要做什么 | 去哪里看 |
|-----------|---------|
| 理解当前产品能力 | [docs/product/PRD.md](docs/product/PRD.md) |
| 理解当前用户操作流 | [docs/product/APP_FLOW.md](docs/product/APP_FLOW.md) |
| 跑功能验收 | [docs/product/TEST_CHECKLIST.md](docs/product/TEST_CHECKLIST.md) |
| 理解模块职责和跨层关系 | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| 查/改 DOM 选择器 | [docs/SELECTORS.md](docs/SELECTORS.md) |
| 看抖音页面调研结论 | [docs/technical/DOUYIN_FIELD_SURVEY.md](docs/technical/DOUYIN_FIELD_SURVEY.md) |
| 看小红书页面调研结论 | [docs/technical/XHS_FIELD_SURVEY.md](docs/technical/XHS_FIELD_SURVEY.md) |
| 查数据库 schema | [docs/technical/DATA_MODEL.md](docs/technical/DATA_MODEL.md) |
| 查消息协议 | [docs/technical/MESSAGE_PROTOCOL.md](docs/technical/MESSAGE_PROTOCOL.md) |
| 查 AI-ready 数据契约 | [docs/technical/AI_READY_DATA_CONTRACT_V1.md](docs/technical/AI_READY_DATA_CONTRACT_V1.md) |
| 看技术决策历史 | [docs/decisions/index.md](docs/decisions/index.md) |
| 看当前时间线 | [progress.txt](progress.txt) |
| 看 active 计划真实状态 | [docs/plans/active/README.md](docs/plans/active/README.md) |
| 看文件落位规则 | [docs/governance/file-placement-standard.md](docs/governance/file-placement-standard.md) |
| 看文档维护规则 | [docs/governance/document-maintenance-protocol.md](docs/governance/document-maintenance-protocol.md) |
| 看技术债务 | [docs/plans/tech-debt.md](docs/plans/tech-debt.md) |

## 源码地图

| 区域 | 路径 | 当前职责 |
|------|------|---------|
| Chrome 容器 | `src/background/index.js` | 下载、tab 通信、后台路由 |
| Content 主入口 | `src/content/index.js` | 平台识别、消息总路由、批量任务与 Dashboard 桥 |
| 小红书平台 | `src/platforms/xhs/*` | 笔记/评论/博主/批量/UI 注入/反检测 |
| 抖音平台 | `src/platforms/douyin/*` | 视频/评论/博主/批量/UI 注入/页面检测 |
| 页面桥接 | `src/injected/*` | `__INITIAL_STATE__`、抖音 API capture、页面上下文桥接 |
| 数据层 | `src/db/*` | Dexie schema、内容/评论/作者/任务/媒体资产存储 |
| Popup | `src/popup/*` | 全局入口、任务触发、批量设置 |
| Dashboard | `src/dashboard/*` | 数据管理、导出、二次下载入口 |
| 共享模块 | `src/shared/*` | 常量、消息封装、通用工具 |

## 协作铁律

1. **先调研再实现**：禁止猜选择器、猜接口、猜页面状态。
2. **不转嫁技术债给用户**：排查、探针、证据收集优先由 AI 承担。
3. **先修事实源，再修结构，再修体验**：执行顺序服从活跃计划。
4. **文档与代码一起维护**：改选择器、协议、数据结构时必须同步更新对应权威文档。
5. **历史资料不能倒灌实现**：外层 `01/02/03` 只作背景参考，不能直接拿来当现行需求或架构。

## 文件治理

1. 新建文件前先看 `docs/governance/file-placement-standard.md`，确认应该进入 `docs/`、`src/`、`tests/`、`scripts/` 还是 `releases/`。
2. 文档维护规则见 `docs/governance/document-maintenance-protocol.md`。代码、协议、字段、页面事实、发布版本变化后，不要只改代码。
3. 根目录只保留 `AGENTS.md`、`CLAUDE.md` 这类入口 Markdown；评审、计划、调研、交接和发布说明进入 `docs/` 对应目录。
4. 禁止用 `*_v2`、`*_final`、`*_old`、`*_new`、`*_copy`、`*_backup`、`*_draft2` 等文件名表达版本。
5. 收尾时运行 `node scripts/check-project-governance.mjs`，确认入口、文档地图和版本口径没有漂移。

## 浏览器执行规则

1. **真实浏览器验收优先使用 Chrome MCP**：若当前会话已挂载可用的 Chrome MCP，并且需要复用用户现成登录态、真实窗口或现有浏览器环境，则优先使用 Chrome MCP 完成验收闭环。
2. **gstack /browse 作为退路，不再是唯一通道**：当 Chrome MCP 不可用，或仅需轻量页面浏览、截图、结构快照时，才退回 gstack `/browse`。
3. **验收结论必须说明浏览环境**：在回复中明确写清“本轮验收基于 Chrome MCP 真实浏览器”还是“基于 gstack 浏览器”，避免把不同运行环境下的结论混为一谈。

## 文档同步硬门禁

> 这是一条项目级硬规则：任何“代码改动 + 测试结论 + 用户反馈”形成闭环后，必须主动完成文档同步，不等待用户提醒。

### 最低门槛

1. **任何一轮实现后，至少更新 `progress.txt`。**
2. **未更新文档的代码回合，不算真正闭环。**

### 触发条件与必须更新的文件

| 触发条件 | 必须主动更新 |
|---------|-------------|
| 任意代码改动已完成并做过测试 | `progress.txt` |
| 用户可见行为、按钮、流程、提示、任务语义发生变化 | `docs/product/PRD.md`、`docs/product/APP_FLOW.md`、`docs/product/TEST_CHECKLIST.md` |
| 字段、数据结构、消息协议、采集深度、任务状态字段发生变化 | `docs/technical/DATA_MODEL.md`、`docs/technical/MESSAGE_PROTOCOL.md`、`docs/technical/AI_READY_DATA_CONTRACT_V1.md` |
| 页面事实、选择器、平台页面类型判断、页面调研结论发生变化 | `docs/technical/SELECTORS.md`、`docs/technical/DOUYIN_FIELD_SURVEY.md` |
| 出现新的稳定策略、架构转向、治理规则、停止/暂停/继续语义结论 | `docs/decisions/index.md`，必要时补 `docs/plans/active/*.md` |

### 关闭一轮工作的标准

一轮工作只有同时满足下面条件才算“已完成”：

1. 代码已修改
2. 构建/自测已完成
3. 用户验收结论已获得，或至少已拿到明确测试反馈
4. 对应权威文档已同步
5. 最终回复中已明确说明“本轮更新了哪些文件”

如果第 4、5 点没做到，这一轮只能算“部分完成”，不能按“已收口”对外表述。

## 构建

```bash
npm run build          # 生产构建 → dist/
npm run dev            # 开发模式（watch）
npm run release:patch  # 语义版本自动化
```
