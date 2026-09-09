//! A04: the next paid packet contains only the missing members of a partial response.
use super::*;

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic Pi"]
async fn partial_output_automatically_recovers_only_missing_comments() {
    let db = fixture::proof_database("partial_auto_recovery").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let mut refs = Vec::new();
    for i in 0..7 {
        refs.push(
            source(
                &db,
                "partial-seven",
                &format!("c{i}"),
                &format!("[MISSING_LAST_TWO_ONCE] 合成表达{i}，每天提醒还是很费精力"),
            )
            .await,
        );
    }
    let batch = selected(&db, config, refs, 100_000).await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let accepted: Vec<Uuid> = sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='succeeded'")
        .bind(batch).fetch_all(db.pool()).await.unwrap();
    assert_eq!(accepted.len(), 5);
    let missing: Vec<Uuid> = sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item WHERE batch_ref=$1 AND failure_code='missing_comment'")
        .bind(batch).fetch_all(db.pool()).await.unwrap();
    assert_eq!(missing.len(), 2);
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap(),
        "cooldown must hold"
    );
    clock(&db, "2026-09-08 15:02:00Z").await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &PiAdapter::configured())
            .await
            .unwrap()
    );
    let packets: Vec<Vec<Uuid>> = sqlx::query_scalar("SELECT source_refs FROM linggan_comment_daily_packet WHERE batch_ref=$1 ORDER BY created_at,packet_ref")
        .bind(batch).fetch_all(db.pool()).await.unwrap();
    let debug_rows: Vec<serde_json::Value> = sqlx::query_scalar("SELECT jsonb_build_object('state',state,'failure',failure_code,'attempts',attempts) FROM linggan_comment_daily_item WHERE batch_ref=$1")
        .bind(batch).fetch_all(db.pool()).await.unwrap();
    assert_eq!(packets.len(), 2, "item outcomes: {debug_rows:?}");
    assert_eq!(packets[0].len(), 7);
    assert_eq!(packets[1].len(), 2);
    assert!(
        packets[1]
            .iter()
            .all(|id| missing.contains(id) && !accepted.contains(id))
    );
    let done: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='succeeded'",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(done, 7);
    let attempts: Vec<i32> = sqlx::query_scalar(
        "SELECT attempts FROM linggan_comment_daily_item WHERE batch_ref=$1 AND source_ref=ANY($2)",
    )
    .bind(batch)
    .bind(accepted)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(attempts.iter().all(|n| *n == 1));
    let receipt: serde_json::Value = sqlx::query_scalar("SELECT v.result FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1 ORDER BY p.created_at LIMIT 1")
        .bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(receipt["normalization"], "bare_json");
    assert_eq!(receipt["diagnostic"]["httpStatus"], 200);
    assert_eq!(receipt["diagnostic"]["usageKnown"], true);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and local synthetic delayed Pi"]
async fn drain_finishes_inflight_packet_and_leaves_next_packet_unreserved() {
    use linggan_intelligence::model_worker_drain::ModelWorkerDrain;
    let db = fixture::proof_database("drain_inflight_packet").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let mut refs = Vec::new();
    for n in 0..14 {
        refs.push(
            source(
                &db,
                "drain-fourteen",
                &format!("c{n:02}"),
                &format!("[DELAY_PROVIDER] 合成表达{n}每天开始很困难"),
            )
            .await,
        );
    }
    let batch = selected(&db, config, refs, 100000).await;
    let drain = ModelWorkerDrain::new();
    let adapter = PiAdapter::configured();
    let running = run_daily_once_with_drain(&db, &SyntheticModelSecrets, &adapter, Some(&drain));
    let signal = async {
        for _ in 0..1000 {
            let started:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_request_trace t JOIN linggan_comment_daily_packet p USING(packet_ref) WHERE p.batch_ref=$1)").bind(batch).fetch_one(db.pool()).await.unwrap();
            if started {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                drain.request();
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("synthetic provider call did not start");
    };
    let (result, ()) = tokio::join!(running, signal);
    assert!(result.unwrap());
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='succeeded'",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(completed, 7);
    assert!(
        !run_daily_once_with_drain(&db, &SyntheticModelSecrets, &adapter, Some(&drain))
            .await
            .unwrap()
    );
    let packets: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(packets, 1);
    let receipt:serde_json::Value=sqlx::query_scalar("SELECT v.result FROM linggan_model_invocation v JOIN linggan_comment_daily_packet p USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(receipt["acceptedComments"], 7);
    assert_eq!(receipt["diagnostic"]["terminalReceived"], true);
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_item WHERE batch_ref=$1 AND state='succeeded'",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(completed, 14);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; two bounded synthetic calls with unknown usage"]
async fn unknown_charge_retry_uses_one_extra_attempt_and_keeps_both_reservations() {
    let db = fixture::proof_database("unknown_retry_real_runner").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-no-usage", None).await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: false,
            config_ref: config,
            source_limit: 100,
            token_limit: 100000,
            auto_policy: AutoPolicy {
                unknown_retry_max_attempts: 1,
                unknown_retry_token_limit: 50000,
                ..AutoPolicy::default()
            },
        },
    )
    .await
    .unwrap();
    let comment = source(&db, "unknown-runner", "a", "[MISSING] 合成评论未被模型回传").await;
    let batch = selected(&db, config, vec![comment], 100000).await;
    let adapter = PiAdapter::configured();
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    clock(&db, "2026-09-08 15:02:00Z").await;
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    clock(&db, "2026-09-08 15:04:00Z").await;
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 2);
    let retry:i32=sqlx::query_scalar("SELECT sw.unknown_retry_attempts FROM linggan_comment_daily_item i JOIN linggan_comment_semantic_work sw USING(semantic_ref) WHERE i.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert_eq!(retry, 1);
    let charged:bool=sqlx::query_scalar("SELECT bool_and(v.charged_tokens=v.reserved_tokens AND v.charged_tokens>0 AND v.input_tokens IS NULL) FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1").bind(batch).fetch_one(db.pool()).await.unwrap();
    assert!(charged);
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; automatic intake precedes historical backlog"]
async fn automatic_research_admits_real_history_and_runs_new_intake_first() {
    let db = fixture::proof_database("automatic_real_history").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-01 02:00:00Z").await;
    let historical = source(
        &db,
        "real-old-history",
        "old",
        "我每天开始任务都特别困难，希望得到帮助",
    )
    .await;
    clock(&db, "2026-09-08 02:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 100,
            token_limit: 100000,
            auto_policy: AutoPolicy {
                continuous_new: true,
                historical_enabled: true,
                history_start: Some("2026-08-01T00:00:00Z".into()),
                ..AutoPolicy::default()
            },
        },
    )
    .await
    .unwrap();
    clock(&db, "2026-09-08 14:59:59Z").await;
    let fresh = source(
        &db,
        "real-new-intake",
        "new",
        "我现在很难开始工作，每天都需要很多提醒",
    )
    .await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    let adapter = PiAdapter::configured();
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let first:Vec<Uuid>=sqlx::query_scalar("SELECT source_refs FROM linggan_comment_daily_packet ORDER BY created_at,packet_ref LIMIT 1").fetch_one(db.pool()).await.unwrap();
    assert_eq!(first, vec![fresh]);
    assert!(
        run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        !run_daily_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let accepted:Vec<Uuid>=sqlx::query_scalar("SELECT source_ref FROM linggan_comment_daily_item WHERE state='succeeded' ORDER BY source_ref").fetch_all(db.pool()).await.unwrap();
    assert_eq!(accepted.len(), 2);
    assert!(accepted.contains(&historical) && accepted.contains(&fresh));
    let selected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_daily_batch WHERE kind='selected'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(selected, 0, "no manual batches are necessary");
    let historical_class: String = sqlx::query_scalar(
        "SELECT queue_class FROM linggan_comment_daily_item WHERE source_ref=$1",
    )
    .bind(historical)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(historical_class, "historical");
    server.kill().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; real local extraction and embedding cache"]
async fn accepted_atoms_reuse_vectors_and_replace_current_generation_without_double_counting() {
    use linggan_intelligence::{
        embedding_settings as embedding, model_runner::run_model_work_once,
    };
    let db = fixture::proof_database("atoms_current_generation").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    let (model, adapter) = configure_atom_vector_reuse(&db, config).await;
    let a = source(&db, "atom-work", "a", "开始很困难，需要更多帮助").await;
    let b = source(&db, "atom-work", "b", "每天都很难开始，需要提醒").await;
    selected(&db, config, vec![a, b], 100000).await;
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM linggan_ci_semantic_atom_current),(SELECT count(*) FROM linggan_ci_atom_vector),(SELECT count(*) FROM linggan_model_invocation WHERE result->>'task'='atom_embedding')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        counts,
        (2, 1, 1),
        "two independent voices share only their equivalent vector"
    );
    create_selected(
        &db,
        &SelectedBatch {
            reanalyze: true,
            batch_ref: Uuid::new_v4(),
            config_ref: config,
            source_refs: vec![a, b],
            token_limit: 100000,
        },
    )
    .await
    .unwrap();
    assert!(
        run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    // Disable embedding before another tick: local projection/reuse must remain possible.
    let settings = embedding::read(&db).await.unwrap();
    embedding::save(
        &db,
        &embedding::SaveEmbedding {
            expected_revision: settings["revision"].as_i64().unwrap(),
            model_ref: model,
            enabled: false,
        },
    )
    .await
    .unwrap();
    linggan_intelligence::comment_intelligence_problems::reconcile_problem_index(&db, 100)
        .await
        .unwrap();
    run_model_work_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM linggan_ci_semantic_atom_current),(SELECT count(*) FROM linggan_ci_semantic_atom),(SELECT count(*) FROM linggan_model_invocation WHERE result->>'task'='atom_embedding')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        counts,
        (2, 4, 1),
        "history retained while only the new generation counts"
    );
    restrict_comment_research_source(&db, a, "synthetic source withdrawal")
        .await
        .unwrap();
    let visible: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_semantic_atom_current")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        visible, 0,
        "shared packet context withdrawal invalidates both current views"
    );
    server.kill().await.unwrap();
}

async fn configure_atom_vector_reuse(
    db: &linggan_storage_postgres::Database,
    config: Uuid,
) -> (Uuid, PiAdapter) {
    use linggan_intelligence::embedding_settings as embedding;
    let model: Uuid =
        sqlx::query_scalar("SELECT model_ref FROM linggan_model_config WHERE config_ref=$1")
            .bind(config)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let saved = embedding::save(
        db,
        &embedding::SaveEmbedding {
            expected_revision: 0,
            model_ref: model,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let embedding_config = Uuid::parse_str(saved["configRef"].as_str().unwrap()).unwrap();
    let adapter = PiAdapter::configured();
    assert_eq!(
        embedding::probe(
            db,
            &SyntheticModelSecrets,
            &adapter,
            &embedding::ProbeEmbedding {
                invocation_ref: Uuid::new_v4(),
                config_ref: embedding_config
            }
        )
        .await
        .unwrap()["embeddingQualified"],
        true
    );
    let settings = embedding::read(&db).await.unwrap();
    embedding::save(
        db,
        &embedding::SaveEmbedding {
            expected_revision: settings["revision"].as_i64().unwrap(),
            model_ref: model,
            enabled: true,
        },
    )
    .await
    .unwrap();
    save_schedule(
        db,
        &DailySchedule {
            expected_revision: 0,
            enabled: false,
            config_ref: config,
            source_limit: 100,
            token_limit: 100000,
            auto_policy: AutoPolicy {
                semantic_token_limit: 40000,
                ..AutoPolicy::default()
            },
        },
    )
    .await
    .unwrap();
    (model, adapter)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; per-day source allowance spans automatic manifests"]
async fn automatic_daily_source_limit_is_shared_with_history_and_resumes_next_day() {
    let db = fixture::proof_database("auto_day_source_budget").await;
    let (mut server, url) = fixture_server().await;
    let (config, _, _) = configured(&db, &url, "synthetic-good", None).await;
    clock(&db, "2026-09-01 02:00:00Z").await;
    let old = source(&db, "daily-cap-history", "a", "这条历史表达需要分析理解").await;
    clock(&db, "2026-09-08 02:00:00Z").await;
    save_schedule(
        &db,
        &DailySchedule {
            expected_revision: 0,
            enabled: true,
            config_ref: config,
            source_limit: 1,
            token_limit: 100000,
            auto_policy: AutoPolicy {
                continuous_new: true,
                historical_enabled: true,
                history_start: Some("2026-08-01T00:00:00Z".into()),
                ..AutoPolicy::default()
            },
        },
    )
    .await
    .unwrap();
    clock(&db, "2026-09-08 14:59:59Z").await;
    let new = source(&db, "daily-cap-new", "a", "这条新的表达需要分析理解").await;
    clock(&db, "2026-09-08 15:00:00Z").await;
    assert!(tick(&db).await);
    assert!(!tick(&db).await);
    let refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT source_ref FROM linggan_comment_daily_item WHERE state='succeeded'",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(refs, vec![new]);
    clock(&db, "2026-09-08 16:00:01Z").await;
    assert!(tick(&db).await);
    let success:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_item WHERE source_ref=$1 AND state='succeeded')").bind(old).fetch_one(db.pool()).await.unwrap();
    assert!(success);
    let unique: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_execution_day_sources()")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(unique, 1);
    server.kill().await.unwrap();
}
