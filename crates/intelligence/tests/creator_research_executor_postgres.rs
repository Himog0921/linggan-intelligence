//! Isolated creator-work executor proof; no shared database or external provider.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::ai_foundation::{
    TaskKey,
    control::BudgetPolicy,
    control_store::ControlScope,
    creator_research as research,
    prompt::{self, ActivatePrompt, SavePrompt},
    provider_store::MaterialClass,
    skill_binding::{SwitchSkill, switch_skill},
    skill_package::{Encoding, SkillPackage, UploadFile},
    skill_store::{ImportSkill, import_skill},
    task_config::{ModelSelection, SaveTaskConfig, save_task_config},
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated PostgreSQL and owned synthetic loopback provider"]
async fn creator_work_uses_frozen_method_and_canonical_dossier() {
    let db = dossier::fixture::proof_database("creator_research_execution").await;
    let work = dossier::work(&db).await;
    dossier::current_image_reading(&db).await;
    let endpoint = wire::Endpoint::start(false).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "synthetic-creator-probe",
        4,
    )
    .await
    .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='creator_research'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorResearch,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: configured.model,
                capability_ref: configured.capability,
                limits: configured.limits.clone(),
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let skill=import_skill(&db,&adapter,&ImportSkill {command_ref:Uuid::new_v4(),expected_version_ref:None,
        package:SkillPackage {directory:"creator-method".into(),files:vec![UploadFile {path:"SKILL.md".into(),encoding:Encoding::Utf8,
        content:"---\nname: creator-method\ndescription: Synthetic creator method\n---\nCREATOR SKILL V1\n".into()}]}}).await.unwrap();
    let active = switch_skill(
        &db,
        &SwitchSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorResearch,
            expected_task_revision: config.revision,
            expected_binding_revision: 0,
            version_ref: skill.version_ref,
        },
    )
    .await
    .unwrap();
    let edited = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorResearch,
            expected_task_revision: active.task_revision,
            system_text: "CREATOR SYSTEM V1".into(),
            instruction_text: "CREATOR INSTRUCTION V1".into(),
        },
    )
    .await
    .unwrap();
    let enabled = prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorResearch,
            expected_task_revision: active.task_revision,
            version_ref: edited.version_ref,
        },
    )
    .await
    .unwrap();
    for control in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::CreatorResearch),
    ] {
        setup::enable(&db, control).await;
    }
    let preview = research::preview(&db, dossier::DOMAIN, work).await.unwrap();
    let start = research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_ref: work,
        knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
        expected_fingerprint: preview["inputFingerprint"].as_str().unwrap().into(),
        task_config_ref: enabled.task_config_ref,
        expected_task_revision: enabled.task_revision,
    };
    assert!(matches!(
        research::start(&db, &start).await,
        Err(linggan_intelligence::ai_foundation::store::FoundationError::AdmissionDenied(_))
    ));
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        1,
        "no creator material is sent without permission"
    );
    setup::allow(
        &db,
        configured.endpoint,
        TaskKey::CreatorResearch,
        MaterialClass::NoteMaterial,
    )
    .await;
    let receipt = research::start(&db, &start).await.unwrap();
    assert_eq!(
        research::start(&db, &start).await.unwrap().request_ref,
        receipt.request_ref
    );
    dossier::model_config(&db).await;
    let after_legacy_config = research::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(
        after_legacy_config["inputFingerprint"], preview["inputFingerprint"],
        "the retired creator model setting cannot change the new input identity"
    );
    let edited2 = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorResearch,
            expected_task_revision: enabled.task_revision,
            system_text: "CREATOR SYSTEM V2".into(),
            instruction_text: "CREATOR INSTRUCTION V2".into(),
        },
    )
    .await
    .unwrap();
    prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorResearch,
            expected_task_revision: enabled.task_revision,
            version_ref: edited2.version_ref,
        },
    )
    .await
    .unwrap();
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-creator-service",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let output = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let value = research::read(&db, receipt.request_ref).await.unwrap();
            if value["state"] == "succeeded" {
                break value;
            }
            assert_ne!(value["state"], "failed", "{value}");
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("creator work must settle");
    assert_eq!(output["result"]["relevance"]["value"], "related");
    let current = linggan_evidence::creator_discovery::load_topic_materials_for_works(
        &db,
        dossier::DOMAIN,
        &[work],
    )
    .await
    .unwrap();
    assert_eq!(
        current.works[0].analysis["automatic"]["relevance"]["value"],
        "related"
    );
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    let system = wire::text(&requests[1], "system");
    let user = wire::text(&requests[1], "user");
    assert!(system.contains("CREATOR SYSTEM V1") && system.contains("CREATOR SKILL V1"));
    assert!(!system.contains("CREATOR SYSTEM V2"));
    let packet: serde_json::Value = serde_json::from_str(&user).unwrap();
    assert_eq!(packet["instruction"], "CREATOR INSTRUCTION V1");
    assert_eq!(packet["input"]["materialContract"], "research-dossier.v2");
    assert!(packet["input"]["coverage"].is_object());
    let stamp: (Uuid,Uuid)=sqlx::query_as("SELECT executed_config_ref,executed_skill_version_ref FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE i.batch_ref=$1").bind(receipt.batch_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(stamp, (enabled.task_config_ref, skill.version_ref));
    assert_ne!(output["result"], json!(null));
    // Account synthesis is a distinct task and requires its own two material grants.
    use linggan_intelligence::ai_foundation::creator_account_research as account;
    let author = current.works[0].author_external_id.clone().unwrap();
    let account_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='creator_account_research'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let account_config = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorAccountResearch,
            expected_revision: account_revision,
            model: Some(ModelSelection {
                model_ref: configured.model,
                capability_ref: configured.capability,
                limits: linggan_intelligence::ai_foundation::capability::ModelLimits {
                    input_tokens: 24576,
                    ..configured.limits.clone()
                },
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let account_active = switch_skill(
        &db,
        &SwitchSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorAccountResearch,
            expected_task_revision: account_config.revision,
            expected_binding_revision: 0,
            version_ref: skill.version_ref,
        },
    )
    .await
    .unwrap();
    let account_prompt = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorAccountResearch,
            expected_task_revision: account_active.task_revision,
            system_text: "ACCOUNT SYSTEM V1".into(),
            instruction_text: "ACCOUNT INSTRUCTION V1".into(),
        },
    )
    .await
    .unwrap();
    let account_enabled = prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorAccountResearch,
            expected_task_revision: account_active.task_revision,
            version_ref: account_prompt.version_ref,
        },
    )
    .await
    .unwrap();
    setup::enable(&db, ControlScope::Task(TaskKey::CreatorAccountResearch)).await;
    let preview = account::preview(&db, dossier::DOMAIN, "xhs", &author)
        .await
        .unwrap();
    let start_account = account::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_refs: vec![work],
        platform: "xhs".into(),
        author_external_id: author,
        knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
        expected_fingerprint: preview["inputFingerprint"].as_str().unwrap().into(),
        task_config_ref: account_enabled.task_config_ref,
        expected_task_revision: account_enabled.task_revision,
    };
    setup::allow(
        &db,
        configured.endpoint,
        TaskKey::CreatorAccountResearch,
        MaterialClass::NoteMaterial,
    )
    .await;
    assert!(
        account::start(&db, &start_account).await.is_err(),
        "biography permission is independent"
    );
    setup::allow(
        &db,
        configured.endpoint,
        TaskKey::CreatorAccountResearch,
        MaterialClass::PublicAuthorBio,
    )
    .await;
    let account_receipt = account::start(&db, &start_account).await.unwrap();
    assert_eq!(
        account::start(&db, &start_account)
            .await
            .unwrap()
            .request_ref,
        account_receipt.request_ref
    );
    let new_prompt = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorAccountResearch,
            expected_task_revision: account_enabled.task_revision,
            system_text: "ACCOUNT SYSTEM V2".into(),
            instruction_text: "ACCOUNT INSTRUCTION V2".into(),
        },
    )
    .await
    .unwrap();
    prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreatorAccountResearch,
            expected_task_revision: account_enabled.task_revision,
            version_ref: new_prompt.version_ref,
        },
    )
    .await
    .unwrap();
    let account_output = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let value = account::read(&db, account_receipt.request_ref)
                .await
                .unwrap();
            if value["state"] == "succeeded" {
                break value;
            }
            assert!(
                !["failed", "skipped"].contains(&value["state"].as_str().unwrap()),
                "{value}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(account_output["result"]["focus"]["value"], "unknown");
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert!(wire::text(&requests[2], "system").contains("ACCOUNT SYSTEM V1"));
    assert!(wire::text(&requests[2], "system").contains("CREATOR SKILL V1"));
    let account_packet: serde_json::Value =
        serde_json::from_str(&wire::text(&requests[2], "user")).unwrap();
    assert!(account_packet["input"]["works"][0]["coverage"].is_object());
    let account_projection = linggan_evidence::creator_discovery::load_topic_materials_for_works(
        &db,
        dossier::DOMAIN,
        &[work],
    )
    .await
    .unwrap();
    assert_eq!(
        account_projection.works[0].author_analysis["state"],
        "succeeded"
    );
    linggan_evidence::record_slot_disposition(
        &db,
        "xhs:dossier-note:image:1",
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "synthetic source restriction",
    )
    .await
    .unwrap();
    let restricted = research::read(&db, receipt.request_ref).await.unwrap();
    assert_eq!(restricted["sourceAvailable"], false);
    assert!(restricted["result"].is_null());
    assert!(
        account::read(&db, account_receipt.request_ref)
            .await
            .unwrap()["result"]
            .is_null()
    );
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
}
