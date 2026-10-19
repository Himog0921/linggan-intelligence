//! Local settings and explicit queued-work commands. Configuration saves never run a provider.
use super::LocalWebState;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{
    ai_foundation::{
        batch_control::{CancelBatch, cancel_batch, read_batch},
        batch_store::{CreateBatch, CreateTrial, create_batch, create_trial},
        catalog::{CatalogQuery, read_catalog},
        control_store::{Actor, ChangeControl, change_control},
        provider_store::{
            ChangeEgress, SaveCapability, SavePrice, change_egress, save_capability, save_price,
        },
        settings_read::read_settings,
        skill_binding::{ActivateSkill, activate_skill},
        skill_store::{ImportSkill, import_skill},
        store::FoundationError,
        task_config::{SaveTaskConfig, save_task_config},
    },
    model_secrets::model_secret_store,
    pi_adapter::PiAdapter,
};
use serde::Serialize;
use serde_json::json;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/ai-settings", get(read))
        .route("/api/local/ai-settings/invocations", get(invocations))
        .route(
            "/api/local/ai-settings/writing-samples",
            get(writing_samples),
        )
        .route("/api/local/ai-settings/catalog", get(catalog_read))
        .route("/api/local/ai-settings/control", post(control))
        .route("/api/local/ai-settings/egress", post(egress))
        .route("/api/local/ai-settings/capabilities", post(capability))
        .route("/api/local/ai-settings/prices", post(price))
        .route("/api/local/ai-settings/batches/cancel", post(batch_cancel))
        .route("/api/local/ai-settings/batches/{batch}", get(batch_read))
        .route("/api/local/ai-settings/task-configs", post(task_config))
        .route(
            "/api/local/ai-settings/task-configs/{config}",
            get(config_read),
        )
        .route(
            "/api/local/ai-settings/skills/activate",
            post(skill_activate),
        )
        .route(
            "/api/local/ai-settings/trials/{comparison}",
            get(trial_read),
        )
        .layer(DefaultBodyLimit::max(65536))
        .merge(
            Router::new()
                .route("/api/local/ai-settings/batches", post(batch_create))
                .route("/api/local/ai-settings/trials", post(trial_create))
                .route("/api/local/ai-settings/skills/import", post(skill_import))
                .route("/api/local/ai-settings/skills/{version}", get(skill_read))
                .layer(DefaultBodyLimit::max(8 * 1048576)),
        )
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
async fn catalog_read(Query(query): Query<CatalogQuery>) -> Response {
    respond(read_catalog(&PiAdapter::configured(), &query).await)
}
async fn writing_samples(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(linggan_intelligence::ai_foundation::settings_inputs::writing_samples(db).await)
}
async fn read(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_settings(db, model_secret_store().is_synthetic()).await)
}
async fn control(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<ChangeControl>(&body) else {
        return invalid();
    };
    respond(change_control(db, &request, Actor::Human).await)
}
async fn egress(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<ChangeEgress>(&body) else {
        return invalid();
    };
    respond(change_egress(db, &request, Actor::Human).await)
}
async fn capability(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<SaveCapability>(&body) else {
        return invalid();
    };
    respond(save_capability(db, &request).await)
}
async fn price(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<SavePrice>(&body) else {
        return invalid();
    };
    respond(save_price(db, &request).await)
}
async fn skill_import(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<ImportSkill>(&body) else {
        return invalid();
    };
    respond(import_skill(db, &PiAdapter::configured(), &request).await)
}
async fn task_config(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<SaveTaskConfig>(&body) else {
        return invalid();
    };
    respond(save_task_config(db, &request).await)
}
async fn config_read(
    State(state): State<LocalWebState>,
    Path(config): Path<uuid::Uuid>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(
        linggan_intelligence::ai_foundation::settings_read::read_configuration(db, config).await,
    )
}
async fn skill_activate(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<ActivateSkill>(&body) else {
        return invalid();
    };
    respond(activate_skill(db, &request).await)
}
async fn batch_create(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<CreateBatch>(&body) else {
        return invalid();
    };
    respond(create_batch(db, &request).await)
}
async fn batch_cancel(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<CancelBatch>(&body) else {
        return invalid();
    };
    respond(cancel_batch(db, &request).await)
}
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchPosition {
    #[serde(default)]
    from_position: u32,
}
async fn batch_read(
    State(state): State<LocalWebState>,
    Path(batch): Path<uuid::Uuid>,
    Query(query): Query<BatchPosition>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(read_batch(db, batch, query.from_position).await)
}
async fn trial_create(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Ok(request) = serde_json::from_slice::<CreateTrial>(&body) else {
        return invalid();
    };
    respond(create_trial(db, &request).await)
}
#[derive(serde::Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct TrialPosition {
    #[serde(default)]
    position: u32,
}
async fn trial_read(
    State(state): State<LocalWebState>,
    Path(comparison): Path<uuid::Uuid>,
    Query(position): Query<TrialPosition>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(
        linggan_intelligence::ai_foundation::settings_read::read_comparison(
            db,
            comparison,
            position.position,
        )
        .await,
    )
}
async fn skill_read(
    State(state): State<LocalWebState>,
    Path(version): Path<uuid::Uuid>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(
        linggan_intelligence::ai_foundation::settings_read::read_skill_version(db, version).await,
    )
}
fn invalid() -> Response {
    respond::<serde_json::Value>(Err(FoundationError::Invalid))
}

pub(crate) fn legacy_closed(task_key: &'static str) -> Response {
    (
        StatusCode::GONE,
        Json(
            json!({"error":"ai_legacy_execution_retired","taskKey":task_key,
        "settingsPath":format!("/settings/models?section=automation&task={task_key}")}),
        ),
    )
        .into_response()
}
fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error":"ai_database_unavailable"})),
    )
        .into_response()
}
fn respond<T: Serialize>(result: Result<T, FoundationError>) -> Response {
    match result {
        Err(FoundationError::AdmissionDenied(reasons)) => (
            StatusCode::CONFLICT,
            Json(json!({"error":"ai_admission_denied","reasons":reasons})),
        )
            .into_response(),
        Ok(value) => Json(value).into_response(),
        Err(error) => {
            let status = match error {
                FoundationError::Invalid | FoundationError::Rule(_) => StatusCode::BAD_REQUEST,
                FoundationError::NotFound => StatusCode::NOT_FOUND,
                FoundationError::Conflict
                | FoundationError::NotConnected
                | FoundationError::ConfigurationIncomplete
                | FoundationError::SkillTrialRequired
                | FoundationError::AdmissionDenied(_)
                | FoundationError::SystemCannotResume => StatusCode::CONFLICT,
                FoundationError::Database(_) | FoundationError::LoaderUnavailable => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
            };
            (status, Json(json!({"error":error.to_string()}))).into_response()
        }
    }
}

async fn invocations(
    State(state): State<LocalWebState>,
    Query(query): Query<linggan_intelligence::ai_foundation::invocation_read::HistoryQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    respond(linggan_intelligence::ai_foundation::invocation_read::read_history(db, &query).await)
}
