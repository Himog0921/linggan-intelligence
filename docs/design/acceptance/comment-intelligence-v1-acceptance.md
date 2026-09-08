# CI-20260907-V1 评论研究验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-08
> 适用范围: `codex/comment-intelligence-v1` 实施包的 84 项验收映射、已执行自动证明与未核实层
> 事实来源: 用户原包 `02_执行与验收.md`、当前源码／测试、执行回执；查询规模日志 `/tmp/ci-combined-custom-plan.log`
> 冲突时以谁为准: 用户实施合同、当前可复现源码与实际执行结果；本报告不能证明未发生的浏览器、真实模型、共享迁移或部署

**这是交付分支的自动验证与未验证事项记录，不是84项全部通过或生产发布报告。** 代码实现、隔离PG、SDK协议、真实语义质量、浏览器和共享运行分别记录。最终跨领域碰撞边界与整包复验回执见本页末尾；不为改时间戳重开审核循环。

## 对象、证据等级与运行边界

- 用户交付包：`/Users/moglenny/Downloads/comment-intelligence-v1`。原包 manifest 的 13 项 SHA-256 已由根代理核验；HTML 和预览图是合成参考，不是本实现截图。
- 开发位置：独立 worktree `.worktrees/comment-intelligence-v1`；建立基线 `origin/main@61e8c35`。最终提交 SHA、合并 SHA、共享迁移回执、实际 API／worker 版本尚未形成，不能从基线推断。
- 页面：`/corpus/comments`，概览／原声／用户问题／每日观察；保留 `/corpus/queries` 和历史资产访问。
- 设计关联：[实施合同](../../data-contracts/comment-intelligence-v1.md)、[本包 UI 变更清单](../changes/comment-intelligence-v1-ui-change-manifest.md)。
- `PASS（自动）`：实际执行且结果通过，范围仅限列明断言与合成／隔离条件。
- `IMPLEMENTED`：已找到对应代码，尚未取得该完整场景的执行证据；有部分自动覆盖时明确写出。
- `NOT VERIFIED`：缺少真实输入、运行、浏览器、独立人工质量或部署证明；不得用代码存在补成通过。

| 证据组 | 已知实际结果 | 当前限制 |
|---|---|---|
| Q · 统一查询 PostgreSQL | 4项功能/规模测试已分别通过；最新规模回执 `/tmp/ci-combined-custom-plan.log`，含资源清理 | 最终整包回执见文末；规模样本为跨领域合成原声 |
| H · 人工反馈／问题 PostgreSQL | 9项通过，随机隔离PostgreSQL/container/volume，清理通过 | 证明合成数据下的业务副作用；不代表真实模型语义质量 |
| R · 日批／语义 PostgreSQL | 18项通过；亦在最终model/Pi整包回归再次通过 | 本地合成 provider，非真实评论语义质量或供应商可靠性 |
| K · Packet | semantic 分包转交 4 项单元测试通过 | 严格合同／对象与引用验证，不等于模型判断正确 |
| N · Pi SDK | 转交 11 项本地协议测试通过 | 使用真实 SDK 对本地合成 endpoint；没有真实供应商费用或数据外发结论 |
| E · 离线质量评测器 | 8 项 Python 标准库测试通过 | 仅四条明确合成评论检验计算／拒绝路径；没有真实质量分数 |
| T · 比较门槛 | 已执行 `volume_is_not_share_growth`、`zero_baseline_is_not_infinity`、`import_and_partial_coverage_do_not_create_trends`、`comparable_rise_is_five_percentage_points` | 新增采集签名/人工分类变化条件已接入查询；任意平台采集模式仍需真实回放 |
| 浏览器 | Chrome 对临时 3307 入口返回 `ERR_BLOCKED_BY_CLIENT` | 没有本轮有效截图，未验收响应式、缩放、焦点与完整点击链；不能用旧包截图替代 |
| 100k 读取 | Apple M4 / 24GiB，100,000条**跨领域合成原声**；概览冷528ms/热P95500ms，原声冷357ms/热P95394ms（每类20次） | 数据层概览<1s、原声<500ms在最终复验通过；范围变化使用事务内custom query plan。排除HTTP/浏览器/LLM，未证明本领域已分析规模、并发资源与浏览器≤2s |
| 真实质量／发布 | 未执行 | 真实 200–300 条／≥20 作品人工留出评测、共享迁移、3000 切换、push／merge／deploy、Mog 验收均 `NOT VERIFIED` |

## 真实代码与测试索引

表中符号是源码中的实际名称。文件级链接用于定位；不得因为名字相近就扩大断言范围。

| 代号 | 文件与函数 |
|---|---|
| Q1–Q4 | [comment_intelligence_query_postgres.rs](../../../crates/intelligence/tests/comment_intelligence_query_postgres.rs)：Q1 `one_scope_counts_unique_comments_and_preserves_first_observation`；Q2 `sources_from_another_domain_never_join_own_query_or_actions`；Q3 `semantic_lenses_are_nonexclusive_and_resonance_has_group_evidence`；Q4 `hundred_thousand_comment_scope_is_paged_and_reports_server_p95` |
| B1–B7 | [comment_intelligence_read_boundaries_postgres.rs](../../../crates/intelligence/tests/comment_intelligence_read_boundaries_postgres.rs)：当前正文A→B→A复用合格旧A；旧父评论撤回；真实媒体/OCR依赖撤回；pending/failed/每日范围隔离；模型配置版本计数；0047升级暂停旧授权；跨领域UUID碰撞读写隔离 |
| H1–H9 | [comment_intelligence_actions_postgres.rs](../../../crates/intelligence/tests/comment_intelligence_actions_postgres.rs)：H1 `source_commands_are_idempotent_versioned_undoable_and_immutable`；H2 `problem_identity_survives_rename_split_merge_and_restricts_cross_domain_actions`；H3 `canonical_feedback_survives_reobservation_and_restriction_blocks_replays`；H4 `term_projection_is_incremental_document_frequency_and_versioned_hiding`；H5 `valid_exact_definitions_form_stable_groups_but_singletons_and_human_locks_are_retained`；H6 `restricted_context_withdraws_problem_evidence_without_hiding_raw_comment`；H7 `model_equivalence_uses_frozen_definition_and_never_overwrites_human_locks`；H8 `edited_source_deactivates_manual_decision_until_explicit_current_text_correction`；H9 `legacy_assets_seed_bookmarks_across_observations_without_preventing_unstar` |
| R1–R6 | [comment_daily_postgres.rs](../../../crates/intelligence/tests/comment_daily_postgres.rs)：R1 `work_packet_is_one_call_with_exact_results_and_restriction_propagation`；R2 `daily_windows_are_gapless_deduplicate_reobservations_and_allow_delayed_cleaning`；R3 `selection_replay_pause_and_budget_do_not_duplicate_dispatch`；R4 `comment_facts_keep_zero_and_unknown_separate`；R5 `partial_packet_keeps_good_comments_and_concurrent_ticks_dispatch_once`；R6 `interrupted_packet_keeps_unknown_reservation_and_does_not_silently_rebill` |
| R7–R12 | 同一测试target，R7/R8见主文件，R9–R12见 [comment_daily_semantic_cases.rs](../../../crates/intelligence/tests/support/comment_daily_semantic_cases.rs)：R7 `cleaning_filter_searches_all_sources_and_binds_cursor`；R8 `daily_permission_cannot_be_duplicated_by_legacy_auto_and_pauses_without_connection`；R9 `semantic_work_is_shared_across_batches_and_reobservations`；R10 `malformed_single_item_keeps_other_semantics`；R11 `pause_after_dispatch_accepts_inflight_result`；R12 `continuation_extends_only_undispatched_budget_and_retains_manifest` |
| R13–R18 | [comment_daily_semantic_cases.rs](../../../crates/intelligence/tests/support/comment_daily_semantic_cases.rs)：R13 `late_context_recovers_original_target_after_debounce`；R14 `daily_source_cap_defers_instead_of_losing_manifest_members`；R15 `daily_text_revision_reuses_original_grant_but_reobservation_does_not`；R16 `historical_probe_does_not_qualify_new_semantic_contract`；R17 `semantic_retry_limit_cannot_be_reset_by_another_batch`；R18 `packet_recalls_readable_same_domain_definitions_and_records_checked_proposal` |
| K1–K4 | [comment_packet.rs](../../../crates/intelligence/src/comment_packet.rs)：K1 `v3_exact_quotes_and_strict_independent_items`；K2 `unicode_duplicate_and_missing_are_not_guessed`；K3 `candidate_reference_is_a_server_checked_proposal`；K4 `semantic_fingerprint_ignores_engagement_and_tracks_parent_text` |
| C | [comment_cleaning.rs](../../../crates/intelligence/src/comment_cleaning.rs)：`clean`、`CleanComment::resolve`、`outbound`；测试 `source_mapping_preserves_negation_and_family_emoji`、`low_information_is_retained_and_short_replies_need_context`、`contact_masking_does_not_create_fake_evidence` |
| T | [comment_intelligence_statistics.rs](../../../crates/intelligence/src/comment_intelligence_statistics.rs)：`compare` 与上列四个确定性测试；[comment_intelligence.rs](../../../crates/intelligence/src/comment_intelligence.rs) 提供实际窗口、范围、版本与聚合 |
| N | [adapter.test.mjs](../../../apps/pi-adapter/test/adapter.test.mjs)、[deepseek.test.mjs](../../../apps/pi-adapter/test/deepseek.test.mjs)：包括 `actual Pi AI and Agent SDK use explicit key, no tools, one request, recorded wire usage`、`model calls work without a model catalog and endpoint failures remain actionable`、`provider errors and redirects do not leak credentials or retry internally` |
| E | [test_comment_intelligence_evaluation.py](../../../scripts/tests/test_comment_intelligence_evaluation.py)：合成成绩不获真实资格、作品泄漏／重复预测、引用错误、pairwise／拒绝全部、版本不可比、独立群体审查、CLI／输入覆盖保护、来源声明／Unicode／重复 JSON key，共八项 |
| UI | [comment_intelligence.js](../../../apps/api/src/local_web/comment_intelligence.js)：`query`、`navigate`、`load`、`renderOverview`、`voiceTable`、`renderProblems`、`renderDaily`、`alignInspector`、`openSource`、`openPrepare`、`openCorrection`、`openProblemEdit`、`openLegacyAssets`、`openBatchRecords`；[comment_intelligence.css](../../../apps/api/src/local_web/comment_intelligence.css) 为局部样式 |
| 内部职责拆分 | [问题命令与历史](../../../crates/intelligence/src/comment_intelligence_problem_commands.rs)、[模型候选复验](../../../crates/intelligence/src/comment_intelligence_problem_candidates.rs)、[词项与向量候选](../../../crates/intelligence/src/comment_intelligence_vocabulary.rs)；通过原模块 re-export 保持对外 API，最终固定版本仍需统一复验 |
| API／状态 | [comment_intelligence.rs](../../../apps/api/src/local_web/comment_intelligence.rs)、[comment_daily.rs](../../../apps/api/src/local_web/comment_daily.rs)；[人工命令](../../../crates/intelligence/src/comment_intelligence_actions.rs)、[问题索引](../../../crates/intelligence/src/comment_intelligence_problems.rs)、[日批运行](../../../crates/intelligence/src/comment_daily_runner.rs)、[0047 迁移](../../../database/migrations/0048_comment_intelligence.sql) |

## D · 数据与计数（16 项）

| ID | 原包场景与实现映射 | 实际证明与当前状态 |
|---|---|---|
| D01 | 同一评论采集三次：0047 稳定 canonical 身份、统一 `read` | **IMPLEMENTED**；Q1／H3／H9 证明多观察归同身份、收藏保留；三次观察的完整 first-observed 数值断言未单独覆盖 |
| D02 | 不同 ID 同文本：原始身份独立；群体统计区分复制表达 | **IMPLEMENTED**；H5 保留三条相同正文不同 ID；复制重复在群体支持统计中的完整展示仍待核验 |
| D03 | 三标签／两问题仍只一行：统一作用域及成员连接 | **IMPLEMENTED**；Q3 实际覆盖两标签／一问题不重复行，尚非原包“三标签／两问题”的完整 fixture |
| D04 | 20 条仅 13 条成功：逐项解析、批次明细、失败重试 | **IMPLEMENTED**；R5／R10 已证明局部失败不连坐成功；实际 fixture 为四条混合状态，并非 20／13／7 的精确规模 |
| D05 | 当前领域＋近 30 天所有数字一致下钻：`ResearchScope`、UI `query/navigate` | **IMPLEMENTED**；Q1／Q2 验证若干查询及领域隔离，完整浏览器数字链 `NOT VERIFIED`；临时HTTP概览/详情/问题的计数一致已验证 |
| D06 | 0 赞与未知：nullable 点赞、`voiceTable` | **PASS（自动：数据层）** R4；UI 的 `0`／`—` 代码存在，浏览器呈现 `NOT VERIFIED` |
| D07 | 未配模型／调用失败仍保留原声与未知需求：nullable 语义统计、UI 状态 | **PASS（自动：未分析计数）** Q1；错误／未配置的完整运行页面 `NOT VERIFIED` |
| D08 | 旧相对发表时间：来源文本及“采集时”显示 | **IMPLEMENTED**；未执行“上周采回 15 分钟前”的端到端时间呈现 fixture |
| D09 | 发表时间未知排除：`timeBasis=published`、`excludedPublished` | **PASS（自动：SQL）** Q1 验证未知两条被排除；UI 排除提示仍待浏览器核验 |
| D10 | 统计点击后版本变化：`resultRevision`、`updated` 与 UI `load` | **IMPLEMENTED**；Q3验证隐藏词改变resultRevision；尚无完整浏览器并发更新下钻证明 |
| D11 | 一条长评论两个需求：v3 `semantic.problems[]` 与精确引用 | **IMPLEMENTED**；K1／H5 覆盖基本合同，尚缺明确双需求长评论的完整计数测试及人工语义判定 |
| D12 | 中文／emoji／重复短句引用：Unicode scalar 区间及 `CleanComment::resolve` | **PASS（自动：坐标与拒绝路径）** K2、C；不证明真实模型每次引用都正确 |
| D13 | 新增证据问题 ID 不变：增量成员、definition_revision | **PASS（自动：身份／成员）** H5／H7、R9；真实第二天运营页面与收藏访问 `NOT VERIFIED` |
| D14 | 更名／合并／拆分：命令日志、版本、重定向、人工修正 | **PASS（自动：后端）** H1／H2；浏览器旧链接返回路径 `NOT VERIFIED` |
| D15 | 重复词只贡献一个文档频数：`comment_terms`、term index | **PASS（自动：DF）** H4 及词语单测；点击词云的浏览器链 `NOT VERIFIED` |
| D16 | 北京时间 23:00 边界：`seal_due` 冻结半开窗口 | **PASS（自动：隔离时钟）** R2；真实机器日历调度未执行 |

## S · 语义与上下文（18 项）

| ID | 原包场景与实现映射 | 实际证明与当前状态 |
|---|---|---|
| S01 | “哪里有／怎么做”不按短字数删除：保守清洗 | **IMPLEMENTED**；C 有短回复／低信息区别，两个指定短句的真实语义质量 `NOT VERIFIED` |
| S02 | 纯 @／表情原声保留、不算强需求：C、词索引及 v3 | **PASS（自动：保留／过滤规则）** C、词语单测；真实模型需求识别 `NOT VERIFIED` |
| S03 | “吃药了吗”的治疗语境：同作品 packet、父评论／标题／正文 | **IMPLEMENTED；真实语义 NOT VERIFIED**；没有真实人工 gold 证明它不会误判购买或用药意图 |
| S04 | 父评论未采回不猜邻行：Evidence 关系及 `contextMissing` | **PASS（自动：缺口恢复机制）** R13；指定短句的真实不确定性解释 `NOT VERIFIED` |
| S05 | OCR 未完成不阻塞全部原声：有界上下文、缺口等待 | **IMPLEMENTED**；R13 验证晚到上下文恢复，边界PG已验证OCR真实媒体工作流的可读/撤回；真实语义质量仍待核验 |
| S06 | 不把作品问题抄作评论需求：packet 角色／来源分区 | **IMPLEMENTED；语义 NOT VERIFIED**；H4 只证明词云不取标题，不可冒充需求语义测试 |
| S07 | 方法是用户自述、不称科学有效：标签证据与 UI 原声上下文 | **IMPLEMENTED；真实表述／语义 NOT VERIFIED**；不以协议通过证明用户方法有效 |
| S08 | 改写金句被拒绝：精确 quote 校验 | **PASS（自动：引用拒绝）** K2／C；不会覆盖原文 |
| S09 | 评论内指令不能读取密钥／调用工具：packet 普通数据与无工具 SDK | **PASS（自动：工具与凭据边界）** N；真实模型抗注入语义表现 `NOT VERIFIED` |
| S10 | 漏项／重复 ID／越界 ID：逐项映射与拒绝 | **PASS（自动）** K1／K2、R5／R10；不推测缺失项成功 |
| S11 | 同词相反需求不能仅按相近合并：精确定义边界、模型等价提案复验 | **PASS（自动：边界护栏）** 定义键单测、K3／H7；实际同词异义分类效果 `NOT VERIFIED` |
| S12 | 同义问题归既有定义：服务器候选快照→模型提案→live 定义复验 | **PASS（自动：受控归并机制）** R18／H7；实际同义识别 precision、误归并率 `NOT VERIFIED` |
| S13 | 新单例保留未归并：candidate 与最小成组规则 | **PASS（自动）** H5；真实新问题发现率未评测 |
| S14 | 人工纠正后重跑不覆盖锁：分别锁标签／成员，保存新提案 | **PASS（自动）** H1／H5／H7／H8；正文变化后人工决定保留但失效 |
| S15 | embedding 模型／维度不可混算：`DefinitionVector`、`recall_vector_candidates` | **PASS（自动：纯函数拒绝）** 向量单测；真实 embedding 供应商、持久索引与重算计划 `NOT VERIFIED`，当前运行召回为词项＋模型复核 |
| S16 | 孤立高赞不自动共鸣：群体支持阈值与多作品约束 | **IMPLEMENTED**；Q3 有六条／三作品的正向 fixture，孤立高赞负例未单独执行 |
| S17 | 支持但担忧不当作反对：stance 类型及冲突聚合 | **IMPLEMENTED；真实语义 NOT VERIFIED**；无独立人工群体审查成绩 |
| S18 | 同命题真实反驳展示双方证据／范围：同 target 立场聚合、问题详情 | **IMPLEMENTED；真实语义及浏览器 NOT VERIFIED**；不得把相反情绪当作已证明的争论 |

## A · 观察与趋势（12 项）

| ID | 原包场景与实现映射 | 实际证明与当前状态 |
|---|---|---|
| A01 | 10／100 到 40／400 不能报增长 300%：`compare` 使用占比差 | **PASS（自动）** `volume_is_not_share_growth`；intelligence lib单测通过 |
| A02 | 零基线不算无穷增长：零分母／历史资格 | **PASS（自动：不算∞）** `zero_baseline_is_not_infinity`；“本库新见”历史资格未独立证明 |
| A03 | 新分析旧材料与新讨论分责：分析时间、首次观察、late_material | **PASS（自动：比较门槛）** `import_and_partial_coverage_do_not_create_trends`；每日观察实际页面 `NOT VERIFIED` |
| A04 | 扩采 100 个目标范围不可直接比较：采集签名／unknown_capture | **IMPLEMENTED**；门槛已接入，未执行原包扩采规模fixture |
| A05 | 覆盖差过大停止升温：覆盖下限与差值门槛 | **PASS（自动：覆盖不足）** T；完整两窗口覆盖差／UI 解释待复验 |
| A06 | 模型／规则版本不同不画涨跌：版本签名比较 | **IMPLEMENTED**；新增边界PG证明相同配置不同semantic UUID合为一个版本、不同配置计为两个版本；真实变更回放未执行 |
| A07 | 仅一篇作品不能称跨社区：current works 阈值／来源分布 | **IMPLEMENTED**；H2 单篇问题可读，单篇变化卡负例与UI集中提示待核验 |
| A08 | 人工整理不制造新问题：稳定定义／别名，classification_changes | **PASS（自动：身份）** H2／H5；比较条件已接入，事件流完整断言未完成 |
| A09 | 不够规则不凑三卡：后端观察资格、UI `observations` | **IMPLEMENTED**；无“必须三张”的客户端填充；完整空观察执行证明待补 |
| A10 | 离线／停采减少不称消退：覆盖与完整日门槛，不自动生成下降结论 | **IMPLEMENTED**；T 验证观察日不足；真实离线时间回放 `NOT VERIFIED` |
| A11 | 词典升级识别旧词不当新词：词典版本、独立词频索引 | **IMPLEMENTED**；H4 覆盖版本化隐藏，不证明实际词典升级后的新旧词事件判断 |
| A12 | 模型数字与程序不符不得发布：观察数字由服务器确定性统计提供 | **IMPLEMENTED**；当前卡片不依赖自由文本 LLM 数字；指定数字冲突负例未执行 |

## U · UX 与视觉（16 项）

**本组均未取得本轮有效浏览器证据。** 下列实现映射不能替代截图、键盘操作、缩放或两次点击路径。

| ID | 原包场景与实现映射 | 状态与待证边界 |
|---|---|---|
| U01 | 无 view 默认概览：页面 initial view、UI 默认 `overview` | **IMPLEMENTED；浏览器 NOT VERIFIED** |
| U02 | 六类点击继承范围并能回退：`query/navigate`、lens 事件、popstate | **IMPLEMENTED；浏览器 NOT VERIFIED** |
| U03 | 点词云进入真实词命中：term index、`markedBody`、词按钮 | **IMPLEMENTED**；Q1／H4 部分 SQL 证明；浏览器 NOT VERIFIED |
| U04 | 评论面板顶边对齐面包屑下沿：`alignInspector` 读取 `.v7-context-row` | **IMPLEMENTED；浏览器 NOT VERIFIED**；未测导航遮挡 |
| U05 | 缩放／壳高变化重新定位：`ResizeObserver`、resize／scroll 监听 | **IMPLEMENTED；浏览器 NOT VERIFIED**；不能用源码没有 118px 证明实际位置正确 |
| U06 | 作品入口复用已有路由：`linkWork/openSource` | **IMPLEMENTED；浏览器 NOT VERIFIED**；未证明不存在焦点或返回路径问题 |
| U07 | 关闭／前后条／后退恢复：`closeInspector`、generation、history state | **IMPLEMENTED；浏览器 NOT VERIFIED** |
| U08 | 跨页／过滤选择明确：selected 集合、`renderSelection/navigate` | **IMPLEMENTED；浏览器 NOT VERIFIED**；未执行两条跨页选中全过程 |
| U09 | 问题→原声→上下文两次点击：`renderProblems/openSource` | **IMPLEMENTED；浏览器 NOT VERIFIED** |
| U10 | 旧资产能找回且备注／出处不丢：`openLegacyAssets`、canonical 继承 | **PASS（自动：后端继承与取消星标）** H9；完整 UI 找回路径 NOT VERIFIED |
| U11 | 研究前看范围／模型／目标／预算：`openPrepare`、prepare 冻结集 | **IMPLEMENTED**；Q2 证明 prepare 不发调用；确认弹窗与运行回执浏览器 NOT VERIFIED |
| U12 | 暂停／重试仍可见：`openBatchRecords/openContinue/openUsageReview`、daily controls | **IMPLEMENTED**；R6／R11／R12 有后端证明，入口可发现性 NOT VERIFIED |
| U13 | API 错误／模型未配／无数据分开：`load/renderOverview` | **IMPLEMENTED；浏览器 NOT VERIFIED**；没有生产 mock 兜底的源码不等于错误态走查已通过 |
| U14 | 长故事／多标签／长标题：`voiceTable/tags/openSource` 与局部 CSS | **IMPLEMENTED；浏览器 NOT VERIFIED**；不援引上一个版本的行高或截图 |
| U15 | 键盘／Esc／焦点／词云文字替代：aside 与 dialog 分责、focus 返回、aria 文本 | **IMPLEMENTED；浏览器 NOT VERIFIED**；实际键盘无死路未验收 |
| U16 | 1440／1920／1280／768，125%／150%：媒体查询与布局目标 | **NOT VERIFIED**；没有这些尺寸／缩放的本轮截图或可交互证明，未宣称小字与横滚问题已排除 |

## O · 执行与安全（17 项）

| ID | 原包场景与实现映射 | 实际证明与当前状态 |
|---|---|---|
| O01 | 部署／刷新／模型设置不启用真研究：日计划默认关闭、prepare 无调用 | **PASS（自动：prepare 无外调）** Q2；完整部署／配置动作不外发未运行，真实自动计划未启用 |
| O02 | 目录 404 仍可手填 model ID 调用：模型设置与 SDK catalog 分责 | **PASS（自动：本地 SDK 协议）** N 中目录失败测试；真实供应商当前可用性 NOT VERIFIED |
| O03 | HTTP 200／旧 probe 不代表新语义合格：v3 qualification、离线评测 | **PASS（自动）** R16、E；真实语义质量仍 NOT VERIFIED |
| O04 | 局部成功后只重试失败：逐项状态、重试上限和成功复用 | **PASS（自动）** R5／R10／R17；原包 20／13 数量变体不在已执行 fixture 中 |
| O05 | 超时未知用量保留预留、不无限重付：invocation账本与 usage review | **PASS（自动）** R6／R17、N；真实供应商最终账单尚未核对 |
| O06 | 多 worker 同日仅一个批次／派发：seal唯一身份、租约与语义工作唯一键 | **PASS（自动）** R2／R5／R9；实际常驻多 worker 日历运行未验证 |
| O07 | 暂停后接纳在途结果、不再新派发：dispatch 与 finish 资格分开 | **PASS（自动）** R11；远端是否最终收费仍未知，不能声称暂停取消收费 |
| O08 | 23:00 离线按原 cutoff 补建：`seal_due` 恢复连续窗口 | **PASS（自动：隔离时钟）** R2；真实主机离线回放 NOT VERIFIED |
| O09 | 首次开启不静默全库历史补跑：schedule 启用窗口 | **PASS（自动：受控窗口）** R2／R8；真实库存初次启用未执行 |
| O10 | 默认模型变化不改已冻结批次，停连接阻新调用：冻结config与permitted | **PASS（自动：连接暂停／权限）** R8；实际配置切换完整组合待最终核验 |
| O11 | 仅点赞更新不重做语义：semantic fingerprint／共享工作 | **PASS（自动）** K4、R9／R15 |
| O12 | 父评论／OCR 晚到仅恢复缺口：`recover_context`、去抖与指纹 | **PASS（自动：晚到上下文）** R13；真实 OCR 连续完成事件负载未验证 |
| O13 | 访问撤回传播原声与派生：readable gate、context gate、当前hash | **PASS（自动：查询／详情／问题／重放）** Q1、H3／H6、R1；浏览器缓存及所有导出渠道完整链 NOT VERIFIED |
| O14 | 采集＋媒体＋研究并发有资源上限且浏览不堵塞 | **NOT VERIFIED**；现有 worker 复用／预算代码和R5不能替代三类任务同时负载下的浏览延迟证明 |
| O15 | 截止时未清洗／缺上下文仍冻结身份、稍后继续：daily manifest | **PASS（自动）** R2／R13；真实23点执行未启用 |
| O16 | 超批次上限余项留积压：source_limit 状态与继续动作 | **PASS（自动）** R12／R14；浏览器积压展示 NOT VERIFIED |
| O17 | 两计划同评论同指纹复用结果／在途：semantic_work唯一性 | **PASS（自动）** R9／R17；无真实供应商重复费用试验 |

## P · 性能与发布（5 项）

| ID | 原包场景与实现映射 | 实际证明与当前状态 |
|---|---|---|
| P01 | 当前真实规模＋10万合成，分页／概览／浏览器P95 | **部分 PASS（自动），整体 NOT VERIFIED**：Q4跨领域合成原声，M4/24GiB；概览冷528/热P95500ms、原声冷357/热P95394ms。本领域已有分析/上下文、HTTP、浏览器2s与混合并发负载未验证 |
| P02 | 集成最新 main 后重跑，迁移无冲突／不改已应用文件 | **NOT VERIFIED**：0047代码存在且隔离应用通过；尚未完成最终main集成与固定版本专项复验，不能把建立分支基线当合并证明 |
| P03 | 真实3000入口 API／worker／DB／UI均新版本 | **NOT VERIFIED**：本包未迁移共享DB或切换3000；3307浏览器还被客户端阻断 |
| P04 | 全仓失败区分既有／本次，不说全部通过 | **已核对基线失败，未全绿**：workspace只有API target的6项creator_lifecycle失败（该target135通过/19忽略）；干净origin/main@61e8c35同组也失败6项。新评论/API定向7项通过。clippy全仓与Rust边界检查另有既有超长项，见文末，不报全仓通过 |
| P05 | 生产API失败返回真实错误，无HTML演示数据兜底 | **PASS（自动：API）**：缺schema返回503且不返回合成统计，非法domain返回400，foreign origin写入返回403；生产失败演练/浏览器验证未发生 |

## 质量门槛与最终更新事项

[离线评测合同](../../audits/comment-intelligence-evaluation-contract.md) 已将真实／合成、校准／留出、评论分类／群体观察分开。评测器测试通过不代表获得核心类 precision ≥0.85、群体观察 precision ≥0.90 或真实误归并率 ≤5%。真实样本未送出，不能填入这些成绩；高级自动发现必须继续保留未校准边界。

后续只追加实际发生的浏览器、真实质量及发布证据。没有真实gold与作品留出成绩时，`linggan_ci_rule_release.advanced_release_enabled`保持false；前端仍展示事实统计与未知/未校准说明。尚未发生的事项继续保留NOT VERIFIED。

## 最终集成核对（2026-09-08）

代码位于 `codex/comment-intelligence-v1` 未提交工作树，基线 `61e8c35bb260dc82c761092482752964e16b7cd9`。这一段的自动结果使用本轮实现，不以原包演示和旧项目截图填空。

- 隔离查询/人工动作/边界/日批：整包最终回执记录在 `/tmp/ci-combined-custom-plan.log`；B1–B7单独回执 `/tmp/ci-collision-boundary-proof.log` 为7/7，包含资源清理。最终整包4查询+9人工动作+7边界+18日批，共38项全部通过，容器/卷清理完成。之后仅将相同错误映射提取为函数、收窄搜索提示词到已实现的原文搜索；clippy/API/JS重新通过，无SQL或运行语义变更。
- 模型兼容整包 `/tmp/ci-model-integration-final.log`：50项通过（新日批18、旧Pi执行11、probe限额1、评论资产10、社交材料7、API材料3），其中18与上述日批重叠，不重复相加为独立测试总数。旧Pi回归显式使用0047前schema，升级后旧授权暂停由B6证明。
- `cargo test -p linggan-intelligence --lib --locked`：19项通过；Pi适配器 `node --test test/*.test.mjs`：11项通过；Python离线评测器：8项通过。
- 新API错误/边界3项、已有评论入口与guard4项通过。JS语法检查与mock DOM冒烟通过（默认概览、范围继承、0/null、转义、旧路由），没有浏览器几何/焦点证明。
- 临时隔离API的合成核对：6条评论/3篇作品/6条分析；原声详情含need/story/resonance、1个问题；问题详情同为6条；prepare仅冻结1条目标，model invocation为0。数据有SYNTHETIC / NOT EVIDENCE标记，不能视作本地真实评论或真实供应商调用。
- 全仓 `cargo test --workspace --locked --no-fail-fast`：仅API target失败，135通过、6失败、19忽略；干净基线同一个creator_lifecycle组复现6项失败（10通过、6失败、1忽略）。失败为创作者抽屉/归档旧HTML断言，本轮没有改其代码；日志 `/tmp/ci-workspace-final.log` 与 `/tmp/ci-baseline-api.log`。忽略的PG测试必须看隔离脚本，不能算未执行即通过。
- 全仓clippy被未修改的 `crates/contracts/src/producer_runtime.rs:408` 103行函数/100行上限阻断；`check-rust-boundaries.sh` 报36错误/28警告，主要为既有超长文件/公开项。本轮新增生产Rust文件均≤500行、测试文件≤900行，未增加allow豁免；目标intelligence lib的`--no-deps` clippy通过。既有错误未顺手扩修。
- Chrome访问临时3307被 `ERR_BLOCKED_BY_CLIENT` 阻断。没有绕过该限制，没有本轮真实截图、1440/1920/1280/768和125%/150%走查、浏览器≤2s或Mog前端验收证据。

真实B0/B1/B2与人工留出评测、真实embedding供应商、当前真实库存/混合负载性能、共享库迁移与3000实际API/worker/页面仍未验证。自动观察默认关闭。以上限制是T7/T8的具体未完成项，不用T1–T6源码存在替代。

性能修复过程保留事实：早期单独基准原声498ms通过，但整包复验528ms失败（`/tmp/ci-combined-final.log`），没有改阈值。执行计划显示广域/单作品/可选过滤的不同选择率被复用generic plan；最终在只读事务内使用`plan_cache_mode=force_custom_plan`，保留64MB局部排序/聚合内存上限，不改变服务器全局设置、权限或统计内容。最终同一10万条测试概览P95为500ms、原声394ms；它仍仅证明本机合成数据层条件。

最终源码/测试/迁移/脚本指纹：`1e12afcfae6b99d4fac68ee0e4c0b0df61dade56fda16905df6390c277fbbf9e`（48个变更文件，不含文档）。format、JS语法、mock DOM、目标clippy、diff和项目治理检查通过；共享3000未切换，临时3307与本轮隔离资源均已清理。

### CI-20260907-V1 本机发布回执（2026-09-08）

- Mog本轮授权提交、推送、合并main和刷新3000。代码PR #192已合并为`cce3f60855fae0143452136a9b96daa9ac243e4a`；独立release_review对head`8d35138`给出PASS。集成后评论PG38、模型兼容50（18重叠）、新API3、治理通过。
- 发布时保留main已有0047采集迁移，评论迁移顺延0048并注册local-runtime。共享台账0048 SHA256=`2ed806e50dbc85fa3d6ceaaffcd3041136a303e9532237953355043161d57b88`与文件一致；应用前已创建加密pg_dump并校验archive目录，未重置数据库。
- 首次发布核验：API/worker/media PID为91654/91657/91659，cwd与executable均属于`~/Library/Application Support/Linggan Intelligence/runtime-main`，revision为cce3f60；health数据库/schema READY、scheduler running。后续文档回执提交会同步runtime，PID可能更新，不能把这些点时PID当永久事实。
- 四视角真实GET均成功：1054评论、15作品、1050可研究、0分析、0问题。每次单次HTTP约982–1043ms，不是P95证明，真实规模/并发性能门槛仍未完成。API的高级观察资格false；daily enabled=false；模型调用数发布前后均5，没有新模型外发。
- 已在用户现有Chrome `localhost:3000`标签刷新，确认概览/原声/用户问题/每日观察导航和原声1054条加载。本次解除的是实际3000基本访问验证，不是此前3307阻断的绕过；完整视口/缩放/键盘/写操作、真实语义质量/embedding与Mog验收仍未完成。
- 旧“未提交/未迁移/3000未切换”及0047评论编号是实施阶段历史快照，由本回执替代。此发布不将84项全部验收或T7质量门槛改成通过；Issue #190保持打开承接剩余项。发布证据另见PR #192与Issue #190回执。


## CI-RUN-002 · 本轮用户实测修复验收

授权：Mog 2026-09-08 实测五项反馈及《研究运行可观测层》第一阶段；代码位于 `codex/comment-research-observability`，基线 `ff91f10`。采用现行 LIDS 白底连续研究区、煤黑结构、橙红强调、紧凑原声表格与原生 dialog，未改全局 Token 或一级导航。

| 用户问题 | 本轮结果与验证 |
|---|---|
| 跨页选择丢失 | 浏览器已走通 50 + 50 → 准备弹窗 100 条；分页/排序保留，范围变化提示清空；新增延迟响应测试阻止旧页错选 |
| 不能指定页 | 页码、前后页、页码输入跳转、20/50 条每页，均为已有表格底部轻量控件 |
| 已分析却问题页空白 | 当前有效结果的未归并候选独立展示、含可查原声，人工确认复用问题动作；候选与稳定问题分列 |
| 每日观察没有可读结果 | 默认最近批次，有结果/无信号/失败分开；标签、原声和候选先展示，调用列表后置；高级观察未启用明确披露 |
| 运行与失败是黑盒 | 请求检查器显示实际上下文、脱敏返回、中文校验说明与技术定位；逐条有效结果保留。旧记录不可还原、过期与受限均有真实状态 |
| 上下文配置 | 一个弹窗控制允许纳入的材料和记录期限，保留模型预算原入口；批次冻结设置，保存不启用每日任务 |

自动化：隔离 PostgreSQL query 4、actions 9、read boundaries 7、daily 23 项通过（含合成预览准备）；新增运行场景对混合结果 1 成功/1 无信号/1 失败、字段错误、候选引用、prepare 后变更设置、每日新设置、来源受限、关闭记录、过期、历史无记录进行验证。Rust library 25 项通过；真实 Pi 0.85.1 SDK 对本地合成 HTTP fixture 16 项通过；前端合成 DOM 行为 15 项通过。合成验证不等于真实 DeepSeek 效果评测。

发布边界：以上为实施验证截止时的证据；Mog 已授权提交、推送、合并 main 并刷新 3000，正在执行发布。实际提交、迁移台账和 runtime revision 以 PR 发布回执为准。未对真实评论新增模型调用；Mog 前端业务验收与真实供应商复测仍待发布后执行。

浏览器集成：1440×1000、1280×900、900×900 实查日批结果、问题候选、上下文与设置 dialog；900 宽下文档 scrollWidth=900、请求 dialog 宽720，无横向溢出；1280 宽设置 dialog 宽600。候选“核对原声”落到对应1条，100条准备弹窗经实际UI确认后取消。独立 HTTP 检查 `scripts/verify-comment-runtime-api.mjs` 通过真实 batch/request/context 路由、领域参数、Origin 防护、设置冲突及新设置不改旧批/不开启日程。每日批次另外隔离原声日期筛选，并按批次 analysis_ref/状态取数，后续未执行批次不能冒领既有结果或遮掉旧批成功状态。
