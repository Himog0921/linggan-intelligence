use super::setup;
use linggan_intelligence::ai_foundation::{
    TaskKey,
    capability::{Capability, ModelLimits},
    control::BudgetPolicy,
    control_store::ControlScope,
    provider_store::{
        CapabilitySource, MaterialClass, OutputMode, SaveCapability, save_capability,
    },
    task_config::{ModelSelection, SaveTaskConfig, save_task_config},
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

pub async fn configure_high(db: &Database, model: &setup::Configured) -> Uuid {
    let identity: Uuid = sqlx::query_scalar(
        "SELECT identity_ref FROM linggan_model_capability WHERE capability_ref=$1",
    )
    .bind(model.capability)
    .fetch_one(db.pool())
    .await
    .unwrap();
    // This declares our loopback endpoint, not a real provider's thinking capability.
    let capability = save_capability(
        db,
        &SaveCapability {
            command_ref: Uuid::new_v4(),
            identity_ref: identity,
            endpoint_ref: model.endpoint,
            expected_capability_ref: Some(model.capability),
            source: CapabilitySource::Manual,
            source_version: "synthetic-content-l1-high-only-not-provider-qualification".into(),
            probe_invocation_ref: None,
            limits: Capability {
                context_tokens: 32768,
                max_output_tokens: 8192,
                max_timeout_seconds: 60,
                thinking_levels: ["off".into(), "high".into()].into(),
            },
            compat: json!({"maxTokensField":"max_tokens","supportsReasoningEffort":true}),
            thinking_level_map: json!({}),
            output_mode: OutputMode::JsonObject,
            strict_schema: false,
        },
    )
    .await
    .unwrap();
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='content_understanding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ContentUnderstanding,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: capability.revision_ref,
                limits: ModelLimits {
                    input_tokens: 28672,
                    output_tokens: 4096,
                    timeout_seconds: 30,
                    attempts: 1,
                    thinking_level: "high".into(),
                },
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    assert!(config.applied);
    setup::enable(db, ControlScope::Task(TaskKey::ContentUnderstanding)).await;
    setup::allow(
        db,
        model.endpoint,
        TaskKey::ContentUnderstanding,
        MaterialClass::NoteMaterial,
    )
    .await;
    config.task_config_ref
}
