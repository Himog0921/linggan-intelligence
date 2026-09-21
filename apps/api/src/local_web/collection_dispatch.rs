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
    AccountEligibilityObservation, CollectionControlError, DetailPageRiskSignalError,
    DetailPageSessionGrant, DetailPageSessionGrantError, DetailPageSessionNavigationError,
    DetailPageSessionProgress, DispatchDecision, DispatchFailureCode, DispatchFailureError,
    DispatchFailureOutcome, ExplicitAccountEligibilitySignal, PreparedLaneDelivery,
    activate_installation_credential, decide_dispatch, grant_detail_page_session,
    grant_detail_page_session_with_lane_deliveries, record_detail_page_session_progress,
    record_dispatch_answer, report_account_eligibility, report_claimed_task_account_eligibility,
    report_detail_page_risk_signal, requeue_failed_dispatch,
};

/// The station asks whether it may execute a bounded task. Published through
/// `/health`; the browser never hardcodes this path.
pub(super) const CLAIM_PATH: &str = "/api/local/dispatch/claim";

/// A successful claim whose browser work cannot start returns this local
/// execution failure before the frozen task may be retried.
pub(super) const FAILURE_PATH: &str = "/api/local/dispatch/failures";
pub(super) const DETAIL_PAGE_SESSION_GRANT_PATH: &str =
    "/api/local/dispatch/detail-page-sessions/grant";

/// 导航前登记通道交付身份的握手版本。
///
/// 采集服务先在 /health 通告它，插件确认后才在授权请求里带上同一个字符串。请求里没有这个
/// 字段的插件（旧版本）拿到的是与升级前完全相同的回答：服务端不会把「没有请求准备」
/// 当成「已经准备完成」。
pub(super) const DETAIL_PAGE_SESSION_LANE_PREPARATION_CONTRACT: &str =
    "linggan.detail-page-session.lane-preparation.v1";
pub(super) const DETAIL_PAGE_SESSION_NAVIGATION_PATH: &str =
    "/api/local/dispatch/detail-page-sessions/navigation-observed";
pub(super) const DETAIL_PAGE_RISK_SIGNAL_PATH: &str =
    "/api/local/dispatch/detail-page-sessions/risk-signals";

pub(super) const ACCOUNT_ELIGIBILITY_PATH: &str =
    "/api/local/stations/account-eligibility-observations";
pub(super) const CREDENTIAL_ACTIVATION_PATH: &str =
    "/api/local/stations/installation-credentials/activate";

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(CLAIM_PATH, post(claim))
        .route(FAILURE_PATH, post(failure))
        .route(
            DETAIL_PAGE_SESSION_GRANT_PATH,
            post(grant_detail_page_session_route),
        )
        .route(
            DETAIL_PAGE_SESSION_NAVIGATION_PATH,
            post(record_detail_page_session_navigation_route),
        )
        .route(
            DETAIL_PAGE_RISK_SIGNAL_PATH,
            post(report_detail_page_risk_signal_route),
        )
        .route(ACCOUNT_ELIGIBILITY_PATH, post(report_account))
        .route(CREDENTIAL_ACTIVATION_PATH, post(activate_credential))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetailPageSessionGrantBody {
    install_key: String,
    installation_credential: String,
    task_id: uuid::Uuid,
    grant_request_id: uuid::Uuid,
    execution_source_url: String,
    /// 插件确认过的握手版本；缺席就是 v1 插件，回答与升级前一致。
    lane_preparation_contract: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetailPageSessionNavigationBody {
    install_key: String,
    installation_credential: String,
    task_id: uuid::Uuid,
    session_ref: uuid::Uuid,
    kind: String,
    stop_reason: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DetailPageRiskSignalBody {
    install_key: String,
    installation_credential: String,
    task_id: uuid::Uuid,
    risk_signal_id: uuid::Uuid,
    detector_version: String,
}

/// Return a previously recorded page-session authorization only to the
/// installation that owns the still-live `content_detail` claim.  This route
/// never starts a browser, and an idempotent response is not evidence that
/// Chrome navigated.
async fn grant_detail_page_session_route(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<DetailPageSessionGrantBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "detail_page_session_grant_invalid",
        );
    };
    // 只有插件明确报出握手版本时才登记通道交付身份。认不出的版本按无效请求拒绝，
    // 不悄悄降级成 v1——那会让插件以为自己拿到了它根本不理解的准备回执。
    let lane_preparation_requested = match request.lane_preparation_contract.as_deref() {
        None => false,
        Some(contract) if contract == DETAIL_PAGE_SESSION_LANE_PREPARATION_CONTRACT => true,
        Some(_) => {
            return local_read_json_error(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "detail_page_session_lane_preparation_contract_unknown",
            );
        }
    };
    let grant = if lane_preparation_requested {
        grant_detail_page_session_with_lane_deliveries(
            database,
            &request.install_key,
            &request.installation_credential,
            request.task_id,
            request.grant_request_id,
            &request.execution_source_url,
        )
        .await
    } else {
        grant_detail_page_session(
            database,
            &request.install_key,
            &request.installation_credential,
            request.task_id,
            request.grant_request_id,
            &request.execution_source_url,
        )
        .await
    };
    // 准备回执与授权一起返回：插件在打开页面前把它写进本机持久状态。回执缺席就是
    // 「这次没有准备」，不是一个可以照常导航的默认值。
    let lane_preparation = |session_ref: uuid::Uuid, prepared_lanes: Vec<PreparedLaneDelivery>| {
        (!prepared_lanes.is_empty()).then(|| {
            serde_json::json!({
                "contractVersion": DETAIL_PAGE_SESSION_LANE_PREPARATION_CONTRACT,
                "sessionRef": session_ref,
                "lanes": prepared_lanes,
            })
        })
    };
    match grant {
        Ok(DetailPageSessionGrant::Authorized {
            session_ref,
            plan,
            prepared_lanes,
        }) => Json(serde_json::json!({
            "outcome": "authorized",
            "sessionRef": session_ref,
            "pageSessionPlan": plan,
            "lanePreparation": lane_preparation(session_ref, prepared_lanes),
        }))
        .into_response(),
        Ok(DetailPageSessionGrant::Replay {
            session_ref,
            plan,
            prepared_lanes,
        }) => Json(serde_json::json!({
            "outcome": "replay",
            "sessionRef": session_ref,
            "pageSessionPlan": plan,
            "lanePreparation": lane_preparation(session_ref, prepared_lanes),
        }))
        .into_response(),
        Ok(DetailPageSessionGrant::Suppressed {
            session_ref,
            reason_code,
        }) => Json(serde_json::json!({
            "outcome": "suppressed",
            "sessionRef": session_ref,
            "reasonCode": reason_code,
        }))
        .into_response(),
        Err(error) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            detail_page_session_grant_error_code(&error),
        ),
    }
}

fn detail_page_session_grant_error_code(error: &DetailPageSessionGrantError) -> &'static str {
    match error {
        DetailPageSessionGrantError::UpgradeRecoveryOnly => "collection_upgrade_recovery_only",
        DetailPageSessionGrantError::SchemaUnavailable => "detail_page_session_schema_unavailable",
        DetailPageSessionGrantError::UnknownInstallation => {
            "detail_page_session_installation_unknown"
        }
        DetailPageSessionGrantError::InvalidCredential => "detail_page_session_credential_invalid",
        DetailPageSessionGrantError::ClaimNotHeld => "detail_page_session_claim_not_held",
        DetailPageSessionGrantError::DetailScopeUnavailable => {
            "detail_page_session_scope_unavailable"
        }
        DetailPageSessionGrantError::ExecutionSourceUnavailable => {
            "detail_page_execution_source_unavailable"
        }
        DetailPageSessionGrantError::ExecutionSourceChanged => {
            "detail_page_execution_source_changed"
        }
        DetailPageSessionGrantError::LanePreparationUnavailable => {
            "detail_page_session_lane_preparation_unavailable"
        }
        DetailPageSessionGrantError::Database(_) => "detail_page_session_grant_write_failed",
    }
}

async fn report_detail_page_risk_signal_route(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<DetailPageRiskSignalBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "detail_page_risk_signal_invalid",
        );
    };
    match report_detail_page_risk_signal(
        database,
        &request.install_key,
        &request.installation_credential,
        request.task_id,
        request.risk_signal_id,
        &request.detector_version,
    )
    .await
    {
        Ok(receipt) => Json(serde_json::json!({
            "outcome": "recorded", "cooldownActive": receipt.cooldown_active,
            "cooldownUntil": receipt.cooldown_until, "consecutiveCount": receipt.consecutive_count,
        }))
        .into_response(),
        Err(error) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            match error {
                DetailPageRiskSignalError::SchemaUnavailable => {
                    "detail_page_risk_schema_unavailable"
                }
                DetailPageRiskSignalError::UnknownInstallation => {
                    "detail_page_risk_installation_unknown"
                }
                DetailPageRiskSignalError::InvalidCredential => {
                    "detail_page_risk_credential_invalid"
                }
                DetailPageRiskSignalError::ClaimNotHeld => "detail_page_risk_claim_not_held",
                DetailPageRiskSignalError::Database(_) => "detail_page_risk_write_failed",
            },
        ),
    }
}

/// Chrome reports the durable progress of its locally consumed session. This
/// route records observed navigation, pending delivery, or a terminal stop;
/// it is never an API that can request or replay a browser action.
async fn record_detail_page_session_navigation_route(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<DetailPageSessionNavigationBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "detail_page_session_navigation_invalid",
        );
    };
    let progress = match detail_page_session_progress(&request) {
        Some(progress) => progress,
        None => {
            return local_read_json_error(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "detail_page_session_progress_invalid",
            );
        }
    };
    match record_detail_page_session_progress(
        database,
        &request.install_key,
        &request.installation_credential,
        request.task_id,
        request.session_ref,
        progress,
    )
    .await
    {
        Ok(()) => Json(serde_json::json!({"outcome":"recorded"})).into_response(),
        Err(error) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            detail_page_session_navigation_error_code(&error),
        ),
    }
}

fn detail_page_session_progress(
    request: &DetailPageSessionNavigationBody,
) -> Option<DetailPageSessionProgress> {
    match request.kind.as_str() {
        "navigation_observed" if request.stop_reason.is_none() => {
            Some(DetailPageSessionProgress::NavigationObserved)
        }
        "delivery_pending" if request.stop_reason.is_none() => {
            Some(DetailPageSessionProgress::DeliveryPending)
        }
        "stopped" => match request.stop_reason.as_deref() {
            Some("navigation_state_unknown") => Some(DetailPageSessionProgress::Stopped {
                reason: "navigation_state_unknown",
            }),
            Some("page_unavailable") => Some(DetailPageSessionProgress::Stopped {
                reason: "page_unavailable",
            }),
            Some("risk_stop") => Some(DetailPageSessionProgress::Stopped {
                reason: "risk_stop",
            }),
            Some("owner_unavailable") => Some(DetailPageSessionProgress::Stopped {
                reason: "owner_unavailable",
            }),
            Some("delivery_terminal") => Some(DetailPageSessionProgress::Stopped {
                reason: "delivery_terminal",
            }),
            _ => None,
        },
        _ => None,
    }
}

fn detail_page_session_navigation_error_code(
    error: &DetailPageSessionNavigationError,
) -> &'static str {
    match error {
        DetailPageSessionNavigationError::SchemaUnavailable => {
            "detail_page_session_schema_unavailable"
        }
        DetailPageSessionNavigationError::UnknownInstallation => {
            "detail_page_session_installation_unknown"
        }
        DetailPageSessionNavigationError::InvalidCredential => {
            "detail_page_session_credential_invalid"
        }
        DetailPageSessionNavigationError::SessionNotHeld => "detail_page_session_not_held",
        DetailPageSessionNavigationError::Database(_) => {
            "detail_page_session_navigation_write_failed"
        }
    }
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountEligibilityBody {
    installation_ref: uuid::Uuid,
    installation_credential: String,
    #[serde(default)]
    task_id: Option<uuid::Uuid>,
    observation: AccountEligibilityObservationBody,
}

/// This is a wire-only shape. `as_evidence_observation` below validates it into the closed
/// evidence type before any database work starts. Separating those two steps is necessary
/// because an internally tagged JSON enum otherwise silently ignores a field from another
/// variant on some serde paths.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountEligibilityObservationBody {
    signal: AccountEligibilitySignalWire,
    #[serde(default)]
    raw_platform_account_id: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum AccountEligibilitySignalWire {
    AuthenticatedObserved,
    CooldownObserved,
    LoginRequired,
    AccessRestricted,
}

impl AccountEligibilityObservationBody {
    /// Validation closes the only field cross-product JSON cannot express in one tagged enum:
    /// positive means exactly one identity, and explicit negatives mean none.
    fn as_evidence_observation(&self) -> Result<AccountEligibilityObservation<'_>, ()> {
        match (&self.signal, self.raw_platform_account_id.as_deref()) {
            (
                AccountEligibilitySignalWire::AuthenticatedObserved,
                Some(raw_platform_account_id),
            ) => Ok(AccountEligibilityObservation::Authenticated {
                raw_platform_account_id,
            }),
            (AccountEligibilitySignalWire::AuthenticatedObserved, None) => Err(()),
            (AccountEligibilitySignalWire::CooldownObserved, None) => {
                Ok(AccountEligibilityObservation::ExplicitBlock(
                    ExplicitAccountEligibilitySignal::CooldownObserved,
                ))
            }
            (AccountEligibilitySignalWire::LoginRequired, None) => {
                Ok(AccountEligibilityObservation::ExplicitBlock(
                    ExplicitAccountEligibilitySignal::LoginRequired,
                ))
            }
            (AccountEligibilitySignalWire::AccessRestricted, None) => {
                Ok(AccountEligibilityObservation::ExplicitBlock(
                    ExplicitAccountEligibilitySignal::AccessRestricted,
                ))
            }
            (_, Some(_)) => Err(()),
        }
    }
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
    let Ok(request) = serde_json::from_slice::<AccountEligibilityBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "account_eligibility_observation_invalid",
        );
    };
    let Ok(observation) = request.observation.as_evidence_observation() else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "account_eligibility_observation_invalid",
        );
    };
    let digest_key = state.account_digest_key.as_deref().map(Vec::as_slice);
    let reported = match request.task_id {
        Some(task_id) => {
            report_claimed_task_account_eligibility(
                database,
                request.installation_ref,
                &request.installation_credential,
                task_id,
                observation,
                digest_key,
            )
            .await
        }
        None => {
            report_account_eligibility(
                database,
                request.installation_ref,
                &request.installation_credential,
                observation,
                digest_key,
            )
            .await
        }
    };
    match reported {
        Ok(receipt) => Json(serde_json::json!({
            "outcome": "observed",
            "installationRef": receipt.installation_ref,
            "accountRef": receipt.account_ref,
            "eligibilityState": receipt.state.as_str(),
            "bindingRequired": receipt.binding_required,
            "bindingMismatch": receipt.binding_mismatch,
            "frozenAccountMismatch": receipt.frozen_account_mismatch,
            "accountBusy": receipt.account_busy,
        }))
        .into_response(),
        Err(CollectionControlError::InvalidCredential) => local_read_json_error(
            axum::http::StatusCode::UNAUTHORIZED,
            "installation_credential_invalid",
        ),
        Err(CollectionControlError::ClaimedTaskNotHeld) => local_read_json_error(
            axum::http::StatusCode::CONFLICT,
            "account_observation_claim_not_held",
        ),
        Err(CollectionControlError::MissingDigestKey) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "account_identity_key_missing",
        ),
        Err(CollectionControlError::SchemaUnavailable) => local_read_json_error(
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
        Ok(decision) => {
            // 把刚才给出的回答留在工位上，执行工位页才说得出「现在为什么不动」。
            // 只供显示：写不进去也不改变这次派发的结果，因此这里不把它变成一个错误响应。
            let _ = record_dispatch_answer(database, &request.install_key, &decision).await;
            Json(payload(&decision)).into_response()
        }
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
        DispatchFailureError::DetailPageSourceUnbound => "dispatch_failure_source_unbound",
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
            payload["reason"] =
                serde_json::json!(if reason_code == "collection_upgrade_recovery_only" {
                    "采集恢复阶段：暂停新访问，已采集数据继续交付。"
                } else {
                    "当前控制资格已变化，任务保持等待。"
                });
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALLATION_REF: &str = "11111111-1111-4111-8111-111111111111";

    fn parse(body: serde_json::Value) -> Result<AccountEligibilityBody, serde_json::Error> {
        serde_json::from_value(body)
    }

    #[test]
    fn account_observation_wire_contract_is_closed_and_nested() {
        let request = parse(serde_json::json!({
            "installationRef": INSTALLATION_REF,
            "installationCredential": "fixture-credential",
            "observation": {
                "signal": "authenticated_observed",
                "rawPlatformAccountId": "current-account"
            }
        }))
        .expect("a positive observation carries its identity inside the variant");
        assert!(matches!(
            request.observation.as_evidence_observation(),
            Ok(AccountEligibilityObservation::Authenticated {
                raw_platform_account_id: "current-account"
            })
        ));

        let negative = parse(serde_json::json!({
            "installationRef": INSTALLATION_REF,
            "installationCredential": "fixture-credential",
            "observation": { "signal": "login_required" }
        }))
        .expect("an explicit negative fact has no identity field");
        assert!(matches!(
            negative.observation.as_evidence_observation(),
            Ok(AccountEligibilityObservation::ExplicitBlock(
                ExplicitAccountEligibilitySignal::LoginRequired
            ))
        ));
    }

    #[test]
    fn account_observation_wire_contract_rejects_legacy_and_crossed_payloads() {
        for invalid in [
            // The old flat format would let signal and identity drift independently.
            serde_json::json!({
                "installationRef": INSTALLATION_REF,
                "installationCredential": "fixture-credential",
                "signal": "authenticated_observed",
                "rawPlatformAccountId": "current-account"
            }),
            // A negative fact must never create or select an arbitrary account record.
            serde_json::json!({
                "installationRef": INSTALLATION_REF,
                "installationCredential": "fixture-credential",
                "observation": {
                    "signal": "login_required",
                    "rawPlatformAccountId": "current-account"
                }
            }),
            // A positive fact cannot omit the identity that makes it meaningful.
            serde_json::json!({
                "installationRef": INSTALLATION_REF,
                "installationCredential": "fixture-credential",
                "observation": { "signal": "authenticated_observed" }
            }),
            // Future accidental fields require an explicit contract change and a version bump.
            serde_json::json!({
                "installationRef": INSTALLATION_REF,
                "installationCredential": "fixture-credential",
                "observation": { "signal": "cooldown_observed", "unexpected": true }
            }),
        ] {
            let parsed = parse(invalid);
            assert!(match parsed.as_ref() {
                Err(_) => true,
                Ok(request) => request.observation.as_evidence_observation().is_err(),
            });
        }
    }
}
