# Linggan Intelligence Agent Contract

## 最高约束：文件治理与变更留痕

本仓库按“图书馆”而不是“临时工作台”管理。任何 Agent 在创建、生成、移动、重命名、归档或删除文件前，必须先阅读 `docs/README.md`、`docs/current-state.md` 和 `docs/governance/`。

- 不得把报告、计划、截图、导出物、生成代码、构建产物或临时文件随意放在根目录或任意业务目录。
- 新文件必须先确定生命周期、归属目录、稳定名称、责任来源和索引入口；无法分类时停止入库，记录为 `DECISION_REQUIRED`。
- 所有重要 Markdown 必须带状态头，并登记到 `docs/README.md`；未被索引的重要文档视为未完成交付。
- 生成型文件必须登记在 `docs/governance/generated-artifacts-registry.md`，说明生成来源、固定位置、是否入 Git、再生方式和清理规则。未登记的生成型文件不得提交。
- `references/` 是校验和保护的历史证据区。不得为了修正文档冲突而改写其中原件。
- 新文档与旧文档、代码、合同或测试冲突时，不得并存两个“最高版本”。必须按 `docs/governance/agent-collaboration.md` 完成冲突裁定、旧文档降级或替代标记，并写入当月变更记录。
- 修改代码、合同、配置、数据库描述或权威文档时，必须同步更新相关说明和 `docs/progress/YYYY-MM.md`，记录事项编号、原因、影响文件、冲突处理和验证结果。不要用无意义代码注释代替变更记录。
- 文件删除、覆盖和归档必须可解释、可追溯；默认保留并降级旧材料，不得静默覆盖或随意清理。
- 提交前运行 `./scripts/check-project-governance.sh`。检查失败时，文件治理任务不算完成。

以上规则高于 `references/`、历史 handoff、一次性报告及任务对话中的临时建议。具体分类和协作流程以 `docs/governance/` 为准。

## 目标

构建能够持续观察世界、保存可信事实、发现变化并形成可行动情报的系统。

## 事实优先级

1. 真实运行结果、真实数据库副作用和可复现测试证据。
2. 新仓库实际代码、数据库 migration、跨边界合同和 producer fixture。
3. `docs/decisions/` 中标为 ACCEPTED 的决定。
4. `docs/README.md` 标记为“权威当前”的文档。
5. 活跃计划、一次性报告和历史归档。
6. `references/` 中的历史代码和讨论资料。

`references/` 永远不是可直接执行的现行需求。旧文档中的“已完成”只描述当时状态。

## 不可违反的规则

- 禁止把旧 Prisma schema 或旧 migration 整体移入新数据库。
- 禁止从旧数据推断不存在、不可用、活跃或可信等事实。
- 禁止 V1/V2 双写、字段级 fallback 和静默降级。
- 所有外部输入、事件、数据库 JSON 都必须运行时校验。
- 下游副作用失败时禁止返回完整成功；需要原子性的链路必须同事务完成。
- mock、类型断言和编译通过不能证明真实链路成功。
- Evidence 不可静默覆盖；更正以新版本追加。
- Agent 输出必须包含证据引用、反例或缺失说明、不确定性和模型/规则版本。
- 不把密码、DSN、Cookie、授权令牌、生产 dump 或个人原始数据提交到 Git。

## Rust 模块边界

- `contracts`：跨边界版本化合同与运行时校验。
- `domain`：纯领域语言和不变量，不依赖数据库或 Web 框架。
- `evidence`：Evidence 接入、完整性和不可变规则。
- `observation`：从 Evidence 形成可确认观察及时间序列。
- `intelligence`：Feature、Cluster、Signal、Intelligence Event、Outcome。
- `storage-postgres`：PostgreSQL 实现，不向领域层泄露 ORM/SQL 类型。
- `apps/api`、`apps/worker`：组合入口，不承载领域规则。

## 交付纪律

每项能力必须同时交付：来源合同、正常路径、攻击性负例、真实 PostgreSQL 集成证明、文档同步。发现来源不足时冻结相关 lane，继续不依赖它的工作，不得猜造事实。
