//! 关键词建档的第二段：把链接换成详情。
//!
//! 建档的第一段只拿得到列表面能看到的东西——标题、封面、点赞、以及一条带签名的链接。
//! 那还不是这个词的面貌：正文、评论、发布时间都在详情页里。**博主的渐进式建档早就是
//! 两段（先目录、后逐篇），关键词此前只有第一段**，于是建完档的词停在一堆链接上。
//!
//! 这一段与博主那段的形状相同，落点不同：博主的作品住证据库，关键词的参照物住跨行业
//! 语料（`0044` 的隔离），所以作用域表也是分开的两张（`0074`）。
//!
//! **去重发生在进语料库那一刻，不在这里。** 建档那一轮采回多少条就留多少条（包不去重，
//! 采集覆盖度要如实），但落库是 upsert：`cross_industry_sample` 一篇一行。所以从样本表
//! 里挑出来的候选天然已经去重，同一篇不会被排两次详情。这正是 Mog 定的规则——「建档的
//! 时候不用去重，100 条都采了；进语料库一定要去重，去重之后的链接才推进详情采集」。
//!
//! **两侧同口径的详情读。**一次「补详情」打开的就是那张详情页，所以两侧都顺手带回前 30 条
//! 一级评论与 2 层回复（`DETAIL_WINDOW_*`，ADR-0002 的固定窗口），额度冻结在各自的作用域行上
//! （证据侧 `0038` 起就有，跨行业侧 `0087` 补上）。
//!
//! **媒体只跟证据侧同口径，跨行业侧跟不了。**「一次打开就顺手做完」对评论成立，因为它就在
//! 已经打开的那张详情页上；对媒体不成立——那是另下字节、另起 OCR 与转录的活。所以 `acquire_media`
//! 是单独一列，取值跟着**这一侧能不能兑现**走，而不是跟着「另一个入口开了没有」走。
//!
//! 证据侧兑现得了：材料住 `linggan_material_content`，有 `linggan_material_media_origin`
//! 这条落地链路，创作者观察一直在下字节。2026-09-16 起关键词观察与它同口径。此前这里是
//! `false`，于是同一个领域里，博主那条路采回了封面与视频，关键词这条路一篇都没有——**不是
//! 下载失败，是根本没下**，而两边在界面上长得一样。
//!
//! 跨行业侧兑现不了：`0044` 的隔离把跨行业样本挡在 `linggan_material_content` 之外，
//! `linggan_material_media_origin.content_public_ref` 又是指向它的非空外键，所以那边没有媒体表、
//! 没有摄取、没有字节。**授权列因此不存在，而不是存在但填 false**：写一个没有任何东西能执行的
//! `true` 是在工单上记一条做不到的承诺，写 `false` 则会被读成「这一单决定不下字节」这样一个
//! 从未做过的决定。两种都不如实。
//!
//! **两种领域各走各的一侧。** 关键词不只有外部领域那一种：本领域的关键词（比如 ADHD 底下
//! 的「a娃」）采回来的材料按 `0044` 的隔离写进**证据侧**，跨行业样本表里一条都没有。
//! 此前这里只查 `cross_industry_sample`，于是本领域关键词永远「没有待补详情的」，
//! 补详情的入口对它们根本不出现——活做了一半，而界面上看不出少了什么。
//! 两侧的候选判据同形（有链接、还没取过详情、没排在在途工单上），落点不同：
//! 证据侧用 `collection_work_order_material_target`，跨行业用 `0074` 那张。

use crate::acquisition_chain::{
    AcquisitionChainError, CrossIndustryDeepeningTarget, DETAIL_WINDOW_COMMENT_LIMIT,
    DETAIL_WINDOW_REPLY_EXPAND_LIMIT, MaterialDeepeningTarget,
    request_and_admit_in_transaction_scoped,
};
use crate::execution_input_eligibility::{
    budget_blocks_new_work_predicate, has_executable_locator_predicate,
    unchanged_input_block_predicate,
};
use crate::step_report::StepOutcome;
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 候选查询里那三条「能不能执行」的条件，本模块四个入口共用。
///
/// **前置，而不是排队后再停。** 缺执行地址的作品从来不该进候选：排进去只会得到一次
/// 「派不出去」——那一轮里它占用了一张工单、一张租约和一次调度，却一次页面都没打开。此前
/// 这里只看「有没有链接」（`source_url IS NOT NULL`），而链接可能没有签名令牌，插件打不开；
/// 判据放宽成「有链接」，实际需要的是「有能打开的链接」。
///
/// 第二条是「已经因此停过、且输入没变」。只加第一条会让停过的作品每一轮重新排队、重新被停，
/// 停止本身变成循环；只加第二条则拦不住从没排过队但同样没有地址的新作品。两条都要。
///
/// 第三条是「这个需求范围的页面失败预算已经不允许再排新工作」——用尽（停止）或还在退避里
/// （等一下）都算。少了它，同一个缺口每被新建一张工单就重新起一轮：预算记在台账的当前资格行
/// 上，而这里正是「要不要为此再开一张工单」的那个决定点。
///
/// `cross_industry_ready` 取 `true`：地址可能在跨行业那一侧（`0044`）兜底，候选必须与派发
/// 用同一把尺子。派发时 `execution_source_url_for_task` 先证据侧、再跨行业兜底；候选这里
/// 取 `false` 就会比它更严——一篇本侧没有签名地址、兜底侧有地址的作品会被候选**静默剔除**：
/// 不排队，也不以任何理由出现在任何地方。`unchanged_input_block_predicate` 同理：那一侧取
/// `false` 时「输入没变」只看证据侧，而停下来的判据看两侧，于是一篇因样本侧输入未变而停过的
/// 作品会每轮重新排队、重新被停——正是它要挡的那个循环。
///
/// 代价是这三条判据会把一个 `cross_industry_sample` 的引用塞进 SQL，哪怕外层查询本来不碰
/// 那张表（`next_evidence_detail_batch` 的外层查询只连证据侧）。它们安全**不是**因为外层查询
/// 本来就提到那张表——此前这里就是这么写的，而它对那个入口并不成立——而是因为走到这些入口
/// 之前都先验过 `sample_facts_schema_ready_sql!()` 与
/// `collection_work_order_cross_industry_target`（例如 `advance_keyword_archive_detail`），
/// 不齐就报 `SchemaUnavailable`，根本不执行。改动这些入口的人要保住那道前置检查：漏掉它，
/// 这里得到的不是「更保守的候选集」，而是一次 42P01。
fn evidence_side_executable() -> (String, String, String) {
    (
        has_executable_locator_predicate("content.content_external_id", true),
        unchanged_input_block_predicate(
            "work_order.target_ref",
            "own_domain",
            "material_content",
            "finding.content_public_ref",
            "content.content_external_id",
            true,
        ),
        budget_blocks_new_work_predicate(
            "work_order.target_ref",
            "own_domain",
            "material_content",
            "finding.content_public_ref",
        ),
    )
}

fn sample_side_executable(target_ref_expression: &str) -> (String, String, String) {
    (
        has_executable_locator_predicate("sample.content_external_id", true),
        unchanged_input_block_predicate(
            target_ref_expression,
            "cross_industry",
            "cross_industry_sample",
            "sample.sample_ref",
            "sample.content_external_id",
            true,
        ),
        budget_blocks_new_work_predicate(
            target_ref_expression,
            "cross_industry",
            "cross_industry_sample",
            "sample.sample_ref",
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
    if !crate::collection_governance_enabled() {
        return Ok(KeywordDetailAdvance::Skipped(
            "collection_upgrade_recovery_only",
        ));
    }
    let schema_ready: bool = sqlx::query_scalar(concat!(
        "SELECT ",
        sample_facts_schema_ready_sql!(),
        " AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    ))
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    // 与其余所有推进路径同一个加锁顺序：先锁目标，再谈授权与容量。
    let target: Option<(String, Option<Uuid>, Option<bool>)> = sqlx::query_as(
        "SELECT target.target_kind,target.domain_ref,domain.is_own_domain \
         FROM collection_observation_target target \
         LEFT JOIN observation_domain domain USING(domain_ref) \
         WHERE target.target_ref=$1 FOR UPDATE OF target",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((target_kind, domain_ref, is_own_domain)) = target else {
        transaction.rollback().await?;
        return Err(AcquisitionChainError::UnknownTarget);
    };
    if target_kind != "keyword" {
        transaction.rollback().await?;
        return Ok(KeywordDetailAdvance::Skipped("not_a_keyword_target"));
    }
    if domain_ref.is_none() {
        transaction.rollback().await?;
        return Err(AcquisitionChainError::TargetDomainUnassigned);
    }
    // 本领域的材料住证据侧，外部领域的住跨行业语料（`0044` 的隔离）。候选判据同形，
    // 取的是两张不同的表。
    let home_domain = is_own_domain == Some(true);
    let (samples, works_in_evidence) = if home_domain {
        (
            Vec::new(),
            next_evidence_detail_batch(&mut transaction, target_ref).await?,
        )
    } else {
        (
            next_detail_batch(&mut transaction, target_ref).await?,
            Vec::new(),
        )
    };
    if samples.is_empty() && works_in_evidence.is_empty() {
        // 「挑不出来」有两种完全不同的原因。候选查询同时排除了「已经取过详情的」和
        // 「已经排在在途工单里的」；只报前者，会把正在跑的批次说成「没有可继续的」，
        // 人看到的是点了没反应，而活其实正在进行。
        let in_flight: bool = sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM collection_work_order work_order \
                 LEFT JOIN collection_work_order_lease lease USING (work_order_ref) \
                 WHERE work_order.target_ref=$1 \
                   AND (EXISTS (SELECT 1 FROM collection_work_order_cross_industry_target scope \
                                WHERE scope.work_order_ref=work_order.work_order_ref) \
                        OR EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                                   WHERE scope.work_order_ref=work_order.work_order_ref)) \
                   AND (work_order.queue_state IN ('queued','leased') \
                        OR (lease.released_at IS NULL \
                            AND lease.expires_at>scope_001_now())))",
        )
        .bind(target_ref)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.rollback().await?;
        return Ok(KeywordDetailAdvance::Skipped(if in_flight {
            "detail_batch_in_flight"
        } else {
            "no_missing_detail"
        }));
    }

    let works = samples.len() + works_in_evidence.len();
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
    let cross_industry_targets = samples
        .into_iter()
        .map(|sample_ref| CrossIndustryDeepeningTarget {
            sample_ref,
            comment_limit: DETAIL_WINDOW_COMMENT_LIMIT,
            reply_expand_limit: DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
        })
        .collect::<Vec<_>>();
    let request = request_and_admit_in_transaction_scoped(
        &mut transaction,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        &material_targets,
        &cross_industry_targets,
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

/// 下一批该补详情的本领域作品。
///
/// 与跨行业那一侧同形，只是材料住在证据侧：候选来自这个目标名下已接纳的搜索发现，
/// 排除掉已经有详情的、以及已经排在某张还活着的工单上的。
///
/// 「有没有详情」直接看 `linggan_material_content_detail`——证据侧本来就有这张表，不必像
/// 跨行业那侧那样从运行时任务反推。
async fn next_evidence_detail_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
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
         LIMIT $2",
        missing_detail = qualified_detail_missing_sql!("finding.content_public_ref"),
    )))
    .bind(target_ref)
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
    let schema_ready: bool = sqlx::query_scalar(concat!(
        "SELECT ",
        sample_facts_schema_ready_sql!(),
        " AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    ))
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        // Empty means “every known detail is complete”; a missing projection cannot truthfully
        // mean that.  The list and patrol gate turn this error into their explicit Unknown state.
        return Err(sqlx::Error::Protocol(
            "keyword detail completeness schema is not ready".to_owned(),
        ));
    }
    let (executable, not_stopped, budget_blocked) = evidence_side_executable();
    let (sample_executable, sample_not_stopped, sample_budget_blocked) =
        sample_side_executable("seen_order.target_ref");
    let rows: Vec<Uuid> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        // 跨行业那一侧。哪些样本算这个目标的，按**观察记录**算而不是样本行上的
        // `target_ref`——那一列只在第一次插入时写定，同一篇被另一个关键词先看到，
        // 这个目标就永远不会给它补详情。
        "SELECT requested.target_ref FROM unnest($1::uuid[]) AS requested(target_ref) \
         WHERE EXISTS (SELECT 1 FROM cross_industry_sample sample \
         JOIN cross_industry_sample_observation seen \
           ON seen.sample_ref=sample.sample_ref \
         JOIN linggan_runtime_capture_package seen_package \
           ON seen_package.package_ref=seen.package_ref \
         JOIN collection_work_order_lease_task seen_lease_task \
           ON seen_lease_task.task_id=seen_package.task_id \
         JOIN collection_work_order_lease seen_lease USING(lease_ref) \
         JOIN collection_work_order seen_order USING(work_order_ref) \
         WHERE seen_order.target_ref=requested.target_ref AND {pending} \
           AND {sample_executable} \
           AND NOT {sample_not_stopped} \
           AND NOT {sample_budget_blocked}) \
         OR EXISTS (SELECT 1 \
         FROM collection_work_order work_order \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
         JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
         JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
         JOIN linggan_material_discovery_finding finding USING(package_ref) \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
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
             JOIN collection_work_order_material_target live_scope USING (work_order_ref) \
             LEFT JOIN collection_work_order_lease live_lease USING (work_order_ref) \
             WHERE live_order.target_ref=work_order.target_ref \
               AND live_scope.content_public_ref=finding.content_public_ref \
               AND (live_order.queue_state IN ('queued','leased') \
                    OR (live_lease.released_at IS NULL \
                        AND live_lease.expires_at>scope_001_now()))) \
           AND {executable} \
           AND NOT {not_stopped} \
           AND NOT {budget_blocked})",
        missing_detail = qualified_detail_missing_sql!("finding.content_public_ref"),
        pending = pending_detail_sql!("seen_order.target_ref"),
    )))
    .bind(target_refs)
    .fetch_all(database.pool())
    .await?;
    Ok(rows.into_iter().collect())
}

/// Return whether the tables that make keyword-detail completeness readable exist in this
/// database.  A missing projection is not an empty pending set: callers that decide whether a
/// keyword may enter patrol must reject the unknown state rather than treating it as complete.
pub(crate) async fn keyword_detail_schema_is_ready_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "SELECT ",
        sample_facts_schema_ready_sql!(),
        " AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    ))
    .fetch_one(&mut **transaction)
    .await
}

/// A single-target counterpart used only by the patrol admission gate.  It deliberately keeps
/// the same two material sides as the list read: cross-industry samples and own-domain evidence.
/// If either side still has a readable candidate, the archive is incomplete.
pub(crate) async fn keyword_target_has_pending_detail_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    let (executable, not_stopped, budget_blocked) = evidence_side_executable();
    let (sample_executable, sample_not_stopped, sample_budget_blocked) =
        sample_side_executable("seen_order.target_ref");
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT EXISTS ( \
           SELECT 1 FROM cross_industry_sample sample \
           JOIN cross_industry_sample_observation seen \
             ON seen.sample_ref=sample.sample_ref \
           JOIN linggan_runtime_capture_package seen_package \
             ON seen_package.package_ref=seen.package_ref \
           JOIN collection_work_order_lease_task seen_lease_task \
             ON seen_lease_task.task_id=seen_package.task_id \
           JOIN collection_work_order_lease seen_lease USING(lease_ref) \
           JOIN collection_work_order seen_order USING(work_order_ref) \
           WHERE seen_order.target_ref=$1 AND {pending} \
             AND {sample_executable} \
             AND NOT {sample_not_stopped} \
             AND NOT {sample_budget_blocked} \
           UNION \
           SELECT 1 FROM collection_work_order work_order \
           JOIN collection_work_order_lease lease USING(work_order_ref) \
           JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
           JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
           JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
           JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
           JOIN linggan_material_discovery_finding finding USING(package_ref) \
           JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
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
               WHERE live_order.target_ref=work_order.target_ref \
                 AND live_scope.content_public_ref=finding.content_public_ref \
                 AND (live_order.queue_state IN ('queued','leased') \
                      OR (live_lease.released_at IS NULL \
                          AND live_lease.expires_at>scope_001_now()))) \
             AND {executable} \
             AND NOT {not_stopped} \
             AND NOT {budget_blocked} \
         )",
        missing_detail = qualified_detail_missing_sql!("finding.content_public_ref"),
        pending = pending_detail_sql!("seen_order.target_ref"),
    )))
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await
}

/// 「这一篇还等着补详情」这条判据本身。单篇挑选与列表页批量共用它，免得同一件事在两处
/// 各写一遍、日后各自漂移。`sample` 是外层给的表别名。
macro_rules! pending_detail_sql {
    ($target:literal) => {
        concat!(
            "sample.source_url IS NOT NULL AND NOT ",
            // 已经取到的不再排队。此前这里问的是运行任务：状态在
            // `('completed','unavailable','blocked')` 里就算做过。后两个是「试过没成功」——
            // 与取到了相反，于是**失败一次的笔记永久从待补清单里消失**；而 `completed`
            // 也不保证材料进来（整包被隔离时任务照样完成）。现在只看 `0079` 那条材料事实。
            sample_detail_obtained_sql!(),
            " AND NOT EXISTS ( \
                 SELECT 1 FROM collection_work_order live_order \
                 JOIN collection_work_order_cross_industry_target live_scope \
                   USING (work_order_ref) \
                 LEFT JOIN collection_work_order_lease live_lease USING (work_order_ref) \
                 WHERE live_order.target_ref=",
            $target,
            " AND live_scope.sample_ref=sample.sample_ref \
                   AND (live_order.queue_state IN ('queued','leased') \
                        OR (live_lease.released_at IS NULL \
                            AND live_lease.expires_at>scope_001_now())))"
        )
    };
}

use crate::cross_industry_sample_facts::{
    sample_detail_obtained_sql, sample_facts_schema_ready_sql, sample_observed_by_target_sql,
};
use crate::qualified_detail::qualified_detail_missing_sql;
use pending_detail_sql;

/// 下一批该补详情的样本。
///
/// 四个条件缺一不可：
/// - 它是这个目标带回来的（`target_ref`），不是同领域别的词带回来的；
/// - 它有平台返回的签名链接——没有链接就打不开详情页，排进去只会白跑一趟；
/// - 还没有取到过详情；
/// - 没有排在某张还活着的工单上。
///
/// 「取到过详情没有」由**材料事实**回答：`cross_industry_sample_detail` 里有没有这篇的行
/// （`0079`）。此前它由运行任务的状态回答，而「任务跑完」与「材料进来」是两件事：整包被
/// 隔离时一个字段都没写进样本行，租约任务照样是 `completed`；`blocked`／`unavailable` 更是
/// 「试过没成功」，当时却也被算作做过，于是失败一次的笔记永久从清单里消失。
///
/// 仍然**不存「已补详情」布尔位**：`0079` 那张表记的是「这一次详情落库了什么」，是追加的
/// 事实而不是一个会与运行时不符的状态位。
async fn next_detail_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let (executable, not_stopped, budget_blocked) = sample_side_executable("$1");
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT sample.sample_ref FROM cross_industry_sample sample \
         WHERE {observed} \
           AND {pending} \
           AND {executable} \
           AND NOT {not_stopped} \
           AND NOT {budget_blocked} \
         ORDER BY sample.like_count DESC NULLS LAST, sample.first_seen_at, sample.sample_ref \
         LIMIT $2",
        observed = sample_observed_by_target_sql!("$1"),
        pending = pending_detail_sql!("$1"),
    )))
    .bind(target_ref)
    .bind(KEYWORD_DETAIL_BATCH_SIZE)
    .fetch_all(&mut **transaction)
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
    let schema_ready: bool = sqlx::query_scalar(concat!(
        "SELECT ",
        sample_facts_schema_ready_sql!(),
        " AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    ))
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        // 与媒体投影、渐进档案两步同一句话：**自己的表不在 = 没轮到**，不是「没有欠详情的
        // 词」。此前这里静默返回空汇总，两种情形在日志与账本上长得一模一样。
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    // 只找仍欠详情的关键词。baseline 的覆盖事实决定“这一轮搜索是否完整”，不能冻结
    // 已经发现却仍不完整的材料；否则 target 进入 monitoring 后会永久失去补详情路径。
    let due: Vec<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target \
         WHERE target_kind='keyword' AND lifecycle_state <> 'dismissed' \
           AND domain_ref IS NOT NULL \
         ORDER BY last_scheduler_considered_at NULLS FIRST,target_ref",
    )
    .fetch_all(database.pool())
    .await?;
    if due.is_empty() {
        return Ok(summary);
    }
    if !crate::collection_governance_enabled() {
        summary.skipped.extend(due.into_iter().map(|target_ref| {
            (target_ref, "collection_upgrade_recovery_only".to_owned())
        }));
        return Ok(summary);
    }
    let pending = keyword_targets_pending_detail(database, &due).await?;
    for target_ref in due
        .into_iter()
        .filter(|target| pending.contains(target))
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
        match advance_keyword_archive_detail(database, target_ref, purpose, "agent").await? {
            KeywordDetailAdvance::Queued { works, .. } => summary.queued.push((target_ref, works)),
            KeywordDetailAdvance::Skipped(reason) => {
                summary.skipped.push((target_ref, reason.to_owned()));
            }
        }
    }
    Ok(summary)
}
