//! Read-only HTTP projection for the clean comment-study lifecycle.
//!
//! These routes deliberately use a new namespace while the retired surface is still present in
//! the tree.  The final page replacement can therefore be verified against the clean layer
//! before the old command routes are removed; no route here reads or writes V1 relations.

use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::comment_study_read::{
    self as read, CommentStudyReadError, CommentStudyReadQuery,
};
use linggan_intelligence::{
    comment_study_embedding::EmbeddingError,
    comment_study_source::{StudySourceRole, preview_sources_for_roles},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "comment_study_api.rs"]
mod catalog_api;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/comment-study/overview", get(read_overview))
        .route(
            "/api/local/comment-study/runs",
            get(read_runs).post(catalog_api::start),
        )
        .route(
            "/api/local/comment-study/runs/{run_ref}",
            get(read_run_detail),
        )
        .route(
            "/api/local/comment-study/runs/{run_ref}/requests",
            get(read_run_requests),
        )
        .route(
            "/api/local/comment-study/requests/{invocation_ref}",
            get(read_request_detail),
        )
        .route("/api/local/comment-study/targets", get(read_targets))
        .route("/api/local/comment-study/signals", get(read_signals))
        .route(
            "/api/local/comment-study/current-signals",
            get(read_current_signals),
        )
        .route(
            "/api/local/comment-study/comments/related",
            get(read_comment_related),
        )
        .route("/api/local/comment-study/problems", get(read_problems))
        .route(
            "/api/local/comment-study/problem-candidates",
            get(read_deferred_expressions),
        )
        .route(
            "/api/local/comment-study/problems/{problem_ref}",
            get(read_problem_detail),
        )
        .route(
            "/api/local/comment-study/problems/{problem_ref}/evidence",
            get(read_problem_evidence),
        )
        .route("/api/local/comment-study/setup", get(read_setup))
        .route(
            "/api/local/comment-study/policy",
            post(catalog_api::retired),
        )
        .route(
            "/api/local/comment-study/embedding-probe",
            post(run_embedding_probe),
        )
        .merge(catalog_api::command_routes())
        .merge(catalog_api::routes())
        .layer(middleware::from_fn(local_comment_study_guard))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DomainQuery {
    domain: Option<Uuid>,
}

async fn read_setup(
    State(state): State<LocalWebState>,
    Query(query): Query<DomainQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let configs: Result<Vec<Value>, sqlx::Error> = sqlx::query_scalar(
        "SELECT jsonb_build_object( \
           'configRef',config.config_ref,'modelId',model.model_id, \
           'inputTokenLimit',config.input_token_limit,'outputTokenLimit',config.output_token_limit, \
           'enabled',connection.enabled \
         ) \
         FROM linggan_model_config config \
         JOIN linggan_model_entry model USING(model_ref) \
         JOIN linggan_model_connection_version version \
           ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE connection.enabled ORDER BY config.created_at",
    )
    .fetch_all(database.pool())
    .await;
    let Ok(configs) = configs else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let as_of: Result<String, sqlx::Error> = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(database.pool())
        .await;
    let Ok(as_of) = as_of else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let Some(domain_ref) = query.domain else {
        return error(StatusCode::BAD_REQUEST, "domain_required");
    };
    let domain_state: Option<String> =
        sqlx::query_scalar("SELECT status FROM observation_domain WHERE domain_ref=$1")
            .bind(domain_ref)
            .fetch_optional(database.pool())
            .await
            .unwrap_or(None);
    let Some(domain_state) = domain_state else {
        return error(StatusCode::NOT_FOUND, "domain_not_found");
    };
    let Ok(mut previews) = preview_sources_for_roles(
        database,
        domain_ref,
        &as_of,
        &[StudySourceRole::Primary, StudySourceRole::Reference],
    )
    .await
    else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let Some(primary_preview) = previews.remove(&StudySourceRole::Primary) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let Some(reference_preview) = previews.remove(&StudySourceRole::Reference) else {
        return error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable");
    };
    let eligible_works = primary_preview.works.clone();
    let reference_eligible_works = reference_preview.works.clone();
    Json(json!({
        "contract":"comment-study.setup.v2",
        "domainRef":domain_ref,
        "domainStatus":domain_state,
        "modelConfigs":configs,
        "sourcePreview":primary_preview,
        "eligibleWorks":eligible_works,
        "referenceSourcePreview":reference_preview,
        "referenceEligibleWorks":reference_eligible_works
    }))
    .into_response()
}
/// Runs the local qualification probe and, only if it passes, makes its profile the one every
/// comparison happens in. An operator action rather than tick work: it loads the model and states
/// what this machine can do, and nothing entitles the system to assert that on its own.
async fn run_embedding_probe(State(state): State<LocalWebState>) -> Response {
    let database = match database(&state) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match linggan_intelligence::comment_study_embedding::probe_and_register_embedding_profile(
        database,
        &linggan_intelligence::pi_adapter::PiAdapter::configured(),
    )
    .await
    {
        Ok(outcome) => Json(json!(outcome)).into_response(),
        // A runtime failure is distinct from a failure to retain the qualification fact.  Both
        // remain deliberately closed, safe codes rather than exposing an adapter or database
        // error string to the local browser.
        Err(probe_error) => error(
            StatusCode::SERVICE_UNAVAILABLE,
            embedding_probe_error_code(&probe_error),
        ),
    }
}

fn embedding_probe_error_code(error: &EmbeddingError) -> &'static str {
    match error {
        // A runtime failure is distinct from a failure to retain the qualification fact. Both
        // remain deliberately closed, safe codes rather than exposing adapter or database text.
        EmbeddingError::Database(_) => "embedding_probe_storage_unavailable",
        EmbeddingError::Model(_) | EmbeddingError::InvalidVector => "embedding_runtime_unavailable",
    }
}

pub(super) async fn local_comment_study_guard(request: Request, next: Next) -> Response {
    if !allowed_origin(request.headers()) {
        return error(StatusCode::FORBIDDEN, "local_comment_study_origin_rejected");
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    response
}

fn allowed_origin(headers: &HeaderMap) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some((name, port)) = host.rsplit_once(':') else {
        return false;
    };
    if !matches!(name, "127.0.0.1" | "localhost") || port.parse::<u16>().is_err() {
        return false;
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !matches!(value, "same-origin" | "none"))
    {
        return false;
    }
    headers.get(header::ORIGIN).is_none_or(|origin| {
        origin
            .to_str()
            .is_ok_and(|value| value == format!("http://{host}"))
    })
}

async fn read_overview(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_overview(database, &query).await)
}

async fn read_runs(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_runs(database, &query).await)
}

async fn read_run_detail(
    State(state): State<LocalWebState>,
    Path(run_ref): Path<Uuid>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    if query.run_ref.is_some_and(|requested| requested != run_ref) {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_run_detail(database, &query, run_ref).await)
}

async fn read_run_requests(
    State(state): State<LocalWebState>,
    Path(run_ref): Path<Uuid>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    if query.run_ref.is_some_and(|requested| requested != run_ref) {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_run_requests(database, &query, run_ref).await)
}

async fn read_request_detail(
    State(state): State<LocalWebState>,
    Path(invocation_ref): Path<Uuid>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_request_detail(database, &query, invocation_ref).await)
}

async fn read_targets(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_targets(database, &query).await)
}

async fn read_signals(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_signals(database, &query).await)
}

async fn read_current_signals(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_current_signals(database, &query).await)
}

async fn read_comment_related(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_comment_related(database, &query).await)
}

async fn read_problems(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_problems(database, &query).await)
}

async fn read_deferred_expressions(
    State(state): State<LocalWebState>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_deferred_expressions(database, &query).await)
}

async fn read_problem_detail(
    State(state): State<LocalWebState>,
    Path(problem_ref): Path<Uuid>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    if query
        .problem_ref
        .is_some_and(|requested| requested != problem_ref)
    {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_problem_detail(database, &query, problem_ref).await)
}

async fn read_problem_evidence(
    State(state): State<LocalWebState>,
    Path(problem_ref): Path<Uuid>,
    Query(query): Query<CommentStudyReadQuery>,
) -> Response {
    if query
        .problem_ref
        .is_some_and(|requested| requested != problem_ref)
    {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    }
    let database = match database(&state) {
        Ok(database) => database,
        Err(response) => return response,
    };
    read_response(read::read_problem_evidence(database, &query, problem_ref).await)
}

fn database(state: &LocalWebState) -> Result<&linggan_storage_postgres::Database, Response> {
    match &state.database {
        LocalDatabaseState::Ready(database) => Ok(database),
        LocalDatabaseState::NotConfigured
        | LocalDatabaseState::DatabaseUnavailable
        | LocalDatabaseState::SchemaUnavailable(_) => Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "comment_study_unavailable",
        )),
    }
}

fn read_response(result: Result<serde_json::Value, CommentStudyReadError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(CommentStudyReadError::InvalidQuery) => error(StatusCode::BAD_REQUEST, "invalid_query"),
        Err(CommentStudyReadError::InvalidCursor) => {
            error(StatusCode::BAD_REQUEST, "invalid_cursor")
        }
        Err(CommentStudyReadError::CursorScopeMismatch) => {
            error(StatusCode::BAD_REQUEST, "cursor_scope_mismatch")
        }
        Err(CommentStudyReadError::RunUnavailable) => {
            error(StatusCode::NOT_FOUND, "comment_study_run_unavailable")
        }
        Err(CommentStudyReadError::ProblemUnavailable) => {
            error(StatusCode::NOT_FOUND, "comment_study_problem_unavailable")
        }
        Err(CommentStudyReadError::RequestUnavailable) => {
            error(StatusCode::NOT_FOUND, "comment_study_request_unavailable")
        }
        Err(CommentStudyReadError::SchemaUnavailable | CommentStudyReadError::Database(_)) => {
            error(StatusCode::SERVICE_UNAVAILABLE, "comment_study_unavailable")
        }
    }
}

/// Header copy for the domain lifecycle. Values outside the current CHECK set stay verbatim so a
/// new state is never silently relabelled (LIDS LANG-03).
fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_read_routes_reject_cross_site_requests() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "127.0.0.1:3000".parse().unwrap());
        headers.insert(header::ORIGIN, "https://other.example".parse().unwrap());
        assert!(!allowed_origin(&headers));
        headers.insert(header::ORIGIN, "http://127.0.0.1:3000".parse().unwrap());
        assert!(allowed_origin(&headers));
    }

    #[test]
    fn embedding_probe_never_reports_an_invalid_runtime_vector_as_a_storage_failure() {
        assert_eq!(
            embedding_probe_error_code(&EmbeddingError::InvalidVector),
            "embedding_runtime_unavailable"
        );
    }
}
