//! Shared synthetic queue setup; no constructed callable/invocation receipt is used here.
use super::setup;
use linggan_intelligence::ai_foundation::{
    TaskKey,
    batch_store::{
        self, BatchInput, BatchKind, CreateBatch, InputRef, ProbeOperation, ProbeProfile,
    },
    control::BudgetPolicy,
    control_store::ControlScope,
    provider_store::MaterialClass,
    task_config::{SaveTaskConfig, save_task_config},
};
use linggan_storage_postgres::Database;
use uuid::Uuid;
pub async fn queued(db: &Database, url: &str, count: usize) -> Uuid {
    let configured = setup::declare(db, url).await;
    let config = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_revision: 0,
            model: None,
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::ModelProbe),
    ] {
        setup::enable(db, scope).await;
    }
    setup::allow(
        db,
        configured.endpoint,
        TaskKey::ModelProbe,
        MaterialClass::SyntheticProbe,
    )
    .await;
    batch_store::create_batch(
        db,
        &CreateBatch {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_task_revision: config.revision,
            task_config_ref: config.task_config_ref,
            kind: BatchKind::Probe,
            items: (0..count)
                .map(|n| BatchInput {
                    item_key: n.to_string(),
                    input: InputRef::Probe {
                        sample_ref: Uuid::new_v4(),
                        connection_version_ref: configured.version,
                        model_ref: Some(configured.model),
                        operation: ProbeOperation::Probe,
                        profile: Some(ProbeProfile {
                            capability_ref: configured.capability,
                            limits: configured.limits.clone(),
                        }),
                    },
                })
                .collect(),
        },
    )
    .await
    .unwrap()
    .batch_ref
}
