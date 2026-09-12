//! Verifiable work lists for one observation target.
//!
//! This is a read-only presentation seam.  It deliberately follows accepted,
//! target-scoped discovery records rather than reconstructing a directory from
//! a creator name or an unscoped corpus search.

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
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if !schema_ready {
        return Ok(None);
    }
    let rows = sqlx::query(
        "SELECT sample.sample_ref,sample.content_external_id,sample.title,sample.author_name, \
                sample.published_at IS NOT NULL AS has_published, \
                to_char(sample.published_at,'YYYY-MM-DD') AS published_on, \
                linggan_human_moment(sample.last_observed_at) AS last_captured_at, \
                sample.comment_count, \
                -- `discovery_order` 是 integer，而展示合同上的名次是 bigint。不显式转换
                -- 的话这一列会在解码时炸掉（`INT4` 对不上 `Option<i64>`），而且只有在真的
                -- 有观察记录的目标上才炸——空库一路绿灯。
                (SELECT (min(seen.discovery_order)+1)::bigint \
                   FROM cross_industry_sample_observation seen \
                  WHERE seen.sample_ref=sample.sample_ref) AS match_position, \
                -- 「详情到手了」要看**材料真的进来了**，不是「那个任务跑完了」。
                --
                -- 包的身份与任务声明对不上时（`task_package_binding_valid` 为假），记录会被
                -- 判为 `quarantined`、`insert_typed_materials` 整段跳过——一个字段都不会写进
                -- 样本行；但租约任务仍然被标成 `completed`——完成与否说的是这一步执行过了，
                -- 不是材料合格。只看 completed 就会对一篇什么都没有的笔记说「详情已取得」。
                -- 2026-09-10 的 120 条整包隔离正是这个形状。
                EXISTS (SELECT 1 FROM collection_work_order done_order \
                        JOIN collection_work_order_lease done_lease USING (work_order_ref) \
                        JOIN collection_work_order_lease_task done_task USING (lease_ref) \
                        JOIN linggan_runtime_task done_runtime \
                          ON done_runtime.task_id=done_task.task_id \
                        JOIN linggan_runtime_capture_package done_package \
                          ON done_package.task_id=done_runtime.task_id \
                        JOIN linggan_runtime_submission_receipt done_receipt \
                          ON done_receipt.package_ref=done_package.package_ref \
                        WHERE done_order.target_ref=$1 \
                          AND done_task.execution_state='completed' \
                          AND done_receipt.material_admission='ACCEPTED' \
                          AND done_runtime.task_spec->'capabilitiesRequested'->>0='content_detail' \
                          AND done_runtime.task_spec #>> '{target,contentExternalId}' \
                              = sample.content_external_id \
                          AND NOT EXISTS ( \
                            SELECT 1 FROM linggan_runtime_record_disposition quarantine \
                            WHERE quarantine.package_ref=done_package.package_ref \
                              AND quarantine.disposition='quarantined')) AS detail_done \
         FROM cross_industry_sample sample \
         WHERE sample.target_ref=$1 \
         ORDER BY sample.like_count DESC NULLS LAST,sample.first_seen_at,sample.sample_ref",
    )
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
         LEFT JOIN LATERAL ( \
             SELECT candidate.title,candidate.published_at,candidate.observed_at \
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
                let detail_state = if detail_title.is_some() {
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
