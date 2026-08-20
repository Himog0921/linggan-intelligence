# SCOPE-001 最终独立对抗审查

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: 正式 SCOPE-001 在代码开工前的合同、数据库、并发、Current、API/CLI、fixture 与文件边界
> 事实来源: 独立 Agent 对当前权威文档和正式 SCOPE 的只读交叉审查
> 冲突时以谁为准: `AGENTS.md`、用户最新确认、修订后的正式 SCOPE、未来真实 fixture/SQL/测试与数据库副作用

## 结论

审查原结论为“条件通过”：总体方向成立，没有扩张到真实插件、AI Agent、Topic、Corpus 或正式趋势，也没有把范围确认误报为代码授权；但原 SCOPE 有 1 个 P0、7 个 P1 和 5 个 P2。当前已将 P0/P1 与适用的 P2 全部写回正式 SCOPE，因此审查结论升级为：

> **范围设计可请求用户做最后一次实施确认；审查通过本身仍不授权代码、migration、fixture、数据库写入或插件改动。**

## 原始发现与处理

| 级别 | 发现 | 处理结果 |
|---|---|---|
| P0 | known-set 只有聚合 Coverage，无法说明具体哪个目标 emitted、failed 或 not_attempted | v1 wire 新增被 hash 覆盖的 `knownTargetResults`；固定 manifest canonical、逐成员唯一、Record 映射和 Coverage 对账；quota 禁止该成员表述 |
| P1 | 先检查 authority 会把首次成功后、authority 已过期的合法 replay 拒绝 | 接入顺序改为认证/hash/锁定既有 Package；已有同 hash 返回原 receipt；只有首次接纳检查当前 authority |
| P1 | API/CLI 没有最小认证合同 | 固定 loopback、运行时临时 secret、三项窄权限、无认证/错权限/secret 泄漏负例 |
| P1 | JCS/hash 可能由同一 Rust 实现自证，I-JSON 和资源边界缺失 | 固定 canonical bytes/hash、RFC 官方向量、独立 Node/reference checker；拒绝重复 key/大整数/非法 Unicode，增加 body/record/string/depth 上限 |
| P1 | 单列 FK 不能阻止跨 Attempt/Package/Content 串错关系 | 冻结组合 FK、聚合一致性、同属与 active authority 约束，并要求绕过 Rust 的直接 SQL 负例 |
| P1 | Current 同时间同值选择来源不确定；unresolved 丢失冲突来源 | 增加类型化 field-source 关系，固定 selected support/conflicting candidates、watermark、exact time 与顺序无关规则 |
| P1 | 新 parser version 承诺与 `capture_record_id` 唯一 Observation 冲突 | 首切片冻结唯一 `content-detail-processor-v1`；第二版本失败关闭，Interpretation Revision 后置 |
| P1 | Package accepted、Attempt 终止和 Work 目标满足可能混用 | 三种结果分责；首次接入固定 Attempt 终态关系并重算 Work satisfaction，部分 Coverage 不冒充目标成功 |
| P2 | F-05/07/08 是多步场景，可能依赖测试顺序 | manifest 固定独立 seed、ordered steps、mutation cases 与逐步预期 |
| P2 | 允许文件漏掉 Cargo、migration、脚本和文档支撑文件 | 分开列出业务 `.rs` 白名单和必要支撑文件白名单 |
| P2 | 函数/public item 门禁可能变成自建语法工具项目 | 文件/依赖方向保持自动硬门；函数优先 Clippy/review；public item 先可靠 warning/report |
| P2 | Gate 5/6 草案仍有“待确认”漂移 | 改成历史退出清单与 SCOPE 已裁定子集；真实 producer/隐私/插件事项继续后置 |
| P2 | 合成 CLI explain 容易被误报为产品价值上线 | 明确 SCOPE-001 是实现证明切片，不是首个真实市场情报产品版本 |

## 不需要重新交给用户决定的事项

审查未发现新的产品权力决定。以下保持不变：

- Rust 模块化单体、一个 PostgreSQL 16、API/worker 组合进程；
- API + minimal CLI；
- synthetic-first，不访问真实平台或真实原文；
- Package 原子接入与逐 Record 处理分开；
- 第一阶段不实现 Topic、Corpus、AI Agent、正式趋势、Web UI 或插件升级。

这些修订是为了让已确认产品规则在合同和数据库中可实现，不是增加新的产品范围。

## 实施确认前最终边界

用户最后确认后才允许：创建 SCOPE 明列的十组 fixture、两份 migration、Rust 模块、loopback API、worker、minimal CLI 和验证脚本。任何真实账号/工位、真实 producer、未脱敏材料、插件、模型或生产部署仍需新的范围与授权。

## 后续授权记录

2026-08-20，用户明确批准按修订后的 SCOPE-001 开始实现。该授权只解除上述合成实现证明切片的代码门，不改变任何真实平台、敏感材料、插件、模型、生产部署或后续产品能力的授权状态。
