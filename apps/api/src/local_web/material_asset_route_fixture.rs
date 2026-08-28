use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

pub(super) struct LocalFixtureFiles(Vec<std::path::PathBuf>);

impl LocalFixtureFiles {
    pub(super) fn new(paths: Vec<std::path::PathBuf>) -> Self {
        Self(paths)
    }
}

impl Drop for LocalFixtureFiles {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(super) async fn assert_asset_response(database: &Database, uri: &str, expected: &[u8]) {
    let response = app_with_database(database.clone())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private, no-store, max-age=0")
    );
    assert_eq!(
        &to_bytes(response.into_body(), usize::MAX).await.unwrap()[..],
        expected
    );
}

pub(super) async fn assert_integrity_and_symlink_attacks_are_unavailable(
    database: &Database,
    uri: &str,
    blob_path: &std::path::Path,
) {
    std::fs::write(blob_path, b"proof-bytes?").unwrap();
    let corrupt = local_read_error(database, uri).await;
    std::fs::write(blob_path, b"short").unwrap();
    let short = local_read_error(database, uri).await;
    std::fs::remove_file(blob_path).unwrap();
    let outside =
        std::env::temp_dir().join(format!("linggan-media-escape-{}", uuid::Uuid::new_v4()));
    std::fs::write(&outside, b"proof-bytes!").unwrap();
    std::os::unix::fs::symlink(&outside, blob_path).unwrap();
    let escaped = local_read_error(database, uri).await;
    std::fs::remove_file(blob_path).unwrap();
    std::fs::remove_file(outside).unwrap();
    std::fs::create_dir(blob_path).unwrap();
    let io_error = local_read_error(database, uri).await;
    std::fs::remove_dir(blob_path).unwrap();
    std::fs::write(blob_path, b"proof-bytes!").unwrap();
    assert_local_read_error(
        corrupt,
        StatusCode::SERVICE_UNAVAILABLE,
        "local_media_integrity_mismatch",
    );
    assert_local_read_error(
        short,
        StatusCode::SERVICE_UNAVAILABLE,
        "local_media_integrity_mismatch",
    );
    assert_local_read_error(escaped, StatusCode::NOT_FOUND, "local_media_not_found");
    assert_local_read_error(
        io_error,
        StatusCode::SERVICE_UNAVAILABLE,
        "local_media_read_unavailable",
    );
}

pub(super) async fn assert_derivative_integrity_is_checked(
    database: &Database,
    uri: &str,
    derivative_path: &std::path::Path,
) {
    std::fs::write(derivative_path, b"ocr-proof-byte?").unwrap();
    let corrupt = local_read_error(database, uri).await;
    std::fs::write(derivative_path, b"ocr-proof-bytes").unwrap();
    assert_local_read_error(
        corrupt,
        StatusCode::SERVICE_UNAVAILABLE,
        "local_media_integrity_mismatch",
    );
}

pub(super) async fn assert_materialization_read_contract(
    database: &Database,
    materialization_url: &str,
    materialization_ref: uuid::Uuid,
    shared_materialization_url: &str,
) {
    assert_asset_response(database, materialization_url, b"proof-bytes!").await;
    assert_asset_response(database, shared_materialization_url, b"proof-bytes!").await;
    let invalid_uri = format!(
        "/api/local/media/{materialization_ref}/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
    let invalid = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(invalid_uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::NOT_FOUND);
    let invalid: Value =
        serde_json::from_slice(&to_bytes(invalid.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        invalid.pointer("/operation").and_then(Value::as_str),
        Some("local_read")
    );
    assert!(invalid.get("delivery").is_none());
}

async fn local_read_error(database: &Database, uri: &str) -> (StatusCode, Value) {
    let response = app_with_database(database.clone())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, body)
}

fn assert_local_read_error(response: (StatusCode, Value), status: StatusCode, code: &str) {
    let (actual_status, body) = response;
    assert_eq!(actual_status, status);
    assert_eq!(
        body.pointer("/operation").and_then(Value::as_str),
        Some("local_read")
    );
    assert_eq!(
        body.pointer("/outcome").and_then(Value::as_str),
        Some("unavailable")
    );
    assert_eq!(body.pointer("/code").and_then(Value::as_str), Some(code));
    assert!(body.get("delivery").is_none());
}
