//! Comment understanding V2. The comment-research router applies the local origin and cache guard.
use crate::local_web::{LocalWebState, ai_settings};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, post},
};
use linggan_intelligence::ai_foundation::comment_understanding as owner;
use serde::Deserialize;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/preview", post(preview))
        .route("/overview", get(overview))
        .route("/runs", get(runs).post(start))
        .route("/runs/{batch}", get(run))
        .route("/cancel", post(cancel))
        .route("/latest", get(latest))
        .route("/problems", get(problems))
        .route("/problems/{problem}", get(problem))
        .route("/consolidation", post(consolidate))
        .route("/redirects/revoke", post(revoke))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Preview {
    domain_ref: Uuid,
    selections: Vec<owner::Selection>,
    #[serde(default)]
    rerun: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Domain {
    domain_ref: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Comment {
    domain_ref: Uuid,
    work_ref: Uuid,
    source_ref: Uuid,
}

async fn preview(State(state): State<LocalWebState>, Json(body): Json<Preview>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::preview(db, body.domain_ref, &body.selections, body.rerun).await)
}

async fn start(State(state): State<LocalWebState>, Json(body): Json<owner::Start>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::start(db, &body).await)
}

async fn runs(State(state): State<LocalWebState>, Query(q): Query<Domain>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::read_runs(db, q.domain_ref).await)
}

async fn run(
    State(state): State<LocalWebState>,
    Query(q): Query<Domain>,
    Path(batch): Path<Uuid>,
) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::read_run(db, q.domain_ref, batch).await)
}

async fn cancel(State(state): State<LocalWebState>, Json(body): Json<owner::Cancel>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::cancel(db, &body).await)
}

async fn latest(State(state): State<LocalWebState>, Query(q): Query<Comment>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::read_latest(db, q.domain_ref, q.work_ref, q.source_ref).await)
}

async fn problems(State(state): State<LocalWebState>, Query(q): Query<Domain>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::read_problems(db, q.domain_ref).await)
}

async fn problem(
    State(state): State<LocalWebState>,
    Query(q): Query<Domain>,
    Path(problem): Path<Uuid>,
) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::read_problem(db, q.domain_ref, problem).await)
}

async fn consolidate(
    State(state): State<LocalWebState>,
    Json(body): Json<owner::StartConsolidation>,
) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::start_consolidation(db, &body).await)
}

async fn revoke(
    State(state): State<LocalWebState>,
    Json(body): Json<owner::RevokeRedirect>,
) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(owner::revoke_redirect(db, &body).await)
}

async fn overview(State(state): State<LocalWebState>, Query(q): Query<Domain>) -> Response {
    let Some(db) = state.database.database() else {
        return ai_settings::unavailable();
    };
    ai_settings::respond(
        linggan_intelligence::ai_foundation::comment_research_overview::read(db, q.domain_ref)
            .await,
    )
}
