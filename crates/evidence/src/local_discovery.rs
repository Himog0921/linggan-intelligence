//! LOCAL-001's narrow discovery admission and Evidence Library read projection.
//!
//! A successful call records only a discovery surface's visible cards and coverage. It does not
//! create detail evidence, media, Observation, Topic, Research, Insight, or a claim about XHS.

use linggan_contracts::{
    DiscoveryContractError, DiscoveryPackage, DiscoveryStopReason, EvidenceQuery, PublishedWindow,
    is_rfc3339_timestamp, parse_discovery_package,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

const CONTRACT_VERSION: &str = "xhs.discovery.visible-card.v1";

#[derive(Debug, Error)]
pub enum DiscoveryIngressError {
    #[error("the discovery delivery does not satisfy the current contract: {0}")]
    Contract(#[from] DiscoveryContractError),
    #[error("the local discovery admission transaction did not commit")]
    Internal(#[source] sqlx::Error),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "admission")]
pub enum DiscoveryIngressOutcome {
    Accepted {
        delivery_ref: Uuid,
        package_ref: Uuid,
        accepted_receipt_ref: Uuid,
        visible_cards: u16,
        stopped_reason: String,
    },
    Replay {
        delivery_ref: Uuid,
        package_ref: Uuid,
        accepted_receipt_ref: Uuid,
        visible_cards: u16,
        stopped_reason: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryLibraryProjection {
    pub cards: Vec<DiscoveryLibraryCard>,
    pub excluded_unknown_published_at: u64,
    pub window: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryLibraryCard {
    pub platform_content_id: String,
    pub title: Option<String>,
    pub creator_display_name: Option<String>,
    pub published_at_source_text: Option<String>,
    pub published_at: String,
    pub first_discovered_at: String,
    pub observed_at: String,
    pub result_position: i32,
    pub coverage_visible_cards: i32,
    pub coverage_maximum_quota: i32,
    pub coverage_stopped_reason: String,
    pub cover_presentation_state: &'static str,
}

/// Validates and atomically admits one discovery-only package. Replaying byte-identical input
/// preserves the original package and makes a new delivery receipt; a different payload never
/// overwrites a prior package.
pub async fn ingest_discovery_package(
    database: &Database,
    body: &str,
) -> Result<DiscoveryIngressOutcome, DiscoveryIngressError> {
    let package = parse_discovery_package(body)?;
    let payload: serde_json::Value = serde_json::from_str(body)
        .expect("the discovery parser accepted JSON that serde_json can retain");
    let package_hash = sha256_hex(body);
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(DiscoveryIngressError::Internal)?;

    if let Some(existing) = existing_package(&mut transaction, &package_hash).await? {
        let delivery_ref = Uuid::new_v4();
        insert_delivery(
            &mut transaction,
            existing.package_id,
            delivery_ref,
            "replay",
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(DiscoveryIngressError::Internal)?;
        return Ok(DiscoveryIngressOutcome::Replay {
            delivery_ref,
            package_ref: existing.package_ref,
            accepted_receipt_ref: existing.accepted_receipt_ref,
            visible_cards: existing.visible_cards,
            stopped_reason: existing.stopped_reason,
        });
    }

    let package_ref = Uuid::new_v4();
    let accepted_receipt_ref = Uuid::new_v4();
    let package_id = insert_package(
        &mut transaction,
        &package,
        &package_hash,
        package_ref,
        accepted_receipt_ref,
        payload,
    )
    .await?;
    insert_cards(&mut transaction, package_id, &package).await?;
    insert_coverage(&mut transaction, package_id, &package).await?;
    let delivery_ref = Uuid::new_v4();
    insert_delivery(&mut transaction, package_id, delivery_ref, "accepted").await?;
    transaction
        .commit()
        .await
        .map_err(DiscoveryIngressError::Internal)?;

    Ok(DiscoveryIngressOutcome::Accepted {
        delivery_ref,
        package_ref,
        accepted_receipt_ref,
        visible_cards: package.coverage().visible_cards(),
        stopped_reason: stop_reason(package.coverage().stopped_reason()).to_owned(),
    })
}

/// Reads only accepted discovery records. The returned cover state deliberately has no URL: a
/// local replica is a later media-acquisition responsibility, not a discovery-card capability.
pub async fn read_discovery_library(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<DiscoveryLibraryProjection, sqlx::Error> {
    let window_days = match query.window() {
        PublishedWindow::Last7Days => 7,
        PublishedWindow::Last30Days => 30,
    };
    let text = query.text().filter(|value| !value.trim().is_empty());
    let rows = sqlx::query(
        "WITH candidate AS ( \
             SELECT occurrence.*, content.platform_content_id, package.maximum_quota, \
                    coverage.visible_cards, coverage.stopped_reason \
             FROM local_discovery_occurrence occurrence \
             JOIN local_discovery_content_item content ON content.id = occurrence.content_item_id \
             JOIN local_discovery_package package ON package.id = occurrence.package_id \
             JOIN local_discovery_coverage coverage ON coverage.package_id = package.id \
             WHERE occurrence.published_at IS NOT NULL \
               AND occurrence.published_at >= scope_001_now() - make_interval(days => $1) \
               AND occurrence.published_at <= scope_001_now() \
               AND ($2::text IS NULL OR lower(coalesce(occurrence.creator_display_name, '')) LIKE '%' || lower($2) || '%' \
                    OR lower(coalesce(occurrence.title, '')) LIKE '%' || lower($2) || '%') \
         ), first_discovery AS ( \
             SELECT occurrence.content_item_id, min(package.accepted_at) AS first_discovered_at \
             FROM local_discovery_occurrence occurrence \
             JOIN local_discovery_package package ON package.id = occurrence.package_id \
             GROUP BY occurrence.content_item_id \
         ), latest AS ( \
             SELECT DISTINCT ON (content_item_id) * FROM candidate \
             ORDER BY content_item_id, observed_at DESC, id DESC \
         ) \
         SELECT latest.platform_content_id, latest.title, latest.creator_display_name, \
                latest.published_at_source_text, latest.published_at::text AS published_at, \
                first_discovery.first_discovered_at::text AS first_discovered_at, \
                latest.observed_at::text AS observed_at, latest.result_position, \
                latest.visible_cards, latest.maximum_quota, latest.stopped_reason \
         FROM latest JOIN first_discovery ON first_discovery.content_item_id = latest.content_item_id \
         ORDER BY first_discovery.first_discovered_at DESC, latest.result_position ASC",
    )
    .bind(window_days)
    .bind(text)
    .fetch_all(database.pool())
    .await?;

    let excluded_unknown_published_at =
        count_unknown_published_at(database, text, window_days).await?;
    let cards = rows
        .into_iter()
        .map(|row| DiscoveryLibraryCard {
            platform_content_id: row.get("platform_content_id"),
            title: row.get("title"),
            creator_display_name: row.get("creator_display_name"),
            published_at_source_text: row.get("published_at_source_text"),
            published_at: row.get("published_at"),
            first_discovered_at: row.get("first_discovered_at"),
            observed_at: row.get("observed_at"),
            result_position: row.get("result_position"),
            coverage_visible_cards: row.get("visible_cards"),
            coverage_maximum_quota: row.get("maximum_quota"),
            coverage_stopped_reason: row.get("stopped_reason"),
            cover_presentation_state: "MEDIA_NOT_ACQUIRED",
        })
        .collect();
    Ok(DiscoveryLibraryProjection {
        cards,
        excluded_unknown_published_at,
        window: match query.window() {
            PublishedWindow::Last7Days => "last_7_days",
            PublishedWindow::Last30Days => "last_30_days",
        },
    })
}

struct ExistingPackage {
    package_id: i64,
    package_ref: Uuid,
    accepted_receipt_ref: Uuid,
    visible_cards: u16,
    stopped_reason: String,
}

async fn existing_package(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    package_hash: &str,
) -> Result<Option<ExistingPackage>, DiscoveryIngressError> {
    let row = sqlx::query(
        "SELECT package.id, package.package_ref, package.accepted_receipt_ref, \
                coverage.visible_cards, coverage.stopped_reason \
         FROM local_discovery_package package \
         JOIN local_discovery_coverage coverage ON coverage.package_id = package.id \
         WHERE package.package_hash = $1 FOR UPDATE",
    )
    .bind(package_hash)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(DiscoveryIngressError::Internal)?;
    Ok(row.map(|row| ExistingPackage {
        package_id: row.get("id"),
        package_ref: row.get("package_ref"),
        accepted_receipt_ref: row.get("accepted_receipt_ref"),
        visible_cards: row.get::<i32, _>("visible_cards") as u16,
        stopped_reason: row.get("stopped_reason"),
    }))
}

async fn insert_package(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    package: &DiscoveryPackage,
    package_hash: &str,
    package_ref: Uuid,
    accepted_receipt_ref: Uuid,
    payload: serde_json::Value,
) -> Result<i64, DiscoveryIngressError> {
    sqlx::query(
        "INSERT INTO local_discovery_package \
         (package_ref, package_hash, accepted_receipt_ref, contract_version, platform, query_text, sort, \
          target_basis, target_unit, maximum_quota, observed_at, payload) \
         VALUES ($1, $2, $3, $4, 'xhs', $5, 'comprehensive', 'maximum_quota', 'visible_search_card', $6, $7::timestamptz, $8) \
         RETURNING id",
    )
    .bind(package_ref)
    .bind(package_hash)
    .bind(accepted_receipt_ref)
    .bind(CONTRACT_VERSION)
    .bind(package.acquisition_spec().query())
    .bind(i32::from(package.acquisition_spec().maximum_quota()))
    .bind(package.observed_at())
    .bind(payload)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get("id"))
    .map_err(DiscoveryIngressError::Internal)
}

async fn insert_cards(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    package_id: i64,
    package: &DiscoveryPackage,
) -> Result<(), DiscoveryIngressError> {
    for card in package.cards() {
        let content_item_id =
            ensure_content_item(transaction, card.content().platform_content_id()).await?;
        let source_text = card.content().published_at_source_text().unwrap_or("");
        let has_exact_published_at = is_rfc3339_timestamp(source_text);
        let parser_version = if has_exact_published_at {
            "rfc3339-source-text-v1"
        } else {
            "none"
        };
        let precision = if has_exact_published_at {
            "exact"
        } else {
            "unknown"
        };
        let cover = card
            .content()
            .cover_candidate()
            .map(|value| value.observed_external_uri());
        sqlx::query(
            "INSERT INTO local_discovery_occurrence \
             (package_id, content_item_id, result_position, observed_at, title, creator_display_name, \
              published_at_source_text, published_at, published_at_source_kind, published_at_reference_observed_at, \
              published_at_parser_version, published_at_precision, cover_candidate_external_uri, cover_presentation_state) \
             VALUES ($1, $2, $3, $4::timestamptz, $5, $6, NULLIF($7, ''), \
                     CASE WHEN $8 THEN NULLIF($7, '')::timestamptz ELSE NULL END, 'visible_card_text', $4::timestamptz, \
                     $9, $10, $11, 'media_not_acquired')",
        )
        .bind(package_id)
        .bind(content_item_id)
        .bind(i32::from(card.occurrence().result_position()))
        .bind(card.occurrence().observed_at())
        .bind(card.content().title())
        .bind(card.content().creator_display_name())
        .bind(source_text)
        .bind(has_exact_published_at)
        .bind(parser_version)
        .bind(precision)
        .bind(cover)
        .execute(&mut **transaction)
        .await
        .map_err(DiscoveryIngressError::Internal)?;
    }
    Ok(())
}

async fn ensure_content_item(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform_content_id: &str,
) -> Result<i64, DiscoveryIngressError> {
    let inserted = sqlx::query(
        "INSERT INTO local_discovery_content_item (platform, platform_content_id) VALUES ('xhs', $1) \
         ON CONFLICT (platform, platform_content_id) DO NOTHING RETURNING id",
    )
    .bind(platform_content_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(DiscoveryIngressError::Internal)?;
    if let Some(row) = inserted {
        return Ok(row.get("id"));
    }
    sqlx::query(
        "SELECT id FROM local_discovery_content_item WHERE platform = 'xhs' AND platform_content_id = $1",
    )
    .bind(platform_content_id)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get("id"))
    .map_err(DiscoveryIngressError::Internal)
}

async fn insert_coverage(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    package_id: i64,
    package: &DiscoveryPackage,
) -> Result<(), DiscoveryIngressError> {
    sqlx::query(
        "INSERT INTO local_discovery_coverage (package_id, unit, visible_cards, stopped_reason) \
         VALUES ($1, 'visible_search_card', $2, $3)",
    )
    .bind(package_id)
    .bind(i32::from(package.coverage().visible_cards()))
    .bind(stop_reason(package.coverage().stopped_reason()))
    .execute(&mut **transaction)
    .await
    .map_err(DiscoveryIngressError::Internal)?;
    Ok(())
}

async fn insert_delivery(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    package_id: i64,
    delivery_ref: Uuid,
    outcome: &str,
) -> Result<(), DiscoveryIngressError> {
    sqlx::query(
        "INSERT INTO local_discovery_ingress_delivery (delivery_ref, package_id, outcome) VALUES ($1, $2, $3)",
    )
    .bind(delivery_ref)
    .bind(package_id)
    .bind(outcome)
    .execute(&mut **transaction)
    .await
    .map_err(DiscoveryIngressError::Internal)?;
    Ok(())
}

async fn count_unknown_published_at(
    database: &Database,
    text: Option<&str>,
    window_days: i32,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "SELECT count(DISTINCT unknown_occurrence.content_item_id) AS count \
         FROM local_discovery_occurrence unknown_occurrence \
         WHERE unknown_occurrence.published_at IS NULL \
           AND ($1::text IS NULL OR lower(coalesce(unknown_occurrence.creator_display_name, '')) LIKE '%' || lower($1) || '%' \
                OR lower(coalesce(unknown_occurrence.title, '')) LIKE '%' || lower($1) || '%') \
           AND NOT EXISTS ( \
                SELECT 1 FROM local_discovery_occurrence eligible_occurrence \
                WHERE eligible_occurrence.content_item_id = unknown_occurrence.content_item_id \
                  AND eligible_occurrence.published_at IS NOT NULL \
                  AND eligible_occurrence.published_at >= scope_001_now() - make_interval(days => $2) \
                  AND eligible_occurrence.published_at <= scope_001_now() \
                  AND ($1::text IS NULL OR lower(coalesce(eligible_occurrence.creator_display_name, '')) LIKE '%' || lower($1) || '%' \
                       OR lower(coalesce(eligible_occurrence.title, '')) LIKE '%' || lower($1) || '%') \
           )",
    )
    .bind(text)
    .bind(window_days)
    .fetch_one(database.pool())
    .await
    .map(|row| row.get::<i64, _>("count") as u64)
}

fn sha256_hex(input: &str) -> String {
    Sha256::digest(input.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn stop_reason(value: DiscoveryStopReason) -> &'static str {
    match value {
        DiscoveryStopReason::QuotaReached => "quota_reached",
        DiscoveryStopReason::SurfaceEnded => "surface_ended",
        DiscoveryStopReason::RiskControl => "risk_control",
        DiscoveryStopReason::ManualStop => "manual_stop",
        DiscoveryStopReason::Unknown => "unknown",
    }
}
