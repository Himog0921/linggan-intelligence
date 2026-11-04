//! Manual note analysis commands; previews and reads never dispatch.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{
    ai_foundation::{note_analysis as owner, store::FoundationError},
    note_analysis::Scope,
};
use serde::Deserialize;
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/note-analysis/preview", get(preview))
        .route("/api/local/note-analysis/latest", get(latest))
        .route("/api/local/note-analysis", post(start))
        .route("/api/local/note-analysis/{request}", get(read))
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Selection {
    domain_ref: Uuid,
    work_ref: Uuid,
    scope: Scope,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Domain {
    domain_ref: Uuid,
}
fn unavailable() -> Response {
    super::ai_settings::respond::<serde_json::Value>(Err(FoundationError::ConfigurationIncomplete))
}
async fn preview(State(s): State<LocalWebState>, Query(q): Query<Selection>) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    super::ai_settings::respond(owner::preview(db, q.domain_ref, q.work_ref, q.scope).await)
}
async fn latest(State(s): State<LocalWebState>, Query(q): Query<Selection>) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    super::ai_settings::respond(owner::latest(db, q.domain_ref, q.work_ref, q.scope).await)
}
async fn read(
    State(s): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Query(q): Query<Domain>,
) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    super::ai_settings::respond(owner::read(db, q.domain_ref, id).await)
}
async fn start(State(s): State<LocalWebState>, Json(q): Json<owner::Start>) -> Response {
    let LocalDatabaseState::Ready(db) = &s.database else {
        return unavailable();
    };
    match owner::start(db, &q).await {
        Ok(v) => (StatusCode::ACCEPTED, Json(v)).into_response(),
        Err(e) => super::ai_settings::respond::<serde_json::Value>(Err(e)),
    }
}
