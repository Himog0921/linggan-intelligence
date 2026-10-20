//! Retired entry points fail before leases, credentials, a child process or a new ledger row.
use super::*;
use linggan_intelligence::{
    model_secrets::ModelSecretStore, model_settings::ModelError, pi_adapter::PiAdapter,
};
use std::sync::atomic::{AtomicUsize, Ordering};
struct NoSecrets(AtomicUsize);
impl ModelSecretStore for NoSecrets {
    fn get(&self, _: Uuid, _: Uuid) -> Result<String, ModelError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ModelError::SecretUnavailable)
    }
    fn put(&self, _: Uuid, _: Uuid, _: &str) -> Result<(), ModelError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ModelError::SecretUnavailable)
    }
    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(ModelError::SecretUnavailable)
    }
}
#[tokio::test]
#[ignore = "owned PostgreSQL and blocked local process proof only"]
async fn ai_foundation_retired_handlers_cannot_read_a_secret_touch_work_or_spawn_any_runtime() {
    use linggan_intelligence as ai;
    let db = fixture::proof_database("ai_legacy_cutoff").await;
    let root = std::path::PathBuf::from(
        std::env::var_os("LINGGAN_CREATION_ASSET_ROOT").expect("owned fixture root required"),
    )
    .join(format!("legacy-cutoff-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let script = root.join("blocked-child.sh");
    std::fs::write(&script, ": > \"${0%/*}/child-started\"\nexit 99\n").unwrap();
    let adapter = PiAdapter::configured_with_test_runtimes(
        "/bin/sh".into(),
        script.clone(),
        "/bin/sh".into(),
        script,
    );
    let secrets = NoSecrets(AtomicUsize::new(0));
    macro_rules! retired {
        ($future:expr) => {
            match $future.await {
                Ok(_) => panic!("retired entry accepted execution"),
                Err(error) => assert_eq!(error.to_string(), "ai_legacy_execution_retired"),
            }
        };
    }
    retired!(ai::model_runner::run_model_work_once(
        &db, &secrets, &adapter
    ));
    retired!(ai::model_runner::prepare_next_batch_across_runs(&db));
    retired!(ai::creator_discovery_worker::run_once(
        &db, &secrets, &adapter
    ));
    retired!(ai::topic_map_research_worker::run_once(
        &db, &secrets, &adapter
    ));
    retired!(ai::comment_study_model_runner::call_study_batch_model(
        &db,
        &secrets,
        &adapter,
        Uuid::new_v4(),
        Uuid::new_v4()
    ));
    retired!(ai::comment_study_pair_worker::run_one_problem_pair(
        &db, &secrets, &adapter
    ));
    retired!(
        ai::comment_study_resolution_worker::run_one_problem_resolution(&db, &secrets, &adapter)
    );
    retired!(ai::comment_study_embedding::embed_pending_signals(
        &db, &adapter
    ));
    retired!(ai::comment_study_embedding::probe_and_register_embedding_profile(&db, &adapter));
    retired!(ai::model_invocation::connection_request(
        &db,
        &secrets,
        Uuid::new_v4()
    ));
    let request = ai::topic_map_research::ResearchCommand::Configure {
        request_ref: Uuid::new_v4(),
        domain_ref: Uuid::new_v4(),
        model_config_ref: Uuid::new_v4(),
        daily_token_limit: 1024,
        run_token_limit: 1024,
        automatic_enabled: true,
        collection_enabled: true,
    };
    retired!(ai::topic_map_research::apply_research_command(
        &db, &request
    ));
    assert_eq!(secrets.0.load(Ordering::SeqCst), 0);
    assert!(!root.join("child-started").exists());
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
    std::fs::remove_dir_all(root).unwrap();
}
