//! Handwritten synthetic recovery proofs; no runtime DB, provider or platform.
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    model_settings::reserve_research_model_semantic_dispatch_permit,
    topic_map_research::{ResearchCommand, apply_research_command},
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

async fn count(db: &Database, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap()
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn automatic_backpressure_and_phase_priority_produce_a_completed_topic() {
    let (db, config, adapter) = setup("topic_core_recovery_progress").await;
    work(
        &db,
        "older-windows",
        &"SYNTHETIC 开始练习需要提醒。".repeat(260),
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let older: Uuid = sqlx::query_scalar("SELECT run_ref FROM linggan_topic_map_research_run")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let resolving: Uuid = sqlx::query_scalar(
        "SELECT task_ref FROM linggan_topic_map_research_task WHERE phase='resolve'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(count(&db, "SELECT count(*) FROM linggan_topic_map_research_task WHERE phase='extract' AND state='queued'").await > 0);
    // Fresh zero-call runs previously won indefinitely over this resolve task.
    let fresh = work(
        &db,
        "zero-call-competitor",
        "SYNTHETIC 新材料中的练习启动。",
    )
    .await;
    apply_research_command(&db, &start(vec![fresh]))
        .await
        .unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    assert_eq!(
        count(&db, "SELECT count(*) FROM linggan_topic_map_research_run").await,
        2,
        "no automatic batch is admitted on top of unfinished work"
    );
    let phase: String = sqlx::query_scalar("SELECT phase FROM linggan_topic_map_research_request WHERE task_ref=$1 ORDER BY attempt_ordinal DESC LIMIT 1")
        .bind(resolving).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        phase, "resolve",
        "available distillation must progress before new extraction"
    );
    assert!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_unit_resolution"
        )
        .await
            > 0
    );
    // New qualified material keeps arriving while the existing batch drains.
    for n in 0..3 {
        work(
            &db,
            &format!("arriving-{n}"),
            "SYNTHETIC 开始练习的新材料。",
        )
        .await;
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        assert_eq!(
            count(&db, "SELECT count(*) FROM linggan_topic_map_research_run").await,
            2
        );
    }
    finish_pending(&db, &adapter).await;
    let state: String =
        sqlx::query_scalar("SELECT state FROM linggan_topic_map_research_run WHERE run_ref=$1")
            .bind(older)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, "completed");
    assert!(count(&db, "SELECT count(*) FROM linggan_topic_map_concept_rule").await > 0);
    assert!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_result"
        )
        .await
            > 0
    );
    assert_eq!(count(&db, "SELECT count(*) FROM linggan_topic_map_research_task WHERE state IN ('queued','running')").await, 0);
    assert!(
        count(&db, "SELECT count(*) FROM linggan_topic_map_research_run").await > 2,
        "automatic admission resumes after prior batches drain"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL sessions/advisory locks; synthetic adapter only"]
async fn simultaneous_automatic_ticks_admit_one_batch_and_send_one_snapshot() {
    let (db, config, adapter) = setup("topic_core_recovery_admission").await;
    apply_research_command(&db, &configure(config, 100000, true))
        .await
        .unwrap();
    work(&db, "concurrent-admission", "SYNTHETIC 开始练习。").await;
    let blocker = reserve_research_model_semantic_dispatch_permit(&db, config)
        .await
        .unwrap();
    let launch = || {
        let db = db.clone();
        let adapter = adapter.clone();
        tokio::spawn(async move {
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        })
    };
    let first = launch();
    let second = launch();
    let mut waiting = 0;
    for _ in 0..200 {
        waiting = count(
            &db,
            "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted",
        )
        .await;
        if waiting >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        waiting >= 2,
        "both workers must reach the held model permit"
    );
    assert_eq!(
        count(&db, "SELECT count(*) FROM linggan_topic_map_research_run").await,
        1
    );
    blocker.release().await.unwrap();
    first.await.unwrap();
    second.await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_request"
        )
        .await,
        1
    );
    assert_eq!(count(&db, "SELECT count(*) FROM linggan_topic_map_research_task WHERE phase='resolve' AND state='queued'").await, 1);
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn legacy_cleanup_keeps_failure_unknown_charge_and_stops_only_unsent_work() {
    let (db, config, adapter) = setup("topic_core_recovery_legacy").await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let unknown = work(&db, "old-unknown", "SYNTHETIC SLOW_UNKNOWN 开始练习。").await;
    apply_research_command(&db, &start(vec![unknown]))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let attacked = work(&db, "old-rejected", "SYNTHETIC ATTACK 开始练习。").await;
    apply_research_command(&db, &start(vec![attacked]))
        .await
        .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let reason: String = sqlx::query_scalar(
        "SELECT last_reason FROM linggan_topic_map_research_task WHERE work_public_ref=$1",
    )
    .bind(attacked)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        reason, "invalid_journey_evidence",
        "specific safe business rejection is retained"
    );
    let ledger_reason: String = sqlx::query_scalar("SELECT i.failure_code FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref) JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.work_public_ref=$1")
        .bind(attacked).fetch_one(db.pool()).await.unwrap();
    assert_eq!(ledger_reason, reason);
    let charge = count(&db, "SELECT sum(i.charged_tokens)::bigint FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref)").await;
    let unsent = work(&db, "old-unsent", "SYNTHETIC 尚未发送的练习。").await;
    apply_research_command(&db, &start(vec![unsent]))
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_topic_map_research_run SET method_version='topic-map.research.v2',state='running'")
        .execute(db.pool()).await.unwrap();
    apply_research_command(
        &db,
        &ResearchCommand::Pause {
            request_ref: Uuid::new_v4(),
            domain_ref: D,
            run_ref: None,
        },
    )
    .await
    .unwrap();
    run_once(&db, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_task WHERE state='stopped'"
        )
        .await,
        0,
        "global pause blocks maintenance disposition too"
    );
    apply_research_command(
        &db,
        &ResearchCommand::Resume {
            request_ref: Uuid::new_v4(),
            domain_ref: D,
            run_ref: None,
        },
    )
    .await
    .unwrap();
    assert!(
        run_once(&db, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap()
    );
    let rows = sqlx::query(
        "SELECT work_public_ref,state,last_reason FROM linggan_topic_map_research_task",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    for (work, state, reason) in [
        (unknown, "unknown_dispatch", "unknown_dispatch"),
        (attacked, "failed", "invalid_journey_evidence"),
        (unsent, "stopped", "method_superseded"),
    ] {
        let row = rows
            .iter()
            .find(|r| r.get::<Uuid, _>("work_public_ref") == work)
            .unwrap();
        assert_eq!(row.get::<String, _>("state"), state);
        assert_eq!(row.get::<String, _>("last_reason"), reason);
    }
    assert_eq!(count(&db, "SELECT count(*) FROM linggan_topic_map_research_run WHERE state='stopped' AND last_reason='method_superseded'").await, 3);
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM linggan_topic_map_research_request"
        )
        .await,
        2
    );
    assert_eq!(count(&db, "SELECT sum(i.charged_tokens)::bigint FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref)").await, charge);
}
