//! Explicit, scoped content-comparison preview, admission and result reads.
//! GET never queues a provider request. POST uses the existing AI executor and
//! command identity, rather than running a model in the HTTP handler.
use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_evidence::target_inspection::Scope;
use linggan_intelligence::ai_foundation::{
    store::FoundationError,
    work_comparison::{self, StartComparison},
};
use serde::Deserialize;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/collection/workspace/target/{target}/inspection/content-comparison/preview",get(preview))
        .route("/api/collection/workspace/target/{target}/inspection/content-comparison/requests",post(start))
        .route("/api/collection/workspace/target/{target}/inspection/content-comparison/requests/{request}",get(read))
        .layer(DefaultBodyLimit::max(16384))
        .layer(middleware::from_fn(super::super::comment_study::local_comment_study_guard))
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Params {
    domain: Option<String>,
    as_of: Option<String>,
    works: Option<String>,
}
impl Params {
    fn scope(&self, target_ref: Uuid) -> Result<Scope, StatusCode> {
        let domain_ref = match self.domain.as_deref() {
            None | Some("all") => None,
            Some(value) => Some(Uuid::parse_str(value).map_err(|_| StatusCode::BAD_REQUEST)?),
        };
        if self
            .as_of
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 64)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        Ok(Scope {
            target_ref,
            domain_ref,
            knowledge_cutoff: self.as_of.clone(),
        })
    }
    fn works(&self) -> Result<Vec<Uuid>, StatusCode> {
        let raw = self.works.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
        if raw.len() > 110 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let ids = raw
            .split(',')
            .map(Uuid::parse_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        if !(2..=3).contains(&ids.len())
            || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
        {
            return Err(StatusCode::BAD_REQUEST);
        }
        Ok(ids)
    }
}
fn unavailable() -> Response {
    local_read_json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "comparison_read_unavailable",
    )
}
fn invalid() -> Response {
    local_read_json_error(StatusCode::BAD_REQUEST, "comparison_invalid_request")
}
fn respond<T: serde::Serialize>(result: Result<T, FoundationError>) -> Response {
    match result {
        Ok(data) => Json(data).into_response(),
        Err(FoundationError::Invalid) => invalid(),
        Err(FoundationError::NotFound) => {
            local_read_json_error(StatusCode::NOT_FOUND, "comparison_not_found")
        }
        Err(FoundationError::Conflict) => {
            local_read_json_error(StatusCode::CONFLICT, "comparison_stale")
        }
        Err(FoundationError::AdmissionDenied(reasons)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":"comparison_not_admitted","reasons":reasons})),
        )
            .into_response(),
        Err(_) => unavailable(),
    }
}
async fn preview(
    State(state): State<LocalWebState>,
    Path(target): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let (scope, works) = match params
        .scope(target)
        .and_then(|scope| params.works().map(|works| (scope, works)))
    {
        Ok(data) => data,
        Err(_) => return invalid(),
    };
    respond(work_comparison::preview(db, &scope, &works).await)
}
async fn start(
    State(state): State<LocalWebState>,
    Path(target): Path<Uuid>,
    Query(params): Query<Params>,
    body: Bytes,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let scope = match params.scope(target) {
        Ok(scope) => scope,
        Err(_) => return invalid(),
    };
    let Ok(request) = serde_json::from_slice::<StartComparison>(&body) else {
        return invalid();
    };
    if request.target_ref != target
        || request.domain_ref != scope.domain_ref
        || request.work_refs.len() < 2
        || request.work_refs.len() > 3
        || request
            .work_refs
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != request.work_refs.len()
        || scope.knowledge_cutoff.as_deref() != Some(request.knowledge_cutoff.as_str())
    {
        return invalid();
    }
    respond(work_comparison::start(db, &request).await)
}
async fn read(
    State(state): State<LocalWebState>,
    Path((target, request)): Path<(Uuid, Uuid)>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let scope = match params.scope(target) {
        Ok(scope) => scope,
        Err(_) => return invalid(),
    };
    respond(work_comparison::read(db, &scope, request).await)
}
