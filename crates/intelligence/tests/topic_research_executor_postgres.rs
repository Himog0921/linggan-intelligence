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
#[path = "support/topic_core_fixture.rs"]
mod core_fixture;
#[path = "support/content_understanding_fixture.rs"]
mod l1_setup;
#[path = "support/research_method_fixture.rs"]
mod method;

#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_publishes_one_analysis_and_assignment_per_window() {
    exercise("normal").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_invalid_response_writes_no_result_or_assignment() {
    exercise("invalid_output").await;
}
#[tokio::test]
#[ignore = "isolated PostgreSQL and synthetic loopback only"]
async fn topic_single_pass_no_signal_publishes_analysis_without_assignment() {
    exercise("no_signal").await;
}
async fn exercise(variant: &'static str) {
    let db = dossier::fixture::proof_database("topic_execution").await;
    let work = dossier::work(&db).await;
    dossier::current_image_reading_with_bytes(&db, Some(core_fixture::bytes())).await;
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
    let mut configured =
        setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    let l1_config = l1_setup::configure_high(&db, &configured).await;
    configured.capability = sqlx::query_scalar("SELECT m.capability_ref FROM linggan_ai_task_config t JOIN linggan_model_config m ON m.config_ref=t.model_config_ref WHERE t.task_config_ref=$1").bind(l1_config).fetch_one(db.pool()).await.unwrap();
    for control in [ControlScope::Global, ControlScope::Executor] {
        setup::enable(&db, control).await;
    }
    core_fixture::prepare(&db, work, &owner).await;
    // The second work proves that an accepted topic is matched, not duplicated.
    let second = if variant == "normal" {
        let package = dossier::fixture::submit_package(&db,"content_detail",serde_json::json!({"contentExternalId":"second-topic-note"}),serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"second-topic-note"},"payload":{"title":"另一合格作品","bodyText":"另一作者的正文。结尾不能丢失🙂","authorId":"another-author"}})).await;
        let second:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id='second-topic-note'").fetch_one(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(second).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        core_fixture::image(&db, "second-topic-note").await;
        core_fixture::prepare(&db, second, &owner).await;
        Some(second)
    } else {
        None
    };
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    for task in research::CURRENT_TASKS {
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
    for task in research::CURRENT_TASKS {
        versions.push(method::assign(&db, &adapter, task, "V1", None).await);
    }
    let request = research::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_refs: vec![work],
    };
    let receipt = research::start(&db, &request).await.unwrap();
    assert_eq!(receipt.stage_count, 1);
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
    for (i, task) in research::CURRENT_TASKS.into_iter().enumerate() {
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
    let run = wait_for(&db, receipt.run_ref, &running).await;
    assert_eq!(run["executionMode"], "single_pass");
    assert_eq!(run["windowCount"], 1);
    assert_eq!(run["succeededStageCount"], if variant == "invalid_output" { 0 } else { 1 });
    assert_eq!(
        run["failedStageCount"],
        if variant == "invalid_output" { 1 } else { 0 }
    );
    assert_eq!(
        run["stages"].as_array().unwrap().iter().map(|s| s["stageNumber"].as_i64().unwrap()).collect::<Vec<_>>(),
        vec![0]
    );
    let results: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_research_result")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let assignments: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_unit_assignment")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(results, if variant == "invalid_output" { 0 } else { 1 });
    assert_eq!(assignments, if variant == "normal" { 1 } else { 0 });
    let query = serde_json::from_value(serde_json::json!({"domainRef":dossier::DOMAIN})).unwrap();
    let graph = linggan_intelligence::topic_map::read_topic_map(&db, &query)
        .await
        .unwrap();
    assert_eq!(!graph.topics.is_empty(), variant == "normal");
    if let Some(second) = second {
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
        let run = wait_for(&db, second_run.run_ref, &running).await;
        assert_eq!(run["succeededStageCount"], 1);
        assert_eq!(run["failedStageCount"], 0);
        let topics: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_topic_workspace w JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=w.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE b.domain_ref=$1",
        )
        .bind(dossier::DOMAIN)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(topics, 1, "a second work must match the accepted topic");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM linggan_topic_map_unit_assignment"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            2
        );
    }
    let requests = endpoint.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), if variant == "normal" { 3 } else { 2 });
    // The first run is pinned to V1 even after the setting advances; the later run starts on V2.
    for request in &requests[1..2] {
        let system = wire::text(request, "system");
        assert!(
            system.contains("METHOD V1") && system.contains("SKILL V1"),
            "{system}"
        );
        assert!(!system.contains("METHOD V2") && !system.contains("SKILL V2"));
    }
    if variant == "normal" {
        let system = wire::text(&requests[2], "system");
        assert!(
            system.contains("METHOD V2") && system.contains("SKILL V2"),
            "{system}"
        );
        assert!(!system.contains("METHOD V1") && !system.contains("SKILL V1"));
    }
    drain.request();
    running.await.unwrap().unwrap();
    endpoint.finish().await;
}

async fn wait_for(
    db: &linggan_storage_postgres::Database,
    run_ref: Uuid,
    running: &tokio::task::JoinHandle<
        Result<linggan_intelligence::ai_foundation::executor::service::DrainReceipt, linggan_intelligence::ai_foundation::executor::ExecutorError>,
    >,
) -> serde_json::Value {
    tokio::time::timeout(std::time::Duration::from_secs(25), async {
        loop {
            let progress = research::read(db, dossier::DOMAIN).await.unwrap();
            let run = progress["runs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|run| run["runRef"] == run_ref.to_string())
                .unwrap()
                .clone();
            if run["pendingStageCount"] == 0 {
                return run;
            }
            assert!(!running.is_finished(), "executor stopped");
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap()
}

#[path = "support/topic_resolution_order.rs"]
mod resolution_order;
