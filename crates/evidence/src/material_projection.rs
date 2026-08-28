//! Typed material admission and read projection for already accepted Browser Producer records.
//!
//! This module never acquires platform data and never rewrites the immutable producer package.
//! It stores only typed, source-linked values whose identity is sufficient under the active
//! material contract; insufficient records remain retained/quarantined at ingress.

use crate::producer_runtime::ProducerRuntimeError;
use linggan_contracts::{EvidenceQuery, ProducerCapturePackage};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
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
        "WITH latest AS ( \
           SELECT DISTINCT ON (detail.content_public_ref) \
             content.platform,content.content_external_id,content.public_ref, \
             detail.material_ref,detail.package_ref,detail.record_ordinal,detail.observed_at, \
             detail.title,detail.title_state,detail.body_text,detail.body_state, \
             detail.creator_display_name,detail.creator_display_name_state, \
             detail.published_at_source_text,detail.published_at_source_text_state \
           FROM linggan_material_content_detail detail \
           JOIN linggan_material_content content ON content.public_ref = detail.content_public_ref \
           ORDER BY detail.content_public_ref,detail.observed_at DESC,detail.created_at DESC \
         ) SELECT * FROM latest \
         WHERE ($1::text IS NULL \
           OR lower(COALESCE(title,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(body_text,'')) LIKE '%' || lower($1) || '%' \
           OR lower(COALESCE(creator_display_name,'')) LIKE '%' || lower($1) || '%') \
           AND ($2::text IS NULL OR $2 = 'detail') \
           AND ($3::text IS NULL OR $3 = 'SEARCHABLE') \
         ORDER BY observed_at DESC,platform,content_external_id",
    )
    .bind(text)
    .bind(lane)
    .bind(lane_state)
    .fetch_all(database.pool())
    .await?;
    let items = rows
        .into_iter()
        .map(|row| material_item(row, text))
        .collect();
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
                AND to_regclass('linggan_material_content_detail') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !tables_exist {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                        WHERE migration_id = '0015_material_projection')",
    )
    .fetch_one(database.pool())
    .await
}

fn material_item(row: sqlx::postgres::PgRow, text: Option<&str>) -> MaterialLibraryItem {
    let title: Option<String> = row.get("title");
    let body: Option<String> = row.get("body_text");
    let creator: Option<String> = row.get("creator_display_name");
    let observed_at: String = row.get("observed_at");
    let material_ref: Uuid = row.get("material_ref");
    let package_ref: Uuid = row.get("package_ref");
    let record_ordinal: i32 = row.get("record_ordinal");
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
                "packageRefs":[package_ref],
                "recordRefs":[{"packageRef":package_ref,"recordOrdinal":record_ordinal}],
                "coverageRefs":[package_ref]
            },
            "displayPolicy":"MINIMUM_NECESSARY",
            "limitations":["COMMENTS_NOT_EVALUATED","MEDIA_NOT_EVALUATED","RAW_BODY_NOT_RETURNED"]
        }),
        matched_fields,
    }
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

pub(crate) async fn insert_typed_materials(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
) -> Result<(), ProducerRuntimeError> {
    if package.package_kind() != "content_detail" {
        return Ok(());
    }
    for (ordinal, record) in package.records().iter().enumerate() {
        let Some(detail) = content_detail_record(record) else {
            continue;
        };
        let expected_content_id = package
            .coverage()
            .pointer("/target/contentExternalId")
            .and_then(Value::as_str);
        if expected_content_id != Some(detail.content_id) {
            continue;
        }
        let content_public_ref = ensure_content(tx, package, detail.content_id).await?;
        insert_content_detail(tx, package, ordinal, detail, content_public_ref).await?;
    }
    Ok(())
}

async fn ensure_content(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    content_id: &str,
) -> Result<Uuid, ProducerRuntimeError> {
    sqlx::query(
        "INSERT INTO linggan_material_content \
         (platform,content_external_id,public_ref,first_package_ref) VALUES ($1,$2,$3,$4) \
         ON CONFLICT (platform,content_external_id) DO NOTHING",
    )
    .bind(package.platform())
    .bind(content_id)
    .bind(Uuid::new_v4())
    .bind(package.package_ref())
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform = $1 AND content_external_id = $2",
    )
    .bind(package.platform())
    .bind(content_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)
}

async fn insert_content_detail(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    ordinal: usize,
    detail: ContentDetailRecord<'_>,
    content_public_ref: Uuid,
) -> Result<(), ProducerRuntimeError> {
    let title = exact_string(detail.payload, "title");
    let body = exact_string(detail.payload, "bodyText");
    let creator = exact_string(detail.payload, "authorName");
    let published_at = exact_scalar_text(detail.payload, "publishedAtText");
    let searchable_text = [title, body]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    sqlx::query(
        "INSERT INTO linggan_material_content_detail \
         (material_ref,content_public_ref,package_ref,record_ordinal,observed_at, \
          title,title_state,body_text,body_state,creator_display_name,creator_display_name_state, \
          published_at_source_text,published_at_source_text_state,searchable_text) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
    )
    .bind(Uuid::new_v4())
    .bind(content_public_ref)
    .bind(package.package_ref())
    .bind(i32::try_from(ordinal).expect("package record count is bounded"))
    .bind(package.observed_at())
    .bind(title)
    .bind(known_state(title))
    .bind(body)
    .bind(known_state(body))
    .bind(creator)
    .bind(known_state(creator))
    .bind(published_at.as_deref())
    .bind(known_state(published_at.as_deref()))
    .bind(searchable_text)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

struct ContentDetailRecord<'a> {
    content_id: &'a str,
    payload: &'a serde_json::Map<String, Value>,
}

fn content_detail_record(record: &Value) -> Option<ContentDetailRecord<'_>> {
    if record.get("kind")?.as_str()? != "content_detail" {
        return None;
    }
    let content_id = record.pointer("/sourceObject/externalId")?.as_str()?.trim();
    let payload = record.get("payload")?.as_object()?;
    if content_id.is_empty() {
        return None;
    }
    Some(ContentDetailRecord {
        content_id,
        payload,
    })
}

fn exact_string<'a>(payload: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a str> {
    payload.get(key).and_then(Value::as_str)
}

fn exact_scalar_text(payload: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    payload.get(key).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn known_state<T>(value: Option<T>) -> &'static str {
    if value.is_some() { "KNOWN" } else { "UNKNOWN" }
}
