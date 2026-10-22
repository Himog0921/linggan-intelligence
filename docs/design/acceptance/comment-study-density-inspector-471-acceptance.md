# ACC-COMMENT-STUDY-DENSITY-INSPECTOR-471

> 状态: 一次性报告
> 最后核对: 2026-10-22
> 对象: 评论研究目录、检查器与概览观察布局
> 适用范围: `codex/comment-study-density-inspector-471` 的评论研究 UI 候选
> 事实来源: 当前候选源码、下列可重复执行的本地 Chromium 1440×900 核验与前端门禁回执
> 冲突时以谁为准: 用户最新授权、真实代码/运行回执、页面合同与 `AGENTS.md`

| 验收项 | 通过定义 | 证据状态 |
|---|---|---|
| 目录密度 | 普通评论与 `3.5万` 点赞行均为 48px 单行，数字与文本垂直居中，长原声/作品标题截断 | 本地 Chromium 1440×900：前三行 `49px`（含 1px 边线）；点赞内容盒 `48px`，上下偏移 `0.5px` |
| 统一控件 | 搜索为 FIND 输入；选择入口、选择态、完成/清空动作与证据库一致，筛选仍显式应用 | 通过：FIND 黑底前缀、清空控制、四格选择图标、选择态与显式应用筛选回归通过 |
| 检查器边界 | 抽屉从 Context Bar 下方开始，不遮主导航或面包屑；只展示有来源价值的信息 | 本地 Chromium：抽屉 top 与共享页头 bottom 均为 `129px`；原声、观察事实、作品、实际清洗差异、已有研究记录/信号可读 |
| 概览平衡 | 图表与左侧统计视觉等高；`评论`/`信号` 分别对齐对应统计标题 | 本地 Chromium：`评论`/“可研究评论”为 `193.80/193.50px`，`信号`/“当前研究信号”为 `523.80/523.30px`；图表 `537.59px`，左统计 `526.80px` |
| 数据不变 | 不发出写请求，不改接口、点赞/研究/观察口径 | 通过：此变更仅 React/CSS/测试/文档；目录自动回归保持 GET-only |
| 工程门禁 | 回归、typecheck、lint、build、LIDS、治理与独立代码复核通过 | 前端 104 文件/1114 项通过；typecheck、lint、生产 build、LIDS verify、`git diff --check`、治理、设计手册和独立代码复核通过 |

## 可追溯执行回执

执行目录：`/Users/moglenny/proma/linggan-intelligence/.worktrees/comment-study-density-inspector-471`。所有页面检查只读取本地 `127.0.0.1:5174` 与既有本地 API `127.0.0.1:3000`，未触发写操作。

| 时间（2026-10-22，Asia/Shanghai） | 命令 / 方法 | 原始结果摘要 |
|---|---|---|
| 17:53 | `npm --prefix apps/web test` | `Test Files 104 passed (104)`；`Tests 1114 passed (1114)` |
| 本次核对 | `npm --prefix apps/web run typecheck`、`npm --prefix apps/web run lint`、`npm --prefix apps/web run build` | 均以 exit 0 完成；build 仅保留既有运行时字体路径警告 |
| 本次核对 | `npm --prefix apps/web run lids:verify` | `LIDS sync: PASS`、3/3 设计系统测试通过、LIDS 构建完成 |
| 本次核对 | `bash scripts/check-project-governance.sh && bash scripts/verify-ui-design-handbook.sh && git diff --check` | `project governance check passed`；`UI design handbook check passed`；diff 无空白错误 |
| 本次核对 | Headless Chromium（viewport `1440×900`，Playwright）读取评论页、评论检查器、概览页的 `getBoundingClientRect()` | 目录前三行：`49/49/49px`（每行含 1px 边线）；点赞盒：`48px`、上偏移 `0.5px`；`--v7-header-height=129px`、检查器 `top=129px`、`bottom=900px`；概览 `评论/可研究评论=193.796875/193.5px`、`信号/当前研究信号=523.796875/523.296875px`、图表高度 `537.59375px`；无实际差异时检查器不显示冗余“清洗差异” |
| 本次核对 | 独立提交前代码复核 | `comment_density_commit_review`：实现结论 PASS；本回执补齐后 P1 已处置 |

本记录不把 CSS diff、静态截图或 HTTP 200 视为部署或业务验收；未获单独授权前不得合并或安装到运行时。
