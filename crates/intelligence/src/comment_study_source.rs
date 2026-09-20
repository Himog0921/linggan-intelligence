//! The replacement Study source gate. Domain qualification and media readability are database
//! predicates, never prompt instructions. This module depends only on retained material
//! evidence and clean comment-study relations.

use crate::comment_cleaning::{CLEANER_VERSION, clean};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Executor, Postgres, Row};
use uuid::Uuid;

pub const ADHD_DOMAIN_REF: &str = "00000000-0000-4000-8000-000000000001";

#[derive(Debug, Clone, PartialEq)]
pub struct StudySource {
    pub source_ref: Uuid,
    pub content_public_ref: Uuid,
    pub research_text: String,
    pub clean_state: String,
    pub context_manifest: Value,
    pub parent_source_ref: Option<Uuid>,
    pub parent_research_text: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StudySourceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the configured Study domain is invalid")]
    InvalidDomain,
}

/// Lists only current, accepted, readable comments belonging to the configured domain. The
/// context array contains source-qualified work text; `text_content` from a withdrawn or
/// restricted derivative cannot pass this query.
pub async fn eligible_sources(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    load_eligible_sources(database, domain_ref, as_of, None, limit).await
}

/// The same database gate, narrowed to the works a user selected for a single Study preview or
/// Run. The filter is part of the query, so a globally popular work cannot consume the caller's
/// budget before a selected work is considered.
pub async fn eligible_sources_for_works(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: &[Uuid],
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    if content_public_refs.is_empty() {
        return Ok(Vec::new());
    }
    load_eligible_sources(
        database,
        domain_ref,
        as_of,
        Some(content_public_refs),
        limit,
    )
    .await
}

async fn load_eligible_sources(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: Option<&[Uuid]>,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    if domain_ref.to_string() != ADHD_DOMAIN_REF {
        return Err(StudySourceError::InvalidDomain);
    }
    fetch_eligible_sources(
        database.pool(),
        domain_ref,
        as_of,
        content_public_refs,
        limit,
    )
    .await
}

pub(crate) async fn eligible_sources_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: &[Uuid],
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    if domain_ref.to_string() != ADHD_DOMAIN_REF {
        return Err(StudySourceError::InvalidDomain);
    }
    fetch_eligible_sources(
        &mut **transaction,
        domain_ref,
        as_of,
        Some(content_public_refs),
        limit,
    )
    .await
}

/// `linggan_material_media_origin` keeps one row per capture observation of a slot, so relating to
/// it by `slot_key` multiplies every derived text by how often that slot was re-observed.  The work
/// ownership check therefore has to stay a semi-join.
async fn fetch_eligible_sources<'e, E>(
    executor: E,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: Option<&[Uuid]>,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError>
where
    E: Executor<'e, Database = Postgres>,
{
    let limit = limit.clamp(1, 3000);
    let rows = sqlx::query(
        "SELECT source.material_ref,source.content_public_ref,source.body_text, \
                parent.material_ref AS parent_source_ref,parent.body_text AS parent_body_text, \
                COALESCE(context.fragments,'[]'::jsonb) AS context_fragments \
         FROM ( \
           SELECT DISTINCT ON (comment.content_public_ref,comment.comment_external_id) \
             comment.material_ref,comment.content_public_ref,comment.comment_external_id, \
             comment.parent_comment_external_id, \
             comment.body_text,comment.body_state,comment.created_at \
           FROM linggan_material_comment comment \
           JOIN linggan_runtime_capture_package package USING(package_ref) \
           WHERE package.accepted_at<=$2::timestamptz AND comment.created_at<=$2::timestamptz \
           ORDER BY comment.content_public_ref,comment.comment_external_id, \
                    comment.observed_at::timestamptz DESC,comment.created_at DESC,comment.material_ref DESC \
         ) source \
         JOIN linggan_material_content content ON content.public_ref=source.content_public_ref \
         LEFT JOIN LATERAL ( \
           SELECT ancestor.material_ref,ancestor.body_text \
           FROM linggan_material_comment ancestor \
           JOIN linggan_runtime_capture_package ancestor_package USING(package_ref) \
           WHERE ancestor.content_public_ref=source.content_public_ref \
             AND ancestor.comment_external_id=source.parent_comment_external_id \
             AND ancestor.body_state='KNOWN' \
             AND ancestor_package.accepted_at<=$2::timestamptz \
             AND ancestor.created_at<=$2::timestamptz \
           ORDER BY ancestor.observed_at::timestamptz DESC,ancestor.created_at DESC,ancestor.material_ref DESC \
           LIMIT 1 \
         ) parent ON true \
         LEFT JOIN LATERAL ( \
           SELECT jsonb_agg(fragment ORDER BY fragment->>'kind',fragment->>'sourceRef') AS fragments \
           FROM ( \
             SELECT fragment FROM ( \
             SELECT jsonb_build_object('kind','native_title','sourceRef',detail.material_ref, \
                'text',detail.title,'observedAt',detail.observed_at) AS fragment \
             FROM linggan_material_content_detail detail \
             JOIN linggan_runtime_capture_package detail_package USING(package_ref) \
             WHERE detail.content_public_ref=source.content_public_ref \
               AND detail.title_state='KNOWN' AND detail_package.accepted_at<=$2::timestamptz \
             ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC LIMIT 1 \
           ) title \
           UNION ALL \
           SELECT fragment FROM ( \
             SELECT jsonb_build_object('kind','body','sourceRef',detail.material_ref, \
                'text',detail.body_text,'observedAt',detail.observed_at) AS fragment \
             FROM linggan_material_content_detail detail \
             JOIN linggan_runtime_capture_package detail_package USING(package_ref) \
             WHERE detail.content_public_ref=source.content_public_ref \
               AND detail.body_state='KNOWN' AND detail_package.accepted_at<=$2::timestamptz \
             ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC LIMIT 1 \
           ) body \
           UNION ALL \
           SELECT jsonb_build_object('kind',text.kind,'sourceRef',derived.derivative_ref, \
              'text',text.text_content,'sourceLocation',text.source_location, \
              'slotOrdinal',slot.ordinal,'contentHash',derived.content_hash) \
           FROM linggan_material_derived_text text \
           JOIN linggan_media_derivative derived USING(derivative_ref) \
           JOIN linggan_media_processing_job job USING(job_ref) \
           LEFT JOIN linggan_media_slot slot ON slot.slot_key=job.slot_key \
           WHERE text.content_public_ref=source.content_public_ref \
             AND text.kind<>'ocr_text' \
             AND EXISTS (SELECT 1 FROM linggan_material_media_origin origin \
               WHERE origin.slot_key=job.slot_key \
                 AND origin.content_public_ref=source.content_public_ref) \
             AND text.created_at<=$2::timestamptz AND derived.created_at<=$2::timestamptz \
             AND job.created_at<=$2::timestamptz \
             AND (SELECT event.state FROM linggan_media_processing_job_event event \
                  WHERE event.job_ref=job.job_ref AND event.occurred_at<=$2::timestamptz \
                  ORDER BY event.occurred_at DESC,event.event_ref DESC LIMIT 1)='succeeded' \
             AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition disposition \
               WHERE disposition.state='WITHDRAWN_OR_RESTRICTED' \
                 AND (disposition.derivative_ref=derived.derivative_ref \
                 OR disposition.blob_sha256=job.blob_sha256 \
                  OR disposition.slot_key=job.slot_key)) \
           UNION ALL \
           SELECT jsonb_build_object('kind','image_substantive_text','sourceRef',derived.derivative_ref, \
              'text',layer.image_substantive_text,'sourceLocation',text.source_location, \
              'slotOrdinal',slot.ordinal,'contentHash',derived.content_hash) \
           FROM linggan_material_derived_text text \
           JOIN linggan_media_derivative derived USING(derivative_ref) \
           JOIN linggan_media_processing_job job USING(job_ref) \
           JOIN linggan_media_ocr_layout layout ON layout.ocr_derivative_ref=derived.derivative_ref \
           JOIN LATERAL ( \
             SELECT result.state,result.image_substantive_text \
             FROM linggan_media_ocr_layering_result result \
             WHERE result.layout_ref=layout.layout_ref AND result.created_at<=$2::timestamptz \
             ORDER BY result.created_at DESC,result.layering_ref DESC LIMIT 1 \
           ) layer ON layer.state='ACCEPTED' \
           LEFT JOIN linggan_media_slot slot ON slot.slot_key=job.slot_key \
           WHERE text.content_public_ref=source.content_public_ref AND text.kind='ocr_text' \
             AND nullif(btrim(layer.image_substantive_text),'') IS NOT NULL \
             AND EXISTS (SELECT 1 FROM linggan_material_media_origin origin \
               WHERE origin.slot_key=job.slot_key \
                 AND origin.content_public_ref=source.content_public_ref) \
             AND text.created_at<=$2::timestamptz AND derived.created_at<=$2::timestamptz \
             AND job.created_at<=$2::timestamptz AND layout.created_at<=$2::timestamptz \
             AND (SELECT event.state FROM linggan_media_processing_job_event event \
                  WHERE event.job_ref=job.job_ref AND event.occurred_at<=$2::timestamptz \
                  ORDER BY event.occurred_at DESC,event.event_ref DESC LIMIT 1)='succeeded' \
             AND NOT EXISTS (SELECT 1 FROM linggan_media_ocr_retirement retired \
               WHERE retired.retired_job_ref=job.job_ref) \
             AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition disposition \
               WHERE disposition.state='WITHDRAWN_OR_RESTRICTED' \
                 AND (disposition.derivative_ref=derived.derivative_ref \
                   OR disposition.blob_sha256=job.blob_sha256 \
                   OR disposition.slot_key=job.slot_key)) \
           ) context_fragments \
         ) context ON true \
         WHERE content.domain_ref=$1 AND source.body_state='KNOWN' \
           AND ($4::uuid[] IS NULL OR source.content_public_ref=ANY($4::uuid[])) \
           AND NOT EXISTS (SELECT 1 FROM linggan_material_comment_restriction restriction \
             WHERE restriction.content_public_ref=source.content_public_ref \
               AND restriction.comment_external_id=source.comment_external_id) \
         ORDER BY row_number() OVER ( \
                    PARTITION BY source.content_public_ref \
                    ORDER BY source.created_at DESC,source.material_ref DESC \
                  ),source.content_public_ref LIMIT $3",
    )
    .bind(domain_ref)
    .bind(as_of)
    .bind(limit)
    .bind(content_public_refs.map(<[Uuid]>::to_vec))
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let raw: Option<String> = row.get("body_text");
            let cleaned = clean(raw.as_deref()?);
            let parent_raw: Option<String> = row.get("parent_body_text");
            let parent_cleaned = parent_raw.as_deref().map(clean);
            matches!(cleaned.state.as_str(), "direct" | "context").then(|| StudySource {
                source_ref: row.get("material_ref"),
                content_public_ref: row.get("content_public_ref"),
                research_text: cleaned.text,
                clean_state: cleaned.state,
                context_manifest: json!({
                    "contract":"comment-study.context.v1",
                    "workRef":row.get::<Uuid,_>("content_public_ref"),
                    "cleanerVersion":CLEANER_VERSION,
                    "sources":row.get::<Value,_>("context_fragments")
                }),
                parent_source_ref: row.get("parent_source_ref"),
                parent_research_text: parent_cleaned.and_then(|value| {
                    matches!(value.state.as_str(), "direct" | "context").then_some(value.text)
                }),
            })
        })
        .collect())
}
