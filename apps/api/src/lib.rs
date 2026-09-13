//! Minimal HTTP composition for the Comment Research User Voices V0 read path.
//!
//! This module exposes only current, deterministically cleaned comment
//! expressions proven by the V0/V1 storage proofs. It does not create research
//! work, call a model, or invent unproven work metadata.

mod user_voices_page_v0;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_contracts::ContextTextAvailabilityV0;
use linggan_domain::comment_research::{
    COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_BODY_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_TITLE_MAX_CHARS, CommentResearchContextPackV1,
    ContextPackOmissionV1, ContextPackPreviewSourceTextV1, ContextPackPreviewTextV1,
    ContextPackReadinessV1,
};
use linggan_storage_postgres::{
    COMMENT_RESEARCH_PLAN_PREVIEW_V0_DEFAULT_LIMIT, CURRENT_COMMENT_VOICES_V0_MAX_LIMIT,
    CommentFactStore, ContextSourceEvidenceRelationV0, CurrentCommentContextBySourceLocatorV0,
    CurrentCommentRelatedReplyV0, CurrentCommentResearchPlanPreviewRequestV0,
    CurrentCommentResearchPlanPreviewV0, CurrentCommentResearchPreviewCandidateV0,
    CurrentCommentResearchPreviewSourceV0, CurrentCommentSourceEvidenceRelationV0,
    CurrentCommentVoiceFilterV1, CurrentCommentVoiceV0, CurrentCommentVoicesPageRequestV0,
    CurrentCommentWorkContextV0, StorageError,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The bounded default used when callers omit limit.
pub const USER_VOICES_V0_DEFAULT_LIMIT: i64 = 50;

/// Builds the only Comment Research HTTP route enabled by this V0 package.
///
/// The route is read-only by construction: its handler invokes the storage
/// adapter's current-projection SELECT method and has no admission dependency.
pub fn comment_research_router_v0(store: Arc<CommentFactStore>) -> Router {
    Router::new()
        .route(
            "/comment-research/voices",
            get(user_voices_page_v0::user_voices_page_v0),
        )
        .route(
            "/comment-research/voices/styles.css",
            get(user_voices_page_v0::user_voices_page_v0_stylesheet),
        )
        .route("/api/v0/comment-research/voices", get(list_user_voices_v0))
        .route(
            "/api/v0/comment-research/plan-preview",
            get(preview_current_comment_research_plan_v0),
        )
        .route(
            "/api/v0/comment-research/voices/context",
            get(get_user_voice_context_v0),
        )
        .route(
            "/api/v0/comment-research/voices/context-pack",
            get(get_user_voice_context_pack_v1),
        )
        .with_state(store)
}

#[derive(Debug, Deserialize)]
struct UserVoicesQueryV0 {
    workspace_id: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
    filter: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CurrentCommentResearchPlanPreviewQueryV0 {
    workspace_id: Option<String>,
    scope: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Serialize)]
struct UserVoicesResponseV0 {
    pagination: UserVoicesPaginationV0,
    preparation: UserVoicesPreparationSummaryV1,
    voices: Vec<UserVoiceDtoV0>,
}

#[derive(Debug, Serialize)]
struct UserVoicesPaginationV0 {
    total: i64,
    limit: i64,
    offset: i64,
}

#[derive(Debug, Serialize)]
struct UserVoicesPreparationSummaryV1 {
    filter: &'static str,
    awaiting_cleaning_total: i64,
}

/// The intentionally narrow browser DTO. It returns only direct current
/// comment facts. Work and discussion context is loaded separately from the
/// list-visible Evidence locator, so this endpoint does not make a premature
/// availability claim for the table.
#[derive(Debug, Serialize)]
struct UserVoiceDtoV0 {
    research_text: String,
    source_note_id: String,
    current_admitted_at: String,
    source_evidence: UserVoiceSourceEvidenceDtoV0,
    readiness: &'static str,
}

#[derive(Debug, Serialize)]
struct UserVoiceSourceEvidenceDtoV0 {
    evidence_id: String,
    record_index: i32,
}

impl From<CurrentCommentVoiceV0> for UserVoiceDtoV0 {
    fn from(voice: CurrentCommentVoiceV0) -> Self {
        Self {
            research_text: voice.research_text,
            source_note_id: voice.source_note_id,
            current_admitted_at: voice.current_admitted_at,
            source_evidence: UserVoiceSourceEvidenceDtoV0 {
                evidence_id: voice.source_evidence.evidence_id.to_string(),
                record_index: voice.source_evidence.record_index,
            },
            readiness: voice.readiness.as_api_value(),
        }
    }
}

async fn list_user_voices_v0(
    State(store): State<Arc<CommentFactStore>>,
    Query(query): Query<UserVoicesQueryV0>,
) -> Result<Json<UserVoicesResponseV0>, ApiError> {
    let workspace_id = query
        .workspace_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::InvalidRequest {
            message: "workspace_id is required",
        })?;
    let limit = parse_i64_parameter(query.limit, "limit", USER_VOICES_V0_DEFAULT_LIMIT)?;
    let offset = parse_i64_parameter(query.offset, "offset", 0)?;
    let filter = parse_user_voices_filter(query.filter)?;
    let page = CurrentCommentVoicesPageRequestV0::with_filter(limit, offset, filter)
        .map_err(ApiError::from_storage_input_error)?;

    let result = store
        .list_current_comment_voices_v0(&workspace_id, page)
        .await
        .map_err(ApiError::from_storage_read_error)?;

    Ok(Json(UserVoicesResponseV0 {
        pagination: UserVoicesPaginationV0 {
            total: result.total,
            limit,
            offset,
        },
        preparation: UserVoicesPreparationSummaryV1 {
            filter: filter.as_storage_value(),
            awaiting_cleaning_total: result.awaiting_cleaning_total,
        },
        voices: result
            .voices
            .into_iter()
            .map(UserVoiceDtoV0::from)
            .collect(),
    }))
}

/// Returns a live scope explanation only. It intentionally does not create a
/// plan, reserve rows, freeze an input sample, schedule work, or contact a
/// model. Any later execution capability must use a separately authorized
/// write path.
async fn preview_current_comment_research_plan_v0(
    State(store): State<Arc<CommentFactStore>>,
    Query(query): Query<CurrentCommentResearchPlanPreviewQueryV0>,
) -> Result<Json<CurrentCommentResearchPlanPreviewResponseV0>, ApiError> {
    let workspace_id = query
        .workspace_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::InvalidRequest {
            message: "workspace_id is required",
        })?;
    let scope = parse_current_comment_research_plan_scope(query.scope)?;
    let limit = parse_i64_parameter(
        query.limit,
        "plan_preview_limit",
        COMMENT_RESEARCH_PLAN_PREVIEW_V0_DEFAULT_LIMIT,
    )?;
    let request = CurrentCommentResearchPlanPreviewRequestV0::with_scope(limit, scope)
        .map_err(ApiError::from_storage_input_error)?;
    let preview = store
        .preview_current_comment_research_plan_v0(&workspace_id, request)
        .await
        .map_err(ApiError::from_storage_read_error)?;

    Ok(Json(preview.into()))
}

#[derive(Debug, Serialize)]
struct CurrentCommentResearchPlanPreviewResponseV0 {
    preview_state: &'static str,
    scope: &'static str,
    limit: i64,
    preparation: CurrentCommentResearchPlanPreparationDtoV0,
    source_distribution: Vec<CurrentCommentResearchPreviewSourceDtoV0>,
    candidates: Vec<CurrentCommentResearchPreviewCandidateDtoV0>,
}

#[derive(Debug, Serialize)]
struct CurrentCommentResearchPlanPreparationDtoV0 {
    current_total: i64,
    available_total: i64,
    ready_total: i64,
    needs_context_total: i64,
    awaiting_cleaning_total: i64,
    excluded_total: i64,
}

#[derive(Debug, Serialize)]
struct CurrentCommentResearchPreviewSourceDtoV0 {
    source_note_id: String,
    eligible_total: i64,
    selected_total: i64,
}

/// Candidate text is the already-visible deterministic research expression,
/// not the raw captured original. The browser gets only the same Evidence
/// locator it can already use for the User Voices detail drawer.
#[derive(Debug, Serialize)]
struct CurrentCommentResearchPreviewCandidateDtoV0 {
    source_note_id: String,
    research_text: String,
    readiness: &'static str,
    current_admitted_at: String,
    source_rotation_turn: i64,
    source_evidence: UserVoiceSourceEvidenceDtoV0,
}

impl From<CurrentCommentResearchPlanPreviewV0> for CurrentCommentResearchPlanPreviewResponseV0 {
    fn from(preview: CurrentCommentResearchPlanPreviewV0) -> Self {
        Self {
            preview_state: "live_read_only",
            scope: preview.scope.as_storage_value(),
            limit: preview.limit,
            preparation: CurrentCommentResearchPlanPreparationDtoV0 {
                current_total: preview.totals.current_total,
                available_total: preview.totals.available_total,
                ready_total: preview.totals.ready_total,
                needs_context_total: preview.totals.needs_context_total,
                awaiting_cleaning_total: preview.totals.awaiting_cleaning_total,
                excluded_total: preview.totals.excluded_total,
            },
            source_distribution: preview
                .sources
                .into_iter()
                .map(CurrentCommentResearchPreviewSourceDtoV0::from)
                .collect(),
            candidates: preview
                .candidates
                .into_iter()
                .map(CurrentCommentResearchPreviewCandidateDtoV0::from)
                .collect(),
        }
    }
}

impl From<CurrentCommentResearchPreviewSourceV0> for CurrentCommentResearchPreviewSourceDtoV0 {
    fn from(source: CurrentCommentResearchPreviewSourceV0) -> Self {
        Self {
            source_note_id: source.source_note_id,
            eligible_total: source.eligible_total,
            selected_total: source.selected_total,
        }
    }
}

impl From<CurrentCommentResearchPreviewCandidateV0>
    for CurrentCommentResearchPreviewCandidateDtoV0
{
    fn from(candidate: CurrentCommentResearchPreviewCandidateV0) -> Self {
        Self {
            source_note_id: candidate.source_note_id,
            research_text: candidate.research_text,
            readiness: candidate.readiness.as_api_value(),
            current_admitted_at: candidate.current_admitted_at,
            source_rotation_turn: candidate.source_turn,
            source_evidence: candidate.source_evidence.into(),
        }
    }
}

fn parse_user_voices_filter(raw: Option<String>) -> Result<CurrentCommentVoiceFilterV1, ApiError> {
    match raw.as_deref().unwrap_or("available") {
        "available" => Ok(CurrentCommentVoiceFilterV1::Available),
        "ready" => Ok(CurrentCommentVoiceFilterV1::Ready),
        "needs_context" => Ok(CurrentCommentVoiceFilterV1::NeedsContext),
        _ => Err(ApiError::InvalidRequest {
            message: "filter must be available, ready, or needs_context",
        }),
    }
}

fn parse_current_comment_research_plan_scope(
    raw: Option<String>,
) -> Result<CurrentCommentVoiceFilterV1, ApiError> {
    match raw.as_deref().unwrap_or("available") {
        "available" => Ok(CurrentCommentVoiceFilterV1::Available),
        "ready" => Ok(CurrentCommentVoiceFilterV1::Ready),
        "needs_context" => Ok(CurrentCommentVoiceFilterV1::NeedsContext),
        _ => Err(ApiError::InvalidRequest {
            message: "scope must be available, ready, or needs_context",
        }),
    }
}

#[derive(Debug, Deserialize)]
struct UserVoiceContextQueryV0 {
    workspace_id: Option<String>,
    evidence_id: Option<String>,
    record_index: Option<String>,
}

/// The browser-visible context response is deliberately narrower than the
/// storage result: it has no comment IDs, author identities, URLs, engagement
/// counts, platform time, OCR, or ASR. Relation booleans only say which
/// observed pointer connected an already-captured reply to the current voice.
#[derive(Debug, Serialize)]
struct UserVoiceContextResponseV0 {
    availability: &'static str,
    /// The captured current CommentObservation source text. It is shown only
    /// in the detail drawer; the list remains the deterministic research text.
    original_voice_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    work_context: Option<UserVoiceWorkContextDtoV0>,
    #[serde(skip_serializing_if = "Option::is_none")]
    related_discussion: Option<Vec<UserVoiceRelatedDiscussionDtoV0>>,
}

#[derive(Debug, Serialize)]
struct UserVoiceWorkContextDtoV0 {
    source_evidence: UserVoiceSourceEvidenceDtoV0,
    title: UserVoiceContextTextDtoV0,
    body_text: UserVoiceContextTextDtoV0,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextTextDtoV0 {
    availability: &'static str,
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct UserVoiceRelatedDiscussionDtoV0 {
    text: String,
    relationship: UserVoiceRelatedDiscussionRelationshipDtoV0,
    source_evidence: UserVoiceSourceEvidenceDtoV0,
}

#[derive(Debug, Serialize)]
struct UserVoiceRelatedDiscussionRelationshipDtoV0 {
    root_comment: bool,
    parent_comment: bool,
    reply_to_comment: bool,
}

impl From<ContextSourceEvidenceRelationV0> for UserVoiceSourceEvidenceDtoV0 {
    fn from(relation: ContextSourceEvidenceRelationV0) -> Self {
        Self {
            evidence_id: relation.evidence_id.to_string(),
            record_index: relation.record_index,
        }
    }
}

impl From<CurrentCommentSourceEvidenceRelationV0> for UserVoiceSourceEvidenceDtoV0 {
    fn from(relation: CurrentCommentSourceEvidenceRelationV0) -> Self {
        Self {
            evidence_id: relation.evidence_id.to_string(),
            record_index: relation.record_index,
        }
    }
}

impl From<ContextTextAvailabilityV0> for UserVoiceContextTextDtoV0 {
    fn from(availability: ContextTextAvailabilityV0) -> Self {
        match availability {
            ContextTextAvailabilityV0::Unavailable => Self {
                availability: "unavailable",
                text: None,
            },
            ContextTextAvailabilityV0::Blank => Self {
                availability: "blank",
                text: None,
            },
            ContextTextAvailabilityV0::Observed(text) => Self {
                availability: "observed",
                text: Some(text),
            },
        }
    }
}

impl From<CurrentCommentWorkContextV0> for UserVoiceWorkContextDtoV0 {
    fn from(context: CurrentCommentWorkContextV0) -> Self {
        Self {
            source_evidence: context.source_evidence.into(),
            title: context.title.into(),
            body_text: context.body_text.into(),
        }
    }
}

impl From<CurrentCommentRelatedReplyV0> for UserVoiceRelatedDiscussionDtoV0 {
    fn from(reply: CurrentCommentRelatedReplyV0) -> Self {
        Self {
            text: reply.text,
            relationship: UserVoiceRelatedDiscussionRelationshipDtoV0 {
                root_comment: reply.root_comment_id.is_some(),
                parent_comment: reply.parent_comment_id.is_some(),
                reply_to_comment: reply.reply_to_comment_id.is_some(),
            },
            source_evidence: reply.source_evidence.into(),
        }
    }
}

impl From<CurrentCommentContextBySourceLocatorV0> for UserVoiceContextResponseV0 {
    fn from(lookup: CurrentCommentContextBySourceLocatorV0) -> Self {
        let Some(context) = lookup.context else {
            return Self {
                availability: "unavailable",
                original_voice_text: lookup.original_source_text,
                work_context: None,
                related_discussion: None,
            };
        };
        Self {
            availability: "available",
            original_voice_text: lookup.original_source_text,
            work_context: Some(context.work_context.into()),
            related_discussion: Some(
                context
                    .related_replies
                    .into_iter()
                    .map(UserVoiceRelatedDiscussionDtoV0::from)
                    .collect(),
            ),
        }
    }
}

async fn get_user_voice_context_v0(
    State(store): State<Arc<CommentFactStore>>,
    Query(query): Query<UserVoiceContextQueryV0>,
) -> Result<Json<UserVoiceContextResponseV0>, ApiError> {
    let workspace_id = query
        .workspace_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::InvalidRequest {
            message: "workspace_id is required",
        })?;
    let evidence_id = query
        .evidence_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::InvalidRequest {
            message: "evidence_id is required",
        })?;
    let evidence_id = Uuid::parse_str(&evidence_id).map_err(|_| ApiError::InvalidRequest {
        message: "evidence_id must be a UUID",
    })?;
    let source_record_index = parse_source_record_index(query.record_index)?;

    let Some(lookup) = store
        .get_current_comment_context_by_source_locator_v0(
            &workspace_id,
            evidence_id,
            source_record_index,
        )
        .await
        .map_err(ApiError::from_storage_read_error)?
    else {
        return Err(ApiError::CurrentVoiceNotFound);
    };

    Ok(Json(lookup.into()))
}

/// Returns a bounded, source-backed content assembly preview for one current
/// cleaned User Voice. It has no prompt/system contract, execution state,
/// provider choice, token data, or persistence side effect. The browser uses
/// the same list-visible Evidence locator as the regular detail drawer.
async fn get_user_voice_context_pack_v1(
    State(store): State<Arc<CommentFactStore>>,
    Query(query): Query<UserVoiceContextQueryV0>,
) -> Result<Json<UserVoiceContextPackResponseV1>, ApiError> {
    let workspace_id = query
        .workspace_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::InvalidRequest {
            message: "workspace_id is required",
        })?;
    let evidence_id = query
        .evidence_id
        .filter(|value| !value.trim().is_empty())
        .ok_or(ApiError::InvalidRequest {
            message: "evidence_id is required",
        })?;
    let evidence_id = Uuid::parse_str(&evidence_id).map_err(|_| ApiError::InvalidRequest {
        message: "evidence_id must be a UUID",
    })?;
    let source_record_index = parse_source_record_index(query.record_index)?;

    let Some(pack) = store
        .get_current_comment_research_context_pack_by_source_locator_v1(
            &workspace_id,
            evidence_id,
            source_record_index,
        )
        .await
        .map_err(ApiError::from_storage_read_error)?
    else {
        return Err(ApiError::CurrentVoiceNotFound);
    };

    Ok(Json(pack.into()))
}

/// This DTO intentionally contains assembled source text only. It must never
/// become a route for raw Evidence, comment identity, author identity, model
/// settings, Prompt text, execution objects, or model output.
#[derive(Debug, Serialize)]
struct UserVoiceContextPackResponseV1 {
    preview_state: &'static str,
    readiness: &'static str,
    direct_comment_evidence: UserVoiceContextPackPreviewTextDtoV1,
    discussion_context: UserVoiceContextPackDiscussionDtoV1,
    work_context: UserVoiceContextPackWorkDtoV1,
    budget: UserVoiceContextPackBudgetDtoV1,
    omissions: Vec<&'static str>,
    future_execution_note: &'static str,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextPackPreviewTextDtoV1 {
    text: String,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextPackDiscussionDtoV1 {
    availability: &'static str,
    excerpts: Vec<UserVoiceContextPackDiscussionExcerptDtoV1>,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextPackDiscussionExcerptDtoV1 {
    text: UserVoiceContextPackPreviewTextDtoV1,
    relationship: UserVoiceRelatedDiscussionRelationshipDtoV0,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextPackWorkDtoV1 {
    availability: &'static str,
    title: UserVoiceContextPackSourceTextDtoV1,
    body_text: UserVoiceContextPackSourceTextDtoV1,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextPackSourceTextDtoV1 {
    availability: &'static str,
    text: Option<String>,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct UserVoiceContextPackBudgetDtoV1 {
    included_characters: usize,
    total_character_limit: usize,
    direct_comment_evidence_character_limit: usize,
    related_discussion_item_limit: usize,
    related_discussion_item_character_limit: usize,
    work_title_character_limit: usize,
    work_body_character_limit: usize,
}

impl From<ContextPackPreviewTextV1> for UserVoiceContextPackPreviewTextDtoV1 {
    fn from(value: ContextPackPreviewTextV1) -> Self {
        Self {
            text: value.text,
            truncated: value.truncated,
        }
    }
}

impl From<ContextPackPreviewSourceTextV1> for UserVoiceContextPackSourceTextDtoV1 {
    fn from(value: ContextPackPreviewSourceTextV1) -> Self {
        match value {
            ContextPackPreviewSourceTextV1::Observed(value) => Self {
                availability: "observed",
                text: Some(value.text),
                truncated: value.truncated,
            },
            ContextPackPreviewSourceTextV1::Blank => Self {
                availability: "blank",
                text: None,
                truncated: false,
            },
            ContextPackPreviewSourceTextV1::Unavailable => Self {
                availability: "unavailable",
                text: None,
                truncated: false,
            },
        }
    }
}

fn context_pack_readiness_api_value(value: ContextPackReadinessV1) -> &'static str {
    match value {
        ContextPackReadinessV1::Ready => "ready",
        ContextPackReadinessV1::NeedsContext => "needs_context",
    }
}

fn context_pack_omission_message(value: ContextPackOmissionV1) -> &'static str {
    match value {
        ContextPackOmissionV1::DirectEvidenceTruncated => {
            "当前原声的研究表达超过预览上限，已仅保留前 480 个字符。"
        }
        ContextPackOmissionV1::DiscussionExcerptTruncated => {
            "至少一条已采到的相关讨论超过每条 220 个字符，预览仅保留其前段。"
        }
        ContextPackOmissionV1::DiscussionItemLimitReached => {
            "本次最多纳入 3 条已读取的相关讨论；其余已读取内容未放入预览。"
        }
        ContextPackOmissionV1::WorkTitleTruncated => {
            "作品标题超过预览上限，已仅保留前 160 个字符。"
        }
        ContextPackOmissionV1::WorkBodyTruncated => "作品正文超过预览上限，已仅保留前 500 个字符。",
        ContextPackOmissionV1::SourceBackedContextUnavailable => {
            "当前原声尚无正文匹配的来源上下文；这不表示平台没有其他文本。"
        }
    }
}

impl From<CommentResearchContextPackV1> for UserVoiceContextPackResponseV1 {
    fn from(pack: CommentResearchContextPackV1) -> Self {
        let has_source_backed_context = pack.has_source_backed_context;
        let readiness = pack.readiness;
        Self {
            preview_state: "source_backed_read_only",
            readiness: context_pack_readiness_api_value(readiness),
            direct_comment_evidence: pack.direct_comment_evidence.into(),
            discussion_context: UserVoiceContextPackDiscussionDtoV1 {
                availability: if has_source_backed_context {
                    "available"
                } else {
                    "unavailable"
                },
                excerpts: pack
                    .discussion_context
                    .into_iter()
                    .map(|entry| UserVoiceContextPackDiscussionExcerptDtoV1 {
                        text: entry.text.into(),
                        relationship: UserVoiceRelatedDiscussionRelationshipDtoV0 {
                            root_comment: entry.root_comment,
                            parent_comment: entry.parent_comment,
                            reply_to_comment: entry.reply_to_comment,
                        },
                    })
                    .collect(),
            },
            work_context: UserVoiceContextPackWorkDtoV1 {
                availability: if has_source_backed_context {
                    "available"
                } else {
                    "unavailable"
                },
                title: pack.work_title.into(),
                body_text: pack.work_body.into(),
            },
            budget: UserVoiceContextPackBudgetDtoV1 {
                included_characters: pack.included_characters,
                total_character_limit: COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS,
                direct_comment_evidence_character_limit:
                    COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS,
                related_discussion_item_limit:
                    COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT,
                related_discussion_item_character_limit:
                    COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_MAX_CHARS,
                work_title_character_limit: COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_TITLE_MAX_CHARS,
                work_body_character_limit: COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_BODY_MAX_CHARS,
            },
            omissions: pack
                .omissions
                .into_iter()
                .map(context_pack_omission_message)
                .collect(),
            future_execution_note: match readiness {
                ContextPackReadinessV1::NeedsContext => {
                    "当前表达标记为需要上下文。此处只展示已采到的文本；后续实际研究仍必须单独检查上下文是否充足。"
                }
                ContextPackReadinessV1::Ready => {
                    "这是只读内容装配预览，不会执行研究。后续实际研究仍必须单独检查输入是否充足。"
                }
            },
        }
    }
}

fn parse_source_record_index(raw: Option<String>) -> Result<i32, ApiError> {
    let raw = raw.ok_or(ApiError::InvalidRequest {
        message: "record_index is required",
    })?;
    let parsed = raw.parse::<i32>().map_err(|_| ApiError::InvalidRequest {
        message: "record_index must be an integer",
    })?;
    if parsed < 0 {
        return Err(ApiError::InvalidRequest {
            message: "record_index must be zero or greater",
        });
    }
    Ok(parsed)
}

fn parse_i64_parameter(
    raw: Option<String>,
    parameter: &'static str,
    default: i64,
) -> Result<i64, ApiError> {
    match raw {
        Some(value) => value.parse::<i64>().map_err(|_| ApiError::InvalidRequest {
            message: match parameter {
                "limit" => "limit must be an integer",
                "offset" => "offset must be an integer",
                "plan_preview_limit" => "limit must be an integer",
                _ => "invalid integer parameter",
            },
        }),
        None => Ok(default),
    }
}

#[derive(Debug)]
enum ApiError {
    InvalidRequest { message: &'static str },
    CurrentVoiceNotFound,
    ReadFailed,
}

impl ApiError {
    fn from_storage_input_error(error: StorageError) -> Self {
        match error {
            StorageError::BlankWorkspaceId => Self::InvalidRequest {
                message: "workspace_id is required",
            },
            StorageError::InvalidCurrentCommentVoicesLimit => Self::InvalidRequest {
                message: "limit must be between 1 and 100",
            },
            StorageError::InvalidCurrentCommentVoicesOffset => Self::InvalidRequest {
                message: "offset must be zero or greater",
            },
            StorageError::InvalidCommentResearchPlanPreviewLimit => Self::InvalidRequest {
                message: "limit must be between 1 and 100",
            },
            StorageError::InvalidCurrentCommentSourceRecordIndex => Self::InvalidRequest {
                message: "record_index must be zero or greater",
            },
            _ => Self::ReadFailed,
        }
    }

    fn from_storage_read_error(error: StorageError) -> Self {
        match error {
            StorageError::BlankWorkspaceId
            | StorageError::InvalidCurrentCommentVoicesLimit
            | StorageError::InvalidCurrentCommentVoicesOffset
            | StorageError::InvalidCommentResearchPlanPreviewLimit
            | StorageError::InvalidCurrentCommentSourceRecordIndex => {
                Self::from_storage_input_error(error)
            }
            _ => Self::ReadFailed,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::InvalidRequest { message } => {
                (StatusCode::BAD_REQUEST, "invalid_request", message)
            }
            Self::CurrentVoiceNotFound => (
                StatusCode::NOT_FOUND,
                "comment_voice_not_found",
                "source evidence locator does not name a current comment voice",
            ),
            Self::ReadFailed => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "comment_voice_read_failed",
                "comment voices could not be read",
            ),
        };
        (
            status,
            Json(serde_json::json!({
                "error": {
                    "code": code,
                    "message": message,
                }
            })),
        )
            .into_response()
    }
}

/// Exposed for API documentation and request validation parity.
pub const fn user_voices_v0_max_limit() -> i64 {
    CURRENT_COMMENT_VOICES_V0_MAX_LIMIT
}
