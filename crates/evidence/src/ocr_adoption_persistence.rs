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

mod read;
mod review;
mod validation;
pub(crate) use read::attach_media;
pub(crate) use read::project_dossier_images;
pub(crate) use review::attach_in;

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
    pub image_text: String,
    pub cover_title: Option<String>,
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
    let projection = rule::project_work(std::slice::from_ref(&assessment))?;
    let image_text = projection
        .images
        .first()
        .map(|i| i.text.clone())
        .unwrap_or_default();
    let cover_title = assessment.cover_title.text.clone();
    Ok(PreparedOcrAdoption {
        observation: b.observation,
        assessment,
        image_text,
        cover_title,
        raw_hash: rule::content_hash(raw),
        raw_size: raw.len() as i64,
    })
}

/// Metadata from the admitted job's source, not from a model or a caller's filename.
pub async fn source_metadata(db: &Database, claim: &MediaProcessingClaim) -> Result<Value, Error> {
    let row=sqlx::query("SELECT job.slot_key,slot.ordinal,bool_or(origin.purpose='cover') AS cover \
        FROM linggan_media_processing_job job JOIN linggan_media_slot slot USING(slot_key) \
        JOIN linggan_material_media_origin origin ON origin.slot_key=job.slot_key \
        WHERE job.job_ref=$1 AND job.blob_sha256=$2 AND job.processor_kind='image_ocr' \
          AND job.processor_version=$3 AND origin.content_public_ref=$4 \
          AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d WHERE d.state='WITHDRAWN_OR_RESTRICTED' AND (d.slot_key=job.slot_key OR d.blob_sha256=job.blob_sha256)) \
        GROUP BY job.slot_key,slot.ordinal")
        .bind(claim.job_ref).bind(&claim.blob_sha256).bind(PROCESSOR_VERSION).bind(claim.content_public_ref)
        .fetch_optional(db.pool()).await.map_err(Error::Internal)?.ok_or(Error::MaterialIdentityConflict)?;
    let ordinal: Option<i32> = row.get("ordinal");
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
    let lines = legacy_anchor_view(&prepared.observation);
    let refs = insert_ocr_lines(&mut tx, layout, &lines).await?;
    insert_adoption(&mut tx, &prepared, derivative, layout, adoption, &refs).await?;
    finish_processing_claim(&mut tx, claim, worker).await?;
    tx.commit().await.map_err(Error::Internal)?;
    Ok(derivative)
}

fn legacy_anchor_view(input: &ImageInput) -> Vec<OcrLineInput> {
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
    let state = if p.image_text.is_empty() && p.cover_title.is_none() {
        "NEEDS_REVIEW"
    } else if p
        .assessment
        .regions
        .iter()
        .any(|r| r.adoption_state.has_unresolved_text())
    {
        "PARTIAL"
    } else {
        "ACCEPTED"
    };
    let map: Vec<_> = p
        .observation
        .anchors
        .lines
        .iter()
        .zip(line_refs)
        .map(|(l, id)| json!({"sourceLineRef":l.line_ref,"lineRef":id}))
        .collect();
    sqlx::query("INSERT INTO linggan_media_ocr_adoption(adoption_ref,derivative_ref,layout_ref,rule_version,input_fingerprint,observation,assessment,line_ref_map,image_text,cover_title,state) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(adoption).bind(derivative).bind(layout).bind(rule::RULE_VERSION).bind(&p.assessment.input_fingerprint)
        .bind(json!(p.observation)).bind(json!(p.assessment)).bind(json!(map)).bind(&p.image_text).bind(&p.cover_title).bind(state)
        .execute(&mut **tx).await.map_err(Error::Internal)?;
    // This is the explicitly adopted machine-text projection. The derivative asset is the
    // separately hashed raw bundle, never disguised as the cleaned text or author prose.
    let text = if p.image_text.is_empty() {
        p.cover_title.as_deref().unwrap_or("")
    } else {
        &p.image_text
    };
    if !text.is_empty() {
        sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text,language_state,language_tag,source_location) VALUES($1,$2,'ocr_text',$3,$4,'UNKNOWN',NULL,$5)")
            .bind(derivative).bind(p.observation.image.work_ref).bind(text).bind(text.chars().take(600).collect::<String>())
            .bind(json!({"blobSha256":p.observation.image.blob_sha256,"processorVersion":PROCESSOR_VERSION,"layoutRef":layout,"adoptionRef":adoption,"ruleVersion":rule::RULE_VERSION,"sourceRelation":"image_transcription_not_author_claim","position":{"imageOrdinal":p.observation.sequence_ordinal}}))
            .execute(&mut **tx).await.map_err(Error::Internal)?;
    }
    Ok(())
}
