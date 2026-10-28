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
    for relation in [
        "linggan_comment_study_policy",
        "linggan_topic_map_research_run",
        "linggan_creator_discovery_policy",
    ] {
        assert!(
            !sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL")
                .bind(relation)
                .fetch_one(db.pool())
                .await
                .unwrap()
        );
    }
    retired!(ai::model_invocation::connection_request(
        &db,
        &secrets,
        Uuid::new_v4()
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
