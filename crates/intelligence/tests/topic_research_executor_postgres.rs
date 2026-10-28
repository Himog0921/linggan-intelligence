//! One complete topic workflow, owned PostgreSQL and synthetic loopback calls only.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_intelligence::ai_foundation::{
    control::BudgetPolicy,
    control_store::ControlScope,
    provider_store::MaterialClass,
    task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    topic_research as research,
};
use uuid::Uuid;
#[path = "support/research_method_fixture.rs"]
mod method;

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_four_stages_publish_only_after_both_reviews() {
    exercise("normal").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_failed_extraction_review_cannot_resolve_or_publish() {
    exercise("reject_extraction").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_failed_resolution_review_cannot_write_any_graph_decision() {
    exercise("reject_resolution").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_no_signal_completes_without_dispatching_assignment_stages() {
    exercise("no_signal").await;
}
async fn exercise(variant: &'static str) {
    let db = dossier::fixture::proof_database("topic_execution").await;
    let work = dossier::work(&db).await;
    dossier::current_image_reading(&db).await;
    let endpoint = wire::Endpoint::variant(false, variant).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "synthetic-topic-probe",
        4,
    )
    .await
    .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    for task in research::TASKS {
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
    for control in [ControlScope::Global, ControlScope::Executor] {
        setup::enable(&db, control).await;
    }
    let mut versions = Vec::new();
    for task in research::TASKS {
        versions.push(method::assign(&db, &adapter, task, "V1", None).await);
    }
    let request = research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_refs: vec![work],
    };
    let receipt = research::start(&db, &request).await.unwrap();
    assert_eq!(receipt.stage_count, 4);
    assert_eq!(
        research::start(&db, &request).await.unwrap().run_ref,
        receipt.run_ref
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    for (i, task) in research::TASKS.into_iter().enumerate() {
        method::assign(&db, &adapter, task, "V2", Some(versions[i])).await;
    }
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-topic-service",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(25), async {
        loop {
            let progress = research::read(&db, dossier::DOMAIN).await.unwrap();
            let run = &progress["runs"][0];
            if run["pendingStageCount"] == 0 {
                if variant == "normal" {
                    assert_eq!(run["succeededStageCount"], 4);
                    assert_eq!(run["failedStageCount"], 0);
                } else if variant == "no_signal" {
                    assert_eq!(run["succeededStageCount"], 2);
                    assert_eq!(run["failedStageCount"], 0);
                } else {
                    assert!(run["failedStageCount"].as_i64().unwrap() > 0);
                }

                break;
            }
            assert!(!running.is_finished(), "executor stopped");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(
        requests.len(),
        if variant == "no_signal" || variant == "reject_extraction" {
            3
        } else {
            5
        }
    );
    for request in requests.iter().skip(1) {
        let system = wire::text(request, "system");
        assert!(
            system.contains("METHOD V1") && system.contains("SKILL V1"),
            "{system}"
        );
        assert!(!system.contains("METHOD V2") && !system.contains("SKILL V2"));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        if variant == "normal" || variant == "no_signal" {
            1
        } else {
            0
        }
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_unit_assignment")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        if variant == "normal" { 1 } else { 0 }
    );
    let query = serde_json::from_value(serde_json::json!({"domainRef":dossier::DOMAIN})).unwrap();
    let graph = linggan_intelligence::topic_map::read_topic_map(&db, &query)
        .await
        .unwrap();
    assert_eq!(!graph.topics.is_empty(), variant == "normal");
    if variant == "normal" {
        // Freeze a valid candidate, then withdraw its source between resolution and review.
        let package=dossier::fixture::submit_package(&db,"content_detail",serde_json::json!({"contentExternalId":"second-topic-note"}),serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"second-topic-note"},"payload":{"title":"另一合格作品","bodyText":"另一作者的正文。结尾不能丢失🙂","authorId":"another-author"}})).await;
        let second:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id='second-topic-note'").fetch_one(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(second).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        let second_run = research::start(
            &db,
            &research::Start {
                command_ref: Uuid::new_v4(),
                domain_ref: dossier::DOMAIN,
                work_refs: vec![second],
            },
        )
        .await
        .unwrap();
        use linggan_intelligence::ai_foundation::control_store::{
            Actor, ChangeControl, change_control,
        };
        let task = research::TASKS[3];
        let scope = ControlScope::Task(task);
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
                reason: "synthetic pause before final review".into(),
            },
            Actor::Human,
        )
        .await
        .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                let progress = research::read(&db, dossier::DOMAIN).await.unwrap();
                let run = progress["runs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["runRef"] == second_run.run_ref.to_string())
                    .unwrap();
                if run["stages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|s| s["stageNumber"] == 2 && s["state"] == "succeeded")
                {
                    break;
                }
                assert_eq!(run["failedStageCount"], 0, "{run}");
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        let before = endpoint.requests.lock().unwrap().len();
        assert_eq!(before, 8);
        linggan_evidence::record_slot_disposition(
            &db,
            "xhs:dossier-note:image:1",
            linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
            "synthetic-proof",
            "withdraw candidate source before review",
        )
        .await
        .unwrap();
        setup::enable(&db, scope).await;
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                let progress = research::read(&db, dossier::DOMAIN).await.unwrap();
                let run = progress["runs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["runRef"] == second_run.run_ref.to_string())
                    .unwrap();
                if run["pendingStageCount"] == 0 {
                    assert!(run["failedStageCount"].as_i64().unwrap() > 0);
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
            "withdrawn candidate cannot reach final reviewer"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_unit_assignment")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1,
            "no second assignment may be accepted"
        );
    }
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
}
