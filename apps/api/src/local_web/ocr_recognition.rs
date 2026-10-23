//! Work-scoped OCR read/start boundary. Browser input is only a work ref and an idempotency key.
use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_intelligence::ai_foundation::{
    ocr_manual_adoption::{self, Adopt},
    ocr_recognition::{self, StartRun},
};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/work-resources/{public_ref}/ocr-recognition",
            get(read).post(start),
        )
        .route(
            "/api/local/work-resources/{public_ref}/ocr-recognition/adoptions",
            axum::routing::post(adopt),
        )
}
async fn adopt(
    State(state): State<LocalWebState>,
    Path(work): Path<String>,
    body: Bytes,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    let Ok(work) = Uuid::parse_str(&work) else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "invalid_material_ref");
    };
    let Ok(request) = serde_json::from_slice::<Adopt>(&body) else {
        return local_read_json_error(
            StatusCode::BAD_REQUEST,
            "invalid_manual_ocr_adoption_command",
        );
    };
    match ocr_manual_adoption::adopt(db, work, &request).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(linggan_intelligence::ai_foundation::store::FoundationError::NotFound) => {
            local_read_json_error(StatusCode::NOT_FOUND, "manual_ocr_adoption_unavailable")
        }
        Err(linggan_intelligence::ai_foundation::store::FoundationError::Conflict) => {
            local_read_json_error(StatusCode::CONFLICT, "manual_ocr_adoption_conflict")
        }
        Err(_) => local_read_json_error(StatusCode::BAD_REQUEST, "manual_ocr_adoption_refused"),
    }
}
async fn read(State(state): State<LocalWebState>, Path(work): Path<String>) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    let Ok(work) = Uuid::parse_str(&work) else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "invalid_material_ref");
    };
    match ocr_recognition::read_run(db, work).await {
        Ok(value) => Json(value).into_response(),
        Err(_) => local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ocr_recognition_read_unavailable",
        ),
    }
}
async fn start(
    State(state): State<LocalWebState>,
    Path(work): Path<String>,
    body: Bytes,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    let Ok(work) = Uuid::parse_str(&work) else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "invalid_material_ref");
    };
    let Ok(request) = serde_json::from_slice::<StartRun>(&body) else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "invalid_ocr_recognition_command");
    };
    match ocr_recognition::start_run(db, work, &request).await {
        Ok(value) => Json(value).into_response(),
        Err(_) => local_read_json_error(StatusCode::CONFLICT, "ocr_recognition_start_refused"),
    }
}
