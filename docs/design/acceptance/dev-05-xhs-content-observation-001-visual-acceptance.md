# ACC-DEV-05-XHS-CONTENT-OBSERVATION-001 · 小红书作品复观测、历史事实与当前状态

> 状态: 一次性报告
> 最后核对: 2026-09-01
> 适用范围: Issue #133 `DEV-05-XHS-CONTENT-OBSERVATION-001` 的 `/corpus/evidence` Inspector、作品事实读取、Coverage 历史与受限复观测动作
> 事实来源: Issue #133、`PAGE-EVIDENCE-001`、当前 worktree Rust/API/HTML/CSS/JS、隔离 PostgreSQL proof、自动检查
> 冲突时以谁为准: Issue #133 的授权边界、真实代码/数据库合同/测试与真实运行；本报告不以截图、静态检查或历史 Issue 取代它们

本报告是该 Issue 的唯一一次前端验收记录。它刻意把“源码/隔离 proof 已验证”与“当前运行时、真实小红书、部署和 Mog 验收未验证”分开，不将后一层的缺席折算为通过。

## 1. 验收对象

- Issue / SCOPE: Issue #133；Issue #3 只作历史验收输入，未创建或复用旧 `0002`。
- 页面/组件/状态: `/corpus/evidence` 的 Selected Work Inspector；`detailCurrent`、`engagementCurrent`、`engagementTimeline`、评论/回复 Coverage history 与 `立即复观测`。
- 关联 PAGE / ACC ID: `PAGE-EVIDENCE-001`、`ACC-DEV-05-XHS-CONTENT-OBSERVATION-001`。
- 验收日期与环境: 2026-09-01；隔离 PostgreSQL + 临时 API test fixture。未修改正在运行的 main/runtime、插件或共享数据库。
- 数据/权限前提: 复观测只接受已存在的 target-linked active deep-archive authorization；无关联作品必须被拒绝，不能回退到作者、标题、URL 或监控目标显示名。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 前端/状态检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 已知当前事实 | 查看一篇多次观察的作品 | 每个互动指标取最新 `KNOWN`，同时显示前值与变化 | 当前值、前值、delta 与 Package/观察时点分开 | `material_projection_postgres` 证明最新合格指标和历史来源 | VERIFIED（隔离 proof） |
| 后续详情不完整 | 查看较早已知标题、作者、发布时间 | 新的未知/缺失字段不得抹掉旧的已知字段 | `detailCurrent` 逐字段来源、未知不伪装为清空 | `material_projection_postgres` 的 field-wise fixture | VERIFIED（隔离 proof） |
| 两种评论 Coverage | 查看详情窗口与独立全公开观察 | `30/80` 与 `70/70` 分别属于各自 Package/Receipt，不可相加 | 历史分组、每项 refs/窗口/stop reason；无总和 | `material_social_postgres` | VERIFIED（隔离 proof） |
| 无关联作品点击动作 | 尝试复观测没有 target 授权的 XHS 作品 | 拒绝是授权事实，不是已排队或平台无内容 | inline failure 保留返回 code，按钮不产生乐观成功状态 | API route 测试返回 `409 reobservation_authorization_not_linked` | VERIFIED（自动） |
| 有关联作品请求复观测 | 发起一次标准详情复观测 | 形成真实 Work Order、Lease、三个 server-issued lanes；并非浏览器已读取 | 显示 request/work order/lease、逐 lane Task→Attempt→Package→Receipt；评论最多 30；无媒体 | `content_reobservation_postgres` 实际派发详情并接纳 synthetic Package；任务为 detail/comments/replies | VERIFIED（隔离 proof） |
| 浏览器视觉呈现 | 在实际本机运行页操作 Inspector | 只可在部署了本 worktree 的运行时确认 | 排版、焦点、窄屏、真实按钮交互和轮询 | 本次未获授权更新 runtime/reload，未创建截图或录屏 | NOT VERIFIED |

## 3. 视觉工作条件

- 声明的桌面工作区/视口: 未启动新的页面运行时；因此没有把旧 main 页面或历史截图误写为本变更的视觉证据。
- 输入内容长度与数据密度: 自动 fixture 覆盖多次 detail/metric 观察、局部字段缺失、两个独立 Coverage 窗口、无授权拒绝与三 lane 复观测。
- 已批准的设计规则: `PAGE-EVIDENCE-001` 的 `Corpus Explorer + Split Evidence Inspector`、中文主表达、inline 必读反馈、LIDS 状态诚实性。
- LIDS 强度 / Pattern / Token 依据: L1 `Corpus Explorer` 内的受限 L2 `Split Evidence Inspector`；沿用现有 local-web CSS 与 token，不新建全局 token 或组件库。
- 状态边界: `QUEUED / CLAIMED / RUNNING / ACCEPTED / EXPIRED_WITHOUT_RECEIPT / COMPLETED_WITHOUT_RECEIPT` 仅描述实际链路位置；`ACCEPTED` 不等于全部 Coverage 完整。
- Reduced Motion / 移动或静态 Poster 降级: 源码沿用现有 Inspector 表面与按钮样式；未在新运行时进行浏览器视觉验证。
- 截图或录屏证据位置及生成物登记状态: 未生成；不把未部署源码变更伪装成浏览器截图验收。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | `PAGE-EVIDENCE-001` 同步定义字段级当前事实、历史、复观测授权和操作状态 | 规格不能证明运行时已更新 |
| 前端/组件实现 | VERIFIED（源码） | Inspector 渲染当前/前值/delta、字段来源、Coverage history、真实 POST/GET 操作与非乐观失败态；`node --check` | 未做浏览器 render/keyboard/响应式实测 |
| 自动检查 | VERIFIED | `test-local-001-discovery-postgres.sh`、local-web API/static tests、Cargo format/check 与 governance check | 自动测试不替代真实小红书或浏览器 |
| 真实链路/回执 | VERIFIED（隔离 synthetic） | 真实 Rust admission/lease/dispatch/receipt 代码路径与隔离 PostgreSQL fixture；不使用手工 Task | 不证明生产 Browser Producer 已领取或页面实际读取 |
| 部署 | NOT VERIFIED | 未更新 shared runtime、未 reload localhost:3000、未打新插件包 | 当前 main/runtime/plugin 已是 0.8.28，但不含本 worktree 源码变更 |
| Mog / 业务验收 | NOT VERIFIED | 未进行用户真实页面验收 | 等待明确的 main/runtime 切换与 Mog 操作授权 |

## 5. 发现与后续

- 发现的规格冲突: 无。Issue #3 的“标准评论窗口”只提供历史验收语义；本卡没有字面创建旧 `0002` migration/decision。
- 是否需要 DECISION_REQUIRED: 否。本卡已将动作收紧为已有授权的实际链路；没有扩张成泛化补采或媒体采集。
- 是否需要设计例外或长期决定: 否。动作作为 `PAGE-EVIDENCE-001` Inspector 的受限例外，不晋升通用组件或全局采集入口。
- 不得因此推断的结论: 不得推断运行中的 localhost:3000 已含此代码、Chrome 已 reload、插件已更新、真实 XHS 已复观测、Coverage 完整、部署完成或 Mog 已验收。
