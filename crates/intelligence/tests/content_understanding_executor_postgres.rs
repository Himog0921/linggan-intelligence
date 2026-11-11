//! Formal L1 start → queue → real Node SDK → settlement → current DB read.
//! Only synthetic material, declared synthetic high capability and a caller-owned proof database.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use dossier::fixture;
#[path = "support/content_understanding_fixture.rs"]
mod l1_setup;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[path = "support/content_understanding_endpoint.rs"]
mod wire;

use base64::Engine as _;
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        capability::ModelLimits,
        content_understanding as owner,
        control::BudgetPolicy,
        control_store::ControlScope,
        note_analysis as legacy,
        provider_store::MaterialClass,
        skill_binding::{SwitchSkill, switch_skill},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
        store::FoundationError,
        task_config::{MaterialColumn, ModelSelection, SaveTaskConfig, save_task_config},
    },
    content_understanding::METHOD,
    note_analysis::Scope,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

const BODY: &str = "合成说明🙂：先摆出材料，再按步骤核对；只用于流程测试。";
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

#[tokio::test]
#[ignore = "owned PostgreSQL + AI_FOUNDATION_PROOF_NODE + owned LINGGAN_LOCAL_MEDIA_ROOT; no real models"]
async fn l1_round_trip_reuses_current_core_and_preserves_legacy_analysis() {
    // The fixture reads LOCAL_001_PROOF_DATABASE_URL itself. Never discover or print a DSN.
    let db = fixture::proof_database(&format!("content_l1_{}", Uuid::new_v4().simple())).await;
    let media_root = PathBuf::from(
        std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
            .expect("supply a dedicated, owned proof media directory"),
    );
    assert!(
        media_root.is_absolute(),
        "proof media directory must be absolute"
    );
    let endpoint = wire::Endpoint::start().await;
    let adapter = PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE")
            .expect("supply the Node executable")
            .into(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut session = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "content-l1-synthetic-probe",
        4,
    )
    .await
    .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &session, &adapter).await;
    configure_legacy(&db, &adapter, &model).await;
    let legacy_config_before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM linggan_ai_task_setting s WHERE task_key='note_interpretation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let l1_config = l1_setup::configure_high(&db, &model).await;
    session.request_drain().await.unwrap();
    session.complete_drain().await.unwrap();
    drop(session);

    let package = submit_body(&db, BODY).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='dossier-note'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    let missing_inventory = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(
        missing_inventory["canStart"], false,
        "a title and body cannot stand in for an unconfirmed media inventory"
    );
    assert!(matches!(
        owner::start(&db, &start_from(&missing_inventory, work)).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    // Acquire through media-slot submission and admit_media_blob, not a forged material projection
    // or OCR result. No image reading is preseeded: the production path must send these bytes.
    let image_bytes = base64::engine::general_purpose::STANDARD
        .decode(PNG)
        .unwrap();
    let (image_work, image_source, image_sha) =
        dossier::acquired_image_source_with_bytes(&db, Some(image_bytes.clone())).await;
    assert_eq!(image_work, work);
    let image_ready = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(image_ready["materialReadiness"]["kind"], "image_text");
    assert_eq!(image_ready["materialReadiness"]["state"], "ready");
    assert_eq!(image_ready["canStart"], true, "static OCR is not required");
    let image_path = media_root.join(format!("blobs/{image_sha}.png"));
    let held_path = image_path.with_extension("held");
    std::fs::rename(&image_path, &held_path).unwrap();
    let unreadable = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(unreadable["canStart"], false);
    assert_eq!(
        unreadable["materialReadiness"]["gaps"][0]["code"],
        "original_unavailable"
    );
    assert!(matches!(
        owner::start(&db, &start_from(&image_ready, work)).await,
        Err(FoundationError::Conflict | FoundationError::AdmissionDenied(_))
    ));
    assert!(matches!(
        owner::start(&db, &start_from(&unreadable, work)).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    std::fs::rename(held_path, &image_path).unwrap();
    saved_video_and_cover(&db, work, &image_bytes, &image_sha).await;
    let canonical = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let timed = canonical
        .fragments
        .iter()
        .find(|f| f.field == "transcript")
        .unwrap();
    assert_eq!(timed.speaker, "other");
    assert_eq!(
        timed.structure["time"],
        json!({"startMs":1000,"endMs":3000})
    );
    assert!(!timed.issues.is_empty());
    let canonical_image = canonical
        .media_inventory
        .iter()
        .find(|s| s["component"] == "image")
        .unwrap();
    assert!(
        canonical_image["sameBytesSources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["slotKey"] == "xhs:dossier-note:cover:1")
    );

    let video_ready = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(video_ready["materialReadiness"]["kind"], "video");
    assert_eq!(video_ready["materialReadiness"]["state"], "ready");
    assert!(
        video_ready["materialReadiness"]["scopeNote"]
            .as_str()
            .unwrap()
            .contains("未覆盖")
    );
    let stopped = owner::start(&db, &start_from(&video_ready, work))
        .await
        .unwrap();
    let video_sha = canonical
        .media_inventory
        .iter()
        .find(|s| s["purpose"] == "video")
        .unwrap()["blobSha256"]
        .as_str()
        .unwrap();
    let video_path = media_root.join(format!("blobs/{video_sha}.mp4"));
    let held_video = video_path.with_extension("held");
    std::fs::rename(&video_path, &held_video).unwrap();
    let stop_drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let stop_worker = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "content-l1-missing-video-proof",
        4,
        Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter.clone(),
        stop_drain.clone(),
    ));
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let current = owner::latest(&db, dossier::DOMAIN, work, None)
                .await
                .unwrap();
            if current["analysis"]["state"] == "skipped" {
                assert_eq!(
                    current["analysis"]["requestRef"],
                    json!(stopped.request_ref)
                );
                assert_eq!(current["analysis"]["failureCode"], "ai_source_unavailable");
                break;
            }
            assert!(!stop_worker.is_finished());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("missing video must stop before dispatch, not defer forever");
    stop_drain.request();
    stop_worker.await.unwrap().unwrap();
    let dispatched: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE task_key='content_understanding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(dispatched, 0);
    std::fs::rename(held_video, &video_path).unwrap();

    let old_preview = legacy::preview(&db, dossier::DOMAIN, work, Scope::Note)
        .await
        .unwrap();
    assert_eq!(old_preview["canStart"], true, "{}", old_preview["reasons"]);
    let old = legacy::start(
        &db,
        &legacy::Start {
            command_ref: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            work_ref: work,
            scope: Scope::Note,
            knowledge_cutoff: old_preview["knowledgeCutoff"].as_str().unwrap().into(),
            input_hash: old_preview["inputHash"].as_str().unwrap().into(),
            task_config_ref: serde_json::from_value(old_preview["taskConfigRef"].clone()).unwrap(),
            task_revision: old_preview["taskRevision"].as_i64().unwrap(),
        },
    )
    .await
    .unwrap();

    let preview = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(preview["canStart"], true, "{}", preview["reasons"]);
    assert_eq!(preview["methodVersion"], METHOD);
    assert_eq!(preview["imageCount"], 1);
    assert_eq!(preview["taskConfigRef"], json!(l1_config));
    let request = start_from(&preview, work);
    let receipt = owner::start(&db, &request).await.unwrap();
    assert!(!receipt.reused);
    assert_eq!(
        owner::start(&db, &request).await.unwrap().request_ref,
        receipt.request_ref
    );
    let pending_reuse = owner::start(&db, &start_from(&preview, work))
        .await
        .unwrap();
    assert!(pending_reuse.reused);
    assert_eq!(pending_reuse.request_ref, receipt.request_ref);
    assert_eq!(pending_reuse.item_ref, receipt.item_ref);
    assert_eq!(pending_reuse.batch_ref, receipt.batch_ref);

    // Authorization must be checked even when a globally reusable Core already exists.
    let foreign_domain = Uuid::new_v4();
    assert!(matches!(
        owner::preview(&db, foreign_domain, work).await,
        Err(FoundationError::NotFound)
    ));
    assert!(matches!(
        owner::latest(&db, foreign_domain, work, None).await,
        Err(FoundationError::NotFound)
    ));
    let mut forbidden = start_from(&preview, work);
    forbidden.domain_ref = foreign_domain;
    assert!(matches!(
        owner::start(&db, &forbidden).await,
        Err(FoundationError::NotFound)
    ));

    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let worker = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "content-l1-owned-proof",
        4,
        Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    let (saved, old_saved) = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let current = owner::latest(&db, dossier::DOMAIN, work, None)
                .await
                .unwrap();
            let old_current = legacy::read(&db, dossier::DOMAIN, old.request_ref)
                .await
                .unwrap();
            for run in [&current["analysis"], &old_current["run"]] {
                assert!(
                    ![json!("failed"), json!("skipped")].contains(&run["state"]),
                    "run failed: {run}"
                );
            }
            if current["analysis"]["state"] == "succeeded"
                && old_current["run"]["state"] == "succeeded"
            {
                break (current, old_current);
            }
            assert!(
                !worker.is_finished(),
                "executor exited before settling both tasks"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("both formal tasks settle");
    drain.request();
    worker.await.unwrap().unwrap();

    let analysis = &saved["analysis"];
    assert_eq!(analysis["requestRef"], json!(receipt.request_ref));
    assert_eq!(analysis["failureCode"], Value::Null);
    let result = &analysis["result"];
    let card = &result["card"];
    assert_eq!(card["schemaVersion"], "content-card.v0.2");
    assert_eq!(card["workRef"], json!(work));
    assert_eq!(card["analysisRef"], json!(receipt.request_ref));
    assert_eq!(card["deep"], Value::Null);
    assert_eq!(
        card["core"]["mainTopicUnitId"], "u2",
        "explicit second discussion survives"
    );
    assert_eq!(card["core"]["provisionUnitIds"], json!(["u3"]));
    assert_eq!(card["generation"]["methodVersion"], METHOD);
    assert_eq!(
        card["generation"]["providerReturnedModel"],
        "synthetic-writing"
    );
    assert_eq!(
        result["imageContributions"][0]["status"],
        "no_new_information"
    );
    assert_eq!(card["evidence"].as_array().unwrap().len(), 2);
    assert_eq!(card["evidence"][0]["quote"], BODY);
    assert_eq!(card["evidence"][1]["sourceRef"], json!(image_source));
    assert_eq!(card["evidence"][1]["blobSha256"], image_sha);
    assert_eq!(card["evidence"][1]["verification"], "model_read");
    assert!(card["evidence"][1].get("start").is_none());

    let (persisted, raw, invocation): (Value, String, Uuid) = sqlx::query_as(
        "SELECT result,raw_output,invocation_ref FROM linggan_content_understanding_result WHERE request_ref=$1 AND failure_code IS NULL",
    ).bind(receipt.request_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(&persisted, result);
    assert_eq!(
        owner::latest(&db, dossier::DOMAIN, work, None)
            .await
            .unwrap(),
        saved
    );
    assert_eq!(
        analysis
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "batchRef",
            "failureCode",
            "itemRef",
            "requestRef",
            "result",
            "state"
        ]
    );
    assert!(!saved.to_string().contains("raw_output") && !saved.to_string().contains("rawOutput"));
    assert!(result.get("evidenceCandidates").is_none());
    let frozen: Value = sqlx::query_scalar(
        "SELECT input_manifest FROM linggan_content_understanding_request WHERE request_ref=$1",
    )
    .bind(receipt.request_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        card["sourceSnapshot"]["materialSnapshotIdentity"],
        frozen["snapshot"]["materialSnapshotIdentity"]
    );
    assert_eq!(
        card["sourceSnapshot"]["materialAsOf"],
        frozen["snapshot"]["materialAsOf"]
    );
    for fragment in &canonical.fragments {
        let source = frozen["snapshot"]["texts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["citationId"] == fragment.fragment_id)
            .unwrap();
        assert_eq!(source["speaker"], fragment.speaker);
        assert_eq!(source["issues"], json!(fragment.issues));
        let mut structure = fragment.structure.clone();
        structure.as_object_mut().unwrap().remove("text");
        assert_eq!(source["structure"], structure);
        assert_eq!(source["text"], fragment.text);
    }
    assert_eq!(
        frozen["inventory"][0]["sameBytesSources"],
        canonical_image["sameBytesSources"]
    );
    let text_source = frozen["snapshot"]["texts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "body")
        .unwrap();
    assert_eq!(
        card["evidence"][0]["inputCitationId"],
        text_source["citationId"]
    );
    assert_eq!(card["evidence"][0]["sourceRef"], text_source["sourceRef"]);
    assert_eq!(
        card["evidence"][0]["sourceVersion"],
        text_source["sourceVersion"]
    );
    assert_eq!(card["evidence"][0]["start"], text_source["start"]);
    assert_eq!(
        card["evidence"][0]["end"].as_u64().unwrap(),
        text_source["start"].as_u64().unwrap() + BODY.chars().count() as u64
    );
    let linked: (String, String, Uuid, Uuid) = sqlx::query_as(
        "SELECT i.state,i.task_key,i.task_config_ref,i.batch_ref FROM linggan_model_invocation i WHERE invocation_ref=$1",
    ).bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        linked,
        (
            "succeeded".into(),
            "content_understanding".into(),
            l1_config,
            receipt.batch_ref
        )
    );

    let current_preview = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    let success_reuse = owner::start(&db, &start_from(&current_preview, work))
        .await
        .unwrap();
    assert!(success_reuse.reused);
    assert_eq!(success_reuse.request_ref, receipt.request_ref);
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM linggan_content_understanding_request),(SELECT count(*) FROM linggan_model_invocation WHERE task_key='content_understanding')",
    ).fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (2, 1), "one pre-dispatch stop and one saved Core");
    let legacy_config_after: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM linggan_ai_task_setting s WHERE task_key='note_interpretation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(legacy_config_after, legacy_config_before);
    assert_eq!(
        legacy::read(&db, dossier::DOMAIN, old.request_ref)
            .await
            .unwrap()["result"],
        old_saved["result"]
    );
    assert_eq!(
        legacy::latest(&db, dossier::DOMAIN, work, Scope::Note)
            .await
            .unwrap()["latestSuccessful"]["requestRef"],
        json!(old.request_ref)
    );

    // Only an admitted new content_detail package changes the body; old immutable results stay put.
    submit_body(
        &db,
        "更新后的合成说明🙂：先核对顺序，再摆出材料；不是旧正文。",
    )
    .await;
    let changed = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_ne!(changed["inputHash"], preview["inputHash"]);
    assert!(
        owner::latest(&db, dossier::DOMAIN, work, None)
            .await
            .unwrap()["analysis"]
            .is_null(),
        "a Core for previous material must not appear as current"
    );
    assert_eq!(
        owner::latest(&db, dossier::DOMAIN, work, Some(&canonical.as_of))
            .await
            .unwrap(),
        saved,
        "the displayed old dossier must keep its matching Core, even if saved after its cutoff"
    );
    assert!(matches!(
        owner::latest(&db, foreign_domain, work, Some(&canonical.as_of)).await,
        Err(FoundationError::NotFound)
    ));
    assert!(
        owner::latest(
            &db,
            dossier::DOMAIN,
            work,
            Some(changed["knowledgeCutoff"].as_str().unwrap())
        )
        .await
        .unwrap()["analysis"]
            .is_null(),
        "the new dossier cutoff cannot receive the old Core"
    );
    assert!(matches!(
        owner::start(&db, &start_from(&preview, work)).await,
        Err(FoundationError::Conflict)
    ));
    let preserved: Value = sqlx::query_scalar(
        "SELECT result FROM linggan_content_understanding_result WHERE request_ref=$1",
    )
    .bind(receipt.request_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(preserved, persisted);
    let old_after_change = legacy::read(&db, dossier::DOMAIN, old.request_ref)
        .await
        .unwrap();
    assert_eq!(old_after_change["result"], old_saved["result"]);
    assert_eq!(old_after_change["freshness"], "source_changed");

    let requests = endpoint.requests.lock().unwrap().clone();
    let l1_calls: Vec<_> = requests
        .iter()
        .filter_map(|request| {
            let packet: Value = serde_json::from_str(&wire::text(request, "user")).ok()?;
            packet["material"]["texts"]
                .is_array()
                .then_some((request, packet))
        })
        .collect();
    assert_eq!(
        l1_calls.len(),
        1,
        "pending and completed duplicates make no extra model call"
    );
    assert_eq!(
        requests.len(),
        3,
        "one real probe, one legacy call, one L1 call"
    );
    let (request, packet) = &l1_calls[0];
    assert_eq!(request["reasoning_effort"], "high");
    assert_eq!(request["response_format"]["type"], "json_object");
    assert_eq!(
        packet["resultSchema"],
        linggan_intelligence::content_understanding::response_schema()
    );
    let raw_draft: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        raw_draft,
        wire::draft(packet),
        "retain the unassembled draft only in the internal result row"
    );
    assert_eq!(
        raw_draft["evidenceCandidates"]["text"][1]["quote"],
        format!("{BODY}”")
    );
    assert!(
        card["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .all(|evidence| evidence["id"] != "unusedText")
    );
    let images: Vec<_> = request["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .flat_map(|m| m["content"].as_array().unwrap())
        .filter(|part| part["type"] == "image_url")
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(
        images[0]["image_url"]["url"],
        format!("data:image/png;base64,{PNG}")
    );
    assert_eq!(
        packet["material"]["images"][0]["sourceRef"],
        json!(image_source)
    );
    assert_eq!(packet["imageInventory"][0]["blobSha256"], image_sha);
    assert_eq!(packet["material"], frozen["snapshot"]);
    assert_eq!(packet["imageInventory"], frozen["inventory"]);
    endpoint.finish().await;
}

fn start_from(preview: &Value, work: Uuid) -> owner::Start {
    owner::Start {
        command_ref: Uuid::new_v4(),
        domain_ref: dossier::DOMAIN,
        work_ref: work,
        knowledge_cutoff: preview["knowledgeCutoff"].as_str().unwrap().into(),
        input_hash: preview["inputHash"].as_str().unwrap().into(),
        task_config_ref: serde_json::from_value(preview["taskConfigRef"].clone()).unwrap(),
        task_revision: preview["taskRevision"].as_i64().unwrap(),
    }
}

async fn submit_body(db: &Database, body: &str) -> Uuid {
    fixture::submit_package(db, "content_detail", json!({"contentExternalId":"dossier-note"}), json!({
        "kind":"content_detail", "sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},
        "payload":{"title":"合成内容理解流程","bodyText":body,"authorId":"synthetic-author"}
    })).await
}

async fn saved_video_and_cover(db: &Database, work: Uuid, image: &[u8], image_sha: &str) {
    use linggan_evidence::{admit_media_blob, material_reading};
    use sha2::{Digest, Sha256};
    let observations = [Uuid::new_v4(), Uuid::new_v4()];
    let records = ["cover", "video"].iter().zip(observations).map(|(role, observation)| {
        let uri = format!("https://media.example/dossier-{role}");
        json!({"kind":"media_slot","slotKey":format!("xhs:dossier-note:{role}:1"),"observationRef":observation,
            "slot":{"role":role,"ordinal":1},
            "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-14T00:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"}})
    }).collect();
    fixture::submit_custom_package(db,"xhs",&["media_slots"],json!({"contentExternalId":"dossier-note"}),"media_slots","xhs",
        json!({"target":{"contentExternalId":"dossier-note"},"layers":[fixture::coverage_layer("media_slots",2)]}),records).await;
    admit_media_blob(
        db,
        observations[0],
        image_sha,
        "image/png",
        image.len() as i64,
        &format!("blobs/{image_sha}.png"),
    )
    .await
    .unwrap();
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=16x16:r=1",
            "-t",
            "1",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "frag_keyframe+empty_moov",
            "-f",
            "mp4",
            "pipe:1",
        ])
        .output()
        .expect("synthetic video requires local ffmpeg");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let video = output.stdout;
    let sha: String = Sha256::digest(&video)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let root = PathBuf::from(std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT").unwrap());
    std::fs::write(root.join(format!("blobs/{sha}.mp4")), &video).unwrap();
    let source = admit_media_blob(
        db,
        observations[1],
        &sha,
        "video/mp4",
        video.len() as i64,
        &format!("blobs/{sha}.mp4"),
    )
    .await
    .unwrap()
    .materialization_ref;
    let mut tx = db.pool().begin().await.unwrap();
    material_reading::publish_in(&mut tx, material_reading::NewReading {
        content_public_ref:work, unit_key:"audio:xhs:dossier-note:video:1".into(), kind:"audio".into(),
        slot_key:Some("xhs:dossier-note:video:1".into()), page:None, source_version:source.to_string(),
        source_state:"present".into(), derivative_ref:None, producer_ref:Some(Uuid::new_v4()),
        generated_by:"asr".into(), rule_version:"synthetic-saved-video-replay.v1".into(),
        blocks:vec![json!({"ref":"timed","kind":"paragraph","text":"第三方口播：先说明条件。","speaker":"other",
            "time":{"startMs":1000,"endMs":3000},"issues":[{"key":"word","code":"uncertain_reading","message":"一个词尚未核对","resolution":"reading_edit"}]})],
        issues:vec![], source_location:json!({"synthetic":true}),
        publication:material_reading::Publication::Automatic { expected_current:None },
    }).await.unwrap();
    material_reading::publish_in(
        &mut tx,
        material_reading::NewReading {
            content_public_ref: work,
            unit_key: "video_frame:xhs:dossier-note:video:1".into(),
            kind: "video_frame".into(),
            slot_key: Some("xhs:dossier-note:video:1".into()),
            page: None,
            source_version: source.to_string(),
            source_state: "known_empty".into(),
            derivative_ref: None,
            producer_ref: Some(Uuid::new_v4()),
            generated_by: "frame_ocr".into(),
            rule_version: "synthetic-saved-video-replay.v1".into(),
            blocks: vec![json!({"kind":"gap","text":"合成视频未检出画面文字"})],
            issues: vec![],
            source_location: json!({"synthetic":true}),
            publication: material_reading::Publication::Automatic {
                expected_current: None,
            },
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn configure_legacy(db: &Database, adapter: &PiAdapter, model: &setup::Configured) {
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='note_interpretation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: model.capability,
                limits: ModelLimits {
                    input_tokens: 31768,
                    ..model.limits.clone()
                },
            }),
            skill_version_ref: None,
            material_columns: [MaterialColumn::Title, MaterialColumn::Body]
                .into_iter()
                .collect(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/data-contracts/ai-midterm-quality-002/skill/linggan-note-analysis");
    let skill = import_skill(
        db,
        adapter,
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: None,
            package: SkillPackage {
                directory: "linggan-note-analysis".into(),
                files: ["SKILL.md", "references/output-guide.md"]
                    .into_iter()
                    .map(|path| UploadFile {
                        path: path.into(),
                        encoding: Encoding::Utf8,
                        content: std::fs::read_to_string(root.join(path)).unwrap(),
                    })
                    .collect(),
            },
        },
    )
    .await
    .unwrap();
    switch_skill(
        db,
        &SwitchSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_task_revision: config.revision,
            expected_binding_revision: 0,
            version_ref: skill.version_ref,
        },
    )
    .await
    .unwrap();
    setup::enable(db, ControlScope::Task(TaskKey::NoteInterpretation)).await;
    setup::allow(
        db,
        model.endpoint,
        TaskKey::NoteInterpretation,
        MaterialClass::NoteMaterial,
    )
    .await;
}
