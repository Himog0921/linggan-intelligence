//! 关键词归档中，发现面与逐篇详情是两个有界阶段。
//!
//! 已接纳的发现记录按 Domain 用途投影到规范 Content。详情补采从该 Domain 可用的
//! Content 中挑选缺少合格详情的作品，并把精确内容集合及评论/媒体能力冻结进 WorkOrder。
//! Domain 关系决定用途范围；内容身份和详情资格由统一材料管线承载。
//!
//! 候选、失败预算、输入可执行性和在途判断必须对同一 Domain 与 Content 集合给出一致结果。
//! 每批最多三篇，跑完再要下一批；停止或预算退避不会把未完成详情误报为完整。

use crate::acquisition_chain::{
    AcquisitionChainError, DETAIL_WINDOW_COMMENT_LIMIT, DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
    MaterialDeepeningTarget, request_and_admit_in_transaction_scoped,
};
use crate::execution_input_eligibility::{
    budget_blocks_new_work_predicate, has_executable_locator_predicate,
    unchanged_input_block_predicate,
};
use crate::qualified_detail::qualified_detail_missing_sql;
use crate::step_report::StepOutcome;
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 候选查询的执行资格、未变输入阻断与预算判据由所有入口共用。
///
/// 缺少可执行地址的作品不会入候选；已因相同输入停止过的作品不会循环重排；当前处于
/// 失败预算退避或预算耗尽的范围也不会创建新工单。
fn evidence_side_executable() -> (String, String, String) {
    (
        has_executable_locator_predicate("content.content_external_id"),
        unchanged_input_block_predicate(
            "work_order.target_ref",
            "own_domain",
            "material_content",
            "finding.content_public_ref",
            "content.content_external_id",
        ),
        budget_blocks_new_work_predicate(
            "work_order.target_ref",
            "own_domain",
            "material_content",
            "finding.content_public_ref",
        ),
    )
}

/// 一次补采多少篇。与博主那条路同样克制：一批三篇，跑完再要下一批。
///
/// 一次把上百篇全排进去不是更快，而是把一张工单的失败面放大到上百篇，并且占死一个工位
/// 直到全部跑完。
const KEYWORD_DETAIL_BATCH_SIZE: i64 = 3;

/// 推进一次详情补采的结果。
#[derive(Debug)]
pub enum KeywordDetailAdvance {
    /// 已排入一张工单，覆盖这么多篇。
    Queued { work_order_ref: Uuid, works: usize },
    /// 这一次没有可推进的，原因如实带出去。
    Skipped(&'static str),
}

/// 为一个关键词观察目标推进下一批详情补采。
///
/// 已发现材料的详情完整性不从属于 target lifecycle。baseline 覆盖仍由
/// `keyword_baselines_qualified` 单独判断；但只要已经有一个可读 discovery 仍欠详情，
/// 它就必须保有继续形成 detail work 的路径。
pub async fn advance_keyword_archive_detail(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
) -> Result<KeywordDetailAdvance, AcquisitionChainError> {
    advance_keyword_archive_detail_inner(database, target_ref, None, purpose, requested_by).await
}

pub async fn advance_keyword_archive_detail_for_domain(
    database: &Database,
    target_ref: Uuid,
    domain_ref: Uuid,
    purpose: &str,
    requested_by: &str,
) -> Result<KeywordDetailAdvance, AcquisitionChainError> {
    advance_keyword_archive_detail_inner(
        database,
        target_ref,
        Some(domain_ref),
        purpose,
        requested_by,
    )
    .await
}

async fn advance_keyword_archive_detail_inner(
    database: &Database,
    target_ref: Uuid,
    explicit_domain_ref: Option<Uuid>,
    purpose: &str,
    requested_by: &str,
) -> Result<KeywordDetailAdvance, AcquisitionChainError> {
    if !crate::collection_governance_enabled() {
        return Ok(KeywordDetailAdvance::Skipped(
            "collection_upgrade_recovery_only",
        ));
    }
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_material_domain_usage') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if !schema_ready {
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    // 与其余所有推进路径同一个加锁顺序：先锁目标，再谈授权与容量。
    let target_kind: Option<String> = sqlx::query_scalar(
        "SELECT target_kind FROM collection_observation_target \
         WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(target_kind) = target_kind else {
        transaction.rollback().await?;
        return Err(AcquisitionChainError::UnknownTarget);
    };
    if target_kind != "keyword" {
        transaction.rollback().await?;
        return Ok(KeywordDetailAdvance::Skipped("not_a_keyword_target"));
    }
    let candidate_domains: Vec<Uuid> = sqlx::query_scalar(
        "SELECT domain.domain_ref \
         FROM observation_domain_target relation \
         JOIN observation_domain domain USING(domain_ref) \
         WHERE relation.target_ref=$1 AND domain.status='active' \
           AND (($2::uuid IS NOT NULL AND relation.domain_ref=$2) \
                OR ($2::uuid IS NULL AND relation.role='primary')) \
         ORDER BY domain.created_at,domain.domain_ref LIMIT 2",
    )
    .bind(target_ref)
    .bind(explicit_domain_ref)
    .fetch_all(&mut *transaction)
    .await?;
    let selected_domain = if explicit_domain_ref.is_some() || candidate_domains.len() == 1 {
        candidate_domains.into_iter().next()
    } else {
        None
    };
    let Some(domain_ref) = selected_domain else {
        transaction.rollback().await?;
        return Err(AcquisitionChainError::TargetDomainUnassigned);
    };
    let works_in_evidence =
        next_evidence_detail_batch(&mut transaction, target_ref, domain_ref).await?;
    if works_in_evidence.is_empty() {
        // 「挑不出来」有两种完全不同的原因。候选查询同时排除了「已经取过详情的」和
        // 「已经排在在途工单里的」；只报前者，会把正在跑的批次说成「没有可继续的」，
        // 人看到的是点了没反应，而活其实正在进行。
        let in_flight: bool = sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM collection_work_order work_order \
                 LEFT JOIN collection_work_order_lease lease USING (work_order_ref) \
                 JOIN collection_work_order_domain_usage usage \
                   ON usage.work_order_ref=work_order.work_order_ref AND usage.domain_ref=$2 \
                 WHERE work_order.target_ref=$1 \
                   AND EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                               WHERE scope.work_order_ref=work_order.work_order_ref) \
                   AND (work_order.queue_state IN ('queued','leased') \
                        OR (lease.released_at IS NULL \
                            AND lease.expires_at>scope_001_now())))",
        )
        .bind(target_ref)
        .bind(domain_ref)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.rollback().await?;
        return Ok(KeywordDetailAdvance::Skipped(if in_flight {
            "detail_batch_in_flight"
        } else {
            "no_missing_detail"
        }));
    }

    let works = works_in_evidence.len();
    // 一次打开笔记详情页，顺手读回的是详情 + 前 30 条一级评论 + 2 层回复——与创作者观察
    // 同口径（`DETAIL_WINDOW_*`，ADR-0002 的固定窗口）。评论在这里不是「另一次采集」：
    // 详情页已经打开着了，读回来不多花一次平台访问；此前的 0/0 让关键词的评论永远采不回来，
    // 只在语料库里留下一个个有评论数、没有评论内容的材料。两侧同口径，不按入口各表一套。
    //
    // 媒体三项也一并交给工单：与创作者观察同为 true/true/true。这一单打开的是同一张详情页、
    // 同一篇材料，材料落的是同一张证据表——**同一次打开覆盖的范围不该按入口各表一套**。
    // 三列都写在这里而不是留给读取点临场判断：冻结在作用域行上的授权才是执行权威。
    let material_targets = works_in_evidence
        .into_iter()
        .map(|content_public_ref| MaterialDeepeningTarget {
            content_public_ref,
            comment_limit: DETAIL_WINDOW_COMMENT_LIMIT,
            reply_expand_limit: DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
            acquire_media: true,
            allow_ocr: true,
            allow_asr: true,
        })
        .collect::<Vec<_>>();
    let request = request_and_admit_in_transaction_scoped(
        &mut transaction,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        Some(domain_ref),
        &material_targets,
        None,
        // 详情补采不是巡检，不绑规则版本。
        None,
        false,
    )
    .await?;
    let Some(work_order_ref) = request.work_order_ref else {
        // 准入没放行（没有授权、暂时没有执行资源、或已有同类工作在途）。把它的结论
        // 原样带出去，不压成一句「没有可推进的」。
        transaction.rollback().await?;
        return Ok(KeywordDetailAdvance::Skipped(request.outcome.code()));
    };
    transaction.commit().await?;
    Ok(KeywordDetailAdvance::Queued {
        work_order_ref,
        works,
    })
}

/// 下一批该补详情的作品。候选来自该 Domain 下已接纳的搜索发现，排除已有详情及在途范围。
async fn next_evidence_detail_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    domain_ref: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let (executable, not_stopped, budget_blocked) = evidence_side_executable();
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        // 按**点赞从高到低**取，与跨行业那一侧以及回执文案一致：先补最值得看的那几篇。
        // `DISTINCT ON` 要求排序键以去重键开头，所以去重与排序分两层。
        "SELECT content_public_ref FROM ( \
           SELECT DISTINCT ON (finding.content_public_ref) finding.content_public_ref, \
                  finding.like_count,package.accepted_at \
         FROM collection_work_order work_order \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
         JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
         JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
         JOIN linggan_material_discovery_finding finding USING(package_ref) \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
         JOIN linggan_material_domain_usage usage \
           ON usage.content_public_ref=content.public_ref AND usage.domain_ref=$2 \
         JOIN linggan_runtime_record_disposition disposition \
           ON disposition.package_ref=finding.package_ref \
          AND disposition.record_ordinal=finding.record_ordinal \
         WHERE work_order.target_ref=$1 \
           AND finding.discovery_kind='discovery_search' \
           AND receipt.material_admission='ACCEPTED' \
           AND disposition.disposition='accepted_for_library_discovery' \
           AND {missing_detail} \
           AND NOT EXISTS ( \
             SELECT 1 FROM collection_work_order live_order \
             JOIN collection_work_order_material_target live_scope USING (work_order_ref) \
             LEFT JOIN collection_work_order_lease live_lease USING (work_order_ref) \
             JOIN collection_work_order_domain_usage live_usage \
               ON live_usage.work_order_ref=live_order.work_order_ref AND live_usage.domain_ref=$2 \
             WHERE live_order.target_ref=$1 \
               AND live_scope.content_public_ref=finding.content_public_ref \
               AND (live_order.queue_state IN ('queued','leased') \
                    OR (live_lease.released_at IS NULL \
                        AND live_lease.expires_at>scope_001_now()))) \
           AND {executable} \
           AND NOT {not_stopped} \
           AND NOT {budget_blocked} \
           ORDER BY finding.content_public_ref,package.accepted_at \
         ) candidate \
         ORDER BY candidate.like_count DESC NULLS LAST,candidate.accepted_at, \
                  candidate.content_public_ref \
         LIMIT $3",
        missing_detail = qualified_detail_missing_sql!("finding.content_public_ref"),
    )))
    .bind(target_ref)
    .bind(domain_ref)
    .bind(KEYWORD_DETAIL_BATCH_SIZE)
    .fetch_all(&mut **transaction)
    .await
}

/// 一批关键词各自**还有没有作品等着补详情**。
///
/// 列表页一次要判断很多行，逐行查会变成 N+1。返回集合里出现的就是「还差详情」；没出现
/// 的是「都补齐了或压根没有可补的」，读不到则以 `Err` 浮上来由调用方如实呈现。
pub async fn keyword_targets_pending_detail(
    database: &Database,
    target_refs: &[Uuid],
) -> Result<std::collections::HashSet<Uuid>, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    if !keyword_detail_schema_is_ready(database.pool()).await? {
        return Err(sqlx::Error::Protocol(
            "keyword detail completeness schema is not ready".to_owned(),
        ));
    }
    let (executable, not_stopped, budget_blocked) = evidence_side_executable();
    let rows: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT requested.target_ref \
         FROM unnest($1::uuid[]) AS requested(target_ref) \
         JOIN observation_domain_target relation \
           ON relation.target_ref=requested.target_ref AND relation.role='primary' \
         JOIN observation_domain domain ON domain.domain_ref=relation.domain_ref \
         WHERE domain.status='active' AND EXISTS ( \
           SELECT 1 FROM collection_work_order work_order \
           JOIN collection_work_order_lease lease USING(work_order_ref) \
           JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
           JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
           JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
           JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
           JOIN linggan_material_discovery_finding finding USING(package_ref) \
           JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
           JOIN linggan_material_domain_usage usage \
             ON usage.content_public_ref=content.public_ref AND usage.domain_ref=relation.domain_ref \
           JOIN linggan_runtime_record_disposition disposition \
             ON disposition.package_ref=finding.package_ref \
            AND disposition.record_ordinal=finding.record_ordinal \
           WHERE work_order.target_ref=requested.target_ref \
             AND finding.discovery_kind='discovery_search' \
             AND receipt.material_admission='ACCEPTED' \
             AND disposition.disposition='accepted_for_library_discovery' \
             AND {missing_detail} \
             AND NOT EXISTS ( \
               SELECT 1 FROM collection_work_order live_order \
               JOIN collection_work_order_material_target live_scope USING(work_order_ref) \
               LEFT JOIN collection_work_order_lease live_lease USING(work_order_ref) \
               WHERE live_order.target_ref=work_order.target_ref \
                 AND live_scope.content_public_ref=finding.content_public_ref \
                 AND (live_order.queue_state IN ('queued','leased') \
                      OR (live_lease.released_at IS NULL \
                          AND live_lease.expires_at>scope_001_now()))) \
             AND {executable} AND NOT {not_stopped} AND NOT {budget_blocked} \
         )",
        missing_detail = qualified_detail_missing_sql!("finding.content_public_ref"),
    )))
    .bind(target_refs)
    .fetch_all(database.pool())
    .await?;
    Ok(rows.into_iter().collect())
}

async fn keyword_detail_schema_is_ready(pool: &sqlx::PgPool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass('linggan_material_domain_usage') IS NOT NULL \
             AND to_regclass('linggan_material_discovery_finding') IS NOT NULL \
             AND to_regclass('linggan_material_content_detail') IS NOT NULL",
    )
    .fetch_one(pool)
    .await
}

pub(crate) async fn keyword_detail_schema_is_ready_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass('linggan_material_domain_usage') IS NOT NULL \
             AND to_regclass('linggan_material_discovery_finding') IS NOT NULL \
             AND to_regclass('linggan_material_content_detail') IS NOT NULL",
    )
    .fetch_one(&mut **transaction)
    .await
}

/// A single-target counterpart used by the patrol admission gate.
pub(crate) async fn keyword_target_has_pending_detail_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    let (executable, not_stopped, budget_blocked) = evidence_side_executable();
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS ( \
           SELECT 1 FROM collection_work_order work_order \
           JOIN collection_work_order_lease lease USING(work_order_ref) \
           JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
           JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
           JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
           JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
           JOIN linggan_material_discovery_finding finding USING(package_ref) \
           JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
           JOIN linggan_material_domain_usage usage ON usage.content_public_ref=content.public_ref \
           JOIN observation_domain_target relation \
             ON relation.domain_ref=usage.domain_ref AND relation.target_ref=work_order.target_ref \
            AND relation.role='primary' \
           JOIN observation_domain domain ON domain.domain_ref=relation.domain_ref AND domain.status='active' \
           JOIN linggan_runtime_record_disposition disposition \
             ON disposition.package_ref=finding.package_ref \
            AND disposition.record_ordinal=finding.record_ordinal \
           WHERE work_order.target_ref=$1 \
             AND finding.discovery_kind='discovery_search' \
             AND receipt.material_admission='ACCEPTED' \
             AND disposition.disposition='accepted_for_library_discovery' \
             AND {missing_detail} \
             AND NOT EXISTS ( \
               SELECT 1 FROM collection_work_order live_order \
               JOIN collection_work_order_material_target live_scope USING(work_order_ref) \
               LEFT JOIN collection_work_order_lease live_lease USING(work_order_ref) \
               WHERE live_order.target_ref=work_order.target_ref \
                 AND live_scope.content_public_ref=finding.content_public_ref \
                 AND (live_order.queue_state IN ('queued','leased') \
                      OR (live_lease.released_at IS NULL \
                          AND live_lease.expires_at>scope_001_now()))) \
             AND {executable} AND NOT {not_stopped} AND NOT {budget_blocked} \
         )",
        missing_detail = qualified_detail_missing_sql!("finding.content_public_ref"),
    )))
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await
}

/// 每轮 tick 自动推进一批关键词的详情补采。
///
/// 建档是两段，第二段不该等人来点。博主那条路早就是自动的（`run_progressive_archives`），
/// 关键词此前只有手点这一个入口：第一段跑完之后界面上多出一个「补采缺口」按钮，人不点
/// 就永远停在一堆链接上——而这个词看上去已经"建好档了"。
///
/// 一轮只推进有限几个目标，每个目标一批（三篇）。这不是为了省事：一次把所有目标的所有
/// 缺口全排进队列，会让一个刚建完档的词独占工位几十分钟，别的目标全在后面等。
const KEYWORD_DETAIL_TARGETS_PER_TICK: usize = 5;

#[derive(Debug, Default)]
pub struct KeywordDetailTickSummary {
    /// 这一轮排出了详情补采的目标，以及各自排了几篇。
    pub queued: Vec<(Uuid, usize)>,
    /// 这一轮没推进的目标与如实原因。
    pub skipped: Vec<(Uuid, String)>,
}

impl KeywordDetailTickSummary {
    /// 这一步在账本上该怎么记。这一步不数「考虑过多少」——留空，不写 0 冒充。
    pub fn step_outcome(&self) -> StepOutcome {
        StepOutcome::Ok {
            considered: None,
            produced: Some(i64::try_from(self.queued.len()).unwrap_or(i64::MAX)),
            skipped: Some(i64::try_from(self.skipped.len()).unwrap_or(i64::MAX)),
        }
    }
}

pub async fn run_keyword_archive_details(
    database: &Database,
    purpose: &str,
) -> Result<KeywordDetailTickSummary, AcquisitionChainError> {
    let mut summary = KeywordDetailTickSummary::default();
    let schema_ready = keyword_detail_schema_is_ready(database.pool()).await?;
    if !schema_ready {
        // 与媒体投影、渐进档案两步同一句话：**自己的表不在 = 没轮到**，不是「没有欠详情的
        // 词」。此前这里静默返回空汇总，两种情形在日志与账本上长得一模一样。
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    // 只找仍欠详情的关键词。baseline 的覆盖事实决定“这一轮搜索是否完整”，不能冻结
    // 已经发现却仍不完整的材料；否则 target 进入 monitoring 后会永久失去补详情路径。
    let due: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT target.target_ref,relation.domain_ref \
         FROM collection_observation_target target \
         JOIN observation_domain_target relation ON relation.target_ref=target.target_ref \
         JOIN observation_domain domain ON domain.domain_ref=relation.domain_ref \
         WHERE target.target_kind='keyword' AND target.lifecycle_state <> 'dismissed' \
           AND relation.role='primary' AND domain.status='active' \
         ORDER BY target.last_scheduler_considered_at NULLS FIRST,target.target_ref,relation.domain_ref",
    )
    .fetch_all(database.pool())
    .await?;
    if due.is_empty() {
        return Ok(summary);
    }
    if !crate::collection_governance_enabled() {
        summary
            .skipped
            .extend(due.into_iter().map(|(target_ref, _)| {
                (target_ref, "collection_upgrade_recovery_only".to_owned())
            }));
        return Ok(summary);
    }
    let due_target_refs = due
        .iter()
        .map(|(target_ref, _)| *target_ref)
        .collect::<Vec<_>>();
    let pending = keyword_targets_pending_detail(database, &due_target_refs).await?;
    for (target_ref, domain_ref) in due
        .into_iter()
        .filter(|(target, _)| pending.contains(target))
        .take(KEYWORD_DETAIL_TARGETS_PER_TICK)
    {
        // 与巡检调度共用同一条公平性事实：推进过的排到后面去，免得前几个词把每一轮都占满。
        sqlx::query(
            "UPDATE collection_observation_target \
             SET last_scheduler_considered_at=scope_001_now() WHERE target_ref=$1",
        )
        .bind(target_ref)
        .execute(database.pool())
        .await?;
        match advance_keyword_archive_detail_for_domain(
            database, target_ref, domain_ref, purpose, "agent",
        )
        .await?
        {
            KeywordDetailAdvance::Queued { works, .. } => summary.queued.push((target_ref, works)),
            KeywordDetailAdvance::Skipped(reason) => {
                summary.skipped.push((target_ref, reason.to_owned()));
            }
        }
    }
    Ok(summary)
}
