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
#[path = "../../../../crates/intelligence/tests/support/content_understanding_fixture.rs"]
mod core_setup;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_endpoint.rs"]
mod endpoint;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;

// Core is a real prerequisite now. Use the existing SDK wire helper and an owned
// loopback response, not a fabricated persisted Core or a browser route stub.
async fn core_endpoint() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = endpoint::read(&mut socket).await;
            let message = request["messages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|message| message["role"] == "user")
                .unwrap();
            // The SDK sends the JSON packet as the first part, followed by each
            // image's identity label and bytes. Do not concatenate the labels.
            let prompt = message["content"]
                .as_str()
                .or_else(|| message["content"][0]["text"].as_str())
                .unwrap();
            let output = if prompt.contains("ai-probe.synthetic.v1") {
                json!({"lingganProbe":"ok"})
            } else {
                let packet: serde_json::Value = serde_json::from_str(prompt).unwrap();
                let source = packet["material"]["texts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|source| source["kind"] == "body")
                    .unwrap();
                assert!(source["text"].as_str().unwrap().contains("结尾不能丢失🙂"));
                let image = &packet["material"]["images"][0];
                json!({
                    "core":{"summary":{"text":"合成正文末尾保留","unitIds":["u1"]},"contentType":"practical_guide","mainTopicUnitId":"u1",
                        "knowledgeUnits":[{"id":"u1","unitType":"discussion","statement":"正文末尾不能丢失。","assertionKind":"author_claim","speakerRole":"author","evidenceRefs":["e1"]}],"candidateLabels":[]},
                    "imageContributions":[{"inputCitationId":image["citationId"],"status":"no_new_information","summary":"合成单像素图片没有新增信息。","unitIds":[],"evidenceRefs":["v1"]}],
                    "evidenceCandidates":{"text":[{"id":"e1","inputCitationId":source["citationId"],"quote":"结尾不能丢失🙂"}],"vision":[{"id":"v1","inputCitationId":image["citationId"],"modelQuote":null,"observation":"合成单像素图片。","speakerRole":"unknown"}]}
                })
            };
            endpoint::respond(&mut socket, &output.to_string()).await;
        }
    });
    (url, task)
}
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
    // A complete one-page inventory and admitted original bytes satisfy Core's real
    // material readiness contract; the old text-only fixture left media unknown.
    let png = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5,
        1, 1, 39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let (image_work, _, _) = dossier::acquired_image_source_with_bytes(&db, Some(png)).await;
    assert_eq!(image_work, work);
    for (id, author) in [("reader-a", "independent-a"), ("reader-b", "independent-b")] {
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":id,"noteId":"dossier-note","text":"我的孩子需要每天有人提醒。","authorId":author}})).await;
    }
    linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, dossier::DOMAIN, 200)
        .await
        .unwrap();
    let endpoint = endpoint::Endpoint::start(false).await;
    let (core_url, core_server) = core_endpoint().await;
    let node = std::env::var("WEB_APP_PROOF_NODE").unwrap();
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        PathBuf::from(&node),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    );
    let mut owner = ai::executor::ExecutorSession::acquire(&db, "ui-proof-bootstrap", 4)
        .await
        .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    let core_model = setup::prepare_with_actual_probe(&db, &core_url, &owner, &adapter).await;
    core_setup::configure_high(&db, &core_model).await;
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
    let preview = ai::content_understanding::preview(&db, dossier::DOMAIN, work)
        .await
        .unwrap();
    assert_eq!(preview["canStart"], true, "{preview}");
    let core = ai::content_understanding::start(
        &db,
        &ai::content_understanding::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            work_ref: work,
            knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
            input_hash: preview["inputHash"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(preview["taskConfigRef"].clone()).unwrap(),
            task_revision: preview["taskRevision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let ready: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_content_understanding_result WHERE request_ref=$1 AND result IS NOT NULL)")
                .bind(core.request_ref).fetch_one(db.pool()).await.unwrap();
            if ready { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await.expect("synthetic Core must be accepted before opening topic research");
    let application = app_with_database(db.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory = PathBuf::from(std::env::var("WEB_APP_PROOF_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("fixture.json"),
        json!({"origin":origin,"domain":dossier::DOMAIN,"work":work,"coreRef":core.request_ref}).to_string(),
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
    core_server.abort();
    let _ = core_server.await;
    assert!(drained.unwrap().unwrap().is_ok());
    assert!(status.success(), "inspect unified research browser report");
    let overview = ai::comment_research_overview::read(&db, dossier::DOMAIN)
        .await
        .unwrap();
    assert_eq!(overview["researchSummary"]["currentSignalCount"], 2);
    assert_eq!(overview["knowledgeSummary"]["problemCount"], 1);
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'reader-a','synthetic source withdrawal')").bind(work).execute(db.pool()).await.unwrap();
    let withdrawn = ai::comment_research_overview::read(&db, dossier::DOMAIN)
        .await
        .unwrap();
    assert_eq!(withdrawn["researchSummary"]["currentSignalCount"], 1);
    assert_eq!(withdrawn["knowledgeSummary"]["problemCount"], 0);
    dossier::fixture::submit_package_at(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":"reader-b","noteId":"dossier-note","text":"😀","authorId":"independent-b"}}),"2026-09-01T10:00:00Z").await;
    dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":"reader-c","noteId":"dossier-note","text":"每天需要提醒才能开始。","authorId":"independent-c"}})).await;
    linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, dossier::DOMAIN, 200)
        .await
        .unwrap();
    let current = ai::comment_research_overview::read(&db, dossier::DOMAIN)
        .await
        .unwrap();
    assert_eq!(current["corpusSummary"]["eligibleCommentCount"], 1);
    assert_eq!(current["researchSummary"]["studiedCommentCount"], 0);
    assert_eq!(current["researchSummary"]["currentSignalCount"], 0);
    assert_eq!(
        current["observationSeries"]["points"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["newObservedCommentCount"],
        3,
        "a newer comment version is not a new stable comment identity"
    );
}

#[tokio::test]
#[ignore = "owned PostgreSQL only; no model, patrol or collection process"]
async fn comment_overview_recovery_keeps_sources_and_rebuilds_only_the_index() {
    let db = dossier::fixture::proof_database("comment_overview_recovery").await;
    dossier::work(&db).await;
    for (id, author) in [("voice-a", "author-a"), ("voice-b", "author-b")] {
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":id,"noteId":"dossier-note","text":"我的孩子需要每天有人提醒。","authorId":author}})).await;
    }
    let before = ai::comment_research_overview::read(&db, dossier::DOMAIN)
        .await
        .unwrap();
    assert_eq!(before["corpusSummary"]["rawReadableCommentCount"], 2);
    assert_eq!(before["indexCoverage"]["pendingCount"], 2);
    assert_eq!(before["knowledgeSummary"]["problemCount"], 0);
    assert_eq!(
        before["observationSeries"]["points"]
            .as_array()
            .unwrap()
            .len(),
        56
    );
    let final_day = before["observationSeries"]["points"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(final_day["newObservedCommentCount"], 2);
    assert_eq!(final_day["acceptedSignalCount"], 0);
    let source_before:(i64,String)=sqlx::query_as("SELECT count(*),sum(hashtextextended(to_jsonb(t)::text,0)::numeric)::text FROM linggan_material_comment t").fetch_one(db.pool()).await.unwrap();
    let rebuilt =
        linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, dossier::DOMAIN, 200)
            .await
            .unwrap();
    assert_eq!(rebuilt.inserted_count, 2);
    assert_eq!(
        linggan_intelligence::comment_study_catalog::refresh_clean_cache(&db, dossier::DOMAIN, 200)
            .await
            .unwrap()
            .inserted_count,
        0
    );
    let after = ai::comment_research_overview::read(&db, dossier::DOMAIN)
        .await
        .unwrap();
    assert_eq!(after["indexCoverage"]["pendingCount"], 0);
    assert_eq!(after["corpusSummary"]["eligibleCommentCount"], 2);
    assert_eq!(after["researchSummary"]["studiedCommentCount"], 0);
    let source_after:(i64,String)=sqlx::query_as("SELECT count(*),sum(hashtextextended(to_jsonb(t)::text,0)::numeric)::text FROM linggan_material_comment t").fetch_one(db.pool()).await.unwrap();
    assert_eq!(source_before, source_after);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
    let legacy: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_run') IS NOT NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(!legacy);
    assert!(matches!(
        ai::comment_research_overview::read(&db, Uuid::new_v4()).await,
        Err(ai::store::FoundationError::NotFound)
    ));
}

#[tokio::test]
#[ignore = "owned PostgreSQL only; domain-scoped deterministic cleaning, no model"]
async fn comment_catalog_refresh_rejects_missing_domain_and_preserves_other_domains() {
    let db = dossier::fixture::proof_database("comment_catalog_domain").await;
    let first = dossier::work(&db).await;
    let second = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain(domain_ref,name) VALUES($1,'synthetic second index domain')").bind(second).execute(db.pool()).await.unwrap();
    let package=dossier::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"other-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"other-note"},"payload":{"title":"另一个合成作品","bodyText":"仅供第二领域测试。","authorId":"other-creator"}})).await;
    let other: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='other-note'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(other).bind(second).bind(package).execute(db.pool()).await.unwrap();
    for note in ["dossier-note", "other-note"] {
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":note}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":{"commentId":format!("{note}-comment"),"noteId":note,"text":"我需要明确的执行步骤。","authorId":"independent-reader"}})).await;
    }
    use linggan_intelligence::comment_study_catalog::{StudyCatalogError, refresh_clean_cache};
    assert!(matches!(
        refresh_clean_cache(&db, Uuid::new_v4(), 200).await,
        Err(StudyCatalogError::UnsupportedDomain)
    ));
    let empty: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_comment_clean_cache")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(empty, 0);
    assert_eq!(
        refresh_clean_cache(&db, dossier::DOMAIN, 200)
            .await
            .unwrap()
            .inserted_count,
        1
    );
    let refs:Vec<Uuid>=sqlx::query_scalar("SELECT source.content_public_ref FROM linggan_comment_clean_cache cache JOIN linggan_material_comment source ON source.material_ref=cache.source_ref").fetch_all(db.pool()).await.unwrap();
    assert_eq!(refs, vec![first]);
    assert_eq!(
        refresh_clean_cache(&db, second, 200)
            .await
            .unwrap()
            .inserted_count,
        1
    );
    assert_eq!(
        refresh_clean_cache(&db, dossier::DOMAIN, 200)
            .await
            .unwrap()
            .inserted_count,
        0
    );
}
