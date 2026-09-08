//! Complete local synthetic SDK receipts, never production credentials or model traffic.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_daily::*,
    comment_intelligence::{Prepare, ResearchScope, Run, prepare, run},
    model_invocation::*,
    model_secrets::*,
    model_settings::*,
    model_settings_read::{current_comment_model_state, read_model_settings},
    pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "support/comment_daily_fixture.rs"]
mod daily_fixture;
use daily_fixture::*;

async fn register(db: &Database, url: &str, model: &str) -> (SaveModelConnection, SaveModelConfig) {
    let connection = SaveModelConnection {
        version_ref: Uuid::new_v4(),
        connection_ref: Uuid::new_v4(),
        expected_revision: 0,
        name: "SYNTHETIC / NOT EVIDENCE".into(),
        api: "openai-completions".into(),
        base_url: url.into(),
        local_endpoint: true,
        api_key: "SYNTHETIC-NOT-A-CREDENTIAL".into(),
    };
    save_model_connection(db, &SyntheticModelSecrets, &connection)
        .await
        .unwrap();
    let model_ref = Uuid::new_v4();
    save_model_entry(
        db,
        &SaveModelEntry {
            model_ref,
            connection_version_ref: connection.version_ref,
            model_id: model.into(),
        },
    )
    .await
    .unwrap();
    let config = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: None,
        model_ref,
        input_token_limit: 16000,
        output_token_limit: 2000,
        timeout_seconds: 5,
        max_attempts: 2,
        auto_source_limit: 10,
        auto_token_limit: 100000,
    };
    (connection, config)
}
async fn probe(db: &Database, c: &SaveModelConnection, m: &SaveModelConfig) -> Value {
    probe_model(
        db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &ProbeModel {
            invocation_ref: Uuid::new_v4(),
            connection_version_ref: c.version_ref,
            model_ref: Some(m.model_ref),
            operation: "probe".into(),
        },
    )
    .await
    .unwrap()
}
async fn calls(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap()
}
async fn prepare_sources(db: &Database, refs: Vec<Uuid>) -> Value {
    prepare(
        db,
        &Prepare {
            reanalyze: false,
            source_refs: Some(refs),
            scope: ResearchScope {
                domain: Some("00000000-0000-4000-8000-000000000001".parse().unwrap()),
                from: Some("2020-01-01T00:00:00Z".into()),
                to: Some("2099-01-01T00:00:00Z".into()),
                view: Some("voices".into()),
                ..Default::default()
            },
        },
    )
    .await
    .unwrap()
}

async fn assert_probe_diagnostics(
    db: &Database,
    connection: &SaveModelConnection,
    config: &SaveModelConfig,
    result: &Value,
) {
    assert_eq!(result["commentValidation"]["status"], "partial");
    assert_eq!(result["commentValidation"]["acceptedFields"], 1);
    assert_eq!(result["commentValidation"]["rejectedFields"], 1);
    let detail = &result["commentValidation"]["diagnostics"][0];
    assert!(detail["path"].is_string() && detail["expected"].is_string());
    assert!(!detail.to_string().contains("合成执行精力"));
    let (invocation, stored): (Uuid, Value) = sqlx::query_as(
        "SELECT invocation_ref,result FROM linggan_model_invocation WHERE operation='probe'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(stored["commentValidation"], result["commentValidation"]);
    let replay = probe_model(
        db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &ProbeModel {
            invocation_ref: invocation,
            connection_version_ref: connection.version_ref,
            model_ref: Some(config.model_ref),
            operation: "probe".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        replay["result"]["commentValidation"],
        result["commentValidation"]
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local SDK"]
async fn partial_probe_saves_prepares_and_dispatches_without_relaxing_actual_analysis() {
    let db = fixture::proof_database("callable_partial").await;
    let (mut server, url) = fixture_server().await;
    let (connection, config) = register(&db, &url, "synthetic-partial").await;
    assert_eq!(calls(&db).await, 0);
    assert!(matches!(
        save_model_config(&db, &config).await,
        Err(ModelError::NotQualified)
    ));
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "NEEDS_CALL_TEST"
    );
    let result = probe(&db, &connection, &config).await;
    assert_eq!(result["ok"], true);
    assert_eq!(result["modelCallable"], true);
    assert_eq!(result["commentQualified"], false);
    assert_probe_diagnostics(&db, &connection, &config, &result).await;

    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "NEEDS_SELECTION"
    );
    save_model_config(&db, &config).await.unwrap();
    let settings = read_model_settings(&db, true).await.unwrap();
    assert_eq!(settings["model"]["modelState"], "CONFIGURED");
    assert_eq!(settings["models"][0]["modelCallable"], true);
    assert_eq!(settings["models"][0]["commentQualified"], false);
    assert_eq!(
        calls(&db).await,
        1,
        "save must reuse the persisted successful call receipt"
    );
    research_fixture::detail(&db, "callable-work", "SYNTHETIC / NOT EVIDENCE").await;
    let good = source(&db, "callable-work", "good", "我需要具体的执行方法").await;
    let invalid = source(
        &db,
        "callable-work",
        "bad",
        "[BAD_SCHEMA] 结构无效也必须拒绝",
    )
    .await;
    let prepared = prepare_sources(&db, vec![good, invalid]).await;
    assert_eq!(prepared["preflight"]["configRef"], json!(config.config_ref));
    assert_eq!(calls(&db).await, 1);
    let batch: Uuid = prepared["prepareRef"].as_str().unwrap().parse().unwrap();
    run(
        &db,
        &Run {
            prepare_ref: batch,
            config_ref: config.config_ref,
            token_limit: 100000,
            reanalyze: false,
        },
    )
    .await
    .unwrap();
    assert!(tick(&db).await);
    let states: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT source_ref,state FROM linggan_comment_daily_item WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(states.contains(&(good, "succeeded".into())));
    assert!(states.contains(&(invalid, "failed".into())));
    let semantic: Value = sqlx::query_scalar(
        "SELECT result->'semantic' FROM linggan_comment_analysis_work WHERE source_ref=$1",
    )
    .bind(good)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(semantic["acceptance"], "partial");
    assert_eq!(semantic["labels"], json!([]));
    assert_eq!(semantic["problems"].as_array().unwrap().len(), 1);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local SDK"]
async fn latest_failed_probe_blocks_existing_default_preflight_and_pending_dispatch() {
    let db = fixture::proof_database("callable_latest").await;
    let (mut server, url) = fixture_server().await;
    let (connection, config) = register(&db, &url, "synthetic-good").await;
    assert_eq!(probe(&db, &connection, &config).await["ok"], true);
    save_model_config(&db, &config).await.unwrap();
    let item = source(&db, "latest-work", "a", "希望得到可以持续执行的方法").await;
    let batch = selected(&db, config.config_ref, vec![item], 100000).await;
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection.connection_ref,
            expected_revision: 1,
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "PAUSED"
    );
    assert!(!tick(&db).await);
    assert!(prepare_sources(&db, vec![item]).await["preflight"]["configRef"].is_null());
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection.connection_ref,
            expected_revision: 2,
            enabled: true,
        },
    )
    .await
    .unwrap();
    server.kill().await.unwrap();
    assert_eq!(probe(&db, &connection, &config).await["ok"], false);
    let count = calls(&db).await;
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "NEEDS_CALL_TEST"
    );
    assert!(!tick(&db).await);
    assert!(prepare_sources(&db, vec![item]).await["preflight"]["configRef"].is_null());
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "pending");
    let new_config = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: Some(config.config_ref),
        ..config
    };
    assert!(matches!(
        save_model_config(&db, &new_config).await,
        Err(ModelError::NotQualified)
    ));
    assert_eq!(
        calls(&db).await,
        count,
        "blocked actions must not dispatch a model"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local SDK"]
async fn truncated_response_and_another_models_success_never_authorize_selection() {
    let db = fixture::proof_database("callable_truncated").await;
    let (mut server, url) = fixture_server().await;
    let (connection, config) = register(&db, &url, "synthetic-limit").await;
    let result = probe(&db, &connection, &config).await;
    assert_eq!(result["modelCallable"], true);
    assert_eq!(result["ok"], false);
    assert!(matches!(
        save_model_config(&db, &config).await,
        Err(ModelError::NotQualified)
    ));
    let (good_connection, good) = register(&db, &url, "synthetic-good").await;
    assert_eq!(probe(&db, &good_connection, &good).await["ok"], true);
    assert!(matches!(
        save_model_config(&db, &config).await,
        Err(ModelError::NotQualified)
    ));
    let settings = read_model_settings(&db, true).await.unwrap();
    let limited = settings["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["modelRef"] == json!(config.model_ref))
        .unwrap();
    assert_eq!(limited["modelCallable"], false);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic local SDK"]
async fn task_b_pending_relation_cannot_dispatch_after_latest_call_test_fails() {
    use linggan_intelligence::comment_intelligence_problems::{
        queue_problem_task, reconcile_problem_index, run_problem_relation_once,
    };
    let db = fixture::proof_database("callable_task_b").await;
    let (mut server, url) = fixture_server().await;
    let (connection, config) = register(&db, &url, "synthetic-good").await;
    assert_eq!(probe(&db, &connection, &config).await["ok"], true);
    save_model_config(&db, &config).await.unwrap();
    research_fixture::detail(&db, "task-b-work", "SYNTHETIC / NOT EVIDENCE").await;
    let a = source(&db, "task-b-work", "a", "每天提醒学习很费力").await;
    let b = source(&db, "task-b-work", "b", "每天陪着写作业很疲惫").await;
    let batch = selected(&db, config.config_ref, vec![a, b], 100000).await;
    assert!(tick(&db).await);
    reconcile_problem_index(&db, 100).await.unwrap();
    let domain: Uuid = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    let candidate_a: Uuid = sqlx::query_scalar("SELECT candidate_ref FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) WHERE s.source_ref=$1").bind(a).fetch_one(db.pool()).await.unwrap();
    let candidate_b: Uuid = sqlx::query_scalar("SELECT candidate_ref FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) WHERE s.source_ref=$1").bind(b).fetch_one(db.pool()).await.unwrap();
    let created = linggan_intelligence::comment_intelligence_actions::execute_action(&db, json!({
        "commandRef":Uuid::new_v4(),"domain":domain,"kind":"problem_create","expectedRevision":0,
        "reason":"SYNTHETIC / NOT EVIDENCE",
        "payload":{"name":"执行精力","meaning":"持续陪伴的精力负担","sourceRefs":[a],"candidateRefs":[candidate_a]}
    })).await.unwrap();
    let target: Uuid = created["problemRef"].as_str().unwrap().parse().unwrap();
    let task = queue_problem_task(
        &db,
        batch,
        domain,
        candidate_b,
        &[target],
        "synthetic-vector.v1",
    )
    .await
    .unwrap();
    server.kill().await.unwrap();
    assert_eq!(probe(&db, &connection, &config).await["ok"], false);
    let before = calls(&db).await;
    assert!(
        !run_problem_relation_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_ci_problem_task WHERE task_ref=$1")
            .bind(task)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "pending");
    assert_eq!(calls(&db).await, before);
}
