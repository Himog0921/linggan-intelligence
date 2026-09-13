//! Minimal HTTP composition for the Comment Research User Voices V0 read path.
//!
//! This module exposes only current comment facts proven by the V0 storage
//! proof. It does not create research work, clean text, call a model, invent
//! work metadata, or expose a page implementation.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_storage_postgres::{
    CURRENT_COMMENT_VOICES_V0_MAX_LIMIT, CommentFactStore, CurrentCommentVoiceV0,
    CurrentCommentVoicesPageRequestV0, StorageError,
};
use serde::{Deserialize, Serialize};

/// The bounded default used when callers omit limit.
pub const USER_VOICES_V0_DEFAULT_LIMIT: i64 = 50;

/// Builds the only Comment Research HTTP route enabled by this V0 package.
///
/// The route is read-only by construction: its handler invokes the storage
/// adapter's current-projection SELECT method and has no admission dependency.
pub fn comment_research_router_v0(store: Arc<CommentFactStore>) -> Router {
    Router::new()
        .route("/api/v0/comment-research/voices", get(list_user_voices_v0))
        .with_state(store)
}

#[derive(Debug, Deserialize)]
struct UserVoicesQueryV0 {
    workspace_id: Option<String>,
    limit: Option<String>,
    offset: Option<String>,
}

#[derive(Debug, Serialize)]
struct UserVoicesResponseV0 {
    pagination: UserVoicesPaginationV0,
    voices: Vec<UserVoiceDtoV0>,
}

#[derive(Debug, Serialize)]
struct UserVoicesPaginationV0 {
    total: i64,
    limit: i64,
    offset: i64,
}

/// The intentionally narrow browser DTO.
///
/// work_context is an explicit absence rather than a guessed title, URL,
/// author, or media summary. research_status is likewise a current-system
/// fact: V0 has no analysis layer, so every listed voice is not researched.
#[derive(Debug, Serialize)]
struct UserVoiceDtoV0 {
    text: String,
    source_note_id: String,
    current_admitted_at: String,
    source_evidence: UserVoiceSourceEvidenceDtoV0,
    work_context: &'static str,
    research_status: &'static str,
}

#[derive(Debug, Serialize)]
struct UserVoiceSourceEvidenceDtoV0 {
    evidence_id: String,
    record_index: i32,
}

impl From<CurrentCommentVoiceV0> for UserVoiceDtoV0 {
    fn from(voice: CurrentCommentVoiceV0) -> Self {
        Self {
            text: voice.text,
            source_note_id: voice.source_note_id,
            current_admitted_at: voice.current_admitted_at,
            source_evidence: UserVoiceSourceEvidenceDtoV0 {
                evidence_id: voice.source_evidence.evidence_id.to_string(),
                record_index: voice.source_evidence.record_index,
            },
            work_context: "unavailable",
            research_status: "not_researched",
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
    let page = CurrentCommentVoicesPageRequestV0::new(limit, offset)
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
        voices: result
            .voices
            .into_iter()
            .map(UserVoiceDtoV0::from)
            .collect(),
    }))
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
                _ => "invalid integer parameter",
            },
        }),
        None => Ok(default),
    }
}

#[derive(Debug)]
enum ApiError {
    InvalidRequest { message: &'static str },
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
            _ => Self::ReadFailed,
        }
    }

    fn from_storage_read_error(error: StorageError) -> Self {
        match error {
            StorageError::BlankWorkspaceId
            | StorageError::InvalidCurrentCommentVoicesLimit
            | StorageError::InvalidCurrentCommentVoicesOffset => {
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
