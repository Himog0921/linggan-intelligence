//! Built desktop UI → real Axum/PG → shared executor → owned synthetic SDK endpoint.
use super::*;
use linggan_intelligence::ai_foundation::{
    self as ai, TaskKey,
    control::BudgetPolicy,
    control_store::ControlScope,
    provider_store::MaterialClass,
    task_config::{ModelSelection, SaveTaskConfig, save_task_config},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_endpoint.rs"]
mod endpoint;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL, built desktop React and synthetic loopback model only"]
async fn unified_research_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref(),
        Ok("SYNTHETIC-NOT-EVIDENCE")
    );
    let db = dossier::fixture::proof_database("unified_research_browser").await;
    let work = dossier::work(&db).await;
    for (id, author) in [("reader-a", "independent-a"), ("reader-b", "independent-b")] {
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":id,"noteId":"dossier-note","text":"我的孩子需要每天有人提醒。","authorId":author}})).await;
    }
    linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, dossier::DOMAIN, 200)
        .await
        .unwrap();
    let endpoint = endpoint::Endpoint::variant(false, "research_ui").await;
    let node = std::env::var("WEB_APP_PROOF_NODE").unwrap();
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        PathBuf::from(&node),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    );
    let mut owner = ai::executor::ExecutorSession::acquire(&db, "ui-proof-bootstrap", 4)
        .await
        .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    for task in [
        TaskKey::CommentStudy,
        TaskKey::CommentProblemResolution,
        TaskKey::CommentProblemPairReview,
        TaskKey::TopicResearch,
        TaskKey::TopicResearchExtractReview,
        TaskKey::TopicResearchResolve,
        TaskKey::TopicResearchResolveReview,
        TaskKey::CreatorResearch,
        TaskKey::CreatorAccountResearch,
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
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let service = tokio::spawn(ai::executor::service::run(
        db.clone(),
        "ui-proof-executor",
        4,
        Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let application = app_with_database(db.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory = PathBuf::from(std::env::var("WEB_APP_PROOF_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("fixture.json"),
        json!({"origin":origin,"domain":dossier::DOMAIN,"work":work}).to_string(),
    )
    .unwrap();
    let status = tokio::task::spawn_blocking(move || {
        std::process::Command::new(node)
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../web/test/e2e/unified-research-real-chain.mjs"),
            )
            .env("WEB_APP_PROOF_DIR", directory)
            .status()
            .unwrap()
    })
    .await
    .unwrap();
    if !status.success() {
        let reasons:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('task',task_key,'state',state,'failure',failure_code,'rule',result->>'validationReason') FROM linggan_model_invocation WHERE task_key LIKE 'topic_research%'").fetch_all(db.pool()).await.unwrap();
        eprintln!("topic validation diagnostics: {:?}", reasons);
        eprintln!(
            "research diagnostics: {}",
            ai::comment_research::runs(&db, dossier::DOMAIN)
                .await
                .unwrap()
        );
    }
    drain.request();
    let drained = tokio::time::timeout(Duration::from_secs(20), service).await;
    server.abort();
    let _ = server.await;
    endpoint.finish().await;
    assert!(drained.unwrap().unwrap().is_ok());
    assert!(status.success(), "inspect unified research browser report");
}
