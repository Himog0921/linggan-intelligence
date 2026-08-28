//! Read-only media enrichment for the work-level material projection.

use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
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

struct SlotReadProjection {
    slots: Vec<Value>,
    preview_url: Option<String>,
    preview_purpose: Option<String>,
    bytes_state: &'static str,
    restriction_state: &'static str,
    limitations: Vec<&'static str>,
}

#[derive(Default)]
struct SlotAccumulator {
    slots: Vec<Value>,
    preview_url: Option<String>,
    preview_purpose: Option<String>,
    acquired: usize,
    failed: usize,
    restricted: bool,
    cleaned: bool,
    limitations: Vec<&'static str>,
}

pub(crate) async fn read(
    tx: &mut Transaction<'_, Postgres>,
    content_ref: Uuid,
    as_of: &str,
) -> Result<MediaReadProjection, sqlx::Error> {
    let slots = read_slots(tx, content_ref, as_of).await?;
    let (derivatives, ocr_state, asr_state, derivative_restricted) =
        read_derivatives(tx, content_ref, as_of).await?;
    Ok(MediaReadProjection {
        slots: slots.slots,
        derivatives,
        preview_url: slots.preview_url,
        preview_purpose: slots.preview_purpose,
        bytes_state: slots.bytes_state,
        ocr_state,
        asr_state,
        restriction_state: if derivative_restricted {
            "WITHDRAWN_OR_RESTRICTED"
        } else {
            slots.restriction_state
        },
        limitations: slots.limitations,
    })
}

async fn read_slots(
    tx: &mut Transaction<'_, Postgres>,
    content_ref: Uuid,
    as_of: &str,
) -> Result<SlotReadProjection, sqlx::Error> {
    let rows = sqlx::query(
        "WITH current_origin AS ( \
           SELECT DISTINCT ON (origin.slot_key) origin.* FROM linggan_material_media_origin origin \
           JOIN linggan_runtime_capture_package origin_package USING(package_ref) \
           WHERE origin.content_public_ref=$1 AND origin_package.accepted_at <= $2::timestamptz \
           ORDER BY origin.slot_key,origin.source_generation DESC \
         ) SELECT origin.observation_ref,origin.slot_key,origin.purpose,origin.producer_ordinal,origin.display_ordinal, \
             origin.display_order_state,origin.display_order_basis,origin.source_generation,origin.candidate_set_state, \
             origin.composite_state,origin.live_photo_still_state,origin.live_photo_motion_state,observation.observed_at, \
             (SELECT count(*) FROM linggan_material_media_candidate candidate WHERE candidate.observation_ref=origin.observation_ref) AS candidate_count, \
             replica.materialization_ref,replica.verified_at,replica.materialization_observation_ref, \
             blob.sha256,blob.mime_type,blob.byte_size,current_download.download_attempt_ref,current_download.terminal_reason, \
             disposition.restricted AS disposition_restricted,disposition.cleaned AS disposition_cleaned \
         FROM current_origin origin JOIN linggan_media_observation observation USING (observation_ref) \
         LEFT JOIN LATERAL (SELECT attempt.* FROM linggan_media_download_attempt attempt WHERE attempt.media_observation_ref=origin.observation_ref AND attempt.started_at <= $2::timestamptz ORDER BY attempt.started_at DESC LIMIT 1) current_download ON true \
         LEFT JOIN LATERAL (SELECT materialization.*,attempt.media_observation_ref AS materialization_observation_ref \
             FROM linggan_material_media_origin historical_origin \
             JOIN linggan_runtime_capture_package historical_package USING(package_ref) \
             JOIN linggan_media_download_attempt attempt ON attempt.media_observation_ref=historical_origin.observation_ref \
             JOIN linggan_media_materialization materialization ON materialization.download_attempt_ref=attempt.download_attempt_ref \
             WHERE historical_origin.slot_key=origin.slot_key AND historical_package.accepted_at <= $2::timestamptz \
               AND attempt.started_at <= $2::timestamptz AND materialization.verified_at <= $2::timestamptz \
             ORDER BY materialization.verified_at DESC LIMIT 1) replica ON true \
         LEFT JOIN linggan_media_blob blob ON blob.sha256=replica.blob_sha256 \
         LEFT JOIN LATERAL (SELECT bool_or(event.state='WITHDRAWN_OR_RESTRICTED') AS restricted, \
                 bool_or(event.state='BYTES_CLEANED') AS cleaned FROM linggan_material_media_disposition_event event \
             WHERE event.recorded_at <= $2::timestamptz AND (event.slot_key=origin.slot_key OR event.materialization_ref=replica.materialization_ref OR event.blob_sha256=blob.sha256) \
         ) disposition ON true \
         ORDER BY origin.producer_ordinal,origin.slot_key",
    )
    .bind(content_ref)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    let mut projection = SlotAccumulator {
        slots: Vec::with_capacity(rows.len()),
        ..SlotAccumulator::default()
    };
    for row in &rows {
        projection.push(row);
    }
    Ok(projection.finish(rows.len()))
}

impl SlotAccumulator {
    fn push(&mut self, row: &sqlx::postgres::PgRow) {
        let disposition_restricted = row
            .get::<Option<bool>, _>("disposition_restricted")
            .unwrap_or(false);
        let disposition_cleaned = row
            .get::<Option<bool>, _>("disposition_cleaned")
            .unwrap_or(false);
        let materialization_ref = row.get::<Option<Uuid>, _>("materialization_ref");
        let blob_sha256 = row.get::<Option<String>, _>("sha256");
        let mut local_asset_url =
            materialization_ref
                .zip(blob_sha256.as_deref())
                .map(|(materialization_ref, sha256)| {
                    format!("/api/local/media/{materialization_ref}/{sha256}")
                });
        let bytes_state = match (disposition_restricted, disposition_cleaned) {
            (true, _) => {
                self.restricted = true;
                local_asset_url = None;
                "WITHDRAWN_OR_RESTRICTED"
            }
            (false, true) => {
                self.cleaned = true;
                local_asset_url = None;
                "BYTES_CLEANED"
            }
            _ if row.get::<Option<Uuid>, _>("materialization_ref").is_some() => {
                self.acquired += 1;
                "ACQUIRED"
            }
            _ if row.get::<Option<String>, _>("terminal_reason").is_some() => {
                self.failed += 1;
                "FAILED"
            }
            _ => "NOT_OBSERVED",
        };
        if self.preview_url.is_none() && local_asset_url.is_some() {
            self.preview_url = local_asset_url.clone();
            self.preview_purpose = Some(row.get("purpose"));
        }
        let candidate_count: i64 = row.get("candidate_count");
        if candidate_count > 1
            && !self
                .limitations
                .contains(&"ACTUAL_CANDIDATE_NOT_REPORTED_BY_PRODUCER")
        {
            self.limitations
                .push("ACTUAL_CANDIDATE_NOT_REPORTED_BY_PRODUCER");
        }
        let purpose: String = row.get("purpose");
        let components = if purpose == "live_photo" {
            serde_json::json!({"bundleState":row.get::<String,_>("composite_state"),"stillState":row.get::<Option<String>,_>("live_photo_still_state"),"motionState":row.get::<Option<String>,_>("live_photo_motion_state")})
        } else {
            Value::Null
        };
        self.slots.push(serde_json::json!({
            "slotKey":row.get::<String,_>("slot_key"),"purpose":purpose,
            "producerOrdinal":row.get::<i32,_>("producer_ordinal"),"displayOrdinal":row.get::<Option<i32>,_>("display_ordinal"),
            "displayOrderState":row.get::<String,_>("display_order_state"),"displayOrderBasis":row.get::<String,_>("display_order_basis"),
            "origin":{"observationRef":row.get::<Uuid,_>("observation_ref"),"sourceGeneration":row.get::<i32,_>("source_generation"),"observedAt":row.get::<String,_>("observed_at"),"candidateUriCount":candidate_count,"candidateSetState":row.get::<String,_>("candidate_set_state"),"actualDownloadCandidateState":"UNKNOWN"},
            "components":components,"bytesState":bytes_state,"replicaState":if local_asset_url.is_some(){"VERIFIED_AT_MATERIALIZATION"}else{"UNKNOWN"},
            "dispositionState":if disposition_restricted{"WITHDRAWN_OR_RESTRICTED"}else if disposition_cleaned{"BYTES_CLEANED"}else{"UNKNOWN"},"localAssetUrl":local_asset_url,
            "currentDownloadAttemptRef":row.get::<Option<Uuid>,_>("download_attempt_ref"),
            "replica":{"materializationRef":row.get::<Option<Uuid>,_>("materialization_ref"),"originObservationRef":row.get::<Option<Uuid>,_>("materialization_observation_ref"),"isCurrentOrigin":row.get::<Option<Uuid>,_>("materialization_observation_ref")==Some(row.get::<Uuid,_>("observation_ref"))},
            "blob":{"sha256":row.get::<Option<String>,_>("sha256"),"mimeType":row.get::<Option<String>,_>("mime_type"),"byteSize":row.get::<Option<i64>,_>("byte_size")}
        }));
    }

    fn finish(self, slot_count: usize) -> SlotReadProjection {
        SlotReadProjection {
            bytes_state: aggregate_bytes_state(
                self.restricted,
                self.cleaned,
                self.acquired,
                self.failed,
                slot_count,
            ),
            restriction_state: if self.restricted {
                "WITHDRAWN_OR_RESTRICTED"
            } else if self.cleaned {
                "BYTES_CLEANED"
            } else {
                "UNKNOWN"
            },
            slots: self.slots,
            preview_url: self.preview_url,
            preview_purpose: self.preview_purpose,
            limitations: self.limitations,
        }
    }
}

async fn read_derivatives(
    tx: &mut Transaction<'_, Postgres>,
    content_ref: Uuid,
    as_of: &str,
) -> Result<(Vec<Value>, &'static str, &'static str, bool), sqlx::Error> {
    let derivative_rows = sqlx::query(
        "SELECT job.job_ref,job.slot_key,job.processor_kind,job.processor_version,job.input_scope, \
             event.state,event.reason,derivative.derivative_ref,derivative.derivative_kind,derivative.storage_key, \
             disposition.restricted AS disposition_restricted \
         FROM linggan_media_processing_job job JOIN linggan_media_slot slot USING (slot_key) \
         LEFT JOIN LATERAL (SELECT state,reason FROM linggan_media_processing_job_event event WHERE event.job_ref=job.job_ref AND occurred_at <= $2::timestamptz ORDER BY occurred_at DESC LIMIT 1) event ON true \
         LEFT JOIN linggan_media_derivative derivative ON derivative.job_ref=job.job_ref AND derivative.created_at <= $2::timestamptz \
         LEFT JOIN LATERAL (SELECT bool_or(disposition.state='WITHDRAWN_OR_RESTRICTED') AS restricted \
             FROM linggan_material_media_disposition_event disposition \
             WHERE disposition.recorded_at <= $2::timestamptz AND (disposition.derivative_ref=derivative.derivative_ref OR disposition.blob_sha256=job.blob_sha256 OR disposition.slot_key=job.slot_key)) disposition ON true \
         JOIN linggan_material_content content ON content.platform=slot.platform AND content.content_external_id=slot.content_external_id \
         WHERE content.public_ref=$1 AND job.created_at <= $2::timestamptz ORDER BY job.created_at",
    ).bind(content_ref).bind(as_of).fetch_all(&mut **tx).await?;
    let mut derivatives = Vec::with_capacity(derivative_rows.len());
    let mut ocr_state = "UNKNOWN";
    let mut asr_state = "UNKNOWN";
    let mut any_restricted = false;
    for row in derivative_rows {
        let event_state: Option<String> = row.get("state");
        let reason: Option<String> = row.get("reason");
        let has_derivative = row.get::<Option<Uuid>, _>("derivative_ref").is_some();
        let restricted = row
            .get::<Option<bool>, _>("disposition_restricted")
            .unwrap_or(false);
        any_restricted |= restricted;
        let state = match (
            restricted,
            event_state.as_deref(),
            reason.as_deref(),
            has_derivative,
        ) {
            (true, _, _, _) => "WITHDRAWN_OR_RESTRICTED",
            (false, Some("pending"), Some("provider_not_enabled"), _) => "NOT_ENABLED",
            (false, Some("pending"), _, _) => "QUEUED",
            (false, Some("running"), _, _) => "PROCESSING",
            (false, Some("failed"), _, _) => "FAILED",
            (false, Some("succeeded"), _, true) => "ACQUIRED",
            (false, Some("succeeded"), _, false) => "UNKNOWN",
            _ => "UNKNOWN",
        };
        let kind: String = row.get("processor_kind");
        if kind == "image_ocr" || kind == "video_frame_ocr" {
            ocr_state = state;
        }
        if kind == "asr" {
            asr_state = state;
        }
        let derivative_ref = row.get::<Option<Uuid>, _>("derivative_ref");
        let storage_key = row.get::<Option<String>, _>("storage_key");
        let source_location = if restricted {
            Value::Null
        } else if derivative_ref.is_some()
            && storage_key
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        {
            serde_json::json!({
                "localAssetUrl":format!("/api/local/derivative/{}",derivative_ref.expect("checked"))
            })
        } else {
            Value::Null
        };
        derivatives.push(serde_json::json!({"jobRef":row.get::<Uuid,_>("job_ref"),"slotKey":row.get::<Option<String>,_>("slot_key"),"kind":row.get::<Option<String>,_>("derivative_kind"),"state":state,"dispositionState":if restricted{"WITHDRAWN_OR_RESTRICTED"}else{"UNKNOWN"},"processorVersion":row.get::<String,_>("processor_version"),"sourceScope":row.get::<String,_>("input_scope"),"sourceLocation":source_location,"reason":reason}));
    }
    Ok((derivatives, ocr_state, asr_state, any_restricted))
}

fn aggregate_bytes_state(
    restricted: bool,
    cleaned: bool,
    acquired: usize,
    failed: usize,
    slot_count: usize,
) -> &'static str {
    if restricted {
        "WITHDRAWN_OR_RESTRICTED"
    } else if cleaned && acquired == 0 {
        "BYTES_CLEANED"
    } else if acquired > 0 && (failed > 0 || acquired < slot_count) {
        "PARTIAL"
    } else if acquired > 0 {
        "ACQUIRED"
    } else if failed > 0 {
        "FAILED"
    } else if slot_count == 0 {
        "UNKNOWN"
    } else {
        "NOT_OBSERVED"
    }
}
