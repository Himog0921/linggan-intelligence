#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_research_projection::*, model_invocation::*, model_plans::*, model_runner::*,
    model_secrets::*, model_settings::*, model_settings_read::*, pi_adapter::*,
};
use linggan_storage_postgres::Database;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;
async fn fixture_server() -> (tokio::process::Child, String) {
    let node = std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
        .join(".nvm/versions/node/v24.13.0/bin/node");
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/pi-adapter/test/fixture-server.mjs");
    let mut child = tokio::process::Command::new(node)
        .arg(script)
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .await
        .unwrap();
    (child, line.trim().into())
}
async fn configured(
    db: &Database,
    url: &str,
    model_id: &str,
    expected: Option<Uuid>,
) -> (Uuid, Uuid, Uuid) {
    let connection = SaveModelConnection {
        version_ref: Uuid::new_v4(),
        connection_ref: Uuid::new_v4(),
        expected_revision: 0,
        name: "合成模型连接".into(),
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
            model_id: model_id.into(),
        },
    )
    .await
    .unwrap();
    let probe = probe_model(
        db,
        &SyntheticModelSecrets,
        &PiAdapter::configured(),
        &ProbeModel {
            invocation_ref: Uuid::new_v4(),
            connection_version_ref: connection.version_ref,
            model_ref: Some(model_ref),
            operation: "probe".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(probe["commentQualified"], true);
    let config = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: expected,
        model_ref,
        input_token_limit: 16000,
        output_token_limit: 2000,
        timeout_seconds: 5,
        max_attempts: 2,
        auto_source_limit: 10,
        auto_token_limit: 100000,
    };
    save_model_config(db, &config).await.unwrap();
    (
        config.config_ref,
        connection.connection_ref,
        connection.version_ref,
    )
}
async fn source(db: &Database, key: &str) -> Uuid {
    research_fixture::comment(
        db,
        key,
        key,
        "SYNTHETIC / NOT EVIDENCE：我👨‍👩‍👧每天提醒很费精力",
        "2026-09-06T01:00:00Z",
    )
    .await
}
fn plan(config: Uuid, kind: &str, sources: Vec<Uuid>, limit: i32, tokens: i64) -> StartModelPlan {
    StartModelPlan {
        plan_ref: Uuid::new_v4(),
        config_ref: config,
        kind: kind.into(),
        source_refs: sources,
        source_limit: limit,
        token_limit: tokens,
        expected_auto_plan_ref: None,
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn settings_to_real_sdk_to_qualified_comment_and_usage_is_persistent() {
    let db = fixture::proof_database("model_pi_end_to_end").await;
    let (_server, url) = fixture_server().await;
    let (config, _, version) = configured(&db, &url, "synthetic-good", None).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_model_work")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    let reference = source(&db, "model-trial").await;
    let grant = plan(config, "trial", vec![reference], 1, 36000);
    start_model_plan(&db, &grant).await.unwrap();
    start_model_plan(&db, &grant).await.unwrap();
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let history = read_comment_research_annotations(&db, reference)
        .await
        .unwrap();
    assert_eq!(history["analysis"][0]["state"], "succeeded");
    let groups = read_comment_problem_groups(&db, &model_version(config))
        .await
        .unwrap();
    assert_eq!(groups["items"][0]["label"], "合成执行精力");
    let settings = read_model_settings(&db, true).await.unwrap();
    assert_eq!(settings["runs"][0]["inputTokens"], 800);
    assert_eq!(settings["runs"][0]["outputTokens"], 100);
    assert!(settings["runs"][0]["costUsd"].is_null());
    let encoded = settings.to_string();
    assert!(!encoded.contains("SYNTHETIC-NOT-A-CREDENTIAL"));
    assert!(!encoded.contains("secret_ref"));
    let test = ProbeModel {
        invocation_ref: Uuid::new_v4(),
        connection_version_ref: version,
        model_ref: None,
        operation: "discover".into(),
    };
    let discovered = probe_model(&db, &SyntheticModelSecrets, &PiAdapter::configured(), &test)
        .await
        .unwrap();
    assert_eq!(discovered["modelIds"].as_array().unwrap().len(), 4);
    assert_eq!(
        probe_model(&db, &SyntheticModelSecrets, &PiAdapter::configured(), &test)
            .await
            .unwrap()["replayed"],
        true
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn automatic_scope_switch_pause_and_budget_have_real_execution_boundaries() {
    let db = fixture::proof_database("model_pi_auto").await;
    let (_server, url) = fixture_server().await;
    let (old, connection, _) = configured(&db, &url, "synthetic-good", None).await;
    let historical = source(&db, "model-old").await;
    let automatic = plan(old, "automatic", vec![], 3, 18000);
    start_model_plan(&db, &automatic).await.unwrap();
    let first = source(&db, "model-new-one").await;
    sync_automatic_model_work(&db).await.unwrap();
    let (new, _, _) = configured(&db, &url, "synthetic-no-usage", Some(old)).await;
    let second = source(&db, "model-new-two").await;
    sync_automatic_model_work(&db).await.unwrap();
    let pairs:Vec<(Uuid,Uuid)>=sqlx::query_as("SELECT work.source_ref,b.config_ref FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work work USING(work_ref)").fetch_all(db.pool()).await.unwrap();
    assert!(pairs.contains(&(first, old)));
    assert!(pairs.contains(&(second, new)));
    assert!(!pairs.iter().any(|p| p.0 == historical));
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 1,
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(
        !run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let settings = read_model_settings(&db, true).await.unwrap();
    assert!(settings["runs"][0]["inputTokens"].is_null());
    assert_eq!(settings["plans"][0]["budgetUsed"], 18000);
    stop_model_plan(&db, automatic.plan_ref).await.unwrap();
    assert_eq!(
        start_model_plan(&db, &automatic).await.unwrap()["enabled"],
        false
    );
    let _third = source(&db, "model-new-three").await;
    assert_eq!(sync_automatic_model_work(&db).await.unwrap(), 0);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn invalid_output_cost_no_signal_and_finite_retry_are_distinct() {
    let db = fixture::proof_database("model_pi_failures").await;
    let (_server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    for (key, marker) in [
        ("invalid", "[BAD_OUTPUT]"),
        ("quiet", "[NO_SIGNAL]"),
        ("failure", "[FAIL_PROVIDER]"),
    ] {
        let reference = research_fixture::comment(
            &db,
            key,
            key,
            &format!("SYNTHETIC / NOT EVIDENCE {marker}"),
            "2026-09-06T02:00:00Z",
        )
        .await;
        start_model_plan(&db, &plan(config, "trial", vec![reference], 1, 54000))
            .await
            .unwrap();
        assert!(
            run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
                .await
                .unwrap()
        );
    }
    let invalid:(String,i64)=sqlx::query_as("SELECT state,charged_tokens FROM linggan_model_invocation WHERE failure_code='model_invalid_output'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(invalid, ("failed".into(), 900));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_analysis_work WHERE state='no_signal'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    sqlx::query("UPDATE linggan_comment_analysis_work SET updated_at=scope_001_now()-interval '31 seconds' WHERE failure_code='provider_unavailable'").execute(db.pool()).await.unwrap();
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    sqlx::query("UPDATE linggan_comment_analysis_work SET updated_at=scope_001_now()-interval '31 seconds' WHERE failure_code='provider_unavailable'").execute(db.pool()).await.unwrap();
    assert!(
        !run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze' AND failure_code='provider_unavailable'").fetch_one(db.pool()).await.unwrap(),2);
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT sum(charged_tokens)::bigint FROM linggan_model_invocation WHERE failure_code='provider_unavailable'").fetch_one(db.pool()).await.unwrap(),36000);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn simultaneous_workers_cannot_overspend_or_duplicate_unknown_usage_work() {
    let db = fixture::proof_database("model_pi_parallel_budget").await;
    let (_server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-no-usage", None).await;
    let refs = vec![
        source(&db, "parallel-one").await,
        source(&db, "parallel-two").await,
    ];
    start_model_plan(&db, &plan(config, "backfill", refs, 2, 18000))
        .await
        .unwrap();
    let adapter = PiAdapter::configured();
    let (a, b) = tokio::join!(
        run_model_work_once(&db, &SyntheticModelSecrets, &adapter),
        run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
    );
    assert_eq!(usize::from(a.unwrap()) + usize::from(b.unwrap()), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1
    );
    assert!(
        !run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn changed_connection_and_config_replay_do_not_restore_old_defaults_or_permissions() {
    let db = fixture::proof_database("model_pi_replay").await;
    let (_server, url) = fixture_server().await;
    let (config, connection, version) = configured(&db, &url, "synthetic-good", None).await;
    let current = read_model_settings(&db, true).await.unwrap();
    let model_ref = Uuid::parse_str(current["config"]["modelRef"].as_str().unwrap()).unwrap();
    let command = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: Some(config),
        model_ref,
        input_token_limit: 16000,
        output_token_limit: 2000,
        timeout_seconds: 3,
        max_attempts: 1,
        auto_source_limit: 2,
        auto_token_limit: 18000,
    };
    save_model_config(&db, &command).await.unwrap();
    let (new, _, _) = configured(&db, &url, "synthetic-no-usage", Some(command.config_ref)).await;
    assert_eq!(
        save_model_config(&db, &command).await.unwrap()["configRef"],
        new.to_string()
    );
    let change = SaveModelConnection {
        version_ref: Uuid::new_v4(),
        connection_ref: connection,
        expected_revision: 1,
        name: "新连接版本".into(),
        api: "openai-completions".into(),
        base_url: url.clone(),
        local_endpoint: true,
        api_key: "SYNTHETIC-NOT-A-CREDENTIAL".into(),
    };
    save_model_connection(&db, &SyntheticModelSecrets, &change)
        .await
        .unwrap();
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 2,
            enabled: false,
        },
    )
    .await
    .unwrap();
    save_model_connection(&db, &SyntheticModelSecrets, &change)
        .await
        .unwrap();
    assert!(matches!(
        connection_request(&db, &SyntheticModelSecrets, version).await,
        Err(ModelError::Disabled)
    ));
    assert!(matches!(
        save_model_config(
            &db,
            &SaveModelConfig {
                config_ref: Uuid::new_v4(),
                expected_config_ref: Some(new),
                ..command
            }
        )
        .await,
        Err(ModelError::Disabled)
    ));
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn configured_deadline_stops_real_sdk_wait_and_restricted_sources_never_dispatch() {
    use linggan_evidence::comment_research_read::restrict_comment_research_source;
    let db = fixture::proof_database("model_pi_deadline").await;
    let (_server, url) = fixture_server().await;
    let (previous, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let current = read_model_settings(&db, true).await.unwrap();
    let model_ref = Uuid::parse_str(current["config"]["modelRef"].as_str().unwrap()).unwrap();
    let config = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: Some(previous),
        model_ref,
        input_token_limit: 16000,
        output_token_limit: 2000,
        timeout_seconds: 1,
        max_attempts: 1,
        auto_source_limit: 2,
        auto_token_limit: 36000,
    };
    save_model_config(&db, &config).await.unwrap();
    let denied = source(&db, "restricted-dispatch").await;
    start_model_plan(
        &db,
        &plan(config.config_ref, "trial", vec![denied], 1, 18000),
    )
    .await
    .unwrap();
    restrict_comment_research_source(&db, denied, "合成撤回")
        .await
        .unwrap();
    assert!(
        !run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let source = research_fixture::comment(
        &db,
        "hang",
        "hang",
        "SYNTHETIC / NOT EVIDENCE [HANG_PROVIDER]",
        "2026-09-06T03:00:00Z",
    )
    .await;
    start_model_plan(
        &db,
        &plan(config.config_ref, "trial", vec![source], 1, 18000),
    )
    .await
    .unwrap();
    let start = std::time::Instant::now();
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(4));
    let result = read_model_settings(&db, true).await.unwrap();
    assert_eq!(result["runs"][0]["failureCode"], "provider_timeout");
    assert!(result["runs"][0]["inputTokens"].is_null());
    assert_eq!(result["runs"][0]["budgetAccounted"], 18000);
    assert!(
        !run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
}
