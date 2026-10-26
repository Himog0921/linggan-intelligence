//! Bounded preparation of verified legacy motion text. No recognizer or job is dispatched.
use crate::creator_discovery::hash;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoricalSource {
    slot_key: String,
    kind: String,
    materialization_ref: Uuid,
    blob_sha256: String,
    job_ref: Option<Uuid>,
    derivative_ref: Option<Uuid>,
    processor_version: Option<String>,
    content_hash: Option<String>,
    text_hash: String,
    display_hash: String,
    source_location: Value,
    event_ref: Option<Uuid>,
    reason: Option<String>,
    #[serde(skip)]
    text: String,
}
impl HistoricalSource {
    fn known_empty(&self) -> bool {
        self.text.trim().is_empty()
            && matches!(
                self.reason.as_deref(),
                Some("no_audio_stream_observed" | "no_speech_observed")
            )
    }
    fn usable(&self) -> bool {
        !self.text.trim().is_empty()
            && self.derivative_ref.is_some()
            && self.content_hash.as_deref() == Some(self.text_hash.as_str())
    }
}

pub struct HistoricalInventory {
    sources: Vec<HistoricalSource>,
}
impl HistoricalInventory {
    pub fn fingerprint(&self) -> String {
        hash(&json!({"rule":"historical-media.v1","sources":self.sources}).to_string())
    }
    pub fn summary(&self) -> Value {
        json!({"units":self.sources.len(),"eligible":self.sources.iter().filter(|s|s.usable()).count(),
            "knownEmpty":self.sources.iter().filter(|s|s.known_empty()).count(),
            "unavailable":self.sources.iter().filter(|s|!s.usable()&&!s.known_empty()).count()})
    }
}

pub async fn inventory_in(
    conn: &mut PgConnection,
    work: Uuid,
    as_of: &str,
) -> Result<HistoricalInventory, sqlx::Error> {
    let rows = sqlx::query(r#"
WITH origins AS (
 SELECT DISTINCT ON(o.slot_key) o.* FROM linggan_material_media_origin o
 JOIN linggan_runtime_capture_package p USING(package_ref)
 JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=o.package_ref AND disposition.record_ordinal=o.record_ordinal
 WHERE o.content_public_ref=$1 AND o.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
 AND disposition.disposition='accepted_for_media_identity' AND o.purpose IN ('video','live_photo')
 ORDER BY o.slot_key,o.source_generation DESC
), motion AS (
 SELECT o.*,m.materialization_ref,m.blob_sha256,b.mime_type FROM origins o
 JOIN LATERAL(SELECT m.* FROM linggan_media_download_attempt a JOIN linggan_media_materialization m USING(download_attempt_ref)
 JOIN linggan_media_blob b ON b.sha256=m.blob_sha256 WHERE a.media_observation_ref=o.observation_ref
 AND m.verified_at<=$2::timestamptz AND (b.mime_type LIKE 'video/%' OR (o.purpose<>'live_photo' AND b.mime_type LIKE 'audio/%'))
 ORDER BY m.verified_at DESC,m.materialization_ref DESC LIMIT 1) m ON true
 JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
 WHERE linggan_dossier_slot_active($1,o.slot_key,$2::timestamptz)
 AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition x WHERE x.state='WITHDRAWN_OR_RESTRICTED'
 AND (x.slot_key=o.slot_key OR x.blob_sha256=m.blob_sha256 OR x.materialization_ref=m.materialization_ref))
)
SELECT m.slot_key,m.materialization_ref,m.blob_sha256,k.kind,result.* FROM motion m
CROSS JOIN (VALUES ('audio','asr','asr_text')) k(kind,processor,derivative_kind)
LEFT JOIN LATERAL (
 SELECT j.job_ref,j.processor_version,d.derivative_ref,d.content_hash,t.text_content,t.display_text,t.source_location,e.event_ref,e.reason
 FROM linggan_media_processing_job j JOIN linggan_media_processing_work w USING(job_ref)
 JOIN LATERAL(SELECT e.* FROM linggan_media_processing_job_event e WHERE e.job_ref=j.job_ref AND e.occurred_at<=$2::timestamptz ORDER BY e.occurred_at DESC,e.event_ref DESC LIMIT 1) e ON true
 LEFT JOIN linggan_media_derivative d ON d.job_ref=j.job_ref AND d.derivative_kind=k.derivative_kind AND d.created_at<=$2::timestamptz
 LEFT JOIN linggan_material_derived_text t ON t.derivative_ref=d.derivative_ref AND t.content_public_ref=$1 AND t.kind=k.derivative_kind AND t.created_at<=$2::timestamptz AND t.observed_at<=$2::timestamptz
 WHERE j.slot_key=m.slot_key AND j.blob_sha256=m.blob_sha256 AND j.processor_kind=k.processor
 AND j.created_at<=$2::timestamptz AND w.reading_source_version IS NULL AND e.state='succeeded'
 AND NOT EXISTS(SELECT 1 FROM linggan_media_ocr_retirement r WHERE r.retired_job_ref=j.job_ref)
 AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition x WHERE x.state='WITHDRAWN_OR_RESTRICTED' AND x.derivative_ref=d.derivative_ref)
 AND (t.derivative_ref IS NOT NULL OR e.reason IN ('no_audio_stream_observed','no_speech_observed','no_frame_text_observed'))
 ORDER BY j.created_at DESC,j.job_ref DESC,d.created_at DESC,d.derivative_ref DESC LIMIT 1
) result ON true
WHERE k.kind='audio' OR m.mime_type LIKE 'video/%'
ORDER BY m.slot_key,k.kind
"#).bind(work).bind(as_of).fetch_all(&mut *conn).await?;
    let sources = rows
        .into_iter()
        .map(|row| {
            let text = row
                .get::<Option<String>, _>("text_content")
                .unwrap_or_default();
            let display = row
                .get::<Option<String>, _>("display_text")
                .unwrap_or_default();
            HistoricalSource {
                slot_key: row.get("slot_key"),
                kind: row.get("kind"),
                materialization_ref: row.get("materialization_ref"),
                blob_sha256: row.get("blob_sha256"),
                job_ref: row.get("job_ref"),
                derivative_ref: row.get("derivative_ref"),
                processor_version: row.get("processor_version"),
                content_hash: row.get("content_hash"),
                text_hash: hash(&text),
                display_hash: hash(&display),
                source_location: row
                    .get::<Option<Value>, _>("source_location")
                    .unwrap_or(Value::Null),
                event_ref: row.get("event_ref"),
                reason: row.get("reason"),
                text,
            }
        })
        .collect();
    Ok(HistoricalInventory { sources })
}

pub async fn apply_in(
    conn: &mut PgConnection,
    work: Uuid,
    as_of: &str,
    inventory: &HistoricalInventory,
) -> Result<Value, sqlx::Error> {
    let current = crate::material_reading::read_current_in(conn, &[work], as_of)
        .await?
        .remove(&work)
        .unwrap_or_default();
    let mut results = vec![];
    for source in &inventory.sources {
        let compatible = current.iter().find(|m| {
            m["kind"] == source.kind
                && m["slotKey"] == source.slot_key
                && m["currentRevisionRef"].is_string()
        });
        if let Some(reading) = compatible {
            results.push(json!({"slotKey":source.slot_key,"kind":source.kind,"state":"already_current","readingRef":reading["currentRevisionRef"],"humanProtected":reading["reading"]["adoption"]!="automatic"}));
        } else if source.known_empty() {
            let published = crate::material_reading::producers::publish_historical_in(
                conn,
                work,
                source.job_ref.expect("empty result job"),
                source.materialization_ref,
                None,
            )
            .await?;
            results.push(json!({"slotKey":source.slot_key,"kind":source.kind,"state":"known_empty","reason":source.reason,"updated":published.current,"readingRef":published.reading_ref}));
        } else if source.usable() {
            let published = crate::material_reading::producers::publish_historical_in(
                conn,
                work,
                source.job_ref.expect("eligible job"),
                source.materialization_ref,
                source.derivative_ref,
            )
            .await?;
            results.push(json!({"slotKey":source.slot_key,"kind":source.kind,"state":if published.current{"updated"}else{"skipped"},"readingRef":published.reading_ref}));
        } else {
            results.push(json!({"slotKey":source.slot_key,"kind":source.kind,"state":"unavailable","reason":if source.derivative_ref.is_some(){"historical_text_hash_or_content_invalid"}else{"no_eligible_historical_text"}}));
        }
    }
    Ok(
        json!({"state":if inventory.sources.is_empty(){"source_not_available"}else{"processed"},"sourceUnits":inventory.sources.len(),"updated":results.iter().filter(|r|r["state"]=="updated"||r["updated"]==true).count(),"alreadyCurrent":results.iter().filter(|r|r["state"]=="already_current").count(),
        "knownEmpty":results.iter().filter(|r|r["state"]=="known_empty").count(),"unavailable":results.iter().filter(|r|r["state"]=="unavailable").count(),
        "skipped":results.iter().filter(|r|r["state"]=="skipped").count(),"results":results}),
    )
}

pub async fn all_current_in(
    conn: &mut PgConnection,
    work: Uuid,
    as_of: &str,
    inventory: &HistoricalInventory,
) -> Result<bool, sqlx::Error> {
    let current = crate::material_reading::read_current_in(conn, &[work], as_of)
        .await?
        .remove(&work)
        .unwrap_or_default();
    Ok(inventory
        .sources
        .iter()
        .filter(|s| s.usable() || s.known_empty())
        .all(|s| {
            current.iter().any(|m| {
                m["kind"] == s.kind
                    && m["slotKey"] == s.slot_key
                    && m["currentRevisionRef"].is_string()
            })
        }))
}
