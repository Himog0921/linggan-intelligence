//! P3 A15 proof: a local Pi fixture drives the real adapter and shared invocation ledger, while
//! replay storage remains a hash/reference manifest and never becomes a production-analysis path.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::proof_database;
use linggan_evidence::comment_research_read::restrict_comment_research_source;
use linggan_intelligence::{
    comment_daily::{AutoPolicy, DailySchedule, OutdatedPolicy, clean_pending, save_schedule},
    comment_replay::{
        ConfigureAutoUpgradePolicy, CreateCandidateReplay, configure_auto_upgrade_policy,
        create_candidate_replay, read_replay, run_next_replay,
    },
    comment_replay_continuity::{
        enqueue_active_rule_health_replays, enqueue_difference_explanations,
        enqueue_replay_follow_ups, rollback_future_default_if_unhealthy,
        run_next_difference_explanation, run_next_replay_follow_up,
    },
    comment_research_rules::{
        AtomicKind, CreateRuleRevision, EditableFieldDefinition, RulePurpose, V4_RULE_REVISION_REF,
        create_candidate, read_active_rule,
    },
    comment_runtime::ContextPolicy,
    model_invocation::{ProbeModel, probe_model},
    model_secrets::SyntheticModelSecrets,
    model_settings::{
        SaveModelConfig, SaveModelConnection, SaveModelEntry, save_model_config,
        save_model_connection, save_model_entry,
    },
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use uuid::Uuid;

async fn fixture_server() -> (tokio::process::Child, String) {
    let mut child =
        tokio::process::Command::new("/Users/moglenny/.nvm/versions/node/v24.13.0/bin/node")
            .arg(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/support/comment_replay_fixture_server.mjs"),
            )
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
    (child, line.trim().to_owned())
}

async fn configured(db: &Database, url: &str, model_id: &str) -> Uuid {
    let connection = SaveModelConnection {
        version_ref: Uuid::new_v4(),
        connection_ref: Uuid::new_v4(),
        expected_revision: 0,
        name: "P3 本地回放模型".into(),
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
        expected_config_ref: None,
        model_ref,
        input_token_limit: 16_000,
        output_token_limit: 2_000,
        timeout_seconds: 5,
        max_attempts: 2,
        auto_source_limit: 120,
        auto_token_limit: 2_000_000,
    };
    save_model_config(db, &config).await.unwrap();
    config.config_ref
}

async fn clock(db: &Database, time: &str) {
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS replay_test_clock(singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),now_at timestamptz NOT NULL); CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT COALESCE((SELECT now_at FROM replay_test_clock WHERE singleton),clock_timestamp()) $$")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO replay_test_clock(singleton,now_at) VALUES(true,$1::timestamptz) ON CONFLICT(singleton) DO UPDATE SET now_at=EXCLUDED.now_at")
        .bind(time)
        .execute(db.pool())
        .await
        .unwrap();
}

async fn schedule(db: &Database, config_ref: Uuid, day_limit: i64, replay_limit: i64) {
    save_schedule(
        db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref,
            source_limit: 120,
            token_limit: day_limit,
            auto_policy: AutoPolicy {
                continuous_new: false,
                historical_enabled: false,
                history_start: None,
                outdated_policy: OutdatedPolicy::Disabled,
                unknown_retry_max_attempts: 0,
                unknown_retry_token_limit: 0,
                day_token_limit: day_limit,
                replay_token_limit: replay_limit,
                semantic_token_limit: 0,
            },
        },
    )
    .await
    .unwrap();
}

fn candidate(parent: Uuid, title: &str) -> CreateRuleRevision {
    CreateRuleRevision {
        rule_revision_ref: Uuid::new_v4(),
        parent_rule_revision_ref: parent,
        purpose: RulePurpose {
            title: title.into(),
            instruction: "仅抽取评论者直接表达的具体困难；每项必须有精确评论证据。".into(),
        },
        field_definitions: vec![EditableFieldDefinition {
            kind: AtomicKind::Problem,
            name: "困难".into(),
            definition: "评论者直接表达的阻碍、困扰或失败原因。".into(),
        }],
        examples: vec![],
    }
}

async fn sources(db: &Database, marker: Option<usize>) -> Vec<Uuid> {
    let mut refs = Vec::new();
    for work in 0..5 {
        let note = format!("p3-work-{work}");
        research_fixture::detail(db, &note, "SYNTHETIC / NOT EVIDENCE 回放作品").await;
        for index in 0..6 {
            let ordinal = work * 6 + index;
            let marker = (marker == Some(ordinal))
                .then_some(" [REPLAY_PARTIAL]")
                .unwrap_or("");
            refs.push(
                research_fixture::comment(
                    db,
                    &note,
                    &format!("p3-comment-{ordinal}"),
                    &format!("P3-RAW-SENTINEL-{ordinal} 我在开始执行时很费精力{marker}"),
                    "2026-09-08T10:00:00Z",
                )
                .await,
            );
        }
    }
    refs
}

async fn sources_from(
    db: &Database,
    prefix: &str,
    start: usize,
    count: usize,
    marker: Option<&str>,
) -> Vec<Uuid> {
    let mut refs = Vec::new();
    for offset in 0..count {
        let ordinal = start + offset;
        let work = ordinal % 5;
        let note = format!("{prefix}-work-{work}");
        research_fixture::detail(db, &note, "SYNTHETIC / NOT EVIDENCE 回放作品").await;
        refs.push(
            research_fixture::comment(
                db,
                &note,
                &format!("{prefix}-comment-{ordinal}"),
                &format!(
                    "P3-RAW-SENTINEL-{prefix}-{ordinal} 我在开始执行时很费精力 {}",
                    marker.unwrap_or("")
                ),
                "2026-09-08T10:00:00Z",
            )
            .await,
        );
    }
    refs
}

fn command(
    config_ref: Uuid,
    baseline: Uuid,
    candidate: Uuid,
    refs: Vec<Uuid>,
) -> CreateCandidateReplay {
    CreateCandidateReplay {
        run_ref: Uuid::new_v4(),
        sample_set_ref: Uuid::new_v4(),
        baseline_rule_revision_ref: baseline,
        candidate_rule_revision_ref: candidate,
        config_ref,
        source_refs: refs,
        seed: 17,
        ttl_seconds: 3600,
        context_policy: ContextPolicy {
            record_content: false,
            ..ContextPolicy::default()
        },
        thresholds: Default::default(),
        repeat_member_limit: 0,
        authorized_token_budget: 2_000_000,
    }
}

async fn exhaust(db: &Database, max_ticks: usize) {
    for _ in 0..max_ticks {
        if !run_next_replay(db, &SyntheticModelSecrets, &PiAdapter::configured(), None)
            .await
            .unwrap()
        {
            break;
        }
    }
}

async fn analyze_count(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn assert_no_production_or_raw_writes(db: &Database) {
    for (table, statement) in [
        (
            "linggan_comment_analysis_work",
            "SELECT count(*) FROM linggan_comment_analysis_work",
        ),
        (
            "linggan_ci_problem_candidate",
            "SELECT count(*) FROM linggan_ci_problem_candidate",
        ),
        (
            "linggan_ci_atom_projection",
            "SELECT count(*) FROM linggan_ci_atom_projection",
        ),
        (
            "linggan_comment_daily_packet",
            "SELECT count(*) FROM linggan_comment_daily_packet",
        ),
    ] {
        let count: i64 = production_count(db, statement).await;
        assert_eq!(count, 0, "replay wrote production table {table}");
    }
    for (table, statement) in [
        (
            "linggan_comment_replay_sample_set",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_comment_replay_sample_set x",
        ),
        (
            "linggan_comment_replay_member",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_comment_replay_member x",
        ),
        (
            "linggan_comment_replay_run",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_comment_replay_run x",
        ),
        (
            "linggan_comment_replay_item",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_comment_replay_item x",
        ),
        (
            "linggan_comment_replay_trace",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_comment_replay_trace x",
        ),
        (
            "linggan_comment_replay_explanation",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_comment_replay_explanation x",
        ),
        (
            "linggan_model_invocation",
            "SELECT COALESCE(string_agg(to_jsonb(x)::text,''),'') FROM linggan_model_invocation x",
        ),
    ] {
        let stored = replay_storage(db, statement).await;
        assert!(
            !stored.contains("P3-RAW-SENTINEL"),
            "raw comment escaped into {table}"
        );
    }
}

async fn production_count(db: &Database, statement: &'static str) -> i64 {
    sqlx::query_scalar(statement)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn replay_storage(db: &Database, statement: &'static str) -> String {
    sqlx::query_scalar(statement)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and the local P3 Pi fixture"]
async fn replay_freezes_pairs_then_adopts_and_excludes_a_withdrawn_v5_pair() {
    let db = proof_database("comment_replay_pairing_adoption").await;
    let (mut server, url) = fixture_server().await;
    assert!(url.starts_with("http://127.0.0.1:"));
    let config = configured(&db, &url, "replay-pass").await;
    schedule(&db, config, 4_000_000, 4_000_000).await;
    let refs = sources(&db, None).await;
    let first = create_candidate(&db, &candidate(V4_RULE_REVISION_REF, "首次回放候选"))
        .await
        .unwrap();
    configure_auto_upgrade_policy(
        &db,
        &ConfigureAutoUpgradePolicy {
            expected_revision: 0,
            enabled: true,
            selected_candidate_rule_revision_ref: Some(first.rule_revision_ref),
        },
    )
    .await
    .unwrap();
    let normal = command(
        config,
        V4_RULE_REVISION_REF,
        first.rule_revision_ref,
        refs.clone(),
    );
    let created = create_candidate_replay(&db, &normal).await.unwrap();
    assert_eq!(created["run"]["state"], "queued");
    assert_eq!(
        analyze_count(&db).await,
        0,
        "create is readonly to the model"
    );
    let replayed = create_candidate_replay(&db, &normal).await.unwrap();
    assert_eq!(replayed["run"]["runRef"], created["run"]["runRef"]);
    assert_eq!(
        analyze_count(&db).await,
        0,
        "idempotent create dispatches nothing"
    );
    let drain = ModelWorkerDrain::new();
    drain.request();
    assert!(
        !run_next_replay(
            &db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            Some(&drain)
        )
        .await
        .unwrap()
    );
    assert_eq!(
        analyze_count(&db).await,
        0,
        "draining worker cannot reserve a call"
    );

    assert_adopted_replay_and_explanation(&db, normal.run_ref, first.rule_revision_ref).await;
    assert_withdrawn_replay_stays_unadopted(&db, config, first.rule_revision_ref, &refs).await;
    assert_no_production_or_raw_writes(&db).await;
    server.kill().await.unwrap();
}

async fn assert_adopted_replay_and_explanation(db: &Database, run: Uuid, adopted: Uuid) {
    exhaust(db, 80).await;
    let passed = read_replay(db, run).await.unwrap();
    assert_eq!(passed["run"]["state"], "pass");
    assert!(passed["run"]["adoptionReceipt"].is_object());
    assert_eq!(
        read_active_rule(db).await.unwrap().rule_revision_ref,
        adopted
    );
    assert_eq!(analyze_count(db).await, 60);
    assert!(enqueue_difference_explanations(db).await.unwrap() > 0);
    assert!(
        run_next_difference_explanation(db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let explanation: Value = sqlx::query_scalar(
        "SELECT result FROM linggan_comment_replay_explanation WHERE state='succeeded' LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(explanation["stabilityNotAccuracy"], true);
    assert_eq!(explanation["citations"].as_array().unwrap().len(), 2);
    assert_eq!(analyze_count(db).await, 61, "explanation is metered");
}

async fn assert_withdrawn_replay_stays_unadopted(
    db: &Database,
    config: Uuid,
    first: Uuid,
    refs: &[Uuid],
) {
    let second = create_candidate(db, &candidate(first, "同架构迭代候选"))
        .await
        .unwrap();
    configure_auto_upgrade_policy(
        db,
        &ConfigureAutoUpgradePolicy {
            expected_revision: 1,
            enabled: true,
            selected_candidate_rule_revision_ref: Some(second.rule_revision_ref),
        },
    )
    .await
    .unwrap();
    let withdrawn = command(config, first, second.rule_revision_ref, refs.to_vec());
    create_candidate_replay(db, &withdrawn).await.unwrap();
    restrict_comment_research_source(db, refs[0], "synthetic P3 withdrawal")
        .await
        .unwrap();
    exhaust(db, 100).await;
    let detail = read_replay(db, withdrawn.run_ref).await.unwrap();
    assert_eq!(detail["run"]["state"], "insufficient_evidence");
    assert_eq!(detail["run"]["comparison"]["frozenCount"], 30);
    assert_eq!(detail["run"]["comparison"]["excludedCount"], 1);
    assert_eq!(detail["run"]["comparison"]["completedPairs"], 29);
    assert_eq!(
        detail["run"]["comparison"]["newDimensionsNotDirectlyCompared"],
        false
    );
    assert_eq!(read_active_rule(db).await.unwrap().rule_revision_ref, first);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and the local P3 Pi fixture"]
async fn unknown_cost_and_one_candidate_transport_failure_cannot_adopt() {
    let db = proof_database("comment_replay_unknown_partial").await;
    let (mut server, url) = fixture_server().await;
    let config = configured(&db, &url, "replay-unknown").await;
    schedule(&db, config, 2_000_000, 2_000_000).await;
    let refs = sources(&db, Some(0)).await;
    let candidate = create_candidate(&db, &candidate(V4_RULE_REVISION_REF, "部分失败演练候选"))
        .await
        .unwrap();
    configure_auto_upgrade_policy(
        &db,
        &ConfigureAutoUpgradePolicy {
            expected_revision: 0,
            enabled: true,
            selected_candidate_rule_revision_ref: Some(candidate.rule_revision_ref),
        },
    )
    .await
    .unwrap();
    let replay = command(
        config,
        V4_RULE_REVISION_REF,
        candidate.rule_revision_ref,
        refs,
    );
    create_candidate_replay(&db, &replay).await.unwrap();
    exhaust(&db, 80).await;
    let detail = read_replay(&db, replay.run_ref).await.unwrap();
    assert_eq!(detail["run"]["state"], "degraded");
    let reasons = detail["run"]["comparison"]["reasons"].as_array().unwrap();
    assert!(reasons.iter().any(|reason| reason == "unpaired_execution"));
    assert!(
        reasons
            .iter()
            .any(|reason| reason == "token_comparison_unknown")
    );
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        V4_RULE_REVISION_REF
    );
    assert_no_production_or_raw_writes(&db).await;
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and the local P3 Pi fixture"]
async fn shared_replay_subbudget_stops_before_a_second_reservation() {
    let db = proof_database("comment_replay_budget").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    let (mut server, url) = fixture_server().await;
    let config = configured(&db, &url, "replay-budget").await;
    schedule(&db, config, 18_000, 18_000).await;
    let refs = sources(&db, None).await;
    let candidate = create_candidate(&db, &candidate(V4_RULE_REVISION_REF, "预算演练候选"))
        .await
        .unwrap();
    let mut replay = command(
        config,
        V4_RULE_REVISION_REF,
        candidate.rule_revision_ref,
        refs.into_iter().take(2).collect(),
    );
    replay.authorized_token_budget = 36_000;
    create_candidate_replay(&db, &replay).await.unwrap();
    assert!(
        run_next_replay(&db, &SyntheticModelSecrets, &PiAdapter::configured(), None)
            .await
            .unwrap()
    );
    assert!(
        !run_next_replay(&db, &SyntheticModelSecrets, &PiAdapter::configured(), None)
            .await
            .unwrap()
    );
    let detail = read_replay(&db, replay.run_ref).await.unwrap();
    assert_eq!(detail["run"]["state"], "waiting_daily_budget");
    assert_eq!(detail["run"]["failureCode"], "day_budget_exhausted");
    assert_eq!(analyze_count(&db).await, 1);
    let metadata: Value =
        sqlx::query_scalar("SELECT result FROM linggan_model_invocation WHERE operation='analyze'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(metadata["budgetPurpose"], "replay");
    clock(&db, "2026-09-09 15:00:00Z").await;
    assert!(
        run_next_replay(&db, &SyntheticModelSecrets, &PiAdapter::configured(), None)
            .await
            .unwrap(),
        "the daily replay allowance can resume on the next execution day"
    );
    assert!(
        !run_next_replay(&db, &SyntheticModelSecrets, &PiAdapter::configured(), None)
            .await
            .unwrap(),
        "the first root authorization remains exhausted after the daily ledger resets"
    );
    let after = read_replay(&db, replay.run_ref).await.unwrap();
    assert_eq!(after["run"]["state"], "waiting_daily_budget");
    assert_eq!(
        after["run"]["failureCode"],
        "replay_authorized_budget_exhausted"
    );
    assert_eq!(analyze_count(&db).await, 2);
    assert_no_production_or_raw_writes(&db).await;
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and the local P3 Pi fixture"]
async fn insufficient_replay_reuses_ten_old_pairs_and_reopens_when_twenty_new_sources_arrive() {
    let db = proof_database("comment_replay_continuity_reuse").await;
    let (mut server, url) = fixture_server().await;
    let config = configured(&db, &url, "replay-pass").await;
    schedule(&db, config, 2_000_000, 2_000_000).await;
    let old_refs = sources_from(&db, "continuity", 0, 10, None).await;
    assert_eq!(clean_pending(&db).await.unwrap(), 10);
    let candidate = create_candidate(&db, &candidate(V4_RULE_REVISION_REF, "续跑候选"))
        .await
        .unwrap();
    let initial = command(
        config,
        V4_RULE_REVISION_REF,
        candidate.rule_revision_ref,
        old_refs,
    );
    create_candidate_replay(&db, &initial).await.unwrap();
    exhaust(&db, 30).await;
    let first = read_replay(&db, initial.run_ref).await.unwrap();
    assert_eq!(first["run"]["state"], "insufficient_evidence");
    assert_eq!(analyze_count(&db).await, 20);

    assert_no_new_follow_up_remains_reopenable(&db, initial.run_ref).await;
    let successor = create_continuity_successor(&db, initial.run_ref).await;
    assert_continuity_successor_completed_within_root_budget(&db, initial.run_ref, successor).await;
    assert_no_production_or_raw_writes(&db).await;
    server.kill().await.unwrap();
}

async fn assert_no_new_follow_up_remains_reopenable(db: &Database, prior: Uuid) {
    assert_eq!(enqueue_replay_follow_ups(db).await.unwrap(), 1);
    let state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_replay_follow_up WHERE prior_run_ref=$1",
    )
    .bind(prior)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "no_new_samples");
    assert!(!run_next_replay_follow_up(db).await.unwrap());
    let state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_replay_follow_up WHERE prior_run_ref=$1",
    )
    .bind(prior)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "no_new_samples");
}

async fn create_continuity_successor(db: &Database, prior: Uuid) -> Uuid {
    sources_from(db, "continuity", 10, 20, None).await;
    assert_eq!(clean_pending(db).await.unwrap(), 20);
    assert_eq!(enqueue_replay_follow_ups(db).await.unwrap(), 1);
    assert!(run_next_replay_follow_up(db).await.unwrap());
    let successor: Uuid = sqlx::query_scalar(
        "SELECT successor_run_ref FROM linggan_comment_replay_follow_up WHERE prior_run_ref=$1",
    )
    .bind(prior)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let detail = read_replay(db, successor).await.unwrap();
    assert_eq!(detail["run"]["authorizationRootRunRef"], prior.to_string());
    assert_eq!(detail["run"]["runKind"], "continuity");
    assert_eq!(detail["sampleSet"]["frozenCount"], 30);
    assert_eq!(
        detail["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["reusedFromItemRef"].is_string())
            .count(),
        20
    );
    assert_eq!(analyze_count(db).await, 20, "old pairs are not replayed");
    successor
}

async fn assert_continuity_successor_completed_within_root_budget(
    db: &Database,
    root: Uuid,
    successor: Uuid,
) {
    exhaust(db, 50).await;
    let completed = read_replay(db, successor).await.unwrap();
    assert_eq!(completed["run"]["state"], "pass");
    assert_eq!(completed["run"]["comparison"]["completedPairs"], 30);
    assert_eq!(completed["run"]["comparison"]["workCount"], 5);
    assert_eq!(analyze_count(db).await, 60, "only fresh pairs are metered");
    let reserved: i64 = sqlx::query_scalar("SELECT COALESCE(sum(reserved_tokens),0)::bigint FROM linggan_comment_replay_run WHERE COALESCE(authorization_root_run_ref,run_ref)=$1")
        .bind(root).fetch_one(db.pool()).await.unwrap();
    assert!(reserved <= 2_000_000);
    let actual: i64 = sqlx::query_scalar("SELECT COALESCE(sum(invocation.charged_tokens),0)::bigint FROM linggan_model_invocation invocation JOIN linggan_comment_replay_item item USING(invocation_ref) JOIN linggan_comment_replay_run run USING(run_ref) WHERE COALESCE(run.authorization_root_run_ref,run.run_ref)=$1")
        .bind(root).fetch_one(db.pool()).await.unwrap();
    assert!(actual <= 2_000_000);
    let old_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_replay_item WHERE run_ref=$1")
            .bind(root)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(old_rows, 20, "old members/items are not rewritten");
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and the local P3 Pi fixture"]
async fn adopted_active_rule_health_regression_reverts_only_future_pointer_with_receipt() {
    let db = proof_database("comment_replay_active_health_rollback").await;
    let (mut server, url) = fixture_server().await;
    let config = configured(&db, &url, "replay-pass").await;
    schedule(&db, config, 4_000_000, 4_000_000).await;
    let refs = sources_from(&db, "health-base", 0, 30, None).await;
    let candidate = create_candidate(&db, &candidate(V4_RULE_REVISION_REF, "健康回退候选"))
        .await
        .unwrap();
    configure_auto_upgrade_policy(
        &db,
        &ConfigureAutoUpgradePolicy {
            expected_revision: 0,
            enabled: true,
            selected_candidate_rule_revision_ref: Some(candidate.rule_revision_ref),
        },
    )
    .await
    .unwrap();
    let mut adopted = command(
        config,
        V4_RULE_REVISION_REF,
        candidate.rule_revision_ref,
        refs,
    );
    adopted.authorized_token_budget = 3_000_000;
    create_candidate_replay(&db, &adopted).await.unwrap();
    exhaust(&db, 80).await;
    assert_eq!(
        read_replay(&db, adopted.run_ref).await.unwrap()["run"]["state"],
        "pass"
    );
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        candidate.rule_revision_ref
    );

    sources_from(
        &db,
        "health-fresh",
        30,
        30,
        Some("[REPLAY_HEALTH_REGRESSION]"),
    )
    .await;
    assert_eq!(clean_pending(&db).await.unwrap(), 60);
    assert_eq!(enqueue_active_rule_health_replays(&db).await.unwrap(), 1);
    let health: Uuid = sqlx::query_scalar("SELECT run_ref FROM linggan_comment_replay_run WHERE authorization_root_run_ref=$1 AND run_kind='active_health'")
        .bind(adopted.run_ref).fetch_one(db.pool()).await.unwrap();
    let health_before = read_replay(&db, health).await.unwrap();
    assert_eq!(
        health_before["run"]["baselineRuleRevisionRef"],
        V4_RULE_REVISION_REF.to_string()
    );
    assert_eq!(
        health_before["run"]["candidateRuleRevisionRef"],
        candidate.rule_revision_ref.to_string()
    );
    exhaust(&db, 80).await;
    let health_after = read_replay(&db, health).await.unwrap();
    assert_eq!(health_after["run"]["state"], "degraded");
    assert_eq!(
        health_after["run"]["comparison"]["sameInputSameModel"],
        true
    );
    assert_eq!(health_after["run"]["comparison"]["supplierFailureCount"], 0);
    assert_eq!(
        health_after["run"]["comparison"]["promptAttributableHealthRegression"],
        true
    );
    assert_eq!(rollback_future_default_if_unhealthy(&db).await.unwrap(), 1);
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        V4_RULE_REVISION_REF
    );
    assert_eq!(rollback_future_default_if_unhealthy(&db).await.unwrap(), 0);
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_rule_rollback_receipt WHERE triggering_run_ref=$1",
    )
    .bind(health)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(receipts, 1);
    assert_no_production_or_raw_writes(&db).await;
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and the local P3 Pi fixture"]
async fn adopted_active_rule_health_supplier_failure_never_reverts_future_pointer() {
    let db = proof_database("comment_replay_active_health_supplier_failure").await;
    let (mut server, url) = fixture_server().await;
    let config = configured(&db, &url, "replay-pass").await;
    schedule(&db, config, 4_000_000, 4_000_000).await;
    let refs = sources_from(&db, "health-supplier-base", 0, 30, None).await;
    let candidate = create_candidate(&db, &candidate(V4_RULE_REVISION_REF, "健康供应商失败候选"))
        .await
        .unwrap();
    configure_auto_upgrade_policy(
        &db,
        &ConfigureAutoUpgradePolicy {
            expected_revision: 0,
            enabled: true,
            selected_candidate_rule_revision_ref: Some(candidate.rule_revision_ref),
        },
    )
    .await
    .unwrap();
    let mut adopted = command(
        config,
        V4_RULE_REVISION_REF,
        candidate.rule_revision_ref,
        refs,
    );
    adopted.authorized_token_budget = 3_000_000;
    create_candidate_replay(&db, &adopted).await.unwrap();
    exhaust(&db, 80).await;
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        candidate.rule_revision_ref
    );

    sources_from(
        &db,
        "health-supplier-fresh",
        30,
        30,
        Some("[REPLAY_HEALTH_SUPPLIER_FAILURE]"),
    )
    .await;
    assert_eq!(clean_pending(&db).await.unwrap(), 60);
    assert_eq!(enqueue_active_rule_health_replays(&db).await.unwrap(), 1);
    let health: Uuid = sqlx::query_scalar("SELECT run_ref FROM linggan_comment_replay_run WHERE authorization_root_run_ref=$1 AND run_kind='active_health'")
        .bind(adopted.run_ref).fetch_one(db.pool()).await.unwrap();
    exhaust(&db, 80).await;
    let comparison = read_replay(&db, health).await.unwrap()["run"]["comparison"].clone();
    assert_eq!(comparison["supplierFailureCount"], 30);
    assert_eq!(comparison["promptAttributableHealthRegression"], false);
    assert_eq!(rollback_future_default_if_unhealthy(&db).await.unwrap(), 0);
    assert_eq!(
        read_active_rule(&db).await.unwrap().rule_revision_ref,
        candidate.rule_revision_ref
    );
    let receipts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_rule_rollback_receipt WHERE triggering_run_ref=$1",
    )
    .bind(health)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(receipts, 0);
    assert_no_production_or_raw_writes(&db).await;
    server.kill().await.unwrap();
}
