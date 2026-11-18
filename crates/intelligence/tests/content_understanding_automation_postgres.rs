//! L1 automatic queueing: opt-in per domain, bounded, one request per work, no model call.
//! Only synthetic material and a caller-owned proof database.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use dossier::fixture;
#[path = "support/content_understanding_fixture.rs"]
mod l1_setup;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/content_understanding_endpoint.rs"]
mod wire;

use linggan_intelligence::{
    ai_foundation::{content_understanding as owner, executor::ExecutorSession},
    pi_adapter::PiAdapter,
};
use std::path::PathBuf;
use uuid::Uuid;

async fn automatic_requests(db: &linggan_storage_postgres::Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_content_understanding_request r JOIN linggan_ai_batch b USING(batch_ref) WHERE b.kind='automatic'")
        .fetch_one(db.pool()).await.unwrap()
}

#[tokio::test]
#[ignore = "owned PostgreSQL + AI_FOUNDATION_PROOF_NODE; synthetic loopback model, no real models"]
async fn automation_queues_each_ready_work_once_and_only_when_enabled() {
    // The fixture reads LOCAL_001_PROOF_DATABASE_URL itself. Never discover or print a DSN.
    let db = fixture::proof_database(&format!("content_l1_auto_{}", Uuid::new_v4().simple())).await;
    let endpoint = wire::Endpoint::start().await;
    let adapter = PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE")
            .expect("supply the Node executable")
            .into(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut session = ExecutorSession::acquire(&db, "content-l1-auto-probe", 4)
        .await
        .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &session, &adapter).await;
    l1_setup::configure_high(&db, &model).await;
    session.request_drain().await.unwrap();
    session.complete_drain().await.unwrap();
    drop(session);
    let work = dossier::work(&db).await;
    let mut cursor = None;

    let preview = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(preview["canStart"], true, "{preview}");

    // Off by default: the scan queues nothing.
    assert_eq!(owner::automation_tick(&db, &mut cursor).await.unwrap(), 0);
    assert_eq!(automatic_requests(&db).await, 0);
    let setting = owner::read_automation(&db, dossier::DOMAIN).await.unwrap();
    assert!(!setting.enabled);
    assert_eq!(setting.configuration_ready, Some(true));

    let enabled = owner::change_automation(
        &db,
        &owner::AutomationChange {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            expected_revision: setting.revision,
            enabled: true,
        },
    )
    .await
    .unwrap();
    assert!(enabled.enabled);
    // A stale revision is refused.
    assert!(
        owner::change_automation(
            &db,
            &owner::AutomationChange {
                command_ref: Uuid::new_v4(),
                domain_ref: dossier::DOMAIN,
                expected_revision: setting.revision,
                enabled: false,
            },
        )
        .await
        .is_err()
    );

    assert_eq!(owner::automation_tick(&db, &mut cursor).await.unwrap(), 1);
    assert_eq!(automatic_requests(&db).await, 1);
    let progress = owner::read_automation(&db, dossier::DOMAIN)
        .await
        .unwrap()
        .progress
        .unwrap();
    assert_eq!(progress["waiting"], 1, "{progress}");

    // A fresh scan (restart) finds the work already attempted and queues nothing more.
    let mut restarted = None;
    assert_eq!(
        owner::automation_tick(&db, &mut restarted).await.unwrap(),
        0
    );
    assert_eq!(automatic_requests(&db).await, 1);

    // A failed request is not retried automatically.
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='failed',failure_code='ai_output_invalid',usable=false,finished_at=now() WHERE state='pending'")
        .execute(db.pool()).await.unwrap();
    let mut again = None;
    assert_eq!(owner::automation_tick(&db, &mut again).await.unwrap(), 0);
    assert_eq!(automatic_requests(&db).await, 1);
    let progress = owner::read_automation(&db, dossier::DOMAIN)
        .await
        .unwrap()
        .progress
        .unwrap();
    assert_eq!(
        progress["failedByCode"]["ai_output_invalid"], 1,
        "{progress}"
    );
}
