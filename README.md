# Linggan Intelligence

> 状态: 权威当前
> 最后核对: 2026-08-20
> 适用范围: 项目总入口
> 事实来源: 当前仓库、ACCEPTED ADR 与实际验证结果
> 冲突时以谁为准: `AGENTS.md`、真实代码与运行证据

Linggan Intelligence 是一个面向垂直领域的持续情报研究系统。它以可追溯的现实世界观察为起点，让人和 Agent 长期理解领域变化，并支持内容、产品和商业判断。

语料库、主题地图、市场洞察、情报、选题库和 Agent 接口是这套内核面向不同任务的应用能力，不是彼此独立的事实系统，也不能因为某个应用深入就重新定义项目内核。

本仓库不是旧“内容工作台”的整体复制，也不是对已验证 V2 内核的盲目推倒重写。它以 Rust 和全新 PostgreSQL 数据库重新实现产品，同时把现役插件与 V2 合同作为可追溯参考资产。

## 从这里开始

1. 先阅读 [`AGENTS.md`](AGENTS.md)，确认最高约束。
2. 通过 [`docs/README.md`](docs/README.md) 进入文档地图。
3. 查看 [`docs/current-state.md`](docs/current-state.md)，只处理当前已确认事项。
4. 根据任务按需阅读产品、架构、运行手册或历史证据，不一次性加载整个 `references/`。
5. 提交变更前运行 `./scripts/check-project-governance.sh`。

## 开发环境

本项目使用“本机 Rust + Docker PostgreSQL 16”。第一次运行先启动 Docker Desktop，然后执行：

```bash
./scripts/setup-local-env.sh
./scripts/dev-db.sh up
./scripts/verify-development-environment.sh
```

详细说明见 [`docs/runbooks/development-environment.md`](docs/runbooks/development-environment.md)。

## 已确认边界

- 后端与 worker 使用 Rust。
- 数据库使用全新 PostgreSQL，不带旧表、不回填伪 V2 事实。
- 浏览器插件以生产验证过的 v2.0.95 为迁移起点。
- 旧工作台与旧数据库只作只读参考。
- Evidence、Observation、Signal、Intelligence 必须能回溯到来源。
- 类型、mock 或接口返回值不能单独证明运行成功。
- 真实 ADHD、儿童、家庭和医疗原文默认仅限内部受控使用；工作台默认脱敏，外部 Agent 不得批量读取原文，获准隐私处置必须传播到派生物和缓存。
- 第一阶段外部 Agent 可以在明确委托范围内查询、分析、提出建议和申请有边界动作；它不能直接改正式知识、观察基线、真实采集或现实行动，申请与授权也不能被报告为执行成功。
- 第一阶段只承诺可追溯 Decision/Action、结果渠道可获得性和有来源的实际结果；未接入的私信、咨询、销售等保持未知，不伪装成自动 Outcome 闭环。
- 第一阶段只展示有时间与捕获范围边界的近期观察和候选变化，不承诺市场增长/下降等正式趋势；趋势资格等待真实跨时间、账号、Coverage 与分析版本可比性实验。
- 正式领域语言采用可追溯的 Topic 长期身份、定义版本和明确发布；实验聚类词表、分类运行和页面导航不能静默改写正式 Topic，定义发布但分析未重算时必须显示版本与未就绪状态。
- Corpus 是同一来源材料上的用途化选择，不复制第二份原文；Signal 只保存有边界的分析发现；Market Insight/Intelligence Brief 固定当时 Claim 与依据；Content Idea 与 Domain Topic、实际发布和 Outcome 分开。

## 当前状态

`DISC-001` 七道基础设计关口和正式 `SCOPE-001` 的合成切片授权已经完成，Rust 与 Docker PostgreSQL 16 环境基线也已建立。当前处于代码前语义冻结：必须先让 F01–F10 的逐层状态、unknown、正负 Oracle 和证明范围通过 G1–G5，再进入合成 fixture、两份 migration 与 Rust 事实内核 TDD。真实平台访问、AI Agent 内核和插件升级仍未获准开始。具体以 [`docs/current-state.md`](docs/current-state.md) 和完整环境验收脚本的实际结果为准。
