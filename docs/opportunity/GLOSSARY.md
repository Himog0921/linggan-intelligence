# GLOSSARY（术语与命名）

复制自实施手册 §5（2026-11-12 版本）。执行中只增不改；代码、接口、数据库、文档统一使用下列标识，不得自创同义词。

| 中文 | 代码标识 | 取值或类型 |
| --- | --- | --- |
| 用户问题 | `problem` | 实体 |
| 问题层级 | `level` | 1、2、3 |
| 问题谱系 | `problem_lineage` | 操作：`merge`、`split`、`rename`、`reparent` |
| 分类版本 | `taxonomy_version` | 整数，按领域递增 |
| 问题归属（评论提及） | `comment_mention` | 实体 |
| 提及人数 | `mentioners` | 去重评论者数 |
| 本期提及数 | `k` | 本期内各评论者对该问题的首次提及评论数 |
| 本期评论数 | `n` | 本期领域内有效评论数 |
| 提及率 | `rate_per_1k` | `k / n × 1000`；`n = 0` 时为 `null` |
| 置信区间 | `ci_low`、`ci_high` | 存储为 0–1 的比例；接口层再换算为每千 |
| 基线 | `baseline_k`、`baseline_n` | 前 B 期合并 |
| 证据等级 | `evidence_tier` | `candidate`、`established`、`significant` |
| 作品回答判定 | `work_problem_judgment` | 标签：`answers`、`mentions`、`none` |
| 判定来源 | `source` | `ai`、`human` |
| 判定状态 | `judgment_status` | `verified`、`ai_suggested` |
| 供给状态 | `supply_state` | `not_evaluated`、`awaiting_understanding`、`insufficient_coverage`、`covered`、`suspected_gap` |
| 供给侧覆盖率 | `coverage` | `collected / found` |
| 作者问题集中度 | `creator_problem_density` | 实体 |
| 样本不足 | `insufficient_sample` | 质量标记 |
| 重要性未校准 | `not_calibrated` | 质量标记 |
| 质量门 | `quality_gate` | 实体 |
| 研究集合 | `research_collection` | 实体 |
| 集合快照 | `collection_snapshot` | 实体，冻结后不可变 |
| 墓碑 | `tombstone` | 撤回内容的占位表示 |
| 口径定义 | `metric_definition` | 实体 |
| 数据截至 | `as_of` | UTC 时间戳 |
| 首次采集时间 | `observed_at` | UTC 时间戳 |
| 发布时间 | `published_at` | UTC 时间戳 |

> 状态: 权威当前
> 最后核对: 2026-11-12
> 适用范围: 机会卡实施中的术语、代码标识与数据库命名。
> 事实来源: 实施手册 v1.3 §5（2026-11-12）、DECISIONS D-028 至 D-030，以及现有代码/迁移命名。
> 冲突时以谁为准: 现有代码与数据库实际命名；本表只约束新实现，不追溯改名。

## 勘察备注（不修改术语表本身）

- 现有数据库中的 `linggan_material_comment.observed_at` 是“该次观察时间”，不是手册定义的“首次采集时间”；已裁定：机会卡领域使用 `first_observed_at`，见 `DECISIONS.md` D-007。

## 2026-11-12 追加（依据 DECISIONS D-004 至 D-020）

原表不改。下列条目为追加；与原表冲突处以本节为准。

| 中文 | 代码标识 | 取值或类型 | 依据 |
| --- | --- | --- | --- |
| 首次采集时间 | `first_observed_at` | UTC 时间戳。原表中的 `observed_at`（首次采集时间）在机会卡领域一律改用此名 | D-007 |
| 首次采集时间为回填 | `first_observed_backfilled` | 布尔 | D-007 |
| 统计周期基准 | `period_basis` | v1：`work_published_cohort`；v2 预留：`comment_published` | D-008 |
| 作品发布时间（周期锚点） | `work_published_at` | UTC 时间戳，来自 `linggan_material_content_detail.published_at` | D-008 |
| 评论身份 | `content_ref` + `comment_external_id` | uuid + text；接口中为 `{contentRef, commentExternalId}` | D-011 |
| 观察溯源引用 | `source_material_ref` | uuid，只用于溯源，不参与去重 | D-011 |
| 评论者哈希版本 | `commenter_digest_version` | v1：`commenter-hmac-sha256-v1` | D-013 |
| 提及排除原因（追加） | `excluded_reason` | 追加 `commenter_unknown`（保留在 n，不计入 k） | D-013 |
| 资格排除原因 | `eligibility_exclusion` | `period_anchor_missing`、`work_author_unknown`、`creator_self`、`restricted`、`observed_at_unparseable`、`not_in_domain_primary` | D-012 |
| 质量标记（追加） | `flags` | 追加 `period_anchor_missing`、`author_unknown_share_high`、`engagement_reference_missing` | D-008、D-010、D-019 |
| 特性开关 | `LINGGAN_OPPORTUNITY_CARD` | 仅 `enabled` 为开启 | D-006 |
| 物理表名前缀 | `linggan_opportunity_` | 例如 `linggan_opportunity_problem` | D-004 |
| 问题来源引用 | `source_ref` / `source_definition_ref` | 0177 的 `problem_ref` / `definition_ref` | D-014 |

## 2026-11-12 追加（依据 DECISIONS D-028 至 D-030、D-034）

| 中文 | 代码标识 | 取值或类型 | 依据 |
| --- | --- | --- | --- |
| 统计周期基准（追加取值） | `period_basis` | 目标口径 `comment_published`；过渡口径 `work_published_cohort` | D-028 |
| 评论平台发布时间 | `comment_published_at` | UTC 时间戳，来自 `linggan_material_comment_platform_time.published_at` | D-028 |
| 评论平台时间表 | `linggan_material_comment_platform_time` | 只追加，以评论身份 + 精度为键 | D-028、D-034 |
| 时间精度 | `precision` | `millisecond`（epoch）；`day`（展示日期，存上海当地零点） | D-034 |
| 时间来源 | `source_kind` | `epoch`、`display_full_date`、`display_month_day`、`display_relative` | D-034 |
| 日精度采集时刻 | `collectedAt` | 记录自身 13 位毫秒；相对时间以此为推算基准 | D-034 |
| 资格排除原因（追加） | `comment_time_unknown` | `comment_published` 口径下没有平台时间 | D-028 |
| 影子口径 | `shadow` | 同步计算并保存、不展示的口径 | D-028 |
| 采集目标 | `collection_target_per_period` | 每领域每周 700 条有效评论 | D-029 |
| 质量标记（追加） | `flags` | `collection_below_target`、`collection_protocol_changed` | D-029、D-030 |
| 采集协议 | `collection_protocol` | 排序方式、单次上限、触发来源；须有版本 | D-030 |
