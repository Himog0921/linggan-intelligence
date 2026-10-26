//! Adapters publish the complete source transcription to the canonical reading owner.
//! Processor output and human review use the same content and source identity contract.
use super::*;
use crate::MediaProcessingClaim;

pub struct ReadingContent {
    pub blocks: Vec<Value>,
    pub issues: Vec<Value>,
    pub source_state: String,
    pub source_location: Value,
}

pub async fn publish_job_in(
    conn: &mut PgConnection,
    claim: &MediaProcessingClaim,
    derivative: Option<Uuid>,
    content: ReadingContent,
) -> Result<PublishedReading, sqlx::Error> {
    let row = sqlx::query("SELECT reading_base_ref,reading_source_version FROM linggan_media_processing_work WHERE work_ref=$1 AND job_ref=$2")
        .bind(claim.work_ref).bind(claim.job_ref).fetch_one(&mut *conn).await?;
    let version: Option<String> = row.get("reading_source_version");
    let version = version.ok_or_else(|| invalid("media_reading_source_not_frozen"))?;
    let (kind, producer) = match claim.processor_kind.as_str() {
        "image_ocr"
            if claim.processor_version == crate::ocr_adoption_persistence::PROCESSOR_VERSION =>
        {
            ("image", "local_vision")
        }
        "image_ocr"
            if claim.processor_version
                == crate::processor_version_for_kind("image_ocr").unwrap() =>
        {
            ("image", "ordinary_ocr")
        }
        "asr" => ("audio", "asr"),
        "video_frame_ocr"
            if claim.processor_version
                == crate::processor_version_for_kind("video_frame_ocr").unwrap() =>
        {
            ("video_frame", "frame_ocr")
        }
        _ => return Err(invalid("material_reading_processor_invalid")),
    };
    let source = media_source_in(
        conn,
        claim.content_public_ref,
        claim.job_ref,
        Some(&version),
    )
    .await?;
    let observation_rule = if producer == "local_vision" {
        format!(
            "{}:{}",
            claim.processor_version,
            crate::ocr_adoption::RULE_VERSION
        )
    } else {
        content.source_location["layerVersion"]
            .as_str()
            .unwrap_or(&claim.processor_version)
            .to_owned()
    };
    // This completion entry belongs to daily processing. Explicit cloud previews use
    // publish_cloud_in below; historical candidates are never republished by a read.
    let publication = Publication::Automatic {
        expected_current: row.get("reading_base_ref"),
    };
    publish_source_in(
        conn,
        MediaPublication {
            source,
            work: claim.content_public_ref,
            kind,
            producer,
            producer_ref: claim.job_ref,
            derivative,
            rule: format!(
                "{}:{observation_rule}",
                crate::research_dossier::content::RULE_VERSION
            ),
            content,
            publication,
        },
    )
    .await
}

struct MediaPublication<'a> {
    source: Value,
    work: Uuid,
    kind: &'a str,
    producer: &'a str,
    producer_ref: Uuid,
    derivative: Option<Uuid>,
    rule: String,
    content: ReadingContent,
    publication: Publication,
}

async fn publish_source_in(
    conn: &mut PgConnection,
    mut input: MediaPublication<'_>,
) -> Result<PublishedReading, sqlx::Error> {
    let slot = input.source["slotKey"]
        .as_str()
        .ok_or_else(|| invalid("media_reading_slot_missing"))?
        .to_owned();
    let version = input.source["sourceVersion"]
        .as_str()
        .ok_or_else(|| invalid("media_reading_source_missing"))?
        .to_owned();
    if let Some(location) = input.content.source_location.as_object() {
        for (key, value) in location {
            input.source[key] = value.clone();
        }
    }
    publish_in(
        conn,
        NewReading {
            content_public_ref: input.work,
            unit_key: format!("{}:{slot}", input.kind),
            kind: input.kind.into(),
            slot_key: Some(slot),
            page: input.source["page"]
                .as_i64()
                .and_then(|v| v.try_into().ok()),
            source_version: version,
            source_state: input.content.source_state,
            derivative_ref: input.derivative,
            producer_ref: Some(input.producer_ref),
            generated_by: input.producer.into(),
            rule_version: input.rule,
            blocks: input.content.blocks,
            issues: input.content.issues,
            source_location: input.source,
            publication: input.publication,
        },
    )
    .await
}

/// Explicit legacy text preparation: retain actual provenance without rewriting the old job freeze.
pub async fn publish_historical_in(
    conn: &mut PgConnection,
    work: Uuid,
    job: Uuid,
    materialization: Uuid,
    derivative: Option<Uuid>,
) -> Result<PublishedReading, sqlx::Error> {
    lock_work(conn, work).await?;
    let row = sqlx::query(r#"
SELECT j.processor_kind,j.processor_version,j.blob_sha256,d.content_hash,t.text_content,t.source_location,e.reason
FROM linggan_media_processing_job j JOIN linggan_media_processing_work w USING(job_ref)
JOIN LATERAL(SELECT e.* FROM linggan_media_processing_job_event e WHERE e.job_ref=j.job_ref ORDER BY e.occurred_at DESC,e.event_ref DESC LIMIT 1) e ON true
LEFT JOIN linggan_media_derivative d ON d.job_ref=j.job_ref AND d.derivative_ref=$2
LEFT JOIN linggan_material_derived_text t ON t.derivative_ref=d.derivative_ref AND t.content_public_ref=$3
WHERE j.job_ref=$1 AND w.reading_source_version IS NULL AND e.state='succeeded'
AND j.processor_kind='asr' AND ((d.derivative_kind='asr_text' AND t.kind='asr_text') OR ($2::uuid IS NULL AND e.reason IN ('no_audio_stream_observed','no_speech_observed')))
AND NOT EXISTS(SELECT 1 FROM linggan_media_ocr_retirement WHERE retired_job_ref=j.job_ref)
"#).bind(job).bind(derivative).bind(work).fetch_one(&mut *conn).await?;
    let text: Option<String> = row.get("text_content");
    let text_hash = text.as_deref().map(crate::creator_discovery::hash);
    if derivative.is_some()
        && (text.as_deref().is_none_or(|t| t.trim().is_empty())
            || row.get::<Option<String>, _>("content_hash") != text_hash)
    {
        return Err(invalid("historical_text_hash_or_content_invalid"));
    }
    let (kind, producer) = ("audio", "asr");
    let source = media_source_in(conn, work, job, Some(&materialization.to_string())).await?;
    if !matches!(source["purpose"].as_str(), Some("video" | "live_photo")) {
        return Err(invalid("historical_media_purpose_invalid"));
    }
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *conn)
        .await?;
    let slot = source["slotKey"]
        .as_str()
        .ok_or_else(|| invalid("media_reading_slot_missing"))?;
    if !source::current(
        conn,
        work,
        kind,
        Some(slot),
        &materialization.to_string(),
        derivative,
        &now,
    )
    .await?
    {
        return Err(invalid("historical_media_source_changed"));
    }
    // The canonical reader recognizes same-byte re-observations and protects their human/current reading.
    let current = read_current_in(conn, &[work], &now)
        .await?
        .remove(&work)
        .unwrap_or_default();
    if let Some(existing) = current
        .iter()
        .find(|m| m["kind"] == kind && m["slotKey"] == slot && m["currentRevisionRef"].is_string())
    {
        return Ok(PublishedReading {
            unit_ref: serde_json::from_value(existing["ref"].clone())
                .map_err(|_| invalid("historical_current_invalid"))?,
            reading_ref: serde_json::from_value(existing["currentRevisionRef"].clone())
                .map_err(|_| invalid("historical_current_invalid"))?,
            current: true,
        });
    }
    let expected_current = sqlx::query_scalar::<_,Option<Uuid>>("SELECT current_reading_ref FROM linggan_material_unit WHERE content_public_ref=$1 AND kind=$2 AND slot_key=$3")
        .bind(work).bind(kind).bind(slot).fetch_optional(&mut *conn).await?.flatten();
    let reason: Option<String> = row.get("reason");
    let mut location = json!({"jobRef":job,"derivativeRef":derivative,"materializationRef":materialization,"blobSha256":row.get::<String,_>("blob_sha256"),"processorVersion":row.get::<String,_>("processor_version"),"recognitionOutcome":reason});
    let content = if let Some(text) = text {
        location["fullTextHash"] = json!(text_hash);
        location["timeUnknown"] = json!(true);
        location["legacySourceLocation"] = row
            .get::<Option<Value>, _>("source_location")
            .unwrap_or(Value::Null);
        ReadingContent {
            blocks: vec![
                json!({"ref":"legacy-text","kind":"paragraph","text":text,"speaker":"unconfirmed"}),
            ],
            issues: vec![
                json!({"key":"historical_time_unknown","code":"time_unknown","message":"历史结果无时间定位。","resolution":"source_refresh"}),
                json!({"key":"historical_machine_unchecked","code":"machine_unchecked","message":"机器识读结果，尚未人工核验。","resolution":"reading_edit"}),
            ],
            source_state: "present".into(),
            source_location: location,
        }
    } else {
        ReadingContent {
            blocks: vec![],
            issues: if kind == "video_frame" {
                vec![
                    json!({"key":"historical_video_sampling_incomplete","code":"sampling_incomplete","message":"历史采样帧未检出文字，不代表整段视频无文字。","resolution":"source_refresh"}),
                ]
            } else {
                vec![]
            },
            source_state: if reason.as_deref() == Some("no_audio_stream_observed") {
                "not_applicable"
            } else {
                "known_empty"
            }
            .into(),
            source_location: location,
        }
    };
    publish_source_in(
        conn,
        MediaPublication {
            source,
            work,
            kind,
            producer,
            producer_ref: job,
            derivative,
            rule: format!(
                "{}:historical-media.v1",
                crate::research_dossier::content::RULE_VERSION
            ),
            content,
            publication: Publication::Automatic { expected_current },
        },
    )
    .await
}

pub async fn publish_cloud_in(
    conn: &mut PgConnection,
    work: Uuid,
    job: Uuid,
    materialization: Uuid,
    derivative: Uuid,
    blocks: Vec<Value>,
    expected_current: Option<Uuid>,
) -> Result<PublishedReading, sqlx::Error> {
    let source = media_source_in(conn, work, job, Some(&materialization.to_string())).await?;
    let content = ReadingContent {
        blocks,
        issues: vec![],
        source_state: "present".into(),
        source_location: json!({}),
    };
    publish_source_in(
        conn,
        MediaPublication {
            source,
            work,
            kind: "image",
            producer: "cloud_vision",
            producer_ref: job,
            derivative: Some(derivative),
            rule: format!(
                "{}:{}",
                crate::research_dossier::content::RULE_VERSION,
                crate::cloud_ocr_adoption::RULE_VERSION
            ),
            content,
            publication: Publication::Automatic { expected_current },
        },
    )
    .await
}

async fn media_source_in(
    conn: &mut PgConnection,
    work: Uuid,
    job: Uuid,
    frozen: Option<&str>,
) -> Result<Value, sqlx::Error> {
    let row = sqlx::query("SELECT j.slot_key,j.blob_sha256,o.observation_ref,o.display_ordinal,o.display_order_state,o.purpose,m.materialization_ref,m.local_asset_path \
        FROM linggan_media_processing_job j JOIN linggan_material_media_origin o ON o.slot_key=j.slot_key AND o.content_public_ref=$1 \
        JOIN linggan_runtime_capture_package p ON p.package_ref=o.package_ref \
        JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=o.package_ref AND disposition.record_ordinal=o.record_ordinal \
        JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m USING(download_attempt_ref) \
        JOIN linggan_media_blob b ON b.sha256=m.blob_sha256 \
        WHERE j.job_ref=$2 AND ($3::text IS NOT NULL OR linggan_dossier_slot_active($1,j.slot_key,scope_001_now())) AND m.blob_sha256=j.blob_sha256 AND p.accepted_at<=scope_001_now() AND o.created_at<=scope_001_now() \
        AND disposition.disposition='accepted_for_media_identity' AND ((j.processor_kind='image_ocr' AND b.mime_type LIKE 'image/%') \
        OR (j.processor_kind='video_frame_ocr' AND b.mime_type LIKE 'video/%') \
        OR (j.processor_kind='asr' AND (b.mime_type LIKE 'video/%' OR (o.purpose<>'live_photo' AND b.mime_type LIKE 'audio/%')))) AND (($3::text IS NOT NULL AND m.materialization_ref::text=$3) OR ($3::text IS NULL \
        AND o.source_generation=(SELECT max(o2.source_generation) FROM linggan_material_media_origin o2 JOIN linggan_runtime_capture_package p2 USING(package_ref) JOIN linggan_runtime_record_disposition d2 ON d2.package_ref=o2.package_ref AND d2.record_ordinal=o2.record_ordinal WHERE o2.slot_key=j.slot_key AND o2.content_public_ref=$1 AND o2.created_at<=scope_001_now() AND p2.accepted_at<=scope_001_now() AND d2.disposition='accepted_for_media_identity' ))) \
        ORDER BY o.source_generation DESC,m.verified_at DESC,m.materialization_ref DESC LIMIT 1")
        .bind(work).bind(job).bind(frozen).fetch_optional(conn).await?.ok_or_else(|| invalid("media_reading_source_unavailable"))?;
    Ok(
        json!({"slotKey":row.get::<String,_>("slot_key"),"blobSha256":row.get::<String,_>("blob_sha256"),"observationRef":row.get::<Uuid,_>("observation_ref"),
        "sourceVersion":row.get::<Uuid,_>("materialization_ref"),"materializationRef":row.get::<Uuid,_>("materialization_ref"),"originalUrl":format!("/api/local/media/{}/{}",row.get::<Uuid,_>("materialization_ref"),row.get::<String,_>("blob_sha256")),
        "page":if row.get::<String,_>("display_order_state")=="KNOWN"{json!(row.get::<Option<i32>,_>("display_ordinal"))}else{Value::Null},"purpose":row.get::<String,_>("purpose")}),
    )
}
