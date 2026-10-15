//! Local qualified material access for the creation workbench.
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{
    content_workbench::types::{SourceField, SourceKind},
    content_workbench_sources as sources,
};
use serde::Deserialize;
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops/api/creation/sources", get(list))
        .route("/ops/api/creation/sources/validate", post(validate))
        .route("/ops/api/creation/sources/{kind}/{reference}", get(read))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
fn respond(result: Result<serde_json::Value, sources::SourceError>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => super::local_read_json_error(
            match e {
                sources::SourceError::Invalid => StatusCode::BAD_REQUEST,
                sources::SourceError::Changed => StatusCode::CONFLICT,
                sources::SourceError::Restricted => StatusCode::FORBIDDEN,
                sources::SourceError::Unavailable => StatusCode::NOT_FOUND,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            },
            e.code(),
        ),
    }
}
async fn list(State(s): State<LocalWebState>, Query(q): Query<sources::SourceQuery>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::list_sources(db, &q).await)
}
async fn validate(
    State(s): State<LocalWebState>,
    Json(q): Json<sources::AnchorRequest>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::validate_anchor(db, &q).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadQuery {
    domain_ref: Uuid,
    work_ref: Option<Uuid>,
    field: SourceField,
}
async fn read(
    State(s): State<LocalWebState>,
    Path((kind, reference)): Path<(SourceKind, Uuid)>,
    Query(q): Query<ReadQuery>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::read_source(db, q.domain_ref, &kind, reference, q.work_ref, &q.field).await)
}
