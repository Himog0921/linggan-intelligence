//! Synthetic setup shared by isolated writing proofs. Declaration does not imply a real probe.
#![allow(dead_code)] // Integration binaries intentionally use different subsets of this fixture.
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        capability::{Capability, ModelLimits},
        control::BudgetPolicy,
        control_store::{Actor, ChangeControl, ControlScope, change_control},
        provider_store::{
            CapabilitySource, ChangeEgress, MaterialClass, OutputMode, SaveCapability,
            change_egress, save_capability,
        },
        task_config::{ModelSelection, SaveTaskConfig, TaskConfigReceipt, save_task_config},
    },
    content_workbench_ai as ai,
    model_secrets::ModelSecretStore,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
pub struct Configured {
    pub task: TaskConfigReceipt,
    pub connection: Uuid,
    pub version: Uuid,
    pub model: Uuid,
    pub endpoint: Uuid,
    pub capability: Uuid,
    pub limits: ModelLimits,
}
pub async fn declare(db: &Database, url: &str) -> Configured {
    declare_with_timeout(db, url, 30).await
}
pub async fn declare_with_timeout(db: &Database, url: &str, timeout_seconds: u64) -> Configured {
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Synthetic writing only','openai-completions',$3,true,$4)")
        .bind(version).bind(connection).bind(url).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,'synthetic-writing','manual')")
        .bind(model).bind(version).execute(db.pool()).await.unwrap();
    let endpoint = sqlx::query_scalar(
        "SELECT endpoint_ref FROM linggan_ai_connection_endpoint WHERE version_ref=$1",
    )
    .bind(version)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let identity=sqlx::query_scalar("SELECT identity_ref FROM linggan_ai_model_identity WHERE connection_ref=$1 AND model_id='synthetic-writing'").bind(connection).fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_price(price_ref,identity_ref,endpoint_ref,source_kind,source_version,input_micro_usd_per_million,output_micro_usd_per_million,cached_read_micro_usd_per_million,cached_write_micro_usd_per_million) VALUES($1,$2,$3,'manual','synthetic-writing-price',1000000,1000000,1000000,1000000)")
        .bind(Uuid::new_v4()).bind(identity).bind(endpoint).execute(db.pool()).await.unwrap();
    let cap = save_capability(
        db,
        &SaveCapability {
            command_ref: Uuid::new_v4(),
            identity_ref: identity,
            endpoint_ref: endpoint,
            expected_capability_ref: None,
            source: CapabilitySource::Manual,
            source_version: "synthetic-writing-declaration".into(),
            probe_invocation_ref: None,
            limits: Capability {
                context_tokens: 32768,
                max_output_tokens: 8192,
                max_timeout_seconds: 60,
                thinking_levels: ["off".into()].into(),
            },
            compat: json!({"maxTokensField":"max_tokens"}),
            thinking_level_map: json!({}),
            output_mode: OutputMode::PromptOnly,
            strict_schema: false,
        },
    )
    .await
    .unwrap();
    let limits = ModelLimits {
        input_tokens: 8192,
        output_tokens: 1000,
        timeout_seconds,
        attempts: 1,
        thinking_level: "off".into(),
    };
    let current_writing_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='writing_legacy'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let task = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_revision: current_writing_revision,
            model: Some(ModelSelection {
                model_ref: model,
                capability_ref: cap.revision_ref,
                limits: limits.clone(),
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    Configured {
        task,
        connection,
        version,
        model,
        endpoint,
        capability: cap.revision_ref,
        limits,
    }
}
pub async fn enable(db: &Database, scope: ControlScope) {
    let revision = sqlx::query_scalar("SELECT revision FROM linggan_ai_control WHERE scope_key=$1")
        .bind(scope.key())
        .fetch_one(db.pool())
        .await
        .unwrap();
    change_control(
        db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope,
            expected_revision: revision,
            enabled: true,
            reason: "Synthetic isolated proof intent".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
}
pub async fn allow(db: &Database, endpoint: Uuid, task: TaskKey, material: MaterialClass) {
    change_egress(
        db,
        &ChangeEgress {
            command_ref: Uuid::new_v4(),
            endpoint_ref: endpoint,
            task_key: task,
            material_class: material,
            expected_event_ref: None,
            allowed: true,
            reason: "Synthetic owned loopback proof only".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
}
pub async fn configure_fake_prerequisites(db: &Database) -> Uuid {
    configure_fake_prerequisites_with_timeout(db, 30).await
}
pub async fn configure_fake_prerequisites_with_timeout(
    db: &Database,
    timeout_seconds: u64,
) -> Uuid {
    let config = declare_with_timeout(db, "http://127.0.0.1:1", timeout_seconds).await;
    // Old consumer contract tests use a local fake child. This is explicitly constructed
    // prerequisite metadata, not proof of a provider or the actual new probe path.
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,finished_at)
        VALUES($1,$2,$3,'probe',repeat('0',64),'succeeded',0,0,0,0,'{\"modelCallable\":true,\"syntheticPrerequisiteFixture\":true}',scope_001_now())")
        .bind(Uuid::new_v4()).bind(config.version).bind(config.model).execute(db.pool()).await.unwrap();
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::WritingLegacy),
    ] {
        enable(db, scope).await;
    }
    allow(
        db,
        config.endpoint,
        TaskKey::WritingLegacy,
        MaterialClass::SelectedWritingMaterial,
    )
    .await;
    config.task.model_config_ref.unwrap()
}
pub async fn run_request(
    db: &Database,
    id: Uuid,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ai::AiError> {
    use linggan_intelligence::ai_foundation::{
        batch_store::InputRef,
        executor::{ExecutorError, ExecutorSession, queue::claim_next, writing::execute_writing},
    };
    let mut owner = None;
    for _ in 0..100 {
        match ExecutorSession::acquire(db, "synthetic-old-consumer-contract", 4).await {
            Ok(session) => {
                owner = Some(session);
                break;
            }
            Err(ExecutorError::Busy) => {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await
            }
            Err(_) => return Err(ai::AiError::Model),
        }
    }
    let mut owner = owner.ok_or(ai::AiError::Model)?;
    if let Some(item) = claim_next(db, owner.fence())
        .await
        .map_err(|_| ai::AiError::Model)?
    {
        if !matches!(item.input,InputRef::WritingRequest {request_ref} if request_ref==id) {
            return Err(ai::AiError::Conflict);
        }
        let execution = execute_writing(db, &item, secrets, adapter);
        tokio::pin!(execution);
        loop {
            tokio::select! {
                result = &mut execution => { result.map_err(|e| match e { ExecutorError::Writing(e) => e, _ => ai::AiError::Model })?; break; },
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => { owner.heartbeat().await.map_err(|_| ai::AiError::Model)?; },
            }
        }
    }
    Ok(())
}

/// One real SDK probe against a caller-owned loopback endpoint. No fabricated callability row.
pub async fn prepare_with_actual_probe(
    db: &Database,
    url: &str,
    owner: &linggan_intelligence::ai_foundation::executor::ExecutorSession,
    adapter: &PiAdapter,
) -> Configured {
    use linggan_intelligence::ai_foundation::{
        batch_store::{self, BatchInput, BatchKind, CreateBatch, InputRef, ProbeOperation},
        executor::{probe::execute_probe, queue::claim_next},
    };
    let model = declare(db, url).await;
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::WritingLegacy),
        ControlScope::Task(TaskKey::ModelProbe),
    ] {
        enable(db, scope).await;
    }
    allow(
        db,
        model.endpoint,
        TaskKey::WritingLegacy,
        MaterialClass::SelectedWritingMaterial,
    )
    .await;
    allow(
        db,
        model.endpoint,
        TaskKey::ModelProbe,
        MaterialClass::SyntheticProbe,
    )
    .await;
    let current_probe_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='model_probe'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_revision: current_probe_revision,
            model: None,
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    batch_store::create_batch(
        db,
        &CreateBatch {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_task_revision: config.revision,
            task_config_ref: config.task_config_ref,
            kind: BatchKind::Probe,
            items: vec![BatchInput {
                item_key: "synthetic-current-key".into(),
                input: InputRef::Probe {
                    sample_ref: Uuid::new_v4(),
                    connection_version_ref: model.version,
                    model_ref: Some(model.model),
                    operation: ProbeOperation::Probe,
                    profile: None,
                },
            }],
        },
    )
    .await
    .unwrap();
    let item = claim_next(db, owner.fence()).await.unwrap().unwrap();
    execute_probe(
        db,
        &item,
        &linggan_intelligence::model_secrets::SyntheticModelSecrets,
        adapter,
    )
    .await
    .unwrap();
    model
}

/// Enable the title task only in an owned synthetic proof database.
pub async fn configure_fake_title(db: &Database) -> Uuid {
    let config = declare(db, "http://127.0.0.1:1").await;
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,finished_at) VALUES($1,$2,$3,'probe',repeat('0',64),'succeeded',0,0,0,0,'{\"modelCallable\":true,\"syntheticPrerequisiteFixture\":true}',scope_001_now())")
        .bind(Uuid::new_v4()).bind(config.version).bind(config.model).execute(db.pool()).await.unwrap();
    let skill: Uuid = sqlx::query_scalar(
        "SELECT current_version_ref FROM linggan_skill_binding WHERE task_key='creation_title'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='creation_title'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let task = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreationTitle,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: config.model,
                capability_ref: config.capability,
                limits: config.limits,
            }),
            skill_version_ref: Some(skill),
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    enable(db, ControlScope::Task(TaskKey::CreationTitle)).await;
    allow(
        db,
        config.endpoint,
        TaskKey::CreationTitle,
        MaterialClass::SelectedWritingMaterial,
    )
    .await;
    task.model_config_ref.unwrap()
}
