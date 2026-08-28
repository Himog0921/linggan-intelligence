//! Qualified loopback delivery for revocable local assets.
//!
//! Callers provide only an Evidence-qualified handle. This module owns the bounded streaming,
//! storage-root confinement, response headers, and honest local-read failures.

use super::*;
use axum::body::Body;
use linggan_evidence::{LocalMaterialAsset, read_local_derivative, read_local_materialization};
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;

pub(super) enum QualifiedAssetRequest {
    Materialization {
        materialization_ref: uuid::Uuid,
        sha256: String,
    },
    Derivative {
        derivative_ref: uuid::Uuid,
    },
}

pub(super) async fn deliver(state: LocalWebState, request: QualifiedAssetRequest) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let asset = match request {
        QualifiedAssetRequest::Materialization {
            materialization_ref,
            sha256,
        } => read_local_materialization(database, materialization_ref, &sha256).await,
        QualifiedAssetRequest::Derivative { derivative_ref } => {
            read_local_derivative(database, derivative_ref).await
        }
    };
    match asset {
        Ok(Some(asset)) => stream(asset).await,
        Ok(None) => not_found(),
        Err(error) => {
            eprintln!(
                "local media qualification unavailable: database_kind={:?}",
                error.as_database_error().and_then(|value| value.code())
            );
            unavailable("local_media_metadata_unavailable")
        }
    }
}

async fn stream(asset: LocalMaterialAsset) -> Response {
    let Some(expected_size) = asset.expected_byte_size else {
        return unavailable("local_media_metadata_invalid");
    };
    if expected_size <= 0 || expected_size > asset.maximum_byte_size {
        return unavailable("local_media_size_out_of_bounds");
    }
    let storage_key = FsPath::new(&asset.storage_key);
    if asset.storage_key.is_empty()
        || storage_key.is_absolute()
        || asset
            .storage_key
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return unavailable("local_media_metadata_invalid");
    }
    let root = match fs::canonicalize(local_media_root()) {
        Ok(root) => root,
        Err(error) => return root_failure(error),
    };
    let candidate = match fs::canonicalize(root.join(storage_key)) {
        Ok(candidate) if candidate.starts_with(&root) => candidate,
        Ok(_) => return not_found(),
        Err(error) => return candidate_failure(error),
    };
    let file = match tokio::fs::File::open(candidate).await {
        Ok(file) => file,
        Err(error) => return candidate_failure(error),
    };
    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(error) => return unavailable_io(error),
    };
    if !metadata.is_file() || i64::try_from(metadata.len()).ok() != Some(expected_size) {
        return unavailable("local_media_integrity_mismatch");
    }
    let Ok(content_type) = HeaderValue::from_str(&asset.delivery_mime_type) else {
        return unavailable("local_media_metadata_invalid");
    };
    let limit = u64::try_from(expected_size).unwrap_or(0);
    let stream = ReaderStream::new(file.take(limit));
    let mut response = Response::new(Body::from_stream(stream));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store, max-age=0"),
    );
    if let Ok(content_length) = HeaderValue::from_str(&expected_size.to_string()) {
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, content_length);
    }
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if !asset.inline_safe {
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment"),
        );
        response.headers_mut().insert(
            "x-linggan-media-state",
            HeaderValue::from_static("UNSUPPORTED_MEDIA_TYPE"),
        );
    }
    response
}

fn root_failure(error: std::io::Error) -> Response {
    eprintln!("local media root unavailable: io_kind={:?}", error.kind());
    unavailable("local_media_root_unavailable")
}

fn candidate_failure(error: std::io::Error) -> Response {
    if error.kind() == std::io::ErrorKind::NotFound {
        not_found()
    } else {
        unavailable_io(error)
    }
}

fn unavailable_io(error: std::io::Error) -> Response {
    eprintln!("local media read unavailable: io_kind={:?}", error.kind());
    unavailable("local_media_read_unavailable")
}

fn not_found() -> Response {
    local_read_json_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found")
}

fn unavailable(code: &'static str) -> Response {
    local_read_json_error(axum::http::StatusCode::SERVICE_UNAVAILABLE, code)
}
