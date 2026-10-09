//! Read-only discussion, author-context, coverage, and provenance enrichment.

use crate::material_projection_types::MaterialLibraryItem;
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

pub(crate) async fn enrich(
    tx: &mut Transaction<'_, Postgres>,
    item: &mut MaterialLibraryItem,
    text: Option<&str>,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    enrich_batch(tx, std::slice::from_mut(item), text, as_of).await
}

pub(crate) async fn enrich_batch(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    text: Option<&str>,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    if items.is_empty() {
        return Ok(());
    }
    let refs: Vec<Uuid> = items.iter().map(|item| item.identity.public_ref).collect();
    let latest = sqlx::query(
        "SELECT DISTINCT ON (lane.content_public_ref,lane.lane) lane.content_public_ref,lane.lane,lane.observed,lane.producer_acquired,lane.retained,lane.failed,lane.known_unattempted,lane.unknown_count,lane.maximum_quota,lane.stopped_reason,lane.observed_at,lane.package_ref,package.coverage \
         FROM linggan_material_lane_observation lane JOIN linggan_runtime_capture_package package USING(package_ref) \
         WHERE lane.content_public_ref=ANY($1) AND package.accepted_at <= $2::timestamptz \
         ORDER BY lane.content_public_ref,lane.lane,lane.observed_at::timestamptz DESC,lane.created_at DESC"
    ).bind(&refs).bind(as_of).fetch_all(&mut **tx).await?;
    let histories = sqlx::query(
        "SELECT lane.content_public_ref,lane.lane,lane.observed,lane.producer_acquired,lane.retained,lane.failed,lane.known_unattempted,lane.unknown_count,lane.maximum_quota,lane.stopped_reason,lane.observed_at,lane.created_at::text AS recorded_at,lane.package_ref,package.coverage,package.task_id,package.attempt_id,receipt.receipt_ref \
         FROM linggan_material_lane_observation lane JOIN linggan_runtime_capture_package package USING(package_ref) \
         LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         WHERE lane.content_public_ref=ANY($1) AND lane.lane IN ('comments','replies') AND package.accepted_at <= $2::timestamptz \
         ORDER BY lane.content_public_ref,lane.lane,lane.observed_at::timestamptz DESC,lane.created_at DESC"
    ).bind(&refs).bind(as_of).fetch_all(&mut **tx).await?;
    let counts: HashMap<Uuid,i64> = sqlx::query_as(
        "SELECT comment.content_public_ref,count(DISTINCT comment.comment_external_id) FROM linggan_material_comment comment \
         JOIN linggan_runtime_capture_package package USING(package_ref) WHERE comment.content_public_ref=ANY($1) \
         AND package.accepted_at <= $2::timestamptz GROUP BY comment.content_public_ref"
    ).bind(&refs).bind(as_of).fetch_all(&mut **tx).await?.into_iter().collect();
    let platforms: Vec<&str> = items
        .iter()
        .map(|item| item.identity.platform.as_str())
        .collect();
    let authors: Vec<Option<&str>> = items
        .iter()
        .map(|item| item.author_external_id.as_deref())
        .collect();
    let contexts = sqlx::query(
        "WITH requested AS (SELECT * FROM unnest($1::uuid[],$2::text[],$3::text[]) AS scope(content_public_ref,platform,author_id)) \
         SELECT DISTINCT ON (requested.content_public_ref) requested.content_public_ref,author.material_ref,author.package_ref,author.observed_at,author.display_name,author.display_name_state,author.biography_state,author.follower_count,author.follower_count_state \
         FROM requested JOIN linggan_material_author_profile author ON author.platform=requested.platform AND author.author_external_id=requested.author_id \
         JOIN linggan_runtime_capture_package package USING(package_ref) WHERE package.accepted_at <= $4::timestamptz \
         ORDER BY requested.content_public_ref,author.observed_at::timestamptz DESC,author.created_at DESC"
    ).bind(&refs).bind(&platforms).bind(&authors).bind(as_of).fetch_all(&mut **tx).await?;
    let provenance_rows = sqlx::query(
        "WITH requested AS (SELECT * FROM unnest($1::uuid[],$2::text[],$3::text[]) AS scope(content_public_ref,platform,author_id)), \
         refs AS (SELECT lane.content_public_ref,lane.package_ref FROM linggan_material_lane_observation lane WHERE lane.content_public_ref=ANY($1) \
             UNION SELECT finding.content_public_ref,finding.package_ref FROM linggan_material_discovery_finding finding WHERE finding.content_public_ref=ANY($1) \
             UNION SELECT requested.content_public_ref,author.package_ref FROM requested JOIN linggan_material_author_profile author ON author.platform=requested.platform AND author.author_external_id=requested.author_id), \
         ranked AS (SELECT refs.content_public_ref,refs.package_ref,package.task_id,package.attempt_id,package.producer_instance_id,receipt.receipt_ref, \
             count(*) OVER(PARTITION BY refs.content_public_ref) AS total_count, \
             row_number() OVER(PARTITION BY refs.content_public_ref ORDER BY package.accepted_at,refs.package_ref) AS row_ordinal \
         FROM refs JOIN linggan_runtime_capture_package package ON package.package_ref=refs.package_ref \
         LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         WHERE package.accepted_at <= $4::timestamptz) SELECT * FROM ranked WHERE row_ordinal<=21 ORDER BY content_public_ref,row_ordinal"
    ).bind(&refs).bind(&platforms).bind(&authors).bind(as_of).fetch_all(&mut **tx).await?;
    let mut matches: HashMap<Uuid, Vec<String>> = HashMap::new();
    if let Some(text) = text {
        let comment_matches: Vec<Uuid> = sqlx::query_scalar(
            "WITH current_comment AS (SELECT DISTINCT ON(comment.content_public_ref,comment.comment_external_id) comment.content_public_ref,comment.body_text \
             FROM linggan_material_comment comment JOIN linggan_runtime_capture_package package USING(package_ref) \
             WHERE comment.content_public_ref=ANY($1) AND package.accepted_at <= $2::timestamptz \
             ORDER BY comment.content_public_ref,comment.comment_external_id,comment.observed_at::timestamptz DESC,comment.created_at DESC,comment.material_ref DESC) \
             SELECT DISTINCT content_public_ref FROM current_comment WHERE lower(COALESCE(body_text,'')) LIKE '%' || lower($3) || '%'"
        ).bind(&refs).bind(as_of).bind(text).fetch_all(&mut **tx).await?;
        for content_ref in comment_matches {
            matches
                .entry(content_ref)
                .or_default()
                .push("comment_body".to_owned());
        }
        for (content_ref,kind) in sqlx::query_as::<_,(Uuid,String)>(
            "SELECT DISTINCT content_public_ref,kind FROM linggan_material_derived_text WHERE content_public_ref=ANY($1) \
             AND created_at <= $2::timestamptz AND lower(text_content) LIKE '%' || lower($3) || '%'"
        ).bind(&refs).bind(as_of).bind(text).fetch_all(&mut **tx).await? {
            matches.entry(content_ref).or_default().push(kind);
        }
    }
    let mut latest = group_rows(latest);
    let mut histories = group_rows(histories);
    let contexts: HashMap<Uuid, PgRow> = contexts
        .into_iter()
        .map(|row| (row.get("content_public_ref"), row))
        .collect();
    let mut provenance_rows = group_rows(provenance_rows);
    for item in items.iter_mut() {
        let content_ref = item.identity.public_ref;
        let mut coverage = lane_coverage(item, latest.remove(&content_ref).unwrap_or_default());
        let mut coverage_history =
            coverage_history(&histories.remove(&content_ref).unwrap_or_default());
        let comments = read_comments(item, text);
        let comments_receipt = comment_receipt(counts.get(&content_ref).copied().unwrap_or(0));
        let author_context = author_context(item, contexts.get(&content_ref));
        let provenance = provenance(provenance_rows.remove(&content_ref).unwrap_or_default());
        for kind in matches.remove(&content_ref).unwrap_or_default() {
            let field = match kind.as_str() {
                "comment_body" => "comment_body",
                "ocr_text" => "ocr_text",
                "asr_text" => "asr_text",
                "frame_ocr_text" => "frame_ocr_text",
                _ => continue,
            };
            if !item.matched_fields.contains(&field) {
                item.matched_fields.push(field);
            }
        }
        if !author_context.is_null()
            && let Some(summary) = item
                .lane_summaries
                .iter_mut()
                .find(|summary| summary.lane == "author")
        {
            summary.state = "SEARCHABLE";
            summary.observed = Some(1);
            summary.retained = Some(1);
            summary.value_state = "KNOWN";
            summary.limitations = vec!["AUTHOR_PROFILE_IS_VERSIONED_CONTEXT"];
            summary.latest_observed_at = author_context
                .get("observedAt")
                .and_then(Value::as_str)
                .map(str::to_owned);
        }

        if let Some(inspector) = item.inspector.as_object_mut() {
            inspector.insert("commentThreads".to_owned(), Value::Array(comments));
            inspector.insert(
                "commentsAccess".to_owned(),
                serde_json::json!({
                    "accessLevel":"RESTRICTED_SOURCE",
                    "bodyReturned":false,
                    "externalIdentityReturned":false
                }),
            );
            inspector.insert(
                "commentsCoverage".to_owned(),
                coverage.remove("comments").unwrap_or(Value::Null),
            );
            inspector.insert(
                "commentsCoverageHistory".to_owned(),
                coverage_history
                    .remove("comments")
                    .unwrap_or_else(|| Value::Array(Vec::new())),
            );
            inspector.insert("commentsReceipt".to_owned(), comments_receipt);
            inspector.insert(
                "repliesCoverage".to_owned(),
                coverage.remove("replies").unwrap_or(Value::Null),
            );
            inspector.insert(
                "repliesCoverageHistory".to_owned(),
                coverage_history
                    .remove("replies")
                    .unwrap_or_else(|| Value::Array(Vec::new())),
            );
            inspector.insert("authorContext".to_owned(), author_context);
            // The collection context already wrote targetRefs and workOrderRefs into provenance
            // before this enrichment runs. Replacing the object wholesale dropped them, so the page
            // reported SOURCE INCOMPLETE for a target that is in fact linked -- and a reader used
            // that to conclude the authorization chain was broken. Merge instead of replace.
            match inspector
                .get_mut("provenance")
                .and_then(Value::as_object_mut)
            {
                Some(existing) => {
                    if let Value::Object(read) = provenance {
                        existing.extend(read);
                    }
                }
                None => {
                    inspector.insert("provenance".to_owned(), provenance);
                }
            }
        }
    }
    crate::material_evidence_fragment::read_batch(tx, items, text, as_of).await?;
    Ok(())
}

fn group_rows(rows: Vec<PgRow>) -> HashMap<Uuid, Vec<PgRow>> {
    let mut grouped = HashMap::new();
    for row in rows {
        grouped
            .entry(row.get("content_public_ref"))
            .or_insert_with(Vec::new)
            .push(row);
    }
    grouped
}

/// Historical lane evidence stays a list of individual package receipts. The projection must
/// never add a detail-window sample to an all-public-comments run, nor copy page counts between
/// attempts just to present a larger-looking total.
fn coverage_history(rows: &[PgRow]) -> serde_json::Map<String, Value> {
    let mut history = serde_json::Map::new();
    for lane in ["comments", "replies"] {
        let entries = rows
            .iter()
            .filter(|row| row.get::<String, _>("lane") == lane)
            .map(coverage_history_entry)
            .collect::<Vec<_>>();
        history.insert(lane.to_owned(), Value::Array(entries));
    }
    history
}

fn coverage_history_entry(row: &sqlx::postgres::PgRow) -> Value {
    let lane: String = row.get("lane");
    let retained: Option<i32> = row.get("retained");
    let producer_acquired: Option<i32> = row.get("producer_acquired");
    let failed: Option<i32> = row.get("failed");
    let known_unattempted: Option<i32> = row.get("known_unattempted");
    let unknown: Option<i32> = row.get("unknown_count");
    let stopped_reason: Option<String> = row.get("stopped_reason");
    let coverage: Value = row.get("coverage");
    let receipt = comment_collection_receipt(&coverage, &lane);
    let state = receipt.as_ref().map_or_else(
        || {
            lane_state(
                retained,
                producer_acquired,
                failed,
                known_unattempted,
                unknown,
                stopped_reason.as_deref(),
            )
        },
        |receipt| comment_collection_lane_state(receipt),
    );
    serde_json::json!({
        "lane":lane,"state":state,
        "observed":row.get::<Option<i32>,_>("observed"),
        "producerAcquired":producer_acquired,"retained":retained,"failed":failed,
        "knownUnattempted":known_unattempted,"unknown":unknown,
        "maximumQuota":row.get::<Option<i32>,_>("maximum_quota"),
        "stoppedReason":stopped_reason,
        "observedAt":row.get::<String,_>("observed_at"),
        "recordedAt":row.get::<String,_>("recorded_at"),
        "packageRef":row.get::<Uuid,_>("package_ref"),
        "taskRef":row.get::<Uuid,_>("task_id"),
        "attemptRef":row.get::<Option<Uuid>,_>("attempt_id"),
        "receiptRef":row.get::<Option<Uuid>,_>("receipt_ref"),
        "collectionScope":receipt.as_ref().and_then(|value| receipt_string(value,"scope")),
        "requestedLimit":receipt.as_ref().and_then(|value| receipt_count(value,"requestedLimit")),
        "pageCommentCount":receipt.as_ref().and_then(|value| receipt_count(value,"pageCommentCount")),
        "expectedCount":receipt.as_ref().and_then(|value| receipt_count(value,"expectedCount")),
        "uniqueCollectedCount":receipt.as_ref().and_then(|value| receipt_count(value,"uniqueCollectedCount")),
        "collectionState":receipt.as_ref().and_then(|value| receipt_string(value,"state")),
        "analysisUsability":receipt.as_ref().and_then(|value| receipt_string(value,"analysisUsability")),
        "targetIdentity":receipt.as_ref().and_then(|value| receipt_string(value,"targetIdentity"))
    })
}

fn comment_receipt(total: i64) -> Value {
    serde_json::json!({"total":total,"returned":0,"truncated":total>0,"nextCursor":Value::Null,"detailRequired":total>0})
}

fn lane_coverage(
    item: &mut MaterialLibraryItem,
    lane_rows: Vec<PgRow>,
) -> serde_json::Map<String, Value> {
    let mut coverage = serde_json::Map::new();
    for row in lane_rows {
        let lane: String = row.get("lane");
        let observed: Option<i32> = row.get("observed");
        let producer_acquired: Option<i32> = row.get("producer_acquired");
        let retained: Option<i32> = row.get("retained");
        let failed: Option<i32> = row.get("failed");
        let unattempted: Option<i32> = row.get("known_unattempted");
        let unknown: Option<i32> = row.get("unknown_count");
        let stopped_reason: Option<String> = row.get("stopped_reason");
        let package_coverage: Value = row.get("coverage");
        let receipt = comment_collection_receipt(&package_coverage, &lane);
        let state = lane_state(
            retained,
            producer_acquired,
            failed,
            unattempted,
            unknown,
            stopped_reason.as_deref(),
        );
        let state = receipt
            .as_ref()
            .map_or(state, |receipt| comment_collection_lane_state(receipt));
        if let Some(summary) = item
            .lane_summaries
            .iter_mut()
            .find(|summary| summary.lane == lane)
        {
            summary.state = state;
            summary.observed = observed.map(i64::from);
            summary.retained = retained.map(i64::from);
            summary.failed = failed.map(i64::from);
            summary.known_unattempted = unattempted.map(i64::from);
            summary.maximum_quota = row.get::<Option<i32>, _>("maximum_quota").map(i64::from);
            summary.requested_limit = receipt
                .as_ref()
                .and_then(|receipt| receipt_count(receipt, "requestedLimit"));
            summary.collection_scope = receipt
                .as_ref()
                .and_then(|receipt| receipt_string(receipt, "scope"));
            summary.expected_count = receipt
                .as_ref()
                .and_then(|receipt| receipt_count(receipt, "expectedCount"));
            summary.unique_collected_count = receipt
                .as_ref()
                .and_then(|receipt| receipt_count(receipt, "uniqueCollectedCount"));
            summary.collection_state = receipt
                .as_ref()
                .and_then(|receipt| receipt_string(receipt, "state"));
            summary.analysis_usability = receipt
                .as_ref()
                .and_then(|receipt| receipt_string(receipt, "analysisUsability"));
            summary.value_state = if receipt.is_some() {
                "KNOWN"
            } else if [
                observed,
                producer_acquired,
                retained,
                failed,
                unattempted,
                unknown,
            ]
            .iter()
            .all(Option::is_some)
                && retained == producer_acquired
            {
                "KNOWN"
            } else {
                "UNKNOWN"
            };
            summary.stopped_reason = stopped_reason.clone();
            summary.limitations = if retained != producer_acquired {
                vec!["RETAINED_COUNT_DIFFERS_FROM_PRODUCER_ACQUIRED"]
            } else if receipt
                .as_ref()
                .and_then(|receipt| receipt_string(receipt, "state"))
                .as_deref()
                != Some("complete")
            {
                vec!["COMMENT_COLLECTION_PARTIAL"]
            } else if unknown.unwrap_or(1) > 0 || unattempted.unwrap_or(1) > 0 {
                vec!["COVERAGE_NOT_EXHAUSTED"]
            } else {
                Vec::new()
            };
            summary.latest_observed_at = Some(row.get("observed_at"));
        }
        if lane == "comments" || lane == "replies" {
            let count_state = if receipt.is_some()
                || (retained == producer_acquired
                    && failed == Some(0)
                    && unattempted == Some(0)
                    && unknown == Some(0))
            {
                "KNOWN"
            } else {
                "UNKNOWN"
            };
            coverage.insert(
                lane,
                serde_json::json!({
                    "count": if count_state == "KNOWN" { retained } else { None },
                    "countState":count_state,
                    "observed":observed,"producerAcquired":producer_acquired,"retained":retained,"failed":failed,
                    "knownUnattempted":unattempted,"unknown":unknown,
                    "stoppedReason":stopped_reason,"sourceRef":row.get::<Uuid,_>("package_ref"),
                    "collectionScope":receipt.as_ref().and_then(|receipt| receipt_string(receipt,"scope")),
                    "requestedLimit":receipt.as_ref().and_then(|receipt| receipt_count(receipt,"requestedLimit")),
                    "pageCommentCount":receipt.as_ref().and_then(|receipt| receipt_count(receipt,"pageCommentCount")),
                    "expectedCount":receipt.as_ref().and_then(|receipt| receipt_count(receipt,"expectedCount")),
                    "uniqueCollectedCount":receipt.as_ref().and_then(|receipt| receipt_count(receipt,"uniqueCollectedCount")),
                    "collectionState":receipt.as_ref().and_then(|receipt| receipt_string(receipt,"state")),
                    "analysisUsability":receipt.as_ref().and_then(|receipt| receipt_string(receipt,"analysisUsability")),
                    "targetIdentity":receipt.as_ref().and_then(|receipt| receipt_string(receipt,"targetIdentity"))
                }),
            );
        }
    }
    coverage
}

fn comment_collection_receipt(coverage: &Value, lane: &str) -> Option<Value> {
    if lane != "comments" && lane != "replies" {
        return None;
    }
    let receipt = coverage.pointer("/target/commentCollection")?.as_object()?;
    let state = receipt.get("state")?.as_str()?;
    let scope = receipt.get("scope")?.as_str()?;
    let usability = receipt.get("analysisUsability")?.as_str()?;
    if !matches!(state, "complete" | "partial" | "invalid_target")
        || !matches!(scope, "detail_window" | "all_public_comments")
        || !matches!(usability, "usable" | "empty" | "not_usable")
    {
        return None;
    }
    Some(Value::Object(receipt.clone()))
}

fn receipt_string(receipt: &Value, key: &str) -> Option<String> {
    receipt.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn receipt_count(receipt: &Value, key: &str) -> Option<i64> {
    receipt
        .get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value >= 0)
}

fn comment_collection_lane_state(receipt: &Value) -> &'static str {
    match receipt.get("state").and_then(Value::as_str) {
        Some("complete") => "COMPLETE",
        Some("invalid_target") => "FAILED",
        Some("partial") => "PARTIAL",
        _ => "UNKNOWN",
    }
}

fn read_comments(_item: &mut MaterialLibraryItem, _text: Option<&str>) -> Vec<Value> {
    Vec::new()
}

fn author_context(item: &MaterialLibraryItem, row: Option<&PgRow>) -> Value {
    let Some(author_id) = item.author_external_id.as_deref() else {
        return Value::Null;
    };
    row.map_or(Value::Null, |row| serde_json::json!({
        "authorExternalId":author_id,"displayName":row.get::<Option<String>,_>("display_name"),
        "displayNameState":row.get::<String,_>("display_name_state"),"biographyState":row.get::<String,_>("biography_state"),
        "followerCount":row.get::<Option<i64>,_>("follower_count"),"followerCountState":row.get::<String,_>("follower_count_state"),
        "sourceRef":row.get::<Uuid,_>("material_ref"),"packageRef":row.get::<Uuid,_>("package_ref"),"observedAt":row.get::<String,_>("observed_at")
    }))
}

/// One row per package means a producer that produced two packages is listed twice. That reads
/// as duplicated data rather than as "the same station did both", so collapse it while keeping
/// acceptance order. The list is capped at 20 rows, so a linear scan is the right shape here.
fn dedupe_preserving_order(refs: impl Iterator<Item = Uuid>) -> Vec<Uuid> {
    let mut seen: Vec<Uuid> = Vec::new();
    for candidate in refs {
        if !seen.contains(&candidate) {
            seen.push(candidate);
        }
    }
    seen
}

fn provenance(mut rows: Vec<PgRow>) -> Value {
    let total = rows.first().map_or(0_i64, |row| row.get("total_count"));
    let truncated = rows.len() > 20;
    if truncated {
        rows.truncate(20);
    }
    let next_cursor = truncated
        .then(|| {
            rows.last()
                .map(|row| format!("package:{}", row.get::<Uuid, _>("package_ref")))
        })
        .flatten();
    serde_json::json!({
        "packageRefs":rows.iter().map(|row| row.get::<Uuid,_>("package_ref")).collect::<Vec<_>>(),
        "taskRefs":rows.iter().map(|row| row.get::<Uuid,_>("task_id")).collect::<Vec<_>>(),
        "attemptRefs":rows.iter().map(|row| row.get::<Uuid,_>("attempt_id")).collect::<Vec<_>>(),
        "receiptRefs":rows.iter().filter_map(|row| row.get::<Option<Uuid>,_>("receipt_ref")).collect::<Vec<_>>(),
        "producers":dedupe_preserving_order(rows.iter().map(|row| row.get::<Uuid,_>("producer_instance_id"))),
        "stationAccountLens":{"state":"UNKNOWN"},"coverageRefs":rows.iter().map(|row| row.get::<Uuid,_>("package_ref")).collect::<Vec<_>>(),
        "receipt":{"total":total,"returned":rows.len(),"truncated":truncated,"nextCursor":next_cursor,"limit":20}
    })
}

pub async fn read_authorized_research_comments(
    database: &Database,
    content_ref: Uuid,
    after: Option<Uuid>,
    text: Option<&str>,
) -> Result<Value, sqlx::Error> {
    let mut rows = sqlx::query(
        "SELECT comment.material_ref,comment.is_reply,left(comment.body_text,4000) AS body_text, \
             char_length(comment.body_text)>4000 AS body_truncated,comment.body_state,comment.observed_at, \
             count(*) OVER() AS total_count \
         FROM linggan_material_comment_current comment \
         JOIN linggan_runtime_capture_package package ON package.package_ref=comment.package_ref \
         WHERE comment.content_public_ref=$1 AND package.accepted_at <= scope_001_now() \
           AND NOT EXISTS (SELECT 1 FROM linggan_material_comment_restriction restriction \
                           WHERE restriction.content_public_ref=comment.content_public_ref \
                             AND restriction.comment_external_id=comment.comment_external_id) \
           AND ($2::uuid IS NULL OR comment.material_ref > $2) \
           AND ($3::text IS NULL OR lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($3) || '%') \
         ORDER BY comment.material_ref LIMIT 21",
    )
    .bind(content_ref)
    .bind(after)
    .bind(text)
    .fetch_all(database.pool())
    .await?;
    let total = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM linggan_material_comment_current comment \
         JOIN linggan_runtime_capture_package package ON package.package_ref=comment.package_ref \
         WHERE comment.content_public_ref=$1 AND package.accepted_at <= scope_001_now() \
           AND NOT EXISTS (SELECT 1 FROM linggan_material_comment_restriction restriction \
                           WHERE restriction.content_public_ref=comment.content_public_ref \
                             AND restriction.comment_external_id=comment.comment_external_id) \
           AND ($2::text IS NULL OR lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($2) || '%')",
    )
    .bind(content_ref)
    .bind(text)
    .fetch_one(database.pool())
    .await?;
    let truncated = rows.len() > 20;
    if truncated {
        rows.truncate(20);
    }
    let next_cursor = truncated
        .then(|| {
            rows.last()
                .map(|row| row.get::<Uuid, _>("material_ref").to_string())
        })
        .flatten();
    let items = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "sourceRef":row.get::<Uuid,_>("material_ref"),
                "relation":if row.get::<bool,_>("is_reply") { "REPLY" } else { "ROOT" },
                "body":row.get::<Option<String>,_>("body_text"),
                "bodyState":row.get::<String,_>("body_state"),
                "bodyTruncated":row.get::<Option<bool>,_>("body_truncated").unwrap_or(false),
                "anonymousAuthorContext":{"identity":"WITHHELD","platformUserIdReturned":false},
                "observedAt":row.get::<String,_>("observed_at")
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "channel":"comments","accessLevel":"LOCAL_AUTHORIZED_RESEARCH",
        "searchableOriginalText":true,"externalAuthorIdentityReturned":false,
        "query":text,
        "total":total,"returned":items.len(),"truncated":truncated,"nextCursor":next_cursor,
        "limit":20,"items":items
    }))
}

fn lane_state(
    retained: Option<i32>,
    producer_acquired: Option<i32>,
    failed: Option<i32>,
    unattempted: Option<i32>,
    unknown: Option<i32>,
    stopped_reason: Option<&str>,
) -> &'static str {
    if stopped_reason == Some("risk_control") {
        return "RISK_CONTROL";
    }
    if retained != producer_acquired {
        return "PARTIAL";
    }
    if retained.unwrap_or(0) > 0
        && (failed.unwrap_or(0) > 0 || unattempted.unwrap_or(0) > 0 || unknown.unwrap_or(0) > 0)
    {
        return "PARTIAL";
    }
    if retained.unwrap_or(0) > 0 {
        return "SEARCHABLE";
    }
    if failed.unwrap_or(0) > 0 {
        return "FAILED";
    }
    if unknown.unwrap_or(0) > 0 {
        return "UNKNOWN";
    }
    if retained == Some(0) {
        return "NOT_OBSERVED";
    }
    "NOT_OBSERVED"
}
