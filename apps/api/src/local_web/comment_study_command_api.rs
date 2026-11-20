//! Explicit comment research commands and source-catalog post-usage annotations.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{RawQuery, Request, State, rejection::JsonRejection},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use linggan_intelligence::comment_study_catalog::{
    PostUsageCommand, PostUsageError, record_comment_post_usage,
};
use serde_json::json;
use uuid::Uuid;

#[path = "comment_understanding_api.rs"]
mod comment_understanding_api;

pub(super) fn additional_routes() -> Router<LocalWebState> {
    Router::new()
        .nest(
            "/api/local/comment-research/v2",
            comment_understanding_api::routes(),
        )
        .route(
            "/api/local/comment-study/comments/post-usage",
            post(post_usage),
        )
        .layer(middleware::from_fn(command_guard))
}

async fn command_guard(request: Request, next: Next) -> Response {
    // Reuse the same origin policy as the outer Comment Study guard.
    let mut response = if super::super::allowed_local_or_public_origin(&request) {
        next.run(request).await
    } else {
        error(
            StatusCode::FORBIDDEN,
            "local_access_denied",
            "不允许从此来源访问本机研究。",
            false,
            None,
        )
    };
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    response
}

/// COMMENT-LIKE-LIBRARY-001 · 记录或撤销「已改成帖子」。页面只按这里的回执更新状态。
async fn post_usage(
    State(state): State<LocalWebState>,
    RawQuery(query): RawQuery,
    body: Result<Json<PostUsageCommand>, JsonRejection>,
) -> Response {
    let invalid_usage = |reference| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "改帖标记参数不符合约定。",
            false,
            reference,
        )
    };
    if query.is_some_and(|q| !q.is_empty()) {
        return invalid_usage(None);
    }
    let Ok(Json(command)) = body else {
        return invalid_usage(None);
    };
    let reference = Some(command.request_ref);
    let LocalDatabaseState::Ready(db) = &state.database else {
        return database_unavailable(&state, reference);
    };
    match record_comment_post_usage(db, &command).await {
        Ok(value) => Json(value).into_response(),
        Err(PostUsageError::InvalidCommand) => invalid_usage(reference),
        Err(PostUsageError::CommentNotFound) => error(
            StatusCode::NOT_FOUND,
            "resource_not_found",
            "这条评论不在当前领域中，没有记录改帖标记。",
            false,
            reference,
        ),
        Err(PostUsageError::IdempotencyConflict) => error(
            StatusCode::CONFLICT,
            "idempotency_conflict",
            "同一请求编号对应了不同的标记操作；请刷新列表后重新操作。",
            false,
            reference,
        ),
        Err(PostUsageError::SchemaUnavailable) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            "study_schema_unavailable",
            "改帖标记所需的数据库结构尚未就绪，标记没有保存。",
            false,
            reference,
        ),
        Err(PostUsageError::Database(error)) => database_failure(
            error
                .as_database_error()
                .and_then(|value| value.code())
                .as_deref(),
            reference,
        ),
    }
}

fn database_unavailable(_state: &LocalWebState, reference: Option<Uuid>) -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "catalog_unavailable",
        "数据库暂不可用。",
        true,
        reference,
    )
}
fn database_failure(_code: Option<&str>, reference: Option<Uuid>) -> Response {
    error(
        StatusCode::SERVICE_UNAVAILABLE,
        "catalog_unavailable",
        "标记未获确认，请读取后核对。",
        true,
        reference,
    )
}
fn error(
    status: StatusCode,
    code: &str,
    message: &str,
    retryable: bool,
    reference: Option<Uuid>,
) -> Response {
    (
        status,
        Json(
            json!({"error":{"code":code,"message":message,"retryable":retryable,"details":{}},
        "requestRef":reference}),
        ),
    )
        .into_response()
}
