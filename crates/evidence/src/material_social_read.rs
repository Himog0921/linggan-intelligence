//! Read-only discussion, author-context, coverage, and provenance enrichment.

use crate::material_projection_types::MaterialLibraryItem;
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn enrich(
    database: &Database,
    item: &mut MaterialLibraryItem,
    text: Option<&str>,
) -> Result<(), sqlx::Error> {
    let mut coverage = read_lane_coverage(database, item).await?;
    let comments = read_comments(database, item, text).await?;
    let author_context = read_author_context(database, item).await?;
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

    let provenance = read_provenance(database, item).await?;
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert("commentThreads".to_owned(), Value::Array(comments));
        inspector.insert(
            "commentsCoverage".to_owned(),
            coverage.remove("comments").unwrap_or(Value::Null),
        );
        inspector.insert(
            "repliesCoverage".to_owned(),
            coverage.remove("replies").unwrap_or(Value::Null),
        );
        inspector.insert("authorContext".to_owned(), author_context);
        inspector.insert("provenance".to_owned(), provenance);
    }
    Ok(())
}

async fn read_lane_coverage(
    database: &Database,
    item: &mut MaterialLibraryItem,
) -> Result<serde_json::Map<String, Value>, sqlx::Error> {
    let lane_rows = sqlx::query(
        "SELECT DISTINCT ON (lane) lane,observed,retained,failed,known_unattempted,unknown_count,maximum_quota,stopped_reason,observed_at,package_ref \
         FROM linggan_material_lane_observation WHERE content_public_ref=$1 \
         ORDER BY lane,created_at DESC",
    )
    .bind(item.identity.public_ref)
    .fetch_all(database.pool())
    .await?;
    let mut coverage = serde_json::Map::new();
    for row in lane_rows {
        let lane: String = row.get("lane");
        let observed: Option<i32> = row.get("observed");
        let retained: Option<i32> = row.get("retained");
        let failed: Option<i32> = row.get("failed");
        let unattempted: Option<i32> = row.get("known_unattempted");
        let unknown: Option<i32> = row.get("unknown_count");
        let stopped_reason: Option<String> = row.get("stopped_reason");
        let state = lane_state(
            retained,
            failed,
            unattempted,
            unknown,
            stopped_reason.as_deref(),
        );
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
            summary.value_state = if [observed, retained, failed, unattempted, unknown]
                .iter()
                .all(Option::is_some)
            {
                "KNOWN"
            } else {
                "UNKNOWN"
            };
            summary.stopped_reason = stopped_reason.clone();
            summary.limitations = if unknown.unwrap_or(1) > 0 || unattempted.unwrap_or(1) > 0 {
                vec!["COVERAGE_NOT_EXHAUSTED"]
            } else {
                Vec::new()
            };
            summary.latest_observed_at = Some(row.get("observed_at"));
        }
        if lane == "comments" || lane == "replies" {
            let count_state = if failed == Some(0) && unattempted == Some(0) && unknown == Some(0) {
                "KNOWN"
            } else {
                "UNKNOWN"
            };
            coverage.insert(
                lane,
                serde_json::json!({
                    "count": if count_state == "KNOWN" { retained } else { None },
                    "countState":count_state,
                    "observed":observed,"retained":retained,"failed":failed,
                    "knownUnattempted":unattempted,"unknown":unknown,
                    "stoppedReason":stopped_reason,"sourceRef":row.get::<Uuid,_>("package_ref")
                }),
            );
        }
    }
    Ok(coverage)
}

async fn read_comments(
    database: &Database,
    item: &mut MaterialLibraryItem,
    text: Option<&str>,
) -> Result<Vec<Value>, sqlx::Error> {
    let comment_rows = sqlx::query(
        "SELECT DISTINCT ON (comment_external_id) material_ref,package_ref,comment_external_id,root_comment_external_id,parent_comment_external_id,parent_identity_source_field,is_reply,body_text,body_state,author_external_id,author_display_name,observed_at \
         FROM linggan_material_comment WHERE content_public_ref=$1 \
         ORDER BY comment_external_id,created_at DESC",
    )
    .bind(item.identity.public_ref)
    .fetch_all(database.pool())
    .await?;
    let needle = text.map(str::to_lowercase);
    let mut comments = Vec::with_capacity(comment_rows.len());
    let mut comment_match = false;
    for row in comment_rows {
        let body: Option<String> = row.get("body_text");
        comment_match |= needle.as_ref().is_some_and(|needle| {
            body.as_ref()
                .is_some_and(|body| body.to_lowercase().contains(needle))
        });
        comments.push(serde_json::json!({
            "commentExternalId":row.get::<String,_>("comment_external_id"),
            "rootCommentExternalId":row.get::<String,_>("root_comment_external_id"),
            "parentCommentExternalId":row.get::<Option<String>,_>("parent_comment_external_id"),
            "parentIdentitySourceField":row.get::<Option<String>,_>("parent_identity_source_field"),
            "isReply":row.get::<bool,_>("is_reply"),
            "snippet":body.as_deref().map(redacted_snippet),"bodyState":row.get::<String,_>("body_state"),
            "accessLevel":"REDACTED_SNIPPET","sourceRef":row.get::<Uuid,_>("material_ref"),
            "packageRef":row.get::<Uuid,_>("package_ref"),"observedAt":row.get::<String,_>("observed_at")
        }));
    }
    if comment_match && !item.matched_fields.contains(&"comments") {
        item.matched_fields.push("comments");
    }
    Ok(comments)
}

async fn read_author_context(
    database: &Database,
    item: &MaterialLibraryItem,
) -> Result<Value, sqlx::Error> {
    let Some(author_id) = item.author_external_id.as_deref() else {
        return Ok(Value::Null);
    };
    Ok(sqlx::query(
        "SELECT material_ref,package_ref,observed_at,display_name,display_name_state,biography_state,follower_count,follower_count_state \
         FROM linggan_material_author_profile WHERE platform=$1 AND author_external_id=$2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&item.identity.platform)
    .bind(author_id)
    .fetch_optional(database.pool())
    .await?
    .map_or(Value::Null, |row| serde_json::json!({
        "authorExternalId":author_id,"displayName":row.get::<Option<String>,_>("display_name"),
        "displayNameState":row.get::<String,_>("display_name_state"),"biographyState":row.get::<String,_>("biography_state"),
        "followerCount":row.get::<Option<i64>,_>("follower_count"),"followerCountState":row.get::<String,_>("follower_count_state"),
        "sourceRef":row.get::<Uuid,_>("material_ref"),"packageRef":row.get::<Uuid,_>("package_ref"),"observedAt":row.get::<String,_>("observed_at")
    })))
}

async fn read_provenance(
    database: &Database,
    item: &MaterialLibraryItem,
) -> Result<Value, sqlx::Error> {
    let rows = sqlx::query(
        "WITH refs AS (SELECT lane.package_ref FROM linggan_material_lane_observation lane WHERE lane.content_public_ref=$1 \
             UNION SELECT finding.package_ref FROM linggan_material_discovery_finding finding WHERE finding.content_public_ref=$1 \
             UNION SELECT author.package_ref FROM linggan_material_author_profile author WHERE author.platform=$2 AND author.author_external_id=$3) \
         SELECT refs.package_ref,package.task_id,package.attempt_id,package.producer_instance_id,receipt.receipt_ref \
         FROM refs JOIN linggan_runtime_capture_package package ON package.package_ref=refs.package_ref \
         LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         ORDER BY package.accepted_at",
    )
    .bind(item.identity.public_ref)
    .bind(&item.identity.platform)
    .bind(item.author_external_id.as_deref())
    .fetch_all(database.pool())
    .await?;
    Ok(serde_json::json!({
        "packageRefs":rows.iter().map(|row| row.get::<Uuid,_>("package_ref")).collect::<Vec<_>>(),
        "taskRefs":rows.iter().map(|row| row.get::<Uuid,_>("task_id")).collect::<Vec<_>>(),
        "attemptRefs":rows.iter().map(|row| row.get::<Uuid,_>("attempt_id")).collect::<Vec<_>>(),
        "receiptRefs":rows.iter().filter_map(|row| row.get::<Option<Uuid>,_>("receipt_ref")).collect::<Vec<_>>(),
        "producers":rows.iter().map(|row| row.get::<Uuid,_>("producer_instance_id")).collect::<Vec<_>>(),
        "stationAccountLens":{"state":"UNKNOWN"},"coverageRefs":rows.iter().map(|row| row.get::<Uuid,_>("package_ref")).collect::<Vec<_>>()
    }))
}

fn lane_state(
    retained: Option<i32>,
    failed: Option<i32>,
    unattempted: Option<i32>,
    unknown: Option<i32>,
    stopped_reason: Option<&str>,
) -> &'static str {
    if stopped_reason == Some("risk_control") {
        return "RISK_CONTROL";
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
    "NOT_OBSERVED"
}

fn redacted_snippet(value: &str) -> String {
    let redacted = value
        .split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.contains("token") || lower.contains("secret") || lower.contains("cookie") {
                "[REDACTED]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let mut snippet = redacted.chars().take(120).collect::<String>();
    if value.chars().count() > 120 {
        snippet.push('…');
    }
    snippet
}
