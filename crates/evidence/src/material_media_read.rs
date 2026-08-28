//! Read-only media enrichment for the work-level material projection.

use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

pub(crate) struct MediaReadProjection {
    pub slots: Vec<Value>,
    pub derivatives: Vec<Value>,
    pub preview_url: Option<String>,
    pub preview_purpose: Option<String>,
    pub bytes_state: &'static str,
    pub ocr_state: &'static str,
    pub asr_state: &'static str,
    pub restriction_state: &'static str,
    pub limitations: Vec<&'static str>,
}

pub(crate) async fn read(
    database: &Database,
    content_ref: Uuid,
) -> Result<MediaReadProjection, sqlx::Error> {
    let rows = sqlx::query(
        "WITH current_origin AS ( \
           SELECT DISTINCT ON (origin.slot_key) origin.* FROM linggan_material_media_origin origin \
           WHERE origin.content_public_ref=$1 ORDER BY origin.slot_key,origin.source_generation DESC \
         ) SELECT origin.observation_ref,origin.slot_key,origin.purpose,origin.producer_ordinal,origin.display_ordinal, \
             origin.display_order_state,origin.display_order_basis,origin.source_generation,origin.candidate_set_state, \
             origin.composite_state,origin.live_photo_still_state,origin.live_photo_motion_state,observation.observed_at, \
             (SELECT count(*) FROM linggan_material_media_candidate candidate WHERE candidate.observation_ref=origin.observation_ref) AS candidate_count, \
             materialization.materialization_ref,materialization.local_asset_path,materialization.verified_at, \
             blob.sha256,blob.mime_type,blob.byte_size,download.download_attempt_ref,download.terminal_reason, \
             disposition.state AS disposition_state \
         FROM current_origin origin JOIN linggan_media_observation observation USING (observation_ref) \
         LEFT JOIN LATERAL (SELECT attempt.* FROM linggan_media_download_attempt attempt WHERE attempt.media_observation_ref=origin.observation_ref ORDER BY attempt.started_at DESC LIMIT 1) download ON true \
         LEFT JOIN linggan_media_materialization materialization ON materialization.download_attempt_ref=download.download_attempt_ref \
         LEFT JOIN linggan_media_blob blob ON blob.sha256=materialization.blob_sha256 \
         LEFT JOIN LATERAL (SELECT event.state FROM linggan_material_media_disposition_event event \
             WHERE event.slot_key=origin.slot_key OR event.materialization_ref=materialization.materialization_ref OR event.blob_sha256=blob.sha256 \
             ORDER BY event.effective_at DESC,event.recorded_at DESC LIMIT 1) disposition ON true \
         ORDER BY origin.producer_ordinal,origin.slot_key",
    )
    .bind(content_ref)
    .fetch_all(database.pool())
    .await?;
    let mut slots = Vec::with_capacity(rows.len());
    let mut preview_url = None;
    let mut preview_purpose = None;
    let mut acquired = 0;
    let mut failed = 0;
    let mut restricted = false;
    let mut cleaned = false;
    let mut limitations = Vec::new();
    for row in &rows {
        let disposition: Option<String> = row.get("disposition_state");
        let mut local_asset_url: Option<String> = row.get("local_asset_path");
        let bytes_state = match disposition.as_deref() {
            Some("WITHDRAWN_OR_RESTRICTED") => {
                restricted = true;
                local_asset_url = None;
                "WITHDRAWN_OR_RESTRICTED"
            }
            Some("BYTES_CLEANED") => {
                cleaned = true;
                local_asset_url = None;
                "BYTES_CLEANED"
            }
            _ if row.get::<Option<Uuid>, _>("materialization_ref").is_some() => {
                acquired += 1;
                "ACQUIRED"
            }
            _ if row.get::<Option<String>, _>("terminal_reason").is_some() => {
                failed += 1;
                "FAILED"
            }
            _ => "NOT_OBSERVED",
        };
        if preview_url.is_none() && local_asset_url.is_some() {
            preview_url = local_asset_url.clone();
            preview_purpose = Some(row.get("purpose"));
        }
        let candidate_count: i64 = row.get("candidate_count");
        if candidate_count > 1
            && !limitations.contains(&"ACTUAL_CANDIDATE_NOT_REPORTED_BY_PRODUCER")
        {
            limitations.push("ACTUAL_CANDIDATE_NOT_REPORTED_BY_PRODUCER");
        }
        let purpose: String = row.get("purpose");
        let components = if purpose == "live_photo" {
            serde_json::json!({"bundleState":row.get::<String,_>("composite_state"),"stillState":row.get::<Option<String>,_>("live_photo_still_state"),"motionState":row.get::<Option<String>,_>("live_photo_motion_state")})
        } else {
            Value::Null
        };
        slots.push(serde_json::json!({
            "slotKey":row.get::<String,_>("slot_key"),"purpose":purpose,
            "producerOrdinal":row.get::<i32,_>("producer_ordinal"),"displayOrdinal":row.get::<Option<i32>,_>("display_ordinal"),
            "displayOrderState":row.get::<String,_>("display_order_state"),"displayOrderBasis":row.get::<String,_>("display_order_basis"),
            "origin":{"observationRef":row.get::<Uuid,_>("observation_ref"),"sourceGeneration":row.get::<i32,_>("source_generation"),"observedAt":row.get::<String,_>("observed_at"),"candidateUriCount":candidate_count,"candidateSetState":row.get::<String,_>("candidate_set_state"),"actualDownloadCandidateState":"UNKNOWN"},
            "components":components,"bytesState":bytes_state,"replicaState":if local_asset_url.is_some(){"VERIFIED_AT_MATERIALIZATION"}else{"UNKNOWN"},
            "dispositionState":disposition.unwrap_or_else(||"UNKNOWN".to_owned()),"localAssetUrl":local_asset_url,
            "downloadAttemptRef":row.get::<Option<Uuid>,_>("download_attempt_ref"),"blob":{"sha256":row.get::<Option<String>,_>("sha256"),"mimeType":row.get::<Option<String>,_>("mime_type"),"byteSize":row.get::<Option<i64>,_>("byte_size")}
        }));
    }
    let derivative_rows = sqlx::query(
        "SELECT job.job_ref,job.slot_key,job.processor_kind,job.processor_version,job.input_scope, \
             event.state,event.reason,derivative.derivative_ref,derivative.derivative_kind \
         FROM linggan_media_processing_job job JOIN linggan_media_slot slot USING (slot_key) \
         LEFT JOIN LATERAL (SELECT state,reason FROM linggan_media_processing_job_event event WHERE event.job_ref=job.job_ref ORDER BY occurred_at DESC LIMIT 1) event ON true \
         LEFT JOIN linggan_media_derivative derivative ON derivative.job_ref=job.job_ref \
         JOIN linggan_material_content content ON content.platform=slot.platform AND content.content_external_id=slot.content_external_id \
         WHERE content.public_ref=$1 ORDER BY job.created_at",
    ).bind(content_ref).fetch_all(database.pool()).await?;
    let mut derivatives = Vec::with_capacity(derivative_rows.len());
    let mut ocr_state = "UNKNOWN";
    let mut asr_state = "UNKNOWN";
    for row in derivative_rows {
        let event_state: Option<String> = row.get("state");
        let reason: Option<String> = row.get("reason");
        let has_derivative = row.get::<Option<Uuid>, _>("derivative_ref").is_some();
        let state = match (event_state.as_deref(), reason.as_deref(), has_derivative) {
            (Some("pending"), Some("provider_not_enabled"), _) => "NOT_ENABLED",
            (Some("pending"), _, _) => "QUEUED",
            (Some("running"), _, _) => "PROCESSING",
            (Some("failed"), _, _) => "FAILED",
            (Some("succeeded"), _, true) => "ACQUIRED",
            (Some("succeeded"), _, false) => "UNKNOWN",
            _ => "UNKNOWN",
        };
        let kind: String = row.get("processor_kind");
        if kind == "image_ocr" || kind == "video_frame_ocr" {
            ocr_state = state;
        }
        if kind == "asr" {
            asr_state = state;
        }
        derivatives.push(serde_json::json!({"jobRef":row.get::<Uuid,_>("job_ref"),"slotKey":row.get::<Option<String>,_>("slot_key"),"kind":row.get::<Option<String>,_>("derivative_kind"),"state":state,"processorVersion":row.get::<String,_>("processor_version"),"sourceLocation":{"inputScope":row.get::<String,_>("input_scope")},"reason":reason}));
    }
    let bytes_state = if restricted {
        "WITHDRAWN_OR_RESTRICTED"
    } else if cleaned && acquired == 0 {
        "BYTES_CLEANED"
    } else if acquired > 0 && (failed > 0 || acquired < rows.len()) {
        "PARTIAL"
    } else if acquired > 0 {
        "ACQUIRED"
    } else if failed > 0 {
        "FAILED"
    } else if rows.is_empty() {
        "UNKNOWN"
    } else {
        "NOT_OBSERVED"
    };
    Ok(MediaReadProjection {
        slots,
        derivatives,
        preview_url,
        preview_purpose,
        bytes_state,
        ocr_state,
        asr_state,
        restriction_state: if restricted {
            "WITHDRAWN_OR_RESTRICTED"
        } else if cleaned {
            "BYTES_CLEANED"
        } else {
            "UNKNOWN"
        },
        limitations,
    })
}
