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

use crate::acquisition_chain::{AcquisitionChainError, request_and_admit_in_transaction_scoped};
use crate::collection_control::keyword_baseline_qualified;
use linggan_storage_postgres::Database;
use uuid::Uuid;

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
/// 前置是**这个词已经建过档**：`keyword_baseline_qualified` 查的是「有没有一轮把搜索面
/// 翻到底、且没有材料被隔离」。没建完就补详情，等于在一个还没挖完的底座上往下修。
pub async fn advance_keyword_archive_detail(
    database: &Database,
    target_ref: Uuid,
    purpose: &str,
    requested_by: &str,
) -> Result<KeywordDetailAdvance, AcquisitionChainError> {
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NOT NULL \
                AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        return Err(AcquisitionChainError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    // 与其余所有推进路径同一个加锁顺序：先锁目标，再谈授权与容量。
    let target: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT target_kind,domain_ref FROM collection_observation_target \
         WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((target_kind, domain_ref)) = target else {
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
    if !keyword_baseline_qualified(&mut transaction, target_ref).await? {
        transaction.rollback().await?;
        return Ok(KeywordDetailAdvance::Skipped("archive_round_not_complete"));
    }

    let samples = next_detail_batch(&mut transaction, target_ref).await?;
    if samples.is_empty() {
        // 「挑不出来」有两种完全不同的原因。候选查询同时排除了「已经取过详情的」和
        // 「已经排在在途工单里的」；只报前者，会把正在跑的批次说成「没有可继续的」，
        // 人看到的是点了没反应，而活其实正在进行。
        let in_flight: bool = sqlx::query_scalar(
            "SELECT EXISTS ( \
                 SELECT 1 FROM collection_work_order work_order \
                 JOIN collection_work_order_cross_industry_target scope USING (work_order_ref) \
                 LEFT JOIN collection_work_order_lease lease USING (work_order_ref) \
                 WHERE work_order.target_ref=$1 \
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

    let works = samples.len();
    let request = request_and_admit_in_transaction_scoped(
        &mut transaction,
        target_ref,
        "deep_archive",
        purpose,
        requested_by,
        &[],
        &samples,
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
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NOT NULL \
                AND to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !schema_ready {
        return Ok(std::collections::HashSet::new());
    }
    let rows: Vec<Uuid> = sqlx::query_scalar(concat!(
        "SELECT DISTINCT sample.target_ref FROM cross_industry_sample sample \
         WHERE sample.target_ref=ANY($1) AND ",
        pending_detail_sql!(),
    ))
    .bind(target_refs)
    .fetch_all(database.pool())
    .await?;
    Ok(rows.into_iter().collect())
}

/// 「这一篇还等着补详情」这条判据本身。单篇挑选与列表页批量共用它，免得同一件事在两处
/// 各写一遍、日后各自漂移。`sample` 是外层给的表别名。
macro_rules! pending_detail_sql {
    () => {
        "sample.source_url IS NOT NULL \
           AND NOT EXISTS ( \
             SELECT 1 FROM collection_work_order done_order \
             JOIN collection_work_order_lease done_lease USING (work_order_ref) \
             JOIN collection_work_order_lease_task done_task USING (lease_ref) \
             JOIN linggan_runtime_task done_runtime ON done_runtime.task_id=done_task.task_id \
             WHERE done_order.target_ref=sample.target_ref \
               AND done_task.execution_state IN ('completed','unavailable','blocked') \
               AND done_runtime.task_spec->'capabilitiesRequested'->>0='content_detail' \
               AND done_runtime.task_spec #>> '{target,contentExternalId}' \
                   = sample.content_external_id) \
           AND NOT EXISTS ( \
             SELECT 1 FROM collection_work_order live_order \
             JOIN collection_work_order_cross_industry_target live_scope \
               USING (work_order_ref) \
             LEFT JOIN collection_work_order_lease live_lease USING (work_order_ref) \
             WHERE live_order.target_ref=sample.target_ref \
               AND live_scope.sample_ref=sample.sample_ref \
               AND (live_order.queue_state IN ('queued','leased') \
                    OR (live_lease.released_at IS NULL \
                        AND live_lease.expires_at>scope_001_now())))"
    };
}

use pending_detail_sql;

/// 下一批该补详情的样本。
///
/// 四个条件缺一不可：
/// - 它是这个目标带回来的（`target_ref`），不是同领域别的词带回来的；
/// - 它有平台返回的签名链接——没有链接就打不开详情页，排进去只会白跑一趟；
/// - 还没有取到过详情；
/// - 没有排在某张还活着的工单上。
///
/// 「取到过详情没有」由运行时事实回答：这个目标底下有没有一条已完成的 `content_detail`
/// 任务指向这篇。**不另存一个「已补详情」布尔位**——那是第二份真相，一旦与运行时不符，
/// 没人看得出来哪边是对的。
async fn next_detail_batch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "SELECT sample.sample_ref FROM cross_industry_sample sample \
         WHERE sample.target_ref=$1 AND ",
        pending_detail_sql!(),
        " ORDER BY sample.like_count DESC NULLS LAST, sample.first_seen_at, sample.sample_ref \
         LIMIT $2",
    ))
    .bind(target_ref)
    .bind(KEYWORD_DETAIL_BATCH_SIZE)
    .fetch_all(&mut **transaction)
    .await
}
