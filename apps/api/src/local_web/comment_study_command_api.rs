//! Explicit comment research commands and source-catalog post-usage annotations.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Path, Query, RawQuery, Request, State, rejection::JsonRejection},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::comment_study_catalog::{
    PostUsageCommand, PostUsageError, record_comment_post_usage,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub(super) fn additional_routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/comment-research/overview",
            get(research_overview),
        )
        .route("/api/local/comment-research/preview", get(research_preview))
        .route("/api/local/comment-research/latest", get(research_latest))
        .route("/api/local/comment-research/runs", get(research_runs))
        .route(
            "/api/local/comment-research/problems",
            get(research_problems),
        )
        .route("/api/local/comment-research/cancel", post(research_cancel))
        .route("/api/local/comment-research", post(research_start))
        .route("/api/local/comment-research/{request}", get(research_read))
        .route("/api/local/comment-study/runs", post(retired))
        .route("/api/local/comment-study/policy", post(retired))
        .route(
            "/api/local/comment-study/comments/post-usage",
            post(post_usage),
        )
        .layer(middleware::from_fn(command_guard))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResearchPreview {
    domain_ref: Uuid,
    work_ref: Uuid,
    source_ref: Uuid,
    observation_role: linggan_intelligence::comment_study_source::StudySourceRole,
}
async fn research_preview(
    State(state): State<LocalWebState>,
    Query(query): Query<ResearchPreview>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research::preview(
            db,
            query.domain_ref,
            query.work_ref,
            query.source_ref,
            query.observation_role,
        )
        .await,
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResearchLatest {
    domain_ref: Uuid,
    work_ref: Uuid,
    source_ref: Uuid,
}
async fn research_latest(
    State(state): State<LocalWebState>,
    Query(query): Query<ResearchLatest>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research::latest(
            db,
            query.domain_ref,
            query.work_ref,
            query.source_ref,
        )
        .await,
    )
}
async fn research_start(
    State(state): State<LocalWebState>,
    Json(body): Json<linggan_intelligence::ai_foundation::comment_research::Start>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research::start(db, &body).await,
    )
}
async fn research_read(State(state): State<LocalWebState>, Path(request): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research::read(db, request).await,
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResearchDomain {
    domain_ref: Uuid,
}
async fn research_overview(
    State(state): State<LocalWebState>,
    Query(q): Query<ResearchDomain>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research_overview::read(db, q.domain_ref)
            .await,
    )
}
async fn research_runs(
    State(state): State<LocalWebState>,
    Query(q): Query<ResearchDomain>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research::runs(db, q.domain_ref).await,
    )
}
async fn research_problems(
    State(state): State<LocalWebState>,
    Query(q): Query<ResearchDomain>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_problem::read_problems(db, q.domain_ref).await,
    )
}
async fn research_cancel(
    State(state): State<LocalWebState>,
    Json(q): Json<linggan_intelligence::ai_foundation::comment_research::Cancel>,
) -> Response {
    let Some(db) = state.database.database() else {
        return crate::local_web::ai_settings::unavailable();
    };
    crate::local_web::ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research::cancel(db, &q).await,
    )
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

async fn retired() -> Response {
    crate::local_web::ai_settings::legacy_closed("comment_study")
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
