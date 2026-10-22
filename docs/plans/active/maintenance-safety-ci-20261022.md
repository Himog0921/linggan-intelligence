# MAINT-20261022-001：评论模型外发安全与 CI 空跑收敛

> 状态: 活跃计划
> 最后核对: 2026-10-22
> 适用范围: Issue #475 的 Comment Study 默认模型外发边界、历史分支空跑 CI 与已证实死代码
> 事实来源: `origin/main@97981b96504f111bb91ffe2263c47f66ae962145`、当前源码、GitHub workflow 运行记录与合成回归
> 冲突时以谁为准: 用户最新授权、`AGENTS.md`、当前代码和实际运行证据

> 受保护候选: [Issue #475](https://github.com/Himog0921/linggan-intelligence/issues/475) · 工作树 `codex/maintenance-safety-ci-20261022`

## 目的与已证实事实

本包只处理盘点中可复现、可用静态或合成测试证明的维护项：

1. `comment_study_model_runner` 会把冻结的 `input_manifest` 序列化为第三方模型默认请求；该清单含目标 `researchText`、父评论语境和作品语境文本。现有 `comment_cleaning::outbound` 已能脱敏，却未位于该外发边界。
2. `collection-single-run-398.yml`、`comment-study-p1.yml` 与 `topic-map-core.yml` 的 job 条件仅匹配已合并历史 PR 的分支，近期香港运行全部或几乎全部被跳过。
3. `eligible_sources_in_transaction` 是未被调用的兼容包装器，唯一行为是构造全 `Primary` 选择后转调仍在使用的函数。

这不是“已发生外泄”的结论：本包没有读取真实评论、请求供应商或检查历史 provider payload，因此实际外发记录仍为 **NOT VERIFIED**。

## 实施合同

1. 保留数据库中冻结的原始 `input_manifest`、`researchText`、清洗 offset 与原文映射；在 v2 方法冻结请求和 legacy 默认请求共同使用的模型 payload 构造边界创建脱敏副本。
2. 目标、父评论和作品语境中供模型理解的文本都使用同一保守规则脱敏。身份、引用、合同版本与枚举字段不改写。
3. 默认模型 prompt 标注 `comment-study-outbound-redaction.v1`；合成回归必须证明 prompt 中不保留 URL、邮箱、手机号或带联系前缀的标识，同时不改变冻结清单本身。
4. v2 的批次装配、容量判定、请求哈希与派发共用同一脱敏 payload；legacy 仍按既有冻结输入上限处理，避免同一 v2 批次在装配时通过、派发时因不会外发的原始文本而拒绝。
5. 退役只会空跑的三条工作流，保留其测试脚本；增加轻量 PR 治理门（空白差异与项目治理），不把全仓 `cargo fmt` 作为通用门，因为当前 `origin/main` 本身不满足该全仓格式检查。
6. 不改迁移、数据库、worker、运行服务、开关、Provider 配置或历史模型回执；不执行模型调用、真实材料读取、部署、合并或重启。

## 验证与停止条件

- 使用 Rust 合成单元测试验证默认 outbound prompt 以及冻结输入不变；运行受影响 crate 的定向测试、`rustfmt --check`（仅改动 Rust 文件）和工作流 YAML/治理检查。
- 若脱敏会改变证据映射、输出接纳、既有冻结 hash，或发现未覆盖的自由文本字段，停止并把该字段与风险回报 Issue，不扩大为数据迁移或历史重算。
- Draft PR、独立审阅、精确 head CI 与 Mog 合并授权是后续门；本计划和本地测试均不代表运行时或业务验收。
