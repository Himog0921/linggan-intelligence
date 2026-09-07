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
    AccountEligibilitySignal, CollectionControlError, DispatchDecision, DispatchFailureCode,
    DispatchFailureError, DispatchFailureOutcome, activate_installation_credential,
    decide_dispatch, report_account_eligibility, requeue_failed_dispatch,
};

/// The station asks whether it may execute a bounded task. Published through
/// `/health`; the browser never hardcodes this path.
pub(super) const CLAIM_PATH: &str = "/api/local/dispatch/claim";

/// A successful claim whose browser work cannot start returns this local
/// execution failure before the frozen task may be retried.
pub(super) const FAILURE_PATH: &str = "/api/local/dispatch/failures";

pub(super) const ACCOUNT_ELIGIBILITY_PATH: &str =
    "/api/local/stations/account-eligibility-observations";
pub(super) const CREDENTIAL_ACTIVATION_PATH: &str =
    "/api/local/stations/installation-credentials/activate";

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(CLAIM_PATH, post(claim))
        .route(FAILURE_PATH, post(failure))
        .route(ACCOUNT_ELIGIBILITY_PATH, post(report_account))
        .route(CREDENTIAL_ACTIVATION_PATH, post(activate_credential))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialActivationBody {
    installation_ref: uuid::Uuid,
    credential_ref: uuid::Uuid,
    installation_credential: String,
}

async fn activate_credential(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<CredentialActivationBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "installation_credential_activation_invalid",
        );
    };
    match activate_installation_credential(
        database,
        request.installation_ref,
        request.credential_ref,
        &request.installation_credential,
    )
    .await
    {
        Ok(()) => Json(serde_json::json!({"outcome":"activated"})).into_response(),
        Err(CollectionControlError::InvalidCredential) => local_read_json_error(
            axum::http::StatusCode::UNAUTHORIZED,
            "installation_credential_invalid",
        ),
        Err(CollectionControlError::SchemaUnavailable)
        | Err(CollectionControlError::Database(_)) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "installation_credential_activation_unavailable",
        ),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "installation_credential_activation_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountEligibilityBody {
    installation_ref: uuid::Uuid,
    installation_credential: String,
    raw_platform_account_id: Option<String>,
    signal: String,
}

/// Consume one closed producer observation. The raw platform id never leaves this stack frame;
/// the evidence layer stores only its keyed digest and a server-owned eligibility projection.
async fn report_account(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Some(digest_key) = state.account_digest_key.as_deref() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "account_digest_key_unavailable",
        );
    };
    let Ok(request) = serde_json::from_slice::<AccountEligibilityBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "account_eligibility_observation_invalid",
        );
    };
    let Some(signal) = AccountEligibilitySignal::parse(&request.signal) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "account_eligibility_signal_invalid",
        );
    };
    match report_account_eligibility(
        database,
        request.installation_ref,
        &request.installation_credential,
        request.raw_platform_account_id.as_deref(),
        signal,
        digest_key,
    )
    .await
    {
        Ok(receipt) => Json(serde_json::json!({
            "outcome": "observed",
            "installationRef": receipt.installation_ref,
            "accountRef": receipt.account_ref,
            "eligibilityState": receipt.state.as_str(),
            "bindingRequired": receipt.binding_required,
        }))
        .into_response(),
        Err(CollectionControlError::InvalidCredential) => local_read_json_error(
            axum::http::StatusCode::UNAUTHORIZED,
            "installation_credential_invalid",
        ),
        Err(CollectionControlError::SchemaUnavailable)
        | Err(CollectionControlError::MissingDigestKey) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "account_control_unavailable",
        ),
        Err(CollectionControlError::Database(_)) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "account_eligibility_observation_failed",
        ),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "account_eligibility_observation_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimBody {
    install_key: String,
    installation_credential: String,
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
    match decide_dispatch(
        database,
        &request.install_key,
        &request.installation_credential,
    )
    .await
    {
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
    installation_credential: String,
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
        &request.installation_credential,
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
        Ok(DispatchFailureOutcome::Unavailable) => Json(serde_json::json!({
            "outcome": "unavailable",
            "taskState": "unavailable",
            // The current material is terminal, so immediately ask for the
            // next sequentially eligible task rather than holding the batch.
            "nextPollAfterSeconds": 0,
        }))
        .into_response(),
        Ok(DispatchFailureOutcome::Blocked) => Json(serde_json::json!({
            "outcome": "blocked",
            "taskState": "blocked",
            // This material exhausted its bounded pre-Attempt retry budget.
            // It no longer holds the sequential batch, so ask for another
            // eligible frozen material immediately.
            "nextPollAfterSeconds": 0,
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
        DispatchFailureError::InvalidCredential => "dispatch_failure_credential_invalid",
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
        DispatchDecision::ControlBlocked { reason_code } => {
            payload["reasonCode"] = serde_json::json!(reason_code);
            payload["reason"] = serde_json::json!("当前控制资格已变化，任务保持等待。");
        }
    }
    payload
}
