//! Model settings JSON API. The only secret input is decoded here without echoing errors.
//!
//! 页面已迁到 React 应用（`apps/web`，WEB-REACT-001 阶段 B），由 `web_app.rs` 提供；这里只剩读写接口。
use super::LocalWebState;
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{model_secrets::*, model_settings::*};
use serde_json::{Value, json};
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/model-settings", get(retired))
        .route(
            "/api/local/model-settings/connections",
            post(save_connection),
        )
        .route(
            "/api/local/model-settings/connections/state",
            post(connection_state),
        )
        .route("/api/local/model-settings/models", post(save_model))
        .route("/api/local/model-settings/probes", post(retired))
        .route("/api/local/model-settings/config", post(retired))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
async fn retired() -> Response {
    (
        StatusCode::GONE,
        Json(json!({"error":"ai_legacy_endpoint_retired",
        "settingsPath":"/settings/models","replacementApi":"/api/local/ai-settings"})),
    )
        .into_response()
}
async fn save_connection(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<SaveModelConnection>(&body) else {
        return respond(Err(ModelError::Invalid));
    };
    respond(save_model_connection(db, model_secret_store().as_ref(), &request).await)
}
async fn connection_state(
    State(state): State<LocalWebState>,
    Json(r): Json<SetModelConnectionEnabled>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(set_model_connection_enabled(db, &r).await)
}
async fn save_model(State(state): State<LocalWebState>, Json(r): Json<SaveModelEntry>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(save_model_entry(db, &r).await)
}
fn respond(result: Result<Value, ModelError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) => {
            let status = match e {
                ModelError::Invalid | ModelError::InputLimit => StatusCode::BAD_REQUEST,
                ModelError::Conflict => StatusCode::CONFLICT,
                ModelError::NotFound => StatusCode::NOT_FOUND,
                ModelError::Disabled | ModelError::NotQualified | ModelError::Budget => {
                    StatusCode::CONFLICT
                }
                _ => StatusCode::SERVICE_UNAVAILABLE,
            };
            (status, Json(json!({"error":e.code()}))).into_response()
        }
    }
}
fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error":"model_database_unavailable"})),
    )
        .into_response()
}
