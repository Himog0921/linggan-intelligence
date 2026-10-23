//! CONTENT-WORKBENCH-001 local authorship HTTP composition. Domain logic belongs to core.
use super::*;
use axum::{extract::DefaultBodyLimit, http::StatusCode};
use core::{WorkbenchError, types::*};
use linggan_intelligence::content_workbench as core;
use serde::Deserialize;
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops/api/creation/works", get(list).post(create))
        .route("/ops/api/creation/storage", get(storage))
        .route("/ops/api/creation/requests/{id}", get(request_receipt))
        .route("/ops/api/creation/works/{id}", get(read))
        .route("/ops/api/creation/works/{id}/save", post(save))
        .route("/ops/api/creation/works/{id}/revisions", get(history))
        .route("/ops/api/creation/works/{id}/restore", post(restore))
        .route("/ops/api/creation/works/{id}/place", post(place))
        .route("/ops/api/creation/works/{id}/assets", post(upload_asset))
        .route("/ops/api/creation/works/{id}/assets/{asset}", get(asset))
        .route("/ops/api/creation/works/{id}/exports", post(freeze))
        .route("/ops/api/creation/exports/{id}", get(export))
        .route(
            "/ops/api/creation/exports/{id}/pages/{page}",
            get(page).put(upload_page),
        )
        .route("/ops/api/creation/exports/{id}/finalize", post(finalize))
        .route("/ops/api/creation/exports/{id}/fail", post(failed))
        .route("/ops/api/creation/exports/{id}/retry", post(retry))
        .route("/ops/api/creation/exports/{id}/download", get(download))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
macro_rules! database {
    ($state:ident) => {
        match $state.database.database() {
            Some(db) => db,
            None => {
                return local_read_json_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "creation_not_connected",
                )
            }
        }
    };
}
async fn request_receipt(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    reply(core::read_request(database!(state), id).await)
}
async fn storage() -> Response {
    Json(json!({"configured":std::env::var("LINGGAN_CREATION_ASSET_ROOT").ok().is_some_and(|s|std::path::Path::new(&s).is_absolute())})).into_response()
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListQuery {
    domain_ref: Option<Uuid>,
}
async fn list(State(state): State<LocalWebState>, Query(q): Query<ListQuery>) -> Response {
    match core::list_works(database!(state), q.domain_ref).await {
        Ok(items) => Json(json!({"items":items})).into_response(),
        Err(e) => error(e),
    }
}
async fn create(State(state): State<LocalWebState>, Json(r): Json<CreateRequest>) -> Response {
    receipt(core::create_work(database!(state), &r).await)
}
async fn read(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    reply(core::read_work(database!(state), id).await)
}
async fn save(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<SaveRequest>,
) -> Response {
    receipt(core::save(database!(state), id, &r).await)
}
async fn history(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    match core::revisions(database!(state), id).await {
        Ok(items) => Json(json!({"items":items})).into_response(),
        Err(e) => error(e),
    }
}
async fn restore(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<RestoreRequest>,
) -> Response {
    receipt(core::restore(database!(state), id, &r).await)
}
async fn place(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<PlaceRequest>,
) -> Response {
    receipt(core::place(database!(state), id, &r).await)
}
async fn upload_asset(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    bytes: Bytes,
) -> Response {
    let Some(request) = headers
        .get("x-creation-request-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
    else {
        return error(WorkbenchError::Invalid("requestId"));
    };
    reply(core::upload_asset(database!(state), id, request, &bytes).await)
}
async fn asset(
    State(state): State<LocalWebState>,
    Path((id, asset)): Path<(Uuid, Uuid)>,
) -> Response {
    match core::asset_file(database!(state), id, asset).await {
        Ok((path, r)) => file(path, &r.mime_type, None).await,
        Err(e) => error(e),
    }
}
async fn freeze(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<FreezeRequest>,
) -> Response {
    frozen(core::freeze_export(database!(state), id, &r).await)
}
async fn export(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    reply(core::read_export(database!(state), id).await)
}
async fn upload_page(
    State(state): State<LocalWebState>,
    Path((id, page)): Path<(Uuid, Uuid)>,
    bytes: Bytes,
) -> Response {
    reply(core::upload_export_page(database!(state), id, page, &bytes).await)
}
async fn page(
    State(state): State<LocalWebState>,
    Path((id, page)): Path<(Uuid, Uuid)>,
) -> Response {
    match core::export_page_file(database!(state), id, page).await {
        Ok(path) => file(path, "image/png", Some("page.png")).await,
        Err(e) => error(e),
    }
}
async fn finalize(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<FinalizeRequest>,
) -> Response {
    reply(core::finalize_export(database!(state), id, &r).await)
}
async fn failed(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<ExportFailureRequest>,
) -> Response {
    reply(core::mark_export_failed(database!(state), id, &r).await)
}
async fn retry(
    State(state): State<LocalWebState>,
    Path(id): Path<Uuid>,
    Json(r): Json<FinalizeRequest>,
) -> Response {
    frozen(core::retry_export(database!(state), id, &r).await)
}
async fn download(State(state): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    match core::export_file(database!(state), id).await {
        Ok(path) => file(path, "application/zip", Some("creation.zip")).await,
        Err(e) => error(e),
    }
}
fn frozen(r: Result<(ExportRecord, Revision), WorkbenchError>) -> Response {
    match r {
        Ok((export, revision)) => {
            Json(json!({"export":export,"revision":revision})).into_response()
        }
        Err(e) => error(e),
    }
}
fn reply<T: serde::Serialize>(r: Result<T, WorkbenchError>) -> Response {
    match r {
        Ok(r) => Json(r).into_response(),
        Err(e) => error(e),
    }
}
fn receipt(r: Result<SaveReceipt, WorkbenchError>) -> Response {
    match r {
        Ok(r) if r.outcome == "stale" => (
            StatusCode::CONFLICT,
            Json(json!({"error":{"code":"creation_stale"},"receipt":r})),
        )
            .into_response(),
        other => reply(other),
    }
}
pub(super) fn error(e: WorkbenchError) -> Response {
    let (status, code) = match e {
        WorkbenchError::Invalid(_) => (StatusCode::BAD_REQUEST, "creation_parameter_invalid"),
        WorkbenchError::NotFound => (StatusCode::NOT_FOUND, "creation_not_found"),
        WorkbenchError::RequestConflict => (StatusCode::CONFLICT, "creation_request_conflict"),
        WorkbenchError::Source => (StatusCode::CONFLICT, "creation_source_unavailable"),
        WorkbenchError::Incomplete => (StatusCode::CONFLICT, "creation_export_incomplete"),
        WorkbenchError::ExportConflict => (StatusCode::CONFLICT, "creation_export_conflict"),
        WorkbenchError::StorageNotConfigured => (
            StatusCode::SERVICE_UNAVAILABLE,
            "creation_storage_not_configured",
        ),
        WorkbenchError::LegacyReferenceUnavailable => (
            StatusCode::CONFLICT,
            "creation_legacy_reference_unavailable",
        ),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "creation_unavailable"),
    };
    local_read_json_error(status, code)
}
async fn file(path: PathBuf, mime: &str, name: Option<&str>) -> Response {
    let f = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return error(WorkbenchError::Invalid("file")),
    };
    let mut r = Response::new(axum::body::Body::from_stream(
        tokio_util::io::ReaderStream::new(f),
    ));
    if let Ok(v) = HeaderValue::from_str(mime) {
        r.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    r.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if let Some(name) = name {
        if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
            r.headers_mut().insert(header::CONTENT_DISPOSITION, v);
        }
    }
    r
}
