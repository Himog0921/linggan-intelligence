# ACC-OBSERVATION-TARGET-DOSSIER-UI-001 · 观察目标档案桌面验收

> 状态: 一次性报告
> 最后核对: 2026-09-05
> 适用范围: Issue #158；`codex/observation-target-dossier-ui-001`，exact base `origin/main@bfe5d7e623b70e314cc6697269e291c3d5f5b3ef`
> 事实来源: 当前分支代码与自动测试、一次性 PostgreSQL 16、隔离 `:3318` API、Chrome 1440 CSS px 实际渲染
> 冲突时以谁为准: 用户最新确认、Issue #158 Claim、真实代码/数据库/浏览器结果、PAGE-COLLECTION-001 与 LIDS；本报告不授权 merge、共享运行或真实平台行动

## 1. 结论

当前分支已经把观察目标从工程监控表收敛为创作者作品档案入口：首层先回答观察谁、档案范围、详情缺口、巡查状态、上次与下次时间及唯一下一步；creator 工作区把最近变化放在生命周期散点之前，再显示档案缺口。keyword 使用独立表头和两职责工作区，不继承 creator 档案、详情进度或散点语义。

此结论只覆盖源码、隔离数据库与 1440 CSS px 桌面浏览器。它不是共享 `:3000`、真实目标数据、真实平台采集、插件执行、部署、merge 或 Mog 业务验收。

## 2. 隔离环境

| 层级 | 本次事实 |
|---|---|
| PostgreSQL | 临时 `postgres:16.14-bookworm` 容器、随机监听端口、一次性 database/schema；只写 synthetic fixture |
| API | 当前 worktree 编译产物，loopback `127.0.0.1:3318`；共享 `:3000` 未停止、替换或刷新 |
| Browser | Chrome headless，外部窗口 `1440×900`，页面 CSS viewport `1440×813`、DPR 1 |
| Fixture | 1 个 creator、2 轮历史巡查、3 个 lifecycle Work；另有 1 个 keyword 用于 kind-aware UI 验证 |
| 清理 | 隔离 tab、API、Chrome CDP profile、database、container 和 volume 均已删除或停止 |

fixture 明确是 `SYNTHETIC / NOT LIVE`。它只证明当前代码能按照冻结合同读取和呈现这些状态，不证明真实小红书主页已经扫描、详情已经采集或巡查已经发生。

## 3. 1440 桌面表格

### Creator

- 表头按固定顺序完整呈现：`编号｜创作者｜平台｜分组｜档案状态｜作品目录｜详情进度｜巡查状态｜最近变化｜上次巡查｜下次巡查｜操作`。
- `documentOverflow=0`，`tableOverflow=0`；普通数据格 `wrappedCells=0`。
- 创作者头像、昵称与可识别账号位于同一横行；长账号单行省略，完整值保留在 `title`，昵称在本次样本中未被截断。
- 上次巡查与下次巡查是两个独立列，分别显示 `09-04 08:30` 与 `09-05 08:00`，没有拿派出时间冒充成功结果。
- 样本行显示 `作品目录 2 篇`、`详情进度 0 / 2 · 缺 2`；没有百分比或总健康分。
- 行内主动作计数为 1，当前真实状态给出“继续完善”；没有并列的常驻“监控规则”按钮。
- 当前没有目标级巡查差分投影，因此最近变化显示“尚未取得”，没有用 `0` 冒充确认无变化。

### Keyword

- 表头为：`编号｜关键词｜平台｜分组｜巡查状态｜最近命中｜数据更新｜上次巡查｜下次巡查｜操作`。
- `documentOverflow=0`，`tableOverflow=0`。
- 表格内没有 creator 的“档案状态 / 作品目录 / 详情进度”。
- keyword 抽屉只有“概览｜巡查”，没有 lifecycle chart 或空的 creator 档案 Tab。

## 4. Creator 档案工作区

- 抽屉宽 `1036.8 / 1440 = 72%`；抽屉和文档横向溢出均为 0。
- 固定 Tab 只有“概览｜档案｜巡查”。
- 概览顺序为“最近变化 → 作品生命周期 → 档案缺口”；没有工程回执、Task、Package、Receipt、Lease、机会评分或监控价值模块。
- 生命周期默认“全部周期 + 点赞”，可切换近 90 天及评论、收藏、转发。
- synthetic 三点投影实际渲染：2 个 `DirectoryLinked` 空心点、1 个 `AuthorConfirmed` 实心点、1 个最近有效巡查新增外圈；一个 stable Work 只对应一个点。
- 图宽 `987.8px`，点可聚焦并带作品标题、发布日期、指标原值和关联状态的 ARIA；Corpus 深链使用 stable Work public ref。
- 档案 Tab 明确写出“前 200 篇作品链接”为本次主页目录扫描上限，并说明 200 不是平台总作品数；页面没有百分比或总分。
- 浏览器采集到的应用级 warning、error 与 exception 事件为 0。Chrome headless 自身的 macOS display-link/updater stderr 不属于页面 console，未记为应用通过证据。

## 5. 自动与 PostgreSQL 证明

- `cargo check --workspace --all-targets --locked` 通过。
- `cargo test --workspace --all-targets --locked` 通过；需要数据库的测试按合同保持 ignored，另由隔离脚本执行。
- `node --check apps/api/src/local_web/collection_workspace.js`、UI handbook、`git diff --check` 通过。
- 完整隔离 PostgreSQL 脚本全绿：9 discovery、6 producer、3 reobservation、11 Material Projection、7 social、8 media、5 creator lifecycle、4 Observation Target dossier、6 Topic 与 19 API，共 78 项 Rust/PostgreSQL/API proof；另有 2 项 Node controller proof。
- dossier 专项 4 项证明：授权上限 200 与根 marker、199 授权原子拒绝且零半成品、stable Work 去重与 exact target chain、空心/实心/最新巡查外圈。脚本终态确认一次性 database、container 与 volume 已删除。

## 6. 未证明与不支持

- `NOT VERIFIED / NOT AUTHORIZED`：merge、`origin/main`、共享数据库或 migration、共享 `:3000`、worker 切换、插件 reload、真实平台、真实账号/Cookie、真实 200 篇目录扫描、部署。
- `NOT VERIFIED`：Mog 对真实数据和日常使用的业务验收。
- 当前没有 target-level “新增 N / 更新 N / 补齐 N”差分读模型；UI 诚实显示“尚未取得”。
- 目录作品没有合格精确发布时间或所选指标为 UNKNOWN 时不会伪造散点；这意味着“目录里有作品”不保证“每篇都立即能画”。
- 手机、小于 13 寸设备、1280 和 390 CSS px 已由 Mog 明确移出项目支持范围。本报告只验收 1440 CSS px 桌面，不再把窄屏问题带回修复循环。
