# ACC-DOMAIN-UNIFICATION-001 · 领域管理页隔离浏览器验收

> 状态: 一次性报告
> 最后核对: 2026-09-27
> 验收日期: 2026-09-24
> 适用范围: §1–5 为 WP4 Domain Management 隔离验收；§6 为 PR #343 发布后回执；§7 为 PR #345 候选验收；§8 为 PR #345 发布回执与跟进反例
> 验收环境: §1–5 使用 Codex In-App Browser、临时 loopback API `127.0.0.1:65435` 与一次性 PostgreSQL schema `domain_browser_acceptance`；§6–8 的环境分别见各节
> 数据边界: §1–5 使用合成 Domain/Target 且未写共享库或切换常驻 runtime；§6 和 §8 如实记录已发生的共享库/运行变更；§7 使用共享开发库只读流式副本，未写共享库或切换常驻 runtime
> 事实来源: 本机浏览器 AX/截图观察、一次性 API POST/redirect 读回、隔离 PostgreSQL 查询与当前页面合同
> 冲突时以谁为准: 用户最新确认、页面/数据合同、真实浏览器与隔离 PostgreSQL 结果；本记录不授权发布

## 1. 验收对象

- 页面：`/collection/domains` 与 `/collection/targets` 的 Domain 选择/关系流程。
- 关联合同：`PAGE-DOMAIN-MANAGEMENT-001`、`DOMAIN-UNIFICATION-001`、`DEC-0008`、LIDS Collection Control。
- 工作区：桌面视口 `1440×1000 CSS px`，窄屏视口 `390×844 CSS px`。
- 临时测试数据：两个 Domain（领域验收 A/B）、一个关键词 Target（`domain-unification-browser-proof`）。
- 浏览器截图仅在临时验收会话中检查；没有将未登记截图作为仓库资产提交。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 创建、编辑、暂停与恢复 | 建立 Domain 并保存配置，暂停后恢复 | 保存后的状态来自 SSR 重新读取；暂停保留既有关系 | 1440 下配置行、状态标识、表单顺序 | POST/redirect 后说明与研究目标读回；B 暂停时新增关系按钮禁用，恢复后可用 | VERIFIED |
| 同一 Target 多领域用途 | 在 A 创建一个关键词 Target，再在 B 以参照用途关联 | Target 身份共享，关系与 role 按 Domain 独立 | 两个 Domain 分别显示同一个关键词，primary/reference 计数匹配 | PostgreSQL 查询得到 1 Target、2 条关系：A=`primary`、B=`reference`；0 Request、0 WorkOrder | VERIFIED |
| 空关联与空 lane | 查看未关联目标的现有领域 | 当前没有关联目标，八个 lane 均为“尚未观察”，不代表平台没有内容 | 区分关系空态与材料状态 | 页面读取真实隔离 schema 的空投影 | VERIFIED |
| 重名冲突 | 尝试创建已存在的 `ADHD` | 请求失败，不可给成功提示，字段上下文保留 | 可见错误与重试表单 | 页面显示“已有同名领域；本次操作没有写入”；此前数据库读数为两个验收 Domain，冲突后没有再查数据库 | VERIFIED（页面回执）；数据库写入后计数未复查 |
| 响应式与焦点 | 观察桌面、手机宽度并用 Tab 到主按钮 | 关键表单在窄屏仍可操作 | 1440×1000 页面内容可见；390×844 表单纵向排列；主按钮有可见焦点环 | 无数据写入 | VERIFIED；精确溢出几何未测量 |
| 数据库不可读/材料部分或失败 | 查看存储读取失败及非空媒体 lane | UNKNOWN、PARTIAL、FAILED、REMOTE_ONLY 与 LOCALIZED 分开显示 | 逐状态浏览器呈现与错误分层 | 当前空 fixture 未提供这些运行事实 | NOT VERIFIED |

## 3. 视觉工作条件

- LIDS 层级 / Pattern：L1 Operations / Collection Control；沿用既有表单、列表和卡片 token。
- 1440×1000：创建表单与领域卡片在视口宽度内排布；创建区与首卡只保留一条分隔线；修复前多出的重复分隔线已消除。
- 390×844：表单列纵向排列；卡片字段纵向排列；目标关系表在卡片内滚动。截图未见页面级横向裁切，但未通过 `document.documentElement.scrollWidth` 做几何断言。
- 键盘：从研究目标输入框按 Tab 到“创建领域”，屏幕上可见焦点轮廓。
- Context Bar：领域页不再注入超长规则、调度健康和时区组合读数。全局共享壳层尚未提供符合 LIDS-SHELL-001 的完整时间锚点/区位证明；本记录不宣称全站 shell 已符合 v7。
- 截图：本机临时浏览器观察，无已登记截图文件。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | PARTIAL | 桌面/窄屏截图与页面状态文案；重复分隔线和 Domain Context Bar 过载已修正 | 精确 viewport overflow 未量；共享 shell 的完整 LIDS 区位未完成 |
| 前端/组件实现 | VERIFIED | 领域页与目标页 SSR/POST/redirect 页面在浏览器内可操作 | 只验收 Domain 管理切片，不代表 Evidence/Comment Study 浏览器验收 |
| 自动检查 | VERIFIED | 该回执对应的最新检查：`cargo check --locked --workspace --tests`、`git diff --check`、项目治理与 UI handbook 检查（见计划/进度） | 完整 LOCAL-001 数据库套件通过于本次 CSS/shell 收敛之前；本轮变化由 workspace check 与浏览器证明覆盖 |
| 真实链路/回执 | VERIFIED（仅隔离环境） | A/B Domain 与 Target 关系写入临时 PostgreSQL 并读回；冲突 POST 返回明确未写入回执；查询确认 0 Request/WorkOrder | 冲突后未再次查询 Domain 计数；未证明真实业务数据、媒体、平台采集、共享库或运行中 worker |
| 部署 | NOT VERIFIED | 没有部署步骤 | `origin/main`、共享 PostgreSQL、runtime-main 与 `:3000` 未修改 |
| Mog / 业务验收 | NOT VERIFIED | 尚未进行 | 等待 Mog 在实际业务数据与运行环境中验收 |

## 5. 发现与后续

- 本次发现并修正两个真实视觉问题：系统边界胶囊英文注释造成窄宽挤压；领域页 Context Bar 信息超过 LIDS 容量且创建表单与首卡重复画线。
- 没有新增产品语义、Domain 状态或决策；没有新增 DECISION_REQUIRED。
- 不能从隔离浏览器和 0 WorkOrder 推断已部署、已开始采集、真实材料能力完整或 Mog 已接受。
- 临时 PostgreSQL、API 进程、容器与卷已清理；`docker ps`、`docker volume ls` 与进程查询均未发现该临时资源。

## 6. 后续发布补充（2026-09-24；不改写上方隔离验收历史）

本报告上方的“未部署”是隔离验收当时的事实。随后经 Mog 另行授权，PR #343 已合并为 `main@46e10837`，共享开发库应用 0103/0104，`runtime-main` 与 API、巡检 worker、媒体 worker 已切换。部署后本机浏览器确认 `/collection/domains` 和 `/collection/tasks` 可读，任务列表与详情同显冻结 Domain/role；390、1085、1440 CSS px 下实测 `document.documentElement.scrollWidth === innerWidth`。本机 API/运行健康与迁移计数详见 [2026-09 月度进度](../../progress/2026-09.md) 的“PR #343 合并与本机发布回执”。这补充本机部署后的 smoke，不补写当时未做的隔离验收，也不代表真实非空媒体、跨 Domain 评论研究、外部平台采集或 Mog 业务验收。

## 7. 全部领域作品直达验收补充（2026-09-26；PR #345 候选）

本节只覆盖 `DOMAIN-UNIFICATION-001` 的 Collection 作品行 → 语料领域弹窗 → Evidence Inspector，不改写 WP4 验收。条件为候选 API `:3107/:3108`、共享开发库的只读流式副本与 Chrome；副本中仅对“全都给姐上岸 × 考研自习”的 50 个合格原包执行有界补投影。未连接 `:3000` 运行态，也未写共享开发库。桌面浏览器实点了正确领域路径；错误领域及不可读分支只做源码与 API 回执审查，未做独立浏览器验收。没有留存登记截图，故不提交截图资产。

| 场景 | 用户任务 | 状态含义与视觉检查 | 真实后果/回执 | 结果 |
|---|---|---|---|---|
| 全部领域目录 | 从 Collection 选择“全部领域”并打开该观察目标的“作品” | 目标抽屉显示 24 条真实作品，标题及逐行评论数属于材料目录，不把 0 或旧表当结果 | 候选 API 读取副本中 24 条 Domain usage、24 条详情与 245 条评论 | VERIFIED（隔离副本） |
| 未选领域的作品深链 | 点击一篇作品“查看”，选择“考研自习” | 先显示既有领域弹窗；选择后保留同一个稳定 `work`，Inspector 显示同篇标题与正文 | 浏览器实点正确领域路径，该篇评论可读；未触发采集或研究运行 | VERIFIED（隔离副本） |
| 显式领域作品深链 | 从已选“考研自习”的作品行进入 | 链接同时携带 `domain` 与 `work`，不再使用旧 `selected` 参数 | 链接单测通过；此入口未单独做浏览器点击 | PARTIAL |
| 误选领域或材料不可读 | 深链详情返回 `material_not_found` | 中文提示“当前领域无法读取这篇作品”；不能仅凭错误码断言未收录，也不能冒充详情已读 | API 有多个返回同一码的原因；源码反馈分支已审，独立浏览器检查未执行 | NOT VERIFIED（视觉） |
| 窄屏、键盘与焦点 | 用窄屏或键盘完成本次作品直达 | 弹窗、Inspector 的本次链路不得遮挡或丢失焦点 | 先前语料弹窗在 375/390 CSS px 验过；本次作品直达未重测窄屏与键盘 | NOT VERIFIED（本次链路） |

本切片沿用 LIDS Collection Control 与 Evidence Library 现有壳层、弹窗和反馈区，没有新增 Token、Primitive、CMP、Pattern 或动效。自动检查为完整隔离 PostgreSQL `test-local-001-discovery-postgres.sh`、API 单测 271 通过/0 失败/29 忽略、workspace check、JS 语法、UI handbook、治理与 diff 检查；它们不替代真实浏览器或生产数据验收。当前结论：前端/组件实现及正确领域的隔离真实链路 VERIFIED；错误反馈的浏览器表现、共享库有界补投影、`main` 合并、`:3000` 部署与 Mog 业务验收均 NOT VERIFIED。部署后的检查若执行，应追加新回执，不得把本节的候选证明改写成已上线事实。

## 8. PR #345 发布后反例与跟进候选（2026-09-27）

PR #345 合并与 `:3000` 切换后，Chrome 实点 Collection `domain=all` → “全都给姐上岸” → 作品（24 条）→ 首篇“查看” → 领域弹窗“考研自习” → 同篇 Inspector，标题、正文与该篇留存评论可读；评论研究显示观察 245 条、可研究 243 条，面包屑领域菜单没有被首行遮挡。共享库补投影 50/50，第二次预览 50/50 已投影且 0 待处理；这些是本机发布后的回执，不是 §7 候选副本的延伸推断。

另以同一 `work` 深链误选 ADHD 时，Chrome 看到 `domain_required` 而非预期的 `material_not_found`：该 Work 不在 ADHD 首页，合成详情 URL 遗漏 `domain`。跟进候选只为这个列表外直达路径补上已选领域查询参数，并增加回归断言。误选领域的正确中文反馈与本次窄屏/键盘路径在该候选合并部署前仍 NOT VERIFIED；不把已发现反例写成通过，也不触发平台采集或研究运行。
