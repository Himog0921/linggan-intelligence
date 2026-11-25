//! Actual SDK request succeeds, then the child reply is lost. No shared data/provider.
#[path = "support/topic_core_fixture.rs"]
mod core_fixture;
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "support/content_understanding_fixture.rs"]
mod l1_setup;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::{
    ai_foundation::{
        self as ai, TaskKey, comment_understanding as comments,
        control::BudgetPolicy,
        control_store::{Actor, ChangeControl, ControlScope, change_control},
        provider_store::MaterialClass,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    comment_study_source::StudySourceRole,
};
use serde_json::json;
use uuid::Uuid;
#[tokio::test]
#[ignore = "owned PostgreSQL, local synthetic SDK endpoint and deliberately corrupted child reply"]
async fn lost_research_receipts_hold_quota_and_block_new_commands_even_after_control_withholding() {
    let db = dossier::fixture::proof_database("research_lost_receipts").await;
    let work = dossier::work(&db).await;
    dossier::current_image_reading_with_bytes(&db, Some(core_fixture::bytes())).await;
    dossier::fixture::submit_package(&db, "comments", json!({"contentExternalId":"dossier-note"}),
        json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},
            "payload":{"commentId":"reader","noteId":"dossier-note","text":"我的孩子需要每天有人提醒。","authorId":"independent-reader"}})).await;
    let source = sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='reader'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let endpoint = wire::Endpoint::start(false).await;
    let node = std::path::PathBuf::from(std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap());
    let script = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/pi-adapter/src/adapter.mjs");
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        node.clone(),
        script.clone(),
    );
    let mut owner = ai::executor::ExecutorSession::acquire(&db, "unknown-proof-bootstrap", 4)
        .await
        .unwrap();
    let mut configured =
        setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    let l1_config = l1_setup::configure_high(&db, &configured).await;
    configured.capability = sqlx::query_scalar("SELECT m.capability_ref FROM linggan_ai_task_config t JOIN linggan_model_config m ON m.config_ref=t.model_config_ref WHERE t.task_config_ref=$1").bind(l1_config).fetch_one(db.pool()).await.unwrap();
    for scope in [ControlScope::Global, ControlScope::Executor] {
        setup::enable(&db, scope).await;
    }
    core_fixture::prepare(&db, work, &owner, core_fixture::CoreStorage::Direct).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    for task in [
        TaskKey::CreatorResearch,
        TaskKey::CreatorAccountResearch,
        TaskKey::CommentUnderstanding,
        TaskKey::TopicResearch,
        TaskKey::TopicResearchExtractReview,
        TaskKey::TopicResearchResolve,
        TaskKey::TopicResearchResolveReview,
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
                    limits: ai::capability::ModelLimits {
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
        if task.as_str().starts_with("comment_") || task.as_str().starts_with("topic_research") {
            setup::allow(
                &db,
                configured.endpoint,
                task,
                MaterialClass::CommentMaterial,
            )
            .await;
        }
        if task == TaskKey::CreatorAccountResearch {
            setup::allow(
                &db,
                configured.endpoint,
                task,
                MaterialClass::PublicAuthorBio,
            )
            .await;
        }
    }
    let p = ai::creator_research::preview(&db, dossier::DOMAIN, work)
        .await
        .unwrap();
    let mut single = ai::creator_research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_ref: work,
        knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
        expected_fingerprint: p["inputFingerprint"].as_str().unwrap().into(),
        task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
        expected_task_revision: p["taskRevision"].as_i64().unwrap(),
    };
    let first = ai::creator_research::start(&db, &single).await.unwrap();
    let p = ai::creator_account_research::preview(&db, dossier::DOMAIN, "xhs", "synthetic-author")
        .await
        .unwrap();
    let mut account = ai::creator_account_research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        platform: "xhs".into(),
        author_external_id: "synthetic-author".into(),
        work_refs: vec![work],
        knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
        expected_fingerprint: p["inputFingerprint"].as_str().unwrap().into(),
        task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
        expected_task_revision: p["taskRevision"].as_i64().unwrap(),
    };
    ai::creator_account_research::start(&db, &account)
        .await
        .unwrap();
    let selections = vec![comments::Selection {
        work_ref: work,
        source_ref: source,
        observation_role: StudySourceRole::Primary,
    }];
    let preview = comments::preview(&db, dossier::DOMAIN, &selections, false)
        .await
        .unwrap();
    let mut comment = comments::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        selections,
        knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
        expected_plan_hash: preview["planHash"].as_str().unwrap().into(),
        task_config_ref: serde_json::from_value(preview["taskConfigRef"].clone()).unwrap(),
        expected_task_revision: preview["taskRevision"].as_i64().unwrap(),
        rerun: false,
    };
    let research = comments::start(&db, &comment).await.unwrap();
    let mut topic = ai::topic_research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_refs: vec![work],
    };
    ai::topic_research::start(&db, &topic).await.unwrap();
    // The wrapper performs the fixed SDK request, discards its valid reply, and emits
    // malformed IPC. Delay lets the test revoke controls after all four calls reach HTTP.
    let directory = std::env::temp_dir().join(format!("linggan-lost-reply-{}", Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let wrapper = directory.join("lost-reply.mjs");
    std::fs::write(&wrapper,format!("import{{spawn}}from'node:child_process';const child=spawn(process.execPath,[{}],{{stdio:['pipe','pipe','ignore']}});process.stdin.pipe(child.stdin);child.stdout.resume();child.on('close',()=>setTimeout(()=>process.stdout.write('CORRUPTED-REPLY\\n'),1500));",serde_json::to_string(&script).unwrap())).unwrap();
    let adapter =
        linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(node, wrapper);
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(ai::executor::service::run(
        db.clone(),
        "unknown-proof-executor",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        while endpoint.requests.lock().unwrap().len() < 5 {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM linggan_ai_control WHERE scope_key=$1")
            .bind(ControlScope::Global.key())
            .fetch_one(db.pool())
            .await
            .unwrap();
    change_control(
        &db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope: ControlScope::Global,
            expected_revision: revision,
            enabled: false,
            reason: "synthetic pause after HTTP dispatch".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(20),async{loop{let n:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_ai_batch_item_control WHERE failure_code='ai_dispatch_unknown'").fetch_one(db.pool()).await.unwrap();if n==4{break;}assert!(!running.is_finished());tokio::time::sleep(std::time::Duration::from_millis(25)).await;}}).await.unwrap();
    let held:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation WHERE failure_code='ai_dispatch_unknown' AND charged_tokens=reserved_tokens AND reserved_tokens>0").fetch_one(db.pool()).await.unwrap();
    assert_eq!(held, 4);
    setup::enable(&db, ControlScope::Global).await;
    assert_eq!(
        ai::creator_research::start(&db, &single)
            .await
            .unwrap()
            .request_ref,
        first.request_ref,
        "same command only replays receipt"
    );
    assert_eq!(comments::start(&db, &comment).await.unwrap(), research);
    let retry = comments::preview(&db, dossier::DOMAIN, &comment.selections, false)
        .await
        .unwrap();
    assert_eq!(retry["dispatchUnresolvedCount"], 1);
    assert_eq!(retry["units"], json!([]));
    comment.command_ref = Uuid::new_v4();
    comment.knowledge_cutoff = retry["knowledgeCutoff"].as_str().unwrap().into();
    comment.expected_plan_hash = retry["planHash"].as_str().unwrap().into();
    assert!(matches!(
        comments::start(&db, &comment).await,
        Err(ai::store::FoundationError::AdmissionDenied(ref reasons))
            if reasons == &["no_researchable_comment"]
    ));
    single.command_ref = Uuid::new_v4();
    account.command_ref = Uuid::new_v4();
    topic.command_ref = Uuid::new_v4();
    for error in [
        ai::creator_research::start(&db, &single).await.unwrap_err(),
        ai::creator_account_research::start(&db, &account)
            .await
            .unwrap_err(),
        ai::topic_research::start(&db, &topic).await.unwrap_err(),
    ] {
        assert!(
            matches!(error,ai::store::FoundationError::AdmissionDenied(ref reasons) if reasons.iter().any(|r|r=="ai_prior_dispatch_unresolved")),
            "{error:?}"
        );
    }
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        5,
        "unknown inputs cannot dispatch another HTTP request"
    );
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
    std::fs::remove_dir_all(directory).unwrap();
}
