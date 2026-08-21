# SCOPE-001 最终独立对抗审查

> 状态: 一次性报告
> 最后核对: 2026-08-20
> 适用范围: 正式 SCOPE-001 在代码开工前的合同、数据库、并发、Current、API/CLI、fixture 与文件边界
> 事实来源: 独立 Agent 对当前权威文档和正式 SCOPE 的只读交叉审查
> 冲突时以谁为准: `AGENTS.md`、用户最新确认、修订后的正式 SCOPE、未来真实 fixture/SQL/测试与数据库副作用

## 结论

首次审查原结论为“条件通过”：总体方向成立，没有扩张到真实插件、AI Agent、Topic、Corpus 或正式趋势，也没有把范围确认误报为代码授权；但原 SCOPE 有 1 个 P0、7 个 P1 和 5 个 P2。它们曾全部写回正式 SCOPE；后续代码前语义冻结又经过四轮独立攻击，所有历史 FAIL 与发现均保留。本报告的当前总状态是：

> **实施范围已经得到用户授权。第四轮当时结论仍为 G1–G4 FAIL/G5 PASS；其 4 个 P1 随后已按最小合同收口。用户明确终止重复的文档复核循环，因此代码门现为 CONTROLLED OPEN FOR TDD，从 F01 失败测试开始；这不把历史 FAIL 改写成 PASS，也不授权真实平台、插件、AI 或生产。**

## 原始发现与处理

| 级别 | 发现 | 处理结果 |
|---|---|---|
| P0 | known-set 只有聚合 Coverage，无法说明具体哪个目标 emitted、failed 或 not_attempted | v1 wire 新增被 hash 覆盖的 `knownTargetResults`；固定 manifest canonical、逐成员唯一、Record 映射和 Coverage 对账；quota 禁止该成员表述 |
| P1 | 先检查 authority 会把首次成功后、authority 已过期的合法 replay 拒绝 | 接入顺序改为认证/hash/完整 routing fence/锁定既有 Package；已有同 hash 创建新 deliveryRef 并返回原 acceptedReceiptRef；只有首次接纳检查当前 authority |
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

## 实施授权边界

用户已授权在 G1–G5 全部通过后创建 SCOPE 明列的十组 fixture、两份 migration、Rust 模块、loopback API、worker、minimal CLI 和验证脚本。任何真实账号/工位、真实 producer、未脱敏材料、插件、模型或生产部署仍需新的范围与授权。

## 后续授权记录

2026-08-20，用户明确批准按修订后的 SCOPE-001 开始实现。该授权授予合成实现证明切片的范围许可，但不能替代后续 G1–G5 语义门；任何一次语义审查发现未关闭 P0/P1，代码门都会重新关闭。该授权也不改变任何真实平台、敏感材料、插件、模型、生产部署或后续产品能力的授权状态。

## 语义冻结基线的第一轮独立审查

2026-08-20，独立 Agent 仅审查已推送 commit `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d`，不读取或修改当前工作区。本轮不重开产品方向，只攻击 G1–G5 是否仍需实现 Agent 自行补规则。

原结论：

> **G1 FAIL / G2 FAIL / G3 FAIL / G4 FAIL / G5 PASS；P0=0，P1=6，P2=2；CODE GATE CLOSED。**

| 级别 | 发现 | 当前处置 |
|---|---|---|
| P1 | API/CLI 没有冻结八责任的字段、未评估/不适用/unknown 及 CLI 词汇 | SCOPE 已增加必有 key、封闭 entry、合法组合、route 权限、provenance 和 CLI 固定词 |
| P1 | 数据库 Oracle 漏 `capture_work_order_target`、`capture_package_coverage`、`record_processing_attempt` | 已补全正式表最终行数、ordered delta、F05 seed 和 lease/epoch 历史 |
| P1 | 外部 ingress code 无唯一故障映射/优先级，existing Package 可跳过 routing fence | 已固定 17 步 ingress 判定、15 个 externalIngressCode、safe error/linked delivery、HTTP/code/零副作用和资源硬上限 |
| P1 | `targetExternalId/source.externalId/payload.sourceExternalId` 无权威顺序 | 已固定三种身份陈述的唯一责任、六行真值和 F06A/F10 输入 |
| P1 | F05 无 Record 却写 `not_formed` | 已规定无 Record 无 Formation 实例，ready/leased 为 not-evaluated，finalized 才 formed/not-formed |
| P1 | 文件白名单、生成物登记与 AGENTS 旧“可改名”时态冲突 | expected fixture 定义为手工权威输入，registry 进白名单，AGENTS 改为本 SCOPE 名称已冻结 |
| P2 | F06B 三类 contract-invalid 输入是否各测一次不明 | 已固定三个 fresh-seed mutation |
| P2 | 实施就绪报告后半部仍有旧 NO-GO 命令式文本 | 已标记为原快照结论，当前以主线处置与 SCOPE 为准 |

上述“已处置”在该轮当时只表示修订已写入权威文档，不表示独立复核已通过；所以该轮结论是 **CODE GATE CLOSED**。后续轮次和用户最终开工裁定见本文末尾，不能用本段历史状态覆盖当前 SCOPE。

## 第一次修订后的第二轮独立审查

2026-08-20，独立 Agent 对第一次修订版再次做只读攻击。它确认三身份、无 Record Formation、F10 unknown、F06B mutation、生成物登记与文件白名单等原问题已经闭合，但发现交付错误边界和外部机器合同仍存在新的实现歧义。

第二轮结论：

> **G1 FAIL / G2 FAIL / G3 FAIL / G4 FAIL / G5 PASS；P0=0，P1=7；CODE GATE CLOSED。**

| P1 | 第二轮发现 | 第三轮最小修订 |
|---:|---|---|
| 1 | pre-routing 错误、已有 Package 后的失败交付和事务内部失败没有可实现的资源边界 | 分成 unlinked safe error、routing-confirmed linked rejection、既有 Package 后的新 rejected delivery 与 commit 前 internal error；逐类固定 HTTP、引用、行数和权威真值 |
| 2 | malformed JSON 未进入封闭 code，route 摘要与 authority 过期 replay 冲突，事务顺序漂移 | ingress 改为唯一 17 步；增加 `malformed_json`；事务纲要逐步复用相同 authority fence 和 replay 顺序 |
| 3 | 每次 delivery 的公开身份与首次 accepted Package receipt 混用 | 分为 `deliveryRef` 与 `acceptedReceiptRef`；GET 只按单次 deliveryRef 查询，replay/conflict 不复用 deliveryRef |
| 4 | applicability、八责任、provenance、epoch 仍缺机器字段、基数或稳定顺序 | 增加精确 JSON 结构、route 真值表、entry 基数/排序、runtime/business/formation 联合真值和 safe error envelope |
| 5 | 多 Package Current 的 received/accepted 时间被压成一个无来源标量 | 时间改为带 delivery/package/observation ref 的有序 entries；F07/F08 明确各返回两组来源时间，不允许聚合 |
| 6 | `workRef` 同时指 Capture Work Order 与 processing work | 外部合同分别固定 `workOrderRef` 与 `processingWorkRef`，并增加互换负例 |
| 7 | 只有 final row total，部分 fixture 仍要求实现者反推事务 delta | F01–F10 全部补齐 fresh seed、accepted ingress、逐 Record processing 与 final Oracle；另补 rejection、internal fault 和 lease epoch 逐步行数 |

第三轮修订没有改变用户已确认的产品权力、真实采集范围、隐私边界或后续 AI/插件授权。它只把已确认语义改写为实现无需自行推断的机器合同。当时在第三次独立复核完成前，主线自审结果没有打开代码门；随后复核结果及第四轮处置见下一节。

## 第三轮独立审查与第四轮修订

第三次独立只读审查核对固定九文件快照后，结论为 `P0=0 / P1=7 / P2=4`，`G1–G4 FAIL / G5 PASS`。它确认前一轮的 safe/linked/internal failure、delivery/receipt、Work ref、多来源时间、epoch 与 staged row Oracle 已实质闭合，同时发现这些边缘仍有七个唯一性缺口：

1. JSON syntax、canonical 与 schema 错误集合重叠；
2. 合法 UUID 指向不存在 Work/Attempt 时无封闭结果；
3. 唯一 v1 合同下 `contract_mismatch` 不可构造；
4. 原 accepted delivery 没有独立强类型定义、Package typed relation 和一次 INSERT 规则；
5. F02 无 Record target 的 Formation N/A 没有合法 wire 落点；
6. Current watermark 与 selected/unresolved/unknown provenance 集合不精确，delivery/processing 可能返回断链 Current；
7. Package Acceptance 与 Attempt Terminal 的 union 仍允许相反真值。

第四轮最小修订已分别关闭上述七点，并吸收 CLI human Oracle、F05 fresh/accepted seed 分行、externalIngressCode/rejectionCode 分责和历史轮次标题等非阻断建议。第四次独立复核随后确认这些问题已闭合，但又发现 4 个 P1：payload 内未知字段的责任层级、pre-routing audit 与 public rejected Delivery 的数据库联合类型、动态 ref/time 的 exact snapshot Oracle、以及跨 Attempt Capture Satisfaction 的来源范围。

## 第四轮独立审查后的最终收口与开工裁定

第四轮当时结论为 `P0=0 / P1=4 / P2=2`、`G1–G4 FAIL / G5 PASS`，因此该快照的 CODE GATE 是 CLOSED。主线随后按最小范围完成：

1. ingress 只拥有 Package/Record envelope，payload subtree 唯一由 `content-detail-processor-v1` 判断；
2. `capture_ingress_delivery` 固定 `pre_routing_error|public_delivery` 联合类型，safe error 不伪造 rejected outcome 或八责任；
3. proof harness 使用 manifest 手工冻结的 ref 与 proofNow，完整 JSON 原值做 snapshot，不删动态字段也不由被测实现生成期望；
4. SCOPE-001 限定 Work 1:1 Attempt，Capture Satisfaction 只依赖该唯一 Package；跨 Attempt 合并后置。

用户随后明确要求停止重复的“修订—独立复核”循环并尽快进入代码阶段。由此形成的当前裁定是 **CONTROLLED OPEN FOR TDD**：不再进行第五轮代码前独立文档审查，从 F01 失败合同测试开始，以测试、数据库约束和真实副作用继续验证规格。历史第四轮 FAIL 不改写为 PASS；若实现需要改变产品含义、权限、F01–F10 结论，或扩入真实 producer/Raw Artifact/插件/AI/生产，仍必须 `DECISION_REQUIRED`。
