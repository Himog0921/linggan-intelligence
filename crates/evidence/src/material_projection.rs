//! Typed material admission and read projection for already accepted Browser Producer records.
//!
//! This module never acquires platform data and never rewrites the immutable producer package.
//! It stores only typed, source-linked values whose identity is sufficient under the active
//! material contract; insufficient records remain retained/quarantined at ingress.

use crate::material_cursor;
use crate::material_media_read;
use crate::material_projection_types::default_lane_summaries;
pub use crate::material_projection_types::{
    MaterialCollectionContext, MaterialDisplay, MaterialEngagement, MaterialIdentity,
    MaterialLibraryItem, MaterialLibraryProjection, MaterialPreview, MaterialSummary,
};
use crate::material_social_read;
use linggan_contracts::{EvidenceQuery, EvidenceQuerySort};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const MATERIAL_PAGE_SIZE: usize = 50;
const MATERIAL_SCAN_BUDGET: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum MaterialReadError {
    #[error("material cursor is invalid or does not match this query")]
    InvalidCursor,
    #[error("material sort is not supported by the current projection")]
    UnsupportedSort,
    #[error("the material projection schema cannot satisfy this query")]
    ProjectionUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn read_material_library(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<MaterialLibraryProjection, MaterialReadError> {
    if query.sort() != EvidenceQuerySort::LatestDiscovery {
        return Err(MaterialReadError::UnsupportedSort);
    }
    let cursor = query
        .cursor()
        .map(|value| material_cursor::decode(query, value).ok_or(MaterialReadError::InvalidCursor))
        .transpose()?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of = match &cursor {
        Some(cursor) => cursor.as_of.clone(),
        None => {
            sqlx::query_scalar("SELECT scope_001_now()::text")
                .fetch_one(&mut *tx)
                .await?
        }
    };
    validate_cursor_times(&mut tx, cursor.as_ref(), &as_of).await?;
    if query.published_window().is_some() {
        tx.commit().await?;
        return Ok(MaterialLibraryProjection {
            items: Vec::new(),
            query_scope: "accepted_typed_material_strict_published_time",
            as_of,
            cursor: None,
            truncated: false,
            scan_limited: false,
            scanned_count: 0,
        });
    }
    read_latest_material_page(tx, query, cursor, as_of).await
}

async fn read_latest_material_page(
    mut tx: Transaction<'_, Postgres>,
    query: &EvidenceQuery,
    cursor: Option<material_cursor::MaterialCursor>,
    as_of: String,
) -> Result<MaterialLibraryProjection, MaterialReadError> {
    let text = query.text().filter(|value| !value.trim().is_empty());
    let mut scan_observed_at = cursor
        .as_ref()
        .map(|cursor| cursor.last_observed_at.clone());
    let mut scan_platform = cursor.as_ref().map(|cursor| cursor.last_platform.clone());
    let mut scan_content_external_id = cursor
        .as_ref()
        .map(|cursor| cursor.last_content_external_id.clone());
    let mut items = Vec::with_capacity(MATERIAL_PAGE_SIZE + 1);
    let mut scanned_count = 0;
    let mut scan_limited = false;
    'scan: loop {
        let rows = sqlx::query(crate::material_query_sql::MATERIAL_PAGE_SQL)
            .bind(text)
            .bind(&as_of)
            .bind(scan_observed_at.as_deref())
            .bind(scan_platform.as_deref())
            .bind(scan_content_external_id.as_deref())
            .bind(query.lane().map(|lane| lane.as_str()))
            .bind(query.media_kind().map(|kind| kind.as_purpose()))
            .bind(None::<Uuid>)
            .fetch_all(&mut *tx)
            .await?;
        let exhausted = rows.len() < MATERIAL_PAGE_SIZE + 1;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            if scanned_count == MATERIAL_SCAN_BUDGET {
                scan_limited = true;
                break 'scan;
            }
            scan_observed_at = Some(row.get("observed_at"));
            scan_platform = Some(row.get("platform"));
            scan_content_external_id = Some(row.get("content_external_id"));
            scanned_count += 1;
            let mut item = material_item(row, text);
            enrich_discovery_material(&mut tx, &mut item, &as_of).await?;
            material_social_read::enrich(&mut tx, &mut item, text, &as_of).await?;
            enrich_media_material(&mut tx, &mut item, &as_of).await?;
            if item_matches_filters(&item, query) {
                items.push(item);
                if items.len() > MATERIAL_PAGE_SIZE {
                    break;
                }
            }
        }
        if items.len() > MATERIAL_PAGE_SIZE || exhausted {
            break;
        }
    }
    let page_overflow = items.len() > MATERIAL_PAGE_SIZE;
    if page_overflow {
        items.truncate(MATERIAL_PAGE_SIZE);
    }
    let next_cursor = if page_overflow {
        items.last().map(|item| {
            material_cursor::encode(
                query,
                material_cursor::for_last_item(
                    query,
                    as_of.clone(),
                    item.summary.last_observed_at.clone(),
                    item.identity.platform.clone(),
                    item.identity.content_external_id.clone(),
                ),
            )
        })
    } else if scan_limited {
        scan_observed_at
            .zip(scan_platform)
            .zip(scan_content_external_id)
            .map(|((observed_at, platform), content_external_id)| {
                material_cursor::encode(
                    query,
                    material_cursor::for_last_item(
                        query,
                        as_of.clone(),
                        observed_at,
                        platform,
                        content_external_id,
                    ),
                )
            })
    } else {
        None
    };
    tx.commit().await?;
    Ok(MaterialLibraryProjection {
        items,
        query_scope: "accepted_typed_material_text_only",
        as_of,
        cursor: next_cursor,
        truncated: page_overflow || scan_limited,
        scan_limited,
        scanned_count,
    })
}

async fn validate_cursor_times(
    tx: &mut Transaction<'_, Postgres>,
    cursor: Option<&material_cursor::MaterialCursor>,
    as_of: &str,
) -> Result<(), MaterialReadError> {
    let as_of_is_valid: bool = sqlx::query_scalar(
        "SELECT CASE WHEN pg_input_is_valid($1, 'timestamp with time zone') \
         THEN $1::timestamptz <= scope_001_now() ELSE false END",
    )
    .bind(as_of)
    .fetch_one(&mut **tx)
    .await?;
    let key_is_valid = match cursor {
        None => true,
        Some(cursor) => {
            sqlx::query_scalar("SELECT pg_input_is_valid($1, 'timestamp with time zone')")
                .bind(&cursor.last_observed_at)
                .fetch_one(&mut **tx)
                .await?
        }
    };
    if as_of_is_valid && key_is_valid {
        Ok(())
    } else {
        Err(MaterialReadError::InvalidCursor)
    }
}

pub(crate) async fn enrich_discovery_material(
    tx: &mut Transaction<'_, Postgres>,
    item: &mut MaterialLibraryItem,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let row=sqlx::query("SELECT finding.material_ref,finding.package_ref,finding.discovery_kind,finding.result_position,finding.observed_at,package.coverage,package.task_id,package.attempt_id,task.task_spec,receipt.receipt_ref FROM linggan_material_discovery_finding finding JOIN linggan_runtime_capture_package package USING(package_ref) JOIN linggan_runtime_task task ON task.task_id=package.task_id LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref WHERE finding.content_public_ref=$1 AND package.accepted_at <= $2::timestamptz ORDER BY finding.observed_at::timestamptz DESC,finding.created_at DESC LIMIT 1")
        .bind(item.identity.public_ref).bind(as_of).fetch_optional(&mut **tx).await?;
    let Some(row) = row else {
        return Ok(());
    };
    let coverage: Value = row.get("coverage");
    let kind: String = row.get("discovery_kind");
    let layer = crate::material_contract_validation::unique_coverage_layer(&coverage, &kind);
    let count = |field| {
        layer
            .and_then(|value| value.get(field))
            .and_then(Value::as_i64)
    };
    let stopped = layer
        .and_then(|value| value.get("stoppedReason"))
        .and_then(Value::as_str);
    let retained: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_discovery_finding WHERE package_ref=$1",
    )
    .bind(row.get::<Uuid, _>("package_ref"))
    .fetch_one(&mut **tx)
    .await?;
    let reconciled = count("acquired") == Some(retained);
    let state = if stopped == Some("risk_control") {
        "RISK_CONTROL"
    } else if !reconciled
        || count("unknown").unwrap_or(1) > 0
        || count("notAttempted").unwrap_or(0) > 0
    {
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
        summary.observed = count("observed");
        summary.retained = Some(retained);
        summary.failed = count("failed");
        summary.known_unattempted = count("notAttempted");
        summary.maximum_quota = row
            .get::<Value, _>("task_spec")
            .get("maximumQuota")
            .and_then(Value::as_i64);
        summary.value_state = if reconciled { "KNOWN" } else { "UNKNOWN" };
        summary.stopped_reason = stopped.map(str::to_owned);
        summary.limitations = if !reconciled {
            vec!["RETAINED_COUNT_DIFFERS_FROM_PRODUCER_ACQUIRED"]
        } else if state == "PARTIAL" {
            vec!["DISCOVERY_SURFACE_NOT_EXHAUSTED"]
        } else {
            Vec::new()
        };
        summary.latest_observed_at = Some(row.get("observed_at"));
    }
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert("discovery".to_owned(),serde_json::json!({"kind":row.get::<String,_>("discovery_kind"),"resultPosition":row.get::<Option<i32>,_>("result_position"),"sourceRef":row.get::<Uuid,_>("material_ref"),"packageRef":row.get::<Uuid,_>("package_ref"),"coverage":coverage}));
        if let Some(provenance) = inspector
            .get_mut("provenance")
            .and_then(Value::as_object_mut)
        {
            provenance.insert(
                "taskRefs".to_owned(),
                serde_json::json!([row.get::<Uuid, _>("task_id")]),
            );
            provenance.insert(
                "attemptRefs".to_owned(),
                serde_json::json!([row.get::<Uuid, _>("attempt_id")]),
            );
            provenance.insert(
                "receiptRefs".to_owned(),
                serde_json::json!(
                    row.get::<Option<Uuid>, _>("receipt_ref")
                        .into_iter()
                        .collect::<Vec<_>>()
                ),
            );
        }
    }
    enrich_collection_context(tx, item, &kind, row.get("task_id"), row.get("task_spec")).await?;
    Ok(())
}

async fn enrich_collection_context(
    tx: &mut Transaction<'_, Postgres>,
    item: &mut MaterialLibraryItem,
    discovery_kind: &str,
    task_id: Uuid,
    task_spec: Value,
) -> Result<(), sqlx::Error> {
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('collection_observation_target') IS NOT NULL \
         AND to_regclass('collection_work_order') IS NOT NULL \
         AND to_regclass('collection_work_order_lease') IS NOT NULL \
         AND to_regclass('collection_work_order_lease_task') IS NOT NULL",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !schema_ready {
        return Ok(());
    }
    let target_kind = task_spec
        .pointer("/target/authorExternalId")
        .and_then(Value::as_str)
        .map(|_| "creator")
        .or_else(|| {
            task_spec
                .pointer("/target/query")
                .and_then(Value::as_str)
                .map(|_| "keyword")
        });
    let identity_key = task_spec
        .pointer("/target/authorExternalId")
        .or_else(|| task_spec.pointer("/target/query"))
        .and_then(Value::as_str);
    let row = sqlx::query(
        "SELECT target.target_ref,target.target_kind,target.identity_key,target.display_name,work_order.work_order_ref, \
                (lease_task.task_id IS NOT NULL) AS task_linked \
         FROM collection_observation_target target \
         LEFT JOIN collection_work_order work_order ON work_order.target_ref=target.target_ref \
         LEFT JOIN collection_work_order_lease lease ON lease.work_order_ref=work_order.work_order_ref \
         LEFT JOIN collection_work_order_lease_task lease_task ON lease_task.lease_ref=lease.lease_ref AND lease_task.task_id=$1 \
         WHERE lease_task.task_id=$1 OR (target.platform=$2 AND target.target_kind=$3 AND target.identity_key=$4) \
         ORDER BY (lease_task.task_id IS NOT NULL) DESC,work_order.created_at DESC NULLS LAST,target.first_stored_at DESC LIMIT 1",
    )
    .bind(task_id)
    .bind(&item.identity.platform)
    .bind(target_kind)
    .bind(identity_key)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else {
        return Ok(());
    };
    let target_kind: String = row.get("target_kind");
    let target_identity: String = row.get("identity_key");
    let target_display_name: Option<String> = row.get("display_name");
    let target_ref: Uuid = row.get("target_ref");
    let work_order_ref: Option<Uuid> = row.get("work_order_ref");
    item.collection_context.relationship_state =
        if discovery_kind == "profile_discovery" && target_kind == "creator" {
            "OBSERVED_ON_TARGET_SURFACE".to_owned()
        } else {
            "DISCOVERED_FOR_TARGET".to_owned()
        };
    item.collection_context.target_ref = Some(target_ref);
    item.collection_context.target_kind = Some(target_kind.clone());
    item.collection_context.target_display_state = if target_display_name.is_some() {
        "KNOWN".to_owned()
    } else {
        "UNKNOWN".to_owned()
    };
    item.collection_context.target_display_name = target_display_name;
    item.collection_context.author_identity_match_state =
        match item.author_external_id.as_deref() {
            Some(author_external_id)
                if target_kind == "creator" && author_external_id == target_identity =>
            {
                "MATCHED"
            }
            Some(_) if target_kind == "creator" => "MISMATCH",
            _ if target_kind == "creator" => "NOT_VERIFIED",
            _ => "NOT_APPLICABLE",
        }
        .to_owned();
    item.collection_context.work_order_ref = work_order_ref;
    if let Some(provenance) = item
        .inspector
        .pointer_mut("/provenance")
        .and_then(Value::as_object_mut)
    {
        provenance.insert("targetRefs".to_owned(), serde_json::json!([target_ref]));
        provenance.insert(
            "workOrderRefs".to_owned(),
            serde_json::json!(work_order_ref.into_iter().collect::<Vec<_>>()),
        );
    }
    Ok(())
}

pub(crate) async fn enrich_media_material(
    tx: &mut Transaction<'_, Postgres>,
    item: &mut MaterialLibraryItem,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let media = material_media_read::read(tx, item.identity.public_ref, as_of).await?;
    item.preview.local_asset_url = media.preview_url;
    item.preview.slot_purpose = media.preview_purpose;
    item.preview.bytes_state = media.bytes_state;
    item.media = media.resource;
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
            if (lane == "media_slots" && !media.slots.is_empty())
                || (lane != "media_slots" && state != "UNKNOWN")
            {
                summary.state = state;
                summary.value_state = "KNOWN";
            }
            if lane == "media_slots" && !media.slots.is_empty() {
                summary.observed = Some(media.slots.len() as i64);
                summary.retained = Some(media.slots.len() as i64);
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
        inspector.insert("mediaSlotsReceipt".to_owned(), media.slot_receipt);
        inspector.insert("derivativesReceipt".to_owned(), media.derivative_receipt);
        inspector.insert("permissions".to_owned(),serde_json::json!({"displayPolicy":"MINIMUM_NECESSARY","remoteCandidateUris":"NOT_EXPOSED"}));
        inspector.insert(
            "limitations".to_owned(),
            serde_json::to_value(media.limitations).expect("limitations serialize"),
        );
    }
    Ok(())
}

fn item_matches_filters(item: &MaterialLibraryItem, query: &EvidenceQuery) -> bool {
    if let Some(lane) = query.lane()
        && !item.lane_summaries.iter().any(|summary| {
            summary.lane == lane.as_str()
                && (summary.state != "UNKNOWN" || summary.value_state != "UNKNOWN")
        })
    {
        return false;
    }
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
                AND to_regclass('linggan_material_discovery_finding') IS NOT NULL \
                AND to_regclass('linggan_material_comment_current') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !tables_exist {
        return Ok(false);
    }
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                        WHERE migration_id = '0025_comment_current_projection') \
                AND EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                            WHERE migration_id = '0026_work_resource_read') \
                AND EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                            WHERE migration_id = '0027_unified_media_resource') \
                AND EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                            WHERE migration_id = '0030_comment_image_media') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='linggan_material_content_detail' \
                              AND column_name='published_at_source_kind')",
    )
    .fetch_one(database.pool())
    .await
}

pub(crate) fn material_item(row: sqlx::postgres::PgRow, text: Option<&str>) -> MaterialLibraryItem {
    let title: Option<String> = row.get("title");
    let body: Option<String> = row.get("body_text");
    let creator: Option<String> = row.get("creator_display_name");
    let published_at: Option<String> = row.get("published_at");
    let published_at_source_text: Option<String> = row.get("published_at_source_text");
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
            published_at: published_at.clone(),
            published_at_source_text: published_at_source_text.clone(),
            published_at_state: if published_at.is_some() {
                "KNOWN"
            } else if published_at_source_text.is_some() {
                "SOURCE_TEXT_ONLY"
            } else {
                "UNKNOWN"
            }
            .to_owned(),
            published_at_source_field: row.get("published_at_source_field"),
            published_at_source_kind: row.get("published_at_source_kind"),
            published_at_precision: row.get("published_at_precision"),
            published_at_reference_observed_at: row.get("published_at_reference_observed_at"),
            published_at_parser_version: row.get("published_at_parser_version"),
            engagement: MaterialEngagement {
                like_count: row.get("like_count"),
                like_count_state: row.get("like_count_state"),
                comment_count: row.get("comment_count"),
                comment_count_state: row.get("comment_count_state"),
                collect_count: row.get("collect_count"),
                collect_count_state: row.get("collect_count_state"),
                share_count: row.get("share_count"),
                share_count_state: row.get("share_count_state"),
            },
        },
        collection_context: MaterialCollectionContext {
            relationship_state: "UNKNOWN".to_owned(),
            target_ref: None,
            target_kind: None,
            target_display_name: None,
            target_display_state: "UNKNOWN".to_owned(),
            author_identity_match_state: "NOT_VERIFIED".to_owned(),
            work_order_ref: None,
        },
        preview: MaterialPreview {
            local_asset_url: None,
            observed_source_state: row.get("cover_source_state"),
            slot_purpose: None,
            bytes_state: "UNKNOWN",
            alt: "没有已验证的本地媒体副本".to_owned(),
        },
        media: serde_json::json!({
            "contractVersion":"linggan.media-resource.v1",
            "state":"NOT_OBSERVED",
            "avatar":{"state":"NOT_OBSERVED","relationship":"author.avatar"},
            "cover":{"state":"NOT_OBSERVED","relationship":"content.cover","selectedBy":"none","localAssetUrl":null},
            "images":[],
            "video":{"state":"NOT_OBSERVED","relationship":"content.video"},
            "ocr":{"state":"NOT_OBSERVED","relationship":"content.ocr","resources":[]},
            "transcript":{"state":"NOT_OBSERVED","relationship":"content.transcript","resources":[]},
            "commentImages":{"state":"NOT_OBSERVED","relationship":"comment.image","items":[]}
        }),
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
