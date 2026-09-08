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
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
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
async fn daily_text_revision_reuses_original_grant_but_reobservation_does_not() {
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
        },
    )
    .await
    .unwrap();
    source(&db, "revision", "a", "我不知道如何开始").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert!(tick(&db).await);
    let origin: Uuid =
        sqlx::query_scalar("SELECT batch_ref FROM linggan_comment_daily_batch WHERE kind='daily'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    clock(&db, "2026-09-08 15:01:00Z").await;
    source(&db, "revision", "a", "我不知道如何开始").await;
    tick(&db).await;
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_batch WHERE kind='supplement'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(n, 0);
    clock(&db, "2026-09-08 15:02:00Z").await;
    let changed = source(&db, "revision", "a", "我已经开始了但不知道如何继续").await;
    // A supplement consumes the original budget, so the first call's reservation prevents a second dispatch.
    assert!(!tick(&db).await);
    let supplement: Uuid = sqlx::query_scalar(
        "SELECT batch_ref FROM linggan_comment_daily_batch WHERE kind='supplement'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    continue_batch(
        &db,
        supplement,
        &ContinueDaily {
            command_ref: Uuid::new_v4(),
            source_limit: 10,
            token_limit: 36000,
            reason: "继续原日批的修订处理".into(),
        },
    )
    .await
    .unwrap();
    assert!(tick(&db).await);
    let r=sqlx::query("SELECT i.source_ref,i.state,b.request FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) WHERE b.batch_ref=$1").bind(supplement).fetch_one(db.pool()).await.unwrap();
    assert_eq!(r.get::<Uuid, _>("source_ref"), changed);
    assert_eq!(r.get::<String, _>("state"), "succeeded");
    assert_eq!(
        r.get::<serde_json::Value, _>("request")["originBatchRef"],
        origin.to_string()
    );
    assert!(!tick(&db).await);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 2);
    clock(&db, "2026-09-08 15:03:00Z").await;
    source(&db, "revision", "a", "我已经开始了但不知道如何继续").await;
    assert!(!tick(&db).await);
    let supplements: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_batch WHERE kind='supplement'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(supplements, 1);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn historical_probe_does_not_qualify_new_semantic_contract() {
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
    assert!(matches!(result, Err(ModelError::NotQualified)));
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn semantic_retry_limit_cannot_be_reset_by_another_batch() {
    let db = fixture::proof_database("semantic_global_attempts").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let source = source(
        &db,
        "global-attempts",
        "a",
        "[BAD_SCHEMA] 持续无法符合结构的结果",
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
    run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
        .await
        .unwrap();
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
async fn packet_recalls_readable_same_domain_definitions_and_records_checked_proposal() {
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
    assert_eq!(
        result["semantic"]["problems"][0]["candidateRef"],
        problem.to_string()
    );
    assert_eq!(
        result["semantic"]["problems"][0]["serverValidatedCandidate"],
        true
    );
    assert_eq!(
        result["candidateSnapshot"][0]["problemRef"],
        problem.to_string()
    );
    assert!(
        result["contextRefs"]["researchSourceRefs"]
            .as_array()
            .unwrap()
            .contains(&json!(first))
    );
    server.kill().await.unwrap();
}
