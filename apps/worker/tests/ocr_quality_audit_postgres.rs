//! Owned DB + actual media/AI executor + pinned SDK + loopback only. No real provider.
#![allow(dead_code)]
#![recursion_limit = "256"]
#[path = "support/ocr_audit_endpoint.rs"]
mod endpoint;
#[path = "../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod fixture;
#[path = "ocr_vl_postgres.rs"]
mod ocr;
#[path = "../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;
use linggan_evidence::research_dossier;
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        control::BudgetPolicy,
        control_store::ControlScope,
        executor::{ExecutorSession, ocr_audit, queue},
        provider_store::{MaterialClass, PriceSource, SavePrice, save_price},
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
        usage::TokenPrices,
    },
    model_secrets::SyntheticModelSecrets,
    ocr_quality_audit::{
        self as audit,
        store::{SaveSettings, StartRun},
    },
    pi_adapter::PiAdapter,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use uuid::Uuid;
fn adapter() -> PiAdapter {
    PiAdapter::configured_with_test_command(
        PathBuf::from(std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap()),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    )
}
async fn execute(
    db: &linggan_storage_postgres::Database,
    owner: &ExecutorSession,
    adapter: &PiAdapter,
) {
    let item = queue::claim_next(db, owner.fence())
        .await
        .unwrap()
        .expect("owned queued item");
    assert_eq!(item.task_key, TaskKey::OcrQualityAudit);
    ocr_audit::execute(db, &item, &SyntheticModelSecrets, adapter)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "owned disposable runner only"]
async fn synthetic_audit_blind_read_review_report_and_withdrawal() {
    let db = fixture::db("ocr_quality_audit_owned").await;
    let initial = audit::store::read_settings(&db).await.unwrap();
    assert_eq!(initial["settings"]["automatic"], false);
    assert!(audit::schedule::tick(&db).await.unwrap().is_none());
    let fresh = std::env::var_os("OCR_AUDIT_FRESH_LOCAL_DIR").map(PathBuf::from);
    if fresh.is_some() {
        assert!(std::env::var_os("GITHUB_ACTIONS").is_none());
    }
    let image = if let Some(p) = &fresh {
        std::fs::read(p.join("original.png")).unwrap()
    } else {
        audit::source::probe_image_bytes()
    };
    let work = ocr::admit(&db, "ocr-audit-synthetic", 1, &image, "image/png").await;
    let script = std::env::var_os("LINGGAN_PADDLE_VL_SCRIPT").unwrap();
    let synthetic = ocr::SYNTHETIC_BRIDGE.replace("width=100,height=100", "width=144,height=96");
    if let Some(fresh_dir) = &fresh {
        let cache = PathBuf::from(&script)
            .parent()
            .unwrap()
            .join("fresh-synthetic");
        std::fs::create_dir_all(&cache).unwrap();
        for name in ["original.png", "bridge.json", "frozen-reference.json"] {
            std::fs::copy(fresh_dir.join(name), cache.join(name)).unwrap();
        }
        std::fs::write(&script,r#"import os,sys,json,hashlib
from pathlib import Path
p=Path(__file__).parent/'fresh-synthetic'
frozen=json.loads((p/'frozen-reference.json').read_text());assert frozen['scope']=='procedural_synthetic_not_real_material' and frozen['expectedText']==['SYNTHETIC OCR','42']
b=json.loads((p/'bridge.json').read_text());m=json.load(open(sys.argv[2]));o=b['observation']
assert hashlib.sha256(Path(sys.argv[1]).read_bytes()).hexdigest()==o['image']['blobSha256']==frozen['identity']['blobSha256']
i=dict(m['image'],width=o['image']['width'],height=o['image']['height'])
o['image']=i;o['anchors']['image']=i;o['layout']['image']=i;o['imageRole']=m['imageRole'];o['sequenceOrdinal']=m['sequenceOrdinal']
print(json.dumps(b,ensure_ascii=False))
"#).unwrap();
    } else {
        std::fs::write(&script, synthetic).unwrap();
    }
    ocr::execute_one(&db).await;
    let package:Uuid=sqlx::query_scalar("SELECT package_ref FROM linggan_material_content_detail WHERE content_public_ref=$1 LIMIT 1").bind(work).fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(fixture::dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    let candidate = linggan_evidence::material_reading::read_candidates(&db, work)
        .await
        .unwrap()
        .remove(0);
    let reading: Uuid = serde_json::from_value(candidate["candidateRevisionRef"].clone()).unwrap();
    let unit: Uuid = serde_json::from_value(candidate["unitRef"].clone()).unwrap();
    let adopt = linggan_evidence::material_reading::ReviewReadingRequest {
        command_ref: Uuid::new_v4(),
        expected_current_revision_ref: None,
        candidate_revision_ref: reading,
        action: "adopt".into(),
        edited_blocks: None,
        verified: false,
        resolved_issue_keys: vec![],
        actor: "synthetic-reviewer".into(),
        reason: "Compare the complete synthetic source image".into(),
    };
    linggan_evidence::material_reading::review_reading(&db, unit, &adopt)
        .await
        .unwrap();
    let source_before: Value = sqlx::query_scalar("SELECT observation FROM linggan_media_ocr_adoption WHERE derivative_ref=(SELECT derivative_ref FROM linggan_material_reading WHERE reading_ref=$1)")
        .bind(reading).fetch_one(db.pool()).await.unwrap();
    let wire = endpoint::Endpoint::start(false).await;
    let adapter = adapter();
    let mut owner = ExecutorSession::acquire(&db, "synthetic-ocr-audit", 4)
        .await
        .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &wire.url, &owner, &adapter).await;
    let identity: Uuid = sqlx::query_scalar(
        "SELECT identity_ref FROM linggan_ai_model_identity WHERE connection_ref=$1",
    )
    .bind(model.connection)
    .fetch_one(db.pool())
    .await
    .unwrap();
    save_price(
        &db,
        &SavePrice {
            command_ref: Uuid::new_v4(),
            identity_ref: identity,
            endpoint_ref: model.endpoint,
            expected_price_ref: None,
            source: PriceSource::Manual,
            source_version: "synthetic-cost-ceiling".into(),
            prices: TokenPrices {
                input: Some(300_000),
                output: Some(1_200_000),
                cached_read: Some(6_000),
                cached_write: Some(300_000),
            },
        },
    )
    .await
    .unwrap();
    save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::OcrQualityAudit,
            expected_revision: 0,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: model.capability,
                limits: model.limits.clone(),
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Capped(200_000),
        },
    )
    .await
    .unwrap();
    let settings = audit::Settings {
        domain_ref: Some(fixture::dossier::DOMAIN),
        privacy_cleared_refs: vec![reading],
        privacy_clearance_actor: "SYNTHETIC-PROOF-NOT-MOG".into(),
        privacy_clearance_reason: "Procedurally generated pixels only; no real personal material"
            .into(),
        random_count: 1,
        risk_count: 0,
        regression_count: 0,
        ..audit::Settings::default()
    };
    audit::store::save_settings(
        &db,
        &SaveSettings {
            command_ref: Uuid::new_v4(),
            expected_revision: 0,
            settings,
        },
    )
    .await
    .unwrap();
    let start = StartRun {
        command_ref: Uuid::new_v4(),
        run_key: "manual:disabled-guard".into(),
        seed: "synthetic-seed".into(),
        visual_probe: false,
    };
    assert!(audit::store::start_run(&db, &start).await.is_err());
    assert_eq!(wire.requests.lock().unwrap().len(), 1);
    setup::enable(&db, ControlScope::Task(TaskKey::OcrQualityAudit)).await;
    setup::allow(
        &db,
        model.endpoint,
        TaskKey::OcrQualityAudit,
        MaterialClass::SyntheticImageProbe,
    )
    .await;
    setup::allow(
        &db,
        model.endpoint,
        TaskKey::OcrQualityAudit,
        MaterialClass::OcrQualityImage,
    )
    .await;
    let prior_visual_probes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ocr_audit_observation WHERE vision_qualified",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        prior_visual_probes, 0,
        "real-image admission must not require a 42 receipt"
    );
    let run = audit::store::start_run(&db, &start).await.unwrap();
    let run_ref: Uuid = serde_json::from_value(run["runRef"].clone()).unwrap();
    assert_eq!(
        audit::store::start_run(&db, &start).await.unwrap(),
        run,
        "duplicate command is a receipt replay"
    );
    execute(&db, &owner, &adapter).await;
    let observed: bool = sqlx::query_scalar("SELECT o.result IS NOT NULL AND i.state='succeeded' FROM linggan_ocr_audit_observation o JOIN linggan_ocr_audit_sample s USING(sample_ref) JOIN linggan_model_invocation i ON i.invocation_ref=o.invocation_ref WHERE s.run_ref=$1")
        .bind(run_ref).fetch_one(db.pool()).await.unwrap();
    assert!(
        observed,
        "direct image succeeds without any synthetic visual qualification"
    );
    let requests = wire.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        2,
        "one historical text fixture plus one direct image; no visual preflight"
    );
    let sent: Value = serde_json::from_str(&endpoint::text(&requests[1], "user")).unwrap();
    assert_eq!(
        sent["imageContext"]["imageRole"],
        source_before["imageRole"]
    );
    assert_eq!(
        sent["imageContext"]["sequenceOrdinal"],
        source_before["sequenceOrdinal"]
    );
    drop(requests);
    let report = audit::report::report(&db, run_ref).await.unwrap();
    assert_eq!(report["pools"][0]["selected"], 1);
    assert_eq!(report["samples"][0]["workRef"], json!(work));
    assert_eq!(report["domainRef"], json!(fixture::dossier::DOMAIN));
    assert_eq!(report["pools"][0]["humanReviewedSamples"], 0);
    assert_eq!(
        report["pools"][0]["metrics"]["bodyMissing"]["denominator"],
        0
    );
    let sample: Uuid =
        sqlx::query_scalar("SELECT sample_ref FROM linggan_ocr_audit_sample WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let detail = audit::report::detail(&db, sample).await.unwrap();
    assert!(detail["issues"].as_array().unwrap().len() > 0);
    assert_eq!(detail["modelOpinion"]["regions"][1]["role"], "brand");
    assert_eq!(
        detail["modelOpinion"]["regions"][2]["role"],
        "call_to_action"
    );
    let issue: Uuid = serde_json::from_value(detail["issues"][0]["issue_ref"].clone()).unwrap();
    let freeze: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let before_review = research_dossier::read(&db, work).await.unwrap().unwrap();
    let review = audit::review::Review {
        command_ref: Uuid::new_v4(),
        sample_ref: sample,
        issue_ref: Some(issue),
        expected_revision: 0,
        decision: "confirmed".into(),
        material_review_command_ref: None,
        actor: "SYNTHETIC-PROOF-NOT-MOG".into(),
        reason: "Synthetic pixels checked independently of material selection".into(),
        metrics: audit::review::HumanMetrics {
            body_missing: audit::review::Count {
                numerator: 1,
                denominator: 1,
            },
            ..Default::default()
        },
    };
    let opinion = audit::review::review(&db, &review).await.unwrap();
    assert_eq!(audit::review::review(&db, &review).await.unwrap(), opinion);
    let d = research_dossier::read(&db, work).await.unwrap().unwrap();
    assert_eq!(
        d.materials, before_review.materials,
        "audit opinion cannot mutate the material reading"
    );
    let keep = linggan_evidence::material_reading::ReviewReadingRequest {
        command_ref: Uuid::new_v4(),
        expected_current_revision_ref: Some(reading),
        candidate_revision_ref: reading,
        action: "keep".into(),
        edited_blocks: None,
        verified: false,
        resolved_issue_keys: vec![],
        actor: "synthetic-reviewer".into(),
        reason: "Compare the complete synthetic source image".into(),
    };
    linggan_evidence::material_reading::review_reading(&db, unit, &keep)
        .await
        .unwrap();
    let dismiss = audit::review::Review {
        command_ref: Uuid::new_v4(),
        expected_revision: 1,
        decision: "dismissed".into(),
        material_review_command_ref: Some(keep.command_ref),
        ..review.clone()
    };
    audit::review::review(&db, &dismiss).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let frozen = research_dossier::read_in(&mut tx, &[work], &freeze)
        .await
        .unwrap();
    assert_eq!(frozen[&work].materials, before_review.materials);
    tx.rollback().await.unwrap();
    let after:Value=sqlx::query_scalar("SELECT observation FROM linggan_media_ocr_adoption WHERE derivative_ref=(SELECT derivative_ref FROM linggan_material_reading WHERE reading_ref=$1)").bind(reading).fetch_one(db.pool()).await.unwrap();
    assert_eq!(source_before, after);
    let report = audit::report::report(&db, run_ref).await.unwrap();
    assert_eq!(
        report["pools"][0]["metrics"]["bodyMissing"]["denominator"],
        1
    );
    let bodies = wire.requests.lock().unwrap();
    assert_eq!(bodies.len(), 2);
    for b in bodies.iter().skip(1) {
        let content = &b["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["role"] == "user")
            .unwrap()["content"];
        assert_eq!(
            content
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "image_url")
                .count(),
            1
        );
        assert!(!content.to_string().contains("Synthetic body"));
    }
    drop(bodies);
    // Preserve synthetic UI fixtures before testing current-source withdrawal.
    if let Some(out) = std::env::var_os("OCR_AUDIT_PROOF_OUTPUT") {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(PathBuf::from(&out).join("fixture.json"),serde_json::to_vec_pretty(&json!({"schema":"ocr_quality_audit_owned","domain":fixture::dossier::DOMAIN,"workRef":work,"readingRef":reading,"sampleRef":sample,"runRef":run_ref,"report":report,"detail":audit::report::detail(&db,sample).await.unwrap(),"dossier":d})).unwrap()).unwrap();
    }
    // Weekly schedule: exactly Monday 09:00 Asia/Shanghai, one frozen run per week.
    let automatic = audit::Settings {
        automatic: true,
        domain_ref: Some(fixture::dossier::DOMAIN),
        privacy_cleared_refs: vec![reading],
        privacy_clearance_actor: "SYNTHETIC-PROOF-NOT-MOG".into(),
        privacy_clearance_reason: "Procedurally generated pixels only; no real personal material"
            .into(),
        random_count: 1,
        risk_count: 0,
        regression_count: 0,
        ..Default::default()
    };
    audit::store::save_settings(
        &db,
        &SaveSettings {
            command_ref: Uuid::new_v4(),
            expected_revision: 1,
            settings: automatic,
        },
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT '2030-01-07T08:59:00+08:00'::timestamptz $$").execute(db.pool()).await.unwrap();
    assert!(audit::schedule::tick(&db).await.unwrap().is_none());
    sqlx::raw_sql("CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT '2030-01-07T09:00:00+08:00'::timestamptz $$").execute(db.pool()).await.unwrap();
    let weekly = audit::schedule::tick(&db)
        .await
        .unwrap()
        .expect("one scheduled run");
    assert!(audit::schedule::tick(&db).await.unwrap().is_none());
    let weekly_run: Uuid = serde_json::from_value(weekly["runRef"].clone()).unwrap();
    let batch: Uuid =
        sqlx::query_scalar("SELECT batch_ref FROM linggan_ai_batch WHERE command_ref=$1")
            .bind(weekly_run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    linggan_intelligence::ai_foundation::batch_control::cancel_batch(
        &db,
        &linggan_intelligence::ai_foundation::batch_control::CancelBatch {
            command_ref: Uuid::new_v4(),
            batch_ref: batch,
            expected_revision: 0,
            reason: "Synthetic schedule proof completes without another call".into(),
        },
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT clock_timestamp() $$").execute(db.pool()).await.unwrap();
    // Local eligibility alone never authorizes egress; revoke a frozen sample before dispatch.
    let privacy_run = audit::store::start_run(
        &db,
        &StartRun {
            command_ref: Uuid::new_v4(),
            run_key: "manual:privacy-proof".into(),
            seed: "privacy-proof".into(),
            visual_probe: false,
        },
    )
    .await
    .unwrap();
    let private_settings = audit::Settings {
        domain_ref: Some(fixture::dossier::DOMAIN),
        random_count: 1,
        risk_count: 0,
        regression_count: 0,
        ..Default::default()
    };
    audit::store::save_settings(
        &db,
        &SaveSettings {
            command_ref: Uuid::new_v4(),
            expected_revision: 2,
            settings: private_settings,
        },
    )
    .await
    .unwrap();
    execute(&db, &owner, &adapter).await;
    let privacy_run: Uuid = serde_json::from_value(privacy_run["runRef"].clone()).unwrap();
    let stopped: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ai_batch_item_control c JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) WHERE b.command_ref=$1 AND c.state='skipped' AND c.attempt_count=0 AND c.failure_code='audit_privacy_unreviewed'").bind(privacy_run).fetch_one(db.pool()).await.unwrap();
    assert_eq!(stopped, 1);
    assert_eq!(
        audit::report::report(&db, privacy_run).await.unwrap()["samples"][0]["failureCode"],
        "audit_privacy_unreviewed"
    );
    let uncleared = audit::store::start_run(
        &db,
        &StartRun {
            command_ref: Uuid::new_v4(),
            run_key: "manual:uncleared".into(),
            seed: "uncleared".into(),
            visual_probe: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(uncleared["selected"], 0);
    assert_eq!(uncleared["inventory"]["skippedPrivacyUnreviewed"], 1);
    assert_eq!(wire.requests.lock().unwrap().len(), 2);
    let recent = audit::report::recent(&db).await.unwrap();
    assert!(
        recent["runs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["runRef"] == json!(run_ref)),
        "the UI fixture's business run must remain in the four recent manual slots"
    );
    owner.heartbeat().await.unwrap();
    if let Some(binary) = std::env::var_os("OCR_AUDIT_BROWSER_PROOF_BINARY") {
        let status = tokio::task::spawn_blocking(move || {
            let proof =
                "local_web::ocr_audit::tests::owned_audit_ui_reads_and_appends_a_human_opinion";
            let listed = std::process::Command::new(&binary)
                .args([proof, "--ignored", "--exact", "--list"])
                .output()
                .expect("list owned audit browser proof");
            assert!(
                listed.status.success()
                    && String::from_utf8_lossy(&listed.stdout)
                        .lines()
                        .any(|line| line == format!("{proof}: test")),
                "the API binary must contain the exact owned audit browser proof"
            );
            std::process::Command::new(binary)
                .arg(proof)
                .arg("--ignored")
                .arg("--exact")
                .arg("--nocapture")
                .status()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(status.success());
    }
    // The optional diagnostic cannot evict the business fixture from the recent-run UI.
    owner.heartbeat().await.unwrap();
    // An explicit diagnostic remains independent, after the business image already succeeded.
    let probe = StartRun {
        command_ref: Uuid::new_v4(),
        run_key: "probe:synthetic-vision".into(),
        seed: "synthetic-probe".into(),
        visual_probe: true,
    };
    let receipt = audit::store::start_run(&db, &probe).await.unwrap();
    assert_eq!(receipt["selected"], 1);
    execute(&db, &owner, &adapter).await;
    let qualified: bool =
        sqlx::query_scalar("SELECT o.vision_qualified FROM linggan_ocr_audit_observation o JOIN linggan_ocr_audit_sample s USING(sample_ref) WHERE s.pool='probe'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(qualified);
    assert_eq!(wire.requests.lock().unwrap().len(), 3);
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    let source = audit::source::load_in(&mut db.pool().acquire().await.unwrap(), reading)
        .await
        .unwrap()
        .unwrap();
    let current_revision: i64 = sqlx::query_scalar(
        "SELECT max(revision) FROM linggan_ocr_audit_review WHERE sample_ref=$1",
    )
    .bind(sample)
    .fetch_one(db.pool())
    .await
    .unwrap();
    audit::review::review(
        &db,
        &audit::review::Review {
            command_ref: Uuid::new_v4(),
            expected_revision: current_revision,
            decision: "pending".into(),
            issue_ref: None,
            metrics: Default::default(),
            ..review.clone()
        },
    )
    .await
    .unwrap();
    let reopened = audit::report::report(&db, run_ref).await.unwrap();
    assert_eq!(
        reopened["pools"][0]["humanReviewedSamples"], 0,
        "a pending latest opinion cannot fall back to an older reviewed version"
    );
    assert_eq!(
        reopened["pools"][0]["metrics"]["bodyMissing"]["denominator"],
        0
    );
    assert_eq!(
        reopened["pools"][0]["pendingDifferences"], reopened["pools"][0]["modelDifferences"],
        "a whole-image pending opinion reopens every difference"
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-ocr-audit-proof','synthetic restriction',clock_timestamp())").bind(Uuid::new_v4()).bind(source.slot_key).execute(db.pool()).await.unwrap();
    assert!(
        audit::review::review(&db, &review).await.unwrap() == opinion,
        "a receipt replay exposes references only, never withdrawn material text"
    );
    assert!(audit::report::detail(&db, sample).await.is_err());
    assert_eq!(
        audit::report::report(&db, run_ref).await.unwrap()["pools"][0]["withheld"],
        1
    );
    wire.finish().await;
    println!(
        "owned OCR worker → AI visual blind read → independent audit opinion → canonical reading/report → withdrawal passed; synthetic only"
    );
}
