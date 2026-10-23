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
        "image_ocr" => ("image", "ordinary_ocr"),
        "asr" => ("audio", "asr"),
        "video_frame_ocr" => ("video_frame", "frame_ocr"),
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
    let publication = if producer == "local_vision" {
        Publication::Candidate
    } else {
        Publication::Automatic {
            expected_current: row.get("reading_base_ref"),
        }
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

pub async fn publish_layout_in(
    conn: &mut PgConnection,
    layout_ref: Uuid,
    expected_current: Option<Uuid>,
) -> Result<PublishedReading, sqlx::Error> {
    let row = sqlx::query("SELECT l.content_public_ref,l.ocr_derivative_ref,l.image_width,l.image_height,d.job_ref FROM linggan_media_ocr_layout l JOIN linggan_media_derivative d ON d.derivative_ref=l.ocr_derivative_ref WHERE layout_ref=$1 AND engine='paddleocr'")
        .bind(layout_ref).fetch_one(&mut *conn).await?;
    let work: Uuid = row.get("content_public_ref");
    let job: Uuid = row.get("job_ref");
    let source = media_source_in(conn, work, job, None).await?;
    let rows = sqlx::query("SELECT line_ref,ordinal,text_content AS text,confidence::float8 AS confidence,left_norm::float8 AS x1,top_norm::float8 AS y1,right_norm::float8 AS x2,bottom_norm::float8 AS y2 FROM linggan_media_ocr_line WHERE layout_ref=$1 ORDER BY ordinal")
        .bind(layout_ref).fetch_all(&mut *conn).await?;
    let lines: Vec<Value> = rows.iter().map(|r| json!({"ref":r.get::<Uuid,_>("line_ref"),"text":r.get::<String,_>("text"),"confidence":r.get::<f64,_>("confidence"),"bbox":[r.get::<f64,_>("x1"),r.get::<f64,_>("y1"),r.get::<f64,_>("x2")-r.get::<f64,_>("x1"),r.get::<f64,_>("y2")-r.get::<f64,_>("y1")]})).collect();
    let layering: Option<Value> = sqlx::query_scalar("SELECT retained_line_refs FROM linggan_media_ocr_layering_result WHERE layout_ref=$1 AND layer_version=$2 ORDER BY created_at DESC LIMIT 1")
        .bind(layout_ref).bind(crate::ocr_layering::VERSION).fetch_optional(&mut *conn).await?;
    let mut lines = lines;
    for line in &mut lines {
        let heading = layering
            .as_ref()
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|v| v["lineRef"] == line["ref"] && v["classification"] == "primary_copy")
            });
        line["kind"] = json!(if heading { "heading" } else { "paragraph" });
        line["speaker"] = json!("unconfirmed");
    }
    let content = ReadingContent {
        blocks: crate::research_dossier::content::ocr_blocks(&lines),
        issues: vec![],
        source_state: "present".into(),
        source_location: json!({"layoutRef":layout_ref,"imageWidth":row.get::<i32,_>("image_width"),"imageHeight":row.get::<i32,_>("image_height")}),
    };
    publish_source_in(
        conn,
        MediaPublication {
            source,
            work,
            kind: "image",
            producer: "ordinary_ocr",
            producer_ref: job,
            derivative: Some(row.get("ocr_derivative_ref")),
            rule: format!(
                "{}:{}",
                crate::research_dossier::content::RULE_VERSION,
                crate::ocr_layering::VERSION
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
            publication: Publication::Candidate,
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
