# ACC-TOPIC-WORKSPACE-REAL-001 · Topic 工作区验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-02
> 适用范围: Issue #112 Topic Workspace 的 current-main integration、自动 proof 与最终浏览器验收
> 事实来源: current integration migration/Rust/API/HTML/CSS/JS、聚焦测试与隔离 PostgreSQL 16 proof；2026-08-31 Draft 浏览器记录仅作历史输入
> 冲突时以谁为准: 用户最新确认、当前代码/API 合同与真实运行证据；不替代合并、部署或业务验收
> 验收状态: current integration head 的自动证明已完成；2026-09-02 已获授权执行一次隔离浏览器验收，桌面/交互通过，390px 共享一级导航截断，最终浏览器验收未通过
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
| 1440×900 桌面 | Definition v1、2 条材料、三栏、当前选择、无页面溢出 | current head `4ef16a9` 的隔离 browser runtime | VERIFIED |
| 挑战筛选 | aria-pressed=true、列表 1 条、Inspector 角色/标题同步 | current head 隔离浏览器点击 | VERIFIED |
| 390×844 窄屏 | 主工作区单栏、无横向溢出、2 条材料保持可读 | current head 隔离 browser runtime；主工作区 `display:block` 且 document 无横向 overflow，但一级导航第 5 项被裁断 | FAILED |
| 浏览器安全边界 | 无外部 asset、无 console error、Reduced Motion rule 存在 | current head 隔离浏览器：warn/error 0、DOM `script/link/img` 外部 URL 0；source 规则 | VERIFIED |

## 自动验证事实

- `cargo fmt --check --all` 与 `git diff --check`：通过。
- `cargo test -p linggan-intelligence --locked`：2 unit passed；4 PostgreSQL integration tests 在普通 suite 中 ignored。
- `./scripts/test-topic-workspace-postgres.sh`：4 intelligence + 1 API proof passed；隔离 container/volume cleanup verified。
- `cargo test -p linggan-api topic_ --locked`：4 route/API tests passed，1 proof test在普通 suite 中 ignored。
- `cargo test --workspace --all-targets --locked`：通过；API 67 passed / 16 isolated ignored，Topic 普通测试 2 passed / 4 proof tests ignored。
- `./scripts/test-local-001-discovery-postgres.sh`：current-main full-schema isolated chain passed，涵盖 discovery、producer、#133 reobservation、projection/social/media、Topic 和 API proof；临时数据库、container、volume 清理完成。

## 隔离浏览器验收（2026-09-02）

- 运行对象：`4ef16a972d02f5f4284b41787c1d82402e0ee559`；独立 PostgreSQL 16 container、独立 `topic_browser_proof_*` database、loopback `127.0.0.1:31092`。数据库只写入两条 synthetic、已接纳的 Work Resource，再通过同一 loopback API 创建 `任务启动困难（隔离验收）` v1；没有读取或复制 shared Evidence、没有访问外部平台。
- 桌面 `1440×900`：暂定/人工/来源边界、v1、冻结 Pack、两条分类材料和右侧当前材料均与 API 对应；`scrollWidth=innerWidth=1440`。
- 交互：点击「挑战材料」后 `aria-pressed=true`、可见 option 数为 1、该 option `aria-selected=true`，Inspector 同步到挑战材料标题。
- 窄屏 `390×844`：Topic 主工作区按规格改为单栏、`scrollWidth=innerWidth=390`、材料仍有两条可读项；但 shared 一级导航中「采集」控制项 bounding rect 为 `344–430px`，超过 `390px` 视口并被裁断。该缺口由 Topic 入口成为第五项时暴露，不能以「页面无 document overflow」替代完整导航可达性。
- 浏览器日志 warn/error 为 0；页面 DOM 中 `script[src]`、`link[href]`、`img[src]` 无外部 URL。临时 tab、API 进程、container、volume 和 synthetic database 已清理；正式 `:3000` 健康仍为 `READY / PLUGIN_RUNTIME_002_SCHEMA_READY`。

## 未证明

尚未证明共享数据库已应用 `0031`、运行时部署、真实业务材料已人工裁定、390px shared 一级导航修复后的完整浏览器验收、外部 Chrome/辅助技术、长期性能、正式 Topic Release、Agent 运行或 Mog 业务验收。本报告不能被解释成“任务启动困难”已成为正式领域知识或趋势事实。
