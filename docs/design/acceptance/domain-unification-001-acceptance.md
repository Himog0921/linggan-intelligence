# ACC-DOMAIN-UNIFICATION-001 · 领域管理页隔离浏览器验收

> 状态: 一次性报告
> 最后核对: 2026-09-24
> 验收日期: 2026-09-24
> 适用范围: WP4 Domain Management；候选 worktree `codex/domain-unification-001`
> 验收环境: Codex In-App Browser、临时 loopback API `127.0.0.1:65435`、一次性 PostgreSQL schema `domain_browser_acceptance`
> 数据边界: 合成验收 Domain 与关键词 Target；没有连接共享数据库、外部平台或常驻 runtime
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
