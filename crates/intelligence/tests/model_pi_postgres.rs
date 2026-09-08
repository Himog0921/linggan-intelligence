// Historical pre-0047 plan contract. V3 dispatch/retirement is proved by comment_intelligence and daily tests.
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
    let db = fixture::proof_database_before_comment_intelligence("model_pi_end_to_end").await;
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
    let db = fixture::proof_database_before_comment_intelligence("model_pi_auto").await;
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
    let db = fixture::proof_database_before_comment_intelligence("model_pi_failures").await;
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
    let db = fixture::proof_database_before_comment_intelligence("model_pi_parallel_budget").await;
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
    let db = fixture::proof_database_before_comment_intelligence("model_pi_replay").await;
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
    let db = fixture::proof_database_before_comment_intelligence("model_pi_deadline").await;
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

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn paused_owner_is_locatable_and_only_a_new_revision_command_resumes_its_remaining_work() {
    let db = fixture::proof_database_before_comment_intelligence("model_pi_resume_owner").await;
    let (_server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let refs = vec![
        source(&db, "resume-one").await,
        source(&db, "resume-two").await,
    ];
    let original = plan(config, "backfill", refs.clone(), 2, 36000);
    start_model_plan(&db, &original).await.unwrap();
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    stop_model_plan(&db, original.plan_ref).await.unwrap();
    let duplicate = plan(config, "backfill", refs, 2, 36000);
    let result = start_model_plan(&db, &duplicate).await.unwrap();
    assert_eq!(result["queued"], 0);
    assert_eq!(
        result["existingPlans"][0]["planRef"],
        original.plan_ref.to_string()
    );
    let focused = read_model_settings_with_plan(&db, true, Some(original.plan_ref))
        .await
        .unwrap();
    assert_eq!(
        focused["plans"][0]["planRef"],
        original.plan_ref.to_string()
    );
    assert_eq!(focused["plans"][0]["budgetUsed"], 900);
    assert_eq!(focused["plans"][0]["revision"], 1);
    assert_eq!(
        start_model_plan(&db, &original).await.unwrap()["enabled"],
        false
    );
    // A later default must not rewrite the paused plan's frozen tasks.
    configured(&db, &url, "synthetic-no-usage", Some(config)).await;
    let resume = ResumeModelPlan {
        expected_revision: 1,
    };
    resume_model_plan(&db, original.plan_ref, &resume)
        .await
        .unwrap();
    stop_model_plan(&db, original.plan_ref).await.unwrap();
    assert!(matches!(
        resume_model_plan(&db, original.plan_ref, &resume).await,
        Err(ModelError::Conflict)
    ));
    assert_eq!(
        start_model_plan(&db, &original).await.unwrap()["enabled"],
        false
    );
    resume_model_plan(
        &db,
        original.plan_ref,
        &ResumeModelPlan {
            expected_revision: 3,
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
    let snapshot = read_model_settings_with_plan(&db, true, Some(original.plan_ref))
        .await
        .unwrap();
    assert_eq!(snapshot["plans"][0]["sourceCount"], 2);
    assert_eq!(snapshot["plans"][0]["budgetUsed"], 1800);
    assert_eq!(snapshot["plans"][0]["tokenLimit"], 36000);
    assert_eq!(snapshot["plans"][0]["finishedCount"], 2);
    let work:Vec<(Uuid,i32,String)>=sqlx::query_as("SELECT b.config_ref,w.attempts,w.state FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work w USING(work_ref) WHERE plan_ref=$1").bind(original.plan_ref).fetch_all(db.pool()).await.unwrap();
    assert!(
        work.iter()
            .all(|(c, a, s)| *c == config && *a == 1 && s == "succeeded")
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn automatic_selection_skips_global_trial_work_before_its_one_source_limit() {
    let db = fixture::proof_database_before_comment_intelligence("model_pi_global_selection").await;
    let (_server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    for n in 0..24 {
        source(&db, &format!("adjacent-{n}")).await;
    }
    let ordered:Vec<Uuid>=sqlx::query_scalar("SELECT material_ref FROM linggan_comment_research_readable ORDER BY created_at,material_ref").fetch_all(db.pool()).await.unwrap();
    let trial = plan(config, "trial", vec![ordered[0]], 1, 18000);
    start_model_plan(&db, &trial).await.unwrap();
    stop_model_plan(&db, trial.plan_ref).await.unwrap();
    let automatic = plan(config, "automatic", vec![], 1, 18000);
    start_model_plan(&db, &automatic).await.unwrap();
    // Only this isolated plan's activation is shifted; Evidence remains immutable.
    sqlx::query("UPDATE linggan_model_plan SET created_at='1970-01-01' WHERE plan_ref=$1")
        .bind(automatic.plan_ref)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(sync_automatic_model_work(&db).await.unwrap(), 1);
    assert_eq!(sync_automatic_model_work(&db).await.unwrap(), 0);
    let chosen:Uuid=sqlx::query_scalar("SELECT w.source_ref FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work w USING(work_ref) WHERE b.plan_ref=$1").bind(automatic.plan_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(chosen, ordered[1]);
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
    assert_eq!(
        read_model_settings_with_plan(&db, true, Some(automatic.plan_ref))
            .await
            .unwrap()["plans"][0]["sourceCount"],
        1
    );
}

async fn interrupted_fixture(
    db: &Database,
    config: Uuid,
    cap: i32,
    budget: i64,
    success: bool,
    key: &str,
) -> Uuid {
    use linggan_intelligence::comment_analysis::*;
    let reference = source(db, key).await;
    let grant = plan(config, "trial", vec![reference], 1, budget);
    start_model_plan(db, &grant).await.unwrap();
    let work: Uuid =
        sqlx::query_scalar("SELECT work_ref FROM linggan_comment_model_work WHERE plan_ref=$1")
            .bind(grant.plan_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    if success {
        let input = claim_selected_comment_analysis(db, &model_version(config), Some(work))
            .await
            .unwrap()
            .unwrap();
        complete_comment_analysis(
            db,
            &input,
            &CommentAnalysisOutput {
                source_ref: input.source_ref,
                source_sha256: input.source_sha256.clone(),
                spans: vec![],
                limitations: vec![],
            },
        )
        .await
        .unwrap();
    } else {
        sqlx::query("UPDATE linggan_comment_analysis_work SET state='running',attempts=$2,lease_ref=gen_random_uuid(),lease_until=scope_001_now()-interval '1 second' WHERE work_ref=$1").bind(work).bind(cap).execute(db.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,work_ref,plan_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,created_at,result) SELECT gen_random_uuid(),m.connection_version_ref,m.model_ref,$1,$2,c.config_ref,'analyze','synthetic interruption','running',18000,18000,scope_001_now()-interval '121 seconds','{\"validationPending\":true}'::jsonb FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$3")
        .bind(work).bind(grant.plan_ref).bind(config).execute(db.pool()).await.unwrap();
    grant.plan_ref
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn maintenance_commits_without_dispatch_and_honors_each_frozen_attempt_cap() {
    let db = fixture::proof_database_before_comment_intelligence("model_pi_recovery_idle").await;
    let (_server, url) = fixture_server().await;
    let (initial, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let current = read_model_settings(&db, true).await.unwrap();
    let model_ref = Uuid::parse_str(current["config"]["modelRef"].as_str().unwrap()).unwrap();
    let mut previous = initial;
    for (key, max, attempts, budget, success, expected) in [
        ("cap-one", 1, 1, 36000, false, "failed"),
        ("cap-two", 2, 2, 36000, false, "failed"),
        ("no-budget", 2, 1, 18000, false, "pending"),
        ("completed", 1, 1, 18000, true, "no_signal"),
        ("legacy-pending", 1, 1, 36000, false, "failed"),
    ] {
        let config = SaveModelConfig {
            config_ref: Uuid::new_v4(),
            expected_config_ref: Some(previous),
            model_ref,
            input_token_limit: 16000,
            output_token_limit: 2000,
            timeout_seconds: 1,
            max_attempts: max,
            auto_source_limit: 1,
            auto_token_limit: 36000,
        };
        save_model_config(&db, &config).await.unwrap();
        previous = config.config_ref;
        let grant =
            interrupted_fixture(&db, config.config_ref, attempts, budget, success, key).await;
        if key == "legacy-pending" {
            sqlx::query("UPDATE linggan_comment_analysis_work SET state='pending',lease_ref=NULL,lease_until=NULL,failure_code='lease_expired' WHERE work_ref IN(SELECT work_ref FROM linggan_comment_model_work WHERE plan_ref=$1)").bind(grant).execute(db.pool()).await.unwrap();
        }
        assert!(
            !run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
                .await
                .unwrap()
        );
        let state = read_model_settings_with_plan(&db, true, Some(grant))
            .await
            .unwrap();
        assert_eq!(state["plans"][0]["runningCount"], 0);
        assert_eq!(state["plans"][0]["budgetUsed"], 18000);
        let work:(String,i32)=sqlx::query_as("SELECT w.state,w.attempts FROM linggan_comment_analysis_work w JOIN linggan_comment_model_work b USING(work_ref) WHERE b.plan_ref=$1").bind(grant).fetch_one(db.pool()).await.unwrap();
        assert_eq!(work, (expected.into(), attempts));
        let receipt:(String,Option<i64>,i64,serde_json::Value)=sqlx::query_as("SELECT state,input_tokens,charged_tokens,result FROM linggan_model_invocation WHERE plan_ref=$1").bind(grant).fetch_one(db.pool()).await.unwrap();
        assert_eq!(receipt.0, if success { "succeeded" } else { "failed" });
        assert!(receipt.1.is_none());
        assert_eq!(receipt.2, 18000);
        assert!(receipt.3.get("validationPending").is_none());
    }
    assert!(
        !run_model_work_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        5
    );
}

#[test]
fn api_address_normalization_preserves_gateway_prefix_and_rejects_embedded_secrets() {
    for (api, input, expected) in [
        (
            "openai-completions",
            " https://example.com/ ",
            "https://example.com/v1",
        ),
        (
            "openai-completions",
            "https://example.com/v1/chat/completions",
            "https://example.com/v1",
        ),
        (
            "openai-responses",
            "https://example.com/gateway/v2/responses/",
            "https://example.com/gateway/v2",
        ),
        (
            "anthropic-messages",
            "https://example.com/v1/messages",
            "https://example.com",
        ),
        (
            "anthropic-messages",
            "https://example.com/gateway/v1/",
            "https://example.com/gateway",
        ),
    ] {
        assert_eq!(
            normalize_model_endpoint(input, api, false).unwrap(),
            expected
        );
    }
    for input in [
        "https://user:key@example.com",
        "https://example.com?key=secret",
        "http://remote.example.com/v1",
    ] {
        assert!(normalize_model_endpoint(input, "openai-completions", false).is_err());
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness and local synthetic SDK server"]
async fn full_api_address_and_retained_key_reach_model_without_catalog() {
    let db =
        fixture::proof_database_before_comment_intelligence("model_pi_repair_connection").await;
    let (_server, url) = fixture_server().await;
    let mut connection = SaveModelConnection {
        version_ref: Uuid::new_v4(),
        connection_ref: Uuid::new_v4(),
        expected_revision: 0,
        name: "合成连接修复".into(),
        api: "openai-completions".into(),
        base_url: format!("{url}/chat/completions"),
        local_endpoint: true,
        api_key: "SYNTHETIC-NOT-A-CREDENTIAL".into(),
    };
    save_model_connection(&db, &SyntheticModelSecrets, &connection)
        .await
        .unwrap();
    let read = read_model_settings(&db, true).await.unwrap();
    assert_eq!(read["connections"][0]["baseUrl"], url);
    connection.version_ref = Uuid::new_v4();
    connection.expected_revision = 1;
    connection.base_url = url.clone();
    connection.api_key.clear();
    connection.name = "更名后沿用凭据".into();
    save_model_connection(&db, &SyntheticModelSecrets, &connection)
        .await
        .unwrap();
    assert!(
        save_model_connection(&db, &SyntheticModelSecrets, &connection)
            .await
            .unwrap()["replayed"]
            .as_bool()
            .unwrap()
    );
    let model_ref = Uuid::new_v4();
    save_model_entry(
        &db,
        &SaveModelEntry {
            model_ref,
            connection_version_ref: connection.version_ref,
            model_id: "synthetic-good".into(),
        },
    )
    .await
    .unwrap();
    let result = probe_model(
        &db,
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
    assert_eq!(result["modelCallable"], true);
    assert_eq!(result["commentQualified"], true);
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation WHERE operation IN ('connect','discover')").fetch_one(db.pool()).await.unwrap(), 0);
    connection.version_ref = Uuid::new_v4();
    connection.expected_revision = 2;
    connection.base_url = format!("{url}/different");
    assert!(matches!(
        save_model_connection(&db, &SyntheticModelSecrets, &connection).await,
        Err(ModelError::Invalid)
    ));
    assert_eq!(
        read_model_settings(&db, true).await.unwrap()["connections"][0]["revision"],
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_model_work")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn missing_model_schema_has_a_specific_recoverable_error() {
    let db = fixture::proof_database_before_comment_intelligence("model_pi_repair_schema").await;
    sqlx::query(
        "ALTER TABLE linggan_model_workspace RENAME TO repair_temporarily_missing_workspace",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        read_model_settings(&db, true).await,
        Err(ModelError::SchemaMissing)
    ));
    sqlx::query(
        "ALTER TABLE repair_temporarily_missing_workspace RENAME TO linggan_model_workspace",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(read_model_settings(&db, true).await.is_ok());
}
