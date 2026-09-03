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
    CreatorLifecycleAnalysis, CreatorLifecycleExclusions, CreatorLifecycleMetric,
    CreatorLifecycleProjection, CreatorLifecycleQuery, CreatorLifecycleReadError,
    CreatorLifecycleReceipt, CreatorLifecycleStatus, CreatorLifecycleSummary,
    CreatorLifecycleWindow, read_creator_lifecycle,
};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LifecycleApiProjection<'a> {
    target_ref: uuid::Uuid,
    target_kind: &'a str,
    status: CreatorLifecycleStatus,
    as_of: &'a str,
    window: CreatorLifecycleWindow,
    metric: CreatorLifecycleMetric,
    summary: &'a CreatorLifecycleSummary,
    exclusions: &'a CreatorLifecycleExclusions,
    receipt: &'a CreatorLifecycleReceipt,
    analysis: &'a CreatorLifecycleAnalysis,
    points: Vec<LifecycleApiPoint>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleApiPoint {
    work_public_ref: uuid::Uuid,
    creator_percentile: f64,
    rolling_median: f64,
}

/// The public target lifecycle API is intentionally a derived result, not a second Work facts
/// API. Title, author, publication and engagement Current remain owned by Work Resource Read;
/// server-rendered HTML may consume the internal projection in-process.
pub(super) fn api_projection(
    projection: &CreatorLifecycleProjection,
) -> LifecycleApiProjection<'_> {
    LifecycleApiProjection {
        target_ref: projection.target_ref,
        target_kind: &projection.target_kind,
        status: projection.status,
        as_of: &projection.as_of,
        window: projection.window,
        metric: projection.metric,
        summary: &projection.summary,
        exclusions: &projection.exclusions,
        receipt: &projection.receipt,
        analysis: &projection.analysis,
        points: projection
            .points
            .iter()
            .map(|point| LifecycleApiPoint {
                work_public_ref: point.work_public_ref,
                creator_percentile: point.creator_percentile,
                rolling_median: point.rolling_median,
            })
            .collect(),
    }
}

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
    let Ok(query) =
        CreatorLifecycleQuery::parse_optional(params.window.as_deref(), params.metric.as_deref())
    else {
        return local_read_json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "creator_lifecycle_query_invalid",
        );
    };
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    match read_creator_lifecycle(database, target_ref, &query).await {
        Ok(Some(projection)) => Json(api_projection(&projection)).into_response(),
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
