//! HTTP boundary tests run without a database; a storage fallback would produce 503, not 400.
use super::*;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request};
use linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF;
use serde_json::Value;
use std::{collections::BTreeSet, sync::Arc};
use tower::ServiceExt;

pub(super) fn state(database: LocalDatabaseState) -> LocalWebState {
    LocalWebState {
        database,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    }
}

pub(super) fn app(database: LocalDatabaseState) -> Router {
    routes().with_state(state(database))
}

pub(super) fn command() -> Value {
    json!({"requestRef":Uuid::new_v4(),"domainRef":ADHD_DOMAIN_REF,"policyRef":Uuid::new_v4(),
        "scope":{"kind":"works","workRefs":[Uuid::new_v4()]},"mode":"new_only",
        "limits":{"commentBudget":100,"contextCharacterBudget":6000,"tokenLimit":100000},"reason":null})
}

pub(super) async fn send(application: Router, path: &str, value: Value) -> (StatusCode, Value) {
    let response = application
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header(header::HOST, "localhost:3000")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_headers(response.headers());
    let status = response.status();
    let body =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    (status, body)
}

fn assert_headers(headers: &HeaderMap) {
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
}

#[test]
fn exhausted_pair_requests_have_a_truthful_ui_state_label() {
    let script = include_str!("comment_study.js");
    assert!(script.contains("failed: '比较未完成，请查看原因'"));
    assert!(script.contains("attempts_exhausted: '请求尝试次数已用尽，未收到可接纳结果'"));
    assert!(script.contains("budget_exhausted: '运行预算已用尽，比较未完成'"));
    assert!(script.contains("input_limit_exceeded: '请求超过模型输入上限，未发送'"));
}

#[test]
fn run_controls_use_cas_receipts_and_confirmation_for_irreversible_stop() {
    let html = include_str!("comment_study.html");
    let script = include_str!("comment_study.js");
    assert!(html.contains("id=\"study-stop-dialog\""));
    assert!(html.contains("id=\"study-stop-run-identity\""));
    assert!(html.contains("aria-describedby=\"study-stop-run-identity study-stop-description\""));
    assert!(html.contains("已经开始的调用无法保证中断，可能仍产生费用"));
    assert!(script.contains("dialog.showModal()"));
    assert!(script.contains("run.pendingCount == null ? '未知' : Number(run.pendingCount)"));
    assert!(script.contains("data-pending-count=\"${pendingCount}\""));
    assert!(!script.contains("run.pendingCount || 0"));
    assert!(script.contains("Run ${stopDialogRunRef} · 当前未终态"));
    assert!(script.contains("data-run-control=\"pause\""));
    assert!(script.contains("data-run-control=\"resume\""));
    assert!(script.contains("runs/${encodeURIComponent(runRef)}/${action}"));
    assert!(script.contains("JSON.stringify({ expectedControlVersion })"));
    assert!(script.contains("receipt.data?.runRef !== runRef"));
    assert!(script.contains("receipt.data?.controlVersion) !== expectedControlVersion + 1"));
    assert!(script.contains("pause: 'user_paused', resume: null, stop: 'user_stopped'"));
    assert!(script.contains("await renderActiveTab()"));
    assert!(script.contains("runControlOutcomeNeedsRefresh"));
    assert!(script.contains("运行列表刷新失败"));
    assert!(script.contains("命令结果未获确认，最新运行状态未重新读取"));
    assert!(script.contains("if (nextControl) nextControl.focus()"));
    assert!(script.contains("else document.querySelector('#study-run-control-feedback')?.focus()"));
    assert!(script.contains("请核对后再操作。"));
    assert!(!script.contains("/cancel"));
    assert!(!script.contains("window.confirm("));
    assert!(!script.contains("run.dispatchState = 'stopped'"));
}

#[test]
fn run_control_request_is_closed_and_requires_a_nonnegative_version() {
    assert!(serde_json::from_value::<StudyRunControlCommand>(json!({})).is_err());
    assert!(
        serde_json::from_value::<StudyRunControlCommand>(json!({
            "expectedControlVersion":0
        }))
        .unwrap()
        .validate()
        .is_ok()
    );
    assert!(
        serde_json::from_value::<StudyRunControlCommand>(json!({
            "expectedControlVersion":-1
        }))
        .unwrap()
        .validate()
        .is_err()
    );
    assert!(
        serde_json::from_value::<StudyRunControlCommand>(json!({
            "expectedControlVersion":0,"domainRef":Uuid::new_v4()
        }))
        .is_err()
    );
}

#[tokio::test]
async fn run_control_routes_validate_closed_body_path_and_query_before_database_access() {
    let path = format!("/api/local/comment-study/runs/{}/pause", Uuid::new_v4());
    for value in [
        json!({}),
        json!({"expectedControlVersion":0,"domainRef":Uuid::new_v4()}),
        json!({"expectedControlVersion":-1}),
    ] {
        let (status, body) = send(app(LocalDatabaseState::NotConfigured), &path, value).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request");
    }
    let (status, body) = send(
        app(LocalDatabaseState::NotConfigured),
        &path,
        json!({"expectedControlVersion":0}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "catalog_unavailable");
    let (status, body) = send(
        app(LocalDatabaseState::NotConfigured),
        &format!("{path}?domainRef={}", Uuid::new_v4()),
        json!({"expectedControlVersion":0}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[test]
fn activation_expected_pointer_is_required_nullable_and_closed() {
    assert!(serde_json::from_value::<ActivateStudyPolicyCommand>(json!({})).is_err());
    let value: ActivateStudyPolicyCommand =
        serde_json::from_value(json!({"expectedActivePolicyRef":null})).unwrap();
    assert!(value.expected_active_policy_ref.is_none());
    assert!(value.validate().is_ok());
    assert!(
        serde_json::from_value::<ActivateStudyPolicyCommand>(
            json!({"expectedActivePolicyRef":null,"origin":"manual"})
        )
        .is_err()
    );
    let invalid: ActivateStudyPolicyCommand =
        serde_json::from_value(json!({"expectedActivePolicyRef":Uuid::nil()})).unwrap();
    assert!(invalid.validate().is_err());
}

#[tokio::test]
async fn origin_guard_precedes_json_and_storage_on_every_command() {
    let activate = format!(
        "/api/local/comment-study/policies/{}/activate",
        Uuid::new_v4()
    );
    let cancel = format!("/api/local/comment-study/runs/{}/cancel", Uuid::new_v4());
    let pause = format!("/api/local/comment-study/runs/{}/pause", Uuid::new_v4());
    let resume = format!("/api/local/comment-study/runs/{}/resume", Uuid::new_v4());
    let stop = format!("/api/local/comment-study/runs/{}/stop", Uuid::new_v4());
    for path in [
        "/api/local/comment-study/runs",
        cancel.as_str(),
        pause.as_str(),
        resume.as_str(),
        stop.as_str(),
        "/api/local/comment-study/selection-preview",
        activate.as_str(),
        "/api/local/comment-study/policy",
    ] {
        for (host, origin, site) in [
            (
                "example.invalid:3000",
                "http://example.invalid:3000",
                "same-origin",
            ),
            ("localhost:3000", "https://other.invalid", "cross-site"),
            ("localhost:3000", "http://localhost:3001", "same-site"),
            ("localhost:3000", "http://localhost:3000", "cross-site"),
        ] {
            let response = app(LocalDatabaseState::NotConfigured)
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(path)
                        .header(header::HOST, host)
                        .header(header::ORIGIN, origin)
                        .header("sec-fetch-site", site)
                        .body(Body::from("not-json-or-source-text"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            assert_headers(response.headers());
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap())
                    .unwrap();
            assert_eq!(body["error"]["code"], "local_access_denied");
            assert!(body["requestRef"].is_null());
            assert!(!body.to_string().contains("not-json"));
        }
    }
}

#[tokio::test]
async fn malformed_json_wrong_media_type_and_oversized_body_are_safe_400() {
    for (media, body) in [
        ("text/plain", "{}".to_owned()),
        ("application/json", "{".to_owned()),
        ("application/json", "x".repeat(2 * 1024 * 1024 + 1)),
    ] {
        let response = app(LocalDatabaseState::NotConfigured)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/local/comment-study/runs")
                    .header(header::HOST, "localhost:3000")
                    .header(header::CONTENT_TYPE, media)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_headers(response.headers());
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(body["error"]["code"], "invalid_request");
    }
}

#[tokio::test]
async fn closed_start_contract_rejects_legacy_and_forged_commands_before_storage() {
    let mut forged = command();
    forged["origin"] = json!("scheduled");
    let mut missing = command();
    missing.as_object_mut().unwrap().remove("reason");
    let mut scope = command();
    scope["scope"]["commentKeys"] = json!([]);
    for value in [
        json!({"contentPublicRefs":[Uuid::new_v4()]}),
        forged,
        missing,
        scope,
    ] {
        let (status, body) = send(
            app(LocalDatabaseState::NotConfigured),
            "/api/local/comment-study/runs",
            value,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request");
    }
}

#[tokio::test]
async fn domain_limits_and_query_are_not_silently_defaulted() {
    for (field, value, code) in [
        ("domainRef", json!(Uuid::nil()), "unsupported_domain"),
        (
            "limits",
            json!({"commentBudget":0,"contextCharacterBudget":6000,"tokenLimit":100}),
            "invalid_limit",
        ),
    ] {
        let mut input = command();
        input[field] = value;
        let expected = input["requestRef"].clone();
        let (status, body) = send(
            app(LocalDatabaseState::NotConfigured),
            "/api/local/comment-study/runs",
            input,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["requestRef"], expected);
    }
    let (status, _) = send(
        app(LocalDatabaseState::NotConfigured),
        "/api/local/comment-study/runs?origin=scheduled",
        command(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn preview_has_no_request_identity_and_activation_validates_path_before_storage() {
    let input: StartStudyRunCommand = serde_json::from_value(command()).unwrap();
    let (_, valid) = send(
        app(LocalDatabaseState::NotConfigured),
        "/api/local/comment-study/selection-preview",
        json!(input.preview()),
    )
    .await;
    assert_eq!(valid["error"]["code"], "catalog_unavailable");
    let mut forged = json!(input.preview());
    forged["requestRef"] = json!(Uuid::new_v4());
    let (status, _) = send(
        app(LocalDatabaseState::NotConfigured),
        "/api/local/comment-study/selection-preview",
        forged,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = send(
        app(LocalDatabaseState::NotConfigured),
        "/api/local/comment-study/policies/not-a-uuid/activate",
        json!({"expectedActivePolicyRef":null}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn cancel_contract_is_domain_scoped_closed_and_idempotent_at_the_http_boundary() {
    let path = format!("/api/local/comment-study/runs/{}/cancel", Uuid::new_v4());
    for body in [
        json!({}),
        json!({"domain_ref":Uuid::nil()}),
        json!({"domain_ref":ADHD_DOMAIN_REF,"domainRef":ADHD_DOMAIN_REF}),
    ] {
        let (status, body) = send(app(LocalDatabaseState::NotConfigured), &path, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request");
    }
    let (status, body) = send(
        app(LocalDatabaseState::NotConfigured),
        &format!("/api/local/comment-study/runs/not-a-uuid/cancel"),
        json!({"domain_ref":ADHD_DOMAIN_REF}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");

    let (status, body) = send(
        app(LocalDatabaseState::NotConfigured),
        &path,
        json!({"domain_ref":ADHD_DOMAIN_REF}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "catalog_unavailable");

    let (status, body) = send(
        app(LocalDatabaseState::NotConfigured),
        &format!("{path}?domain={ADHD_DOMAIN_REF}"),
        json!({"domain_ref":ADHD_DOMAIN_REF}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn safe_errors_preserve_identity_retry_semantics_and_closed_shape() {
    let reference = Uuid::new_v4();
    for (cause, status, code, retryable) in [
        (
            StudyStartError::IdempotencyConflict,
            StatusCode::CONFLICT,
            "idempotency_conflict",
            false,
        ),
        (
            StudyStartError::Policy(StudyPolicyStoreError::ActiveConflict),
            StatusCode::CONFLICT,
            "control_version_conflict",
            false,
        ),
        (
            StudyStartError::SchemaUnavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "study_schema_unavailable",
            false,
        ),
        (
            StudyStartError::Database(sqlx::Error::Protocol("PRIVATE-DSN-RAW-TEXT".into())),
            StatusCode::SERVICE_UNAVAILABLE,
            "catalog_unavailable",
            true,
        ),
        (
            StudyStartError::Busy,
            StatusCode::CONFLICT,
            "study_busy",
            true,
        ),
    ] {
        let response = failure(cause, Some(reference));
        assert_eq!(response.status(), status);
        let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("PRIVATE-DSN"));
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body.as_object().unwrap().len(), 2);
        assert_eq!(body["error"].as_object().unwrap().len(), 4);
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["retryable"], retryable);
        assert_eq!(body["error"]["details"], json!({}));
        assert_eq!(body["requestRef"], json!(reference));
    }
}

#[tokio::test]
async fn database_failure_codes_do_not_expose_sql_or_fabricate_empty_results() {
    for (sqlstate, status, code) in [
        ("55P03", 409, "study_busy"),
        ("40P01", 409, "study_busy"),
        ("57014", 503, "query_timeout"),
        ("42703", 503, "study_schema_unavailable"),
        ("XXXXX", 503, "catalog_unavailable"),
    ] {
        let response = database_failure(Some(sqlstate), None);
        assert_eq!(response.status().as_u16(), status);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(body["error"]["code"], code);
        assert!(body.get("items").is_none());
    }
}

#[tokio::test]
async fn live_router_uses_the_p2_command_surface_and_retires_the_old_policy_write() {
    let (status, body) = send(
        app(LocalDatabaseState::NotConfigured),
        "/api/local/comment-study/policy",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
    assert_eq!(body["error"]["code"], "policy_endpoint_retired");
    let live = crate::local_web::comment_study::routes()
        .with_state(state(LocalDatabaseState::NotConfigured));
    let response = live
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/comment-study/selection-preview")
                .header(header::HOST, "localhost:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let (status, body) = send(
        live,
        "/api/local/comment-study/runs",
        json!({"contentPublicRefs":[Uuid::new_v4()]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
}
