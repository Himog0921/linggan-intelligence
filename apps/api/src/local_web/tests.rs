const LIDS_TOKENS: &str = include_str!("../../../web/src/styles/lids_tokens.css");
const SHELL_CSS: &str = include_str!("../../../web/src/styles/shell.css");
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use sqlx::Row;
use std::collections::BTreeMap;
use tower::ServiceExt;

const LOCAL_001_MIGRATIONS: &str = full_schema_fixture::FULL_MIGRATIONS;

#[test]
fn recovery_control_read_failure_still_blocks_the_runtime_page() {
    let recovery_lanes = collection::collection_control_surface_view::recovery_runtime_lanes();
    let control = runtime_control_for_page(None, Some(&recovery_lanes), false)
        .expect("the known recovery gate supplies a control projection");
    assert_eq!(control.lanes.len(), 3);
    assert!(control.lanes.iter().all(|lane| {
        !lane.available
            && !lane.queueable
            && lane.reason_code == Some("collection_upgrade_recovery_only")
    }));
    assert!(runtime_control_for_page(None, None, false).is_none());
}

#[tokio::test]
async fn legacy_evidence_route_cannot_bypass_domain_scoped_work_resources() {
    for uri in [
        "/api/local/evidence-library/legacy",
        "/api/local/evidence-library/legacy?domain=00000000-0000-4000-8000-000000000001",
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(header::HOST, "127.0.0.1:8080")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn evidence_library_rejects_unknown_url_filters_before_reading() {
    let domain = "00000000-0000-4000-8000-000000000001";
    let invalid = app()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/local/work-resources?domain={domain}&creatorKye=typo"
                ))
                .header(header::HOST, "127.0.0.1:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let valid = app().oneshot(Request::builder()
        .uri(format!("/api/local/work-resources?domain={domain}&creatorKey=xhs%3A41&creatorFilter=%7B%22domain%22%3A%22{domain}%22%7D&returnTo=%2Fcorpus%2Fcreators%3Fdomain%3D{domain}"))
        .header(header::HOST, "127.0.0.1:3000")
        .body(Body::empty()).unwrap()).await.unwrap();
    assert_ne!(valid.status(), StatusCode::BAD_REQUEST);
    let parsed: material_projection::EvidenceLibraryParams = serde_json::from_value(json!({
        "domain": domain,
        "creatorKey": "xhs:41",
        "creatorFilter": format!(r#"{{"domain":"{domain}"}}"#),
        "returnTo": format!("/corpus/creators?domain={domain}"),
    }))
    .unwrap();
    assert!(material_projection::local_query(&parsed).is_ok());
}

#[test]
fn lease_refusal_keeps_the_paused_domain_reason() {
    assert_eq!(
        lease_error_code(&LeaseError::ControlBlocked {
            reason_code: "domain_paused_or_unscoped".to_owned(),
        }),
        "domain_paused_or_unscoped"
    );
}

#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod domain_management_fixture;

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn domain_management_page_queries_real_lane_receipts_without_english_or_false_zero() {
    let database = domain_management_fixture::proof_database("domain_page_lane_receipts").await;
    let external_id = "domain-page-lane-proof";
    let package_ref = domain_management_fixture::submit_package(
        &database,
        "profile_discovery",
        serde_json::json!({"authorExternalId":"domain-page-author"}),
        serde_json::json!({
            "kind":"profile_discovery_card",
            "resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"title":"领域页通道回执证明"}
        }),
    )
    .await;
    let content_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .expect("the submitted producer record creates canonical material");
    sqlx::query(
        "INSERT INTO linggan_material_domain_usage \
            (usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) \
         VALUES ($1,$2,'00000000-0000-4000-8000-000000000001','primary', \
                 'legacy_domain_migration',$3)",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(content_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await
    .expect("the proof attaches an explicit legacy Domain usage to accepted material");
    let discovery_facts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_discovery_finding \
         WHERE package_ref=$1 AND content_public_ref=$2",
    )
    .bind(package_ref)
    .bind(content_ref)
    .fetch_one(database.pool())
    .await
    .expect("the accepted package retains its typed discovery fact");
    assert_eq!(discovery_facts, 1);

    let response = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri(
                    "/api/collection/workspace/domains?domain=00000000-0000-4000-8000-000000000001",
                )
                .header(header::HOST, "127.0.0.1:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    let domain = result["domains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["domainRef"] == "00000000-0000-4000-8000-000000000001")
        .unwrap();
    let lanes = domain["lanes"].as_array().unwrap();
    assert!(
        lanes
            .iter()
            .any(|lane| lane["label"] == "发现" && lane["summary"] == "可检索 1 篇")
    );
    assert!(
        lanes
            .iter()
            .any(|lane| lane["label"] == "评论" && lane["summary"] == "状态未知")
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn retired_policy_endpoint_cannot_write_even_for_a_paused_domain() {
    let database = domain_management_fixture::proof_database("comment_policy_paused_domain").await;
    sqlx::raw_sql(include_str!(
        "../../../../database/bootstrap/comment-study-001.sql"
    ))
    .execute(database.pool())
    .await
    .expect("the Comment Study derived schema is initialized for the API route proof");
    let domain_ref = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    sqlx::query("UPDATE observation_domain SET status='paused' WHERE domain_ref=$1")
        .bind(domain_ref)
        .execute(database.pool())
        .await
        .expect("the test Domain is paused before the policy write");
    let model_config_ref = uuid::Uuid::new_v4();
    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/comment-study/policy")
                .header(header::HOST, "127.0.0.1:8080")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "domainRef":domain_ref,
                        "modelConfigRef":model_config_ref,
                        "commentBudget":100,
                        "contextCharacterBudget":12000
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["error"]["code"], "policy_endpoint_retired");
    let policy_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_study_policy \
         WHERE domain_ref=$1 AND model_config_ref=$2",
    )
    .bind(domain_ref)
    .bind(model_config_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(policy_count, 0, "a paused Domain retains no new policy");
}

#[test]
fn check_in_payload_returns_the_server_canonical_station_name_and_pause_state() {
    let station_ref = uuid::Uuid::new_v4();
    let payload = check_in_payload(&CheckInOutcome::Heartbeat {
        installation_ref: uuid::Uuid::new_v4(),
        station_ref: Some(station_ref),
        station_display_name: Some("本机 Chrome".to_owned()),
        accepting_tasks: Some(false),
        credential: None,
    });
    assert_eq!(
        payload.pointer("/stationRef"),
        Some(&serde_json::Value::String(station_ref.to_string()))
    );
    assert_eq!(
        payload.pointer("/stationDisplayName"),
        Some(&serde_json::Value::String("本机 Chrome".to_owned()))
    );
    assert_eq!(
        payload.pointer("/stationAccepting"),
        Some(&serde_json::Value::Bool(false))
    );
}

#[tokio::test]
async fn health_route_returns_machine_readable_local_state() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload.pointer("/routes/localProducer"),
        Some(&serde_json::Value::Null),
        "a non-ready local host must not publish a full Producer delivery bundle"
    );
    assert_eq!(
        payload.pointer("/readiness/state").and_then(Value::as_str),
        Some("not_configured"),
        "没给数据库地址是配置错误，与「连不上」不是同一件事：{payload}"
    );
    assert_eq!(payload.pointer("/readiness/detail"), Some(&Value::Null));
    assert_eq!(
        payload.pointer("/readiness/checkedAt"),
        Some(&Value::Null),
        "问不到数据库时钟就不编造判定时刻"
    );
}

#[tokio::test]
async fn health_readiness_separates_an_unreachable_database_from_a_missing_configuration() {
    let application = router(LocalWebState {
        database: LocalDatabaseState::DatabaseUnavailable,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    });
    let payload = health_payload(application).await;

    assert_eq!(
        payload.pointer("/readiness/state").and_then(Value::as_str),
        Some("database_unreachable"),
        "连不上是环境问题，等它回来就会好：{payload}"
    );
    assert_eq!(
        payload.pointer("/readiness/detail").and_then(Value::as_str),
        Some("connect_failed"),
        "detail 只写类别，不带连接串"
    );
    assert_eq!(payload.pointer("/readiness/checkedAt"), Some(&Value::Null));
}

#[tokio::test]
async fn local_entry_redirects_to_the_login_page() {
    let response = app()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response.headers().get(header::LOCATION).unwrap(), "/login");
}

#[test]
fn runtime_token_source_matches_the_full_lids_baseline() {
    let runtime = declared_token_values(LIDS_TOKENS);
    let documented = declared_token_values(LIDS_TOKEN_DOCUMENT);

    assert_eq!(runtime.len(), 186);
    assert_eq!(documented.len(), 186);
    assert_eq!(runtime, documented);
    assert!(declared_token_values(SHELL_CSS).is_empty());
}

#[test]
fn default_local_query_is_latest_accepted_discovery_and_explicit_windows_remain_published_only() {
    let default = material_projection::local_query(&material_projection::EvidenceLibraryParams {
        creator_key: None,
        creator_filter: None,
        _return_to: None,
        public_refs: None,
        cursor: None,
        q: None,
        window: None,
        sort: None,
        lane: None,
        lane_state: None,
        media_kind: None,
        restriction: None,
        domain: None,
        collection_run_ref: None,
    })
    .expect("an omitted URL window selects the explicit default discovery view");
    assert_eq!(
        default.time_view(),
        linggan_contracts::EvidenceTimeView::LatestAcceptedDiscovery
    );
    assert_eq!(default.published_window(), None);

    let explicit = material_projection::local_query(&material_projection::EvidenceLibraryParams {
        creator_key: None,
        creator_filter: None,
        _return_to: None,
        public_refs: None,
        cursor: None,
        q: None,
        window: Some("last_30_days".to_owned()),
        sort: None,
        lane: None,
        lane_state: None,
        media_kind: None,
        restriction: None,
        domain: None,
        collection_run_ref: None,
    })
    .expect("an explicit published window remains valid");
    assert_eq!(
        explicit.time_view(),
        linggan_contracts::EvidenceTimeView::PublishedLast30Days
    );
    assert_eq!(
        explicit.published_window(),
        Some(linggan_contracts::PublishedWindow::Last30Days)
    );
}

#[test]
fn resumable_media_temp_bytes_are_never_published_until_the_final_promotion() {
    let root = std::env::temp_dir().join(format!("linggan-media-helper-{}", uuid::Uuid::new_v4()));
    let temporary = root.join("uploads/fixture/bytes.part");
    let final_path = root.join("blobs/88/fixture");

    write_media_chunk(&temporary, 0, b"ab").expect("first synthetic chunk writes");
    assert!(
        !final_path.exists(),
        "an interrupted upload has no public blob path"
    );
    assert!(
        write_media_chunk(&temporary, 0, b"cd").is_err(),
        "offset replay cannot overwrite the partial file"
    );
    write_media_chunk(&temporary, 2, b"cd").expect("second synthetic chunk resumes exactly");
    assert!(
        atomically_promote_media_upload(&temporary, &final_path).expect("final promotion succeeds")
    );
    assert_eq!(
        std::fs::read(&final_path).expect("published synthetic bytes"),
        b"abcd"
    );
    std::fs::remove_dir_all(root).expect("synthetic media root cleans up");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn loopback_rejects_conflicting_quota_reached_and_keeps_truthful_partial_cards() {
    let database = proof_database("local_api_quota_consistency").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);
    let conflicting = discovery_package(&observed_at).replace(
        "\"stoppedReason\":\"risk_control\"",
        "\"stoppedReason\":\"quota_reached\"",
    );

    let response = post_discovery_package(application.clone(), conflicting).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("\"admission\":\"not_accepted\""));

    let response = post_discovery_package(application, discovery_package(&observed_at)).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn loopback_local_producer_acknowledges_one_partial_package_and_replays_timeout_submission() {
    let database = proof_database("local_api_producer").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let application = app_with_database(database);
    let task = local_task_spec();
    let attempt = local_attempt();
    let submission = local_submission(&observed_at);
    for (path, body, expected) in [
        (
            "/api/local/producer/manual-tasks",
            task.clone(),
            "\"outcome\":\"created\"",
        ),
        (
            "/api/local/producer/attempts",
            attempt.clone(),
            "\"outcome\":\"started\"",
        ),
        (
            "/api/local/producer/submissions",
            submission.clone(),
            "\"delivery\":\"acknowledged\"",
        ),
        (
            "/api/local/producer/submissions",
            submission,
            "\"delivery\":\"replay\"",
        ),
    ] {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains(expected));
    }
    let conflicting_attempt = attempt.replace(
        "22222222-2222-4222-8222-222222222222",
        "88888888-8888-4888-8888-888888888888",
    );
    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/attempts")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(conflicting_attempt))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("attempt_identity_conflict"));
    let terminal_conflict = local_submission(&observed_at)
        .replace(
            "44444444-4444-4444-8444-444444444444",
            "55555555-5555-4555-8555-555555555555",
        )
        .replace("note-api-known", "note-api-conflict");
    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(terminal_conflict))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("attempt_terminal_submission_conflict"));
    let invalid_scheduler = task.replace("\"manual\"", "\"scheduler\"");
    let response = application
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/manual-tasks")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(invalid_scheduler))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

/// `/health` 的就绪回答与 worker 用的是同一套判据（`runtime_readiness`），同一次故障在这里
/// 和在 worker 日志里必须是同一句话。注入用「台账少一行」：迁移文件全部应用过，只是记账少了
/// `0034`。这类故障最容易糊过去——库是好的、表都在，只有记账不齐。
#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn health_readiness_speaks_the_same_verdict_as_the_worker() {
    let database = proof_database("local_api_readiness").await;
    let application = app_with_database(database.clone());

    let ready = health_payload(application.clone()).await;
    assert_eq!(
        ready.pointer("/readiness/state").and_then(Value::as_str),
        Some("ready"),
        "迁移与表都齐就该判成可接活：{ready}"
    );
    assert_eq!(ready.pointer("/readiness/detail"), Some(&Value::Null));
    let checked_at = ready
        .pointer("/readiness/checkedAt")
        .and_then(Value::as_str)
        .expect("连得上就有数据库时钟给的判定时刻");
    assert!(
        checked_at.ends_with('Z'),
        "判定时刻用数据库时钟：{checked_at}"
    );

    sqlx::query(
        "DELETE FROM linggan_local_schema_migration \
         WHERE migration_id = '0034_collection_control_closure'",
    )
    .execute(database.pool())
    .await
    .expect("the ledger row is removed for this proof");

    let not_applied = health_payload(application).await;
    assert_eq!(
        not_applied
            .pointer("/readiness/state")
            .and_then(Value::as_str),
        Some("migrations_not_applied"),
        "连得上、表在、记账缺一行，就该报未迁移：{not_applied}"
    );
    assert_eq!(
        not_applied
            .pointer("/readiness/detail")
            .and_then(Value::as_str),
        Some("0034_collection_control_closure"),
        "detail 写声明序里第一个缺的 id，与 worker 报的是同一句"
    );
}

/// 本地读取面用不了时（`SchemaUnavailable`）仍然拿得住连接，所以 `/health` 能说出**为什么**
/// 不能接活，而不是在这里替它猜一个原因。这里把台账表整个删掉：读取面判死，而就绪判定报的
/// 是「台账读不到」——两者是不同的问题，第二个回答更具体。
#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_schema_unavailable_api_still_says_why_it_cannot_take_work() {
    let database = proof_database("local_api_readiness_schema").await;
    sqlx::query("DROP TABLE linggan_local_schema_migration")
        .execute(database.pool())
        .await
        .expect("the ledger table is dropped for this proof");

    let state = classify_database(database).await;
    assert!(
        matches!(state, LocalDatabaseState::SchemaUnavailable(_)),
        "基础 schema 不在时读取面判死"
    );
    let application = router(LocalWebState {
        database: state,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    });
    let payload = health_payload(application).await;

    assert_eq!(
        payload.pointer("/database/state").and_then(Value::as_str),
        Some("CONFIGURED_UNAVAILABLE"),
        "读取面的状态不变：{payload}"
    );
    assert_eq!(
        payload.pointer("/readiness/state").and_then(Value::as_str),
        Some("migration_ledger_unreadable"),
        "就绪回答要说出确切原因，不是笼统的「schema 不行」：{payload}"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn loopback_runtime_producer_uses_the_routes_published_by_health() {
    let database = proof_database("local_api_runtime_route_contract").await;
    let application = app_with_database(database);
    let health_response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health_response.status(), StatusCode::OK);
    let health_body = to_bytes(health_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: serde_json::Value = serde_json::from_slice(&health_body).unwrap();
    assert_eq!(
        health.pointer("/dataState"),
        Some(&serde_json::Value::String(
            "LINGGAN_BROWSER_PRODUCER_RUNTIME".to_owned()
        ))
    );
    assert_eq!(
        health.pointer("/database/schema"),
        Some(&serde_json::Value::String(
            "PLUGIN_RUNTIME_002_SCHEMA_READY".to_owned()
        ))
    );
    assert_eq!(
        health
            .pointer("/routes/workResources")
            .and_then(Value::as_str),
        Some("/api/local/work-resources")
    );
    assert_eq!(
        health
            .pointer("/routes/dispatch/claim")
            .and_then(Value::as_str),
        Some("/api/local/dispatch/claim"),
        "a ready local runtime publishes the task-claim route"
    );
    assert_eq!(
        health
            .pointer("/routes/dispatch/failure")
            .and_then(Value::as_str),
        Some("/api/local/dispatch/failures"),
        "a ready local runtime publishes the failure-recovery route with claim"
    );
    let task_path = health
        .pointer("/routes/localProducer/taskCreation")
        .and_then(serde_json::Value::as_str)
        .expect("health publishes task creation route")
        .to_owned();
    let attempt_path = health
        .pointer("/routes/localProducer/attemptStart")
        .and_then(serde_json::Value::as_str)
        .expect("health publishes attempt start route")
        .to_owned();
    let submission_path = health
        .pointer("/routes/localProducer/submission")
        .and_then(serde_json::Value::as_str)
        .expect("health publishes submission route")
        .to_owned();
    let task = runtime_producer_task_spec();
    let attempt = runtime_producer_attempt();
    let submission = runtime_producer_submission();
    for (path, body, expected) in [
        (task_path, task, "\"outcome\":\"created\""),
        (attempt_path, attempt, "\"outcome\":\"started\""),
        (
            submission_path.clone(),
            submission.clone(),
            "\"delivery\":\"acknowledged\"",
        ),
        (submission_path, submission, "\"delivery\":\"replay\""),
    ] {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains(expected));
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn accepted_author_profile_creates_a_deduplicated_observation_target_with_target_receipt() {
    let database = proof_database("author_profile_target_receipt").await;
    let application = app_with_database(database.clone());
    for (path, body) in [
        (
            "/api/local/producer/tasks",
            runtime_author_producer_task_spec(),
        ),
        (
            "/api/local/producer/runtime-attempts",
            runtime_author_producer_attempt(),
        ),
    ] {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let response = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/runtime-submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(runtime_author_producer_submission()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let receipt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        receipt.pointer("/delivery").and_then(Value::as_str),
        Some("acknowledged")
    );
    assert_eq!(
        receipt.pointer("/targetSync/state").and_then(Value::as_str),
        Some("created")
    );
    assert_eq!(
        receipt
            .pointer("/targetSync/targetRef")
            .and_then(Value::as_str)
            .is_some(),
        true,
    );
    let target: (String, String, serde_json::Value) = sqlx::query_as(
        "SELECT identity_key,display_name,identity_facts \
         FROM collection_observation_target WHERE platform='xhs' AND target_kind='creator'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(target.0, "author-route-1");
    assert_eq!(target.1, "路由作者");
    assert_eq!(
        target.2["avatar"],
        "https://media.example/author-route-1.jpg"
    );
}

/// 采集准入按**状态**拒绝时，回执说的必须是「状态前置」，不是「稍后可以重试」。
///
/// 这条阶梯两段共用。此前它写在两个分支里、只有第一段分了类，于是第二段遇到同一种拒绝
/// （关键词被停掉之后，详情补采同样不被准入接受）仍然说「稍后可以重试」——一句让人反复
/// 做同一件永远不会成的事的话。这张卡要修的正是回执说谎，所以分类只留一处。
///
/// 另一半同样要钉住：**真的没送出去**（目标已不在、schema 没就绪、数据库故障）必须继续
/// 说「可以稍后重试」。分不清这两类，只是把「一律说重试」翻成「一律说别重试」，一样是
/// 让人照着错的话做决定。
#[test]
fn a_refusal_by_state_is_not_told_to_retry_later_while_a_genuine_miss_is() {
    assert_eq!(
        keyword_archive_error_receipt(&AcquisitionChainError::TargetNotRequestable {
            state: "dismissed".to_owned(),
        }),
        "keyword_archive_not_requestable",
        "准入按状态拒绝是永久前置，说成「稍后可以重试」会让人一直点"
    );
    assert_eq!(
        keyword_archive_error_receipt(&AcquisitionChainError::TargetDomainUnassigned),
        "target_domain_unassigned",
        "缺领域要如实说成缺领域，不能落进信息量最低的那条兜底"
    );
    assert_eq!(
        keyword_archive_error_receipt(&AcquisitionChainError::SchemaUnavailable),
        "archive_unavailable",
        "schema 没就绪是真的可以稍后重试，不能和状态前置混成一句"
    );
}

/// 「建立档案」按下去要真的开始建档：一个**从没建过档**的关键词必须发出第一段
/// （翻搜索面拿链接），而不是回一句「详情已补齐」然后什么都不做。
///
/// 这个入口此前是「先试第二段，挑不出候选就当已完成」：一个还没有作品链接的词，第二段
/// 当然挑不出候选——空的候选集被读成了「都补完了」，于是它回一句「详情已补齐」就结束，
/// **第一段永远发不出去**，库里一张工单都没有。行上写着「尚未建立」，界面上却没有任何
/// 入口能把它建起来（2026-09-15 在「a 娃」这个词上就是整个页面没有任何反应）。
///
/// 判据只有一条是别人替不了的：**库里真的多了一张待翻搜索面的工单**。只断言回执文案
/// 不够——那句话本来就是错的，错的地方正在于它说的话和库里的状态对不上。
#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn nul_submission_is_terminal_422_without_partial_package_or_receipt() {
    let database = proof_database("nul_submission_rejection").await;
    let application = app_with_database(database.clone());
    let mut submission: Value =
        serde_json::from_str(&runtime_author_producer_submission()).unwrap();
    submission["capturePackage"]["records"][0]["payload"]["description"] =
        serde_json::json!("前\0后");
    for _ in 0..2 {
        let response = application
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/local/producer/runtime-submissions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(submission.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("submission_invalid"));
    }
    let counts: (i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM linggan_runtime_capture_package), (SELECT count(*) FROM linggan_runtime_submission_receipt)")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(counts, (0, 0));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_never_archived_keyword_starts_its_first_stage_when_asked_to_archive() {
    let database = proof_database("keyword_archive_first_stage").await;
    let application = app_with_database(database.clone());

    // 从页面加入一个关键词：领域当场选定——缺领域的目标发不出任何采集，准入会先拦住它，
    // 那样这条用例就绕开了真正要证明的那一段。
    let created = collection_json_command(&application, "new", json!({"target_kind":"keyword","identity":"archive-first-stage-proof","domain":"00000000-0000-4000-8000-000000000001"})).await;
    assert_eq!(created["outcome"], "applied");
    let target_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target \
         WHERE target_kind='keyword' AND identity_key='archive-first-stage-proof'",
    )
    .fetch_one(database.pool())
    .await
    .expect("页面新建的关键词目标落库");

    // 授权走产品自己的入口，与人在授权页上按下的那一次是同一条路。
    let granted = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/collection/authorizations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "targetKind": "keyword",
                        "lane": "deep_archive",
                        "purpose": "从观察目标页发起关键词历史建档",
                        "maxTargets": 200,
                        "maxWorksPerTarget": 200,
                        "validForDays": 30,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(granted.status(), StatusCode::OK);

    // 人在列表上按下的那一下。
    let archived = collection_json_command(
        &application,
        "archive",
        json!({"row_target_ref":target_ref}),
    )
    .await;
    assert_eq!(archived["outcome"], "applied");
    let location = archived["location"].as_str().expect("回执地址");
    assert!(
        location.contains("archive_requested"),
        "一个从没建过档的词，这一下应当发出第一段建档，实际回执：{location}"
    );
    assert!(
        !location.contains("keyword_detail_complete"),
        "链接都还没有的词不可能「详情已补齐」：{location}"
    );

    let orders: Vec<String> = sqlx::query_scalar(
        "SELECT lane FROM collection_work_order WHERE target_ref=$1 ORDER BY created_at",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
    .expect("工单可查");
    assert_eq!(
        orders,
        vec!["deep_archive".to_owned()],
        "这一下应当只发出翻搜索面那一段，实际工单：{orders:?}"
    );
}

/// 历史上已经在观察、但未建档的关键词必须能补建第一阶段档案。
///
/// 巡查准入仍被关键词完成门槛阻断；这里仅允许修复旧状态留下的缺失底座，不能把一次
/// `deep_archive` 成功误报成巡查已恢复。
#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn a_patrolling_keyword_can_repair_its_missing_first_stage_archive() {
    let database = proof_database("keyword_archive_patrolling_refusal").await;
    let application = app_with_database(database.clone());
    let created = collection_json_command(&application, "new", json!({"target_kind":"keyword","identity":"archive-patrolling-refusal","domain":"00000000-0000-4000-8000-000000000001"})).await;
    assert_eq!(created["outcome"], "applied");
    let target_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target \
         WHERE target_kind='keyword' AND identity_key='archive-patrolling-refusal'",
    )
    .fetch_one(database.pool())
    .await
    .expect("页面新建的关键词目标落库");

    let granted = application
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/collection/authorizations")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "targetKind": "keyword",
                        "lane": "deep_archive",
                        "purpose": "从观察目标页发起关键词历史建档",
                        "maxTargets": 200,
                        "maxWorksPerTarget": 200,
                        "validForDays": 30,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(granted.status(), StatusCode::OK);

    // 线上 a 娃 与 adhd 的真实状态：正在按周观察。
    sqlx::query(
        "UPDATE collection_observation_target SET lifecycle_state='monitoring' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("置为观察中");

    let archived = collection_json_command(
        &application,
        "archive",
        json!({"row_target_ref":target_ref}),
    )
    .await;
    assert_eq!(archived["outcome"], "applied");
    let location = archived["location"].as_str().expect("回执地址");
    assert!(
        location.contains("archive_requested"),
        "历史巡查状态不得把关键词困在未建档状态，实际回执：{location}"
    );
    assert!(
        !location.contains("keyword_archive_not_requestable"),
        "修复性建档已被接受，不能再报成不可请求：{location}"
    );

    let orders: Vec<String> = sqlx::query_scalar(
        "SELECT lane FROM collection_work_order WHERE target_ref=$1 ORDER BY created_at",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
    .expect("工单可数");
    assert_eq!(orders, vec!["deep_archive"], "修复性建档只创建第一阶段工单");
}

#[tokio::test]
#[ignore = "requires an isolated PostgreSQL proof database"]
async fn creator_gap_route_never_falls_back_to_a_homepage_rescan() {
    let database = proof_database("creator_gap_route").await;
    let application = app_with_database(database.clone());
    let created = collection_json_command(&application, "new", json!({"target_kind":"creator","identity":"69aad16e000000003201b111","domain":"00000000-0000-4000-8000-000000000001"})).await;
    assert_eq!(created["outcome"], "applied");
    let target_ref: uuid::Uuid = sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE identity_key='69aad16e000000003201b111'")
        .fetch_one(database.pool()).await.unwrap();
    let response = collection_json_command(
        &application,
        "archive",
        json!({"row_target_ref":target_ref,"archive_action":"gaps"}),
    )
    .await;
    assert_eq!(response["outcome"], "rejected");
    assert_eq!(response["code"], "archive_nothing_to_continue");
    let work_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order WHERE target_ref=$1")
            .bind(target_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(work_count, 0);
}

// Exercise the canonical React JSON transport while retaining the original owner oracles.
async fn collection_json_command(application: &Router, action: &str, fields: Value) -> Value {
    let context = application
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/collection/commands/context")
                .header(header::HOST, "127.0.0.1:8080")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(context.status(), StatusCode::OK);
    let context: Value =
        serde_json::from_slice(&to_bytes(context.into_body(), usize::MAX).await.unwrap()).unwrap();
    let response = application.clone().oneshot(Request::builder().method("POST")
        .uri(format!("/api/collection/commands/{action}"))
        .header(header::HOST, "127.0.0.1:8080").header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({"requestRef":uuid::Uuid::new_v4(),"epoch":context["epoch"],"fields":fields}).to_string())).unwrap()).await.unwrap();
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("test script must provide the isolated proof database URL");
    isolated_proof_schema(&url, schema, LOCAL_001_MIGRATIONS)
        .await
        .expect("isolated migration applies")
}

/// 走真实路由取一次 `/health` 的回答。
async fn health_payload(application: Router) -> Value {
    let response = application
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn producer_fixture_observed_at(database: &Database) -> String {
    sqlx::query(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS observed_at",
    )
        .fetch_one(database.pool())
        .await
        .expect("database returns a timestamp")
        .get("observed_at")
}

async fn post_discovery_package(application: Router, package: String) -> axum::response::Response {
    application
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/discovery-packages")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn assert_discovery_package_is_accepted(application: Router, package: String) {
    let response = post_discovery_package(application, package).await;
    assert_eq!(response.status(), StatusCode::OK);
}

async fn get_successful_utf8_response(application: Router, uri: &str) -> String {
    let response = application
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn discovery_package(observed_at: &str) -> String {
    format!(
        r#"{{
          "contractVersion":"xhs.discovery.visible-card.v1",
          "acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},
          "observedAt":"{observed_at}",
          "coverage":{{"unit":"visible_search_card","visibleCards":2,"stoppedReason":"risk_control"}},
          "cards":[
            {{"content":{{"platformContentId":"note-api-known","title":"API 接纳卡片 ADHD","creatorDisplayName":"A娃家长","publishedAtSourceText":"{observed_at}","coverCandidate":{{"observedExternalUri":"https://xhscdn.example/api-cover"}}}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":1}}}},
            {{"content":{{"platformContentId":"note-api-unknown","title":"ADHD 未知发布时间","creatorDisplayName":"另一位家长"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":2}}}}
          ]
        }}"#,
    )
}

fn shared_api_content_unknown_package(observed_at: &str) -> String {
    format!(
        r#"{{
          "contractVersion":"xhs.discovery.visible-card.v1",
          "acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},
          "observedAt":"{observed_at}",
          "coverage":{{"unit":"visible_search_card","visibleCards":1,"stoppedReason":"risk_control"}},
          "cards":[
            {{"content":{{"platformContentId":"note-api-known","title":"API 接纳卡片 ADHD","creatorDisplayName":"A娃家长"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":1}}}}
          ]
        }}"#,
    )
}

fn local_task_spec() -> String {
    r#"{"contractVersion":"linggan.task-spec.v1","taskId":"11111111-1111-4111-8111-111111111111","source":"manual","platform":"xhs","pageType":"search_results","target":"current_visible_search_surface","capabilitiesRequested":["discover_visible_cards"],"maximumQuota":20,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["current_surface_read_once","maximum_quota"]}"#.to_owned()
}

fn local_attempt() -> String {
    r#"{"contractVersion":"linggan.local-trusted.attempt.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333"}"#.to_owned()
}

fn local_submission(observed_at: &str) -> String {
    format!(
        r#"{{"contractVersion":"linggan.local-trusted.submission.v1","producerInstanceId":"22222222-2222-4222-8222-222222222222","taskId":"11111111-1111-4111-8111-111111111111","attemptId":"33333333-3333-4333-8333-333333333333","submissionId":"44444444-4444-4444-8444-444444444444","discoveryPackage":{}}}"#,
        discovery_package(observed_at)
    )
}

#[tokio::test]
async fn unreadable_target_json_never_substitutes_empty_html() {
    let response = app().oneshot(Request::builder().uri("/api/collection/workspace/target/00000000-0000-4000-8000-000000000001?domain=all").header(header::HOST,"127.0.0.1:8080").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["code"], "collection_targets_unavailable");
}

/// DESIGN-010 · LIDS v7 on the Operations surface.
///
/// Two things this locks. First, the English descriptive labels that used to sit under every
/// Chinese one are gone and must stay gone: LANG-05 budgets English to machine facts, closed-set
/// state words and structural numbering, and none of these were any of those. The event-kind
/// filters are the control case — they are contract literals, so they stay.
///
/// Second, the sampling lattice on the dark stream stays clipped to its top strip. An earlier
/// build let the sweep travel the full panel height and it crossed the body copy, which is the
/// one thing `materials.md` rules out outright: edges only, never behind running text.
#[test]
fn station_capability_matrix_separates_unverified_from_degraded() {
    use linggan_evidence::{CapabilityState, StationCapability};

    let base = |capability: &str| StationCapability {
        capability: capability.to_owned(),
        declared: true,
        successes: 0,
        last_success_at: None,
        capability_failures: 0,
        execution_failures: 0,
        last_failure_at: None,
    };

    // 声明支持、窗口内没有任何证据 —— 未验证，不是降级。
    assert_eq!(
        base("discovery_search").state(),
        CapabilityState::Unverified
    );

    // 有成功产出 —— 就绪。
    let ready = StationCapability {
        successes: 3,
        ..base("comments")
    };
    assert_eq!(ready.state(), CapabilityState::Ready);

    // 超时一堆但插件从没说过跑不了 —— 仍然就绪。执行失败不是能力缺陷。
    let timing_out = StationCapability {
        successes: 3,
        execution_failures: 11,
        ..base("author_profile")
    };
    assert_eq!(
        timing_out.state(),
        CapabilityState::Ready,
        "页面超时是这一次没成，不能把能力判成坏的"
    );

    // 插件明确回报跑不了 —— 降级，且即使有成功记录也降级。
    let degraded = StationCapability {
        successes: 3,
        capability_failures: 1,
        ..base("media_slots")
    };
    assert_eq!(degraded.state(), CapabilityState::Degraded);

    // 没声明 —— 不支持，与「声明了但没验证」是两件事。
    let undeclared = StationCapability {
        declared: false,
        ..base("replies")
    };
    assert_eq!(undeclared.state(), CapabilityState::NotDeclared);
}

fn runtime_producer_task_spec() -> String {
    r#"{"contractVersion":"linggan.producer.task-spec.v2","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","source":"manual","platform":"xhs","pageType":"note_detail","target":{"contentExternalId":"note-a"},"capabilitiesRequested":["media_slots"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"slots","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#.to_owned()
}

fn runtime_producer_attempt() -> String {
    r#"{"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc"}"#.to_owned()
}

fn runtime_producer_submission() -> String {
    r#"{"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccc","submissionId":"dddddddd-dddd-4ddd-8ddd-dddddddddddd","capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee","packageKind":"media_slots","platform":"xhs","observedAt":"2026-08-25T00:00:00Z","capturedAt":"2026-08-25T00:00:01Z","coverage":{"target":{"basis":"known_set","contentExternalId":"note-a"},"layers":[{"capability":"media_slots","observed":2,"attempted":2,"acquired":0,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"media_acquisition_not_started"}]},"records":[{"kind":"media_slot","slotKey":"xhs:note-a:image:1","slot":{"role":"image","ordinal":1},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://fixture.invalid/one.jpg"},"observationRef":"ffffffff-ffff-4fff-8fff-ffffffffffff"},{"kind":"media_slot","slotKey":"xhs:note-a:image:2","slot":{"role":"image","ordinal":2},"sourceObject":{"externalId":"note-a"},"observation":{"externalUri":"https://fixture.invalid/two.jpg"},"observationRef":"11111111-2222-4333-8444-555555555555"}]}}"#.to_owned()
}

fn runtime_author_producer_task_spec() -> String {
    r#"{"contractVersion":"linggan.producer.task-spec.v2","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","source":"manual","platform":"xhs","pageType":"profile","target":{"authorExternalId":"author-route-1"},"capabilitiesRequested":["author_profile"],"maximumQuota":1,"commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["manual_stop","maximum_quota"]}"#.to_owned()
}

fn runtime_author_producer_attempt() -> String {
    r#"{"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbc","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccd"}"#.to_owned()
}

fn runtime_author_producer_submission() -> String {
    r#"{"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbc","taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaab","attemptId":"cccccccc-cccc-4ccc-8ccc-cccccccccccd","submissionId":"dddddddd-dddd-4ddd-8ddd-ddddddddddde","capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":"eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeef","packageKind":"author_profile","platform":"xhs","observedAt":"2026-09-02T10:00:00Z","capturedAt":"2026-09-02T10:00:01Z","coverage":{"target":{"basis":"known_set","authorExternalId":"author-route-1"},"layers":[{"capability":"author_profile","observed":1,"attempted":1,"acquired":1,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"profile_read_complete"}]},"records":[{"kind":"author_profile","sourceObject":{"platform":"xhs","type":"author","externalId":"author-route-1"},"payload":{"userId":"author-route-1","name":"路由作者","avatar":"https://media.example/author-route-1.jpg","description":"从已接纳包建立观察目标"}}]}}"#.to_owned()
}

fn declared_token_values(stylesheet: &str) -> BTreeMap<&str, &str> {
    stylesheet
        .lines()
        .filter_map(|line| line.trim().strip_prefix("--lgi-"))
        .filter_map(|line| {
            line.split_once(':')
                .map(|(name, value)| (name, value.trim().trim_end_matches(';')))
        })
        .collect()
}

#[tokio::test]
async fn collection_default_resolves_to_the_daily_target_page() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/collection")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response.headers().get(header::LOCATION).unwrap(),
        "/collection/targets"
    );
}

#[tokio::test]
async fn collection_task_return_scope_only_applies_to_manual_observation() {
    for (command, expected_page) in [
        ("manual_observe", "/collection/tasks?"),
        ("save_rule", "/collection/targets?"),
    ] {
        let form:MonitorRuleWire=serde_json::from_value(json!({"target_ref":"00000000-0000-4000-8000-000000000001","expected_revision":0,"idempotency_key":"00000000-0000-4000-8000-000000000002","command_kind":command,"return_scope":"tasks","return_domain":"all"})).unwrap();
        let response = collection_target_rule_command(
            State(LocalWebState {
                database: LocalDatabaseState::NotConfigured,
                active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
                account_digest_key: None,
            }),
            axum::extract::Form(form),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.starts_with(expected_page));
        assert!(location.contains("error=read_model_not_connected"));
        if command == "manual_observe" {
            assert!(location.contains("domain=all"));
        }
    }
}
