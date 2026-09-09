//! Drain admission proof against an isolated schema. It never configures or calls a provider.

#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_intelligence::{
    comment_daily::run_daily_once_with_drain,
    comment_intelligence_problems::run_problem_relation_once_with_drain,
    model_secrets::SyntheticModelSecrets, model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;

async fn invocation_count(db: &Database) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; verifies drain before model reservation"]
async fn requested_drain_prevents_task_a_and_task_b_reservations() {
    let db = proof_database("model_worker_drain").await;
    let drain = ModelWorkerDrain::new();
    drain.request();

    assert!(
        !run_daily_once_with_drain(
            &db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            Some(&drain),
        )
        .await
        .unwrap()
    );
    assert!(
        !run_problem_relation_once_with_drain(
            &db,
            &SyntheticModelSecrets,
            &PiAdapter::configured(),
            Some(&drain),
        )
        .await
        .unwrap()
    );
    assert_eq!(invocation_count(&db).await, 0);
}
