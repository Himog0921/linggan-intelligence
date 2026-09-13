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
