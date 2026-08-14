# Linggan Intelligence Agent Contract

## 目标

构建能够持续观察世界、保存可信事实、发现变化并形成可行动情报的系统。

## 事实优先级

1. 新仓库实际代码、数据库 migration 和真实运行结果。
2. `docs/decisions/` 中标为 ACCEPTED 的决定。
3. `docs/architecture/` 与 `docs/product/`。
4. `references/` 中的历史代码和讨论资料。

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
