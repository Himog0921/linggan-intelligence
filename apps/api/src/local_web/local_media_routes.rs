//! Loopback delivery for revocable, Evidence-qualified local assets.

use super::*;
use linggan_evidence::{LocalMaterialAsset, read_local_derivative, read_local_materialization};

pub(super) async fn materialization(
    State(state): State<LocalWebState>,
    Path((materialization_ref, sha256)): Path<(String, String)>,
) -> Response {
    let Ok(materialization_ref) = uuid::Uuid::parse_str(&materialization_ref) else {
        return not_found();
    };
    if !is_sha256(&sha256) {
        return not_found();
    }
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    match read_local_materialization(database, materialization_ref, &sha256).await {
        Ok(Some(asset)) => read_asset(asset),
        Ok(None) => not_found(),
        Err(_) => unavailable(),
    }
}

pub(super) async fn derivative(
    State(state): State<LocalWebState>,
    Path(derivative_ref): Path<String>,
) -> Response {
    let Ok(derivative_ref) = uuid::Uuid::parse_str(&derivative_ref) else {
        return not_found();
    };
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    match read_local_derivative(database, derivative_ref).await {
        Ok(Some(asset)) => read_asset(asset),
        Ok(None) => not_found(),
        Err(_) => unavailable(),
    }
}

fn read_asset(asset: LocalMaterialAsset) -> Response {
    let Ok(content_type) = HeaderValue::from_str(&asset.mime_type) else {
        return local_producer_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "local_media_metadata_invalid",
        );
    };
    let storage_key = FsPath::new(&asset.storage_key);
    if storage_key.is_absolute()
        || storage_key
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return local_producer_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "local_media_metadata_invalid",
        );
    }
    match fs::read(local_media_root().join(storage_key)) {
        Ok(bytes) => (
            [
                (header::CONTENT_TYPE, content_type),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("private, no-store, max-age=0"),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => local_producer_error(
            axum::http::StatusCode::NOT_FOUND,
            "local_media_bytes_unavailable",
        ),
    }
}

fn not_found() -> Response {
    local_producer_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found")
}

fn unavailable() -> Response {
    local_producer_error(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "local_media_metadata_unavailable",
    )
}
