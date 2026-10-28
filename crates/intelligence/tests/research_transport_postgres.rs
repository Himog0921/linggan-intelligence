//! Actual SDK request succeeds, then the child reply is lost. No shared data/provider.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::{
    ai_foundation::{
        self as ai, TaskKey,
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
    dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":"reader","noteId":"dossier-note","text":"我的孩子需要每天有人提醒。","authorId":"independent-reader"}})).await;
    let source: Uuid = sqlx::query_scalar(
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
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    for task in [
        TaskKey::CreatorResearch,
        TaskKey::CreatorAccountResearch,
        TaskKey::CommentStudy,
        TaskKey::CommentProblemResolution,
        TaskKey::CommentProblemPairReview,
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
    let p =
        ai::comment_research::preview(&db, dossier::DOMAIN, work, source, StudySourceRole::Primary)
            .await
            .unwrap();
    let mut comment = ai::comment_research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_ref: work,
        source_ref: source,
        observation_role: StudySourceRole::Primary,
        knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
        expected_input_hash: p["inputHash"].as_str().unwrap().into(),
        task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
        expected_task_revision: p["taskRevision"].as_i64().unwrap(),
    };
    ai::comment_research::start(&db, &comment).await.unwrap();
    let package=dossier::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"unknown-topic-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"unknown-topic-note"},"payload":{"title":"未知回执专用主题材料","bodyText":"结尾不能丢失🙂","authorId":"another-author"}})).await;
    let topic_work:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id='unknown-topic-note'").fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(topic_work).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    let mut topic = ai::topic_research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_refs: vec![topic_work],
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
    single.command_ref = Uuid::new_v4();
    account.command_ref = Uuid::new_v4();
    comment.command_ref = Uuid::new_v4();
    topic.command_ref = Uuid::new_v4();
    for error in [
        ai::creator_research::start(&db, &single).await.unwrap_err(),
        ai::creator_account_research::start(&db, &account)
            .await
            .unwrap_err(),
        ai::comment_research::start(&db, &comment)
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
