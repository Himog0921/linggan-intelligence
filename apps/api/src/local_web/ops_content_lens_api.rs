use super::*;
use axum::{
    extract::rejection::JsonRejection,
    http::{HeaderMap, StatusCode},
};
use linggan_intelligence::ops_content_lens::{LensDefinitionError, SnapshotScope, build_snapshot};

pub(super) fn routes(enabled: bool) -> Router<LocalWebState> {
    if !enabled {
        return Router::new()
            .route(
                "/api/local/ops/content-lens/runs",
                post(|| async {
                    local_read_json_error(StatusCode::NOT_FOUND, "ops_content_lens_disabled")
                }),
            )
            .route(
                "/api/local/ops/content-lens/snapshots",
                post(|| async {
                    local_read_json_error(StatusCode::NOT_FOUND, "ops_content_lens_disabled")
                }),
            );
    }
    Router::new()
        .route("/api/local/ops/content-lens/runs", post(start))
        .route("/api/local/ops/content-lens/snapshots", post(generate))
}

async fn generate(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    body: Result<Json<SnapshotScope>, JsonRejection>,
) -> Response {
    if let Err(response) = super::ops_overview_api::require_admin(&state, &headers).await {
        return response;
    }
    let Some(db) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let Ok(Json(scope)) = body else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid");
    };
    match build_snapshot(db, scope).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(LensDefinitionError::InvalidScope) => {
            local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid")
        }
        Err(LensDefinitionError::MissingAgreement) => {
            local_read_json_error(StatusCode::CONFLICT, "ops_lens_missing_agreement")
        }
        Err(LensDefinitionError::NoMethod) => {
            local_read_json_error(StatusCode::CONFLICT, "ops_lens_no_current_method")
        }
        Err(_) => local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ops_lens_snapshot_unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use std::collections::BTreeSet;
    use tower::ServiceExt;

    #[tokio::test]
    async fn snapshot_write_obeys_feature_and_admin_guards() {
        for (enabled, status, code) in [
            (false, StatusCode::NOT_FOUND, "ops_content_lens_disabled"),
            (
                true,
                StatusCode::SERVICE_UNAVAILABLE,
                "ops_auth_unavailable",
            ),
        ] {
            let app = routes(enabled).with_state(LocalWebState {
                database: LocalDatabaseState::NotConfigured,
                active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
                account_digest_key: None,
            });
            let response = app
                .oneshot(
                    Request::post("/api/local/ops/content-lens/snapshots")
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["code"],
                code
            );
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartRun {
    command_ref: uuid::Uuid,
}

async fn start(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    body: Result<Json<StartRun>, JsonRejection>,
) -> Response {
    if let Err(response) = super::ops_overview_api::require_admin(&state, &headers).await {
        return response;
    }
    let Some(db) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let Ok(Json(request)) = body else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_invalid_run");
    };
    super::ai_settings::respond(
        linggan_intelligence::ops_content_lens::label::start(db, request.command_ref).await,
    )
}
