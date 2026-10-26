//! Explicit normalization of every eligible existing layout; this never dispatches recognition.
use crate::{OcrLineInput, ProducerRuntimeError, ocr_layering};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(super) fn rule_version() -> String {
    format!("{}:{}", super::content::RULE_VERSION, ocr_layering::VERSION)
}

pub(super) async fn source_layouts_in(
    conn: &mut PgConnection,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Vec<Value>>, sqlx::Error> {
    let rows=sqlx::query(r#"
WITH origins AS (
 SELECT DISTINCT ON(o.slot_key) o.* FROM linggan_material_media_origin o
 JOIN linggan_runtime_capture_package p USING(package_ref)
 WHERE o.content_public_ref=ANY($1) AND o.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz
 ORDER BY o.slot_key,o.source_generation DESC
)
SELECT DISTINCT ON(o.content_public_ref,o.slot_key) o.content_public_ref,o.slot_key,m.materialization_ref,
 l.layout_ref,l.created_at,s.reading_ref,s.adoption,r.rule_version
FROM origins o JOIN LATERAL (
 SELECT m.* FROM linggan_media_materialization m JOIN linggan_media_download_attempt a USING(download_attempt_ref)
 JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
 WHERE a.media_observation_ref=o.observation_ref AND m.verified_at<=$2::timestamptz
 AND b.mime_type LIKE 'image/%'
 ORDER BY m.verified_at DESC,m.materialization_ref DESC LIMIT 1
) m ON true
JOIN linggan_media_processing_job j ON j.slot_key=o.slot_key AND j.blob_sha256=m.blob_sha256
JOIN linggan_media_derivative d ON d.job_ref=j.job_ref
JOIN linggan_media_ocr_layout l ON l.ocr_derivative_ref=d.derivative_ref AND l.engine='paddleocr'
LEFT JOIN linggan_material_unit u ON u.content_public_ref=o.content_public_ref AND u.slot_key=o.slot_key AND u.kind='image'
LEFT JOIN LATERAL (
 SELECT s.* FROM linggan_material_reading_selection s WHERE s.unit_ref=u.unit_ref AND s.created_at<=$2::timestamptz
 ORDER BY s.revision DESC LIMIT 1
) s ON s.source_version=m.materialization_ref::text
LEFT JOIN linggan_material_reading r ON r.reading_ref=s.reading_ref
WHERE j.processor_kind='image_ocr' AND j.processor_version<>$3
 AND linggan_dossier_slot_active(o.content_public_ref,o.slot_key,$2::timestamptz)
 AND j.created_at<=$2::timestamptz AND d.created_at<=$2::timestamptz AND l.created_at<=$2::timestamptz
 AND NOT EXISTS(SELECT 1 FROM linggan_media_ocr_retirement x WHERE x.retired_job_ref=j.job_ref)
 AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition x WHERE x.state='WITHDRAWN_OR_RESTRICTED'
 AND (x.slot_key=o.slot_key OR x.blob_sha256=m.blob_sha256 OR x.materialization_ref=m.materialization_ref OR x.derivative_ref=d.derivative_ref))
 ORDER BY o.content_public_ref,o.slot_key,l.created_at DESC,l.layout_ref DESC
"#).bind(works).bind(as_of).bind(crate::ocr_adoption_persistence::PROCESSOR_VERSION).fetch_all(conn).await?;
    let mut result = BTreeMap::<Uuid, Vec<Value>>::new();
    for row in rows {
        result.entry(row.get("content_public_ref")).or_default().push(json!({"slotKey":row.get::<String,_>("slot_key"),
            "sourceVersion":row.get::<Uuid,_>("materialization_ref"),"layoutRef":row.get::<Uuid,_>("layout_ref"),
            "currentRevisionRef":row.get::<Option<Uuid>,_>("reading_ref"),"ruleVersion":row.get::<Option<String>,_>("rule_version"),
            "humanProtected":row.get::<Option<String>,_>("adoption").is_some_and(|s|s!="automatic")}));
    }
    Ok(result)
}

pub async fn relayer_work(
    database: &Database,
    work: Uuid,
    expected_revision: &str,
) -> Result<Value, ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    sqlx::query("SELECT pg_advisory_xact_lock(420,hashtext($1::uuid::text))")
        .bind(work)
        .execute(&mut *tx)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let now = super::cutoff_in(&mut tx, None)
        .await
        .map_err(ProducerRuntimeError::Internal)?;
    let dossier = super::read_in(&mut tx, &[work], &now)
        .await
        .map_err(ProducerRuntimeError::Internal)?
        .remove(&work)
        .ok_or(ProducerRuntimeError::MaterialIdentityConflict)?;
    if dossier.revision != expected_revision {
        return Err(ProducerRuntimeError::MaterialIdentityConflict);
    }
    let plan = super::relayer_plan(&dossier);
    let mut results = vec![];
    for item in &dossier.media {
        let Some(layout) = item["layoutRef"]
            .as_str()
            .and_then(|v| v.parse::<Uuid>().ok())
        else {
            continue;
        };
        if item["humanProtected"] == true {
            results.push(
                json!({"layoutRef":layout,"state":"skipped","reason":"human_current_protected"}),
            );
            continue;
        }
        if item["ruleVersion"] == rule_version() {
            results.push(json!({"layoutRef":layout,"state":"already_current"}));
            continue;
        }
        let rows=sqlx::query("SELECT line_ref,text_content,confidence::float8 AS confidence,left_norm::float8 AS l,top_norm::float8 AS t,right_norm::float8 AS r,bottom_norm::float8 AS b FROM linggan_media_ocr_line WHERE layout_ref=$1 ORDER BY ordinal")
            .bind(layout).fetch_all(&mut *tx).await.map_err(ProducerRuntimeError::Internal)?;
        let lines: Vec<_> = rows
            .iter()
            .map(|r| OcrLineInput {
                text: r.get("text_content"),
                confidence: r.get("confidence"),
                bbox_norm: [r.get("l"), r.get("t"), r.get("r"), r.get("b")],
            })
            .collect();
        let refs: Vec<Uuid> = rows.iter().map(|r| r.get("line_ref")).collect();
        let input = ocr_layering::layer_lines(&lines);
        crate::material_processing::insert_ocr_layering(&mut tx, layout, &refs, &input).await?;
        let expected = item["currentRevisionRef"]
            .as_str()
            .and_then(|v| v.parse().ok());
        let published =
            crate::material_reading::producers::publish_layout_in(&mut tx, layout, expected)
                .await
                .map_err(ProducerRuntimeError::Internal)?;
        results.push(json!({"layoutRef":layout,"readingRef":published.reading_ref,"state":if published.current{"updated"}else{"skipped"}}));
    }
    tx.commit().await.map_err(ProducerRuntimeError::Internal)?;
    Ok(
        json!({"workRef":work,"layerVersion":ocr_layering::VERSION,"needsOcr":plan["needsOcr"],"needsAcquisition":plan["needsAcquisition"],"scopeCount":results.len(),
        "updated":results.iter().filter(|r|r["state"]=="updated").count(),"alreadyCurrent":results.iter().filter(|r|r["state"]=="already_current").count(),
        "skipped":results.iter().filter(|r|r["state"]=="skipped").count(),"results":results}),
    )
}
