//! Durable, bounded acquisition work for short-lived media source URLs.
//!
//! The work row is mutable orchestration state.  It never replaces the append-only source
//! observation, download-attempt, blob or materialization facts.

use crate::producer_runtime::ProducerRuntimeError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

const MAX_ATTEMPTS: i32 = 3;

#[derive(Debug, Error)]
pub enum MediaAcquisitionError {
    #[error("media acquisition schema is unavailable")]
    SchemaUnavailable,
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "decision")]
pub enum MediaAcquisitionDecision {
    Acquired {
        work_ref: Uuid,
        observation_ref: Uuid,
        claim_generation: i32,
        candidate_uris: Vec<String>,
        lease_expires_at: String,
        next_poll_after_seconds: u64,
    },
    NothingWaiting {
        next_poll_after_seconds: u64,
    },
    InstallationNotClaimed {
        next_poll_after_seconds: u64,
    },
    CapabilityUnavailable {
        next_poll_after_seconds: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAcquisitionFailureOutcome {
    pub work_ref: Uuid,
    pub state: String,
    pub attempt_count: i32,
}

pub async fn media_acquisition_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass('linggan_discovery_cover_media_link') IS NOT NULL \
             AND to_regclass('linggan_media_acquisition_work') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// Project cover observations for historical accepted discovery rows that predate 0021.
/// It is idempotent and only adds the missing media identity/control records.
pub async fn ensure_discovery_cover_media_work(
    database: &Database,
) -> Result<u64, MediaAcquisitionError> {
    if !media_acquisition_schema_is_ready(database).await? {
        return Err(MediaAcquisitionError::SchemaUnavailable);
    }
    let rows = sqlx::query(
        "WITH latest AS (SELECT DISTINCT ON (content_public_ref) finding.* \
           FROM linggan_material_discovery_finding finding \
           WHERE finding.cover_source_state='KNOWN' AND finding.cover_source_url IS NOT NULL \
           ORDER BY content_public_ref,observed_at::timestamptz DESC,created_at DESC) \
         SELECT finding.material_ref,finding.content_public_ref,finding.package_ref, \
                finding.record_ordinal,finding.observed_at,finding.cover_source_url, \
                content.platform,content.content_external_id \
         FROM latest finding \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref \
         LEFT JOIN linggan_discovery_cover_media_link link USING(material_ref) \
         WHERE link.material_ref IS NULL \
         ORDER BY finding.created_at LIMIT 500",
    )
    .fetch_all(database.pool())
    .await?;
    let mut projected = 0;
    for row in rows {
        let mut tx = database.pool().begin().await?;
        let inserted = project_discovery_cover(
            &mut tx,
            row.get("material_ref"),
            row.get("content_public_ref"),
            row.get("package_ref"),
            row.get("record_ordinal"),
            row.get("platform"),
            row.get("content_external_id"),
            row.get("observed_at"),
            row.get("cover_source_url"),
        )
        .await
        .map_err(|error| match error {
            ProducerRuntimeError::Internal(error) => MediaAcquisitionError::Database(error),
            _ => MediaAcquisitionError::SchemaUnavailable,
        })?;
        tx.commit().await?;
        projected += u64::from(inserted);
    }
    Ok(projected)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn project_discovery_cover(
    tx: &mut Transaction<'_, Postgres>,
    material_ref: Uuid,
    content_public_ref: Uuid,
    package_ref: Uuid,
    record_ordinal: i32,
    platform: &str,
    content_external_id: &str,
    observed_at: &str,
    cover_source_url: &str,
) -> Result<bool, ProducerRuntimeError> {
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_discovery_cover_media_link') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
    if !schema_ready {
        return Ok(false);
    }
    let slot_key = format!("{platform}:{content_external_id}:cover:1");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&slot_key)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_discovery_cover_media_link WHERE material_ref=$1)",
    )
    .bind(material_ref)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if exists {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO linggan_media_slot \
         (slot_key,platform,content_external_id,role,ordinal,first_package_ref) \
         VALUES ($1,$2,$3,'cover',1,$4) ON CONFLICT (slot_key) DO NOTHING",
    )
    .bind(&slot_key)
    .bind(platform)
    .bind(content_external_id)
    .bind(package_ref)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let generation: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(source_generation),0)+1 \
         FROM linggan_material_media_origin WHERE slot_key=$1",
    )
    .bind(&slot_key)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let observation_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_media_observation \
         (observation_ref,slot_key,package_ref,observed_external_uri,observed_at) \
         VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(observation_ref)
    .bind(&slot_key)
    .bind(package_ref)
    .bind(cover_source_url)
    .bind(observed_at)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "INSERT INTO linggan_material_media_origin \
         (observation_ref,content_public_ref,slot_key,package_ref,record_ordinal,source_generation, \
          purpose,producer_ordinal,display_ordinal,display_order_state,display_order_basis, \
          candidate_set_state,composite_state,live_photo_still_state,live_photo_motion_state) \
         VALUES ($1,$2,$3,$4,$5,$6,'cover',1,1,'KNOWN','platform_explicit', \
                 'OBSERVED_SET','NOT_APPLICABLE',NULL,NULL)",
    )
    .bind(observation_ref)
    .bind(content_public_ref)
    .bind(&slot_key)
    .bind(package_ref)
    .bind(record_ordinal)
    .bind(generation)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "INSERT INTO linggan_material_media_candidate \
         (candidate_ref,observation_ref,candidate_ordinal,external_uri,producer_primary, \
          source_field,source_field_state,expires_at,expires_at_state) \
         VALUES ($1,$2,1,$3,true,'cover_source_url','KNOWN',NULL,'UNKNOWN')",
    )
    .bind(Uuid::new_v4())
    .bind(observation_ref)
    .bind(cover_source_url)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query(
        "INSERT INTO linggan_discovery_cover_media_link (material_ref,observation_ref) VALUES ($1,$2)",
    )
    .bind(material_ref)
    .bind(observation_ref)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let already_materialized: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_material_media_origin origin \
          JOIN linggan_media_download_attempt attempt \
            ON attempt.media_observation_ref=origin.observation_ref \
          JOIN linggan_media_materialization materialization USING(download_attempt_ref) \
          WHERE origin.slot_key=$1)",
    )
    .bind(&slot_key)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let acquisition_active: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM linggan_material_media_origin origin \
          JOIN linggan_media_acquisition_work work USING(observation_ref) \
          WHERE origin.slot_key=$1 AND work.state IN ('pending','leased','retry_wait'))",
    )
    .bind(&slot_key)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !already_materialized && !acquisition_active {
        sqlx::query(
            "INSERT INTO linggan_media_acquisition_work (work_ref,observation_ref) VALUES ($1,$2)",
        )
        .bind(Uuid::new_v4())
        .bind(observation_ref)
        .execute(&mut **tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(true)
}

pub async fn claim_media_acquisition(
    database: &Database,
    install_key: &str,
) -> Result<MediaAcquisitionDecision, MediaAcquisitionError> {
    if !media_acquisition_schema_is_ready(database).await? {
        return Err(MediaAcquisitionError::SchemaUnavailable);
    }
    let mut tx = database.pool().begin().await?;
    let installation = sqlx::query(
        "SELECT installation.installation_ref,installation.capabilities \
         FROM plugin_installation installation \
         JOIN execution_station station ON station.station_ref=installation.station_ref \
           AND station.retired_at IS NULL \
         WHERE installation.install_key=$1 AND installation.superseded_at IS NULL \
         ORDER BY installation.first_seen_at DESC LIMIT 1 FOR UPDATE OF installation",
    )
    .bind(install_key)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(installation) = installation else {
        tx.commit().await?;
        return Ok(MediaAcquisitionDecision::InstallationNotClaimed {
            next_poll_after_seconds: 300,
        });
    };
    let installation_ref: Uuid = installation.get("installation_ref");
    let capabilities: serde_json::Value = installation.get("capabilities");
    if !capabilities
        .as_array()
        .is_some_and(|values| values.iter().any(|value| value == "media_slots"))
    {
        tx.commit().await?;
        return Ok(MediaAcquisitionDecision::CapabilityUnavailable {
            next_poll_after_seconds: 900,
        });
    }
    sqlx::query(
        "UPDATE linggan_media_acquisition_work SET \
           state=CASE WHEN attempt_count >= $1 THEN 'terminal' ELSE 'retry_wait' END, \
           claimed_by_installation_ref=NULL,lease_expires_at=NULL,next_attempt_at=scope_001_now(), \
           last_error='lease_expired',updated_at=scope_001_now() \
         WHERE state='leased' AND lease_expires_at <= scope_001_now()",
    )
    .bind(MAX_ATTEMPTS)
    .execute(&mut *tx)
    .await?;
    let replay = sqlx::query(
        "SELECT work_ref,observation_ref,claim_generation,lease_expires_at::text AS lease_expires_at \
         FROM linggan_media_acquisition_work \
         WHERE state='leased' AND claimed_by_installation_ref=$1 \
           AND lease_expires_at > scope_001_now() \
         ORDER BY updated_at LIMIT 1 FOR UPDATE",
    )
    .bind(installation_ref)
    .fetch_optional(&mut *tx)
    .await?;
    let work = if let Some(row) = replay {
        row
    } else {
        let candidate = sqlx::query(
            "SELECT work_ref FROM linggan_media_acquisition_work \
             WHERE state IN ('pending','retry_wait') AND attempt_count < $1 \
               AND next_attempt_at <= scope_001_now() \
             ORDER BY next_attempt_at,created_at LIMIT 1 FOR UPDATE SKIP LOCKED",
        )
        .bind(MAX_ATTEMPTS)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(candidate) = candidate else {
            tx.commit().await?;
            return Ok(MediaAcquisitionDecision::NothingWaiting {
                next_poll_after_seconds: 300,
            });
        };
        sqlx::query(
            "UPDATE linggan_media_acquisition_work SET state='leased',attempt_count=attempt_count+1, \
             claim_generation=claim_generation+1,claimed_by_installation_ref=$2, \
             lease_expires_at=scope_001_now()+interval '5 minutes',updated_at=scope_001_now() \
             WHERE work_ref=$1 \
             RETURNING work_ref,observation_ref,claim_generation,lease_expires_at::text AS lease_expires_at",
        )
        .bind(candidate.get::<Uuid, _>("work_ref"))
        .bind(installation_ref)
        .fetch_one(&mut *tx)
        .await?
    };
    let observation_ref: Uuid = work.get("observation_ref");
    let candidate_uris = sqlx::query_scalar::<_, String>(
        "SELECT external_uri FROM linggan_material_media_candidate \
         WHERE observation_ref=$1 ORDER BY candidate_ordinal LIMIT 6",
    )
    .bind(observation_ref)
    .fetch_all(&mut *tx)
    .await?;
    let lease_expires_at: String = work.get("lease_expires_at");
    let outcome = MediaAcquisitionDecision::Acquired {
        work_ref: work.get("work_ref"),
        observation_ref,
        claim_generation: work.get("claim_generation"),
        candidate_uris,
        lease_expires_at,
        next_poll_after_seconds: 0,
    };
    tx.commit().await?;
    Ok(outcome)
}

pub async fn record_media_acquisition_failure(
    database: &Database,
    work_ref: Uuid,
    observation_ref: Uuid,
    install_key: &str,
    claim_generation: i32,
    error: &str,
) -> Result<MediaAcquisitionFailureOutcome, MediaAcquisitionError> {
    let mut tx = database.pool().begin().await?;
    let row = sqlx::query(
        "SELECT work.attempt_count,work.claim_generation,work.state,installation.install_key \
         FROM linggan_media_acquisition_work work \
         LEFT JOIN plugin_installation installation \
           ON installation.installation_ref=work.claimed_by_installation_ref \
         WHERE work.work_ref=$1 AND work.observation_ref=$2 FOR UPDATE OF work",
    )
    .bind(work_ref)
    .bind(observation_ref)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(MediaAcquisitionFailureOutcome {
            work_ref,
            state: "lost_authority".to_owned(),
            attempt_count: 0,
        });
    };
    let attempt_count: i32 = row.get("attempt_count");
    let live = row.get::<String, _>("state") == "leased"
        && row.get::<i32, _>("claim_generation") == claim_generation
        && row.get::<Option<String>, _>("install_key").as_deref() == Some(install_key);
    if !live {
        tx.commit().await?;
        return Ok(MediaAcquisitionFailureOutcome {
            work_ref,
            state: "lost_authority".to_owned(),
            attempt_count,
        });
    }
    let terminal = attempt_count >= MAX_ATTEMPTS;
    let state = if terminal { "terminal" } else { "retry_wait" };
    sqlx::query(
        "UPDATE linggan_media_acquisition_work SET state=$2,claimed_by_installation_ref=NULL, \
         lease_expires_at=NULL,next_attempt_at=scope_001_now() + \
           make_interval(secs => LEAST(300, 30 * (1 << GREATEST(0,attempt_count-1)))), \
         last_error=$3,updated_at=scope_001_now() WHERE work_ref=$1",
    )
    .bind(work_ref)
    .bind(state)
    .bind(error)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(MediaAcquisitionFailureOutcome {
        work_ref,
        state: state.to_owned(),
        attempt_count,
    })
}

pub(crate) async fn complete_media_acquisition_for_observation(
    tx: &mut Transaction<'_, Postgres>,
    observation_ref: Uuid,
) -> Result<(), sqlx::Error> {
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_media_acquisition_work') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await?;
    if !schema_ready {
        return Ok(());
    }
    sqlx::query(
        "UPDATE linggan_media_acquisition_work SET state='completed',completed_at=scope_001_now(), \
         claimed_by_installation_ref=NULL,lease_expires_at=NULL,last_error=NULL,updated_at=scope_001_now() \
         WHERE observation_ref=$1 AND state <> 'completed'",
    )
    .bind(observation_ref)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
