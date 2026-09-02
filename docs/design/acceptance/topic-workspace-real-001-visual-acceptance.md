# ACC-TOPIC-WORKSPACE-REAL-001 · Topic 工作区验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-02
> 适用范围: Issue #112 Topic Workspace 的 current-integration 实现、自动 proof 与最终浏览器验收
> 事实来源: current integration migration/Rust/API/HTML/CSS/JS、聚焦测试与隔离 PostgreSQL 16 proof；2026-08-31 Draft 浏览器记录仅作历史输入
> 冲突时以谁为准: 用户最新确认、当前代码/API 合同与真实运行证据；不替代合并、部署或业务验收
> 验收状态: current integration head 的自动证明已完成；2026-09-02 已获授权完成一次隔离浏览器验收，并在同一 Issue 内根治 shared Shell 窄屏一级导航截断后完整复验通过
> Issue: #112
> 页面: `/topics/task-initiation-difficulty`

## 验收矩阵

| 场景 | 预期 | 当前证据 | 结果 |
|---|---|---|---|
| 新 Topic import | v1 暂定定义、run、pack、members、receipt 原子形成 | PostgreSQL proof | VERIFIED |
| 未知 Work 引用 | 整笔拒绝且 Topic 表 0 行 | PostgreSQL 负向 proof | VERIFIED |
| exact replay | 返回同 Receipt，不扩写版本 | PostgreSQL proof | VERIFIED |
| 同 key 异内容 | 409 idempotency conflict | domain + API mapping | VERIFIED |
| 版本竞争 | 只有当前 expectedVersion 可产生下一版 | PostgreSQL proof | VERIFIED |
| API 组合 | exact member 顺序 + shared Work Resource；无 body copy | full migration API proof | VERIFIED |
| 失败语义 | 拒绝、冲突、未找到和不可用不伪装成同一状态 | Topic API unit test | VERIFIED |
| 未配置页面 | 首屏明确暂定/人工/来源边界/未读取 | Rust route test | VERIFIED（source/DOM） |
| 1440×900 桌面 | Definition v1、2 条材料、三栏、当前选择、无页面溢出 | shared Shell 修复后的 current integration 隔离 browser runtime | VERIFIED |
| 挑战筛选 | aria-pressed=true、列表 1 条、Inspector 角色/标题同步 | current integration 隔离浏览器真实点击 | VERIFIED |
| 390×844 窄屏 | 主工作区单栏、无横向溢出；5 个一级职责初始可见 | full-data 隔离验收内容区 375px（滚动条）时五项为 0–75、75–150、150–225、225–300、300–375px；最后 stylesheet 复验内容区 390px 时为 0–78、78–156、156–234、234–312、312–390px | VERIFIED |
| 430×932 窄屏 | 5 个一级职责同一行、无横向 overflow | document/nav 宽度均为 430px；五项依次为 0–86、86–172、172–258、258–344、344–430px | VERIFIED |
| 320×844 防御窄屏 | 可用列不足时换行，不裁断或横滑隐藏后续职责 | 前四项各 80px；「采集」自动进入第二行 0–80px；document/nav `scrollWidth=clientWidth=320` | VERIFIED |
| 窄屏入口操作与键盘焦点 | 可用入口保持真实链接、禁用项不变、focus 可见 | 390px 真实点击「采集」抵达 `/collection/attention`；Topic 链接具有 `solid 2px` focus outline，启用入口均为真实 `<a>`、禁用职责仍是 disabled `<button>` | VERIFIED |
| 浏览器安全边界 | 无外部 asset、无 console error、Reduced Motion rule 存在 | current head 隔离浏览器：warn/error 0、DOM `script/link/img` 外部 URL 0；source 规则 | VERIFIED |

## 自动验证事实

- `cargo fmt --check --all` 与 `git diff --check`：通过。
- `cargo test -p linggan-api narrow_primary_navigation_wraps_instead_of_hiding_a_responsibility_offscreen --locked`：shared Shell 窄屏回归守门通过。
- `./scripts/test-topic-workspace-postgres.sh`：6 intelligence（含 cross-domain canonical 与 cross-Topic idempotency 并发）+ 1 API proof passed；隔离 container/volume cleanup verified。
- `cargo test --workspace --all-targets --locked`：通过；API 68 passed / 16 isolated ignored，Topic 普通测试 2 passed / 4 proof tests ignored。
- `node --check apps/api/src/local_web/topic_workspace.js` 与 `./scripts/check-project-governance.sh origin/main`：通过。
- 一次最终严格审查（`origin/main...current integration HEAD`）：在同一审查内根治 canonical/idempotency 两类全局唯一身份的并发语义泄漏，并以 `WorkspaceVersionRefs` 移除位置参数豁免；复验后 PASS，无遗留高置信发现。
- `./scripts/test-local-001-discovery-postgres.sh`：current-main full-schema isolated chain passed，涵盖 discovery、producer、#133 reobservation、projection/social/media、Topic 和 API proof；临时数据库、container、volume 清理完成。

## 隔离浏览器验收与共享 Shell 复验（2026-09-02）

- 运行对象：shared Shell 修复后的 current integration working tree；独立 PostgreSQL 16 container、独立 `topic_browser_proof_*` database、loopback `127.0.0.1:31092`。数据库只写入两条 synthetic、已接纳的 Work Resource，再通过同一 loopback API 创建 `任务启动困难（隔离验收）` v1；没有读取或复制 shared Evidence、没有访问外部平台。
- 桌面 `1440×900`：暂定/人工/来源边界、v1、冻结 Pack、两条分类材料和右侧当前材料均与 API 对应；`scrollWidth=innerWidth=1440`。
- 交互：点击「挑战材料」后 `aria-pressed=true`、可见 option 数为 1、该 option `aria-selected=true`，Inspector 同步到挑战材料标题。
- 根因修复：shared `shell.css` 在 `≤640px` 将一级 `<nav>` 改为 `repeat(auto-fit,minmax(72px,1fr))` 的网格，并显式取消父 global row 遗留的 `overflow-x:auto`。五项在 390px 同屏；更窄或未来职责增加时增长下一行，不以不可见的横向滑动作为可达性的前提。路由、current、disabled、中文职责和状态真值没有改变；只有英文技术旁注在窄屏隐藏。
- 窄屏：请求 `390×844` 时，实际文档内容宽度为 375px，五项的完整 bounds 为 0–75、75–150、150–225、225–300、300–375px；请求 `430×932` 时为 0–86、86–172、172–258、258–344、344–430px。两档均 `scrollWidth=clientWidth`，没有 document 或 nav 横向溢出。额外 `320×844` 核对了第五项自动换入第二行，仍无裁断。
- 入口与焦点：390px 下真实点击「采集」到达 `/collection/attention` 并正确标记 current；启用项保持真实 `<a>`，禁用项保持 disabled `<button>`。Topic 主链接获得 `solid 2px` visible focus outline。浏览器自动化接口不提供全局 Tab-walker；本次不把这替代为外部辅助技术覆盖，但 DOM 的原生可聚焦入口、禁用排除与可见 focus 已在运行页核对。
- 最后 exact-stylesheet 复验：在关闭 full-data proof 后，以不连接数据库的 loopback 静态 Topic 页加载同一 current integration CSS；请求 `390×844` 与 `430×932` 均为 document/nav `scrollWidth=clientWidth`，父 global row 与 nav 的 `overflow-x` 均为 `visible`，五项完整 bounds 分别为 0–78 … 312–390 与 0–86 … 344–430。该复验只验证最终样式的 shared header，不替代上面的 full-data 读取/筛选验收。
- 浏览器日志 warn/error 为 0；页面 DOM 中 `script[src]`、`link[href]`、`img[src]` 无外部 URL。验收 tab 与 API 进程已关闭；独立 `linggan-topic-browser-proof-2b75b814` container、同名 volume 和其 synthetic database 均已删除，列举复核为空。正式 `:3000 /health` 仍是 `READY / PLUGIN_RUNTIME_002_SCHEMA_READY`。

## 未证明

尚未证明共享数据库已应用 `0031`、运行时部署、真实业务材料已人工裁定、外部 Chrome/完整辅助技术、长期性能、正式 Topic Release、Agent 运行或 Mog 业务验收。本报告不能被解释成“任务启动困难”已成为正式领域知识或趋势事实。
