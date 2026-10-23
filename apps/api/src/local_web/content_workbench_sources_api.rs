//! Local qualified material access for the creation workbench.
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::{
    content_workbench::types::{SourceField, SourceKind},
    content_workbench_sources as sources,
};
use serde::Deserialize;
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops/api/creation/sources", get(list))
        .route("/ops/api/creation/sources/prepare", post(prepare))
        .route("/ops/api/creation/sources/documents", get(documents))
        .route("/ops/api/creation/sources/document", get(document))
        .route("/ops/api/creation/sources/resolve", post(resolve))
        .route("/ops/api/creation/sources/validate", post(validate))
        .route("/ops/api/creation/sources/{kind}/{reference}", get(read))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
fn respond(result: Result<serde_json::Value, sources::SourceError>) -> Response {
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => super::local_read_json_error(
            match e {
                sources::SourceError::Invalid => StatusCode::BAD_REQUEST,
                sources::SourceError::Changed => StatusCode::CONFLICT,
                sources::SourceError::Restricted => StatusCode::FORBIDDEN,
                sources::SourceError::Unavailable => StatusCode::NOT_FOUND,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            },
            e.code(),
        ),
    }
}
async fn list(State(s): State<LocalWebState>, Query(q): Query<sources::SourceQuery>) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::list_sources(db, &q).await)
}
async fn validate(
    State(s): State<LocalWebState>,
    Json(q): Json<sources::AnchorRequest>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::validate_anchor(db, &q).await)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadQuery {
    domain_ref: Uuid,
    work_ref: Option<Uuid>,
    field: SourceField,
    block_ref: String,
    source_version: String,
    as_of: String,
}
async fn read(
    State(s): State<LocalWebState>,
    Path((kind, reference)): Path<(SourceKind, Uuid)>,
    Query(q): Query<ReadQuery>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::read_source(db, q.domain_ref, &kind, reference, q.work_ref, &q.field, &q.block_ref, &q.source_version, &q.as_of).await)
}

async fn prepare(
    State(s): State<LocalWebState>,
    Json(q): Json<sources::materials::PrepareRequest>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::materials::prepare(db, &q).await)
}
async fn documents(
    State(s): State<LocalWebState>,
    Query(q): Query<sources::materials::SearchQuery>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::materials::search(db, &q).await)
}
async fn resolve(
    State(s): State<LocalWebState>,
    Json(q): Json<sources::materials::ResolveRequest>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::materials::resolve(db, &q).await)
}

async fn document(
    State(s): State<LocalWebState>,
    Query(q): Query<sources::materials::DocumentQuery>,
) -> Response {
    let Some(db) = s.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    respond(sources::materials::read_document(db, &q).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    fn request(uri: &str, body: Option<serde_json::Value>) -> Request<Body> {
        let mut r = Request::builder().uri(uri).header("Host", "127.0.0.1:4185");
        if body.is_some() {
            r = r.method("POST").header("Content-Type", "application/json");
        }
        r.body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
            .unwrap()
    }
    #[tokio::test]
    async fn content_workbench_material_routes_keep_missing_database_distinct_from_empty() {
        let domain = Uuid::new_v4();
        for (uri, body) in [
            (
                "/ops/api/creation/sources/prepare".to_owned(),
                Some(serde_json::json!({"domainRef":domain,"title":"作业启动","angle":"第一步"})),
            ),
            (
                "/ops/api/creation/sources/resolve".to_owned(),
                Some(serde_json::json!({"domainRef":domain,"selections":[]})),
            ),
            (
                format!("/ops/api/creation/sources/documents?domainRef={domain}&query=homework"),
                None,
            ),
            (
                format!(
                    "/ops/api/creation/sources/document?domainRef={domain}&sourceKind=note&sourceRef={domain}&workRef={domain}"
                ),
                None,
            ),
        ] {
            let response = super::super::app()
                .oneshot(request(&uri, body))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        }
    }
    #[tokio::test]
    async fn content_workbench_material_routes_reject_foreign_origin_and_unknown_payload() {
        let body =
            serde_json::json!({"domainRef":Uuid::new_v4(),"title":"作业启动","angle":"第一步"});
        let mut r = request("/ops/api/creation/sources/prepare", Some(body.clone()));
        r.headers_mut()
            .insert("Origin", "https://external.invalid".parse().unwrap());
        assert_eq!(
            super::super::app().oneshot(r).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let mut bad = body;
        bad["triggerCollection"] = serde_json::json!(true);
        assert_eq!(
            super::super::app()
                .oneshot(request("/ops/api/creation/sources/prepare", Some(bad)))
                .await
                .unwrap()
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}
