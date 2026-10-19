# ACC-WEB-REACT-001-E · 运营台账五页迁移证明

> 状态: 一次性报告
> 最后核对: 2026-10-18
> 适用范围: WEB-REACT-001 阶段 E / Issue #438 的候选代码与验收证据
> 事实来源: main/runtime 712286c3 实际界面、独立工作树源码、当前自动检查与隔离 PostgreSQL 浏览器证明
> 冲突时以谁为准: 用户确认、真实回执与可复现合同；候选证明不等于部署或 Mog 签收

## 全站收尾当前入口（2026-10-19）

正式main/runtime/3000已为362dad3f，包含本阶段及D3 #445；下文acdfbb87和旧候选按各自日期保留。Mog已派定完整收尾，新增修正、复验与发布只由[收尾矩阵](web-react-001-closeout.md)及最终PR回执证明，不从旧截图/测试扩大结论。

## 2026-10-18 当前状态补记

E 已随 [PR #439](https://github.com/Himog0921/linggan-intelligence/pull/439) 合入 `82d48a72`；运营概览后续改版 #444 已合入 `07d6c6ca`，两者已包含在正式 3000 的 `acdfbb87` 中。下文截图与测试属于原 E 迁移候选；不能用它们冒充 #444 的视觉证明。Mog 业务验收独立保留。

当前 `origin/main` 与正式本机 3000 均为 `acdfbb876bf3fcb5cb4aaf20ceea565562f0b428`（PR #441）。本次同步依据 GitHub 合并记录、运行进程及 `/health.webApp`，不把历史候选证明改写成新版本重跑结果。

## 对象与条件

执行者 Codex root，task-id WEB-REACT-001-E-OPS-20261016，工作树 `.worktrees/web-react-001-e-ops`，对应 [工作单与能力盘点 #438](https://github.com/Himog0921/linggan-intelligence/issues/438)。范围为总览、发布记录、账号管理、每日数据、导入导出；沿用 PAGE-OPS-001/002 与 OPS-HTTP-001。L1 连续表格/总览工作面，唯一 lids_tokens.css + shell.css；局部 CSS Modules。迁移只保留当前获确认的 UI，不宣称完成全部历史 LIDS 欠账。

运行基线是 3000 的 712286c3。只读视觉候选为 3111：本包生产前端 dist + 限 GET/HEAD 的本机桥接服务，读请求转发同一 3000 接口；**此服务只证明同数据 UI 保真，不是 Axum/写入链路证明**。后者由下面一次性 PG + 正式 Axum Router 验证，二者分开记录。无共享库写入、迁移、导入、平台/业务模型或 worker 调用。

## 场景矩阵与实际结果

| 类别 | 核对结果 | 证据 |
|---|---|---|
| 五页常用能力 | 发布筛选/搜索/日期/URL/分页、重复说明、停用账号、封面；账号零与未知；daily scope；总览四时段/三趋势粒度/产出指标；六下载均通过 | real-chain-reviewed/ops/report.json |
| 目标回执 | accepted 100、unchanged 100、并发客户端 250、stale 200、确认 200、提交 300 后丢答复、关闭重开同 payload/requestId 重试、真实 409 展示 | 浏览器步骤 + Rust DB 断言 |
| 真实数据库后果 | 恰好 4 条 revision、6 条 request receipt（accepted 4 / unchanged 1 / stale 1）；掉答复重试未追加；采集工单/模型调用/导入批次不变 | ops_browser_tests.rs |
| 导出 | 三类 CSV/JSON 六个浏览器下载逐字节等于浏览器启动前冻结的 Rust 核心导出；CSV 北京时点/JSON asOf 保留 | expected-* 与 ops-* 文件 |
| 状态负例 | 503 与异常 JSON 显示失败、无伪造空表；失败可重试；旧抖音请求晚到不覆盖新小红书筛选；非法业务纠正 all；无账号登记只在 all 中，不误归某业务 | 浏览器证明 + 单元测试 |
| 交互与视口 | 1440/1280/1024/390，文档无横向溢出（表格自身可滚动）；Esc、焦点返回、模态 Tab、reduced motion 通过 | 浏览器证明 |
| 原有业务合同 | intelligence 单元 25、迁移摘要 1；隔离 PostgreSQL 导入 6 / 台账读取 7 / 总览目标 12 全通过 | ops-ledger-regression.log |
| API 及外壳 | API 单元 170 passed / 68 ignored；旧页面输出和旧 assets 退役、未知路径 404、/ops 默认 creation 保留 | api-unit-final.log |
| 前端 | TypeScript、lint、40 files / 410 tests、生产构建通过 | frontend-verify-reviewed.log |

## 实际页面截图逐项核对

- 自动路径：`apps/web/test/e2e/ops-visual.mjs`，只允许同源 GET/HEAD；原图及比较结果全部在本工作树 `artifacts/private/web-react-001-e/`，已登记生成物，不入 Git/CI。
- 最终基线 `baseline-reviewed-final/`，最终候选 `candidate-delivery/`，像素记录 `comparison/pixels-delivery.json`。
- **256 / 256 组图像像素完全一致**（RGB、相同尺寸，不裁切、不遮罩、不模糊、不使用容差）。覆盖四种视口、五页首屏、所有表格纵滚区段与最右列、总览下半页、业务选择器展开、周/月目标、自定义日期、发布日期与封面大图。原图保留，不修改截图充当现场。
- 走查修正：共享外壳旧 renderer 把胶囊本地化为「本机服务状态」，React 初版传入不同文字，导致右侧宽度漂移；候选改为相同文字。目标周期全角括号使用完整字符串，避免文本节点分段的字符间距差异。
- 一轮弹窗截图曾抢在历史读取结束前拍到「读取中」；截图脚本已显式等目标表单读取完成，再比较完成态，保留早期原图与日志。
- 以上只证明实际可读数据及声明展开状态的呈现。真实共享库目标保存、其他尚未出现的状态组合和 Mog 主观业务体验不由截图证明。

## 删除与保留

删除 12 个旧资产/DOM 测试：ops.html、ops_publications.html、ops_accounts.html、ops_daily.html、ops_io.html、ops_overview.html、ops.css、ops_overview.css、ops.js、ops_overview.js、ops.test.mjs、ops_overview.test.mjs。ops.rs 只留下入口重定向、既有 API 组合和 guard；shell.rs 移除只服务运营五页的 OpsPage/ops_side_nav。保留 ops_api.rs、ops_overview_api.rs、ops_export.rs、内核与 schema，不留双轨开关。

## 分层结论

| 层 | 状态 | 边界 |
|---|---|---|
| Design | VERIFIED（保真） | 页面规格、当前实际 UI 与候选截图一致；不等于全部 LIDS 历史欠账已修复 |
| Code | VERIFIED（候选） | 五页 React + TS，旧实现同包移除 |
| Automated checks | VERIFIED；GPT 提交前独立复核 PASS | ignored 测试不当作通过；关键隔离 proof 另行实跑 |
| Real-chain proof | VERIFIED（隔离合成输入） | 正式 Router + PostgreSQL + React；不证明共享库写入或平台数据真实性 |
| Deploy | NOT VERIFIED | 3000 保持旧主线，未部署本候选 |
| Mog acceptance | NOT VERIFIED | 待 Mog 在候选审阅，再单独授权合并部署 |

目标无答复重试身份目前为浏览器 JS 模块内存，能跨本次 React 导航与弹窗重开，完整刷新或关闭浏览器后不保留；不把它报告为跨刷新恢复。数据和来源原文未新增浏览器持久化。隔离容器/卷/支持目录均 trap 清理并核验；3111 只读视觉候选留作前端审阅。

## 提交前审查授权与记录

已按 AGENTS.md 调用指定 commit-reviewer（Sonnet/high）。当前进程的认证/model 别名环境与 Claude 登录冲突；仅对子进程去除相关环境覆盖后已到达正式 Sonnet，但返回会话额度用尽，2026-10-16 17:30 Asia/Shanghai 重置（commit-reviewer-sonnet.log）。没有审查结论，不能写为 PASS。Mog 随后在当前会话明确允许本包改用 1 个 GPT 只读审查代理，按同一规范复核后提交 Draft PR。审查 task-id 为 WEB-REACT-001-E-GPT-REVIEW-20261016；独立复核已给出 PASS（staged tree e0e9921ef9931dfd88272249dfbe8cc843800a7e），本节随后同步结论。源码与证明可审阅；Draft PR 交付，不推断合并或部署。

最后自审补充：目标 POST 的 5xx 也可能发生在提交事务后的确认丢失阶段，因此与网络/异常 JSON 一样保留原完整请求；只有明确 4xx 拒绝才清理请求身份。已用独立回执单元反例验证 503 后带修改值重试仍发送原 payload/requestId。

### GPT 独立复核与闭合（2026-10-16）

Reviewer `/root/ops_commit_review`，task-id `WEB-REACT-001-E-GPT-REVIEW-20261016`。结论 **PASS**，审查基线 712286c3 后全部候选源码，复核 tree e0e9921ef9931dfd88272249dfbe8cc843800a7e，随后只同步本审查结论/交付文档。私有完整回执为 commit-reviewer-gpt.md，正式 PR exact head 另留核验记录。

| 发现 | 核实与修正 | 复核结果 |
|---|---|---|
| P2：省略必有 asOf 被解释为 null、误称未迁入；同类 nullable/list 字段也有宽松接受 | 本模块 nullable string/number/bool/object 先要求字段存在，再接受显式 null；字符串列表必须是数组。新增4个合同反例在旧代码上4 fail/3 pass，修复后全通过；正式浏览器增加完整但缺 asOf 的200回包，不再显示未迁入 | reviewer 独立复现原浏览器反例并确认修复；运营18项单元、全前端410测试、正式隔离链路通过 |
| P2：原 target-dialog 与 target-month-dialog 都是月目标，周目标覆盖声明不成立 | 首张明确选择/断言周，下一张选择/断言月；报告记录 targetPeriod；保留早期原图，重新生成最终256组 | reviewer 独立 Pillow 重算256/256 RGB exact，8张周/月元数据与名称逐一对应 |

Reviewer 还独立用浏览器 mock（未写共享库）核实：保存未答复关闭重开、周回执晚到不覆盖月表单、503后跨React路由返回仍原样重试。独立跑运营3文件18测试、Rust相关13 passed/4 ignored、脚本语法和diff检查；全前端及正式PG链路日志由其交叉复核，没有由其再次启动PG重跑。最终PG写入仍只在实施者隔离环境：4 revision/6 receipt，六导出bytes/asOf一致，容器/卷/支持目录已清理。无未闭合的提交阻断项；Mog业务签收与部署仍独立待办。
