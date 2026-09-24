//! The replacement Study source gate. Domain qualification and media readability are database
//! predicates, never prompt instructions. This module depends only on retained material
//! evidence and clean comment-study relations.

use crate::comment_cleaning::{CLEANER_VERSION, clean};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Executor, Postgres, Row};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct StudySource {
    pub source_ref: Uuid,
    pub content_public_ref: Uuid,
    pub work_title: String,
    pub research_text: String,
    pub clean_state: String,
    pub context_manifest: Value,
    pub parent_source_ref: Option<Uuid>,
    pub parent_research_text: Option<String>,
    pub observation_role: StudySourceRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StudySourceRole {
    Primary,
    Reference,
}

impl StudySourceRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Reference => "reference",
        }
    }
}

/// A read-only explanation of the exact source gate used by StudyRun freezing.  It deliberately
/// reports source facts and deterministic cleaning outcomes only; it is not a second eligibility
/// engine for the setup page.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourcePreview {
    pub as_of: String,
    pub observation_role: StudySourceRole,
    pub total_comment_count: usize,
    pub eligible_comment_count: usize,
    pub excluded_counts: StudySourceExcludedCounts,
    pub works: Vec<StudySourcePreviewWork>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourceExcludedCounts {
    pub comment_author_unknown: usize,
    pub work_author_unknown: usize,
    pub creator_voice: usize,
    pub body_unavailable: usize,
    pub source_restricted: usize,
    pub text_not_researchable: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudySourcePreviewWork {
    pub work_ref: Uuid,
    pub title: String,
    pub eligible_comment_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum StudySourceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the configured Study domain is invalid")]
    InvalidDomain,
}

/// Lists only current, accepted, readable user comments belonging to the configured domain. The
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

/// Builds the setup-page counts from the same candidate rows and the same eligibility decision
/// used by target freezing. `as_of` is returned so callers can state exactly which snapshot their
/// preview describes; a later Run intentionally re-evaluates at its own frozen snapshot.
pub async fn preview_sources(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
) -> Result<StudySourcePreview, StudySourceError> {
    preview_sources_for_role(database, domain_ref, as_of, StudySourceRole::Primary).await
}

pub async fn preview_sources_for_role(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    observation_role: StudySourceRole,
) -> Result<StudySourcePreview, StudySourceError> {
    let mut previews =
        preview_sources_for_roles(database, domain_ref, as_of, &[observation_role]).await?;
    Ok(previews
        .remove(&observation_role)
        .expect("a requested source role always receives a preview"))
}

pub async fn preview_sources_for_roles(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    observation_roles: &[StudySourceRole],
) -> Result<std::collections::BTreeMap<StudySourceRole, StudySourcePreview>, StudySourceError> {
    ensure_domain_exists(database, domain_ref).await?;
    let role_values: Vec<&str> = observation_roles.iter().map(|role| role.as_str()).collect();
    let candidates = fetch_source_candidates(
        database.pool(),
        domain_ref,
        as_of,
        None,
        None,
        Some(&role_values),
    )
    .await?;
    let mut candidates_by_role =
        std::collections::BTreeMap::<StudySourceRole, Vec<SourceCandidate>>::new();
    for candidate in candidates {
        candidates_by_role
            .entry(candidate.observation_role)
            .or_default()
            .push(candidate);
    }
    Ok(observation_roles
        .iter()
        .copied()
        .map(|role| {
            let candidates = candidates_by_role.remove(&role).unwrap_or_default();
            (role, build_source_preview(candidates, role, as_of))
        })
        .collect())
}

fn build_source_preview(
    candidates: Vec<SourceCandidate>,
    observation_role: StudySourceRole,
    as_of: &str,
) -> StudySourcePreview {
    let total_comment_count = candidates.len();
    let mut excluded_counts = StudySourceExcludedCounts::default();
    let mut eligible_by_work = std::collections::BTreeMap::<Uuid, (String, usize)>::new();

    for candidate in candidates {
        match classify_candidate(candidate) {
            Ok(source) => {
                let entry = eligible_by_work
                    .entry(source.content_public_ref)
                    .or_insert_with(|| (source.work_title, 0));
                entry.1 += 1;
            }
            Err(reason) => reason.increment(&mut excluded_counts),
        }
    }

    let eligible_comment_count = eligible_by_work.values().map(|(_, count)| *count).sum();
    let mut works: Vec<_> = eligible_by_work
        .into_iter()
        .map(
            |(work_ref, (title, eligible_comment_count))| StudySourcePreviewWork {
                work_ref,
                title,
                eligible_comment_count,
            },
        )
        .collect();
    works.sort_by(|left, right| {
        right
            .eligible_comment_count
            .cmp(&left.eligible_comment_count)
            .then_with(|| left.work_ref.cmp(&right.work_ref))
    });
    works.truncate(100);
    StudySourcePreview {
        as_of: as_of.to_owned(),
        observation_role,
        total_comment_count,
        eligible_comment_count,
        excluded_counts,
        works,
    }
}

async fn load_eligible_sources(
    database: &Database,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: Option<&[Uuid]>,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    ensure_domain_exists(database, domain_ref).await?;
    fetch_eligible_sources(
        database.pool(),
        domain_ref,
        as_of,
        content_public_refs,
        StudySourceRole::Primary,
        limit,
    )
    .await
}

async fn ensure_domain_exists(
    database: &Database,
    domain_ref: Uuid,
) -> Result<(), StudySourceError> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain_ref)
            .fetch_one(database.pool())
            .await?;
    exists.then_some(()).ok_or(StudySourceError::InvalidDomain)
}

pub(crate) async fn eligible_sources_in_transaction_for_selections(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    domain_ref: Uuid,
    as_of: &str,
    selections: &[(Uuid, StudySourceRole)],
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError> {
    let domain_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain_ref)
            .fetch_one(&mut **transaction)
            .await?;
    if !domain_exists {
        return Err(StudySourceError::InvalidDomain);
    }
    let limit = limit.clamp(1, 3000);
    let work_refs: Vec<Uuid> = selections.iter().map(|(work_ref, _)| *work_ref).collect();
    let roles: Vec<&str> = selections.iter().map(|(_, role)| role.as_str()).collect();
    Ok(fetch_source_candidates(
        &mut **transaction,
        domain_ref,
        as_of,
        Some(&work_refs),
        Some(&roles),
        None,
    )
    .await?
    .into_iter()
    .filter_map(|candidate| classify_candidate(candidate).ok())
    .take(limit as usize)
    .collect())
}

#[derive(Debug)]
struct SourceCandidate {
    source_ref: Uuid,
    content_public_ref: Uuid,
    observation_role: StudySourceRole,
    work_title: String,
    body_text: Option<String>,
    body_state: String,
    author_external_id: Option<String>,
    work_author_external_id: Option<String>,
    source_restricted: bool,
    parent_source_ref: Option<Uuid>,
    parent_body_text: Option<String>,
    context_fragments: Value,
}

#[derive(Debug, Clone, Copy)]
enum SourceExclusionReason {
    CommentAuthorUnknown,
    WorkAuthorUnknown,
    CreatorVoice,
    BodyUnavailable,
    SourceRestricted,
    TextNotResearchable,
}

impl SourceExclusionReason {
    fn increment(self, counts: &mut StudySourceExcludedCounts) {
        match self {
            Self::CommentAuthorUnknown => counts.comment_author_unknown += 1,
            Self::WorkAuthorUnknown => counts.work_author_unknown += 1,
            Self::CreatorVoice => counts.creator_voice += 1,
            Self::BodyUnavailable => counts.body_unavailable += 1,
            Self::SourceRestricted => counts.source_restricted += 1,
            Self::TextNotResearchable => counts.text_not_researchable += 1,
        }
    }
}

fn classify_candidate(candidate: SourceCandidate) -> Result<StudySource, SourceExclusionReason> {
    if candidate.body_state != "KNOWN" || candidate.body_text.is_none() {
        return Err(SourceExclusionReason::BodyUnavailable);
    }
    let comment_author = candidate
        .author_external_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(SourceExclusionReason::CommentAuthorUnknown)?;
    let work_author = candidate
        .work_author_external_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(SourceExclusionReason::WorkAuthorUnknown)?;
    if comment_author == work_author {
        return Err(SourceExclusionReason::CreatorVoice);
    }
    if candidate.source_restricted {
        return Err(SourceExclusionReason::SourceRestricted);
    }
    let cleaned = clean(candidate.body_text.as_deref().expect("checked above"));
    if !matches!(cleaned.state.as_str(), "direct" | "context") {
        return Err(SourceExclusionReason::TextNotResearchable);
    }
    let parent_cleaned = candidate.parent_body_text.as_deref().map(clean);
    Ok(StudySource {
        source_ref: candidate.source_ref,
        content_public_ref: candidate.content_public_ref,
        work_title: candidate.work_title,
        research_text: cleaned.text,
        clean_state: cleaned.state,
        context_manifest: json!({
            "contract":"comment-study.context.v1",
            "workRef":candidate.content_public_ref,
            "cleanerVersion":CLEANER_VERSION,
            "sources":candidate.context_fragments
        }),
        parent_source_ref: candidate.parent_source_ref,
        parent_research_text: parent_cleaned.and_then(|value| {
            matches!(value.state.as_str(), "direct" | "context").then_some(value.text)
        }),
        observation_role: candidate.observation_role,
    })
}

/// `linggan_material_media_origin` keeps one row per capture observation of a slot, so relating to
/// it by `slot_key` multiplies every derived text by how often that slot was re-observed.  The work
/// ownership check therefore has to stay a semi-join.
async fn fetch_eligible_sources<'e, E>(
    executor: E,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: Option<&[Uuid]>,
    observation_role: StudySourceRole,
    limit: i64,
) -> Result<Vec<StudySource>, StudySourceError>
where
    E: Executor<'e, Database = Postgres>,
{
    let limit = limit.clamp(1, 3000);
    let uniform_roles =
        content_public_refs.map(|works| vec![observation_role.as_str(); works.len()]);
    let preview_roles = content_public_refs
        .is_none()
        .then(|| vec![observation_role.as_str()]);
    let sources = fetch_source_candidates(
        executor,
        domain_ref,
        as_of,
        content_public_refs,
        uniform_roles.as_deref(),
        preview_roles.as_deref(),
    )
    .await?
    .into_iter()
    .filter_map(|candidate| classify_candidate(candidate).ok())
    .take(limit as usize)
    .collect();
    Ok(sources)
}

/// Loads the frozen, de-duplicated comment candidates once.  All consumers turn a candidate into
/// an eligible source through `classify_candidate`, which prevents setup and Run from quietly
/// drifting into separate source gates.
async fn fetch_source_candidates<'e, E>(
    executor: E,
    domain_ref: Uuid,
    as_of: &str,
    content_public_refs: Option<&[Uuid]>,
    selected_roles: Option<&[&str]>,
    preview_roles: Option<&[&str]>,
) -> Result<Vec<SourceCandidate>, StudySourceError>
where
    E: Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query(
        "SELECT source.material_ref,source.content_public_ref,source_domain_role.observation_role,source.body_text,source.body_state, \
                source.author_external_id,content_author.author_external_id AS work_author_external_id, \
                COALESCE((SELECT detail.title FROM linggan_material_content_detail detail \
                  JOIN linggan_runtime_capture_package title_package USING(package_ref) \
                  WHERE detail.content_public_ref=source.content_public_ref AND detail.title_state='KNOWN' \
                    AND title_package.accepted_at<=$2::timestamptz AND detail.created_at<=$2::timestamptz \
                  ORDER BY detail.observed_at DESC,detail.created_at DESC LIMIT 1),'未命名作品') AS work_title, \
                EXISTS(SELECT 1 FROM linggan_material_comment_restriction restriction \
                  WHERE restriction.content_public_ref=source.content_public_ref \
                    AND restriction.comment_external_id=source.comment_external_id) AS source_restricted, \
                parent.material_ref AS parent_source_ref,parent.body_text AS parent_body_text, \
                COALESCE(context.fragments,'[]'::jsonb) AS context_fragments \
         FROM ( \
           SELECT DISTINCT ON (comment.content_public_ref,comment.comment_external_id) \
             comment.material_ref,comment.content_public_ref,comment.comment_external_id, \
             comment.parent_comment_external_id,comment.author_external_id, \
             comment.body_text,comment.body_state,comment.created_at \
           FROM linggan_material_comment comment \
           JOIN linggan_runtime_capture_package package USING(package_ref) \
           WHERE package.accepted_at<=$2::timestamptz AND comment.created_at<=$2::timestamptz \
           ORDER BY comment.content_public_ref,comment.comment_external_id, \
                    comment.observed_at::timestamptz DESC,comment.created_at DESC,comment.material_ref DESC \
         ) source \
         JOIN LATERAL ( \
           SELECT selected.observation_role \
           FROM unnest($3::uuid[],$4::text[]) AS selected(content_public_ref,observation_role) \
           WHERE selected.content_public_ref=source.content_public_ref \
           UNION ALL SELECT preview.observation_role \
             FROM unnest($5::text[]) AS preview(observation_role) \
            WHERE $3::uuid[] IS NULL \
         ) source_domain_role ON true \
         JOIN linggan_material_content content ON content.public_ref=source.content_public_ref \
         LEFT JOIN linggan_material_content_author content_author \
           ON content_author.content_public_ref=source.content_public_ref \
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
         WHERE EXISTS (SELECT 1 FROM linggan_material_domain_usage usage \
                       WHERE usage.content_public_ref=content.public_ref \
                         AND usage.domain_ref=$1 \
                         AND usage.role=source_domain_role.observation_role) \
         ORDER BY row_number() OVER ( \
                    PARTITION BY source.content_public_ref \
                    ORDER BY source.created_at DESC,source.material_ref DESC \
                  ),source.content_public_ref",
    )
    .bind(domain_ref)
    .bind(as_of)
    .bind(content_public_refs.map(<[Uuid]>::to_vec))
    .bind(selected_roles.map(<[&str]>::to_vec))
    .bind(preview_roles.map(<[&str]>::to_vec))
    .fetch_all(executor)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| SourceCandidate {
            source_ref: row.get("material_ref"),
            content_public_ref: row.get("content_public_ref"),
            observation_role: match row.get::<String, _>("observation_role").as_str() {
                "primary" => StudySourceRole::Primary,
                "reference" => StudySourceRole::Reference,
                _ => unreachable!("the source-role query only returns selected Domain roles"),
            },
            work_title: row.get("work_title"),
            body_text: row.get("body_text"),
            body_state: row.get("body_state"),
            author_external_id: row.get("author_external_id"),
            work_author_external_id: row.get("work_author_external_id"),
            source_restricted: row.get("source_restricted"),
            parent_source_ref: row.get("parent_source_ref"),
            parent_body_text: row.get("parent_body_text"),
            context_fragments: row.get("context_fragments"),
        })
        .collect())
}
