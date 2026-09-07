//! Daily research HTTP composition. All mutations retain the local research origin guard.
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{comment_daily::*, model_settings::ModelError};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/comment-research/daily", get(read))
        .route("/api/local/comment-research/daily/schedule", post(schedule))
        .route("/api/local/comment-research/daily/selected", post(selected))
        .route(
            "/api/local/comment-research/daily/{batch}",
            get(detail).post(action),
        )
        .route(
            "/api/local/comment-research/daily/{batch}/retry",
            post(retry),
        )
        .route("/assets/comment-daily.js", get(script))
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("comment_daily.js"),
    )
}
fn response(result: Result<Value, ModelError>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let status = match e {
                ModelError::Invalid => StatusCode::BAD_REQUEST,
                ModelError::Conflict => StatusCode::CONFLICT,
                ModelError::NotFound => StatusCode::NOT_FOUND,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            };
            (status, Json(json!({"error":e.code()}))).into_response()
        }
    }
}
fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error":"comment_daily_schema_missing"})),
    )
        .into_response()
}
async fn read(State(s): State<LocalWebState>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(overview(db).await)
}
async fn schedule(State(s): State<LocalWebState>, Json(r): Json<DailySchedule>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(save_schedule(db, &r).await)
}
async fn selected(State(s): State<LocalWebState>, Json(r): Json<SelectedBatch>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(create_selected(db, &r).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    after: Option<Uuid>,
}
async fn detail(
    State(s): State<LocalWebState>,
    Path(batch): Path<Uuid>,
    Query(q): Query<Cursor>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(batch_detail(db, batch, q.after).await)
}
async fn action(
    State(s): State<LocalWebState>,
    Path(batch): Path<Uuid>,
    Json(r): Json<BatchAction>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(set_batch_enabled(db, batch, r.enabled).await)
}

async fn retry(
    State(s): State<LocalWebState>,
    Path(batch): Path<Uuid>,
    Json(r): Json<RetryDaily>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(retry_failed(db, batch, r.command_ref).await)
}
