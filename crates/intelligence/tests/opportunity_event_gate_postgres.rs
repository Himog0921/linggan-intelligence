//! Isolated Opportunity source-gate activation and executor drain.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use dossier::fixture;

#[tokio::test]
#[ignore = "isolated PostgreSQL and enabled opportunity executor"]
async fn event_gate_activates_with_overlapping_source_writes() {
    let db = fixture::proof_database("opportunity_gate_fairness").await;
    let mut offline_writer = db.pool().begin().await.unwrap();
    sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
        .bind(dossier::DOMAIN.to_string())
        .execute(&mut *offline_writer)
        .await
        .unwrap();
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-opportunity-gate-fairness",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(12), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity \
                 WHERE wait_event_type='Lock' AND query LIKE \
                 '%pg_advisory_lock(linggan_opportunity_card_event_gate_key()%')",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            assert!(!running.is_finished(), "executor must remain running");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("enabled executor must queue for the gate while old writers finish");
    // Keep the old transaction open beyond one server-side wait timeout.
    tokio::time::sleep(std::time::Duration::from_millis(2300)).await;
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
            .bind(dossier::DOMAIN.to_string())
            .execute(db.pool()),
    )
    .await
    .expect("new source writes must not wait behind the queued gate")
    .unwrap();
    let queued: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        queued, 1,
        "overlapping new writes must be queued for catch-up"
    );
    offline_writer.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let mut probe = db.pool().begin().await.unwrap();
            let free: bool = sqlx::query_scalar(
                "SELECT pg_try_advisory_xact_lock_shared(linggan_opportunity_card_event_gate_key())",
            )
            .fetch_one(&mut *probe)
            .await
            .unwrap();
            probe.rollback().await.unwrap();
            if !free {
                break;
            }
            assert!(!running.is_finished(), "executor must remain running");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the queued executor must acquire its gate");
    drain.request();
    tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .expect("the gate holder must not prevent drain")
        .unwrap()
        .unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL and enabled opportunity executor"]
async fn event_gate_wait_does_not_block_executor_drain() {
    assert_gate_wait_drains(false).await;
    assert_gate_wait_drains(true).await;
}

async fn assert_gate_wait_drains(waiting_for_intent: bool) {
    let schema = if waiting_for_intent {
        "opportunity_gate_intent_drain"
    } else {
        "opportunity_gate_drain"
    };
    let db = fixture::proof_database(schema).await;
    let mut offline_writer = db.pool().begin().await.unwrap();
    let pattern = if waiting_for_intent {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(linggan_opportunity_card_event_gate_intent_key())",
        )
        .execute(&mut *offline_writer)
        .await
        .unwrap();
        "%pg_advisory_lock(linggan_opportunity_card_event_gate_intent_key()%"
    } else {
        sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
            .bind(dossier::DOMAIN.to_string())
            .execute(&mut *offline_writer)
            .await
            .unwrap();
        "%pg_advisory_lock(linggan_opportunity_card_event_gate_key()%"
    };
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-opportunity-gate-drain",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(12), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity \
                 WHERE wait_event_type='Lock' AND query LIKE $1)",
            )
            .bind(pattern)
            .fetch_one(db.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            assert!(!running.is_finished(), "executor must remain running");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the enabled executor must be waiting on the gate");
    drain.request();
    tokio::time::timeout(std::time::Duration::from_secs(5), running)
        .await
        .expect("drain must not wait for an offline source transaction")
        .unwrap()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity \
                 WHERE wait_event_type='Lock' AND query LIKE $1)",
            )
            .bind(pattern)
            .fetch_one(db.pool())
            .await
            .unwrap();
            if !waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("drain must close the queued gate connection");
    offline_writer.commit().await.unwrap();
}
