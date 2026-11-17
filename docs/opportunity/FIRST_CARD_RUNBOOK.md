# 第一张真实机会卡：待授权运行手册

**当前状态：未执行。** 现有只读记录显示本机没有评论研究问题定义、归属或机会卡；最新已确认的本机迁移为 0223–0225，本分支新增 0226 尚未部署或迁移。数字和运行版本可能随时间变化，执行前重新只读核对。相关授权 A1–A4 当前均为否；生产环境不在本手册授权范围。

## 0. 确认范围

负责人指定领域和评论数上限、允许的模型与费用上限，明确本机库身份。先核对迁移账本、服务排空、备份及恢复验证；任何身份、账本、备份、配置或预算不明确时停止。不要把 DSN、密钥、评论正文或原始标识写到 Git、日志或截图。

## 1. 迁移：⏸ 待授权 A1

经明确授权且完成上述检查后，使用 `bash scripts/local-runtime.sh migrate`；命令入口 `scripts/local-runtime.sh:544`，脚本对迁移哈希不一致拒绝继续（同文件约 291 行）。核对运行库账本与实际部署版本迁移逐项一致，要求缺失数、checksum 不符数均为 0。脚本报错立即停止；不要重试或改写已执行迁移掩盖不一致。

## 2. 评论研究：⏸ 待授权 A2

选择指定领域中的真实评论，通过现有评论研究页面逐条预览和发起，串行执行。现有入口 `apps/api/src/local_web/comment_study_command_api.rs:21` 定义了预览、发起和查询接口；`crates/intelligence/src/ai_foundation/comment_research.rs:19` 定义请求字段，预览返回输入哈希与配置版本，提交收据包含 `requestRef`、`batchRef`。不能凭空构造来源或绕过配置、频率与权限。

每次请求只指定一条评论；`crates/intelligence/src/ai_foundation/comment_research.rs:130` 创建 1 个分析项，随后为问题归属和配对审查各创建 1 个阶段项（同文件约 149 行）。按授权上限 N 条，基础阶段项最多按 3N 估算；并非模型调用次数上限，实际可能因资格、跳过、失败和重试不同，必须在授权前另核实可执行的调用与费用上限。模型可能接收选定评论、父评论和作品上下文（`crates/intelligence/src/ai_foundation/executor/service/comment_research.rs:126`）。任务配置、预算、提供方或可用材料不满足时停止；不能因无结果自动扩大采集范围。

逐条查看任务终态和问题定义、归属是否实际发布。研究完成并不保证出现卡片；例如发布门槛、资格、未知协议和当前周期样本均会影响结果。不要用手写问题或合成评论代替真实结果。

## 3. 提及、指标与卡片：⏸ 待授权 A3

以下命令由 `apps/worker/src/bin/opportunity.rs:13` 定义；运行前确认所用二进制版本、库身份、摘要密钥和开关配置。只为获授权领域执行；`sync-comment-mentions` 没有领域参数，实际影响范围须在授权时明确，不能当作领域级写入。

```sh
cargo run -p linggan-worker --bin linggan-opportunity --locked -- sync-comment-mentions
cargo run -p linggan-worker --bin linggan-opportunity --locked -- rebuild-problem-metrics --domain <领域 UUID>
cargo run -p linggan-worker --bin linggan-opportunity --locked -- rebuild-cards --domain <领域 UUID> --full
cargo run -p linggan-worker --bin linggan-opportunity --locked -- check-invariants
```

命令读取 `LINGGAN_LOCAL_DATABASE_URL`；前三条另要求机会卡开关启用及有效摘要密钥（`opportunity.rs:98–178`）。不得把地址和密钥写入命令记录。依次检查：提及报告的 `sourceMemberships`、`problemsSynced`、`reconciliations`；指标报告的 `problemsRebuilt`、`periodRowsWritten`、`invariants`；卡片报告的 `cardsBuilt`、`flagCounts`；不变量报告的 `totalViolations` 和 `incomplete`。这些字段定义分别见 `opportunity/mentions.rs:71`、`problem_metrics.rs:68`、`card.rs:69`、`invariants.rs:21`。任何命令非零、对账原因非空、不变量不完整或非零时停止并调查，不继续后续写入。

## 4. 页面验收：⏸ 待授权 A4

获得授权后，按现有部署流程确认目标内部环境启用开关，从机会首页进入指定领域，检查卡片、详情和证据链路。记录卡片数、等级、质量提示及当前周样本与目标的比值；缺失值如实记未知。截图放在授权的私有证明位置，并排除原始评论者身份和其他敏感信息。当前尚无真实卡片截图或业务验收结果。
