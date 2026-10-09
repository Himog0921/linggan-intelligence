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
use crate::work_resource_current::{
    WorkResourceCurrent, WorkResourceCurrentPageQuery, WorkResourceCurrentSource,
    read_work_resource_current_page,
};
use linggan_contracts::{EvidenceQuery, EvidenceQuerySort};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{AssertSqlSafe, postgres::PgRow};
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashMap;
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
    if let Some(refs) = query.public_refs() {
        if refs.is_empty()
            || refs.len() > 100
            || query.cursor().is_some()
            || query.creator_scope().is_some()
            || query.domain_ref().is_none()
            || query.text().is_some_and(|text| !text.trim().is_empty())
            || query.published_window().is_some()
        {
            return Err(MaterialReadError::InvalidCursor);
        }
        if query.sort() != EvidenceQuerySort::LatestDiscovery {
            return Err(MaterialReadError::UnsupportedSort);
        }
        return read_resource_batch(database, query, refs).await;
    }
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

/// Validate the parts of a work-resource query that do not depend on the material projection
/// tables. Corpus uses this when the domain registry is unavailable so malformed cursors still
/// receive a client error instead of being hidden by an unrelated schema-read failure.
pub async fn validate_material_query(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<(), MaterialReadError> {
    if query.sort() != EvidenceQuerySort::LatestDiscovery {
        return Err(MaterialReadError::UnsupportedSort);
    }
    let Some(cursor) = query
        .cursor()
        .map(|value| material_cursor::decode(query, value).ok_or(MaterialReadError::InvalidCursor))
        .transpose()?
    else {
        return Ok(());
    };
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_cursor_times(&mut tx, Some(&cursor), &cursor.as_of).await?;
    tx.commit().await?;
    Ok(())
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
    let creator_refs = if let Some(scope) = query.creator_scope() {
        if Some(scope.domain) != query.domain_ref() || scope.validate().is_err() {
            return Err(MaterialReadError::InvalidCursor);
        }
        let data = crate::creator_discovery::load_in(&mut tx, scope)
            .await
            .map_err(|error| match error {
                crate::creator_discovery::DiscoveryError::Database(e) => {
                    MaterialReadError::Database(e)
                }
                _ => MaterialReadError::InvalidCursor,
            })?;
        Some(crate::creator_discovery::matching_work_refs(&data, scope))
    } else {
        None
    };
    let mut items = Vec::with_capacity(MATERIAL_PAGE_SIZE + 1);
    let mut scanned_count = 0;
    let mut scan_limited = false;
    'scan: loop {
        let rows = read_work_resource_current_page(
            &mut tx,
            WorkResourceCurrentPageQuery {
                text,
                as_of: &as_of,
                observed_before: scan_observed_at.as_deref(),
                platform_after: scan_platform.as_deref(),
                content_external_id_after: scan_content_external_id.as_deref(),
                lane: query.lane().map(|lane| lane.as_str()),
                media_kind: query.media_kind().map(|kind| kind.as_purpose()),
                one_public_ref: None,
                domain_ref: query.domain_ref(),
                allowed_refs: creator_refs.as_deref(),
            },
        )
        .await?;
        let exhausted = rows.len() < MATERIAL_PAGE_SIZE + 1;
        if rows.is_empty() {
            break;
        }
        let remaining = MATERIAL_SCAN_BUDGET - scanned_count;
        let reached_budget = rows.len() > remaining;
        let rows: Vec<_> = rows.into_iter().take(remaining).collect();
        let mut batch = enrich_current_batch(&mut tx, &rows, text, &as_of).await?;
        for (row, item) in rows.iter().zip(batch.drain(..)) {
            scan_observed_at = Some(row.observed_at.clone());
            scan_platform = Some(row.platform.clone());
            scan_content_external_id = Some(row.content_external_id.clone());
            scanned_count += 1;
            if item_matches_filters(&item, query) {
                items.push(item);
                if items.len() > MATERIAL_PAGE_SIZE {
                    break;
                }
            }
        }
        if reached_budget && items.len() <= MATERIAL_PAGE_SIZE {
            scan_limited = true;
            break 'scan;
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
    author_attribution_source: Option<&str>,
) -> Result<(), sqlx::Error> {
    enrich_discovery_batch(
        tx,
        std::slice::from_mut(item),
        as_of,
        &[author_attribution_source],
    )
    .await
}

async fn enrich_discovery_batch(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    as_of: &str,
    author_sources: &[Option<&str>],
) -> Result<(), sqlx::Error> {
    if items.is_empty() {
        return Ok(());
    }
    let refs: Vec<Uuid> = items.iter().map(|item| item.identity.public_ref).collect();
    let rows = sqlx::query(
        "WITH latest AS MATERIALIZED (SELECT DISTINCT ON(finding.content_public_ref) finding.content_public_ref,finding.material_ref,finding.package_ref,finding.discovery_kind,finding.result_position,finding.observed_at,package.coverage,package.task_id,package.attempt_id,task.task_spec,receipt.receipt_ref \
         FROM linggan_material_discovery_finding finding JOIN linggan_runtime_capture_package package USING(package_ref) \
         JOIN linggan_runtime_task task ON task.task_id=package.task_id \
         LEFT JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
         WHERE finding.content_public_ref=ANY($1) AND package.accepted_at <= $2::timestamptz \
         ORDER BY finding.content_public_ref,finding.observed_at::timestamptz DESC,finding.created_at DESC), \
         retained AS (SELECT package_ref,count(*) AS retained_count FROM linggan_material_discovery_finding \
             WHERE package_ref IN(SELECT package_ref FROM latest) GROUP BY package_ref) \
         SELECT latest.*,retained.retained_count FROM latest JOIN retained USING(package_ref)"
    ).bind(&refs).bind(as_of).fetch_all(&mut **tx).await?;
    let rows: HashMap<Uuid, PgRow> = rows
        .into_iter()
        .map(|row| (row.get("content_public_ref"), row))
        .collect();
    for item in items.iter_mut() {
        if let Some(row) = rows.get(&item.identity.public_ref) {
            apply_discovery(item, row);
        }
    }
    enrich_collection_context_batch(tx, items, author_sources, &rows).await
}

fn apply_discovery(item: &mut MaterialLibraryItem, row: &PgRow) {
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
    let retained: i64 = row.get("retained_count");
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
}

async fn enrich_collection_context_batch(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    author_sources: &[Option<&str>],
    discoveries: &HashMap<Uuid, PgRow>,
) -> Result<(), sqlx::Error> {
    if discoveries.is_empty() {
        return Ok(());
    }
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

    let mut refs = Vec::new();
    let mut tasks = Vec::new();
    let mut platforms = Vec::new();
    let mut kinds = Vec::new();
    let mut identities = Vec::new();
    for item in items.iter() {
        let Some(row) = discoveries.get(&item.identity.public_ref) else {
            continue;
        };
        let spec: Value = row.get("task_spec");
        refs.push(item.identity.public_ref);
        tasks.push(row.get::<Uuid, _>("task_id"));
        platforms.push(item.identity.platform.clone());
        kinds.push(
            spec.pointer("/target/authorExternalId")
                .and_then(Value::as_str)
                .map(|_| "creator")
                .or_else(|| {
                    spec.pointer("/target/query")
                        .and_then(Value::as_str)
                        .map(|_| "keyword")
                }),
        );
        identities.push(
            spec.pointer("/target/authorExternalId")
                .or_else(|| spec.pointer("/target/query"))
                .and_then(Value::as_str)
                .map(str::to_owned),
        );
    }
    let contexts = sqlx::query(
        "WITH requested AS (SELECT * FROM unnest($1::uuid[],$2::uuid[],$3::text[],$4::text[],$5::text[]) AS scope(content_public_ref,task_id,platform,target_kind,identity_key)) \
         SELECT requested.content_public_ref,context.* FROM requested CROSS JOIN LATERAL ( \
         SELECT target.target_ref,target.target_kind,target.identity_key,target.display_name,work_order.work_order_ref, \
                (lease_task.task_id IS NOT NULL) AS task_linked \
         FROM collection_observation_target target \
         LEFT JOIN collection_work_order work_order ON work_order.target_ref=target.target_ref \
         LEFT JOIN collection_work_order_lease lease ON lease.work_order_ref=work_order.work_order_ref \
         LEFT JOIN collection_work_order_lease_task lease_task ON lease_task.lease_ref=lease.lease_ref AND lease_task.task_id=requested.task_id \
         WHERE lease_task.task_id=requested.task_id OR (target.platform=requested.platform AND target.target_kind=requested.target_kind AND target.identity_key=requested.identity_key) \
         ORDER BY (lease_task.task_id IS NOT NULL) DESC,work_order.created_at DESC NULLS LAST,target.first_stored_at DESC LIMIT 1) context"
    ).bind(&refs).bind(&tasks).bind(&platforms).bind(&kinds).bind(&identities).fetch_all(&mut **tx).await?;
    let contexts: HashMap<Uuid, PgRow> = contexts
        .into_iter()
        .map(|row| (row.get("content_public_ref"), row))
        .collect();
    for (item, source) in items.iter_mut().zip(author_sources.iter().copied()) {
        if let (Some(context), Some(discovery)) = (
            contexts.get(&item.identity.public_ref),
            discoveries.get(&item.identity.public_ref),
        ) {
            let kind: String = discovery.get("discovery_kind");
            apply_collection_context(item, context, &kind, source);
        }
    }
    Ok(())
}

fn apply_collection_context(
    item: &mut MaterialLibraryItem,
    row: &PgRow,
    discovery_kind: &str,
    author_attribution_source: Option<&str>,
) {
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
    item.collection_context.author_identity_match_state = match (
        target_kind.as_str(),
        author_attribution_source,
        item.author_external_id.as_deref(),
    ) {
        ("creator", Some("content_detail"), Some(author)) if author == target_identity.as_str() => {
            "MATCHED"
        }
        ("creator", Some("content_detail"), Some(_)) => "MISMATCH",
        ("creator", _, _) => "NOT_VERIFIED",
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
}

pub(crate) async fn enrich_media_material(
    tx: &mut Transaction<'_, Postgres>,
    item: &mut MaterialLibraryItem,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    enrich_media_batch(tx, std::slice::from_mut(item), as_of).await
}

async fn enrich_media_batch(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    as_of: &str,
) -> Result<(), sqlx::Error> {
    if items.is_empty() {
        return Ok(());
    }
    let refs: Vec<Uuid> = items.iter().map(|item| item.identity.public_ref).collect();
    let mut media = material_media_read::read_batch(tx, &refs, as_of).await?;
    apply_cover_ocr_titles(tx, items, as_of).await?;
    for item in items.iter_mut() {
        apply_media(
            item,
            media
                .remove(&item.identity.public_ref)
                .expect("requested media projection exists"),
        );
    }
    apply_observation_target_avatars(tx, items).await
}

fn apply_media(item: &mut MaterialLibraryItem, media: material_media_read::MediaReadProjection) {
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
}

/// Fill a blank platform title only from a selected *cover headline*. Raw OCR, substantive image
/// text, comments, and later images are intentionally ineligible: this is a display fallback,
/// never a generated summary and never a mutation of the producer's original title field.
async fn apply_cover_ocr_titles(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let refs: Vec<Uuid> = items
        .iter()
        .filter(|item| {
            !item
                .display
                .title
                .as_ref()
                .is_some_and(|title| !title.trim().is_empty())
        })
        .map(|item| item.identity.public_ref)
        .collect();
    if refs.is_empty() {
        return Ok(());
    }
    let ocr_schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_media_ocr_layout') IS NOT NULL \
                AND to_regclass('linggan_media_ocr_layering_result') IS NOT NULL \
                AND to_regclass('linggan_media_ocr_retirement') IS NOT NULL",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !ocr_schema_ready {
        return Ok(());
    }

    // Rebind only a compile-time placeholder to this query's private work scope. The canonical
    // cover selector remains shared with Comment Study and does not copy its eligibility rules.
    let selector = crate::work_resource_read::COVER_HEADLINE_SQL.replacen(
        " = $1",
        " = requested.content_public_ref",
        1,
    );
    let sql = format!(
        "SELECT requested.content_public_ref,cover.* FROM unnest($1::uuid[]) AS requested(content_public_ref) CROSS JOIN LATERAL ({selector}) cover"
    );
    let candidates = sqlx::query(AssertSqlSafe(sql))
        .bind(&refs)
        .bind(as_of)
        .fetch_all(&mut **tx)
        .await?;
    let candidates: HashMap<Uuid, PgRow> = candidates
        .into_iter()
        .map(|row| (row.get("content_public_ref"), row))
        .collect();
    for item in items {
        if let Some(candidate) = candidates.get(&item.identity.public_ref) {
            apply_cover_ocr_title(item, candidate);
        }
    }
    Ok(())
}

fn apply_cover_ocr_title(item: &mut MaterialLibraryItem, candidate: &PgRow) {
    let title: String = candidate.get("cover_headline");
    item.display.title = Some(title.clone());
    item.display.title_state = "KNOWN".to_owned();
    item.display.title_source = "cover_ocr".to_owned();
    item.display.title_media_display_ordinal = candidate.get("display_ordinal");
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert(
            "displayTitle".to_owned(),
            serde_json::json!({
                "value":title,
                "source":"cover_ocr",
                "slotKey":candidate.get::<String,_>("slot_key"),
                "displayOrdinal":candidate.get::<Option<i32>,_>("display_ordinal"),
                "layoutRef":candidate.get::<Uuid,_>("layout_ref"),
                "originalTitleState":"UNKNOWN"
            }),
        );
    }
}

/// Show the observation target's avatar when the work's own author avatar has not been observed.
///
/// A work discovered on a creator target's surface normally carries no author avatar until its
/// detail is captured, so the row would render an empty avatar next to a name that the page has
/// already fallen back to the target for. That is an inconsistency in the page, not in the data:
/// the name falls back and the picture does not.
///
/// The fallback is a *presentation* handle only. `state` keeps saying `NOT_OBSERVED`, because
/// this work's author avatar genuinely has not been observed, and `authorIdentityMatchState`
/// still governs whether the author is confirmed. Callers that need the confirmed-author picture
/// must read `fallbackUsed`; this mirrors how `cover` already falls back to a body image.
async fn apply_observation_target_avatars(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
) -> Result<(), sqlx::Error> {
    let needs_avatar = |item: &MaterialLibraryItem| {
        item.collection_context.target_kind.as_deref() == Some("creator")
            && item.collection_context.target_ref.is_some()
            && !item
                .media
                .pointer("/avatar/localAssetUrl")
                .is_some_and(|url| !url.is_null())
    };
    let mut refs: Vec<Uuid> = items
        .iter()
        .filter(|item| needs_avatar(item))
        .filter_map(|item| item.collection_context.target_ref)
        .collect();
    refs.sort_unstable();
    refs.dedup();
    if refs.is_empty() {
        return Ok(());
    }
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('collection_observation_target') IS NOT NULL \
         AND to_regclass('linggan_media_resource_relation') IS NOT NULL",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !schema_ready {
        return Ok(());
    }

    let rows = sqlx::query(
        "SELECT DISTINCT ON(target.target_ref) target.target_ref,materialization.local_asset_path,blob.mime_type,blob.byte_size \
         FROM collection_observation_target target \
         JOIN linggan_media_resource_relation relation \
              ON relation.platform=target.platform AND relation.subject_kind='author' \
             AND relation.subject_external_id=target.identity_key \
             AND relation.relationship_kind='author.avatar' \
         JOIN linggan_media_observation observation ON observation.slot_key=relation.slot_key \
         JOIN linggan_media_download_attempt attempt \
              ON attempt.media_observation_ref=observation.observation_ref \
         JOIN linggan_media_materialization materialization USING(download_attempt_ref) \
         JOIN linggan_media_blob blob ON blob.sha256=materialization.blob_sha256 \
         WHERE target.target_ref=ANY($1) AND blob.mime_type LIKE 'image/%' \
           AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition disposition \
                           WHERE disposition.slot_key=relation.slot_key \
                              OR disposition.blob_sha256=materialization.blob_sha256 \
                              OR disposition.materialization_ref=materialization.materialization_ref) \
         ORDER BY target.target_ref,materialization.verified_at DESC",
    ).bind(&refs).fetch_all(&mut **tx).await?;
    let rows: HashMap<Uuid, PgRow> = rows
        .into_iter()
        .map(|row| (row.get("target_ref"), row))
        .collect();
    for item in items.iter_mut().filter(|item| needs_avatar(item)) {
        if let Some(row) = item
            .collection_context
            .target_ref
            .and_then(|target| rows.get(&target))
        {
            apply_target_avatar(item, row);
        }
    }
    Ok(())
}

fn apply_target_avatar(item: &mut MaterialLibraryItem, row: &PgRow) {
    let local_asset_path: String = row.get("local_asset_path");
    let mime_type: String = row.get("mime_type");
    let byte_size: i64 = row.get("byte_size");
    let Some(avatar) = item.media.get_mut("avatar").and_then(Value::as_object_mut) else {
        return;
    };
    avatar.insert("localAssetUrl".to_owned(), Value::String(local_asset_path));
    avatar.insert("fallbackUsed".to_owned(), Value::Bool(true));
    avatar.insert(
        "fallbackSource".to_owned(),
        Value::String("observation_target".to_owned()),
    );
    avatar.insert(
        "blob".to_owned(),
        serde_json::json!({
            "deliveryState":"INLINE_SAFE",
            "declaredMimeType":mime_type,
            "deliveryMimeType":mime_type,
            "byteSize":byte_size
        }),
    );
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
                AND to_regclass('linggan_material_comment_current') IS NOT NULL \
                AND to_regclass('linggan_creator_discovery_policy') IS NOT NULL \
                AND to_regproc('linggan_material_content_author_at') IS NOT NULL",
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
                AND EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                            WHERE migration_id = '0032_author_profile_avatar_media') \
                AND EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                            WHERE migration_id = '0115_creator_discovery') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='linggan_material_content_detail' \
                              AND column_name='published_at_source_kind')",
    )
    .fetch_one(database.pool())
    .await
}

pub(crate) fn material_item(
    current: &WorkResourceCurrent,
    text: Option<&str>,
) -> MaterialLibraryItem {
    let title = current.title.clone();
    let body = current.body_text.clone();
    let creator = current.creator_display_name.clone();
    let published_at = current.published_at.clone();
    let published_at_source_text = current.published_at_source_text.clone();
    let observed_at = current.observed_at.clone();
    let package_ref = current.package_ref;
    let title_source_refs = material_source_refs(&current.title_source);
    let body_source_refs = material_source_refs(&current.body_source);
    let creator_source_refs = material_source_refs(&current.creator_source);
    let (package_refs, record_refs) = current_detail_provenance(current);
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
            platform: current.platform.clone(),
            content_external_id: current.content_external_id.clone(),
            public_ref: current.public_ref,
        },
        display: MaterialDisplay {
            title,
            title_state: current.title_state.clone(),
            title_source: if current
                .title
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                "platform_title"
            } else {
                "unknown"
            }
            .to_owned(),
            title_media_display_ordinal: None,
            creator_display_name: creator,
            creator_state: current.creator_display_name_state.clone(),
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
            published_at_source_field: current.published_at_source_field.clone(),
            published_at_source_kind: current.published_at_source_kind.clone(),
            published_at_precision: current.published_at_precision.clone(),
            published_at_reference_observed_at: current.published_at_reference_observed_at.clone(),
            published_at_parser_version: current.published_at_parser_version.clone(),
            engagement: MaterialEngagement {
                like_count: current.like_count,
                like_count_state: current.like_count_state.clone(),
                comment_count: current.comment_count,
                comment_count_state: current.comment_count_state.clone(),
                collect_count: current.collect_count,
                collect_count_state: current.collect_count_state.clone(),
                share_count: current.share_count,
                share_count_state: current.share_count_state.clone(),
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
            observed_source_state: current.cover_source_state.clone(),
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
        lane_summaries: default_lane_summaries(&observed_at, current.detail_material_ref.is_some()),
        summary: MaterialSummary {
            last_observed_at: observed_at.clone(),
            primary_limitation: "OTHER_LANES_NOT_EVALUATED",
            restriction_state: "UNKNOWN",
        },
        inspector: serde_json::json!({
            "overview": {
                "fields": [
                    {"field":"title","state":current.title_state,"sourceRefs":title_source_refs},
                    {"field":"body","state":current.body_state,"value":null,"accessLevel":"RESTRICTED_SOURCE","sourceRefs":body_source_refs},
                    {"field":"creator","state":current.creator_display_name_state,"sourceRefs":creator_source_refs}
                ]
            },
            "commentThreads": [],
            "mediaSlots": [],
            "derivatives": [],
            "provenance": {
                "packageRefs":package_refs,
                "recordRefs":record_refs,
                "coverageRefs":package_ref.into_iter().collect::<Vec<_>>()
            },
            "displayPolicy":"MINIMUM_NECESSARY",
            "limitations":["COMMENTS_NOT_EVALUATED","MEDIA_NOT_EVALUATED","RAW_BODY_NOT_RETURNED"]
        }),
        matched_fields,
        evidence_fragment: None,
        author_external_id: current.author_external_id.clone(),
        body_text: body,
    }
}

fn material_source_refs(source: &WorkResourceCurrentSource) -> Vec<Uuid> {
    source.material_ref.into_iter().collect()
}

fn current_detail_provenance(current: &WorkResourceCurrent) -> (Vec<Uuid>, Vec<Value>) {
    let mut package_refs = Vec::new();
    let mut record_refs = Vec::new();
    let mut include = |package_ref: Option<Uuid>, record_ordinal: Option<i32>| {
        if let Some(package_ref) = package_ref
            && !package_refs.contains(&package_ref)
        {
            package_refs.push(package_ref);
        }
        if let (Some(package_ref), Some(record_ordinal)) = (package_ref, record_ordinal)
            && !record_refs.iter().any(|record: &Value| {
                record.get("packageRef") == Some(&serde_json::json!(package_ref))
                    && record.get("recordOrdinal") == Some(&serde_json::json!(record_ordinal))
            })
        {
            record_refs.push(serde_json::json!({
                "packageRef":package_ref,
                "recordOrdinal":record_ordinal
            }));
        }
    };
    include(current.package_ref, current.record_ordinal);
    for source in [
        &current.title_source,
        &current.body_source,
        &current.creator_source,
        &current.published_source,
    ] {
        include(source.package_ref, source.record_ordinal);
    }
    (package_refs, record_refs)
}

/// Finite explicit-ref display hydration shares the standard field/media owners and domain gate.
async fn read_resource_batch(
    database: &Database,
    query: &EvidenceQuery,
    refs: &[uuid::Uuid],
) -> Result<MaterialLibraryProjection, MaterialReadError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let allowed:Vec<uuid::Uuid>=sqlx::query_scalar("SELECT DISTINCT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=ANY($2)").bind(query.domain_ref()).bind(refs).fetch_all(&mut *tx).await?;
    let currents =
        crate::work_resource_current::read_work_resource_currents(&mut tx, &allowed, &as_of)
            .await?;
    let mut items = Vec::new();
    for item in enrich_current_batch(&mut tx, &currents, None, &as_of).await? {
        if item_matches_filters(&item, query) {
            items.push(item);
        }
    }
    items.sort_by(|a, b| {
        b.summary
            .last_observed_at
            .cmp(&a.summary.last_observed_at)
            .then_with(|| a.identity.platform.cmp(&b.identity.platform))
            .then_with(|| {
                a.identity
                    .content_external_id
                    .cmp(&b.identity.content_external_id)
            })
    });
    tx.commit().await?;
    Ok(MaterialLibraryProjection {
        scanned_count: allowed.len(),
        items,
        query_scope: "accepted_typed_material_explicit_refs",
        as_of,
        cursor: None,
        truncated: false,
        scan_limited: false,
    })
}

async fn enrich_current_batch(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[WorkResourceCurrent],
    text: Option<&str>,
    as_of: &str,
) -> Result<Vec<MaterialLibraryItem>, sqlx::Error> {
    let mut items: Vec<_> = rows.iter().map(|row| material_item(row, text)).collect();
    let sources: Vec<_> = rows
        .iter()
        .map(|row| row.author_attribution_source.as_deref())
        .collect();
    enrich_discovery_batch(tx, &mut items, as_of, &sources).await?;
    material_social_read::enrich_batch(tx, &mut items, text, as_of).await?;
    enrich_media_batch(tx, &mut items, as_of).await?;
    Ok(items)
}
