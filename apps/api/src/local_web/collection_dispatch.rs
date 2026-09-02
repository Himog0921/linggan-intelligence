//! Loopback dispatch contract for the Browser Producer.
//!
//! This module owns the narrow boundary between a station asking for its next
//! scheduled task and reporting a failure before a producer Attempt exists.
//! Neither route contacts a platform or accepts Evidence.

use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    body::Bytes,
    extract::State,
    response::{IntoResponse, Response},
    routing::post,
};
use linggan_evidence::{
    DispatchDecision, DispatchFailureCode, DispatchFailureError, DispatchFailureOutcome,
    decide_dispatch, requeue_failed_dispatch,
};

/// The station asks whether it may execute a bounded task. Published through
/// `/health`; the browser never hardcodes this path.
pub(super) const CLAIM_PATH: &str = "/api/local/dispatch/claim";

/// A successful claim whose browser work cannot start returns this local
/// execution failure before the frozen task may be retried.
pub(super) const FAILURE_PATH: &str = "/api/local/dispatch/failures";

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(CLAIM_PATH, post(claim))
        .route(FAILURE_PATH, post(failure))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimBody {
    install_key: String,
}

/// A station asks whether there is work it may run right now.
///
/// Only one answer permits a platform to be touched. Every other answer names
/// the gate that stopped it: a flat "nothing for you" cannot distinguish quota,
/// risk pause, missing station claim, and no waiting work.
async fn claim(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<ClaimBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "dispatch_claim_invalid",
        );
    };
    match decide_dispatch(database, &request.install_key).await {
        Ok(decision) => Json(payload(&decision)).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "dispatch_claim_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FailureBody {
    install_key: String,
    task_id: uuid::Uuid,
    failure_id: uuid::Uuid,
    failure_code: String,
}

/// A claimed task can fail before a producer Attempt exists: the browser may
/// be unable to create a tab, wait for page readiness, or receive a coherent
/// page receipt. Record that fact and requeue only while the caller still owns
/// this exact live claim. This endpoint never contacts a platform, accepts
/// Evidence, or interprets raw browser error text.
async fn failure(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<FailureBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "dispatch_failure_invalid",
        );
    };
    let Some(failure_code) = DispatchFailureCode::parse(&request.failure_code) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "dispatch_failure_code_invalid",
        );
    };
    match requeue_failed_dispatch(
        database,
        &request.install_key,
        request.task_id,
        request.failure_id,
        failure_code,
    )
    .await
    {
        Ok(DispatchFailureOutcome::Requeued {
            retry_after_seconds,
        }) => Json(serde_json::json!({
            "outcome": "requeued",
            "taskState": "pending",
            "nextPollAfterSeconds": retry_after_seconds,
        }))
        .into_response(),
        Ok(DispatchFailureOutcome::Replay {
            retry_after_seconds,
        }) => Json(serde_json::json!({
            "outcome": "replay",
            "taskState": "pending",
            "nextPollAfterSeconds": retry_after_seconds,
        }))
        .into_response(),
        Err(error) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            failure_error_code(&error),
        ),
    }
}

fn failure_error_code(error: &DispatchFailureError) -> &'static str {
    match error {
        DispatchFailureError::SchemaUnavailable => "dispatch_schema_unavailable",
        DispatchFailureError::UnknownInstallation => "dispatch_failure_installation_unknown",
        DispatchFailureError::ClaimNotHeld => "dispatch_failure_claim_not_held",
        DispatchFailureError::FailureIdentityConflict => "dispatch_failure_identity_conflict",
        DispatchFailureError::Database(_) => "dispatch_failure_write_failed",
    }
}

fn payload(decision: &DispatchDecision) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "decision": decision.code(),
        // The plugin must key off this and nothing else. A task body without
        // permission is still not permission.
        "mayExecute": decision.permits_execution(),
        // The server owns poll cadence, otherwise every plugin would carry a
        // divergent hard-coded retry interval.
        "nextPollAfterSeconds": decision.next_poll_after_seconds(),
    });
    match decision {
        DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
            execution_source_url,
            page_session_plan,
        } => {
            payload["taskId"] = serde_json::json!(task_id);
            payload["leaseRef"] = serde_json::json!(lease_ref);
            payload["taskSpec"] = task_spec.clone();
            if let Some(url) = execution_source_url {
                payload["executionSourceUrl"] = serde_json::json!(url);
            }
            if let Some(plan) = page_session_plan {
                payload["pageSessionPlan"] = plan.clone();
            }
            // Claim atomically moved this lease task out of pending. A second
            // poll cannot receive it while the first producer starts its page.
            payload["taskState"] = serde_json::json!("in_progress");
        }
        DispatchDecision::RiskPaused { reason } => {
            payload["reason"] = serde_json::json!(reason);
        }
        DispatchDecision::DailyQuotaReached { quota, used } => {
            payload["reason"] = serde_json::json!(format!(
                "这台工位今天已入库 {used} 篇，达到每日 {quota} 篇上限。工位没有离线，\
                 明天窗口重置后自然恢复。"
            ));
        }
        DispatchDecision::InstallationNotClaimed => {
            payload["reason"] =
                serde_json::json!("这个插件安装还没有归位到任何工位，不属于任何工位的产能。");
        }
        DispatchDecision::NothingWaiting => {
            payload["reason"] = serde_json::json!("没有等待派发的任务。");
        }
        DispatchDecision::ExecutionLocatorUnavailable { reason } => {
            payload["reason"] = serde_json::json!(reason);
        }
    }
    payload
}
