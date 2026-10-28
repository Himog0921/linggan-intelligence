//! Real retained material, disposable restored database, synthetic loopback provider only.
//! This proves flow/provenance/versioning. Outputs are explicitly NOT model-quality evidence.
#[path = "support/research_method_fixture.rs"]
mod method;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::{
    ai_foundation::{
        self as ai, TaskKey,
        control::BudgetPolicy,
        control_store::ControlScope,
        provider_store::MaterialClass,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    comment_study_source::{self, StudySourceRole},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;
const DOMAIN: Uuid = Uuid::from_u128(0x00000000000040008000000000000001);
#[tokio::test]
#[ignore = "only test-unified-research-real-notes.sh may supply an owned restored database"]
async fn real_note_reaches_each_owner_with_exact_dossier_sources() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = Database::connect(&url).await.unwrap();
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(
        database.starts_with("linggan_unified_real_proof_"),
        "refusing a shared or ordinary fixture database"
    );
    let work: Uuid = std::env::var("UNIFIED_RESEARCH_REAL_WORK")
        .unwrap()
        .parse()
        .unwrap();
    let before = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    assert!(!before.fragments.is_empty());
    // Isolate execution from any unrelated batch retained in the restored snapshot.
    sqlx::query("UPDATE linggan_ai_batch_control SET cancel_requested=true,revision=revision+1 WHERE state='open'").execute(db.pool()).await.unwrap();
    let endpoint = wire::Endpoint::variant(false, "real_material").await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = ai::executor::ExecutorSession::acquire(&db, "real-input-synthetic-provider", 4)
        .await
        .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let tasks = [
        TaskKey::CreatorResearch,
        TaskKey::CreatorAccountResearch,
        TaskKey::CommentStudy,
        TaskKey::CommentProblemResolution,
        TaskKey::CommentProblemPairReview,
        TaskKey::TopicResearch,
        TaskKey::TopicResearchExtractReview,
        TaskKey::TopicResearchResolve,
        TaskKey::TopicResearchResolveReview,
    ];
    let mut methods = Vec::new();
    for task in tasks {
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
        methods.push(method::assign(&db, &adapter, task, "V1", None).await);
    }
    let preview = ai::creator_research::preview(&db, DOMAIN, work)
        .await
        .unwrap();
    let creator = ai::creator_research::start(
        &db,
        &ai::creator_research::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: DOMAIN,
            work_ref: work,
            knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
            expected_fingerprint: preview["inputFingerprint"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(preview["taskConfigRef"].clone()).unwrap(),
            expected_task_revision: preview["taskRevision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let sources =
        comment_study_source::eligible_sources_for_works(&db, DOMAIN, &cutoff, &[work], 20)
            .await
            .unwrap();
    let mut authors = std::collections::HashSet::new();
    let mut selected = Vec::new();
    for source in sources {
        let author: String = sqlx::query_scalar(
            "SELECT author_external_id FROM linggan_material_comment WHERE material_ref=$1",
        )
        .bind(source.source_ref)
        .fetch_one(db.pool())
        .await
        .unwrap();
        if authors.insert(author) {
            selected.push(source.source_ref);
        }
        if selected.len() == 2 {
            break;
        }
    }
    assert_eq!(
        selected.len(),
        2,
        "the selected note must have two independent eligible reader comments"
    );
    let mut comments = Vec::new();
    for source in &selected {
        let p = ai::comment_research::preview(&db, DOMAIN, work, *source, StudySourceRole::Primary)
            .await
            .unwrap();
        comments.push(
            ai::comment_research::start(
                &db,
                &ai::comment_research::Start {
                    command_ref: Uuid::new_v4(),
                    domain_ref: DOMAIN,
                    work_ref: work,
                    source_ref: *source,
                    observation_role: StudySourceRole::Primary,
                    knowledge_cutoff: p["knowledgeCutoff"].as_str().unwrap().into(),
                    expected_input_hash: p["inputHash"].as_str().unwrap().into(),
                    task_config_ref: serde_json::from_value(p["taskConfigRef"].clone()).unwrap(),
                    expected_task_revision: p["taskRevision"].as_i64().unwrap(),
                },
            )
            .await
            .unwrap(),
        );
    }
    let topic = ai::topic_research::start(
        &db,
        &ai::topic_research::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: DOMAIN,
            work_refs: vec![work],
        },
    )
    .await
    .unwrap();
    for (i, task) in tasks.into_iter().enumerate() {
        method::assign(&db, &adapter, task, "V2", Some(methods[i])).await;
    }
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(ai::executor::service::run(
        db.clone(),
        "real-input-synthetic-provider",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(100), async {
        loop {
            let c = ai::creator_research::read(&db, creator.request_ref)
                .await
                .unwrap();
            let t = ai::topic_research::read(&db, DOMAIN).await.unwrap();
            let topic = &t["runs"][0];
            assert_eq!(
                topic["failedStageCount"], 0,
                "a real-input topic stage failed: {}",
                topic
            );
            let mut done = c["state"] == "succeeded" && topic["pendingStageCount"] == 0;
            for c in &comments {
                let r = ai::comment_research::read(&db, c.request_ref)
                    .await
                    .unwrap();
                assert_ne!(r["state"], "failed");
                done &= r["state"] == "succeeded";
                assert!(r["sourceAvailable"].as_bool().unwrap());
            }
            if done {
                break;
            }
            assert!(!running.is_finished());
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let data = linggan_evidence::creator_discovery::load_topic_materials_for_works_at_in(
        &mut tx,
        DOMAIN,
        &[work],
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let current = ai::prompt::read(&db, TaskKey::CreatorAccountResearch)
        .await
        .unwrap();
    let configuration:Uuid=sqlx::query_scalar("SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='creator_account_research'").fetch_one(db.pool()).await.unwrap();
    let w = &data.works[0];
    let account = ai::creator_account_research::start(
        &db,
        &ai::creator_account_research::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: DOMAIN,
            work_refs: vec![work],
            platform: w.platform.clone(),
            author_external_id: w.author_external_id.clone().unwrap(),
            knowledge_cutoff: data.as_of.clone(),
            expected_fingerprint: linggan_evidence::creator_discovery::account_research_fingerprint(
                &[w],
            ),
            task_config_ref: configuration,
            expected_task_revision: current["revision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let r = ai::creator_account_research::read(&db, account.request_ref)
                .await
                .unwrap();
            assert_ne!(r["state"], "failed");
            if r["state"] == "succeeded" {
                break;
            }
            assert!(!running.is_finished());
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let after = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.material_revision, after.material_revision);
    assert_eq!(before.revision, after.revision);
    let requests = endpoint.requests.lock().unwrap().clone();
    let mut v1 = 0;
    let mut v2 = 0;
    for r in requests.iter().skip(1) {
        let system = wire::text(r, "system");
        if system.contains("METHOD V1") && system.contains("SKILL V1") {
            v1 += 1;
        } else if system.contains("METHOD V2") && system.contains("SKILL V2") {
            v2 += 1;
        } else {
            panic!("a call did not use its frozen method");
        }
    }
    assert_eq!(
        v2, 1,
        "the later account command must use the switched method"
    );
    assert!(v1 >= 7);
    let traces:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('taskKey',task_key,'promptVersion',result->>'promptVersion','state',state,'invocationRef',invocation_ref,'taskConfigRef',task_config_ref) FROM linggan_model_invocation WHERE task_config_ref IS NOT NULL AND result->>'producer'='ai-gen1-executor.v1' ORDER BY created_at").fetch_all(db.pool()).await.unwrap();
    let report = json!({"proof":"real-material.synthetic-provider.v1","notModelQualityEvidence":true,"workRef":work,"title":before.note["displayTitle"],"dossierRevision":before.revision,"materialRevision":before.material_revision,"coverage":before.coverage,"selectedCommentSourceRefs":selected,"creatorRequestRef":creator.request_ref,"accountRequestRef":account.request_ref,"topicRunRef":topic.run_ref,"topicWindows":topic.window_count,"frozenV1Calls":v1,"subsequentV2Calls":v2,"totalSyntheticCalls":requests.len(),"traces":traces,"rawSourcesUnchanged":true});
    std::fs::write(
        std::env::var("UNIFIED_RESEARCH_REPORT").unwrap(),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
}
