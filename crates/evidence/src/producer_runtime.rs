//! Linggan-owned Browser Producer Runtime admission.
//!
//! Acknowledging a package records an immutable producer statement and receipt.  It is *not* an
//! Evidence/Observation/Claim promotion. Package-level acceptance, later record handling and
//! Coverage-based use remain deliberately separate.

use crate::local_discovery::{DiscoveryLibraryCard, DiscoveryLibraryProjection};
use linggan_contracts::{
    EvidenceQuery, EvidenceTimeView, ProducerAttempt, ProducerCapturePackage,
    ProducerRuntimeContractError, ProducerSubmission, ProducerTaskSpec, PublishedWindow,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ProducerRuntimeError {
    #[error("producer runtime contract is invalid: {0}")]
    Contract(#[from] ProducerRuntimeContractError),
    #[error("a scheduled task may only be created by the server's lease path")]
    ScheduledTaskNotServerIssued,
    #[error("the producer task or attempt does not exist")]
    RoutingNotFound,
    #[error("the producer identity does not own the attempt")]
    AttemptIdentityMismatch,
    #[error("the runtime transaction did not commit")]
    Internal(#[source] sqlx::Error),
    #[error("the referenced media observation does not exist")]
    MediaObservationNotFound,
    #[error("media bytes conflict with an existing content-addressed blob")]
    MediaBlobConflict,
    #[error("typed material identity conflicts with an existing immutable fact")]
    MaterialIdentityConflict,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum RuntimeTaskOutcome {
    Created { task_id: Uuid },
    Replay { task_id: Uuid },
    Conflict { task_id: Uuid },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum RuntimeAttemptOutcome {
    Started { attempt_id: Uuid, task_id: Uuid },
    Replay { attempt_id: Uuid, task_id: Uuid },
    Conflict { attempt_id: Uuid },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "delivery")]
pub enum RuntimeSubmissionOutcome {
    Acknowledged {
        submission_id: Uuid,
        receipt_ref: Uuid,
        package_ref: Uuid,
        package_kind: String,
    },
    Replay {
        submission_id: Uuid,
        receipt_ref: Uuid,
        package_ref: Uuid,
        package_kind: String,
    },
    Conflict {
        submission_id: Uuid,
    },
}

#[derive(Debug, Serialize)]
pub struct MediaBlobAdmission {
    pub blob_sha256: String,
    pub local_asset_path: String,
    pub download_attempt_ref: Uuid,
    pub materialization_ref: Uuid,
    pub processing_jobs: Vec<Uuid>,
}

/// Mutable delivery state for a media upload. It is deliberately separate from raw-media
/// admission: knowing a byte offset does not create Evidence or a media blob.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaUploadSession {
    pub session_ref: Uuid,
    pub media_observation_ref: Uuid,
    pub expected_sha256: String,
    pub mime_type: String,
    pub expected_byte_size: i64,
    pub temporary_storage_key: String,
    pub next_offset: i64,
    pub state: String,
}

#[derive(Debug)]
pub enum MediaUploadFinalizeClaim {
    Ready(MediaUploadSession),
    Materialized(MediaBlobAdmission),
    Incomplete,
    Busy,
}

pub async fn admit_media_blob(
    database: &Database,
    media_observation_ref: Uuid,
    sha256: &str,
    mime_type: &str,
    byte_size: i64,
    storage_key: &str,
) -> Result<MediaBlobAdmission, ProducerRuntimeError> {
    if !crate::material_storage_key::is_safe_storage_key(storage_key) {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let source_uri = sqlx::query_scalar::<_, String>(
        "SELECT observed_external_uri FROM linggan_media_observation WHERE observation_ref = $1 FOR UPDATE",
    )
    .bind(media_observation_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?
    .ok_or(ProducerRuntimeError::MediaObservationNotFound)?;
    let download_attempt_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_download_attempt (download_attempt_ref,media_observation_ref,ended_at,terminal_reason,attempted_uri) VALUES ($1,$2,scope_001_now(),'acquired',$3)")
        .bind(download_attempt_ref).bind(media_observation_ref).bind(source_uri)
        .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    let existing = sqlx::query(
        "SELECT mime_type,byte_size FROM linggan_media_blob WHERE sha256 = $1 FOR UPDATE",
    )
    .bind(sha256)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if let Some(row) = existing {
        if row.get::<String, _>("mime_type") != mime_type
            || row.get::<i64, _>("byte_size") != byte_size
        {
            return Err(ProducerRuntimeError::MediaBlobConflict);
        }
    } else {
        sqlx::query("INSERT INTO linggan_media_blob (sha256,mime_type,byte_size,storage_key) VALUES ($1,$2,$3,$4)")
            .bind(sha256).bind(mime_type).bind(byte_size).bind(storage_key)
            .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    }
    let materialization_ref = Uuid::new_v4();
    let qualified_read_schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('linggan_material_media_disposition_event') IS NOT NULL",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let local_asset_path = if qualified_read_schema_ready {
        format!("/api/local/media/{materialization_ref}/{sha256}")
    } else {
        format!("/api/local/media/{sha256}")
    };
    sqlx::query("INSERT INTO linggan_media_materialization (materialization_ref,blob_sha256,download_attempt_ref,local_asset_path) VALUES ($1,$2,$3,$4)")
        .bind(materialization_ref).bind(sha256).bind(download_attempt_ref).bind(&local_asset_path)
        .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    let slot_key = sqlx::query_scalar::<_, String>(
        "SELECT slot_key FROM linggan_media_observation WHERE observation_ref = $1",
    )
    .bind(media_observation_ref)
    .fetch_one(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let mut processing_jobs = Vec::new();
    for processor_kind in processors_for_mime(mime_type) {
        let job_ref = Uuid::new_v4();
        let inserted = sqlx::query("INSERT INTO linggan_media_processing_job (job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES ($1,$2,$3,$4,'local-v1','full_blob') ON CONFLICT (blob_sha256,slot_key,processor_kind,processor_version,input_scope) DO NOTHING")
            .bind(job_ref).bind(sha256).bind(&slot_key).bind(processor_kind)
            .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
        if inserted.rows_affected() == 1 {
            sqlx::query("INSERT INTO linggan_media_processing_job_event (event_ref,job_ref,state,reason) VALUES ($1,$2,'pending','provider_not_enabled')")
                .bind(Uuid::new_v4()).bind(job_ref).execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
            processing_jobs.push(job_ref);
        }
    }
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(MediaBlobAdmission {
        blob_sha256: sha256.to_owned(),
        local_asset_path,
        download_attempt_ref,
        materialization_ref,
        processing_jobs,
    })
}

/// A failed download is still an immutable observation about the acquisition attempt. It does
/// not alter the text package, slot identity, or a future retry's chance to acquire bytes.
pub async fn record_media_download_failure(
    database: &Database,
    media_observation_ref: Uuid,
    attempted_uri: &str,
    terminal_reason: &str,
) -> Result<Uuid, ProducerRuntimeError> {
    let reason = match terminal_reason {
        "expired_url" | "mime_mismatch" | "size_limit" | "cancelled" | "network_error" => {
            terminal_reason
        }
        _ => "unknown",
    };
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM linggan_media_observation WHERE observation_ref = $1)",
    )
    .bind(media_observation_ref)
    .fetch_one(database.pool())
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !exists {
        return Err(ProducerRuntimeError::MediaObservationNotFound);
    }
    let attempt_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_download_attempt (download_attempt_ref,media_observation_ref,ended_at,terminal_reason,attempted_uri) VALUES ($1,$2,scope_001_now(),$3,$4)")
        .bind(attempt_ref).bind(media_observation_ref).bind(reason).bind(attempted_uri)
        .execute(database.pool()).await.map_err(ProducerRuntimeError::Internal)?;
    Ok(attempt_ref)
}

pub async fn begin_media_upload(
    database: &Database,
    media_observation_ref: Uuid,
    expected_sha256: &str,
    mime_type: &str,
    expected_byte_size: i64,
    temporary_storage_key: &str,
) -> Result<MediaUploadSession, ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let observation_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM linggan_media_observation WHERE observation_ref = $1)",
    )
    .bind(media_observation_ref)
    .fetch_one(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !observation_exists {
        return Err(ProducerRuntimeError::MediaObservationNotFound);
    }
    let existing = sqlx::query(
        "SELECT session_ref,media_observation_ref,expected_sha256,mime_type,expected_byte_size,temporary_storage_key,received_byte_size,state \
         FROM linggan_media_upload_session \
         WHERE media_observation_ref = $1 AND expected_sha256 = $2 AND expected_byte_size = $3 FOR UPDATE",
    )
    .bind(media_observation_ref)
    .bind(expected_sha256)
    .bind(expected_byte_size)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    let session = if let Some(row) = existing {
        if row.get::<String, _>("mime_type") != mime_type {
            return Err(ProducerRuntimeError::MediaBlobConflict);
        }
        media_upload_session_from_row(&row)
    } else {
        let session_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_media_upload_session \
             (session_ref,media_observation_ref,expected_sha256,mime_type,expected_byte_size,temporary_storage_key,state) \
             VALUES ($1,$2,$3,$4,$5,$6,'receiving')",
        )
        .bind(session_ref)
        .bind(media_observation_ref)
        .bind(expected_sha256)
        .bind(mime_type)
        .bind(expected_byte_size)
        .bind(temporary_storage_key)
        .execute(&mut *tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
        MediaUploadSession {
            session_ref,
            media_observation_ref,
            expected_sha256: expected_sha256.to_owned(),
            mime_type: mime_type.to_owned(),
            expected_byte_size,
            temporary_storage_key: temporary_storage_key.to_owned(),
            next_offset: 0,
            state: "receiving".to_owned(),
        }
    };
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(session)
}

pub async fn record_media_upload_chunk(
    database: &Database,
    session_ref: Uuid,
    offset: i64,
    byte_count: i64,
) -> Result<MediaUploadSession, ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let row = sqlx::query(
        "SELECT session_ref,media_observation_ref,expected_sha256,mime_type,expected_byte_size,temporary_storage_key,received_byte_size,state \
         FROM linggan_media_upload_session WHERE session_ref = $1 FOR UPDATE",
    )
    .bind(session_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?
    .ok_or(ProducerRuntimeError::MediaObservationNotFound)?;
    let mut session = media_upload_session_from_row(&row);
    if session.state == "materialized" {
        tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
        return Ok(session);
    }
    if session.state != "receiving" || session.next_offset != offset || byte_count < 1 {
        return Err(ProducerRuntimeError::MediaBlobConflict);
    }
    let next_offset = offset
        .checked_add(byte_count)
        .ok_or(ProducerRuntimeError::MediaBlobConflict)?;
    if next_offset > session.expected_byte_size {
        return Err(ProducerRuntimeError::MediaBlobConflict);
    }
    let state = if next_offset == session.expected_byte_size {
        "ready_to_finalize"
    } else {
        "receiving"
    };
    sqlx::query("UPDATE linggan_media_upload_session SET received_byte_size = $2,state = $3,updated_at = scope_001_now() WHERE session_ref = $1")
        .bind(session_ref).bind(next_offset).bind(state).execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    session.next_offset = next_offset;
    session.state = state.to_owned();
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(session)
}

pub async fn read_media_upload_session(
    database: &Database,
    session_ref: Uuid,
) -> Result<Option<MediaUploadSession>, ProducerRuntimeError> {
    sqlx::query(
        "SELECT session_ref,media_observation_ref,expected_sha256,mime_type,expected_byte_size,temporary_storage_key,received_byte_size,state \
         FROM linggan_media_upload_session WHERE session_ref = $1",
    )
    .bind(session_ref)
    .fetch_optional(database.pool())
    .await
    .map(|row| row.map(|row| media_upload_session_from_row(&row)))
    .map_err(ProducerRuntimeError::Internal)
}

pub async fn claim_media_upload_finalize(
    database: &Database,
    session_ref: Uuid,
) -> Result<MediaUploadFinalizeClaim, ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let row = sqlx::query(
        "SELECT session_ref,media_observation_ref,expected_sha256,mime_type,expected_byte_size,temporary_storage_key,received_byte_size,state,download_attempt_ref \
         FROM linggan_media_upload_session WHERE session_ref = $1 FOR UPDATE",
    )
    .bind(session_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?
    .ok_or(ProducerRuntimeError::MediaObservationNotFound)?;
    let session = media_upload_session_from_row(&row);
    let claim = match session.state.as_str() {
        "materialized" => {
            let download_attempt_ref = row.get::<Uuid, _>("download_attempt_ref");
            let materialization = sqlx::query(
                "SELECT materialization.materialization_ref,materialization.blob_sha256,materialization.local_asset_path,blob.mime_type,blob.byte_size \
                 FROM linggan_media_materialization materialization JOIN linggan_media_blob blob ON blob.sha256 = materialization.blob_sha256 \
                 WHERE materialization.download_attempt_ref = $1 ORDER BY materialization.verified_at DESC LIMIT 1",
            )
            .bind(download_attempt_ref)
            .fetch_one(&mut *tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
            MediaUploadFinalizeClaim::Materialized(MediaBlobAdmission {
                blob_sha256: materialization.get("blob_sha256"),
                local_asset_path: materialization.get("local_asset_path"),
                download_attempt_ref,
                materialization_ref: materialization.get("materialization_ref"),
                processing_jobs: Vec::new(),
            })
        }
        "ready_to_finalize" => {
            sqlx::query("UPDATE linggan_media_upload_session SET state = 'finalizing',updated_at = scope_001_now() WHERE session_ref = $1")
                .bind(session_ref).execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
            MediaUploadFinalizeClaim::Ready(session)
        }
        "receiving" => MediaUploadFinalizeClaim::Incomplete,
        // `finalizing` is a recoverable delivery state, not a truth state. The local process may
        // have stopped after the atomic filesystem promotion but before recording its receipt.
        // Retrying from the same immutable session is safe; the API serializes active requests.
        "finalizing" => MediaUploadFinalizeClaim::Ready(session),
        _ => MediaUploadFinalizeClaim::Busy,
    };
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(claim)
}

pub async fn complete_media_upload(
    database: &Database,
    session_ref: Uuid,
    download_attempt_ref: Uuid,
) -> Result<(), ProducerRuntimeError> {
    sqlx::query("UPDATE linggan_media_upload_session SET state = 'materialized',download_attempt_ref = $2,updated_at = scope_001_now() WHERE session_ref = $1 AND state = 'finalizing'")
        .bind(session_ref).bind(download_attempt_ref).execute(database.pool()).await.map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

pub async fn release_media_upload_finalize(
    database: &Database,
    session_ref: Uuid,
) -> Result<(), ProducerRuntimeError> {
    sqlx::query("UPDATE linggan_media_upload_session SET state = 'ready_to_finalize',updated_at = scope_001_now() WHERE session_ref = $1 AND state = 'finalizing'")
        .bind(session_ref).execute(database.pool()).await.map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

fn media_upload_session_from_row(row: &sqlx::postgres::PgRow) -> MediaUploadSession {
    MediaUploadSession {
        session_ref: row.get("session_ref"),
        media_observation_ref: row.get("media_observation_ref"),
        expected_sha256: row.get("expected_sha256"),
        mime_type: row.get("mime_type"),
        expected_byte_size: row.get("expected_byte_size"),
        temporary_storage_key: row.get("temporary_storage_key"),
        next_offset: row.get("received_byte_size"),
        state: row.get("state"),
    }
}

/// Runtime packages become a deliberately narrow read projection for the Evidence Library. It
/// reads only material the Browser Producer already delivered; it does not initiate platform
/// access, manufacture missing publication times, or turn package admission into a claim.
pub async fn read_runtime_library(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<DiscoveryLibraryProjection, sqlx::Error> {
    let window_days = query.published_window().map(published_window_days);
    let text = query.text().filter(|value| !value.trim().is_empty());
    let rows = sqlx::query(runtime_library_sql())
        .bind(window_days)
        .bind(text)
        .fetch_all(database.pool())
        .await?;
    let excluded_unknown_published_at = match query.published_window() {
        Some(_) => {
            sqlx::query_scalar::<_, i64>(runtime_unknown_time_sql())
                .bind(text)
                .fetch_one(database.pool())
                .await? as u64
        }
        None => 0,
    };
    let cards = rows
        .into_iter()
        .map(|row| DiscoveryLibraryCard {
            platform: row.get("platform"),
            platform_content_id: row.get("platform_content_id"),
            title: row.get("title"),
            creator_display_name: row.get("creator_display_name"),
            published_at_source_text: row.get("published_at_source_text"),
            published_at: row.get("published_at"),
            published_at_state: if row.get::<Option<String>, _>("published_at").is_some() {
                "KNOWN"
            } else {
                "UNKNOWN"
            },
            first_discovered_at: row.get("first_discovered_at"),
            observed_at: row.get("observed_at"),
            result_position: row.get("result_position"),
            coverage_visible_cards: row.get("coverage_visible_cards"),
            coverage_maximum_quota: row.get("coverage_maximum_quota"),
            coverage_stopped_reason: row.get("coverage_stopped_reason"),
            cover_presentation_state: if row
                .get::<Option<String>, _>("cover_local_asset_url")
                .is_some()
            {
                "LOCAL_MEDIA_AVAILABLE"
            } else {
                "MEDIA_NOT_ACQUIRED"
            },
            cover_local_asset_url: row.get("cover_local_asset_url"),
        })
        .collect();
    Ok(DiscoveryLibraryProjection {
        cards,
        excluded_unknown_published_at,
        time_view: time_view_code(query.time_view()),
    })
}

fn published_window_days(window: PublishedWindow) -> i32 {
    match window {
        PublishedWindow::Last7Days => 7,
        PublishedWindow::Last30Days => 30,
    }
}

fn time_view_code(view: EvidenceTimeView) -> &'static str {
    match view {
        EvidenceTimeView::LatestAcceptedDiscovery => "latest_accepted_discovery",
        EvidenceTimeView::PublishedLast7Days => "last_7_days",
        EvidenceTimeView::PublishedLast30Days => "last_30_days",
    }
}

fn runtime_library_sql() -> &'static str {
    "WITH raw AS ( \
       SELECT package.platform, package.package_kind, package.accepted_at, package.observed_at, package.coverage, task.task_spec, record.value AS record \
       FROM linggan_runtime_capture_package package \
       JOIN linggan_runtime_task task ON task.task_id = package.task_id \
       CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records') WITH ORDINALITY AS record(value, ordinal) \
       JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref = package.package_ref AND disposition.record_ordinal = record.ordinal - 1 \
       WHERE disposition.disposition IN ('accepted_for_library_discovery','accepted_for_library_content') \
       AND package.package_kind IN ('discovery_search','profile_discovery','content_detail') \
     ), normalized AS ( \
       SELECT platform, accepted_at, observed_at, coverage, task_spec, record, \
              COALESCE(record #>> '{sourceObject,externalId}', coverage #>> '{target,contentExternalId}') AS platform_content_id, \
              NULLIF(record #>> '{payload,title}', '') AS title, \
              NULLIF(COALESCE(record #>> '{payload,authorName}', record #>> '{payload,user,nickname}', record #>> '{payload,author,nickname}'), '') AS creator_display_name, \
              NULLIF(COALESCE(record #>> '{payload,publishedAtText}', record #>> '{payload,releaseDate}', record #>> '{payload,time}'), '') AS published_at_source_text, \
              CASE WHEN COALESCE(record #>> '{payload,publishedAt}', '') ~ '^[0-9]+$' AND (record #>> '{payload,publishedAt}')::numeric > 0 \
                THEN to_timestamp((record #>> '{payload,publishedAt}')::numeric / CASE WHEN length(record #>> '{payload,publishedAt}') < 11 THEN 1 ELSE 1000 END) END AS published_at, \
              concat_ws(' ', record #>> '{payload,title}', record #>> '{payload,content}', record #>> '{payload,bodyText}', record #>> '{payload,desc}', record #>> '{payload,text}', record #>> '{payload,contentText}') AS evidence_text, \
              COALESCE((record->>'resultPosition')::integer, 2147483647) AS result_position \
       FROM raw \
     ), grouped AS ( \
       SELECT platform, platform_content_id, max(title) AS title, max(creator_display_name) AS creator_display_name, \
              max(published_at_source_text) FILTER (WHERE published_at IS NOT NULL) AS published_at_source_text, max(published_at) AS published_at, \
              min(accepted_at) AS first_discovered_at, max(observed_at) AS observed_at, min(result_position) AS result_position, \
              max((coverage #>> '{layers,0,observed}')::integer) AS coverage_visible_cards, \
              max(COALESCE((task_spec->>'maximumQuota')::integer, 1)) AS coverage_maximum_quota, \
              max(COALESCE(coverage #>> '{layers,0,stoppedReason}', 'unknown')) AS coverage_stopped_reason, \
              string_agg(evidence_text, ' ') AS evidence_text \
       FROM normalized \
       WHERE platform_content_id IS NOT NULL \
       GROUP BY platform, platform_content_id \
     ) \
     SELECT grouped.platform, grouped.platform_content_id, grouped.title, grouped.creator_display_name, grouped.published_at_source_text, grouped.published_at::text AS published_at, \
            grouped.first_discovered_at::text AS first_discovered_at, grouped.observed_at AS observed_at, grouped.result_position, grouped.coverage_visible_cards, grouped.coverage_maximum_quota, grouped.coverage_stopped_reason, \
            (SELECT materialization.local_asset_path FROM linggan_media_slot slot \
              JOIN linggan_media_observation observation ON observation.slot_key = slot.slot_key \
              JOIN linggan_media_download_attempt download_attempt ON download_attempt.media_observation_ref = observation.observation_ref \
              JOIN linggan_media_materialization materialization ON materialization.download_attempt_ref = download_attempt.download_attempt_ref \
              WHERE slot.platform = grouped.platform AND slot.content_external_id = grouped.platform_content_id \
              ORDER BY CASE WHEN slot.role = 'cover' THEN 0 ELSE 1 END, slot.ordinal, materialization.verified_at DESC LIMIT 1) AS cover_local_asset_url \
     FROM grouped \
     WHERE ($1::integer IS NULL OR (grouped.published_at IS NOT NULL \
       AND grouped.published_at >= scope_001_now() - make_interval(days => $1) \
       AND grouped.published_at <= scope_001_now())) \
       AND ($2::text IS NULL OR lower(COALESCE(grouped.creator_display_name, '')) LIKE '%' || lower($2) || '%' \
         OR lower(COALESCE(grouped.title, '')) LIKE '%' || lower($2) || '%' \
         OR lower(COALESCE(grouped.evidence_text, '')) LIKE '%' || lower($2) || '%') \
     ORDER BY CASE WHEN $2::text IS NULL THEN 4 \
       WHEN lower(COALESCE(grouped.creator_display_name, '')) LIKE '%' || lower($2) || '%' THEN 1 \
       WHEN lower(COALESCE(grouped.title, '')) LIKE '%' || lower($2) || '%' THEN 2 ELSE 3 END, \
       grouped.first_discovered_at DESC, grouped.result_position ASC"
}

fn runtime_unknown_time_sql() -> &'static str {
    // This is intentionally the same admitted/scope/text surface as the Library query.  An
    // all-library unknown counter would make a narrow author/title query claim it excluded
    // material the user did not ask to inspect.
    "WITH normalized AS ( \
       SELECT package.platform, package.coverage, record.value AS record, \
              concat_ws(' ', record.value #>> '{payload,title}', record.value #>> '{payload,content}', record.value #>> '{payload,bodyText}', record.value #>> '{payload,desc}', record.value #>> '{payload,text}', record.value #>> '{payload,contentText}') AS evidence_text, \
              NULLIF(COALESCE(record.value #>> '{payload,authorName}', record.value #>> '{payload,user,nickname}', record.value #>> '{payload,author,nickname}'), '') AS creator_display_name, \
              NULLIF(record.value #>> '{payload,title}', '') AS title, \
              COALESCE(record.value #>> '{sourceObject,externalId}', package.coverage #>> '{target,contentExternalId}') AS platform_content_id, \
              CASE WHEN COALESCE(record.value #>> '{payload,publishedAt}', '') ~ '^[0-9]+$' AND (record.value #>> '{payload,publishedAt}')::numeric > 0 THEN 1 ELSE 0 END AS has_published_at \
       FROM linggan_runtime_capture_package package \
       CROSS JOIN LATERAL jsonb_array_elements(package.payload->'records') WITH ORDINALITY AS record(value, ordinal) \
       JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref = package.package_ref AND disposition.record_ordinal = record.ordinal - 1 \
       WHERE disposition.disposition IN ('accepted_for_library_discovery','accepted_for_library_content') \
         AND package.package_kind IN ('discovery_search','profile_discovery','content_detail') \
     ), matching AS ( \
       SELECT * FROM normalized WHERE platform_content_id IS NOT NULL \
       AND ($1::text IS NULL OR lower(COALESCE(creator_display_name, '')) LIKE '%' || lower($1) || '%' \
         OR lower(COALESCE(title, '')) LIKE '%' || lower($1) || '%' \
         OR lower(COALESCE(evidence_text, '')) LIKE '%' || lower($1) || '%') \
     ) SELECT count(*) FROM ( \
       SELECT platform, platform_content_id FROM matching GROUP BY platform, platform_content_id \
       HAVING max(has_published_at) = 0 \
     ) unknown_content"
}

pub async fn producer_runtime_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    // PostgreSQL still resolves a table mentioned inside EXISTS when the preceding
    // `to_regclass(...)` expression is false. Check the physical surface first so a legacy
    // LOCAL-001 database cleanly selects its discovery projection rather than reporting the
    // whole Evidence Library unavailable.
    let tables_exist = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('linggan_local_schema_migration') IS NOT NULL \
                AND to_regclass('linggan_runtime_task') IS NOT NULL \
                AND to_regclass('linggan_runtime_attempt') IS NOT NULL \
                AND to_regclass('linggan_runtime_capture_package') IS NOT NULL \
                AND to_regclass('linggan_runtime_submission_receipt') IS NOT NULL \
                AND to_regclass('linggan_media_slot') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await?;
    if !tables_exist {
        return Ok(false);
    }
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                        WHERE migration_id = '0004_plugin_runtime_all_capabilities')",
    )
    .fetch_one(database.pool())
    .await
}

pub async fn producer_runtime_has_packages(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS (SELECT 1 FROM linggan_runtime_capture_package)")
        .fetch_one(database.pool())
        .await
}

pub async fn create_producer_task(
    database: &Database,
    task: &ProducerTaskSpec,
) -> Result<RuntimeTaskOutcome, ProducerRuntimeError> {
    let task_hash = sha256_hex(&task.raw().to_string());
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let existing = sqlx::query(
        "SELECT task_spec_hash FROM linggan_runtime_task WHERE task_id = $1 FOR UPDATE",
    )
    .bind(task.task_id())
    .fetch_optional(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    // 派发任务只能由服务端的租约路径**创建**。少了这一条，插件就能自签一份
    // `source: scheduled` 的任务——而 scheduled 必须配 `server_authorized_leased`，
    // 等于自己发一张「服务端已授权」，整条授权链被绕过。
    //
    // 判据是「这个 task_id 此前不存在」，而不是「本次是不是插件来的」：**重放必须放行**，
    // 插件执行服务端派下来的任务时会把同一份规格原样再送一次，那不是新造。一律拦掉会让
    // 派发好的任务永远交不回结果。
    if existing.is_none() && task.source() == "scheduled" {
        return Err(ProducerRuntimeError::ScheduledTaskNotServerIssued);
    }
    let outcome = match existing {
        Some(row) if row.get::<String, _>("task_spec_hash") == task_hash => {
            RuntimeTaskOutcome::Replay {
                task_id: task.task_id(),
            }
        }
        Some(_) => RuntimeTaskOutcome::Conflict {
            task_id: task.task_id(),
        },
        None => {
            sqlx::query("INSERT INTO linggan_runtime_task (task_id, task_spec_hash, task_spec, source, platform, page_type) VALUES ($1,$2,$3,$4,$5,$6)")
                .bind(task.task_id()).bind(task_hash).bind(task.raw()).bind(task.source()).bind(task.platform()).bind(task.page_type())
                .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
            RuntimeTaskOutcome::Created {
                task_id: task.task_id(),
            }
        }
    };
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(outcome)
}

pub async fn start_producer_attempt(
    database: &Database,
    attempt: &ProducerAttempt,
) -> Result<RuntimeAttemptOutcome, ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let task_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM linggan_runtime_task WHERE task_id = $1)",
    )
    .bind(attempt.task_id())
    .fetch_one(&mut *tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !task_exists {
        return Err(ProducerRuntimeError::RoutingNotFound);
    }
    let existing = sqlx::query("SELECT task_id, producer_instance_id FROM linggan_runtime_attempt WHERE attempt_id = $1 FOR UPDATE")
        .bind(attempt.attempt_id()).fetch_optional(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    let outcome = match existing {
        Some(row)
            if row.get::<Uuid, _>("task_id") == attempt.task_id()
                && row.get::<Uuid, _>("producer_instance_id") == attempt.producer_instance_id() =>
        {
            RuntimeAttemptOutcome::Replay {
                attempt_id: attempt.attempt_id(),
                task_id: attempt.task_id(),
            }
        }
        Some(_) => RuntimeAttemptOutcome::Conflict {
            attempt_id: attempt.attempt_id(),
        },
        None => {
            sqlx::query("INSERT INTO linggan_runtime_attempt (attempt_id, task_id, producer_instance_id) VALUES ($1,$2,$3)")
                .bind(attempt.attempt_id()).bind(attempt.task_id()).bind(attempt.producer_instance_id())
                .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
            RuntimeAttemptOutcome::Started {
                attempt_id: attempt.attempt_id(),
                task_id: attempt.task_id(),
            }
        }
    };
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(outcome)
}

pub async fn submit_producer_package(
    database: &Database,
    submission: &ProducerSubmission,
) -> Result<RuntimeSubmissionOutcome, ProducerRuntimeError> {
    let package_hash = sha256_hex(&submission.capture_package().raw().to_string());
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    if let Some(outcome) = existing_submission(&mut tx, submission, &package_hash).await? {
        tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
        return Ok(outcome);
    }
    assert_attempt_owner(&mut tx, submission).await?;
    if terminal_package_exists(&mut tx, submission.attempt_id()).await? {
        tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
        return Ok(RuntimeSubmissionOutcome::Conflict {
            submission_id: submission.submission_id(),
        });
    }
    let package = submission.capture_package();
    sqlx::query("INSERT INTO linggan_runtime_capture_package (package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform,package_hash,observed_at,captured_at,coverage,checkpoint,payload) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
        .bind(package.package_ref()).bind(submission.attempt_id()).bind(submission.task_id()).bind(submission.producer_instance_id())
        .bind(package.package_kind()).bind(package.platform()).bind(&package_hash).bind(package.observed_at()).bind(package.captured_at())
        .bind(package.coverage()).bind(package.checkpoint()).bind(package.raw())
        .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    // 任务给的篇数上限必须在接纳时执行，而不只是在页面上显示。
    let task_spec: Value =
        sqlx::query_scalar("SELECT task_spec FROM linggan_runtime_task WHERE task_id = $1")
            .bind(submission.task_id())
            .fetch_optional(&mut *tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?
            .ok_or(ProducerRuntimeError::RoutingNotFound)?;
    let maximum_quota = task_spec
        .get("maximumQuota")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok());
    let task_binding_valid =
        crate::material_contract_validation::task_package_binding_valid(&task_spec, package);
    insert_record_dispositions(&mut tx, package, maximum_quota, task_binding_valid).await?;
    if task_binding_valid {
        crate::material_admission::insert_typed_materials(&mut tx, package).await?;
    }
    let receipt_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_runtime_submission_receipt (submission_id,task_id,attempt_id,producer_instance_id,package_hash,package_ref,receipt_ref) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(submission.submission_id()).bind(submission.task_id()).bind(submission.attempt_id()).bind(submission.producer_instance_id())
        .bind(&package_hash).bind(package.package_ref()).bind(receipt_ref)
        .execute(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(RuntimeSubmissionOutcome::Acknowledged {
        submission_id: submission.submission_id(),
        receipt_ref,
        package_ref: package.package_ref(),
        package_kind: package.package_kind().to_owned(),
    })
}

/// 逐条决定记录的处置。
///
/// `maximum_quota` 是任务给的篇数上限。**超出上限的记录照常入库**，只是把「本条超出了
/// 任务上限」记进 reason。
///
/// 这一点 2026-08-27 修正过一次：最初把超额记录判为隔离，Mog 指出那站不住——
/// **配额是「平台访问」的闸门，不是「数据入库」的闸门**。材料既然已经取回，那次访问的
/// 风险早已付掉；把它挡在语料库外并不能让访问没发生，只会让风险白付、情报白丢。真正
/// 该拦的地方在访问之前：任务的止损条件、租约到期、领任务时的当日额度过滤。
///
/// 但**超额本身必须留痕**。它是「执行端没有守住给它的边界」的证据，而 Coverage 也不能
/// 因此声称自己守住了那条边界。留痕的方式是 reason，不是把材料藏起来。
async fn insert_record_dispositions(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    maximum_quota: Option<i32>,
    task_binding_valid: bool,
) -> Result<(), ProducerRuntimeError> {
    for (ordinal, record) in package.records().iter().enumerate() {
        let beyond_quota = maximum_quota
            .is_some_and(|quota| i64::try_from(ordinal).unwrap_or(i64::MAX) >= i64::from(quota));
        let media_identity_conflict =
            if task_binding_valid && package.package_kind() == "media_slots" {
                crate::material_media::identity_conflict_reason(tx, package, record).await?
            } else {
                None
            };
        let (disposition, reason) = if !task_binding_valid {
            ("quarantined", "task_package_contract_mismatch")
        } else if let Some(reason) = media_identity_conflict {
            ("quarantined", reason)
        } else if let Some(disposition) =
            crate::material_contract_validation::record_disposition(package, ordinal, record)
        {
            disposition
        } else if library_card_record(record) {
            if matches!(
                package.package_kind(),
                "discovery_search" | "profile_discovery"
            ) {
                (
                    "accepted_for_library_discovery",
                    "typed_discovery_card_identity_valid",
                )
            } else {
                (
                    "accepted_for_library_content",
                    "typed_content_card_identity_valid",
                )
            }
        } else {
            // This generic runtime does not promote opaque collector records into Evidence.
            // A later type-specific admission can use this immutable record reference.
            ("retained_uninterpreted", "typed_evidence_admission_not_run")
        };
        // 超额不改变处置，只改变理由：材料照常可用，而「它超出了任务上限」这件事留在
        // 记录上，日后可查、可显示，不需要靠翻任务规格反推。
        let reason = if beyond_quota {
            format!("{reason}__beyond_task_maximum_quota")
        } else {
            reason.to_owned()
        };
        sqlx::query("INSERT INTO linggan_runtime_record_disposition (package_ref,record_ordinal,disposition,reason) VALUES ($1,$2,$3,$4)")
            .bind(package.package_ref()).bind(i32::try_from(ordinal).expect("package record count is bounded"))
            .bind(disposition).bind(&reason).execute(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    }
    Ok(())
}

fn library_card_record(record: &Value) -> bool {
    record
        .pointer("/sourceObject/externalId")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
        && record.get("payload").is_some_and(Value::is_object)
}

async fn existing_submission(
    tx: &mut Transaction<'_, Postgres>,
    submission: &ProducerSubmission,
    package_hash: &str,
) -> Result<Option<RuntimeSubmissionOutcome>, ProducerRuntimeError> {
    let row = sqlx::query("SELECT task_id,attempt_id,producer_instance_id,package_hash,receipt_ref,package_ref FROM linggan_runtime_submission_receipt WHERE submission_id = $1")
        .bind(submission.submission_id()).fetch_optional(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    Ok(row.map(|row| {
        let matches = row.get::<Uuid, _>("task_id") == submission.task_id()
            && row.get::<Uuid, _>("attempt_id") == submission.attempt_id()
            && row.get::<Uuid, _>("producer_instance_id") == submission.producer_instance_id()
            && row.get::<String, _>("package_hash") == package_hash;
        if !matches {
            return RuntimeSubmissionOutcome::Conflict {
                submission_id: submission.submission_id(),
            };
        }
        RuntimeSubmissionOutcome::Replay {
            submission_id: submission.submission_id(),
            receipt_ref: row.get("receipt_ref"),
            package_ref: row.get("package_ref"),
            package_kind: submission.capture_package().package_kind().to_owned(),
        }
    }))
}

async fn assert_attempt_owner(
    tx: &mut Transaction<'_, Postgres>,
    submission: &ProducerSubmission,
) -> Result<(), ProducerRuntimeError> {
    let row = sqlx::query("SELECT task_id,producer_instance_id FROM linggan_runtime_attempt WHERE attempt_id = $1 FOR UPDATE")
        .bind(submission.attempt_id()).fetch_optional(&mut **tx).await.map_err(ProducerRuntimeError::Internal)?;
    let Some(row) = row else {
        return Err(ProducerRuntimeError::RoutingNotFound);
    };
    if row.get::<Uuid, _>("task_id") != submission.task_id()
        || row.get::<Uuid, _>("producer_instance_id") != submission.producer_instance_id()
    {
        return Err(ProducerRuntimeError::AttemptIdentityMismatch);
    }
    Ok(())
}

async fn terminal_package_exists(
    tx: &mut Transaction<'_, Postgres>,
    attempt_id: Uuid,
) -> Result<bool, ProducerRuntimeError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM linggan_runtime_capture_package WHERE attempt_id = $1)",
    )
    .bind(attempt_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn processors_for_mime(mime_type: &str) -> &'static [&'static str] {
    if mime_type.starts_with("image/") {
        &["thumbnail", "image_ocr"]
    } else if mime_type.starts_with("video/") {
        &["thumbnail", "audio_extract", "asr", "video_frame_ocr"]
    } else {
        &[]
    }
}
