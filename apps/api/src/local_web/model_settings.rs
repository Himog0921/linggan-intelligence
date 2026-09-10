//! Model settings composition. The only secret input is decoded here without echoing errors.
use super::{LocalWebState, shell};
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    http::{StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{
    model_invocation::*, model_secrets::*, model_settings::*, model_settings_read::*, pi_adapter::*,
};
use serde_json::{Value, json};
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/model-settings/embedding",
            get(read_embedding).post(save_embedding),
        )
        .route(
            "/api/local/model-settings/embedding/probe",
            post(probe_embedding),
        )
        .route(
            "/settings",
            get(|| async { axum::response::Redirect::temporary("/settings/models") }),
        )
        .route("/settings/models", get(page))
        .route("/assets/model-settings.css", get(styles))
        .route("/assets/model-settings.js", get(script))
        .route("/api/local/model-settings", get(read))
        .route(
            "/api/local/model-settings/connections",
            post(save_connection),
        )
        .route(
            "/api/local/model-settings/connections/state",
            post(connection_state),
        )
        .route("/api/local/model-settings/models", post(save_model))
        .route("/api/local/model-settings/probes", post(probe))
        .route("/api/local/model-settings/config", post(save_config))
        .layer(middleware::from_fn(
            super::comment_research::local_research_guard,
        ))
}
async fn read(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_model_settings(db, model_secret_store().is_synthetic()).await)
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
async fn probe(State(state): State<LocalWebState>, Json(r): Json<ProbeModel>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(
        probe_model(
            db,
            model_secret_store().as_ref(),
            &PiAdapter::configured(),
            &r,
        )
        .await,
    )
}
async fn save_config(
    State(state): State<LocalWebState>,
    Json(r): Json<SaveModelConfig>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(save_model_config(db, &r).await)
}
fn respond(result: Result<Value, ModelError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(e) => {
            let status = match e {
                ModelError::Invalid | ModelError::InputLimit => StatusCode::BAD_REQUEST,
                ModelError::Conflict => StatusCode::CONFLICT,
                ModelError::NotFound => StatusCode::NOT_FOUND,
                ModelError::Disabled
                | ModelError::NotQualified
                | ModelError::EmbeddingNotQualified
                | ModelError::Budget => StatusCode::CONFLICT,
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
async fn page() -> Html<String> {
    Html(include_str!("model_settings.html").replace(
        "{{HEADER}}",
        &shell::global_header(
            shell::PrimarySurface::Settings,
            "模型设置",
            "设置 <span class=\"v7-slash\">/</span> <b>模型与 AI</b>",
            "<span>本机工作空间</span>",
            None,
        ),
    ))
}
async fn styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            "{}\n{}\n{}",
            super::LIDS_TOKENS,
            super::SHELL_CSS,
            include_str!("model_settings.css")
        ),
    )
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("model_settings.js"),
    )
}

async fn read_embedding(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(linggan_intelligence::embedding_settings::read(db).await)
}
async fn save_embedding(
    State(state): State<LocalWebState>,
    Json(r): Json<linggan_intelligence::embedding_settings::SaveEmbedding>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(linggan_intelligence::embedding_settings::save(db, &r).await)
}
async fn probe_embedding(
    State(state): State<LocalWebState>,
    Json(r): Json<linggan_intelligence::embedding_settings::ProbeEmbedding>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(
        linggan_intelligence::embedding_settings::probe(
            db,
            model_secret_store().as_ref(),
            &PiAdapter::configured(),
            &r,
        )
        .await,
    )
}
