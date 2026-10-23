//! Built UI, real Axum/PG/service/Node/SDK, strictly caller-owned synthetic endpoints and files.
use super::*;
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        control::BudgetPolicy,
        executor::{ExecutorSession, service},
        skill_binding::{ActivateSkill, activate_skill},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    content_workbench_ai as writing,
    model_secrets::SyntheticModelSecrets,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::PiAdapter,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_endpoint.rs"]
mod endpoint;
#[path = "../../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod fixture;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;

async fn browser(node: &str, directory: &Path, phase: &str) -> std::process::ExitStatus {
    let node = node.to_owned();
    let directory = directory.to_owned();
    let phase = phase.to_owned();
    tokio::task::spawn_blocking(move || {
        std::process::Command::new(node)
            .arg(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../web/test/e2e/settings-trials-real-chain.mjs"),
            )
            .env("WEB_APP_PROOF_DIR", directory)
            .arg(phase)
            .status()
            .unwrap()
    })
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL, built React, synthetic loopback SDK and dedicated service only"]
async fn ai_settings_skill_trial_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref(),
        Ok("SYNTHETIC-NOT-EVIDENCE")
    );
    let db = fixture::db(&format!("ai_ui_trial_{}", Uuid::new_v4().simple())).await;
    let endpoint = endpoint::Endpoint::start(false).await;
    let node = std::env::var("WEB_APP_PROOF_NODE").unwrap();
    let adapter = PiAdapter::configured_with_test_command(
        PathBuf::from(&node),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    );
    let owner = ExecutorSession::acquire(&db, "synthetic-ui-bootstrap", 4)
        .await
        .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    let first=import_skill(&db,&adapter,&ImportSkill {command_ref:Uuid::new_v4(),expected_version_ref:None,
        package:SkillPackage {directory:"writing-method".into(),files:vec![UploadFile {path:"SKILL.md".into(),encoding:Encoding::Utf8,content:"---\nname: writing-method\ndescription: Synthetic fixed writing method\n---\nbaseline-v1\n".into()}]}}).await.unwrap();
    let baseline = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_revision: configured.task.revision,
            model: Some(ModelSelection {
                model_ref: configured.model,
                capability_ref: configured.capability,
                limits: configured.limits.clone(),
            }),
            skill_version_ref: Some(first.version_ref),
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let enabled = activate_skill(
        &db,
        &ActivateSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_task_revision: configured.task.revision,
            expected_binding_revision: 0,
            candidate_config_ref: baseline.task_config_ref,
            trial_comparison_ref: None,
            first_version_exception: true,
            reason: "Synthetic UI baseline".into(),
        },
    )
    .await
    .unwrap();
    let mut document = fixture::cited_doc(&db).await;
    let restricted_work = document.excerpts[0].citation.work_ref.unwrap();
    let comment = fixture::comment_doc_for_work(&db, restricted_work).await;
    document.excerpts.extend(comment.excerpts);
    document.canvas.extend(comment.canvas);
    let work = fixture::create(&db, document).await;
    let preview = writing::PreviewRequest {
        selection: None,
        preview_ref: Uuid::new_v4(),
        work_ref: work.work_id,
        expected_revision: work.revision.id,
        action: writing::WritingAction::Draft,
        config_ref: configured.task.model_config_ref.unwrap(),
        citations: work
            .revision
            .document
            .excerpts
            .iter()
            .map(|e| e.citation.clone())
            .collect(),
        paragraph_ids: vec![],
        instruction: String::new(),
    };
    writing::preview(&db, &preview).await.unwrap();
    drop(owner);
    let drain = ModelWorkerDrain::new();
    let service = tokio::spawn(service::run(
        db.clone(),
        "synthetic-ui-trial-service",
        4,
        Arc::new(SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let application = app_with_database(db.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory = PathBuf::from(std::env::var("WEB_APP_PROOF_DIR").unwrap());
    fs::create_dir_all(&directory).unwrap();
    let package = directory.join("input/writing-method");
    fs::create_dir_all(&package).unwrap();
    fs::write(package.join("SKILL.md"),"---\nname: writing-method\ndescription: Synthetic fixed writing method\n---\ncandidate-v2\n").unwrap();
    fs::write(directory.join("fixture.json"),json!({"origin":origin,"package":package,"previewRef":preview.preview_ref,"baselineConfigRef":enabled.task_config_ref,"firstVersionRef":first.version_ref}).to_string()).unwrap();
    let first_phase = browser(&node, &directory, "trial").await;
    let refreshed_phase = if first_phase.success() {
        fixture::dossier::fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成替代版本","bodyText":"新版原文","authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
        Some(browser(&node, &directory, "refreshed-source").await)
    } else {
        None
    };
    let restricted_phase = if refreshed_phase
        .as_ref()
        .is_some_and(|phase| phase.success())
    {
        fixture::restrict_comment(&db, restricted_work).await;
        Some(browser(&node, &directory, "restricted-source").await)
    } else {
        None
    };
    drain.request();
    let drained = tokio::time::timeout(Duration::from_secs(20), service).await;
    server.abort();
    let _ = server.await;
    let actual_calls = endpoint.requests.lock().unwrap().len();
    endpoint.finish().await;
    assert!(
        drained.unwrap().unwrap().is_ok(),
        "owned service drains after the browser proof"
    );
    assert!(first_phase.success(), "inspect the trial browser report");
    assert!(
        refreshed_phase.is_some_and(|phase| phase.success()),
        "inspect the refreshed-source browser report"
    );
    assert!(
        restricted_phase.is_some_and(|phase| phase.success()),
        "inspect the restricted-source browser report"
    );
    assert_eq!(
        actual_calls, 3,
        "one probe and two trial requests, never ordinary writing"
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM linggan_model_invocation WHERE usage_state='measured' AND measured_tokens=26),(SELECT count(*) FROM linggan_creation_ai_request),(SELECT count(*) FROM linggan_ai_batch_item_control WHERE state='running')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        counts,
        (3, 0, 0),
        "one actual probe plus two trial calls; no ordinary writing requests"
    );
}
