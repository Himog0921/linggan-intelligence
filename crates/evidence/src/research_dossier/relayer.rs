//! Bounded, explicit re-layering of existing OCR coordinates. Never enqueues OCR or collection.
use crate::{OcrLineInput, ocr_layering};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub async fn relayer_work(
    database: &Database,
    work: Uuid,
    expected_revision: &str,
) -> Result<Value, crate::ProducerRuntimeError> {
    let mut tx = database
        .pool()
        .begin()
        .await
        .map_err(crate::ProducerRuntimeError::Internal)?;
    // Serialize explicit operations on this work; retries converge on (layout, rule version).
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,420))")
        .bind(work.to_string())
        .execute(&mut *tx)
        .await
        .map_err(crate::ProducerRuntimeError::Internal)?;
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::ProducerRuntimeError::Internal)?;
    let dossier = super::read_in(&mut tx, &[work], &now)
        .await
        .map_err(crate::ProducerRuntimeError::Internal)?
        .remove(&work)
        .ok_or(crate::ProducerRuntimeError::MaterialIdentityConflict)?;
    if dossier.revision != expected_revision {
        return Err(crate::ProducerRuntimeError::MaterialIdentityConflict);
    }
    let plan = super::relayer_plan(&dossier);
    let layouts: Vec<_> = dossier
        .media
        .iter()
        .filter(|m| m["state"] == "ACQUIRED" && m["contextOnly"] != true)
        .filter_map(|m| {
            m["ocrLayering"]["layoutRef"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if layouts.len() > 100 {
        return Err(crate::ProducerRuntimeError::MaterialIdentityConflict);
    }
    let mut results = vec![];
    for layout in &layouts {
        let rows=sqlx::query("SELECT line_ref,ordinal,text_content,confidence::float8 AS confidence,left_norm::float8 AS l,top_norm::float8 AS t,right_norm::float8 AS r,bottom_norm::float8 AS b FROM linggan_media_ocr_line WHERE layout_ref=$1 ORDER BY ordinal").bind(layout).fetch_all(&mut *tx).await.map_err(crate::ProducerRuntimeError::Internal)?;
        if rows.is_empty()
            || rows
                .iter()
                .enumerate()
                .any(|(i, r)| r.get::<i32, _>("ordinal") != i as i32)
        {
            results.push(
                json!({"layoutRef":layout,"state":"skipped","reason":"layout_lines_unavailable"}),
            );
            continue;
        }
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
        let inserted =
            crate::material_processing::insert_ocr_layering(&mut tx, *layout, &refs, &input)
                .await?;
        results.push(json!({"layoutRef":layout,"state":if inserted==1{"updated"}else{"already_current"},"layeringState":input.state}));
    }
    tx.commit()
        .await
        .map_err(crate::ProducerRuntimeError::Internal)?;
    Ok(
        json!({"workRef":work,"layerVersion":ocr_layering::VERSION,"needsOcr":plan["needsOcr"],"scopeCount":layouts.len(),
        "updated":results.iter().filter(|r|r["state"]=="updated").count(),
        "alreadyCurrent":results.iter().filter(|r|r["state"]=="already_current").count(),
        "skipped":results.iter().filter(|r|r["state"]=="skipped").count(),"results":results}),
    )
}
