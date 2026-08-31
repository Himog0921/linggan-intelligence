//! Read-only media enrichment for the work-level material projection.

use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const DETAIL_MEDIA_SLOT_LIMIT: usize = 20;
const DETAIL_DERIVATIVE_LIMIT: usize = 20;

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
    pub slot_receipt: Value,
    pub derivative_receipt: Value,
    pub resource: Value,
}

struct SlotReadProjection {
    slots: Vec<Value>,
    bytes_state: &'static str,
    restriction_state: &'static str,
    limitations: Vec<&'static str>,
    receipt: Value,
}

#[derive(Default)]
struct SlotAccumulator {
    slots: Vec<Value>,
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
    let (derivatives, derivative_receipt, ocr_state, asr_state, derivative_restricted) =
        read_derivatives(tx, content_ref, as_of).await?;
    let (resource, preview_url, preview_purpose) =
        build_media_resource(&slots.slots, &derivatives, ocr_state, asr_state);
    Ok(MediaReadProjection {
        slots: slots.slots,
        derivatives,
        preview_url,
        preview_purpose,
        bytes_state: slots.bytes_state,
        ocr_state,
        asr_state,
        restriction_state: if derivative_restricted {
            "WITHDRAWN_OR_RESTRICTED"
        } else {
            slots.restriction_state
        },
        limitations: slots.limitations,
        slot_receipt: slots.receipt,
        derivative_receipt,
        resource,
    })
}

fn build_media_resource(
    slots: &[Value],
    derivatives: &[Value],
    ocr_state: &'static str,
    asr_state: &'static str,
) -> (Value, Option<String>, Option<String>) {
    let (avatars, covers, images, videos) = partition_resource_slots(slots);
    let avatar = select_avatar(&avatars);
    let cover = select_cover(&covers, &images, &videos, derivatives);
    let (ocr_resources, transcript_resources) = derivative_resources(derivatives);
    let resource = serde_json::json!({
        "contractVersion":"linggan.media-resource.v1",
        "state":if slots.is_empty() && derivatives.is_empty(){"NOT_OBSERVED"}else{"OBSERVED"},
        "avatar":avatar,
        "cover":{
            "state":cover.state,
            "relationship":"content.cover",
            "sourceRelationship":cover.source_relationship,
            "selectedBy":cover.selected_by,
            "fallbackUsed":cover.selected_by != "explicit_cover" && cover.selected_by != "none",
            "localAssetUrl":cover.url.clone(),
            "intrinsicDimensions":cover.dimensions
        },
        "coverCandidates":covers,
        "images":images,
        "video":{
            "state":if videos.is_empty(){"NOT_OBSERVED"}else{"OBSERVED"},
            "relationship":"content.video",
            "items":videos
        },
        "ocr":{"state":ocr_state,"relationship":"content.ocr","resources":ocr_resources},
        "transcript":{"state":asr_state,"relationship":"content.transcript","resources":transcript_resources},
        "commentImages":{"state":"NOT_OBSERVED","relationship":"comment.image","items":[]}
    });
    (resource, cover.url, cover.purpose)
}

struct CoverSelection {
    url: Option<String>,
    selected_by: &'static str,
    purpose: Option<String>,
    source_relationship: &'static str,
    state: &'static str,
    dimensions: Value,
}

fn resource_slot(slot: &Value) -> Value {
    let mut value = slot.clone();
    let purpose = slot
        .get("purpose")
        .and_then(Value::as_str)
        .unwrap_or("body_image");
    let relationship = slot
        .get("relationshipKind")
        .and_then(Value::as_str)
        .unwrap_or(match purpose {
            "author_avatar" => "author.avatar",
            "cover" => "content.cover",
            "video" | "live_photo" => "content.video",
            _ => "content.image",
        });
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "relationship".to_owned(),
            Value::String(relationship.to_owned()),
        );
        object.insert(
            "relationshipOrdinal".to_owned(),
            slot.get("relationshipOrdinal")
                .filter(|value| !value.is_null())
                .cloned()
                .or_else(|| slot.get("producerOrdinal").cloned())
                .unwrap_or(Value::Null),
        );
        object.insert(
            "intrinsicDimensions".to_owned(),
            slot.pointer("/blob/intrinsicDimensions")
                .cloned()
                .unwrap_or_else(
                    || serde_json::json!({"state":"UNKNOWN","width":null,"height":null}),
                ),
        );
    }
    value
}

fn partition_resource_slots(slots: &[Value]) -> (Vec<Value>, Vec<Value>, Vec<Value>, Vec<Value>) {
    let select = |purposes: &[&str]| {
        slots
            .iter()
            .filter(|slot| {
                slot.get("purpose")
                    .and_then(Value::as_str)
                    .is_some_and(|purpose| purposes.contains(&purpose))
            })
            .map(resource_slot)
            .collect::<Vec<_>>()
    };
    (
        select(&["author_avatar"]),
        select(&["cover"]),
        select(&["body_image"]),
        select(&["video", "live_photo"]),
    )
}

fn select_avatar(avatars: &[Value]) -> Value {
    let selected = avatars
        .iter()
        .find(|avatar| local_handle(avatar).is_some())
        .or_else(|| avatars.first());
    let Some(selected) = selected else {
        return serde_json::json!({
            "state":"NOT_OBSERVED","relationship":"author.avatar","localAssetUrl":null
        });
    };
    let mut avatar = selected.clone();
    if let Some(object) = avatar.as_object_mut() {
        object.insert(
            "state".to_owned(),
            Value::String(
                selected
                    .get("bytesState")
                    .and_then(Value::as_str)
                    .unwrap_or("OBSERVED")
                    .to_owned(),
            ),
        );
        object.insert(
            "relationship".to_owned(),
            Value::String("author.avatar".to_owned()),
        );
    }
    avatar
}

fn local_handle(value: &Value) -> Option<String> {
    (value.pointer("/blob/deliveryState").and_then(Value::as_str) == Some("INLINE_SAFE"))
        .then(|| {
            value
                .get("localAssetUrl")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .flatten()
}

fn select_cover(
    covers: &[Value],
    images: &[Value],
    videos: &[Value],
    derivatives: &[Value],
) -> CoverSelection {
    let explicit_cover = covers.iter().find_map(local_handle);
    let first_image = images.iter().find_map(local_handle);
    let video_slot_keys = videos
        .iter()
        .filter_map(|slot| slot.get("slotKey").and_then(Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    let video_poster = derivatives.iter().find_map(|derivative| {
        let slot_key = derivative.get("slotKey").and_then(Value::as_str)?;
        (derivative.get("kind").and_then(Value::as_str) == Some("thumbnail")
            && video_slot_keys.contains(slot_key))
        .then(|| {
            derivative
                .pointer("/sourceLocation/localAssetUrl")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .flatten()
    });
    let (url, selected_by, purpose, source_relationship) = if let Some(url) = explicit_cover {
        (
            Some(url),
            "explicit_cover",
            Some("cover".to_owned()),
            "content.cover",
        )
    } else if let Some(url) = first_image {
        (
            Some(url),
            "first_body_image",
            Some("body_image".to_owned()),
            "content.image",
        )
    } else if let Some(url) = video_poster {
        (
            Some(url),
            "video_poster",
            Some("video".to_owned()),
            "content.video",
        )
    } else {
        (None, "none", None, "content.cover")
    };
    let dimensions = match selected_by {
        "explicit_cover" => covers.iter().find(|slot| local_handle(slot).is_some()),
        "first_body_image" => images.iter().find(|slot| local_handle(slot).is_some()),
        _ => None,
    }
    .and_then(|slot| slot.get("intrinsicDimensions"))
    .cloned()
    .unwrap_or_else(|| serde_json::json!({"state":"UNKNOWN","width":null,"height":null}));
    let state = if url.is_some() {
        "AVAILABLE"
    } else if covers.is_empty() && images.is_empty() && videos.is_empty() {
        "NOT_OBSERVED"
    } else {
        "OBSERVED"
    };
    CoverSelection {
        url,
        selected_by,
        purpose,
        source_relationship,
        state,
        dimensions,
    }
}

fn derivative_resources(derivatives: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let ocr = derivatives
        .iter()
        .filter(|item| {
            matches!(
                item.get("kind").and_then(Value::as_str),
                Some("ocr_text" | "frame_ocr_text")
            )
        })
        .cloned()
        .collect();
    let transcript = derivatives
        .iter()
        .filter(|item| item.get("kind").and_then(Value::as_str) == Some("asr_text"))
        .cloned()
        .collect();
    (ocr, transcript)
}

async fn read_slots(
    tx: &mut Transaction<'_, Postgres>,
    content_ref: Uuid,
    as_of: &str,
) -> Result<SlotReadProjection, sqlx::Error> {
    let mut rows = sqlx::query(
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
             blob.sha256,blob.mime_type,blob.byte_size,blob.pixel_width,blob.pixel_height,blob.duration_ms,relation.relationship_kind,relation.relationship_ordinal,current_download.download_attempt_ref,current_download.terminal_reason, \
             disposition.restricted AS disposition_restricted,disposition.cleaned AS disposition_cleaned,disposition.slot_restricted, \
             excluded.newer_disposed AS newer_disposed,count(*) OVER() AS total_count \
         FROM current_origin origin JOIN linggan_media_observation observation USING (observation_ref) \
         LEFT JOIN LATERAL (SELECT attempt.* FROM linggan_media_download_attempt attempt WHERE attempt.media_observation_ref=origin.observation_ref AND attempt.started_at <= $2::timestamptz ORDER BY attempt.started_at DESC LIMIT 1) current_download ON true \
         LEFT JOIN LATERAL (SELECT materialization.*,attempt.media_observation_ref AS materialization_observation_ref \
             FROM linggan_material_media_origin historical_origin \
             JOIN linggan_runtime_capture_package historical_package USING(package_ref) \
             JOIN linggan_media_download_attempt attempt ON attempt.media_observation_ref=historical_origin.observation_ref \
             JOIN linggan_media_materialization materialization ON materialization.download_attempt_ref=attempt.download_attempt_ref \
             JOIN linggan_media_blob candidate_blob ON candidate_blob.sha256=materialization.blob_sha256 \
             WHERE historical_origin.slot_key=origin.slot_key AND historical_package.accepted_at <= $2::timestamptz \
               AND attempt.started_at <= $2::timestamptz AND materialization.verified_at <= $2::timestamptz \
               AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition event \
                 WHERE (event.slot_key=origin.slot_key \
                   OR event.materialization_ref=materialization.materialization_ref OR event.blob_sha256=candidate_blob.sha256)) \
             ORDER BY materialization.verified_at DESC LIMIT 1) replica ON true \
         LEFT JOIN linggan_media_blob blob ON blob.sha256=replica.blob_sha256 \
         LEFT JOIN linggan_media_resource_relation relation ON relation.slot_key=origin.slot_key \
         LEFT JOIN LATERAL (SELECT bool_or(event.state='WITHDRAWN_OR_RESTRICTED') AS restricted, \
                 bool_or(event.slot_key=origin.slot_key AND event.state='WITHDRAWN_OR_RESTRICTED') AS slot_restricted, \
                 bool_or(event.state='BYTES_CLEANED') AS cleaned \
             FROM linggan_current_material_media_disposition event \
             WHERE (event.slot_key=origin.slot_key \
               OR event.materialization_ref IN (SELECT scoped_materialization.materialization_ref FROM linggan_material_media_origin scoped_origin JOIN linggan_media_download_attempt scoped_attempt ON scoped_attempt.media_observation_ref=scoped_origin.observation_ref JOIN linggan_media_materialization scoped_materialization USING(download_attempt_ref) WHERE scoped_origin.slot_key=origin.slot_key) \
               OR event.blob_sha256 IN (SELECT scoped_materialization.blob_sha256 FROM linggan_material_media_origin scoped_origin JOIN linggan_media_download_attempt scoped_attempt ON scoped_attempt.media_observation_ref=scoped_origin.observation_ref JOIN linggan_media_materialization scoped_materialization USING(download_attempt_ref) WHERE scoped_origin.slot_key=origin.slot_key)) \
         ) disposition ON true \
         LEFT JOIN LATERAL (SELECT EXISTS (SELECT 1 \
             FROM linggan_material_media_origin excluded_origin \
             JOIN linggan_media_download_attempt excluded_attempt ON excluded_attempt.media_observation_ref=excluded_origin.observation_ref \
             JOIN linggan_media_materialization excluded_materialization USING(download_attempt_ref) \
             WHERE excluded_origin.slot_key=origin.slot_key AND excluded_materialization.verified_at <= $2::timestamptz \
               AND (replica.verified_at IS NULL OR excluded_materialization.verified_at > replica.verified_at) \
               AND EXISTS (SELECT 1 FROM linggan_current_material_media_disposition excluded_event \
                 WHERE (excluded_event.slot_key=origin.slot_key \
                   OR excluded_event.materialization_ref=excluded_materialization.materialization_ref \
                   OR excluded_event.blob_sha256=excluded_materialization.blob_sha256))) AS newer_disposed \
         ) excluded ON true \
         ORDER BY origin.producer_ordinal,origin.slot_key LIMIT 21",
    )
    .bind(content_ref)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    let total = rows.first().map_or(0_i64, |row| row.get("total_count"));
    let truncated = rows.len() > DETAIL_MEDIA_SLOT_LIMIT;
    if truncated {
        rows.truncate(DETAIL_MEDIA_SLOT_LIMIT);
    }
    let next_cursor = truncated
        .then(|| {
            rows.last()
                .map(|row| format!("slot:{}", row.get::<String, _>("slot_key")))
        })
        .flatten();
    let component_work_ready: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_attribute \
         WHERE attrelid=to_regclass('linggan_media_acquisition_work') \
           AND attname='component_kind' AND NOT attisdropped)",
    )
    .fetch_one(&mut **tx)
    .await?;
    let mut acquired_components = std::collections::HashSet::new();
    if component_work_ready {
        for row in sqlx::query(
            "SELECT work.observation_ref,work.component_kind \
             FROM linggan_media_acquisition_work work \
             JOIN linggan_material_media_origin origin USING(observation_ref) \
             WHERE origin.content_public_ref=$1 AND work.state='completed'",
        )
        .bind(content_ref)
        .fetch_all(&mut **tx)
        .await?
        {
            acquired_components.insert((
                row.get::<Uuid, _>("observation_ref"),
                row.get::<String, _>("component_kind"),
            ));
        }
    }
    let mut projection = SlotAccumulator {
        slots: Vec::with_capacity(rows.len()),
        ..SlotAccumulator::default()
    };
    for row in &rows {
        projection.push(row, &acquired_components);
    }
    let mut projection = projection.finish(rows.len());
    projection.receipt = serde_json::json!({
        "total":total,"returned":rows.len(),"truncated":truncated,"nextCursor":next_cursor,
        "limit":DETAIL_MEDIA_SLOT_LIMIT
    });
    Ok(projection)
}

impl SlotAccumulator {
    fn push(
        &mut self,
        row: &sqlx::postgres::PgRow,
        acquired_components: &std::collections::HashSet<(Uuid, String)>,
    ) {
        let disposition_restricted = row
            .get::<Option<bool>, _>("disposition_restricted")
            .unwrap_or(false);
        let disposition_cleaned = row
            .get::<Option<bool>, _>("disposition_cleaned")
            .unwrap_or(false);
        let newer_disposed = row
            .get::<Option<bool>, _>("newer_disposed")
            .unwrap_or(false);
        let slot_restricted = row
            .get::<Option<bool>, _>("slot_restricted")
            .unwrap_or(false);
        let materialization_ref = row.get::<Option<Uuid>, _>("materialization_ref");
        let blob_sha256 = row.get::<Option<String>, _>("sha256");
        let mut local_asset_url =
            materialization_ref
                .zip(blob_sha256.as_deref())
                .map(|(materialization_ref, sha256)| {
                    format!("/api/local/media/{materialization_ref}/{sha256}")
                });
        let bytes_state = match (
            materialization_ref.is_some(),
            disposition_restricted,
            disposition_cleaned,
        ) {
            (true, _, _) => {
                self.acquired += 1;
                "ACQUIRED"
            }
            (false, true, _) => {
                self.restricted = true;
                local_asset_url = None;
                "WITHDRAWN_OR_RESTRICTED"
            }
            (false, false, true) => {
                self.cleaned = true;
                local_asset_url = None;
                "BYTES_CLEANED"
            }
            _ if row.get::<Option<String>, _>("terminal_reason").is_some() => {
                self.failed += 1;
                "FAILED"
            }
            _ => "NOT_OBSERVED",
        };
        if slot_restricted && !self.limitations.contains(&"SLOT_WITHDRAWN_OR_RESTRICTED") {
            self.limitations.push("SLOT_WITHDRAWN_OR_RESTRICTED");
        } else if newer_disposed && !self.limitations.contains(&"NEWER_MATERIALIZATION_DISPOSED") {
            self.limitations.push("NEWER_MATERIALIZATION_DISPOSED");
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
            let observation_ref = row.get::<Uuid, _>("observation_ref");
            let still_state =
                if acquired_components.contains(&(observation_ref, "still".to_owned())) {
                    "ACQUIRED".to_owned()
                } else {
                    row.get::<Option<String>, _>("live_photo_still_state")
                        .unwrap_or_else(|| "UNKNOWN".to_owned())
                };
            let motion_state =
                if acquired_components.contains(&(observation_ref, "motion".to_owned())) {
                    "ACQUIRED".to_owned()
                } else {
                    row.get::<Option<String>, _>("live_photo_motion_state")
                        .unwrap_or_else(|| "UNKNOWN".to_owned())
                };
            let bundle_state = if still_state == "ACQUIRED" && motion_state == "ACQUIRED" {
                "COMPLETE".to_owned()
            } else {
                row.get::<String, _>("composite_state")
            };
            serde_json::json!({"bundleState":bundle_state,"stillState":still_state,"motionState":motion_state})
        } else {
            Value::Null
        };
        self.slots.push(serde_json::json!({
            "slotKey":row.get::<String,_>("slot_key"),"purpose":purpose,
            "producerOrdinal":row.get::<i32,_>("producer_ordinal"),"displayOrdinal":row.get::<Option<i32>,_>("display_ordinal"),
            "relationshipKind":row.get::<Option<String>,_>("relationship_kind"),"relationshipOrdinal":row.get::<Option<i32>,_>("relationship_ordinal"),
            "displayOrderState":row.get::<String,_>("display_order_state"),"displayOrderBasis":row.get::<String,_>("display_order_basis"),
            "origin":{"observationRef":row.get::<Uuid,_>("observation_ref"),"sourceGeneration":row.get::<i32,_>("source_generation"),"observedAt":row.get::<String,_>("observed_at"),"candidateUriCount":candidate_count,"candidateSetState":row.get::<String,_>("candidate_set_state"),"actualDownloadCandidateState":"UNKNOWN"},
            "components":components,"bytesState":bytes_state,"replicaState":if local_asset_url.is_some(){"VERIFIED_AT_MATERIALIZATION"}else{"UNKNOWN"},
            "dispositionState":if materialization_ref.is_some(){"UNKNOWN"}else if disposition_restricted{"WITHDRAWN_OR_RESTRICTED"}else if disposition_cleaned{"BYTES_CLEANED"}else{"UNKNOWN"},"localAssetUrl":local_asset_url,
            "currentDownloadAttemptRef":row.get::<Option<Uuid>,_>("download_attempt_ref"),
            "replica":{"materializationRef":row.get::<Option<Uuid>,_>("materialization_ref"),"originObservationRef":row.get::<Option<Uuid>,_>("materialization_observation_ref"),"isCurrentOrigin":row.get::<Option<Uuid>,_>("materialization_observation_ref")==Some(row.get::<Uuid,_>("observation_ref"))},
            "replicaSelection":{"limitations":if slot_restricted{vec!["SLOT_WITHDRAWN_OR_RESTRICTED"]}else if newer_disposed{vec!["NEWER_MATERIALIZATION_DISPOSED"]}else{Vec::<&str>::new()},"excludedDispositionStates":{"restricted":disposition_restricted,"cleaned":disposition_cleaned}},
            "blob":media_blob_contract(
                row.get::<Option<String>,_>("sha256"),
                row.get::<Option<String>,_>("mime_type"),
                row.get::<Option<i64>,_>("byte_size"),
                row.get::<Option<i32>,_>("pixel_width"),
                row.get::<Option<i32>,_>("pixel_height"),
                row.get::<Option<i64>,_>("duration_ms")
            )
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
            limitations: self.limitations,
            receipt: Value::Null,
        }
    }
}

fn media_blob_contract(
    sha256: Option<String>,
    declared_mime_type: Option<String>,
    byte_size: Option<i64>,
    pixel_width: Option<i32>,
    pixel_height: Option<i32>,
    duration_ms: Option<i64>,
) -> Value {
    let inline_safe = declared_mime_type
        .as_deref()
        .and_then(crate::material_storage_key::safe_inline_mime)
        .is_some();
    serde_json::json!({
        "sha256":sha256,"declaredMimeType":declared_mime_type,"detectedMimeType":Value::Null,
        "deliveryMimeType":if inline_safe { declared_mime_type } else { Some("application/octet-stream".to_owned()) },
        "deliveryState":if inline_safe { "INLINE_SAFE" } else { "UNSUPPORTED_MEDIA_TYPE" },
        "byteSize":byte_size,
        "intrinsicDimensions":{
            "state":if pixel_width.is_some() && pixel_height.is_some(){"KNOWN"}else{"UNKNOWN"},
            "width":pixel_width,
            "height":pixel_height
        },
        "durationMs":duration_ms
    })
}

async fn read_derivatives(
    tx: &mut Transaction<'_, Postgres>,
    content_ref: Uuid,
    as_of: &str,
) -> Result<(Vec<Value>, Value, &'static str, &'static str, bool), sqlx::Error> {
    let mut derivative_rows = sqlx::query(
        "SELECT job.job_ref,job.slot_key,job.processor_kind,job.processor_version,job.input_scope, \
             event.state,event.reason,derivative.derivative_ref,derivative.derivative_kind,derivative.storage_key, \
             derived.display_text,derived.language_state,derived.language_tag, \
             disposition.restricted AS disposition_restricted,count(*) OVER() AS total_count \
         FROM linggan_media_processing_job job JOIN linggan_media_slot slot USING (slot_key) \
         LEFT JOIN LATERAL (SELECT state,reason FROM linggan_media_processing_job_event event WHERE event.job_ref=job.job_ref AND occurred_at <= $2::timestamptz ORDER BY occurred_at DESC LIMIT 1) event ON true \
         LEFT JOIN linggan_media_derivative derivative ON derivative.job_ref=job.job_ref AND derivative.created_at <= $2::timestamptz \
         LEFT JOIN linggan_material_derived_text derived ON derived.derivative_ref=derivative.derivative_ref AND derived.created_at <= $2::timestamptz \
         LEFT JOIN LATERAL (SELECT bool_or(disposition.state='WITHDRAWN_OR_RESTRICTED') AS restricted \
             FROM linggan_current_material_media_disposition disposition \
             WHERE (disposition.derivative_ref=derivative.derivative_ref OR disposition.blob_sha256=job.blob_sha256 OR disposition.slot_key=job.slot_key)) disposition ON true \
         JOIN linggan_material_content content ON content.platform=slot.platform AND content.content_external_id=slot.content_external_id \
         WHERE content.public_ref=$1 AND job.created_at <= $2::timestamptz ORDER BY job.created_at LIMIT 21",
    ).bind(content_ref).bind(as_of).fetch_all(&mut **tx).await?;
    let total = derivative_rows
        .first()
        .map_or(0_i64, |row| row.get("total_count"));
    let truncated = derivative_rows.len() > DETAIL_DERIVATIVE_LIMIT;
    if truncated {
        derivative_rows.truncate(DETAIL_DERIVATIVE_LIMIT);
    }
    let next_cursor = truncated
        .then(|| {
            derivative_rows
                .last()
                .map(|row| format!("job:{}", row.get::<Uuid, _>("job_ref")))
        })
        .flatten();
    let mut derivatives = Vec::with_capacity(derivative_rows.len());
    let mut ocr_state = "UNKNOWN";
    let mut asr_state = "UNKNOWN";
    let mut any_restricted = false;
    for row in &derivative_rows {
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
            (false, Some("pending"), Some("queued_for_local_processor"), _) => "QUEUED",
            (false, Some("pending"), _, _) => "QUEUED",
            (false, Some("running"), _, _) => "PROCESSING",
            (false, Some("failed"), _, _) => "FAILED",
            (false, Some("succeeded"), _, true) => "ACQUIRED",
            (
                false,
                Some("succeeded"),
                Some("no_text_observed" | "no_speech_observed" | "no_frame_text_observed"),
                false,
            ) => "KNOWN_EMPTY",
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
        derivatives.push(serde_json::json!({"jobRef":row.get::<Uuid,_>("job_ref"),"slotKey":row.get::<Option<String>,_>("slot_key"),"kind":row.get::<Option<String>,_>("derivative_kind"),"state":state,"displayText":if restricted{None}else{row.get::<Option<String>,_>("display_text")},"languageState":row.get::<Option<String>,_>("language_state"),"languageTag":if restricted{None}else{row.get::<Option<String>,_>("language_tag")},"dispositionState":if restricted{"WITHDRAWN_OR_RESTRICTED"}else{"UNKNOWN"},"processorVersion":row.get::<String,_>("processor_version"),"sourceScope":row.get::<String,_>("input_scope"),"sourceLocation":source_location,"reason":reason}));
    }
    Ok((
        derivatives,
        serde_json::json!({
            "total":total,"returned":derivative_rows.len(),"truncated":truncated,
            "nextCursor":next_cursor,"limit":DETAIL_DERIVATIVE_LIMIT
        }),
        ocr_state,
        asr_state,
        any_restricted,
    ))
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
