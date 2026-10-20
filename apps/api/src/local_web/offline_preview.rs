//! Synthetic, stateless calculations. This router has no database state or model adapter.
use axum::{
    Json, Router,
    body::Bytes,
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_intelligence::note_interpretation_offline::fixture::{self, PreviewRequest, Scenario};
use serde_json::json;

pub(super) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/local/ai-offline-preview", get(read).post(recompute))
        .layer(DefaultBodyLimit::max(16384))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

async fn read() -> Response {
    match fixture::preview(&PreviewRequest {
        scenario: Scenario::Baseline,
        judgments: vec![],
    }) {
        Ok(value) => Json(value).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"code":"offline_fixture_invalid"})),
        )
            .into_response(),
    }
}

async fn recompute(body: Bytes) -> Response {
    let Ok(request) = serde_json::from_slice::<PreviewRequest>(&body) else {
        return invalid("offline_request_invalid");
    };
    match fixture::preview(&request) {
        Ok(value) => Json(value).into_response(),
        Err(error) => invalid(error.0),
    }
}

fn invalid(code: &'static str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({"code":code}))).into_response()
}
