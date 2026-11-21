use super::*;
use axum::{
    extract::rejection::JsonRejection,
    http::{HeaderMap, StatusCode},
};
use linggan_intelligence::ops_content_lens::{LensDefinitionError, SnapshotScope, build_snapshot};

pub(super) fn routes(enabled: bool) -> Router<LocalWebState> {
    if !enabled {
        return Router::new().route(
            "/api/local/ops/content-lens/snapshots",
            post(|| async {
                local_read_json_error(StatusCode::NOT_FOUND, "ops_content_lens_disabled")
            }),
        );
    }
    Router::new().route("/api/local/ops/content-lens/snapshots", post(generate))
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
