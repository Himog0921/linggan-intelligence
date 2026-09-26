//! Verifiable work lists for one observation target.
//!
//! This is a read-only presentation seam.  It deliberately follows accepted,
//! target-scoped discovery records rather than reconstructing a directory from
//! a creator name or an unscoped corpus search.

use crate::execution_input_eligibility::{MaterialExecutionState, read_material_execution_states};
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
    /// 这一篇此刻「能不能取详情、不能时欠的是什么」——`0097` 台账的唯一读取口。
    ///
    /// 空表示台账里没有这一篇的行：那既不是停止也不是冷却，界面不替它编一个原因。
    pub execution_state: Option<MaterialExecutionState>,
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
    domain_ref: Option<Uuid>,
) -> Result<Option<CreatorDirectoryProjection>, sqlx::Error> {
    let Some(mut works) = read_catalog(
        database,
        target_ref,
        domain_ref,
        "creator",
        "profile_discovery",
    )
    .await?
    else {
        return Ok(None);
    };
    stamp_material_execution_states(
        database,
        target_ref,
        "own_domain",
        "material_content",
        &mut works,
    )
    .await?;
    Ok(Some(CreatorDirectoryProjection { works }))
}

pub async fn read_keyword_hits(
    database: &Database,
    target_ref: Uuid,
    domain_ref: Option<Uuid>,
) -> Result<Option<KeywordHitProjection>, sqlx::Error> {
    let Some(mut works) = read_catalog(
        database,
        target_ref,
        domain_ref,
        "keyword",
        "discovery_search",
    )
    .await?
    else {
        return Ok(None);
    };
    stamp_material_execution_states(
        database,
        target_ref,
        "own_domain",
        "material_content",
        &mut works,
    )
    .await?;
    Ok(Some(KeywordHitProjection { works }))
}

/// 把同一标准材料资格台账中的当前状态贴到这一页作品上；页面不自行拼第二套判据。
async fn stamp_material_execution_states(
    database: &Database,
    target_ref: Uuid,
    domain_scope: &str,
    object_kind: &str,
    works: &mut [CatalogWork],
) -> Result<(), sqlx::Error> {
    let object_refs: Vec<Uuid> = works.iter().map(|work| work.public_ref).collect();
    let mut states = read_material_execution_states(
        database,
        target_ref,
        domain_scope,
        object_kind,
        &object_refs,
    )
    .await?;
    for work in works.iter_mut() {
        work.execution_state = states.remove(&work.public_ref);
    }
    Ok(())
}

pub async fn read_keyword_catalog_counts(
    database: &Database,
    target_refs: &[Uuid],
    domain_ref: Option<Uuid>,
) -> Result<std::collections::HashMap<Uuid, KeywordCatalogCounts>, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut counts: std::collections::HashMap<Uuid, KeywordCatalogCounts> =
        std::collections::HashMap::new();

    // 同一标准材料链：具体 Domain 限定用途，None 汇总全部有冻结用途的领域。
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
         JOIN collection_work_order_domain_usage work_usage USING(work_order_ref) \
         JOIN linggan_material_domain_usage material_usage \
           ON material_usage.content_public_ref=finding.content_public_ref \
          AND material_usage.domain_ref=work_usage.domain_ref \
         JOIN linggan_runtime_record_disposition disposition \
           ON disposition.package_ref=finding.package_ref \
          AND disposition.record_ordinal=finding.record_ordinal \
         WHERE work_order.target_ref=ANY($1) \
           AND ($2::uuid IS NULL OR work_usage.domain_ref=$2) \
           AND finding.discovery_kind='discovery_search' \
           AND receipt.material_admission='ACCEPTED' \
           AND disposition.disposition='accepted_for_library_discovery' \
         GROUP BY 1",
    ))
    .bind(target_refs)
    .bind(domain_ref)
    .fetch_all(database.pool())
    .await?;
    for (target_ref, works, details) in evidence {
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
    domain_ref: Option<Uuid>,
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
               AND EXISTS (SELECT 1 FROM collection_work_order_domain_usage work_usage \
                           JOIN linggan_material_domain_usage material_usage \
                             ON material_usage.domain_ref=work_usage.domain_ref \
                            AND material_usage.content_public_ref=finding.content_public_ref \
                            WHERE work_usage.work_order_ref=work_order.work_order_ref \
                              AND ($3::uuid IS NULL OR work_usage.domain_ref=$3)) \
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
               AND ($3::uuid IS NULL OR EXISTS (SELECT 1 FROM linggan_material_domain_usage material_usage \
                            WHERE material_usage.content_public_ref=author.content_public_ref \
                              AND material_usage.domain_ref=$3)) \
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
    .bind(domain_ref)
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
                    // 台账里的当前状态在 `read_creator_directory` / `read_keyword_hits` 里贴上：
                    // 这条查询只读发现、详情与评论，不自己拼一份资格判据。
                    execution_state: None,
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
