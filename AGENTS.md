# Linggan Intelligence Agent Contract

## 开始工作

1. 核对目录、分支、HEAD、未提交改动与本次用户目标。
2. 实施任务先读[协作协议](docs/governance/agent-collaboration.md)，在一个工作包内持续执行已授权工作。只读问题无需建立工作包。
3. 按下表读取本次需要的合同；`docs/README.md` 用于检索入口，不要求通读。仅在核对运行、发布、跨包依赖或当前阻塞时读取 `docs/current-state.md`。
4. 代码和 CI 改动使用独立分支；共享 checkout 或并行开发使用 `<仓库>/.worktrees/<branch-slug>/`。保留他人改动。
5. 提交前运行 `bash scripts/ci-preflight.sh origin/main` 与受影响模块验证；审阅、合并和完成口径遵循协作协议。

用户最新明确授权优先于本仓库流程建议。已确认决定持续有效，不重复请求包内普通技术选择；真实权限、敏感数据、外部副作用和不可逆操作按具体授权执行。

## 按任务读取

| 触发条件 | 必读入口 |
|---|---|
| 修改协作、CI 或交付流程 | [协作协议](docs/governance/agent-collaboration.md)；GitHub 操作按 [Issue 指南](docs/agents/issue-tracker.md) |
| 创建、移动、归档文件 | [文件放置标准](docs/governance/file-placement-standard.md)；生成物另读 [登记册](docs/governance/generated-artifacts-registry.md) |
| 修改业务行为、API、数据、契约、权限、采集、插件或模型调用 | [安全与领域约束](docs/agents/safety-invariants.md)；领域命名按 [领域入口](docs/agents/domain.md) |
| 修改 SCOPE-001 内的 Rust、SQL、worker、fixture 或完成声明 | [SCOPE 执行合同](docs/agents/scope-001-execution-contract.md) 和本次切片；不将旧切片用于扩大本次范围 |
| UI 设计、实现、审查或规范维护 | [UI Agent 渐进执行标准](docs/governance/ui-design-execution.md)、[UI 执行合同](docs/agents/ui-execution-contract.md)、[LIDS](docs/design/lids/README.md)；再读受影响分册 |
| UI 标准件修改 | [组件同步合同](docs/design/lids/component-sync.md)，声明实现修正、共享标准或页面例外 |
| 部署、本机服务或发布 | 对应 runbook、当前运行身份和明确发布授权；本机服务见 [部署手册](docs/runbooks/local-runtime-deployment.md) |

LIDS 是唯一设计表达标准；已确认 Q01–Q12 不重开选型，候选不自动认证。正式前端按 [DEC-0010](docs/decisions/0010-web-frontend-react.md) 使用 `apps/web` React + TypeScript，由 Axum 同源提供；业务资格、授权与真实回执留在 Rust/PostgreSQL。运行设计值从批准的 JSON 源同步至唯一 `apps/web/src/styles/lids_tokens.css`，不复制页面参数。

## 全任务边界

- 真实运行、数据库副作用、可复现测试优先于代码推断；代码/合同优先于历史文档。冲突按协作协议区分事实漂移和未决产品选择。
- Evidence 不静默覆盖；未知不写成零；编译、mock、HTTP 200、PR 合并不能替代真实链路、部署或业务验收。
- 密码、DSN、Cookie、令牌、生产 dump、个人原始材料不进入 Git。真实采集、模型外发、生产写入、迁移和不可逆处置需要相应明确授权。
- `references/` 保留为历史证据；旧 handoff、截图、原型、旧 worktree 不自动成为当前需求或可发布源。
- 新插件只修改当前受控分支内的 `plugins/linggan-intelligence-browser/`；工位、安装身份、授权和额度由 Linggan Rust/domain、数据库、local API 与 collection 页面负责，插件只上报安装并执行受控任务。
- worktree 合并后核对推送、未提交/未跟踪资料及独有提交，再安全回收。常驻 runtime-main 是部署目录，按部署生命周期管理。

审阅频率、协作委托、Issue/Claim、文档同步和验证分级只由[协作协议](docs/governance/agent-collaboration.md)定义；CLAUDE/codx 与模块合同仅路由或补充本模块语义，不复制另一套流程。
