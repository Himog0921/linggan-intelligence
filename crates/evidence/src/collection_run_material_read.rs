//! Run-scoped reads of shared comment observations. Later observations never enter old results.
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub async fn comments(
    db: &Database,
    run: Uuid,
    domain: Uuid,
    work: Uuid,
    after: Option<Uuid>,
    text: Option<&str>,
) -> Result<Value, sqlx::Error> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let bound:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM collection_user_member m WHERE m.run_ref=$1 AND m.domain_ref=$2 AND m.content_public_ref=$3)").bind(run).bind(domain).bind(work).fetch_one(&mut *tx).await?;
    if !bound {
        return Err(sqlx::Error::RowNotFound);
    }
    let mut rows = sqlx::query(COMMENT_PAGE)
        .bind(run)
        .bind(work)
        .bind(text)
        .bind(after)
        .bind(domain)
        .fetch_all(&mut *tx)
        .await?;
    let total: i64 = sqlx::query_scalar(COMMENT_COUNT)
        .bind(run)
        .bind(work)
        .bind(text)
        .bind(domain)
        .fetch_one(&mut *tx)
        .await?;
    let more = rows.len() > 20;
    rows.truncate(20);
    let next = if more {
        rows.last().map(|r| r.get::<Uuid, _>("material_ref"))
    } else {
        None
    };
    let items:Vec<Value>=rows.iter().map(|r|json!({"sourceRef":r.get::<Uuid,_>("material_ref"),"relation":if r.get::<bool,_>("is_reply"){"REPLY"}else{"ROOT"},"body":r.get::<Option<String>,_>("body_text"),"bodyState":r.get::<String,_>("body_state"),"bodyTruncated":r.get::<bool,_>("body_truncated"),"rootCommentIdentity":r.get::<String,_>("root_comment_external_id"),"parentCommentIdentity":r.get::<Option<String>,_>("parent_comment_external_id"),"anonymousAuthorContext":{"identity":"WITHHELD","platformUserIdReturned":false},"observedAt":r.get::<String,_>("observed_at")})).collect();
    tx.commit().await?;
    Ok(
        json!({"channel":"comments","accessLevel":"LOCAL_AUTHORIZED_RESEARCH","searchableOriginalText":true,"externalAuthorIdentityReturned":false,"collectionRunRef":run,"domainRef":domain,"query":text,"total":total,"returned":items.len(),"truncated":more,"nextCursor":next,"limit":20,"items":items}),
    )
}
const COMMENT_PAGE: &str = r#"WITH scoped AS (
 SELECT DISTINCT ON(c.comment_external_id) c.* FROM linggan_material_comment c JOIN collection_user_package p USING(package_ref)
 WHERE p.run_ref=$1 AND c.content_public_ref=$2 AND p.domain_ref=$5 ORDER BY c.comment_external_id,p.accepted_at DESC,c.material_ref
), qualified AS (
 SELECT c.material_ref,c.is_reply,c.root_comment_external_id,c.parent_comment_external_id,left(c.body_text,500) AS body_text,c.body_state,c.observed_at,length(c.body_text)>500 AS body_truncated
 FROM scoped c WHERE NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)
 AND ($3::text IS NULL OR strpos(lower(COALESCE(c.body_text,'')),lower($3))>0)
) SELECT material_ref,is_reply,root_comment_external_id,parent_comment_external_id,body_text,body_state,observed_at,COALESCE(body_truncated,false) AS body_truncated FROM qualified
 WHERE ($4::uuid IS NULL OR material_ref>$4) ORDER BY material_ref LIMIT 21"#;
const COMMENT_COUNT: &str = r#"WITH scoped AS (
 SELECT DISTINCT ON(c.comment_external_id) c.* FROM linggan_material_comment c JOIN collection_user_package p USING(package_ref)
 WHERE p.run_ref=$1 AND c.content_public_ref=$2 AND p.domain_ref=$4 ORDER BY c.comment_external_id,p.accepted_at DESC,c.material_ref
) SELECT count(*) FROM scoped c
 WHERE NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)
 AND ($3::text IS NULL OR strpos(lower(COALESCE(c.body_text,'')),lower($3))>0)"#;

/// Replace cumulative comment and text observations with the run's actual accepted versions.
/// Shared media remains the current resource layer and is labelled separately in the UI.
pub async fn scope_items_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run: Uuid,
    domain: Uuid,
    items: &mut [crate::material_projection_types::MaterialLibraryItem],
) -> Result<(), sqlx::Error> {
    let refs: Vec<Uuid> = items.iter().map(|i| i.identity.public_ref).collect();
    let rows=sqlx::query(r#"SELECT m.content_public_ref,detail.title,detail.body_text,detail.creator_display_name,detail.package_ref AS detail_package_ref,to_jsonb(detail) AS detail_version,to_jsonb(discovery) AS discovery_version,
      (SELECT count(DISTINCT c.comment_external_id) FROM linggan_material_comment c JOIN collection_user_package p USING(package_ref) WHERE c.content_public_ref=m.content_public_ref AND p.run_ref=$1 AND p.domain_ref=$2 AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)) AS comments,
      (SELECT count(DISTINCT c.comment_external_id) FROM linggan_material_comment c JOIN collection_user_package p USING(package_ref) WHERE c.content_public_ref=m.content_public_ref AND p.run_ref=$1 AND p.domain_ref=$2 AND c.is_reply AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)) AS replies,
      (SELECT raw.coverage #> '{target,commentCollection}' FROM collection_user_package p JOIN linggan_runtime_capture_package raw USING(package_ref) WHERE p.run_ref=$1 AND p.domain_ref=$2 AND raw.package_kind='comments' AND raw.coverage #>> '{target,contentExternalId}'=(SELECT content_external_id FROM linggan_material_content WHERE public_ref=m.content_public_ref) ORDER BY p.accepted_at DESC,p.package_ref DESC LIMIT 1) AS receipt
     FROM collection_user_member m LEFT JOIN collection_run_work w ON w.run_ref=m.run_ref AND w.content_public_ref=m.content_public_ref
     LEFT JOIN LATERAL(SELECT d.* FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package p USING(package_ref) WHERE d.content_public_ref=m.content_public_ref AND (d.package_ref=w.reused_detail_package_ref OR EXISTS(SELECT 1 FROM collection_user_package u WHERE u.run_ref=$1 AND u.domain_ref=$2 AND u.package_ref=d.package_ref)) ORDER BY p.accepted_at DESC,d.material_ref LIMIT 1) detail ON true
     LEFT JOIN LATERAL(SELECT f.* FROM linggan_material_discovery_finding f JOIN collection_user_package p USING(package_ref) WHERE f.content_public_ref=m.content_public_ref AND p.run_ref=$1 AND p.domain_ref=$2 ORDER BY p.accepted_at DESC,f.material_ref LIMIT 1) discovery ON true
     WHERE m.run_ref=$1 AND m.domain_ref=$2 AND m.content_public_ref=ANY($3)"#).bind(run).bind(domain).bind(&refs).fetch_all(&mut **tx).await?;
    let rows: std::collections::HashMap<Uuid, _> = rows
        .into_iter()
        .map(|r| (r.get("content_public_ref"), r))
        .collect();
    for item in items {
        let Some(row) = rows.get(&item.identity.public_ref) else {
            continue;
        };
        let cover_title = if item.display.title_source == "cover_ocr" {
            Some((
                item.display.title.clone(),
                item.display.title_media_display_ordinal,
            ))
        } else {
            None
        };
        let receipt: Option<Value> = row.get("receipt");
        let detail_package: Option<Uuid> = row.get("detail_package_ref");
        if detail_package.is_some() {
            item.display.title = row.get("title");
            item.display.title_state = if item.display.title.is_some() {
                "KNOWN"
            } else {
                "UNKNOWN"
            }
            .into();
            item.display.creator_display_name = row.get("creator_display_name");
            item.display.creator_state = if item.display.creator_display_name.is_some() {
                "KNOWN"
            } else {
                "UNKNOWN"
            }
            .into();
            item.body_text = row.get("body_text");
            item.evidence_fragment = item.body_text.as_ref().map(|body| {
                crate::material_projection_types::MaterialEvidenceFragment {
                    text: body.chars().take(500).collect(),
                    source_kind: "detail_body",
                    selection_basis: "FIRST_AVAILABLE",
                    truncated: body.chars().count() > 500,
                    source_ref: None,
                    slot_key: None,
                    match_offset: None,
                    match_length: None,
                }
            });
        } else {
            // An unrelated future detail/comment cannot become the first excerpt of an old run.
            item.body_text = None;
            item.evidence_fragment = None;
        }
        let detail: Option<Value> = row.get("detail_version");
        let discovery: Option<Value> = row.get("discovery_version");
        let version = detail.as_ref().or(discovery.as_ref());
        let string = |key: &str| version.and_then(|v| v[key].as_str()).map(str::to_owned);
        if detail_package.is_none() {
            item.display.title = string("title");
            item.display.creator_display_name = string("creator_display_name");
        }
        item.display.title_state = if item.display.title.is_some() {
            "KNOWN"
        } else {
            "UNKNOWN"
        }
        .into();
        item.display.title_source = "platform_title".into();
        item.display.title_media_display_ordinal = None;
        if item
            .display
            .title
            .as_ref()
            .is_none_or(|t| t.trim().is_empty())
        {
            if let Some((title, ordinal)) = cover_title {
                item.display.title = title;
                item.display.title_source = "cover_ocr".into();
                item.display.title_media_display_ordinal = ordinal;
                item.display.title_state = "KNOWN".into();
            }
        }
        item.display.creator_state = if item.display.creator_display_name.is_some() {
            "KNOWN"
        } else {
            "UNKNOWN"
        }
        .into();
        item.author_external_id = string("author_external_id");
        item.display.published_at = string("published_at");
        item.display.published_at_source_text = string("published_at_source_text");
        item.display.published_at_state = if item.display.published_at.is_some() {
            "KNOWN"
        } else {
            "UNKNOWN"
        }
        .into();
        item.display.published_at_source_field = string("published_at_source_field");
        item.display.published_at_source_kind =
            string("published_at_source_kind").unwrap_or_else(|| "unknown".into());
        item.display.published_at_precision =
            string("published_at_precision").unwrap_or_else(|| "unknown".into());
        item.display.published_at_reference_observed_at =
            string("published_at_reference_observed_at");
        item.display.published_at_parser_version = string("published_at_parser_version");
        let count = |key: &str| version.and_then(|v| v[key].as_i64());
        item.display.engagement = crate::material_projection_types::MaterialEngagement {
            like_count: count("like_count"),
            like_count_state: string("like_count_state").unwrap_or_else(|| "UNKNOWN".into()),
            comment_count: count("comment_count"),
            comment_count_state: string("comment_count_state").unwrap_or_else(|| "UNKNOWN".into()),
            collect_count: count("collect_count"),
            collect_count_state: string("collect_count_state").unwrap_or_else(|| "UNKNOWN".into()),
            share_count: count("share_count"),
            share_count_state: string("share_count_state").unwrap_or_else(|| "UNKNOWN".into()),
        };
        // Current author profile/media can be inspected as shared context; do not claim they
        // were obtained by this historical run. Basic author identity comes from its detail.
        item.inspector["sharedCurrentAuthorContext"] = item.inspector["authorContext"].clone();
        item.inspector["authorContext"] = Value::Null;
        item.matched_fields.clear();
        item.inspector["commentThreads"] = json!([]);
        item.inspector["commentsCoverageHistory"] = json!([]);
        item.inspector["repliesCoverageHistory"] = json!([]);
        item.inspector["commentsCoverage"] = receipt.clone().unwrap_or(Value::Null);
        item.inspector["repliesCoverage"] = receipt.clone().unwrap_or(Value::Null);
        item.inspector["commentsReceipt"] = receipt.clone().unwrap_or(Value::Null);
        item.inspector["collectionRunContext"] = json!({"runRef":run,"domainRef":domain,"comments":row.get::<i64,_>("comments"),"replies":row.get::<i64,_>("replies"),"detailPackageRef":detail_package,"mediaBasis":"SHARED_CURRENT_RESOURCES","commentBasis":"RUN_ACCEPTED_PACKAGES"});
        for lane in item
            .lane_summaries
            .iter_mut()
            .filter(|l| matches!(l.lane, "comments" | "replies"))
        {
            lane.retained = Some(row.get(if lane.lane == "comments" {
                "comments"
            } else {
                "replies"
            }));
            lane.state = if receipt.is_none() {
                "NOT_OBSERVED"
            } else if receipt.as_ref().is_some_and(|r| r["state"] == "complete") {
                "OBSERVED"
            } else {
                "PARTIAL"
            };
            lane.value_state = if receipt.is_some() {
                "KNOWN"
            } else {
                "UNKNOWN"
            };
            lane.observed = None;
            lane.unique_collected_count = None;
            lane.collection_scope = receipt
                .as_ref()
                .and_then(|r| r["scope"].as_str().map(str::to_owned));
            lane.collection_state = receipt
                .as_ref()
                .and_then(|r| r["state"].as_str().map(str::to_owned));
            lane.expected_count = receipt.as_ref().and_then(|r| r["expectedCount"].as_i64());
            lane.requested_limit = receipt.as_ref().and_then(|r| r["requestedLimit"].as_i64());
            lane.stopped_reason = receipt
                .as_ref()
                .and_then(|r| r["stopReason"].as_str().map(str::to_owned));
            lane.limitations = vec!["RUN_ACCEPTED_OBSERVATIONS_ONLY"];
            lane.latest_observed_at = None;
        }
    }
    Ok(())
}

/// Match only text versions admitted by this run or its explicitly reused detail.
pub(crate) async fn matching_refs_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run: Uuid,
    domain: Uuid,
    text: Option<&str>,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(r#"SELECT m.content_public_ref FROM collection_user_member m LEFT JOIN collection_run_work w ON w.run_ref=m.run_ref AND w.content_public_ref=m.content_public_ref
 LEFT JOIN LATERAL(SELECT d.title,d.body_text,d.creator_display_name FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package p USING(package_ref) WHERE d.content_public_ref=m.content_public_ref AND (d.package_ref=w.reused_detail_package_ref OR EXISTS(SELECT 1 FROM collection_user_package u WHERE u.run_ref=$1 AND u.domain_ref=$2 AND u.package_ref=d.package_ref)) ORDER BY p.accepted_at DESC,d.material_ref LIMIT 1) detail ON true
 LEFT JOIN LATERAL(SELECT f.title,f.creator_display_name FROM linggan_material_discovery_finding f JOIN collection_user_package p USING(package_ref) WHERE f.content_public_ref=m.content_public_ref AND p.run_ref=$1 AND p.domain_ref=$2 ORDER BY p.accepted_at DESC,f.material_ref LIMIT 1) discovery ON true
 WHERE m.run_ref=$1 AND m.domain_ref=$2 AND ($3::text IS NULL OR strpos(lower(concat_ws(' ',COALESCE(detail.title,discovery.title),detail.body_text,COALESCE(detail.creator_display_name,discovery.creator_display_name))),lower($3))>0 OR EXISTS(
 SELECT 1 FROM (SELECT DISTINCT ON(c.comment_external_id) c.comment_external_id,c.body_text FROM linggan_material_comment c JOIN collection_user_package p USING(package_ref) WHERE c.content_public_ref=m.content_public_ref AND p.run_ref=$1 AND p.domain_ref=$2 ORDER BY c.comment_external_id,p.accepted_at DESC,c.material_ref) c WHERE strpos(lower(COALESCE(c.body_text,'')),lower($3))>0 AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction WHERE restriction.content_public_ref=m.content_public_ref AND restriction.comment_external_id=c.comment_external_id))) ORDER BY m.content_public_ref"#).bind(run).bind(domain).bind(text).fetch_all(&mut **tx).await
}
