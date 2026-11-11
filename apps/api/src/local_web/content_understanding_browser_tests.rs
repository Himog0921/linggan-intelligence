use super::*;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "../../../../crates/intelligence/tests/support/content_understanding_fixture.rs"]
mod l1_setup;
#[path = "../../../../crates/intelligence/tests/support/ai_writing_fixture.rs"]
mod setup;
#[path = "../../../../crates/intelligence/tests/support/content_understanding_endpoint.rs"]
mod wire;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "owned PostgreSQL + built React + Node/Chromium; synthetic loopback model only"]
async fn content_understanding_real_browser_chain() {
    let output = PathBuf::from(std::env::var_os("WEB_APP_PROOF_DIR").unwrap());
    assert!(output.is_absolute());
    fs::create_dir_all(&output).unwrap();
    let db =
        dossier::fixture::proof_database(&format!("l1_browser_{}", Uuid::new_v4().simple())).await;
    let endpoint = wire::Endpoint::start().await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    );
    let mut session = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "l1-browser-probe",
        4,
    )
    .await
    .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &session, &adapter).await;
    l1_setup::configure_high(&db, &model).await;
    let deep_config = configure_deep(&db, &model).await;
    session.request_drain().await.unwrap();
    session.complete_drain().await.unwrap();
    drop(session);

    let body = "合成说明🙂：先摆出材料，再按步骤核对；只用于流程测试。";
    let mut works = Vec::new();
    for (external, text) in [
        ("dossier-note", body),
        ("l1-unprocessed", "这篇合成原文尚未经过理解，仍可完整阅读。"),
    ] {
        let package = dossier::fixture::submit_package(&db, "content_detail", json!({"contentExternalId":external}), json!({
            "kind":"content_detail", "sourceObject":{"platform":"xhs","type":"content","externalId":external},
            "payload":{"title":format!("合成理解流程 {external}"),"bodyText":text,"authorId":"synthetic-author"}
        })).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(external)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
            .bind(Uuid::new_v4()).bind(work).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        works.push(work);
    }
    let mut image = Vec::new();
    {
        let encoder = png::Encoder::new(&mut image, 1, 1);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0]).unwrap();
    }
    let (image_work, _, _) = dossier::acquired_image_source_with_bytes(&db, Some(image)).await;
    assert_eq!(image_work, works[0]);
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let worker = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "l1-browser-worker",
        4,
        Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let application = app_with_database(db.clone());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    fs::write(output.join("fixture.json"), serde_json::to_vec_pretty(&json!({
        "origin":origin,"domain":dossier::DOMAIN,"work":works[0],"unprocessed":works[1],"body":body,
        "provider":"owned loopback synthetic response", "realModelQualityVerified":false,
        "deepConfigRef":deep_config.task_config_ref, "deepRevision":deep_config.revision,
        "endpointRef":model.endpoint
    })).unwrap()).unwrap();
    let status = std::process::Command::new(std::env::var_os("WEB_APP_PROOF_NODE").unwrap())
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../web/test/e2e/content-understanding-real-chain.mjs"),
        )
        .status()
        .unwrap();
    server.abort();
    let _ = server.await;
    drain.request();
    worker.await.unwrap().unwrap();
    let deep_calls = endpoint
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|request| {
            serde_json::from_str::<Value>(&wire::text(request, "user"))
                .is_ok_and(|packet| packet["question"].is_string())
        })
        .count();
    endpoint.finish().await;
    assert!(status.success(), "browser report: {}", output.display());
    let report: Value =
        serde_json::from_slice(&fs::read(output.join("report.json")).unwrap()).unwrap();
    let saved: Value = sqlx::query_scalar(
        "SELECT result FROM linggan_content_understanding_result WHERE result IS NOT NULL",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(saved, report["result"]);
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM linggan_content_understanding_request),(SELECT count(*) FROM linggan_model_invocation WHERE task_key='content_understanding')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (1, 1), "refreshing and rereading do not regenerate");
    let deep_counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
          (SELECT count(*) FROM linggan_content_understanding_deep WHERE invocation_ref IS NULL),
          (SELECT count(*) FROM linggan_content_understanding_deep d
             JOIN linggan_model_invocation i USING(invocation_ref)
             WHERE i.task_key='content_understanding_deep' AND i.state='succeeded'),
          (SELECT count(*) FROM linggan_content_understanding_deep_request),
          (SELECT count(*) FROM linggan_model_invocation WHERE task_key='content_understanding_deep'),
          (SELECT count(*) FROM linggan_content_understanding_deep_request
             JOIN linggan_ai_batch_item_control USING(item_ref)
             WHERE failure_code='ai_output_invalid' AND attempt_count=1)"
    ).fetch_one(db.pool()).await.unwrap();
    assert_eq!(deep_counts, (1, 1, 2, 2, 1));
    assert_eq!(deep_calls, 2);
    db.pool().close().await;
}

async fn configure_deep(
    db: &linggan_storage_postgres::Database,
    model: &setup::Configured,
) -> linggan_intelligence::ai_foundation::task_config::TaskConfigReceipt {
    use linggan_intelligence::ai_foundation::{
        TaskKey,
        capability::ModelLimits,
        control::BudgetPolicy,
        control_store::ControlScope,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    };
    let capability_ref = sqlx::query_scalar("SELECT capability_ref FROM linggan_model_capability WHERE identity_ref=(SELECT identity_ref FROM linggan_model_capability WHERE capability_ref=$1) AND endpoint_ref=$2 ORDER BY revision_sequence DESC LIMIT 1")
        .bind(model.capability).bind(model.endpoint).fetch_one(db.pool()).await.unwrap();
    let config = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ContentUnderstandingDeep,
            expected_revision: 0,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref,
                limits: ModelLimits {
                    input_tokens: 8192,
                    output_tokens: 1024,
                    timeout_seconds: 30,
                    attempts: 1,
                    thinking_level: "off".into(),
                },
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Capped(30_000),
        },
    )
    .await
    .unwrap();
    assert!(config.applied);
    setup::enable(db, ControlScope::Task(TaskKey::ContentUnderstandingDeep)).await;
    config
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "explicit authorized single-note live run; private input, owned PostgreSQL and real credential store required"]
async fn content_understanding_authorized_single_live_chain() {
    use linggan_intelligence::{
        ai_foundation::{
            TaskKey,
            batch_store::{
                self, BatchInput, BatchKind, CreateBatch, InputRef, ProbeOperation, ProbeProfile,
            },
            capability::{Capability, ModelLimits},
            control::BudgetPolicy,
            control_store::{Actor, ChangeControl, ControlScope, change_control},
            executor::{ExecutorSession, probe::execute_probe, queue::claim_next},
            provider_store::{
                CapabilitySource, ChangeEgress, MaterialClass, SaveCapability, change_egress,
                save_capability,
            },
            task_config::{ModelSelection, SaveTaskConfig, save_task_config},
        },
        model_secrets::LocalFileModelSecrets,
        pi_adapter::PiAdapter,
    };
    assert_eq!(
        std::env::var("CONTENT_L1_LIVE_AUTHORIZATION").as_deref(),
        Ok("one-authorized-note-no-retries")
    );
    assert!(std::env::var_os("LINGGAN_MODEL_SYNTHETIC_PREVIEW").is_none());
    let proof_url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    assert!(proof_url.contains("@127.0.0.1:") && proof_url.ends_with("/l1_synthetic_proof"));
    let output = PathBuf::from(std::env::var_os("WEB_APP_PROOF_DIR").unwrap());
    let input: Value =
        serde_json::from_slice(&fs::read(output.join("input.json")).unwrap()).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("started.lock"))
        .expect("never automatically replay a live run");
    let schema = format!("l1_live_{}", Uuid::new_v4().simple());
    fs::write(output.join("schema.txt"), &schema).unwrap();
    let db = dossier::fixture::proof_database(&schema).await;
    let route = &input["route"];
    assert_eq!(route["api"], "openai-responses");
    assert_eq!(route["baseUrl"], "https://api.deepseek.com/v1");
    let workspace: Uuid = serde_json::from_value(route["workspaceRef"].clone()).unwrap();
    let secret: Uuid = serde_json::from_value(route["secretRef"].clone()).unwrap();
    sqlx::query("UPDATE linggan_model_workspace SET workspace_ref=$1 WHERE singleton")
        .bind(workspace)
        .execute(db.pool())
        .await
        .unwrap();
    let connection = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision) VALUES($1,1)")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref) VALUES($1,$2,1,'Authorized isolated single-note replay',$3,$4,false,$5)")
        .bind(version).bind(connection).bind(route["api"].as_str().unwrap()).bind(route["baseUrl"].as_str().unwrap()).bind(secret).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) VALUES($1,$2,$3,'manual')")
        .bind(model).bind(version).bind(route["modelId"].as_str().unwrap()).execute(db.pool()).await.unwrap();
    let endpoint: Uuid = sqlx::query_scalar(
        "SELECT endpoint_ref FROM linggan_ai_connection_endpoint WHERE version_ref=$1",
    )
    .bind(version)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let identity: Uuid = sqlx::query_scalar("SELECT identity_ref FROM linggan_ai_model_identity WHERE connection_ref=$1 AND model_id=$2")
        .bind(connection).bind(route["modelId"].as_str().unwrap()).fetch_one(db.pool()).await.unwrap();
    let catalog = &input["capability"]["model"];
    let context = catalog["context_window"].as_u64().unwrap();
    let max_output = catalog["max_output_tokens"].as_u64().unwrap();
    let capability = save_capability(&db, &SaveCapability {
        command_ref: Uuid::new_v4(), identity_ref: identity, endpoint_ref: endpoint,
        expected_capability_ref: None, source: CapabilitySource::Manual,
        source_version: "Authenticated 2026-11-10 provider catalog; client wait ceiling 3600s, not provider SLA".into(),
        probe_invocation_ref: None,
        limits: Capability { context_tokens: context, max_output_tokens: max_output, max_timeout_seconds: 3600,
            thinking_levels: serde_json::from_value(catalog["effort"]["supported_levels"].clone()).unwrap() },
        compat: route["compat"].clone(), thinking_level_map: json!({"high":"high"}),
        output_mode: serde_json::from_value(route["outputMode"].clone()).unwrap(), strict_schema: false,
    }).await.unwrap().revision_ref;
    let limits = ModelLimits {
        input_tokens: context - max_output,
        output_tokens: max_output,
        timeout_seconds: 3600,
        attempts: 1,
        thinking_level: "high".into(),
    };
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::ModelProbe),
        ControlScope::Task(TaskKey::ContentUnderstanding),
    ] {
        let revision =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_control WHERE scope_key=$1")
                .bind(scope.key())
                .fetch_one(db.pool())
                .await
                .unwrap();
        change_control(
            &db,
            &ChangeControl {
                command_ref: Uuid::new_v4(),
                scope,
                expected_revision: revision,
                enabled: true,
                reason:
                    "Mog authorized one real note in owned isolated schema; no production changes"
                        .into(),
            },
            Actor::Human,
        )
        .await
        .unwrap();
    }
    for (task, material) in [
        (TaskKey::ModelProbe, MaterialClass::SyntheticProbe),
        (TaskKey::ContentUnderstanding, MaterialClass::NoteMaterial),
    ] {
        change_egress(&db, &ChangeEgress { command_ref: Uuid::new_v4(), endpoint_ref: endpoint, task_key: task, material_class: material,
            expected_event_ref: None, allowed: true, reason: "Bounded one-probe and one-note run; existing confidentiality confirmation; private local results only".into() }, Actor::Human).await.unwrap();
    }
    let revision = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='model_probe'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let probe = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_revision: revision,
            model: None,
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    batch_store::create_batch(
        &db,
        &CreateBatch {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_task_revision: probe.revision,
            task_config_ref: probe.task_config_ref,
            kind: BatchKind::Probe,
            items: vec![BatchInput {
                item_key: "authorized-current-route".into(),
                input: InputRef::Probe {
                    sample_ref: Uuid::new_v4(),
                    connection_version_ref: version,
                    model_ref: Some(model),
                    operation: ProbeOperation::Probe,
                    profile: Some(ProbeProfile {
                        capability_ref: capability,
                        limits: limits.clone(),
                    }),
                },
            }],
        },
    )
    .await
    .unwrap();
    let secrets = Arc::new(LocalFileModelSecrets::new(PathBuf::from(
        std::env::var_os("CONTENT_L1_SECRET_ROOT").unwrap(),
    )));
    let adapter = PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        output.join("capture-adapter.mjs"),
    );
    let revision = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='content_understanding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ContentUnderstanding,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: model,
                capability_ref: capability,
                limits,
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let sample = &input["sample"];
    let external = sample["externalId"].as_str().unwrap();
    let text = sample["text"].as_array().unwrap();
    let title = text.iter().find(|t| t["kind"] == "title").unwrap()["text"]
        .as_str()
        .unwrap();
    let body = input["inventory"]["body_text"].as_str();
    let package = live_submit(&db, "content_detail", external, vec![json!({"kind":"content_detail", "sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":{"title":title,"bodyText":body}})]).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(external)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let domain: Uuid = serde_json::from_value(sample["domain"].clone()).unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(domain).bind(package).execute(db.pool()).await.unwrap();
    for slot in input["replaySlots"].as_array().unwrap() {
        let observation = Uuid::new_v4();
        let key = slot["slotKey"].as_str().unwrap();
        let uri = slot["sourceUrl"].as_str().unwrap();
        live_submit(&db, "media_slots", external, vec![json!({"kind":"media_slot","slotKey":key,"observationRef":observation,
            "slot":{"role":slot["role"],"ordinal":slot["ordinal"]}, "sourceObject":{"platform":"xhs","type":"content","externalId":external},
            "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":sample["materialAsOf"]}})]).await;
        if let Some(blob) = slot.get("blob") {
            linggan_evidence::admit_media_blob(
                &db,
                observation,
                blob["sha256"].as_str().unwrap(),
                blob["mimeType"].as_str().unwrap(),
                blob["bytes"].as_i64().unwrap(),
                blob["storageKey"].as_str().unwrap(),
            )
            .await
            .unwrap();
        }
    }
    let before = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    for material in input["dossier"]["materials"].as_array().unwrap() {
        let kind = material["kind"].as_str().unwrap();
        if !["body", "audio", "video_frame"].contains(&kind)
            || material["currentRevisionRef"].is_null()
        {
            continue;
        }
        let unit = before
            .materials
            .iter()
            .find(|m| m["kind"] == kind && m["slotKey"] == material["slotKey"])
            .unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        let receipt = linggan_evidence::material_reading::publish_in(
            &mut tx,
            linggan_evidence::material_reading::NewReading {
                content_public_ref: work,
                unit_key: if kind == "body" { "body".into() } else {
                    format!("{kind}:{}", unit["slotKey"].as_str().unwrap())
                },
                kind: kind.into(), slot_key: unit["slotKey"].as_str().map(str::to_owned),
                page: None, source_version: unit["sourceVersion"].as_str().unwrap().into(),
                source_state: material["sourceState"].as_str().unwrap().into(),
                derivative_ref: None, producer_ref: Some(Uuid::new_v4()),
                generated_by: material["reading"]["generatedBy"].as_str().unwrap().into(),
                rule_version: "private-authorized-saved-reading-replay.v1".into(),
                blocks: serde_json::from_value(material["blocks"].clone()).unwrap(),
                issues: serde_json::from_value(material["issues"].clone()).unwrap(),
                source_location: json!({"purpose":"isolated_saved_reading_replay",
                    "originalWorkRef":sample["workRef"],"originalReadingRef":material["currentRevisionRef"],
                    "originalSourceVersion":material["sourceVersion"]}),
                publication: linggan_evidence::material_reading::Publication::Automatic {
                    expected_current: serde_json::from_value(unit["currentRevisionRef"].clone()).unwrap(),
                },
            },
        ).await.unwrap();
        assert!(receipt.current);
        tx.commit().await.unwrap();
    }
    let imported = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let mut expected: Vec<_> = text
        .iter()
        .map(|t| {
            (
                t["kind"].as_str().unwrap().to_owned(),
                t["text"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut actual: Vec<_> = imported
        .fragments
        .iter()
        .filter(|f| !f.text.trim().is_empty())
        .map(|f| (f.field.clone(), f.text.clone()))
        .collect();
    expected.sort();
    actual.sort();
    assert_eq!(
        actual, expected,
        "isolated import must preserve every text fragment and its kind"
    );
    assert_eq!(
        imported
            .materials
            .iter()
            .filter(|m| m["kind"] == "image" && m["originalUrl"].is_null())
            .count() as u64,
        sample["unavailableImageSlots"].as_u64().unwrap()
    );
    for original in input["dossier"]["materials"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["kind"] == "audio" || m["kind"] == "video_frame")
    {
        let replay = imported
            .materials
            .iter()
            .find(|m| m["kind"] == original["kind"] && m["slotKey"] == original["slotKey"])
            .unwrap();
        let without_refs = |material: &Value| {
            material["blocks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|block| {
                    let mut block = block.clone();
                    block.as_object_mut().unwrap().remove("ref");
                    block
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            without_refs(replay),
            without_refs(original),
            "saved speech and timing must be unchanged"
        );
        assert_eq!(replay["sourceState"], original["sourceState"]);
    }
    fs::write(output.join("imported.json"), serde_json::to_vec_pretty(&json!({
        "workRef":work,"textFragments":actual,"materials":imported.materials,"coverage":imported.coverage,
        "sourceIdentity":"isolated reimport, not production UUIDs"
    })).unwrap()).unwrap();
    if std::env::var_os("CONTENT_L1_IMPORT_ONLY").is_some() {
        let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(calls, 0);
        db.pool().close().await;
        return;
    }
    let unprocessed_external = "live-chain-unprocessed-synthetic-control";
    let unprocessed_body = "这篇合成原文尚未经过理解，仍可完整阅读。";
    let package = live_submit(&db, "content_detail", unprocessed_external, vec![json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":unprocessed_external},"payload":{"title":"未分析对照（合成）","bodyText":unprocessed_body}})]).await;
    let unprocessed: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(unprocessed_external)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(unprocessed).bind(domain).bind(package).execute(db.pool()).await.unwrap();
    {
        let mut session = ExecutorSession::acquire(&db, "l1-authorized-live-probe", 4)
            .await
            .unwrap();
        let item = claim_next(&db, session.fence()).await.unwrap().unwrap();
        let execution = execute_probe(&db, &item, secrets.as_ref(), &adapter);
        tokio::pin!(execution);
        loop {
            tokio::select! {
                result = &mut execution => { result.unwrap(); break; },
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => { session.heartbeat().await.unwrap(); },
            }
        }
        let callable: bool = sqlx::query_scalar("SELECT state='succeeded' AND result->>'modelCallable'='true' FROM linggan_model_invocation WHERE operation='probe'").fetch_one(db.pool()).await.unwrap();
        assert!(callable, "live probe did not qualify; no note request sent");
        session.request_drain().await.unwrap();
        session.complete_drain().await.unwrap();
    }
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let worker = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "l1-authorized-single-live",
        4,
        secrets,
        adapter,
        drain.clone(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let application = app_with_database(db.clone());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    fs::write(output.join("fixture.json"), serde_json::to_vec_pretty(&json!({"origin":origin,"domain":domain,"work":work,
        "originalWorkRef":sample["workRef"],"unprocessed":unprocessed,"body":body.unwrap_or(""),
        "originalTexts":text.iter().filter(|t| t["kind"] != "title").map(|t| t["text"].clone()).collect::<Vec<_>>(),
        "imageCount":sample["images"].as_array().unwrap().len(),"waitMs":3616000,"live":true,
        "provider":route["modelId"],"sourceIdentity":"isolated reimport, not production UUIDs","realModelQualityVerified":false})).unwrap()).unwrap();
    let status = std::process::Command::new(std::env::var_os("WEB_APP_PROOF_NODE").unwrap())
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../web/test/e2e/content-understanding-real-chain.mjs"),
        )
        .status()
        .unwrap();
    server.abort();
    let _ = server.await;
    drain.request();
    worker.await.unwrap().unwrap();
    let ledger: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(i) FROM linggan_model_invocation i ORDER BY created_at",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    fs::write(
        output.join("invocations.json"),
        serde_json::to_vec_pretty(&ledger).unwrap(),
    )
    .unwrap();
    let result: Option<Value> =
        sqlx::query_scalar("SELECT to_jsonb(r) FROM linggan_content_understanding_result r")
            .fetch_optional(db.pool())
            .await
            .unwrap();
    fs::write(
        output.join("persisted.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    assert!(
        status.success(),
        "see private live report; never automatically resend"
    );
    let report: Value =
        serde_json::from_slice(&fs::read(output.join("report.json")).unwrap()).unwrap();
    assert_eq!(result.unwrap()["result"], report["result"]);
    assert_eq!(
        ledger
            .iter()
            .filter(|r| r["task_key"] == "content_understanding")
            .count(),
        1
    );
    db.pool().close().await;
}

async fn live_submit(
    db: &linggan_storage_postgres::Database,
    kind: &str,
    external: &str,
    records: Vec<Value>,
) -> Uuid {
    use linggan_contracts::{
        parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
    };
    use linggan_evidence::{
        RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, create_producer_task,
        start_producer_attempt, submit_producer_package,
    };
    let task = Uuid::new_v4();
    let producer = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    let package = Uuid::new_v4();
    let spec = parse_producer_task_spec(&json!({"contractVersion":"linggan.producer.task-spec.v2","taskId":task,"source":"manual","platform":"xhs",
        "pageType":"isolated_authorized_material_replay","target":{"contentExternalId":external},"capabilitiesRequested":[kind],"maximumQuota":records.len(),
        "commentLimit":"not_requested","acquireMedia":"not_requested","riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]}).to_string()).unwrap();
    assert!(matches!(
        create_producer_task(db, &spec).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    let started = parse_producer_attempt(&json!({"contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer,"taskId":task,"attemptId":attempt}).to_string()).unwrap();
    assert!(matches!(
        start_producer_attempt(db, &started).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let captured: String =
        sqlx::query_scalar("SELECT (to_jsonb(scope_001_now() AT TIME ZONE 'UTC') #>> '{}') || 'Z'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let submission = parse_producer_submission(&json!({"contractVersion":"linggan.producer.capture-package.v1","producerInstanceId":producer,"taskId":task,"attemptId":attempt,"submissionId":Uuid::new_v4(),
        "capturePackage":{"contractVersion":"linggan.producer.capture-package.v1","packageRef":package,"packageKind":kind,"platform":"xhs","observedAt":captured,"capturedAt":captured,
        "coverage":{"target":{"contentExternalId":external},"layers":[{"capability":kind,"observed":records.len(),"attempted":records.len(),"acquired":records.len(),"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"authorized_local_replay"}]},"records":records}}).to_string()).unwrap();
    assert!(matches!(
        submit_producer_package(db, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));
    package
}
