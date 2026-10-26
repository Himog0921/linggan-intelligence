//! OPS-HTTP-001 §4 (OPS-MIGRATION-001 D3): 总览 reads and the target save. The save is the only
//! write of the 「运营」 pages; like the reads it sits behind the local guard in `ops::routes`, so
//! only a same-origin page on this machine can call it.
use super::*;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderMap;
use linggan_intelligence::ops_ledger::read::{
    OpsBusiness, OpsOverviewQuery, OverviewSlice, read_ops_overview, read_ops_overview_trend,
};
use linggan_intelligence::ops_ledger::targets::{
    OpsTargetError, SetTargetRequest, TargetPeriod, read_ops_target_history, set_ops_target,
};
use serde::Deserialize;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/ops/overview", get(overview))
        .route("/api/local/ops/overview/trend", get(trend))
        .route("/api/local/ops/targets", get(targets).post(save_target))
}

#[derive(Deserialize)]
struct OverviewQuery {
    domain: Option<String>,
    slice: Option<String>,
    from: Option<String>,
    to: Option<String>,
    granularity: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetQuery {
    period_type: Option<String>,
    period_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveTargetBody {
    period_type: String,
    period_key: String,
    target_value: i64,
    expected_revision: Option<uuid::Uuid>,
    request_id: uuid::Uuid,
}

async fn overview(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<OverviewQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let parsed = OpsBusiness::parse(query.domain.as_deref()).and_then(|business| {
        let slice = OverviewSlice::parse(
            query.slice.as_deref(),
            query.from.as_deref(),
            query.to.as_deref(),
        )?;
        Ok((business, slice))
    });
    match parsed {
        Ok((business, slice)) => super::ops_api::json(
            read_ops_overview(database, business, &OpsOverviewQuery { slice, today: None }).await,
        ),
        Err(error) => super::ops_api::read_error(error),
    }
}

async fn trend(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<OverviewQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => super::ops_api::json(
            read_ops_overview_trend(database, business, query.granularity.as_deref(), None).await,
        ),
        Err(error) => super::ops_api::read_error(error),
    }
}

async fn targets(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<TargetQuery>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let period = match TargetPeriod::parse(
        query.period_type.as_deref().unwrap_or_default(),
        query.period_key.as_deref().unwrap_or_default(),
    ) {
        Ok(period) => period,
        Err(error) => return target_error(error),
    };
    match read_ops_target_history(database, &period).await {
        Ok(history) => Json(history).into_response(),
        Err(error) => target_error(error),
    }
}

async fn save_target(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    body: Result<Json<SaveTargetBody>, JsonRejection>,
) -> Response {
    if let Err(response) = require_admin(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let Ok(Json(body)) = body else {
        return target_error(OpsTargetError::InvalidParameter("body"));
    };
    let period = match TargetPeriod::parse(&body.period_type, &body.period_key) {
        Ok(period) => period,
        Err(error) => return target_error(error),
    };
    let Ok(target_value) = i32::try_from(body.target_value) else {
        return target_error(OpsTargetError::InvalidParameter("targetValue"));
    };
    let request = SetTargetRequest {
        period,
        target_value,
        expected_revision: body.expected_revision,
        request_ref: body.request_id,
    };
    match set_ops_target(database, &request).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => target_error(error),
    }
}

async fn require_admin(state: &LocalWebState, headers: &HeaderMap) -> Result<(), Response> {
    let member = super::ops_auth::current_member(state, headers).await?;
    if member.role == "admin" {
        Ok(())
    } else {
        Err(local_read_json_error(
            axum::http::StatusCode::FORBIDDEN,
            "ops_auth_forbidden",
        ))
    }
}

fn target_error(error: OpsTargetError) -> Response {
    match error {
        OpsTargetError::InvalidParameter(_) => {
            local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "ops_parameter_invalid")
        }
        OpsTargetError::RequestConflict => local_read_json_error(
            axum::http::StatusCode::CONFLICT,
            "ops_target_request_conflict",
        ),
        OpsTargetError::Database(error) => {
            eprintln!(
                "ops target unavailable: database_kind={:?}",
                error.as_database_error().and_then(|value| value.code())
            );
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "ops_read_unavailable",
            )
        }
    }
}
