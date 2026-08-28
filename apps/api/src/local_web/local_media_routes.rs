//! Loopback delivery for revocable, Evidence-qualified local assets.

use super::local_asset_delivery::{self, QualifiedAssetRequest};
use super::*;

pub(super) async fn materialization(
    State(state): State<LocalWebState>,
    Path((materialization_ref, sha256)): Path<(String, String)>,
) -> Response {
    let Ok(materialization_ref) = uuid::Uuid::parse_str(&materialization_ref) else {
        return invalid_handle();
    };
    if !is_sha256(&sha256) {
        return invalid_handle();
    }
    local_asset_delivery::deliver(
        state,
        QualifiedAssetRequest::Materialization {
            materialization_ref,
            sha256,
        },
    )
    .await
}

pub(super) async fn derivative(
    State(state): State<LocalWebState>,
    Path(derivative_ref): Path<String>,
) -> Response {
    let Ok(derivative_ref) = uuid::Uuid::parse_str(&derivative_ref) else {
        return invalid_handle();
    };
    local_asset_delivery::deliver(state, QualifiedAssetRequest::Derivative { derivative_ref }).await
}

fn invalid_handle() -> Response {
    local_read_json_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found")
}
