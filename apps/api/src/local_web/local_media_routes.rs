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
        return local_read_json_error(
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
        return local_read_json_error(
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
        return local_read_json_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "local_media_metadata_invalid",
        );
    };
    let storage_key = FsPath::new(&asset.storage_key);
    if asset.storage_key.is_empty()
        || storage_key.is_absolute()
        || asset
            .storage_key
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return local_read_json_error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "local_media_metadata_invalid",
        );
    }
    let root = match fs::canonicalize(local_media_root()) {
        Ok(root) => root,
        Err(error) => return io_failure(error),
    };
    let candidate = match fs::canonicalize(root.join(storage_key)) {
        Ok(candidate) if candidate.starts_with(&root) => candidate,
        Ok(_) => return not_found(),
        Err(error) => return io_failure(error),
    };
    match fs::read(candidate) {
        Ok(bytes) if bytes_match(&asset, &bytes) => (
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
        Ok(_) => {
            eprintln!("local media read unavailable: integrity_mismatch");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "local_media_integrity_mismatch",
            )
        }
        Err(error) => io_failure(error),
    }
}

fn bytes_match(asset: &LocalMaterialAsset, bytes: &[u8]) -> bool {
    let size_matches = asset.expected_byte_size.is_none_or(|expected| {
        usize::try_from(expected).is_ok_and(|expected| expected == bytes.len())
    });
    size_matches && sha256_bytes(bytes) == asset.expected_sha256
}

fn io_failure(error: std::io::Error) -> Response {
    if error.kind() == std::io::ErrorKind::NotFound {
        return not_found();
    }
    eprintln!("local media read unavailable: io_kind={:?}", error.kind());
    local_read_json_error(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "local_media_read_unavailable",
    )
}

fn not_found() -> Response {
    local_read_json_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found")
}

fn unavailable() -> Response {
    local_read_json_error(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "local_media_metadata_unavailable",
    )
}
