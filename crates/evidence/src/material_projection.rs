//! Typed material admission and read projection for already accepted Browser Producer records.
//!
//! This module never acquires platform data and never rewrites the immutable producer package.
//! It stores only typed, source-linked values whose identity is sufficient under the active
//! material contract; insufficient records remain retained/quarantined at ingress.

use linggan_contracts::EvidenceQuery;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLibraryProjection {
    pub items: Vec<MaterialLibraryItem>,
    pub query_scope: &'static str,
    pub as_of: String,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLibraryItem {
    pub identity: MaterialIdentity,
    pub display: MaterialDisplay,
    pub preview: MaterialPreview,
    pub lane_summaries: Vec<MaterialLaneSummary>,
    pub summary: MaterialSummary,
    pub inspector: Value,
    pub matched_fields: Vec<&'static str>,
    #[serde(skip)]
    author_external_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialIdentity {
    pub platform: String,
    pub content_external_id: String,
    pub public_ref: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDisplay {
    pub title: Option<String>,
    pub title_state: String,
    pub creator_display_name: Option<String>,
    pub creator_state: String,
    pub published_at: Option<String>,
    pub published_at_source_text: Option<String>,
    pub published_at_state: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialPreview {
    pub local_asset_url: Option<String>,
    pub slot_purpose: Option<String>,
    pub bytes_state: &'static str,
    pub alt: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLaneSummary {
    pub lane: &'static str,
    pub state: &'static str,
    pub observed: Option<i64>,
    pub retained: Option<i64>,
    pub failed: Option<i64>,
    pub known_unattempted: Option<i64>,
    pub maximum_quota: Option<i64>,
    pub value_state: &'static str,
    pub stopped_reason: Option<String>,
    pub limitations: Vec<&'static str>,
    pub latest_observed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSummary {
    pub last_observed_at: String,
    pub primary_limitation: &'static str,
    pub restriction_state: &'static str,
}

pub async fn read_material_library(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<MaterialLibraryProjection, sqlx::Error> {
    let text = query.text().filter(|value| !value.trim().is_empty());
    let lane = query.lane().map(|value| value.as_str());
    let lane_state = query.lane_state().map(|value| value.as_str());
    let rows = sqlx::query(
        "WITH latest_detail AS ( \
           SELECT DISTINCT ON (detail.content_public_ref) \
             detail.content_public_ref,detail.material_ref,detail.package_ref,detail.record_ordinal,detail.observed_at, \
             detail.title,detail.title_state,detail.body_text,detail.body_state, \
             detail.creator_display_name,detail.creator_display_name_state, \
             detail.published_at_source_text,detail.published_at_source_text_state,detail.author_external_id \
           FROM linggan_material_content_detail detail \
           ORDER BY detail.content_public_ref,detail.observed_at DESC,detail.created_at DESC \
         ), latest_lane AS ( \
           SELECT content_public_ref,max(observed_at) AS observed_at \
           FROM linggan_material_lane_observation WHERE content_public_ref IS NOT NULL GROUP BY content_public_ref \
         ) SELECT content.platform,content.content_external_id,content.public_ref, \
             detail.material_ref,detail.package_ref,detail.record_ordinal, \
             COALESCE(detail.observed_at,lane_latest.observed_at) AS observed_at, \
             detail.title,COALESCE(detail.title_state,'UNKNOWN') AS title_state,detail.body_text, \
             COALESCE(detail.body_state,'UNKNOWN') AS body_state,detail.creator_display_name, \
             COALESCE(detail.creator_display_name_state,'UNKNOWN') AS creator_display_name_state, \
             detail.published_at_source_text,COALESCE(detail.published_at_source_text_state,'UNKNOWN') AS published_at_source_text_state, \
             detail.author_external_id \
         FROM linggan_material_content content \
         LEFT JOIN latest_detail detail ON detail.content_public_ref = content.public_ref \
         LEFT JOIN latest_lane lane_latest ON lane_latest.content_public_ref = content.public_ref \
         WHERE ($1::text IS NULL \
           OR lower(COALESCE(detail.title,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(detail.body_text,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(detail.creator_display_name,'')) LIKE '%' || lower($1) || '%' \
           OR EXISTS (SELECT 1 FROM linggan_material_comment comment WHERE comment.content_public_ref=content.public_ref AND lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($1) || '%') \
           OR EXISTS (SELECT 1 FROM linggan_material_author_profile author WHERE author.platform=content.platform AND author.author_external_id=detail.author_external_id AND (lower(COALESCE(author.display_name,'')) LIKE '%' || lower($1) || '%' OR lower(COALESCE(author.biography,'')) LIKE '%' || lower($1) || '%'))) \
           AND ($2::text IS NULL OR EXISTS (SELECT 1 FROM linggan_material_lane_observation lane WHERE lane.content_public_ref=content.public_ref AND lane.lane=$2)) \
           AND ($3::text IS NULL OR $3 = 'SEARCHABLE') \
           AND COALESCE(detail.observed_at,lane_latest.observed_at) IS NOT NULL \
         ORDER BY COALESCE(detail.observed_at,lane_latest.observed_at) DESC,content.platform,content.content_external_id",
    )
    .bind(text)
    .bind(lane)
    .bind(lane_state)
    .fetch_all(database.pool())
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let mut item = material_item(row, text);
        enrich_social_material(database, &mut item, text).await?;
        items.push(item);
    }
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(database.pool())
        .await?;
    Ok(MaterialLibraryProjection {
        items,
        query_scope: "accepted_typed_material_text_only",
        as_of,
        cursor: None,
    })
}

pub async fn material_projection_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    let tables_exist = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('linggan_material_content') IS NOT NULL \
                AND to_regclass('linggan_material_content_detail') IS NOT NULL \
                AND to_regclass('linggan_material_lane_observation') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !tables_exist {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                        WHERE migration_id = '0016_material_social_lanes')",
    )
    .fetch_one(database.pool())
    .await
}

fn material_item(row: sqlx::postgres::PgRow, text: Option<&str>) -> MaterialLibraryItem {
    let title: Option<String> = row.get("title");
    let body: Option<String> = row.get("body_text");
    let creator: Option<String> = row.get("creator_display_name");
    let observed_at: String = row.get("observed_at");
    let material_ref: Option<Uuid> = row.get("material_ref");
    let package_ref: Option<Uuid> = row.get("package_ref");
    let record_ordinal: Option<i32> = row.get("record_ordinal");
    let matched_fields = match text {
        None => Vec::new(),
        Some(text) => {
            let needle = text.to_lowercase();
            [
                ("title", title.as_deref()),
                ("detail_body", body.as_deref()),
                ("creator", creator.as_deref()),
            ]
            .into_iter()
            .filter_map(|(field, value)| {
                value
                    .is_some_and(|value| value.to_lowercase().contains(&needle))
                    .then_some(field)
            })
            .collect()
        }
    };
    MaterialLibraryItem {
        identity: MaterialIdentity {
            platform: row.get("platform"),
            content_external_id: row.get("content_external_id"),
            public_ref: row.get("public_ref"),
        },
        display: MaterialDisplay {
            title,
            title_state: row.get("title_state"),
            creator_display_name: creator,
            creator_state: row.get("creator_display_name_state"),
            published_at: None,
            published_at_source_text: row.get("published_at_source_text"),
            // A source string is not promoted to an exact instant until a producer contract
            // guarantees its encoding. Preserve the source text while keeping time unknown.
            published_at_state: "UNKNOWN".to_owned(),
        },
        preview: MaterialPreview {
            local_asset_url: None,
            slot_purpose: None,
            bytes_state: "UNKNOWN",
            alt: "没有已验证的本地媒体副本".to_owned(),
        },
        lane_summaries: lane_summaries(&observed_at),
        summary: MaterialSummary {
            last_observed_at: observed_at.clone(),
            primary_limitation: "OTHER_LANES_NOT_EVALUATED",
            restriction_state: "UNKNOWN",
        },
        inspector: serde_json::json!({
            "overview": {
                "fields": [
                    {"field":"title","state":row.get::<String,_>("title_state"),"sourceRefs":[material_ref]},
                    {"field":"body","state":row.get::<String,_>("body_state"),"value":null,"accessLevel":"RESTRICTED_SOURCE","sourceRefs":[material_ref]},
                    {"field":"creator","state":row.get::<String,_>("creator_display_name_state"),"sourceRefs":[material_ref]}
                ]
            },
            "commentThreads": [],
            "mediaSlots": [],
            "derivatives": [],
            "provenance": {
                "packageRefs":package_ref.into_iter().collect::<Vec<_>>(),
                "recordRefs":[{"packageRef":package_ref,"recordOrdinal":record_ordinal}],
                "coverageRefs":package_ref.into_iter().collect::<Vec<_>>()
            },
            "displayPolicy":"MINIMUM_NECESSARY",
            "limitations":["COMMENTS_NOT_EVALUATED","MEDIA_NOT_EVALUATED","RAW_BODY_NOT_RETURNED"]
        }),
        matched_fields,
        author_external_id: row.get("author_external_id"),
    }
}

async fn enrich_social_material(
    database: &Database,
    item: &mut MaterialLibraryItem,
    text: Option<&str>,
) -> Result<(), sqlx::Error> {
    let content_ref = item.identity.public_ref;
    let lane_rows = sqlx::query(
        "SELECT DISTINCT ON (lane) lane,observed,retained,failed,known_unattempted,unknown_count,maximum_quota,stopped_reason,observed_at,package_ref \
         FROM linggan_material_lane_observation WHERE content_public_ref=$1 \
         ORDER BY lane,created_at DESC",
    )
    .bind(content_ref)
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
        let state = social_lane_state(
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

    let comment_rows = sqlx::query(
        "SELECT DISTINCT ON (comment_external_id) material_ref,package_ref,comment_external_id,root_comment_external_id,parent_comment_external_id,parent_identity_source_field,is_reply,body_text,body_state,author_external_id,author_display_name,observed_at \
         FROM linggan_material_comment WHERE content_public_ref=$1 \
         ORDER BY comment_external_id,created_at DESC",
    )
    .bind(content_ref)
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

    let author_context = match item.author_external_id.as_deref() {
        None => Value::Null,
        Some(author_id) => sqlx::query(
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
        })),
    };

    let provenance_rows = sqlx::query(
        "SELECT lane.package_ref,package.task_id,package.attempt_id,package.producer_instance_id,receipt.receipt_ref \
         FROM linggan_material_lane_observation lane \
         JOIN linggan_runtime_capture_package package ON package.package_ref=lane.package_ref \
         LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         WHERE lane.content_public_ref=$1 ORDER BY lane.created_at",
    )
    .bind(content_ref)
    .fetch_all(database.pool())
    .await?;
    let provenance = serde_json::json!({
        "packageRefs":provenance_rows.iter().map(|row| row.get::<Uuid,_>("package_ref")).collect::<Vec<_>>(),
        "taskRefs":provenance_rows.iter().map(|row| row.get::<Uuid,_>("task_id")).collect::<Vec<_>>(),
        "attemptRefs":provenance_rows.iter().map(|row| row.get::<Uuid,_>("attempt_id")).collect::<Vec<_>>(),
        "receiptRefs":provenance_rows.iter().filter_map(|row| row.get::<Option<Uuid>,_>("receipt_ref")).collect::<Vec<_>>(),
        "producers":provenance_rows.iter().map(|row| row.get::<Uuid,_>("producer_instance_id")).collect::<Vec<_>>(),
        "stationAccountLens":{"state":"UNKNOWN"},"coverageRefs":provenance_rows.iter().map(|row| row.get::<Uuid,_>("package_ref")).collect::<Vec<_>>()
    });
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

fn social_lane_state(
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

fn lane_summaries(observed_at: &str) -> Vec<MaterialLaneSummary> {
    const LANES: &[&str] = &[
        "discovery",
        "detail",
        "comments",
        "replies",
        "author",
        "media_slots",
        "media_bytes",
        "ocr",
        "asr",
    ];
    LANES
        .iter()
        .map(|lane| MaterialLaneSummary {
            lane,
            state: if *lane == "detail" {
                "SEARCHABLE"
            } else {
                "UNKNOWN"
            },
            observed: (*lane == "detail").then_some(1),
            retained: (*lane == "detail").then_some(1),
            failed: None,
            known_unattempted: None,
            maximum_quota: None,
            value_state: if *lane == "detail" {
                "KNOWN"
            } else {
                "UNKNOWN"
            },
            stopped_reason: None,
            limitations: if *lane == "detail" {
                vec!["RAW_BODY_NOT_RETURNED"]
            } else {
                vec!["LANE_NOT_EVALUATED"]
            },
            latest_observed_at: (*lane == "detail").then(|| observed_at.to_owned()),
        })
        .collect()
}
