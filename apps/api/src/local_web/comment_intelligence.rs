//! HTTP composition for a single, domain-scoped comment research contract.
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{comment_intelligence as ci, model_settings::ModelError};
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "comment_research_settings.rs"]
mod settings;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .merge(settings::routes())
        .route("/api/local/comment-intelligence", get(read))
        .route(
            "/api/local/comment-intelligence/sources/{source}",
            get(source),
        )
        .route(
            "/api/local/comment-intelligence/problems/{problem}",
            get(problem),
        )
        .route("/api/local/comment-intelligence/actions", post(action))
        .route("/api/local/comment-intelligence/prepare", post(prepare))
        .route("/api/local/comment-intelligence/run", post(run))
        .route("/assets/comment-intelligence.js", get(script))
        .route("/assets/comment-intelligence.css", get(styles))
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("comment_intelligence.js"),
    )
}
async fn styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("comment_intelligence.css"),
    )
}
fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({"error":"comment_intelligence_schema_missing"})),
    )
        .into_response()
}
fn response(result: Result<Value, ModelError>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let (status, code) = match e {
                ModelError::Invalid => (StatusCode::BAD_REQUEST, "invalid_query"),
                ModelError::SelectionLimit => (StatusCode::BAD_REQUEST, "research_selection_limit"),
                ModelError::Conflict => (StatusCode::CONFLICT, "revision_conflict"),
                ModelError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
                ModelError::Source => (StatusCode::NOT_FOUND, "source_unavailable"),
                _ => (StatusCode::SERVICE_UNAVAILABLE, e.code()),
            };
            (status, Json(json!({"error":code}))).into_response()
        }
    }
}
async fn read(State(s): State<LocalWebState>, Query(q): Query<ci::ResearchScope>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if !ci::schema_ready(db).await.unwrap_or(false) {
        return unavailable();
    }
    response(ci::read(db, &q).await)
}
async fn source(
    State(s): State<LocalWebState>,
    Path(source): Path<Uuid>,
    Query(q): Query<ci::ResearchScope>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    if q.domain.is_none() {
        return response(Err(ModelError::Invalid));
    };
    response(ci::source_with_scope(db, &q, source).await)
}
fn storage_response(r: Result<Value, linggan_storage_postgres::StorageError>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let (status, code) = match &e {
                linggan_storage_postgres::StorageError::Statement(sqlx::Error::Protocol(code)) => {
                    if code.contains("CONFLICT") {
                        (StatusCode::CONFLICT, "revision_conflict")
                    } else if code.contains("SOURCE") || code.contains("NOT_FOUND") {
                        (StatusCode::NOT_FOUND, "source_unavailable")
                    } else {
                        (StatusCode::BAD_REQUEST, "invalid_command")
                    }
                }
                _ => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "comment_research_unavailable",
                ),
            };
            let _ = msg;
            (status, Json(json!({"error":code}))).into_response()
        }
    }
}
async fn action(State(s): State<LocalWebState>, Json(r): Json<Value>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    storage_response(
        linggan_intelligence::comment_intelligence_actions::execute_action(db, r).await,
    )
}
async fn problem(
    State(s): State<LocalWebState>,
    Path(problem): Path<Uuid>,
    Query(mut q): Query<ci::ResearchScope>,
) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    let Some(domain) = q.domain else {
        return response(Err(ModelError::Invalid));
    };
    let details =
        linggan_intelligence::comment_intelligence_problems::problem_details(db, domain, problem)
            .await;
    let details = match details {
        Ok(v) => v,
        Err(e) => return storage_response(Err(e)),
    };
    q.problem_ref = details["resolvedProblemRef"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .or(Some(problem));
    let mut data = match ci::read(db, &q).await {
        Ok(v) => v,
        Err(e) => return response(Err(e)),
    };
    if let Some(obj) = details.as_object() {
        for (k, v) in obj {
            if !matches!(k.as_str(), "members" | "sourceDistribution") {
                data[k] = v.clone();
            }
        }
    }
    Json(data).into_response()
}
async fn prepare(State(s): State<LocalWebState>, Json(r): Json<ci::Prepare>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(ci::prepare(db, &r).await)
}
async fn run(State(s): State<LocalWebState>, Json(r): Json<ci::Run>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    response(ci::run(db, &r).await)
}

#[cfg(test)]
mod tests {
    use super::super::app;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    #[tokio::test]
    async fn no_database_returns_unavailability_instead_of_synthetic_counts() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/local/comment-intelligence")
                    .header("host", "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"], "comment_intelligence_schema_missing");
        assert!(value.get("summary").is_none());
    }
    #[tokio::test]
    async fn v1_tab_read_routes_never_substitute_empty_research_for_a_missing_schema() {
        for path in [
            "/api/local/comment-research/v1/overview",
            "/api/local/comment-research/v1/voices",
            "/api/local/comment-research/v1/problems",
            "/api/local/comment-research/v1/changes",
            "/api/local/comment-research/v1/runs",
        ] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header("host", "127.0.0.1:3000")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                value["error"], "comment_research_v1_schema_missing",
                "{path}"
            );
            assert!(value.get("page").is_none(), "{path}");
        }
    }
    #[tokio::test]
    async fn new_mutation_routes_inherit_origin_and_host_guards() {
        for path in [
            "/api/local/comment-intelligence/actions",
            "/api/local/comment-intelligence/prepare",
            "/api/local/comment-intelligence/run",
            "/api/local/comment-research/policy",
            "/api/local/comment-research/runs",
        ] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .header("host", "127.0.0.1:3000")
                        .header("origin", "https://untrusted.invalid")
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
        }
    }
    #[tokio::test]
    async fn malformed_scope_is_rejected_before_a_database_read() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/local/comment-intelligence?domain=not-a-uuid")
                    .header("host", "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
