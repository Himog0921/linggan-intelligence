//! Productization directory adapters. Mounted below the existing local Host/Origin guard.
//! GET handlers never refresh the cache, create a Run or start a model.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_intelligence::comment_study_catalog::{
    CatalogSummaryQuery, CommentCatalogQuery, CommentDetailQuery, CommentHistoryQuery,
    StudyCatalogError, WorkCatalogQuery, read_catalog_summary, read_comment_catalog,
    read_comment_detail, read_comment_history, read_comment_versions, read_work_catalog,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/comment-study/comments", get(comments))
        .route("/api/local/comment-study/comments/detail", get(detail))
        .route("/api/local/comment-study/comments/history", get(history))
        .route("/api/local/comment-study/comments/versions", get(versions))
        .route("/api/local/comment-study/catalog-summary", get(summary))
        .route("/api/local/comment-study/works", get(works))
}

async fn works(
    State(state): State<LocalWebState>,
    query: Result<Query<WorkCatalogQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid_query(); };
    let Some(database) = database(&state) else { return unavailable(); };
    response(read_work_catalog(database, &query).await)
}

async fn comments(
    State(state): State<LocalWebState>,
    query: Result<Query<CommentCatalogQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid_query(); };
    let Some(database) = database(&state) else { return unavailable(); };
    response(read_comment_catalog(database, &query).await)
}

async fn summary(
    State(state): State<LocalWebState>,
    query: Result<Query<CatalogSummaryQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid_query(); };
    let Some(database) = database(&state) else { return unavailable(); };
    response(read_catalog_summary(database, &query).await)
}

async fn detail(
    State(state): State<LocalWebState>,
    query: Result<Query<CommentDetailQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid_query(); };
    let Some(database) = database(&state) else { return unavailable(); };
    response(read_comment_detail(database, &query).await)
}

async fn history(
    State(state): State<LocalWebState>,
    query: Result<Query<CommentHistoryQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid_query(); };
    let Some(database) = database(&state) else { return unavailable(); };
    response(read_comment_history(database, &query).await)
}

async fn versions(
    State(state): State<LocalWebState>,
    query: Result<Query<CommentHistoryQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid_query(); };
    let Some(database) = database(&state) else { return unavailable(); };
    response(read_comment_versions(database, &query).await)
}

fn database(state: &LocalWebState) -> Option<&Database> {
    match &state.database {
        LocalDatabaseState::Ready(database) => Some(database),
        _ => None,
    }
}

fn invalid_query() -> Response {
    error(StatusCode::BAD_REQUEST, "invalid_request", "评论目录参数不符合约定。", false)
}

fn unavailable() -> Response {
    error(StatusCode::SERVICE_UNAVAILABLE, "catalog_unavailable", "暂时无法读取评论目录。", true)
}

fn response(result: Result<Value, StudyCatalogError>) -> Response {
    let failure = match result {
        Ok(value) => return Json(value).into_response(),
        Err(failure) => failure,
    };
    let (status, code, message, retryable) = match failure {
        StudyCatalogError::ResourceNotFound =>
            (StatusCode::NOT_FOUND, "resource_not_found", "未找到可访问的评论。", false),
        StudyCatalogError::UnsupportedDomain =>
            (StatusCode::BAD_REQUEST, "unsupported_domain", "当前评论研究不支持这个领域。", false),
        StudyCatalogError::InvalidLimit =>
            (StatusCode::BAD_REQUEST, "invalid_limit", "每页数量必须在 1–100 之间。", false),
        StudyCatalogError::InvalidQuery => return invalid_query(),
        StudyCatalogError::InvalidCursor =>
            (StatusCode::BAD_REQUEST, "invalid_cursor", "翻页位置无效，请重新加载列表。", false),
        StudyCatalogError::CursorScopeMismatch =>
            (StatusCode::BAD_REQUEST, "cursor_scope_mismatch", "筛选范围已变化，请从第一页读取。", false),
        StudyCatalogError::InputComparisonUnavailable =>
            (StatusCode::BAD_REQUEST, "invalid_request", "完整输入变化筛选尚未接通；其他目录筛选仍可使用。", false),
        StudyCatalogError::SchemaUnavailable =>
            (StatusCode::SERVICE_UNAVAILABLE, "study_schema_unavailable", "评论目录升级尚未就绪；不会自动建表或重置数据。", false),
        StudyCatalogError::QueryTimeout =>
            (StatusCode::SERVICE_UNAVAILABLE, "query_timeout", "本次查询超时，没有获得完整结果。", true),
        StudyCatalogError::Database(_) | StudyCatalogError::CacheConflict
        | StudyCatalogError::ProjectionInvalid => return unavailable(),
    };
    error(status, code, message, retryable)
}

fn error(status: StatusCode, code: &str, message: &str, retryable: bool) -> Response {
    (status, Json(json!({
        "error": {"code": code, "message": message, "retryable": retryable, "details": {}},
        "requestRef": null
    }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn catalog_errors_preserve_safe_json_and_never_turn_failures_into_empty_items() {
        let response = response(Err(StudyCatalogError::Database(sqlx::Error::RowNotFound)));
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"], "catalog_unavailable");
        assert!(body.get("items").is_none());
        assert_eq!(body["error"]["details"], json!({}));
        assert!(body["requestRef"].is_null());
    }

    #[test]
    fn cursor_and_schema_failures_are_distinct() {
        assert_eq!(response(Err(StudyCatalogError::ResourceNotFound)).status(), StatusCode::NOT_FOUND);
        assert_eq!(response(Err(StudyCatalogError::CursorScopeMismatch)).status(), StatusCode::BAD_REQUEST);
        assert_eq!(response(Err(StudyCatalogError::SchemaUnavailable)).status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
