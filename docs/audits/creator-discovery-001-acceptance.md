# CREATOR-DISCOVERY-001 实施与验收

> 状态: 一次性报告
> 最后核对: 2026-10-08
> 适用范围: codex/creator-discovery-001 的语料与创作者模块候选实现
> 事实来源: 当前分支代码、隔离 PostgreSQL、合成 Pi adapter、HTTP 与独立代码审查
> 冲突时以谁为准: 用户最新授权、当前代码/运行与可复现测试；本报告不授予部署或真实调用权限

## 交付对象

基线 main `5f31b8588a0d4e2d6b8b1a7b55656b311516b6ff`。开发工作树为 `<repo>/.worktrees/creator-discovery-001`，未写入共享 checkout。需求、UI 依据和冲突裁定见[执行规格](../plans/active/creator-discovery-001.md)。

页面 `/corpus/creators` 从当前领域的材料用途读取作品、稳定作者和来源分析；通过现有语料路径读代表作与完整命中集合。迁移新增用途策略、作品/作者派生结果与有界租约状态，并统一作者归属的共享 owner。具体实现由 GPT-6 Sol subagent 执行；主代理负责范围、架构和集成审查，另有独立 Sol reviewer。

## 验证账本

单元、合成数据和本地候选运行都不能代替共享环境部署或 Mog 验收。临时日志按产物登记规则保存在 `/tmp/creator-discovery-*.log`，不提交业务数据或构建产物。

| 层级 | 结果 | 证据及限制 |
|---|---|---|
| API / worker 编译 | VERIFIED | `cargo check -p linggan-api -p linggan-worker --locked`；最终日志 `creator-discovery-final-check.log` |
| 单元与二进制测试 | VERIFIED | API 304、contracts 21、evidence 96、intelligence 134，共 555 通过；42 项按原测试标注 ignored，未计为通过。日志 `creator-discovery-final-unit.log` |
| 8 作者 / 12 作品基准及反例 | VERIFIED | evidence PostgreSQL 2/2，日志 `creator-discovery-pg-final.log`；作者8、作品12、未知身份1、相关作者7、垂类2、亲历4、名单外6、名单外爆款3；组合亲历爆款命中2。包含精确回链、同篇交集、关系查询失败、人工支持片段失效、角色保留、其他暂停领域主角色冲突与并发登记 |
| worker 合成模型路径 | VERIFIED | PostgreSQL 1/1；真实本地 Pi 子进程与 invocation 账本，预算、资格、三次有界失败、租约/孤儿回执恢复、指标变更不重跑、人工结果保留。日志 `creator-discovery-worker-dispatch-admission.log` |
| 共享作品读取回归 | VERIFIED | `material_projection_postgres` 串行 17/17；日志 `creator-discovery-shared-regression-serial.log` |
| 真实本地 HTTP | VERIFIED | 候选 API + 一次性 PostgreSQL，37/37；页面与资产、列表/代表作、域隔离、语料精确回链、非法参数、跨域写拦截、登记角色/状态保留、策略 CAS、人工覆盖/恢复、重试边界与暂停域写保护。日志 `creator-discovery-http.log` |
| 静态与治理检查 | VERIFIED | 三项 JS 语法、运行脚本语法、diff 空白检查、项目治理及 UI 手册治理检查；这些不证明视觉体验 |
| 浏览器视觉及交互 | NOT VERIFIED | IAB 与 Chrome 对专用 3107 地址返回 `net::ERR_BLOCKED_BY_CLIENT`；没有截图、完整页面点击或 Mog 验收 |
| main / origin 集成 | NOT VERIFIED | 当前候选分支；提交或 Draft PR 不等于合并 |
| 共享 migration / 3000 服务 | NOT VERIFIED | 本轮没有应用到业务库或更新常驻服务 |
| 真实模型语义与平台采集 | NOT VERIFIED | 未调用真实模型端点，没有发起平台采集 |

共享作品 PG 首次并行执行触发测试容器 PostgreSQL 锁内存上限，串行重跑 17/17 通过。worker 的来源变更用例最终使用子进程 ready/release 屏障，避免以 sleep 猜测模型已经发出。屏障还证明：调用已经准入时禁用策略可以提交；旧调用的实际合成用量仍结算，但不能覆盖已改变的任务快照。

## 隔离证明的再生方法

使用任务专有的 PostgreSQL 16 + pgvector 数据库，`LOCAL_001_PROOF_DATABASE_URL` 必须指向可丢弃的测试实例。fixture helper 会重建命名 schema，不得给它共享业务 DSN。以下数据库用例必须串行执行。

```sh
cargo test -p linggan-api -p linggan-contracts -p linggan-evidence -p linggan-intelligence --lib --bins --locked
cargo check -p linggan-api -p linggan-worker --locked
cargo test -p linggan-evidence --test creator_discovery_postgres --locked -- --ignored --nocapture --test-threads=1
cargo test -p linggan-intelligence --test creator_discovery_worker_postgres --locked -- --ignored --nocapture --test-threads=1
cargo test -p linggan-evidence --test material_projection_postgres --locked -- --ignored --nocapture --test-threads=1
./scripts/check-project-governance.sh
./scripts/verify-ui-design-handbook.sh
```

worker 测试另需 `CREATOR_PROOF_NODE` 指向本机 Node 可执行文件；子进程为已检入的 `tests/support/creator_discovery_adapter.mjs`，只解析真实 Pi 请求并返回合成 JSON，不连接模型端点。预占、完成和故障回执经过现有 model invocation owner。

临时 HTTP 检查脚本为 `/tmp/creator-discovery-http-check.py`，运行方式 `python3 /tmp/creator-discovery-http-check.py`；它依赖上述 fixture 与候选 API。HTTP 核对使用候选 API 的独立 3107 监听，数据库仅指向上述 fixture。测试覆盖读取和必要的合成写命令，实际业务库、常驻 API、worker、插件均未参与。HTTP 验收后、fixture 重建前的快照：WorkOrder 0、model invocation 0、作品人工覆盖非空0，领域已恢复 active，跨域参照登记保持原 dismissed 状态。最后一项前端弹窗上下文保护只做 JS 与窄代码复验，未将之前 HTTP 结果冒充该交互的浏览器证明。

## 审查结论与运行语义

独立审查发现的历史额度字段失配、focus 引用不足、简介更新缺少重新排队、调用资格、失败恢复、机构身份线索、媒体显示、分页回链及关系未知态均已修正。最后复验没有剩余已确认 P1/P2。主代理另修正切换领域时旧行可操作、保存阈值意外改模型选择及前端自行格式化指标时间的问题。

人工积极判断须绑定当前支持片段；依据失效时读为 unknown 并保留原记录。已有目标的角色、暂停/忽略与规则原样保留；跨领域新增主角色冲突由用户显式改选参照处理，登记不创建 WorkOrder。

模型调用在短事务持久记录派发准入后执行；网络等待期间不持有数据库事务。禁用用途阻止后续准入，已准入调用可完成结算。返回结果必须同时匹配 lease 和语义指纹，指标变化本身不触发重新语义分析。

## 部署前置及验收责任

迁移 0115 是新源码的共享作者归属与分析状态前置；代码提交、测试通过或 PR 不意味着已迁移或已部署。共享迁移、合并与更新本机 3000 需要针对最终版本明确授权。正式来源分析用途默认关闭，由用户明确选择模型与预算后才运行。

Mog 仍需在可访问的前端验收真实目录、筛选、依据、返回和纳入观察的产品效果。真实模型效果只能由已授权的小范围真实调用另行证明。
