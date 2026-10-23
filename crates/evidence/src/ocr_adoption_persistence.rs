//! Full local OCR results on the existing job/lease/derivative/layout chain.
//! No model call, scheduling, automatic backfill, or activation occurs in this module.
use crate::material_processing::{claim_is_live, finish_processing_claim, insert_ocr_lines};
use crate::ocr_adoption::{self as rule, *};
use crate::producer_runtime::ProducerRuntimeError as Error;
use crate::{MediaProcessingClaim, OcrLineInput};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

mod validation;

pub const PROCESSOR_VERSION: &str = "local-vl-v1";

pub fn is_safe_input_key(key: &str) -> bool {
    crate::material_storage_key::is_safe_storage_key(key)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bridge {
    schema_version: u8,
    ok: bool,
    observation: ImageInput,
    anchor_raw_json: String,
    vl_raw_json: String,
}

pub struct PreparedOcrAdoption {
    pub observation: ImageInput,
    pub assessment: Assessment,
    pub raw_hash: String,
    pub raw_size: i64,
}

pub fn prepare(raw: &[u8]) -> Result<PreparedOcrAdoption, InputError> {
    if raw.is_empty() || raw.len() > 8_000_000 {
        return Err(InputError::Bounds);
    }
    let mut b: Bridge = serde_json::from_slice(raw).map_err(|_| InputError::Encoding)?;
    if b.schema_version != 2
        || !b.ok
        || !b.observation.layout.require_termination
        || rule::content_hash(b.anchor_raw_json.as_bytes())
            != b.observation.anchors.pipeline.raw_sha256
        || rule::content_hash(b.vl_raw_json.as_bytes()) != b.observation.layout.pipeline.raw_sha256
    {
        return Err(InputError::Source);
    }
    validation::validate_raw(&mut b)?;
    let assessment = rule::assess(&b.observation)?;
    Ok(PreparedOcrAdoption {
        observation: b.observation,
        assessment,
        raw_hash: rule::content_hash(raw),
        raw_size: raw.len() as i64,
    })
}

/// Metadata from the admitted job's source, not from a model or a caller's filename.
pub async fn source_metadata(db: &Database, claim: &MediaProcessingClaim) -> Result<Value, Error> {
    let row=sqlx::query("SELECT job.slot_key,origin.display_ordinal,origin.display_order_state,origin.purpose='cover' AS cover \
        FROM linggan_media_processing_job job JOIN linggan_media_processing_work work USING(job_ref) \
        JOIN linggan_media_materialization m ON m.materialization_ref::text=work.reading_source_version AND m.blob_sha256=job.blob_sha256 \
        JOIN linggan_media_download_attempt attempt USING(download_attempt_ref) \
        JOIN linggan_material_media_origin origin ON origin.observation_ref=attempt.media_observation_ref AND origin.slot_key=job.slot_key \
        JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=origin.package_ref AND disposition.record_ordinal=origin.record_ordinal \
        WHERE job.job_ref=$1 AND job.blob_sha256=$2 AND job.processor_kind='image_ocr' \
          AND job.processor_version=$3 AND origin.content_public_ref=$4 AND disposition.disposition='accepted_for_media_identity' \
          AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d WHERE d.slot_key=job.slot_key OR d.blob_sha256=job.blob_sha256 OR d.materialization_ref=m.materialization_ref)")
        .bind(claim.job_ref).bind(&claim.blob_sha256).bind(PROCESSOR_VERSION).bind(claim.content_public_ref)
        .fetch_optional(db.pool()).await.map_err(Error::Internal)?.ok_or(Error::MaterialIdentityConflict)?;
    let ordinal: Option<i32> = if row.get::<String, _>("display_order_state") == "KNOWN" {
        row.get("display_ordinal")
    } else {
        None
    };
    let cover: bool = row.get("cover");
    let role = if cover {
        ImageRole::ExplicitCover
    } else {
        match ordinal {
            Some(1) => ImageRole::FirstImage,
            Some(p) if p > 1 => ImageRole::Interior,
            _ => ImageRole::Unknown,
        }
    };
    Ok(
        json!({"image":{"workRef":claim.content_public_ref,"imageRef":row.get::<String,_>("slot_key"),"blobSha256":claim.blob_sha256},
        "imageRole":role,"sequenceOrdinal":ordinal.filter(|o|*o>0)}),
    )
}

pub async fn complete(
    db: &Database,
    claim: &MediaProcessingClaim,
    worker: Uuid,
    raw: &[u8],
    storage_key: &str,
) -> Result<Uuid, Error> {
    if claim.processor_kind != "image_ocr"
        || claim.processor_version != PROCESSOR_VERSION
        || !crate::material_storage_key::is_safe_storage_key(storage_key)
    {
        return Err(Error::MaterialIdentityConflict);
    }
    let prepared = prepare(raw).map_err(|_| Error::MaterialIdentityConflict)?;
    let meta = source_metadata(db, claim).await?;
    let image = &prepared.observation.image;
    if image.work_ref != claim.content_public_ref
        || image.blob_sha256 != claim.blob_sha256
        || meta["image"]["imageRef"] != image.image_ref
        || meta["imageRole"] != json!(prepared.observation.image_role)
        || meta["sequenceOrdinal"] != json!(prepared.observation.sequence_ordinal)
    {
        return Err(Error::MaterialIdentityConflict);
    }
    let mut tx = db.pool().begin().await.map_err(Error::Internal)?;
    if !claim_is_live(&mut tx, claim, worker).await? {
        return Err(Error::MaterialIdentityConflict);
    }
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT derivative_ref FROM linggan_media_derivative WHERE job_ref=$1 LIMIT 1",
    )
    .bind(claim.job_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(Error::Internal)?;
    if let Some(id) = existing {
        return Ok(id);
    }
    let derivative = Uuid::new_v4();
    let layout = Uuid::new_v4();
    let adoption = Uuid::new_v4();
    insert_result(&mut tx, claim, &prepared, storage_key, derivative, layout).await?;
    let lines = anchor_lines(&prepared.observation);
    let refs = insert_ocr_lines(&mut tx, layout, &lines).await?;
    insert_adoption(&mut tx, &prepared, derivative, layout, adoption, &refs).await?;
    let blocks = candidate_blocks(&prepared);
    crate::material_reading::producers::publish_job_in(&mut tx, claim, Some(derivative), crate::material_reading::producers::ReadingContent {
        blocks,issues:vec![],source_state:"present".into(),source_location:json!({"layoutRef":layout,"imageWidth":image.width,"imageHeight":image.height})
    }).await.map_err(Error::Internal)?;
    finish_processing_claim(&mut tx, claim, worker).await?;
    tx.commit().await.map_err(Error::Internal)?;
    Ok(derivative)
}

fn anchor_lines(input: &ImageInput) -> Vec<OcrLineInput> {
    let w = f64::from(input.image.width);
    let h = f64::from(input.image.height);
    input
        .anchors
        .lines
        .iter()
        .map(|l| OcrLineInput {
            text: l.text.clone(),
            confidence: l.confidence,
            bbox_norm: [l.bbox[0] / w, l.bbox[1] / h, l.bbox[2] / w, l.bbox[3] / h],
        })
        .collect()
}

async fn insert_result(
    tx: &mut Transaction<'_, Postgres>,
    claim: &MediaProcessingClaim,
    p: &PreparedOcrAdoption,
    key: &str,
    derivative: Uuid,
    layout: Uuid,
) -> Result<(), Error> {
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,$4,$5)")
        .bind(derivative).bind(claim.job_ref).bind(&p.raw_hash).bind(p.raw_size).bind(key).execute(&mut **tx).await.map_err(Error::Internal)?;
    sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr',$5,$6,$7,$8,$9,$10)")
        .bind(layout).bind(derivative).bind(claim.content_public_ref).bind(&claim.blob_sha256).bind("PaddleOCR-VL-1.6;PP-OCRv4-anchor;receipt-v1")
        .bind(p.observation.image.width as i32).bind(p.observation.image.height as i32).bind(&p.raw_hash).bind(p.raw_size).bind(key)
        .execute(&mut **tx).await.map_err(Error::Internal)?;
    Ok(())
}

async fn insert_adoption(
    tx: &mut Transaction<'_, Postgres>,
    p: &PreparedOcrAdoption,
    derivative: Uuid,
    layout: Uuid,
    adoption: Uuid,
    line_refs: &[Uuid],
) -> Result<(), Error> {
    let map: Vec<_> = p
        .observation
        .anchors
        .lines
        .iter()
        .zip(line_refs)
        .map(|(l, id)| json!({"sourceLineRef":l.line_ref,"lineRef":id}))
        .collect();
    sqlx::query("INSERT INTO linggan_media_ocr_adoption(adoption_ref,derivative_ref,layout_ref,rule_version,input_fingerprint,observation,assessment,line_ref_map) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(adoption).bind(derivative).bind(layout).bind(rule::RULE_VERSION).bind(&p.assessment.input_fingerprint)
        .bind(json!(p.observation)).bind(json!(p.assessment)).bind(json!(map))
        .execute(&mut **tx).await.map_err(Error::Internal)?;
    Ok(())
}

fn candidate_blocks(p: &PreparedOcrAdoption) -> Vec<Value> {
    let mut ordered: Vec<_> = p.observation.layout.blocks.iter().collect();
    ordered.sort_by_key(|b| b.reading_order);
    let mut result = vec![];
    for block in ordered {
        let assessment = p
            .assessment
            .regions
            .iter()
            .find(|r| r.block_ref == block.block_ref);
        let mut pieces =
            crate::research_dossier::content::blocks_from_text(&block.text, "unconfirmed");
        for (index, piece) in pieces.iter_mut().enumerate() {
            piece["ref"] = json!(format!("{}.{index}", block.block_ref));
            let [l, t, r, b] = block.bbox;
            piece["bbox"] = json!([
                l / f64::from(p.observation.image.width),
                t / f64::from(p.observation.image.height),
                (r - l) / f64::from(p.observation.image.width),
                (b - t) / f64::from(p.observation.image.height)
            ]);
            if assessment.is_some_and(|a| matches!(a.role, Role::CoverTitle | Role::ImageHeading))
                && piece["kind"] == "paragraph"
            {
                piece["kind"] = json!("heading");
            }
            if assessment.is_none_or(|a| a.quality != Quality::Supported) {
                piece["issues"] = json!([{"key":format!("{}.{index}:reading",block.block_ref),"code":"uncertain_reading","message":"机器识别内容需核验","resolution":"reading_edit","blockRef":piece["ref"]}]);
            }
        }
        result.extend(pieces);
    }
    if result.is_empty() {
        result.push(json!({"ref":"empty","kind":"gap","speaker":"unconfirmed","text":"〔识别未返回文字，尚未确认图片无文字〕","issues":[{"key":"recognition_empty","code":"recognition_empty","message":"未返回文字不证明原图没有文字","resolution":"reading_edit","blockRef":"empty"}]}));
    }
    result
}
