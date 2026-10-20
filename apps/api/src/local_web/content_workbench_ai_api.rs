//! Confirmed requests are queued atomically; only the independent AI service dispatches them.
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::content_workbench_ai as ai;
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops/api/creation/ai/readiness", get(readiness))
        .route("/ops/api/creation/ai/preview", post(preview))
        .route("/ops/api/creation/ai/requests", post(execute))
        .route("/ops/api/creation/ai/requests/{id}", get(status))
        .route("/ops/api/creation/ai/requests/{id}/cancel", post(cancel))
        .route("/ops/api/creation/ai/requests/{id}/adopt", post(adopt))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
fn respond(result: Result<serde_json::Value, ai::AiError>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(ai::AiError::AdmissionDenied(reasons)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":"creation_ai_admission_denied","reasons":reasons})),
        )
            .into_response(),
        Err(e) => super::local_read_json_error(
            match e {
                ai::AiError::Invalid => StatusCode::BAD_REQUEST,
                ai::AiError::Stale | ai::AiError::Conflict => StatusCode::CONFLICT,
                ai::AiError::NotFound => StatusCode::NOT_FOUND,
                ai::AiError::Source => StatusCode::FORBIDDEN,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            },
            e.code(),
        ),
    }
}
async fn readiness(State(s): State<LocalWebState>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(ai::readiness(db).await)
}
async fn preview(State(s): State<LocalWebState>, Json(r): Json<ai::PreviewRequest>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(ai::preview(db, &r).await)
}
async fn execute(State(s): State<LocalWebState>, Json(r): Json<ai::ExecuteRequest>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(ai::execute(db, &r).await)
}
async fn status(State(s): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(ai::status(db, id).await)
}
async fn cancel(State(s): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(ai::cancel(db, id).await)
}
async fn adopt(
    State(s): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<ai::AdoptRequest>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(ai::adopt(db, id, &r).await)
}
