//! Topic Map HTTP composition. Canonical reads and durable commands remain in intelligence.
use super::{LocalWebState, shell};
use axum::{
    Json, Router,
    extract::{
        Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::topic_map::{TopicMapCommand, TopicMapQuery};
use linggan_intelligence::topic_map_research::{ResearchCommand, ResearchError};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/topics", get(page))
        .route("/assets/topic-map.css", get(stylesheet))
        .route("/assets/topic-map.js", get(script))
        .route("/api/local/topic-map", get(snapshot))
        .route("/api/local/topic-map/alternative", get(saved_alternative))
        .route("/api/local/topic-map/commands", post(command))
        .route(
            "/api/local/topic-map/structure/preview",
            post(structure_preview),
        )
        .route(
            "/api/local/topic-map/structure/apply",
            post(structure_apply),
        )
        .route("/api/local/topic-map/research", get(research_progress))
        .route(
            "/api/local/topic-map/research/commands",
            post(research_command),
        )
        .route(
            "/api/local/topic-map/collection/commands",
            post(collection_command),
        )
        .route("/api/local/topic-map/search", get(search_progress))
        .route("/api/local/topic-map/search/commands", post(search_command))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
async fn page() -> Html<String> {
    let header = shell::global_header(
        shell::PrimarySurface::Topic,
        "LOCAL HOST / NO PLATFORM ACCESS",
        "主题图谱 <span class=\"v7-slash\">/</span> <b>领域概览</b>",
        "<span>样本范围</span><span>研究依据</span>",
        None,
    );
    Html(format!(
        r#"<!doctype html><html lang="zh-CN" data-theme="linggan-intelligence"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>主题图谱 · Linggan Intelligence</title><link rel="stylesheet" href="/assets/topic-map.css"><script defer src="/assets/topic-map.js"></script></head><body class="topic-map-page"><div class="v7-app topic-map-app">{header}<div id="topic-map-root" aria-label="主题图谱"><p role="status">正在读取领域与主题图谱…</p></div><noscript>主题图谱需要启用 JavaScript。已有作品可以在语料入口查看。</noscript></div></body></html>"#
    ))
}
async fn stylesheet() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            "{}\n{}\n{}",
            super::LIDS_TOKENS,
            super::SHELL_CSS,
            include_str!("topic_map.css")
        ),
    )
        .into_response()
}
async fn script() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("topic_map.js"),
    )
        .into_response()
}
fn database(state: &LocalWebState) -> Result<&linggan_storage_postgres::Database, Response> {
    state
        .database
        .database()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "topic_map_not_connected"))
}
fn error(status: StatusCode, code: &str) -> Response {
    (status,Json(json!({"operation":"topic_map","outcome":if status==StatusCode::CONFLICT {"conflict"} else if status.is_client_error() {"rejected"} else {"unavailable"},"code":code}))).into_response()
}
async fn snapshot(
    State(state): State<LocalWebState>,
    query: Result<Query<TopicMapQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_map_query");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map::read_topic_map(db, &query).await {
        Ok(value) => Json(value).into_response(),
        Err(err) => map_error(err),
    }
}
async fn command(
    State(state): State<LocalWebState>,
    body: Result<Json<TopicMapCommand>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_map_command");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map::save_topic_map_command(db, &body).await {
        Ok(value) => Json(value).into_response(),
        Err(err) => map_error(err),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedAlternativeQuery {
    domain_ref: Uuid,
    alternative_ref: Uuid,
}
async fn saved_alternative(
    State(state): State<LocalWebState>,
    query: Result<Query<SavedAlternativeQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_topic_map_alternative_query",
        );
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map::read_saved_alternative(
        db,
        query.domain_ref,
        query.alternative_ref,
    )
    .await
    {
        Ok(value) => Json(value).into_response(),
        Err(err) => map_error(err),
    }
}
fn map_error(err: linggan_intelligence::topic_map::TopicMapError) -> Response {
    let (status, code) = match err {
        linggan_intelligence::topic_map::TopicMapError::Invalid(code) => {
            (StatusCode::BAD_REQUEST, code)
        }
        linggan_intelligence::topic_map::TopicMapError::NotFound => {
            (StatusCode::NOT_FOUND, "topic_map_resource_not_found")
        }
        linggan_intelligence::topic_map::TopicMapError::Conflict => {
            (StatusCode::CONFLICT, "topic_map_version_conflict")
        }
        linggan_intelligence::topic_map::TopicMapError::Source(_)
        | linggan_intelligence::topic_map::TopicMapError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "topic_map_read_unavailable",
        ),
    };
    error(status, code)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResearchQuery {
    domain_ref: Uuid,
}
async fn research_progress(
    State(state): State<LocalWebState>,
    query: Result<Query<ResearchQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_map_query");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_research::read_research_progress(db, query.domain_ref)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(err) => research_error(err),
    }
}
async fn research_command(
    State(state): State<LocalWebState>,
    body: Result<Json<ResearchCommand>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_research_command");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_research::apply_research_command(db, &body).await {
        Ok(value) => Json(value).into_response(),
        Err(err) => research_error(err),
    }
}
fn research_error(err: ResearchError) -> Response {
    let status = match &err {
        ResearchError::Database(_) => StatusCode::SERVICE_UNAVAILABLE,
        ResearchError::NotFound => StatusCode::NOT_FOUND,
        ResearchError::Conflict => StatusCode::CONFLICT,
        ResearchError::Invalid(_) => StatusCode::BAD_REQUEST,
    };
    error(status, err.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::Value;
    use tower::ServiceExt;
    #[tokio::test]
    async fn topic_map_entry_is_a_real_page_and_rejects_cross_site_access() {
        let response = super::super::app()
            .oneshot(
                Request::builder()
                    .uri("/topics")
                    .header(header::HOST, "localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(html.contains("topic-map-root"));
        assert!(!html.contains("模拟新一轮资料"));
        let response = super::super::app()
            .oneshot(
                Request::builder()
                    .uri("/api/local/topic-map")
                    .header(header::HOST, "localhost:3000")
                    .header(header::ORIGIN, "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    #[tokio::test]
    #[ignore = "requires disposable synthetic PostgreSQL proof"]
    async fn topic_map_http_persists_receipt_replays_and_rejects_foreign_origin() {
        let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("disposable proof URL");
        let db = linggan_storage_postgres::testing::isolated_proof_schema(
            &url,
            "topic_map_http_proof",
            super::super::full_schema_fixture::FULL_MIGRATIONS,
        )
        .await
        .unwrap();
        let application = super::super::app_with_database(db.clone());
        let domain = "00000000-0000-4000-8000-000000000001";
        let value = json!({"action":"ownCreator","idempotencyKey":"http-own-proof","domainRef":domain,"platform":"xhs","authorExternalId":"synthetic-http-own","active":true});
        let mut receipts = Vec::new();
        for origin in [
            "https://evil.example",
            "http://localhost:3000",
            "http://localhost:3000",
        ] {
            let response = application
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/local/topic-map/commands")
                        .header(header::HOST, "localhost:3000")
                        .header(header::ORIGIN, origin)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(value.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            if origin.starts_with("https") {
                assert_eq!(response.status(), StatusCode::FORBIDDEN);
                continue;
            }
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            receipts.push(serde_json::from_slice::<Value>(&bytes).unwrap());
        }
        assert_eq!(receipts[0], receipts[1]);
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/local/topic-map?domainRef={domain}"))
                    .header(header::HOST, "localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let snapshot: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(snapshot["works"].as_array().unwrap().len(), 0);
        assert_eq!(
            snapshot["ownCreators"][0]["authorExternalId"],
            "synthetic-http-own"
        );
        assert_eq!(snapshot["statistics"]["workCount"], 0);
        let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(calls, 0);
        let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_work_order")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(orders, 0);
        let response = application
            .oneshot(
                Request::builder()
                    .uri(format!("/api/local/topic-map/search?domainRef={domain}"))
                    .header(header::HOST, "localhost:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    #[tokio::test]
    async fn disconnected_read_and_malformed_command_have_distinct_outcomes() {
        for (method, path, body, status, code) in [
            (
                "GET",
                "/api/local/topic-map",
                "",
                StatusCode::SERVICE_UNAVAILABLE,
                "topic_map_not_connected",
            ),
            (
                "POST",
                "/api/local/topic-map/commands",
                "{",
                StatusCode::BAD_REQUEST,
                "invalid_topic_map_command",
            ),
        ] {
            let response = super::super::app()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header(header::HOST, "localhost:3000")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(body["code"], code);
        }
    }
}

async fn structure_preview(
    State(state): State<LocalWebState>,
    body: Result<Json<linggan_intelligence::topic_map_structure::StructurePlan>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid_structure_plan");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_structure::preview_structure(db, &body).await {
        Ok(value) => Json(value).into_response(),
        Err(err) => map_error(err),
    }
}
async fn structure_apply(
    State(state): State<LocalWebState>,
    body: Result<Json<linggan_intelligence::topic_map_structure::ApplyStructure>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid_structure_plan");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_structure::apply_structure(db, &body).await {
        Ok(value) => Json(value).into_response(),
        Err(err) => map_error(err),
    }
}

async fn collection_command(
    State(state): State<LocalWebState>,
    body: Result<
        Json<linggan_intelligence::topic_map_research_collection::CollectionCommand>,
        JsonRejection,
    >,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_collection_command");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_research_collection::apply_collection_command(db, &body)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(err) => research_error(err),
    }
}

async fn search_progress(
    State(state): State<LocalWebState>,
    query: Result<Query<ResearchQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_map_query");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_collection_search::read_search_progress(
        db,
        query.domain_ref,
    )
    .await
    {
        Ok(value) => Json(value).into_response(),
        Err(err) => research_error(err),
    }
}
async fn search_command(
    State(state): State<LocalWebState>,
    body: Result<
        Json<linggan_intelligence::topic_map_collection_search::SearchCommand>,
        JsonRejection,
    >,
) -> Response {
    let Ok(Json(body)) = body else {
        return error(StatusCode::BAD_REQUEST, "invalid_topic_search_command");
    };
    let db = match database(&state) {
        Ok(db) => db,
        Err(response) => return response,
    };
    match linggan_intelligence::topic_map_collection_search::apply_search_command(db, &body).await {
        Ok(value) => Json(value).into_response(),
        Err(err) => research_error(err),
    }
}
