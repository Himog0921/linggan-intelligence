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
}
async fn check_draft(
    State(state): State<LocalWebState>,
    Json(request): Json<DraftCheck>,
) -> Response {
    reply(assets::check_draft(database!(state), &request.document).await)
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
