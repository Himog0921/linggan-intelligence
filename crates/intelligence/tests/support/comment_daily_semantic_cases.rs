//! Semantic identity, context recovery, and grant continuation scenarios.
use super::*;

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn semantic_work_is_shared_across_batches_and_reobservations() {
    let db = fixture::proof_database("semantic_shared").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let first = source(&db, "shared", "a", "我每天提醒也没有用").await;
    let a = selected(&db, config, vec![first], 100000).await;
    let b = selected(&db, config, vec![first], 100000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=ANY($1)",
    )
    .bind(vec![a, b])
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    let refs: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT analysis_ref FROM linggan_comment_daily_item WHERE batch_ref=ANY($1)",
    )
    .bind(vec![a, b])
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(refs[0].is_some());
    assert_eq!(refs[0], refs[1]);
    let refreshed = source(&db, "shared", "a", "我每天提醒也没有用").await;
    let c = selected(&db, config, vec![refreshed], 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(c)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(n, 0);
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn malformed_single_item_keeps_other_semantics() {
    let db = fixture::proof_database("semantic_partial_schema").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let good = source(&db, "partial-schema", "good", "我不知道怎样坚持下去").await;
    let bad = source(&db, "partial-schema", "bad", "[BAD_SCHEMA] 这个结构非法").await;
    let batch = selected(&db, config, vec![good, bad], 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let good_state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=$2",
    )
    .bind(batch)
    .bind(good)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(good_state, "succeeded");
    let bad_code: String = sqlx::query_scalar(
        "SELECT failure_code FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=$2",
    )
    .bind(batch)
    .bind(bad)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(bad_code, "item_schema_invalid");
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn pause_after_dispatch_accepts_inflight_result() {
    let db = fixture::proof_database("semantic_pause_inflight").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let source = source(&db, "pause-flight", "a", "[DELAY_PROVIDER] 我想了解原因").await;
    let batch = selected(&db, config, vec![source], 100000).await;
    let db2 = db.clone();
    let task = tokio::spawn(async move {
        run_daily_once(&db2, &SyntheticModelSecrets, &PiAdapter::configured()).await
    });
    for _ in 0..200 {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1",
        )
        .bind(batch)
        .fetch_one(db.pool())
        .await
        .unwrap();
        if n > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    // Let the bounded adapter receive its request, then pause while the fixture is awaiting its response.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    set_batch_enabled(&db, batch, false).await.unwrap();
    task.await.unwrap().unwrap();
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "succeeded");
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn continuation_extends_only_undispatched_budget_and_retains_manifest() {
    let db = fixture::proof_database("semantic_continue").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let a = source(&db, "budget-work-a", "a", "我每天催促仍无法解决").await;
    let b = source(&db, "budget-work-b", "b", "我不知道如何开始第一步").await;
    let batch = selected(&db, config, vec![a, b], 18000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let command = ContinueDaily {
        command_ref: Uuid::new_v4(),
        source_limit: 2,
        token_limit: 36000,
        reason: "合成验证明确追加未派发额度".into(),
    };
    continue_batch(&db, batch, &command).await.unwrap();
    assert_eq!(
        continue_batch(&db, batch, &command).await.unwrap()["replayed"],
        true
    );
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let original: i64 = sqlx::query_scalar(
        "SELECT token_limit FROM linggan_comment_daily_batch WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(original, 18000);
    let complete: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='succeeded'",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(complete, 2);
    let data = overview(&db).await.unwrap();
    assert_eq!(data["items"][0]["tokenLimit"], 36000);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn late_context_recovers_original_target_after_debounce() {
    let db = fixture::proof_database("semantic_context_resume").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-08 01:00:00Z").await;
    let source = source(&db, "missing-context", "a", "我也是").await;
    let batch = selected(&db, config, vec![source], 100000).await;
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "context_missing");
    research_fixture::detail(&db, "missing-context", "补到的作品上下文").await;
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    clock(&db, "2026-09-08 01:01:01Z").await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "succeeded");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_batch")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    server.kill().await.unwrap();
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn daily_source_cap_defers_instead_of_losing_manifest_members() {
    let db = fixture::proof_database("semantic_source_continue").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-08 14:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 1,
            token_limit: 100000,
            auto_policy: AutoPolicy::new_intake_only(100000),
        },
    )
    .await
    .unwrap();
    source(&db, "capped", "a", "我需要了解如何开始").await;
    source(&db, "capped", "b", "我想知道怎样持续做下去").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let batch: Uuid =
        sqlx::query_scalar("SELECT batch_ref FROM linggan_comment_daily_batch WHERE kind='daily'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let pending:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='source_limit'").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(pending, 1);
    continue_batch(
        &db,
        batch,
        &ContinueDaily {
            command_ref: Uuid::new_v4(),
            source_limit: 2,
            token_limit: 100000,
            reason: "明确继续处理冻结剩余项".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        !tick(&db).await,
        "extending a batch must not bypass the shared day source cap"
    );
    clock(&db, "2026-09-09 01:00:00Z").await;
    assert!(
        tick(&db).await,
        "the already frozen remaining member resumes next day"
    );
    let complete: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='succeeded'",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(complete, 2);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn changed_text_follows_automatic_update_policy_and_shared_day_budget() {
    let db = fixture::proof_database("semantic_text_revision").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-08 14:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 10,
            token_limit: 18000,
            auto_policy: AutoPolicy {
                outdated_policy: OutdatedPolicy::CurrentOnly,
                ..AutoPolicy::new_intake_only(18000)
            },
        },
    )
    .await
    .unwrap();
    research_fixture::comment(
        &db,
        "revision",
        "a",
        "我不知道如何开始",
        "2026-09-08T14:01:00Z",
    )
    .await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert!(tick(&db).await);
    clock(&db, "2026-09-08 15:01:00Z").await;
    research_fixture::comment(
        &db,
        "revision",
        "a",
        "我不知道如何开始",
        "2026-09-08T15:01:00Z",
    )
    .await;
    assert!(
        !tick(&db).await,
        "same-body observation reuses the semantic result"
    );
    clock(&db, "2026-09-08 15:02:00Z").await;
    let changed = research_fixture::comment(
        &db,
        "revision",
        "a",
        "我已经开始了但不知道如何继续",
        "2026-09-08T15:02:00Z",
    )
    .await;
    assert!(
        !tick(&db).await,
        "remaining shared day budget cannot reserve another packet"
    );
    let backlog: Uuid = sqlx::query_scalar(
        "SELECT batch_ref FROM linggan_comment_daily_batch WHERE kind='backlog'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let state = item_state(&db, backlog, changed).await;
    assert_eq!(state, "pending");
    clock(&db, "2026-09-09 01:00:00Z").await;
    assert!(
        tick(&db).await,
        "next execution day resumes the same automatic recovery owner"
    );
    let state = item_state(&db, backlog, changed).await;
    assert_eq!(state, "succeeded");
    assert!(!tick(&db).await);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 2);
    research_fixture::comment(
        &db,
        "revision",
        "a",
        "我已经开始了但不知道如何继续",
        "2026-09-09T01:01:00Z",
    )
    .await;
    assert!(!tick(&db).await);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        calls, 2,
        "re-observing revised text never grants a third call"
    );
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn historical_callable_probe_allows_current_contract_without_retesting() {
    let db = fixture::proof_database("semantic_contract_qualification").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let source = source(&db, "contract", "a", "我不知道应该如何继续").await;
    sqlx::query("UPDATE linggan_model_invocation SET result=jsonb_set(result,'{commentContract}','\"comment-research.v2\"'::jsonb) WHERE operation='probe'").execute(db.pool()).await.unwrap();
    let result = create_selected(
        &db,
        &SelectedBatch {
            batch_ref: Uuid::new_v4(),
            config_ref: config,
            source_refs: vec![source],
            token_limit: 100000,
            reanalyze: false,
        },
    )
    .await;
    assert!(result.is_ok());
    assert!(tick(&db).await);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn semantic_retry_limit_cannot_be_reset_by_another_batch() {
    let db = fixture::proof_database("semantic_global_attempts").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let source = source(
        &db,
        "global-attempts",
        "a",
        "[MISSING] 持续遗漏本条的合成供应商结果",
    )
    .await;
    let first = selected(&db, config, vec![source], 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    assert_eq!(
        retry_failed(&db, first, Uuid::new_v4()).await.unwrap()["queued"],
        1
    );
    clock(&db, "2026-09-08 15:02:00Z").await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let diagnostic: serde_json::Value = sqlx::query_scalar("SELECT jsonb_build_object('semantics',(SELECT jsonb_agg(jsonb_build_object('state',state,'attempts',attempts,'failureCode',failure_code)) FROM linggan_comment_semantic_work),'items',(SELECT jsonb_agg(jsonb_build_object('state',state,'attempts',attempts,'failureCode',failure_code)) FROM linggan_comment_daily_item))")
        .fetch_one(db.pool()).await.unwrap();
    let calls_so_far: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        calls_so_far, 2,
        "second allowed call must actually execute: {diagnostic}"
    );
    let second = selected(&db, config, vec![source], 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    assert_eq!(
        retry_failed(&db, second, Uuid::new_v4()).await.unwrap()["queued"],
        0
    );
    let attempts: i32 = sqlx::query_scalar(
        "SELECT attempts FROM linggan_comment_semantic_work WHERE source_ref=$1",
    )
    .bind(source)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(attempts, 2);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 2);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn extraction_does_not_recall_or_decide_problem_equivalence() {
    let db = fixture::proof_database("semantic_candidate_proposal").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let first = source(
        &db,
        "existing-problem",
        "a",
        "执行功能出现困难，需要每天提醒",
    )
    .await;
    let batch = selected(&db, config, vec![first], 100000).await;
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
    let analysis: Uuid = sqlx::query_scalar(
        "SELECT analysis_ref FROM linggan_comment_daily_item WHERE batch_ref=$1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM linggan_ci_source WHERE source_ref=$1")
            .bind(first)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let problem = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,definition,origin) VALUES($1,$2,'执行功能困难','已经知道方法仍然无法继续执行','{\"boundary\":\"已知道具体方法\"}'::jsonb,'manual')").bind(problem).bind(domain).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,analysis_ref) SELECT $1,canonical_ref,'manual',$3 FROM linggan_ci_source WHERE source_ref=$2").bind(problem).bind(first).bind(analysis).execute(db.pool()).await.unwrap();
    let next = source(
        &db,
        "existing-problem-next",
        "b",
        "[ASSIGN_EXISTING] 执行功能困难，已经学会却难以坚持",
    )
    .await;
    let next_batch = selected(&db, config, vec![next], 100000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let result:serde_json::Value=sqlx::query_scalar("SELECT a.result FROM linggan_comment_daily_item i JOIN linggan_comment_analysis_work a ON a.work_ref=i.analysis_ref WHERE i.batch_ref=$1").bind(next_batch).fetch_one(db.pool()).await.unwrap();
    assert!(result["semantic"]["problems"][0]["candidateRef"].is_null());
    assert_eq!(
        result["semantic"]["problems"][0]["serverValidatedCandidate"],
        false
    );
    assert_eq!(result["candidateSnapshot"], json!([]));
    assert!(
        !result["contextRefs"]["researchSourceRefs"]
            .as_array()
            .unwrap()
            .contains(&json!(first))
    );
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn model_readiness_separates_callability_from_comment_contract_diagnostics() {
    use linggan_intelligence::model_settings_read::{
        current_comment_model_state, read_model_settings,
    };
    let db = fixture::proof_database("model_readiness").await;
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "NOT_CONFIGURED"
    );
    let (mut server, url) = fixture_server().await;
    // configured probes the actual synthetic provider before saving any default.
    let (config, connection, _) = configured(&db, &url, "synthetic-good", None).await;
    let mut data = read_model_settings(&db, true).await.unwrap();
    assert_eq!(data["model"]["modelState"], "CONFIGURED");
    assert_eq!(data["models"][0]["commentQualified"], true);
    assert_eq!(data["models"][0]["currentVersion"], true);
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=NULL")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "NEEDS_SELECTION"
    );
    sqlx::query("UPDATE linggan_model_invocation SET result=jsonb_set(result,'{commentContract}','\"comment-research.v2\"'::jsonb) WHERE operation='probe'").execute(db.pool()).await.unwrap();
    data = read_model_settings(&db, true).await.unwrap();
    assert_eq!(data["model"]["modelState"], "NEEDS_SELECTION");
    assert_eq!(data["models"][0]["modelCallable"], true);
    assert_eq!(data["models"][0]["commentQualified"], false);
    let request = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        expected_config_ref: None,
        model_ref: serde_json::from_value(data["models"][0]["modelRef"].clone()).unwrap(),
        input_token_limit: 16000,
        output_token_limit: 2000,
        timeout_seconds: 5,
        max_attempts: 2,
        auto_source_limit: 10,
        auto_token_limit: 100000,
    };
    save_model_config(&db, &request).await.unwrap();
    sqlx::query("UPDATE linggan_model_invocation SET result=jsonb_set(result,'{commentContract}',to_jsonb($1::text)),state='failed' WHERE operation='probe'").bind(DAILY_RULE).execute(db.pool()).await.unwrap();
    assert_eq!(
        read_model_settings(&db, true).await.unwrap()["models"][0]["commentQualified"],
        false
    );
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=$1")
        .bind(config)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelConnected"],
        false
    );
    sqlx::query("UPDATE linggan_model_invocation SET state='succeeded' WHERE operation='probe'")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelConnected"],
        true
    );
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=NULL")
        .execute(db.pool())
        .await
        .unwrap();
    let request = SaveModelConfig {
        config_ref: Uuid::new_v4(),
        ..request
    };
    save_model_config(&db, &request).await.unwrap();
    assert_connection_pause_round_trip(&db, connection).await;
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=NULL")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        current_comment_model_state(&db).await.unwrap()["modelState"],
        "NEEDS_SELECTION"
    );
    let calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE operation='analyze'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(calls, 0);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn next_daily_grant_takes_only_never_dispatched_backlog_without_duplicate_ownership() {
    let db = fixture::proof_database("daily_backlog_owner").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-07 14:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 1,
            token_limit: 100000,
            auto_policy: AutoPolicy::new_intake_only(100000),
        },
    )
    .await
    .unwrap();
    let first = source(&db, "backlog-one", "a", "这是第一条待研究的原声").await;
    let second = source(&db, "backlog-two", "b", "这是第二条待研究的原声").await;
    clock(&db, "2026-09-07 15:00:00Z").await;
    assert!(seal_due(&db).await.unwrap());
    let origin: Uuid =
        sqlx::query_scalar("SELECT batch_ref FROM linggan_comment_daily_batch WHERE kind='daily'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(tick(&db).await);
    assert!(!tick(&db).await);
    let deferred:Uuid=sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='source_limit'").bind(origin).fetch_one(db.pool()).await.unwrap();
    assert!([first, second].contains(&deferred));
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert!(seal_due(&db).await.unwrap());
    let state: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=$2",
    )
    .bind(origin)
    .bind(deferred)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, "carried_forward");
    let carried=sqlx::query("SELECT i.batch_ref,i.origin_batch_ref,b.request FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) WHERE i.source_ref=$1 AND i.batch_ref<>$2").bind(deferred).bind(origin).fetch_one(db.pool()).await.unwrap();
    assert_eq!(carried.get::<Uuid, _>("origin_batch_ref"), origin);
    assert_eq!(
        carried.get::<serde_json::Value, _>("request")["backlogCount"],
        1
    );
    assert_eq!(
        carried.get::<serde_json::Value, _>("request")["newIntakeCount"],
        0
    );
    // Increasing the old grant cannot reclaim transferred ownership.
    continue_batch(
        &db,
        origin,
        &ContinueDaily {
            command_ref: Uuid::new_v4(),
            source_limit: 2,
            token_limit: 100000,
            reason: "验证不能重复领取已接续来源".into(),
        },
    )
    .await
    .unwrap();
    assert!(tick(&db).await);
    assert!(!tick(&db).await);
    let packets: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_packet WHERE $1=ANY(source_refs)",
    )
    .bind(deferred)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(packets, 1);
    let oldstate: String = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=$2",
    )
    .bind(origin)
    .bind(deferred)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(oldstate, "carried_forward");
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn accepted_semantics_reuse_even_when_the_batch_has_no_remaining_call_budget() {
    let db = fixture::proof_database("reuse_exhausted_budget").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let voice = source(&db, "reuse-no-budget", "a", "我需要知道怎样把方法坚持下去").await;
    let first = selected(&db, config, vec![voice], 18000).await;
    assert!(tick(&db).await);
    let second = selected(&db, config, vec![voice], 18000).await;
    // Isolated ledger fixture: the second batch already consumed its allowance elsewhere.
    let billed = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,finished_at) SELECT $1,connection_version_ref,model_ref,config_ref,'analyze',request_hash,'succeeded',18000,18000,17000,1000,'{}',scope_001_now() FROM linggan_model_invocation WHERE config_ref=$2 AND operation='analyze' LIMIT 1").bind(billed).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_daily_packet(packet_ref,batch_ref,source_refs,context_refs,context_hash,invocation_ref,lease_until,state) VALUES($1,$2,'{}','{}','synthetic-accounting-fixture',$3,scope_001_now(),'succeeded')").bind(Uuid::new_v4()).bind(second).bind(billed).execute(db.pool()).await.unwrap();
    assert!(!tick(&db).await);
    let results: Vec<Uuid> = sqlx::query_scalar(
        "SELECT analysis_ref FROM linggan_comment_daily_item WHERE batch_ref=ANY($1)",
    )
    .bind(vec![first, second])
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(results[0], results[1]);
    let calls: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(second)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(calls, 1);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn field_rejection_retains_other_extractions_and_precise_diagnostics() {
    let db = fixture::proof_database("daily_partial_field").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let voice = source(
        &db,
        "partial-field",
        "a",
        "[PARTIAL_FIELD] 我需要知道怎样坚持",
    )
    .await;
    let batch = selected(&db, config, vec![voice], 100000).await;
    assert!(tick(&db).await);
    let result:serde_json::Value=sqlx::query_scalar("SELECT a.result FROM linggan_comment_daily_item i JOIN linggan_comment_analysis_work a ON a.work_ref=i.analysis_ref WHERE i.batch_ref=$1 AND i.state='succeeded'").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(result["semantic"]["acceptance"], "partial");
    assert_eq!(result["semantic"]["problems"].as_array().unwrap().len(), 1);
    let diagnostics:serde_json::Value=sqlx::query_scalar("SELECT t.validation FROM linggan_comment_request_trace t JOIN linggan_comment_daily_packet p USING(packet_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(diagnostics[0]["path"], "$.comments[0].labels[0].label");
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn automatic_transient_retry_obeys_cooldown_attempt_cap_and_unknown_usage() {
    let db = fixture::proof_database("daily_auto_retry").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-08 01:00:00Z").await;
    let voice = source(&db, "retry-known", "a", "[BAD_QUOTE] 合成可核账的失败").await;
    let batch = selected(&db, config, vec![voice], 100000).await;
    assert!(tick(&db).await);
    // Only the classified failure is substituted in this isolated accounting fixture. Actual
    // invocation and known usage came through the local SDK; eligibility is the subject of this test.
    sqlx::query("UPDATE linggan_comment_semantic_work SET failure_code='provider_unavailable' WHERE semantic_ref=(SELECT semantic_ref FROM linggan_comment_daily_item WHERE batch_ref=$1)").bind(batch).execute(db.pool()).await.unwrap();
    assert!(!tick(&db).await);
    clock(&db, "2026-09-08 01:01:01Z").await;
    assert!(tick(&db).await);
    sqlx::query("UPDATE linggan_comment_semantic_work SET failure_code='provider_unavailable' WHERE semantic_ref=(SELECT semantic_ref FROM linggan_comment_daily_item WHERE batch_ref=$1)").bind(batch).execute(db.pool()).await.unwrap();
    clock(&db, "2026-09-08 01:02:02Z").await;
    assert!(!tick(&db).await);
    let attempts: i32 =
        sqlx::query_scalar("SELECT attempts FROM linggan_comment_daily_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(attempts, 2);
    let (other, _, _) = configured(&db, &url, "synthetic-no-usage", Some(config)).await;
    let unknown = source(&db, "retry-unknown", "a", "[BAD_QUOTE] 合成用量未知的失败").await;
    let unknown_batch = selected(&db, other, vec![unknown], 100000).await;
    assert!(tick(&db).await);
    sqlx::query("UPDATE linggan_comment_semantic_work SET failure_code='provider_unavailable' WHERE semantic_ref=(SELECT semantic_ref FROM linggan_comment_daily_item WHERE batch_ref=$1)").bind(unknown_batch).execute(db.pool()).await.unwrap();
    clock(&db, "2026-09-08 01:04:00Z").await;
    assert!(!tick(&db).await);
    let calls: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(unknown_batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(calls, 1);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn legacy_batch_contract_is_readable_but_cannot_be_reactivated_or_executed() {
    let db = fixture::proof_database("daily_legacy_grant").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let voice = source(&db, "legacy-grant", "a", "旧规则下冻结的原声").await;
    let legacy = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request) VALUES($1,'selected',$2,scope_001_now(),scope_001_now(),1,100000,'{\"ruleVersion\":\"comment-research.v3\",\"cleanerVersion\":\"comment-clean.v1\"}')").bind(legacy).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_daily_item(batch_ref,source_ref) VALUES($1,$2)")
        .bind(legacy)
        .bind(voice)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(!tick(&db).await);
    assert!(set_batch_enabled(&db, legacy, true).await.is_err());
    let detail = batch_detail(&db, legacy, None).await.unwrap();
    assert_eq!(detail["items"][0]["state"], "pending");
    assert_eq!(detail["items"][0]["cleanState"], serde_json::Value::Null);
    let calls: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(legacy)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(calls, 0);
    server.kill().await.unwrap();
}

async fn item_state(db: &Database, batch: Uuid, source: Uuid) -> String {
    sqlx::query_scalar(
        "SELECT state FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=$2",
    )
    .bind(batch)
    .bind(source)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

async fn assert_connection_pause_round_trip(db: &Database, connection: Uuid) {
    use linggan_intelligence::model_settings_read::{
        current_comment_model_state, read_model_settings,
    };
    set_model_connection_enabled(
        db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 1,
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        current_comment_model_state(db).await.unwrap()["modelState"],
        "PAUSED"
    );
    set_model_connection_enabled(
        db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 2,
            enabled: true,
        },
    )
    .await
    .unwrap();
    let data = read_model_settings(db, true).await.unwrap();
    assert_eq!(data["model"]["modelState"], "CONFIGURED");
    // Enable/disable increments the connection revision without creating a version.
    assert_eq!(data["models"][0]["currentVersion"], true);
}
