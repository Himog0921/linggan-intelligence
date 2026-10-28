//! Isolated comment executor proof; only synthetic data and an owned loopback endpoint.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use dossier::fixture;
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::{
    ai_foundation::{
        TaskKey, comment_research as research,
        control::BudgetPolicy,
        control_store::ControlScope,
        prompt::{self, ActivatePrompt, SavePrompt},
        provider_store::MaterialClass,
        skill_binding::{SwitchSkill, switch_skill},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    comment_study_source::StudySourceRole,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated PostgreSQL and owned synthetic loopback provider"]
async fn comment_uses_target_dossier_and_both_permissions() {
    let db = fixture::proof_database("comment_research_execution").await;
    let work = dossier::work(&db).await;
    dossier::current_image_reading(&db).await;
    comments::comment_with_author(
        &db,
        "dossier-note",
        "parent-a",
        "家长原话：如何安排练习？",
        Some("parent"),
        "2026-10-14T00:00:00Z",
    )
    .await;
    let target = comments::reply_with_author(
        &db,
        "dossier-note",
        "child-a",
        "parent-a",
        "我的孩子需要每天有人提醒。",
        Some("reader"),
        "2026-10-14T00:01:00Z",
    )
    .await;
    let endpoint = wire::Endpoint::start(false).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "synthetic-comment-probe",
        4,
    )
    .await
    .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='comment_study'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CommentStudy,
            expected_revision: revision,
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
    let skill=import_skill(&db,&adapter,&ImportSkill {command_ref:Uuid::new_v4(),expected_version_ref:None,
        package:SkillPackage {directory:"comment-method".into(),files:vec![UploadFile {path:"SKILL.md".into(),encoding:Encoding::Utf8,
        content:"---\nname: comment-method\ndescription: Synthetic comment method\n---\nCOMMENT SKILL V1\n".into()}]}}).await.unwrap();
    let active = switch_skill(
        &db,
        &SwitchSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CommentStudy,
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
            task_key: TaskKey::CommentStudy,
            expected_task_revision: active.task_revision,
            system_text: "COMMENT SYSTEM V1".into(),
            instruction_text: "COMMENT INSTRUCTION V1".into(),
        },
    )
    .await
    .unwrap();
    let enabled = prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CommentStudy,
            expected_task_revision: active.task_revision,
            version_ref: edited.version_ref,
        },
    )
    .await
    .unwrap();
    for control in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::CommentStudy),
    ] {
        setup::enable(&db, control).await;
    }
    for task in [
        TaskKey::CommentProblemResolution,
        TaskKey::CommentProblemPairReview,
    ] {
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_task_setting WHERE task_key=$1")
                .bind(task.as_str())
                .fetch_one(db.pool())
                .await
                .unwrap();
        save_task_config(
            &db,
            &SaveTaskConfig {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_revision: revision,
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
        setup::enable(&db, ControlScope::Task(task)).await;
        setup::allow(&db, configured.endpoint, task, MaterialClass::NoteMaterial).await;
        setup::allow(
            &db,
            configured.endpoint,
            task,
            MaterialClass::CommentMaterial,
        )
        .await;
    }
    let preview = research::preview(&db, dossier::DOMAIN, work, target, StudySourceRole::Primary)
        .await
        .unwrap();
    let start = research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_ref: work,
        source_ref: target,
        observation_role: StudySourceRole::Primary,
        knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
        expected_input_hash: preview["inputHash"].as_str().unwrap().into(),
        task_config_ref: enabled.task_config_ref,
        expected_task_revision: enabled.task_revision,
    };
    assert!(research::start(&db, &start).await.is_err());
    setup::allow(
        &db,
        configured.endpoint,
        TaskKey::CommentStudy,
        MaterialClass::NoteMaterial,
    )
    .await;
    assert!(
        research::start(&db, &start).await.is_err(),
        "comment permission must be independent"
    );
    assert_eq!(endpoint.requests.lock().unwrap().len(), 1);
    setup::allow(
        &db,
        configured.endpoint,
        TaskKey::CommentStudy,
        MaterialClass::CommentMaterial,
    )
    .await;
    let receipt = research::start(&db, &start).await.unwrap();
    assert_eq!(
        research::start(&db, &start).await.unwrap().request_ref,
        receipt.request_ref
    );
    let later = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CommentStudy,
            expected_task_revision: enabled.task_revision,
            system_text: "COMMENT SYSTEM V2".into(),
            instruction_text: "COMMENT INSTRUCTION V2".into(),
        },
    )
    .await
    .unwrap();
    prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CommentStudy,
            expected_task_revision: enabled.task_revision,
            version_ref: later.version_ref,
        },
    )
    .await
    .unwrap();
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-comment-service",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let output = tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let value = research::read(&db, receipt.request_ref).await.unwrap();
            if value["state"] == "needs_support" {
                break value;
            }
            assert!(value["state"] != "failed" && value["state"] != "skipped", "{value}");
            if running.is_finished() { panic!("executor stopped unexpectedly; state={value}"); }
            let wait: Option<String> = sqlx::query_scalar("SELECT c.wait_reason FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE i.batch_ref=$1").bind(receipt.batch_ref).fetch_one(db.pool()).await.unwrap();
            assert!(wait.is_none(), "executor deferred: {wait:?}");
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .ok();
    if output.is_none() {
        let state = research::read(&db, receipt.request_ref).await.unwrap();
        let diagnostic: (String,String,String,String,String,i32) = sqlx::query_as("SELECT c.state,coalesce(c.wait_reason,''),b.state,t.lifecycle,coalesce(cfg.execution_contract,'NULL') || '/' || coalesce(t.handler_contract,'NULL'),b.last_round_no FROM linggan_ai_batch_item i JOIN linggan_ai_batch_item_control c USING(item_ref) JOIN linggan_ai_batch bb USING(batch_ref) JOIN linggan_ai_batch_control b USING(batch_ref) JOIN linggan_ai_task t ON t.task_key=bb.task_key JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=bb.task_config_ref WHERE i.batch_ref=$1").bind(receipt.batch_ref).fetch_one(db.pool()).await.unwrap();
        panic!(
            "comment did not settle; service finished: {}, state={state}, diagnostic={diagnostic:?}",
            running.is_finished()
        );
    }
    let output = output.unwrap();
    assert_eq!(
        research::latest(&db, dossier::DOMAIN, work, target)
            .await
            .unwrap()["requestRef"],
        receipt.request_ref.to_string()
    );
    assert_eq!(output["result"]["outcome"], "signals");
    assert_eq!(
        output["result"]["signals"][0]["sourceRef"],
        target.to_string()
    );
    assert!(
        !output["result"]
            .to_string()
            .contains("我的孩子需要每天有人提醒")
    );
    assert_eq!(output["state"], "needs_support");
    assert!(
        linggan_intelligence::ai_foundation::comment_problem::read_problems(&db, dossier::DOMAIN)
            .await
            .unwrap()["problems"]
            .as_array()
            .unwrap()
            .is_empty(),
        "one author cannot establish a shared problem"
    );
    let second_source = comments::comment_with_author(
        &db,
        "dossier-note",
        "independent-reader",
        "我的孩子需要每天有人提醒。",
        Some("another-reader"),
        "2026-10-14T00:02:00Z",
    )
    .await;
    let p = research::preview(
        &db,
        dossier::DOMAIN,
        work,
        second_source,
        StudySourceRole::Primary,
    )
    .await
    .unwrap();
    let second = research::start(
        &db,
        &research::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            work_ref: work,
            source_ref: second_source,
            observation_role: StudySourceRole::Primary,
            knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
            expected_input_hash: p["inputHash"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
            expected_task_revision: p["taskRevision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let value = research::read(&db, second.request_ref).await.unwrap();
            if value["state"] == "succeeded" {
                break;
            }
            assert!(
                value["state"] != "failed" && value["state"] != "needs_support",
                "{value}"
            );
            if running.is_finished() {
                panic!("service stopped");
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        research::read(&db, receipt.request_ref).await.unwrap()["supportState"],
        "admitted_by_later_review"
    );
    assert_eq!(
        research::read(&db, receipt.request_ref).await.unwrap()["state"],
        "succeeded"
    );
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 6);
    let system = wire::text(&requests[1], "system");
    assert!(system.contains("COMMENT SYSTEM V1") && system.contains("COMMENT SKILL V1"));
    assert!(!system.contains("COMMENT SYSTEM V2"));
    let packet: serde_json::Value =
        serde_json::from_str(&wire::text(&requests[1], "user")).unwrap();
    assert_eq!(
        packet["workContext"]["dossierContract"],
        "research-dossier.v2"
    );
    assert_eq!(
        packet["target"]["researchText"],
        "我的孩子需要每天有人提醒。"
    );
    assert!(
        packet["parentContext"]["researchText"]
            .as_str()
            .unwrap()
            .contains("如何安排")
    );
    let problems =
        linggan_intelligence::ai_foundation::comment_problem::read_problems(&db, dossier::DOMAIN)
            .await
            .unwrap();
    assert_eq!(problems["problems"].as_array().unwrap().len(), 1);
    assert_eq!(
        problems["problems"][0]["evidence"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_problem_membership")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'child-a','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let restricted = research::read(&db, receipt.request_ref).await.unwrap();
    assert_eq!(restricted["sourceAvailable"], false);
    assert!(restricted["result"].is_null());
    assert!(
        research::latest(&db, dossier::DOMAIN, work, target)
            .await
            .unwrap()["result"]
            .is_null()
    );
    assert!(
        linggan_intelligence::ai_foundation::comment_problem::read_problems(&db, dossier::DOMAIN)
            .await
            .unwrap()["problems"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // A withdrawn seed cannot permanently reserve a normalized identity.
    let old_problem = problems["problems"][0]["problemRef"].clone();
    for (index, author) in ["fresh-reader-c", "fresh-reader-d", "fresh-reader-e"]
        .into_iter()
        .enumerate()
    {
        let source = comments::comment_with_author(
            &db,
            "dossier-note",
            author,
            "我的孩子需要每天有人提醒。",
            Some(author),
            "2026-10-14T00:03:00Z",
        )
        .await;
        let p = research::preview(&db, dossier::DOMAIN, work, source, StudySourceRole::Primary)
            .await
            .unwrap();
        let receipt = research::start(
            &db,
            &research::Start {
                command_ref: Uuid::new_v4(),
                domain_ref: dossier::DOMAIN,
                work_ref: work,
                source_ref: source,
                observation_role: StudySourceRole::Primary,
                knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
                expected_input_hash: p["inputHash"].as_str().unwrap().into(),
                task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
                expected_task_revision: p["taskRevision"].as_i64().unwrap(),
            },
        )
        .await
        .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            loop {
                let r = research::read(&db, receipt.request_ref).await.unwrap();
                if r["state"]
                    == if index < 2 {
                        "needs_support"
                    } else {
                        "succeeded"
                    }
                {
                    break;
                }
                assert_ne!(r["state"], "failed", "{r}");
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        if index == 0 {
            research::cancel(
                &db,
                &research::Cancel {
                    command_ref: Uuid::new_v4(),
                    domain_ref: dossier::DOMAIN,
                    request_ref: receipt.request_ref,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                research::read(&db, receipt.request_ref).await.unwrap()["state"],
                "cancelled"
            );
        }
    }
    let renewed =
        linggan_intelligence::ai_foundation::comment_problem::read_problems(&db, dossier::DOMAIN)
            .await
            .unwrap();
    assert_eq!(renewed["problems"].as_array().unwrap().len(), 1);
    assert_ne!(renewed["problems"][0]["problemRef"], old_problem);
    assert_eq!(
        renewed["problems"][0]["evidence"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_problem_definition")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        2
    );
    // A frozen candidate's supporting voice is withdrawn after proposal, before review.
    let source = comments::comment_with_author(
        &db,
        "dossier-note",
        "candidate-f",
        "我的孩子需要每天有人提醒。",
        Some("fresh-reader-f"),
        "2026-10-14T00:04:00Z",
    )
    .await;
    let p = research::preview(&db, dossier::DOMAIN, work, source, StudySourceRole::Primary)
        .await
        .unwrap();
    let receipt = research::start(
        &db,
        &research::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            work_ref: work,
            source_ref: source,
            observation_role: StudySourceRole::Primary,
            knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
            expected_input_hash: p["inputHash"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
            expected_task_revision: p["taskRevision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();
    use linggan_intelligence::ai_foundation::control_store::{
        Actor, ChangeControl, change_control,
    };
    let scope = ControlScope::Task(TaskKey::CommentProblemPairReview);
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM linggan_ai_control WHERE scope_key=$1")
            .bind(scope.key())
            .fetch_one(db.pool())
            .await
            .unwrap();
    change_control(
        &db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope: scope.clone(),
            expected_revision: revision,
            enabled: false,
            reason: "synthetic pause before candidate review".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let result = research::read(&db, receipt.request_ref).await.unwrap();
            if result["stages"][1]["state"] == "succeeded" {
                break;
            }
            assert_ne!(result["state"], "failed", "{result}");
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    let before = endpoint.requests.lock().unwrap().len();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'fresh-reader-d','withdraw candidate supporting voice before review')").bind(work).execute(db.pool()).await.unwrap();
    setup::enable(&db, scope).await;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let result = research::read(&db, receipt.request_ref).await.unwrap();
            if result["state"] == "failed" {
                assert_eq!(result["stages"][2]["failureCode"], "ai_source_unavailable");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        before,
        "withdrawn problem definition cannot be sent to independent reviewer"
    );
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
}
