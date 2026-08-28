//! Typed material admission and read projection for already accepted Browser Producer records.
//!
//! This module never acquires platform data and never rewrites the immutable producer package.
//! It stores only typed, source-linked values whose identity is sufficient under the active
//! material contract; insufficient records remain retained/quarantined at ingress.

use crate::material_media_read;
use crate::material_projection_types::default_lane_summaries;
pub use crate::material_projection_types::{
    MaterialDisplay, MaterialIdentity, MaterialLaneSummary, MaterialLibraryItem,
    MaterialLibraryProjection, MaterialPreview, MaterialSummary,
};
use crate::material_social_read;
use linggan_contracts::EvidenceQuery;
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

pub async fn read_material_library(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<MaterialLibraryProjection, sqlx::Error> {
    if query.published_window().is_some() {
        let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(database.pool())
            .await?;
        return Ok(MaterialLibraryProjection {
            items: Vec::new(),
            query_scope: "accepted_typed_material_strict_published_time",
            as_of,
            cursor: None,
        });
    }
    let text = query.text().filter(|value| !value.trim().is_empty());
    let lane = query.lane().map(|value| value.as_str());
    let rows = sqlx::query(
        "WITH latest_detail AS ( \
           SELECT DISTINCT ON (detail.content_public_ref) \
             detail.content_public_ref,detail.material_ref,detail.package_ref,detail.record_ordinal,detail.observed_at, \
             detail.title,detail.title_state,detail.body_text,detail.body_state, \
             detail.creator_display_name,detail.creator_display_name_state, \
             detail.published_at_source_text,detail.published_at_source_text_state,detail.author_external_id \
           FROM linggan_material_content_detail detail \
           ORDER BY detail.content_public_ref,detail.observed_at DESC,detail.created_at DESC \
         ), latest_discovery AS ( \
           SELECT DISTINCT ON (finding.content_public_ref) finding.* FROM linggan_material_discovery_finding finding \
           ORDER BY finding.content_public_ref,finding.created_at DESC \
         ), latest_lane AS ( \
           SELECT content_public_ref,max(observed_at) AS observed_at \
           FROM linggan_material_lane_observation WHERE content_public_ref IS NOT NULL GROUP BY content_public_ref \
         ) SELECT content.platform,content.content_external_id,content.public_ref, \
             detail.material_ref AS detail_material_ref,COALESCE(detail.material_ref,discovery.material_ref) AS material_ref,COALESCE(detail.package_ref,discovery.package_ref) AS package_ref,COALESCE(detail.record_ordinal,discovery.record_ordinal) AS record_ordinal, \
             COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) AS observed_at, \
             COALESCE(detail.title,discovery.title) AS title,CASE WHEN detail.title IS NOT NULL THEN detail.title_state ELSE COALESCE(discovery.title_state,'UNKNOWN') END AS title_state,detail.body_text, \
             COALESCE(detail.body_state,'UNKNOWN') AS body_state,COALESCE(detail.creator_display_name,discovery.creator_display_name) AS creator_display_name, \
             CASE WHEN detail.creator_display_name IS NOT NULL THEN detail.creator_display_name_state ELSE COALESCE(discovery.creator_state,'UNKNOWN') END AS creator_display_name_state, \
             COALESCE(detail.published_at_source_text,discovery.published_at_source_text) AS published_at_source_text,CASE WHEN detail.published_at_source_text IS NOT NULL THEN detail.published_at_source_text_state ELSE COALESCE(discovery.published_at_source_text_state,'UNKNOWN') END AS published_at_source_text_state, \
             detail.author_external_id \
         FROM linggan_material_content content \
         LEFT JOIN latest_detail detail ON detail.content_public_ref = content.public_ref \
         LEFT JOIN latest_discovery discovery ON discovery.content_public_ref = content.public_ref \
         LEFT JOIN latest_lane lane_latest ON lane_latest.content_public_ref = content.public_ref \
         WHERE ($1::text IS NULL \
           OR lower(COALESCE(detail.title,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(detail.body_text,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(detail.creator_display_name,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(discovery.title,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(discovery.creator_display_name,'')) LIKE '%' || lower($1) || '%' \
           OR EXISTS (SELECT 1 FROM linggan_material_comment comment WHERE comment.content_public_ref=content.public_ref AND lower(COALESCE(comment.body_text,'')) LIKE '%' || lower($1) || '%') \
           OR EXISTS (SELECT 1 FROM linggan_material_author_profile author WHERE author.platform=content.platform AND author.author_external_id=detail.author_external_id AND (lower(COALESCE(author.display_name,'')) LIKE '%' || lower($1) || '%' OR lower(COALESCE(author.biography,'')) LIKE '%' || lower($1) || '%'))) \
           AND ($2::text IS NULL OR ($2='discovery' AND discovery.material_ref IS NOT NULL) OR EXISTS (SELECT 1 FROM linggan_material_lane_observation lane WHERE lane.content_public_ref=content.public_ref AND lane.lane=$2)) \
           AND COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) IS NOT NULL \
         ORDER BY COALESCE(detail.observed_at,discovery.observed_at,lane_latest.observed_at) DESC,content.platform,content.content_external_id",
    )
    .bind(text)
    .bind(lane)
    .fetch_all(database.pool())
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let mut item = material_item(row, text);
        enrich_discovery_material(database, &mut item).await?;
        material_social_read::enrich(database, &mut item, text).await?;
        enrich_media_material(database, &mut item).await?;
        if item_matches_filters(&item, query) {
            items.push(item);
        }
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

async fn enrich_discovery_material(
    database: &Database,
    item: &mut MaterialLibraryItem,
) -> Result<(), sqlx::Error> {
    let row=sqlx::query("SELECT finding.material_ref,finding.package_ref,finding.discovery_kind,finding.result_position,finding.observed_at,package.coverage,task.task_spec FROM linggan_material_discovery_finding finding JOIN linggan_runtime_capture_package package USING(package_ref) JOIN linggan_runtime_task task ON task.task_id=package.task_id WHERE finding.content_public_ref=$1 ORDER BY finding.created_at DESC LIMIT 1")
        .bind(item.identity.public_ref).fetch_optional(database.pool()).await?;
    let Some(row) = row else {
        return Ok(());
    };
    let coverage: Value = row.get("coverage");
    let layer = coverage.pointer("/layers/0");
    let count = |field| {
        layer
            .and_then(|value| value.get(field))
            .and_then(Value::as_i64)
    };
    let stopped = layer
        .and_then(|value| value.get("stoppedReason"))
        .and_then(Value::as_str);
    let state = if stopped == Some("risk_control") {
        "RISK_CONTROL"
    } else if count("unknown").unwrap_or(1) > 0 || count("notAttempted").unwrap_or(0) > 0 {
        "PARTIAL"
    } else {
        "OBSERVED"
    };
    if let Some(summary) = item
        .lane_summaries
        .iter_mut()
        .find(|summary| summary.lane == "discovery")
    {
        summary.state = state;
        summary.observed = Some(1);
        summary.retained = Some(1);
        summary.failed = count("failed");
        summary.known_unattempted = count("notAttempted");
        summary.maximum_quota = row
            .get::<Value, _>("task_spec")
            .get("maximumQuota")
            .and_then(Value::as_i64);
        summary.value_state = "KNOWN";
        summary.stopped_reason = stopped.map(str::to_owned);
        summary.limitations = if state == "PARTIAL" {
            vec!["DISCOVERY_SURFACE_NOT_EXHAUSTED"]
        } else {
            Vec::new()
        };
        summary.latest_observed_at = Some(row.get("observed_at"));
    }
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert("discovery".to_owned(),serde_json::json!({"kind":row.get::<String,_>("discovery_kind"),"resultPosition":row.get::<Option<i32>,_>("result_position"),"sourceRef":row.get::<Uuid,_>("material_ref"),"packageRef":row.get::<Uuid,_>("package_ref"),"coverage":coverage}));
    }
    Ok(())
}

async fn enrich_media_material(
    database: &Database,
    item: &mut MaterialLibraryItem,
) -> Result<(), sqlx::Error> {
    let media = material_media_read::read(database, item.identity.public_ref).await?;
    item.preview.local_asset_url = media.preview_url;
    item.preview.slot_purpose = media.preview_purpose;
    item.preview.bytes_state = media.bytes_state;
    if item.preview.local_asset_url.is_some() {
        item.preview.alt = "已验证的本地媒体副本".to_owned();
    }
    for (lane, state) in [
        (
            "media_slots",
            if media.slots.is_empty() {
                "UNKNOWN"
            } else if media.slots.iter().any(|slot| {
                slot.pointer("/components/bundleState")
                    .and_then(Value::as_str)
                    == Some("PARTIAL")
            }) {
                "PARTIAL"
            } else {
                "OBSERVED"
            },
        ),
        ("media_bytes", media.bytes_state),
        ("ocr", media.ocr_state),
        ("asr", media.asr_state),
    ] {
        if let Some(summary) = item
            .lane_summaries
            .iter_mut()
            .find(|summary| summary.lane == lane)
        {
            summary.state = state;
            if lane == "media_slots" {
                summary.observed = Some(media.slots.len() as i64);
                summary.retained = Some(media.slots.len() as i64);
                summary.value_state = "KNOWN";
            }
        }
    }
    item.summary.restriction_state = media.restriction_state;
    if let Some(first) = media.limitations.first() {
        item.summary.primary_limitation = first;
    }
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert("mediaSlots".to_owned(), Value::Array(media.slots));
        inspector.insert("derivatives".to_owned(), Value::Array(media.derivatives));
        inspector.insert("permissions".to_owned(),serde_json::json!({"displayPolicy":"MINIMUM_NECESSARY","remoteCandidateUris":"NOT_EXPOSED"}));
        inspector.insert(
            "limitations".to_owned(),
            serde_json::to_value(media.limitations).expect("limitations serialize"),
        );
    }
    Ok(())
}

fn item_matches_filters(item: &MaterialLibraryItem, query: &EvidenceQuery) -> bool {
    if let Some(state) = query.lane_state() {
        let lane = query.lane().map(|lane| lane.as_str());
        if !item.lane_summaries.iter().any(|summary| {
            lane.is_none_or(|lane| summary.lane == lane) && summary.state == state.as_str()
        }) {
            return false;
        }
    }
    if let Some(kind) = query.media_kind()
        && !item
            .inspector
            .pointer("/mediaSlots")
            .and_then(Value::as_array)
            .is_some_and(|slots| {
                slots.iter().any(|slot| {
                    slot.get("purpose").and_then(Value::as_str) == Some(kind.as_purpose())
                })
            })
    {
        return false;
    }
    if let Some(restriction) = query.restriction()
        && item.summary.restriction_state != restriction.as_str()
    {
        return false;
    }
    true
}

pub async fn material_projection_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    let tables_exist = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('linggan_material_content') IS NOT NULL \
                AND to_regclass('linggan_material_content_detail') IS NOT NULL \
                AND to_regclass('linggan_material_lane_observation') IS NOT NULL \
                AND to_regclass('linggan_material_media_origin') IS NOT NULL \
                AND to_regclass('linggan_material_discovery_finding') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !tables_exist {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                        WHERE migration_id = '0018_material_discovery_lane')",
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
        lane_summaries: default_lane_summaries(
            &observed_at,
            row.get::<Option<Uuid>, _>("detail_material_ref").is_some(),
        ),
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
