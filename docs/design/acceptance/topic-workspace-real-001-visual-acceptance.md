# ACC-TOPIC-WORKSPACE-REAL-001 · Topic 工作区验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-02
> 适用范围: Issue #112 Topic Workspace 的 current-main integration、自动 proof 与最终浏览器验收
> 事实来源: current integration migration/Rust/API/HTML/CSS/JS、聚焦测试与隔离 PostgreSQL 16 proof；2026-08-31 Draft 浏览器记录仅作历史输入
> 冲突时以谁为准: 用户最新确认、当前代码/API 合同与真实运行证据；不替代合并、部署或业务验收
> 验收状态: current integration head 的自动证明已完成，最终浏览器验收待授权
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
| 1440×900 桌面 | Definition v1、2 条材料、三栏、当前选择、无页面溢出 | 2026-08-31 Draft historical browser evidence | NOT VERIFIED（current integration head） |
| 挑战筛选 | aria-pressed=true、列表 1 条、Inspector 角色/标题同步 | 2026-08-31 Draft historical browser evidence | NOT VERIFIED（current integration head） |
| 390×844 窄屏 | 主工作区单栏、无横向溢出、2 条材料保持可读 | 2026-08-31 Draft historical browser evidence | NOT VERIFIED（current integration head） |
| 浏览器安全边界 | 无外部 asset、无 console error、Reduced Motion rule 存在 | source route/asset tests；current runtime 尚无 Topic schema | NOT VERIFIED（current integration head） |

## 自动验证事实

- `cargo fmt --check --all` 与 `git diff --check`：通过。
- `cargo test -p linggan-intelligence --locked`：2 unit passed；4 PostgreSQL integration tests 在普通 suite 中 ignored。
- `./scripts/test-topic-workspace-postgres.sh`：4 intelligence + 1 API proof passed；隔离 container/volume cleanup verified。
- `cargo test -p linggan-api topic_ --locked`：4 route/API tests passed，1 proof test在普通 suite 中 ignored。
- `cargo test --workspace --all-targets --locked`：通过；API 67 passed / 16 isolated ignored，Topic 普通测试 2 passed / 4 proof tests ignored。
- `./scripts/test-local-001-discovery-postgres.sh`：current-main full-schema isolated chain passed，涵盖 discovery、producer、#133 reobservation、projection/social/media、Topic 和 API proof；临时数据库、container、volume 清理完成。

## 未证明

尚未证明共享数据库已应用 `0031`、运行时部署、真实业务材料已人工裁定、current integration head 的浏览器走查、外部 Chrome/辅助技术、长期性能、正式 Topic Release、Agent 运行或 Mog 业务验收。本报告不能被解释成“任务启动困难”已成为正式领域知识或趋势事实。
