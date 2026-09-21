//! Verifiable work lists for one observation target.
//!
//! This is a read-only presentation seam.  It deliberately follows accepted,
//! target-scoped discovery records rather than reconstructing a directory from
//! a creator name or an unscoped corpus search.

use crate::cross_industry_sample_facts::{
    sample_detail_obtained_sql, sample_facts_schema_ready_sql, sample_observed_by_target_sql,
};
use crate::qualified_detail::qualified_detail_exists_sql;
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CatalogSource {
    InitialArchive,
    PatrolDiscovery,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CatalogDetailState {
    Complete,
    Pending,
}

#[derive(Debug, Clone)]
pub struct CatalogWork {
    pub public_ref: Uuid,
    pub content_external_id: String,
    pub title: Option<String>,
    /// Present for keyword search hits.  A creator directory already has this owner.
    pub creator_display_name: Option<String>,
    /// The rank is a search-result fact, not inferred from table order.
    pub match_position: Option<i64>,
    pub published_at: Option<String>,
    pub source: CatalogSource,
    pub detail_state: CatalogDetailState,
    pub media_state: &'static str,
    pub comment_count: Option<i64>,
    pub last_captured_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CreatorDirectoryProjection {
    pub works: Vec<CatalogWork>,
}

#[derive(Debug, Clone, Default)]
pub struct KeywordHitProjection {
    pub works: Vec<CatalogWork>,
}

pub async fn read_creator_directory(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<CreatorDirectoryProjection>, sqlx::Error> {
    read_catalog(database, target_ref, "creator", "profile_discovery")
        .await
        .map(|value| value.map(|works| CreatorDirectoryProjection { works }))
}

pub async fn read_keyword_hits(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<KeywordHitProjection>, sqlx::Error> {
    read_catalog(database, target_ref, "keyword", "discovery_search")
        .await
        .map(|value| value.map(|works| KeywordHitProjection { works }))
}

/// 一个**跨行业**关键词目标的命中作品。
///
/// 外部领域的材料按 `0044` 的隔离住在 `cross_industry_sample`，证据侧一条都没有。检查器
/// 此前只读证据侧，于是跨行业目标的作品页永远是「0 条可查证命中」「当前筛选没有匹配的
/// 作品」——采回来 204 篇，界面上一篇都看不到，而且看不出是"没采到"还是"读错了地方"。
///
/// 返回形状与证据侧那条刻意一致（`CatalogWork`），同一套渲染直接吃两种领域的数据：
/// **界面统一，数据不合并**。这不是给证据侧查询加参数——那是规格的接口红线，两条路
/// 不共享查询、不共享接口，隔离才不依赖任何人记得在某处加一个条件。
pub async fn read_cross_industry_hits(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<KeywordHitProjection>, sqlx::Error> {
    let schema_ready: bool =
        sqlx::query_scalar(concat!("SELECT ", sample_facts_schema_ready_sql!()))
            .fetch_one(database.pool())
            .await?;
    if !schema_ready {
        return Ok(None);
    }
    let rows = sqlx::query(concat!(
        "SELECT sample.sample_ref,sample.content_external_id,sample.title,sample.author_name, \
                sample.published_at IS NOT NULL AS has_published, \
                to_char(sample.published_at,'YYYY-MM-DD') AS published_on, \
                linggan_human_moment(sample.last_observed_at) AS last_captured_at, \
                sample.comment_count, \
                -- `discovery_order` 是 integer，而展示合同上的名次是 bigint。不显式转换
                -- 的话这一列会在解码时炸掉（`INT4` 对不上 `Option<i64>`），而且只有在真的
                -- 有观察记录的目标上才炸——空库一路绿灯。
                --
                -- 名次只算**这个目标自己那几轮**看到的位次。跨目标取 min 会把另一个关键词
                -- 那一轮的位次摆在这个关键词的作品页上——同一篇在两个词底下的位次本来就
                -- 不是同一件事，混起来看不出是错的。
                (SELECT (min(seen.discovery_order)+1)::bigint \
                   FROM cross_industry_sample_observation seen \
                   JOIN linggan_runtime_capture_package seen_package \
                     ON seen_package.package_ref=seen.package_ref \
                   JOIN collection_work_order_lease_task seen_lease_task \
                     ON seen_lease_task.task_id=seen_package.task_id \
                   JOIN collection_work_order_lease seen_lease USING(lease_ref) \
                   JOIN collection_work_order seen_order USING(work_order_ref) \
                  WHERE seen.sample_ref=sample.sample_ref \
                    AND seen_order.target_ref=$1) AS match_position, \
                -- 「详情到手了」看的是 `0079` 那条材料事实，不是运行任务的状态。
                ",
        sample_detail_obtained_sql!(),
        " AS detail_done
         FROM cross_industry_sample sample
         -- 命中范围按**观察记录**算，不按样本行上的 `target_ref`：那一列只在第一次插入时
         -- 写定，同一篇被另一个关键词先看到，这个目标就永远看不到它。
         WHERE ",
        sample_observed_by_target_sql!("$1"),
        " ORDER BY sample.like_count DESC NULLS LAST,sample.first_seen_at,sample.sample_ref",
    ))
    .bind(target_ref)
    .fetch_all(database.pool())
    .await?;
    let works = rows
        .into_iter()
        .map(|row| CatalogWork {
            public_ref: row.get("sample_ref"),
            content_external_id: row.get("content_external_id"),
            title: row.get("title"),
            creator_display_name: row.get("author_name"),
            match_position: row.get("match_position"),
            published_at: row.get("published_on"),
            // 跨行业样本目前不区分「建档带回」与「巡检新增」：两者都 upsert 进同一行，
            // 哪一轮先看到它记在观察记录里（`0073`），不在样本行上。统一记作建档来源，
            // 而不是编一个分不出来的巡检标记。
            source: CatalogSource::InitialArchive,
            detail_state: if row.get::<bool, _>("detail_done") {
                CatalogDetailState::Complete
            } else {
                CatalogDetailState::Pending
            },
            // 跨行业侧不下载媒体，也不做 OCR/转录：参照物只看列表与正文。
            media_state: "NOT_APPLICABLE",
            comment_count: row.get("comment_count"),
            last_captured_at: row.get("last_captured_at"),
        })
        .collect();
    Ok(Some(KeywordHitProjection { works }))
}

/// 一批关键词各自**命中了多少篇、其中多少篇取到了详情**。
///
/// 列表页一次要显示很多行，逐行查会变成 N+1。两侧都要数：本领域的材料在证据侧，外部
/// 领域的在跨行业语料（`0044` 的隔离）。只数一侧，另一侧那些行会显示成 0——而 0 和
/// 「还没采」在界面上长得一样。
///
/// 读不到时返回 `Err`，由调用方如实呈现；不把读不到压成 0。
pub async fn read_keyword_catalog_counts(
    database: &Database,
    target_refs: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, KeywordCatalogCounts>, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let cross_industry_ready: bool =
        sqlx::query_scalar(concat!("SELECT ", sample_facts_schema_ready_sql!()))
            .fetch_one(database.pool())
            .await?;
    let mut counts: std::collections::HashMap<Uuid, KeywordCatalogCounts> =
        std::collections::HashMap::new();

    // 证据侧：本领域关键词的命中作品与它们的详情。
    let evidence: Vec<(Uuid, i64, i64)> = sqlx::query_as(concat!(
        "SELECT work_order.target_ref, \
                count(DISTINCT finding.content_public_ref), \
                count(DISTINCT finding.content_public_ref) FILTER (WHERE ",
        qualified_detail_exists_sql!("finding.content_public_ref"),
        ") \
         FROM collection_work_order work_order \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
         JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
         JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
         JOIN linggan_material_discovery_finding finding USING(package_ref) \
         JOIN linggan_runtime_record_disposition disposition \
           ON disposition.package_ref=finding.package_ref \
          AND disposition.record_ordinal=finding.record_ordinal \
         WHERE work_order.target_ref=ANY($1) \
           AND finding.discovery_kind='discovery_search' \
           AND receipt.material_admission='ACCEPTED' \
           AND disposition.disposition='accepted_for_library_discovery' \
         GROUP BY 1",
    ))
    .bind(target_refs)
    .fetch_all(database.pool())
    .await?;
    for (target_ref, works, details) in evidence {
        let entry = counts.entry(target_ref).or_default();
        entry.works += works;
        entry.details += details;
    }
    if !cross_industry_ready {
        return Ok(counts);
    }

    // 跨行业侧：口径与检查器那条完全一致——两处共用 `cross_industry_sample_facts` 里的
    // 同一份判据，不再各写一遍。**按目标分组也走观察记录**：样本行上的 `target_ref` 只在
    // 第一次插入时写定，用它分组会把同一篇只记在最早看到它的那个关键词名下。
    let cross: Vec<(Uuid, i64, i64)> = sqlx::query_as(concat!(
        "SELECT seen_order.target_ref, \
                count(DISTINCT sample.sample_ref), \
                count(DISTINCT sample.sample_ref) FILTER (WHERE ",
        sample_detail_obtained_sql!(),
        ") \
         FROM cross_industry_sample sample \
         JOIN cross_industry_sample_observation seen \
           ON seen.sample_ref=sample.sample_ref \
         JOIN linggan_runtime_capture_package seen_package \
           ON seen_package.package_ref=seen.package_ref \
         JOIN collection_work_order_lease_task seen_lease_task \
           ON seen_lease_task.task_id=seen_package.task_id \
         JOIN collection_work_order_lease seen_lease USING(lease_ref) \
         JOIN collection_work_order seen_order USING(work_order_ref) \
         WHERE seen_order.target_ref=ANY($1) GROUP BY 1",
    ))
    .bind(target_refs)
    .fetch_all(database.pool())
    .await?;
    for (target_ref, works, details) in cross {
        let entry = counts.entry(target_ref).or_default();
        entry.works += works;
        entry.details += details;
    }
    Ok(counts)
}

/// 一个关键词目标的命中与详情计数。
#[derive(Debug, Clone, Copy, Default)]
pub struct KeywordCatalogCounts {
    pub works: i64,
    pub details: i64,
}

async fn read_catalog(
    database: &Database,
    target_ref: Uuid,
    target_kind: &str,
    discovery_kind: &str,
) -> Result<Option<Vec<CatalogWork>>, sqlx::Error> {
    let target_exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target WHERE target_ref=$1 AND target_kind=$2",
    )
    .bind(target_ref)
    .bind(target_kind)
    .fetch_optional(database.pool())
    .await?;
    if target_exists.is_none() {
        return Ok(None);
    }
    let rows = sqlx::query(
        "WITH discoveries AS ( \
             SELECT finding.content_public_ref,content.content_external_id,work_order.lane,package.accepted_at, \
                    finding.title,finding.title_state,finding.creator_display_name,finding.creator_state, \
                    finding.result_position,finding.published_at_source_text \
             FROM collection_work_order work_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             JOIN linggan_material_discovery_finding finding USING(package_ref) \
             JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal \
             WHERE work_order.target_ref=$1 AND work_order.lane IN ('deep_archive','patrol') \
               AND finding.discovery_kind=$2 AND receipt.material_admission='ACCEPTED' \
               AND disposition.disposition <> 'quarantined' \
         ), attributed AS ( \
             -- 作者归属来自作品自己（`linggan_material_content_author` 推自 append-only 事实），
             -- 不再只靠「这篇是在哪张工单下被发现的」。创作者目录因此不依赖控制面：
             -- 删掉观察目标之后，作品仍然属于这个博主。
             SELECT author.content_public_ref \
             FROM linggan_material_content_author author \
             JOIN collection_observation_target target \
               ON target.identity_key=author.author_external_id \
              AND target.platform=author.platform \
              AND target.target_kind='creator' \
             WHERE target.target_ref=$1 \
         ), owned AS ( \
             -- 目标是创作者时以作者归属为准；关键词目标没有作者可言，仍按发现所属的工单算。
             SELECT * FROM discoveries \
             WHERE $2 <> 'profile_discovery' \
                OR content_public_ref IN (SELECT content_public_ref FROM attributed) \
         ), first_discovery AS ( \
             SELECT DISTINCT ON (content_public_ref) * FROM owned \
             ORDER BY content_public_ref,accepted_at,CASE WHEN lane='deep_archive' THEN 0 ELSE 1 END \
         ) \
         SELECT first_discovery.content_public_ref,first_discovery.content_external_id, \
                CASE WHEN first_discovery.title_state='KNOWN' THEN first_discovery.title END AS discovery_title, \
                CASE WHEN first_discovery.creator_state='KNOWN' THEN first_discovery.creator_display_name END AS creator_display_name, \
                first_discovery.result_position::bigint AS result_position, \
                linggan_human_moment(first_discovery.published_at_source_text) \
                    AS published_at_source_text, \
                first_discovery.lane, \
                detail.content_public_ref AS qualified_detail_ref, \
                detail.title AS detail_title, \
                linggan_human_moment(detail.published_at) AS detail_published_at, \
                linggan_human_moment(detail.observed_at) AS detail_observed_at, \
                comments.comment_count, \
                CASE WHEN EXISTS (SELECT 1 FROM linggan_material_derived_text derived \
                                  WHERE derived.content_public_ref=first_discovery.content_public_ref) THEN '已处理' \
                     WHEN EXISTS (SELECT 1 FROM linggan_material_media_origin origin \
                                  WHERE origin.content_public_ref=first_discovery.content_public_ref) THEN '待处理' \
                     ELSE '—' END AS media_state \
         FROM first_discovery \
         -- 这份内容**合格详情材料**的最新一行。存在性就是「详情已取得」（判据见
         -- `qualified_detail.rs`，这里是它的行级孪生：为了取那一行的字段才把连接写在这里）；
         -- 标题只从这一行顺带取出，**不参与完成判断**。
         LEFT JOIN LATERAL ( \
             SELECT candidate.content_public_ref,candidate.title,candidate.published_at,candidate.observed_at \
             FROM linggan_material_content_detail candidate \
             JOIN linggan_runtime_capture_package package USING(package_ref) \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=candidate.package_ref AND disposition.record_ordinal=candidate.record_ordinal \
             WHERE candidate.content_public_ref=first_discovery.content_public_ref \
               AND receipt.material_admission='ACCEPTED' AND disposition.disposition <> 'quarantined' \
             ORDER BY package.accepted_at DESC,candidate.created_at DESC LIMIT 1 \
         ) detail ON true \
         LEFT JOIN LATERAL ( \
             SELECT count(*)::bigint AS comment_count FROM linggan_material_comment_current comment \
             WHERE comment.content_public_ref=first_discovery.content_public_ref \
         ) comments ON true \
         ORDER BY COALESCE(detail.published_at::text,first_discovery.published_at_source_text) DESC NULLS LAST, \
                  first_discovery.accepted_at DESC,first_discovery.content_public_ref DESC",
    )
    .bind(target_ref)
    .bind(discovery_kind)
    .fetch_all(database.pool())
    .await?;

    Ok(Some(
        rows.into_iter()
            .map(|row| {
                let detail_title: Option<String> = row.get("detail_title");
                // 「详情已取得」= 有一份合格详情材料，与标题无关。`0015` 把标题有无单独记在
                // `title_state` 上：**平台没给标题**（`title` 为空）与**还没取到详情**是两件
                // 事，此前都由 `detail_title.is_some()` 回答，于是取到但没有标题的作品在列表和
                // 抽屉里显示成还欠一篇，括号里的覆盖统计却已经把它算作取得。
                let detail_state = if row.get::<Option<Uuid>, _>("qualified_detail_ref").is_some() {
                    CatalogDetailState::Complete
                } else {
                    CatalogDetailState::Pending
                };
                CatalogWork {
                    public_ref: row.get("content_public_ref"),
                    content_external_id: row.get("content_external_id"),
                    title: detail_title.or_else(|| row.get("discovery_title")),
                    creator_display_name: row.get("creator_display_name"),
                    match_position: row.get("result_position"),
                    published_at: row
                        .get::<Option<String>, _>("detail_published_at")
                        .or_else(|| row.get("published_at_source_text")),
                    source: if row.get::<String, _>("lane") == "patrol" {
                        CatalogSource::PatrolDiscovery
                    } else {
                        CatalogSource::InitialArchive
                    },
                    detail_state,
                    media_state: match row.get::<String, _>("media_state").as_str() {
                        "已处理" => "已处理",
                        "待处理" => "待处理",
                        _ => "—",
                    },
                    comment_count: row.get("comment_count"),
                    last_captured_at: row.get("detail_observed_at"),
                }
            })
            .collect(),
    ))
}
