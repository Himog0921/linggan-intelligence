use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::ai_foundation::{content_understanding as owner, store::FoundationError};
use serde::Deserialize;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/content-understanding/preview", get(preview))
        .route("/api/local/content-understanding/latest", get(latest))
        .route("/api/local/content-understanding", post(start))
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Selection {
    domain_ref: Uuid,
    work_ref: Uuid,
}
fn unavailable() -> Response {
    super::ai_settings::respond::<serde_json::Value>(Err(FoundationError::ConfigurationIncomplete))
}
async fn preview(State(s): State<LocalWebState>, Query(q): Query<Selection>) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    super::ai_settings::respond(owner::preview(db, q.domain_ref, q.work_ref).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadSelection {
    domain_ref: Uuid,
    work_ref: Uuid,
    as_of: Option<String>,
}
async fn latest(State(s): State<LocalWebState>, Query(q): Query<ReadSelection>) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    super::ai_settings::respond(
        owner::latest(db, q.domain_ref, q.work_ref, q.as_of.as_deref()).await,
    )
}
async fn start(State(s): State<LocalWebState>, Json(q): Json<owner::Start>) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    match owner::start(db, &q).await {
        Ok(value) => (StatusCode::ACCEPTED, Json(value)).into_response(),
        Err(error) => super::ai_settings::respond::<serde_json::Value>(Err(error)),
    }
}
