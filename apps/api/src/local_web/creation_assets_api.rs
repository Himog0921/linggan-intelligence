//! CREATION-ASSETS-001 S1a local HTTP composition: account identity, product fact cards, the
//! bridge sentence library, manuscript checks and the copy receipt. Rules and storage belong to
//! `linggan_intelligence::creation_assets`; this file only maps requests and errors.
use super::*;
use axum::{extract::DefaultBodyLimit, http::StatusCode, routing::put};
use linggan_intelligence::content_workbench::{BridgeGroup, WorkDocument};
use linggan_intelligence::creation_assets::{self as assets, AssetsError, types::*};
use serde::Deserialize;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops/api/creation/accounts", get(accounts))
        .route(
            "/ops/api/creation/accounts/{id}/identity",
            put(put_identity),
        )
        .route(
            "/ops/api/creation/accounts/{id}/candidates",
            get(candidates),
        )
        .route(
            "/ops/api/creation/fact-cards",
            get(list_fact_cards).post(create_fact_card),
        )
        .route("/ops/api/creation/fact-cards/{code}", put(put_fact_card))
        .route(
            "/ops/api/creation/fact-cards/{code}/confirm",
            post(confirm_fact_card),
        )
        .route(
            "/ops/api/creation/bridge-sentences",
            get(list_sentences).post(create_sentence),
        )
        .route(
            "/ops/api/creation/bridge-sentences/check",
            post(check_sentence),
        )
        .route(
            "/ops/api/creation/bridge-sentences/{code}",
            put(edit_sentence),
        )
        .route(
            "/ops/api/creation/bridge-sentences/{code}/review",
            post(review_sentence),
        )
        .route("/ops/api/creation/checks", post(check_draft))
        .route(
            "/ops/api/creation/persona-facts",
            get(list_persona_facts).post(put_persona_fact),
        )
        .route(
            "/ops/api/creation/persona-facts/{id}/history",
            get(persona_fact_history),
        )
        .route(
            "/ops/api/creation/persona-requests/{id}",
            get(persona_request_status).post(cancel_persona_request),
        )
        .route(
            "/ops/api/creation/persona-facts/{id}/confirm",
            post(confirm_persona_fact),
        )
        .route(
            "/ops/api/creation/persona-facts/{id}/withdraw",
            post(withdraw_persona_fact),
        )
        .route(
            "/ops/api/creation/experiences",
            get(list_experiences).post(create_experience),
        )
        .route(
            "/ops/api/creation/experience-requests/{id}",
            get(experience_request_status).post(cancel_experience_request),
        )
        .route(
            "/ops/api/creation/experiences/{id}",
            get(read_experience).put(edit_experience),
        )
        .route(
            "/ops/api/creation/experiences/{id}/confirm",
            post(confirm_experience),
        )
        .route(
            "/ops/api/creation/experiences/{id}/withdraw",
            post(withdraw_experience),
        )
        .route("/ops/api/creation/works/{id}/copy", post(prepare_copy))
        .route(
            "/ops/api/creation/works/{id}/copy/{receipt}/confirm",
            post(confirm_copy),
        )
        .route("/ops/api/creation/works/{id}/copy-status", get(copy_status))
        .layer(DefaultBodyLimit::max(3 * 1024 * 1024))
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

macro_rules! database {
    ($state:ident) => {
        match $state.database.database() {
            Some(db) => db,
            None => {
                return local_read_json_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "creation_not_connected",
                )
            }
        }
    };
}

fn reply<T: serde::Serialize>(result: Result<T, AssetsError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => failure(error),
    }
}

fn failure(error: AssetsError) -> Response {
    let (status, code) = match error {
        AssetsError::Invalid("person_change_required") => {
            (StatusCode::BAD_REQUEST, "creation_person_change_required")
        }
        AssetsError::Invalid("persona_conflict_decision") => (
            StatusCode::BAD_REQUEST,
            "creation_persona_conflict_decision",
        ),
        AssetsError::Invalid(_) => (StatusCode::BAD_REQUEST, "creation_assets_parameter_invalid"),
        AssetsError::NotFound => (StatusCode::NOT_FOUND, "creation_assets_not_found"),
        AssetsError::RequestConflict => (StatusCode::CONFLICT, "creation_assets_request_conflict"),
        AssetsError::Stale => (StatusCode::CONFLICT, "creation_assets_stale"),
        AssetsError::SourceUnavailable => {
            (StatusCode::CONFLICT, "creation_assets_source_unavailable")
        }
        AssetsError::Blocked(findings) => {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "operation": "local_read", "outcome": "unavailable",
                    "code": "creation_assets_blocked", "findings": findings,
                })),
            )
                .into_response();
        }
        AssetsError::Workbench(_) | AssetsError::Database(_) | AssetsError::Json(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "creation_assets_unavailable",
        ),
    };
    local_read_json_error(status, code)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountsQuery {
    domain_ref: Option<Uuid>,
}
async fn accounts(State(state): State<LocalWebState>, Query(q): Query<AccountsQuery>) -> Response {
    match assets::list_accounts(database!(state), q.domain_ref).await {
        Ok(items) => Json(json!({"items": items})).into_response(),
        Err(error) => failure(error),
    }
}
async fn put_identity(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<IdentityCommand>,
) -> Response {
    reply(assets::put_identity(database!(state), id, &command).await)
}
async fn candidates(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    reply(assets::candidates_for_account(database!(state), id).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersonaFactsQuery {
    account_ref: Uuid,
}
async fn list_persona_facts(
    State(state): State<LocalWebState>,
    Query(q): Query<PersonaFactsQuery>,
) -> Response {
    match assets::list_persona_facts(database!(state), q.account_ref).await {
        Ok(items) => Json(json!({"items":items})).into_response(),
        Err(error) => failure(error),
    }
}
async fn put_persona_fact(
    State(state): State<LocalWebState>,
    Json(command): Json<PersonaFactPut>,
) -> Response {
    reply(assets::put_persona_fact(database!(state), &command).await)
}
async fn persona_fact_history(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
) -> Response {
    match assets::persona_fact_history(database!(state), id).await {
        Ok(items) => Json(json!({"items":items})).into_response(),
        Err(error) => failure(error),
    }
}
async fn persona_request_status(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
) -> Response {
    match assets::persona_request_status(database!(state), id).await {
        Ok(result) => Json(json!({"result":result})).into_response(),
        Err(error) => failure(error),
    }
}
async fn cancel_persona_request(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
) -> Response {
    reply(assets::cancel_persona_request(database!(state), id).await)
}
async fn confirm_persona_fact(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<PersonaFactConfirm>,
) -> Response {
    reply(assets::confirm_persona_fact(database!(state), id, &command).await)
}
async fn withdraw_persona_fact(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<WithdrawCommand>,
) -> Response {
    reply(assets::withdraw_persona_fact(database!(state), id, command.request_id).await)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExperiencesQuery {
    account_ref: Uuid,
}
async fn list_experiences(
    State(state): State<LocalWebState>,
    Query(q): Query<ExperiencesQuery>,
) -> Response {
    match assets::list_experiences(database!(state), q.account_ref).await {
        Ok(items) => Json(json!({"items": items})).into_response(),
        Err(error) => failure(error),
    }
}
async fn create_experience(
    State(state): State<LocalWebState>,
    Json(command): Json<ExperienceCreate>,
) -> Response {
    reply(assets::create_experience(database!(state), &command).await)
}
async fn experience_request_status(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
) -> Response {
    match assets::experience_request_status(database!(state), id).await {
        Ok(value) => Json(json!({"result": value})).into_response(),
        Err(error) => failure(error),
    }
}
async fn cancel_experience_request(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
) -> Response {
    reply(assets::cancel_experience_request(database!(state), id).await)
}
async fn read_experience(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    reply(assets::read_experience(database!(state), id).await)
}
async fn edit_experience(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<ExperienceEdit>,
) -> Response {
    reply(assets::edit_experience(database!(state), id, &command).await)
}
async fn confirm_experience(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<ExperienceConfirm>,
) -> Response {
    reply(assets::confirm_experience(database!(state), id, &command).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WithdrawCommand {
    request_id: Uuid,
}
async fn withdraw_experience(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<WithdrawCommand>,
) -> Response {
    reply(assets::withdraw_experience(database!(state), id, command.request_id).await)
}

async fn list_fact_cards(State(state): State<LocalWebState>) -> Response {
    match assets::list_fact_cards(database!(state)).await {
        Ok(items) => Json(json!({"items": items})).into_response(),
        Err(error) => failure(error),
    }
}
async fn create_fact_card(
    State(state): State<LocalWebState>,
    Json(command): Json<FactCardCommand>,
) -> Response {
    reply(assets::put_fact_card(database!(state), None, &command).await)
}
async fn put_fact_card(
    State(state): State<LocalWebState>,
    Path(code): Path<String>,
    Json(command): Json<FactCardCommand>,
) -> Response {
    reply(assets::put_fact_card(database!(state), Some(&code), &command).await)
}
async fn confirm_fact_card(
    State(state): State<LocalWebState>,
    Path(code): Path<String>,
    Json(command): Json<ConfirmCommand>,
) -> Response {
    reply(assets::confirm_fact_card(database!(state), &code, &command).await)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SentencesQuery {
    group: Option<BridgeGroup>,
    status: Option<SentenceStatus>,
}
async fn list_sentences(
    State(state): State<LocalWebState>,
    Query(q): Query<SentencesQuery>,
) -> Response {
    match assets::list_sentences(database!(state), q.group, q.status).await {
        Ok(items) => Json(json!({"items": items})).into_response(),
        Err(error) => failure(error),
    }
}
async fn create_sentence(
    State(state): State<LocalWebState>,
    Json(command): Json<SentenceCreate>,
) -> Response {
    reply(assets::create_sentence(database!(state), &command).await)
}
async fn edit_sentence(
    State(state): State<LocalWebState>,
    Path(code): Path<String>,
    Json(command): Json<SentenceEdit>,
) -> Response {
    reply(assets::edit_sentence(database!(state), &code, &command).await)
}
async fn review_sentence(
    State(state): State<LocalWebState>,
    Path(code): Path<String>,
    Json(command): Json<SentenceReview>,
) -> Response {
    reply(assets::review_sentence(database!(state), &code, &command).await)
}
async fn check_sentence(
    State(state): State<LocalWebState>,
    Json(request): Json<SentenceCheckRequest>,
) -> Response {
    reply(assets::check_sentence_text(database!(state), &request).await)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftCheck {
    document: WorkDocument,
    work_ref: Option<Uuid>,
}
async fn check_draft(
    State(state): State<LocalWebState>,
    Json(request): Json<DraftCheck>,
) -> Response {
    reply(assets::check_draft_for_work(database!(state), &request.document, request.work_ref).await)
}
async fn prepare_copy(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(command): Json<CopyCommand>,
) -> Response {
    reply(assets::prepare_copy(database!(state), id, &command).await)
}
async fn confirm_copy(
    State(state): State<LocalWebState>,
    Path((id, receipt)): Path<(Uuid, Uuid)>,
    Json(command): Json<CopyConfirm>,
) -> Response {
    reply(assets::confirm_copy(database!(state), id, receipt, &command).await)
}
async fn copy_status(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    reply(assets::copy_status(database!(state), id).await)
}
