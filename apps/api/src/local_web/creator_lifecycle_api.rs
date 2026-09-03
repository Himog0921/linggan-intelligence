//! Loopback-only target-scoped creator lifecycle read API.

use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_evidence::{
    CreatorLifecycleMetric, CreatorLifecycleQuery, CreatorLifecycleReadError,
    CreatorLifecycleWindow, read_creator_lifecycle,
};

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new().route(
        "/api/local/collection/targets/{target_ref}/lifecycle",
        get(read_json),
    )
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleParams {
    window: Option<String>,
    metric: Option<String>,
}

async fn read_json(
    State(state): State<LocalWebState>,
    Path(target_ref): Path<String>,
    Query(params): Query<LifecycleParams>,
) -> Response {
    let Ok(target_ref) = uuid::Uuid::parse_str(&target_ref) else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "creator_lifecycle_target_invalid");
    };
    let Some(window) = params
        .window
        .as_deref()
        .map(CreatorLifecycleWindow::parse)
        .unwrap_or(Some(CreatorLifecycleWindow::Recent90Days))
    else {
        return local_read_json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "creator_lifecycle_query_invalid",
        );
    };
    let Some(metric) = params
        .metric
        .as_deref()
        .map(CreatorLifecycleMetric::parse)
        .unwrap_or(Some(CreatorLifecycleMetric::Likes))
    else {
        return local_read_json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "creator_lifecycle_query_invalid",
        );
    };
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    match read_creator_lifecycle(
        database,
        target_ref,
        &CreatorLifecycleQuery { window, metric },
    )
    .await
    {
        Ok(Some(projection)) => Json(projection).into_response(),
        Ok(None) => {
            local_read_json_error(StatusCode::NOT_FOUND, "creator_lifecycle_target_not_found")
        }
        Err(CreatorLifecycleReadError::ProjectionUnavailable) => local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "creator_lifecycle_projection_unavailable",
        ),
        Err(CreatorLifecycleReadError::Database(error)) => {
            eprintln!("creator lifecycle read unavailable: {error}");
            local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "creator_lifecycle_read_unavailable",
            )
        }
    }
}
