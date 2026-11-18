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
        TaskKey, content_understanding as owner, store::FoundationError,
    },
    content_understanding::METHOD,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc, time::Duration};
use uuid::Uuid;

const BODY: &str = "合成说明🙂：先摆出材料，再按步骤核对；只用于流程测试。";
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

/// The L2 migrations must stay identical across the file, the runtime applier and
/// the API full-schema fixture ledger; a hand-copied digest drifts silently.
#[test]
fn full_schema_fixtures_record_the_actual_deep_v2_migration_digest() {
    let fixture = include_str!("../../../apps/api/src/local_web/full_schema_fixture.rs");
    let runtime = include_str!("../../../scripts/local-runtime.sh");
    for (id, sql) in [
        (
            "0224_content_understanding_deep_v2",
            include_str!("../../../database/migrations/0224_content_understanding_deep_v2.sql"),
        ),
        (
            "0225_content_understanding_deep_outcome",
            include_str!(
                "../../../database/migrations/0225_content_understanding_deep_outcome.sql"
            ),
        ),
    ] {
        let digest: String = Sha256::digest(sql.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert!(
            fixture.contains(&format!("('{id}','{digest}')")),
            "api fixture migration ledger differs from the actual {id} SQL"
        );
        assert!(
            fixture.contains(&format!("database/migrations/{id}.sql")),
            "api fixture does not load the {id} migration"
        );
        assert!(
            runtime.contains(&format!("apply_migration_once \"{id}\"")),
            "local runtime does not apply the {id} migration"
        );
    }
}

#[tokio::test]
#[ignore = "owned PostgreSQL + AI_FOUNDATION_PROOF_NODE + owned LINGGAN_LOCAL_MEDIA_ROOT; no real models"]
async fn l1_round_trip_reuses_current_core_and_preserves_retired_task_config() {
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

    let no_slot_package = fixture::submit_package(&db, "content_detail", json!({"contentExternalId":"video-no-slot"}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"video-no-slot"},
            "payload":{"title":"合成视频","bodyText":BODY,"type":"video"}})).await;
    let no_slot_work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='video-no-slot'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(no_slot_work).bind(dossier::DOMAIN).bind(no_slot_package).execute(db.pool()).await.unwrap();
    let no_slot_preview = owner::preview(&db, dossier::DOMAIN, no_slot_work)
        .await
        .unwrap();
    assert_eq!(no_slot_preview["materialReadiness"]["kind"], "video");
    assert_eq!(no_slot_preview["canStart"], false);
    assert!(
        no_slot_preview["materialReadiness"]["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gap| gap["code"] == "video_transcript_required")
    );

    // An incomplete later capture can default to normal; it must not erase an
    // earlier explicit video fact for the same platform work.
    fixture::submit_package(&db, "content_detail", json!({"contentExternalId":"video-no-slot"}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"video-no-slot"},
            "payload":{"title":"合成视频新版本","bodyText":BODY,"type":"normal"}})).await;
    let recaptured = owner::preview(&db, dossier::DOMAIN, no_slot_work)
        .await
        .unwrap();
    assert_eq!(recaptured["materialReadiness"]["kind"], "video");
    assert_eq!(recaptured["canStart"], false);

    for (external, media) in [
        (
            "video-url-no-slot",
            json!({"type":"normal","video":"https://media.example/video.mp4"}),
        ),
        (
            "video-stream-no-slot",
            json!({"type":"normal","videoStreams":[{"url":"https://media.example/stream.mp4"}]}),
        ),
    ] {
        let mut payload = json!({"title":"合成视频","bodyText":BODY});
        payload
            .as_object_mut()
            .unwrap()
            .extend(media.as_object().unwrap().clone());
        let package = fixture::submit_package(&db, "content_detail", json!({"contentExternalId":external}),
            json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":payload})).await;
        let source_work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(external)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
            .bind(Uuid::new_v4()).bind(source_work).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        let preview = owner::preview(&db, dossier::DOMAIN, source_work)
            .await
            .unwrap();
        assert_eq!(preview["materialReadiness"]["kind"], "video", "{external}");
        assert_eq!(preview["canStart"], false, "{external}");
    }

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
        missing_inventory["canStart"], true,
        "available body is enough without proving a complete media inventory"
    );
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
    assert_eq!(
        unreadable["canStart"], true,
        "a missing image does not block an available body"
    );
    assert_eq!(
        unreadable["imageCount"], 0,
        "unreadable image is not sent to the model"
    );
    assert!(matches!(
        owner::start(&db, &start_from(&image_ready, work)).await,
        Err(FoundationError::Conflict | FoundationError::AdmissionDenied(_))
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
            .contains("不包含视频画面")
    );
    let stopped = owner::start(&db, &start_from(&video_ready, work))
        .await
        .unwrap();
    // The original video is not in the frozen model input. Its loss must not
    // make the queued task disappear from the current status.
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
    let during = owner::latest(&db, dossier::DOMAIN, work, None)
        .await
        .unwrap();
    assert_eq!(during["currentRequest"]["state"], "pending");
    std::fs::rename(&held_video, &video_path).unwrap();
    // Removing a frozen image prevents sending bytes that the model input used.
    let held_image = image_path.with_extension("held");
    std::fs::rename(&image_path, &held_image).unwrap();
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
            let (state, failure): (String, Option<String>) = sqlx::query_as(
                "SELECT state,failure_code FROM linggan_ai_batch_item_control WHERE item_ref=$1",
            )
            .bind(stopped.item_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
            if state == "skipped" {
                assert_eq!(failure.as_deref(), Some("ai_source_unavailable"));
                break;
            }
            assert!(!stop_worker.is_finished());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("missing frozen image must stop before dispatch, not defer forever");
    stop_drain.request();
    stop_worker.await.unwrap().unwrap();
    let dispatched: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE task_key='content_understanding'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(dispatched, 0);
    std::fs::rename(held_image, &image_path).unwrap();

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
        adapter.clone(),
        drain.clone(),
    ));
    let saved = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let current = owner::latest(&db, dossier::DOMAIN, work, None)
                .await
                .unwrap();
            let run = &current["analysis"];
            assert!(
                ![json!("failed"), json!("skipped")].contains(&run["state"]),
                "run failed: {run}"
            );
            if run["state"] == "succeeded" {
                break current;
            }
            assert!(!worker.is_finished(), "executor exited before settling L1");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("L1 task settles");
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
            "deepAnalyses",
            "failureCode",
            "itemRef",
            "requestRef",
            "result",
            "sourceChanged",
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

    // This synthetic provider intentionally omits optional imageReadings.
    // A usable Core and its raw model receipt must still be preserved.
    let image_outcome: Value = sqlx::query_scalar(
        "SELECT result->'imageReadings' FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(image_outcome["state"], "invalid");
    assert_eq!(image_outcome["code"], "l1_image_readings_invalid");
    let stored_raw: Option<String> = sqlx::query_scalar(
        "SELECT raw_output FROM linggan_content_understanding_result WHERE request_ref=$1",
    )
    .bind(receipt.request_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(stored_raw.is_some());

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
    // Only an admitted new content_detail package changes the body; old immutable results stay put.
    submit_body(
        &db,
        "更新后的合成说明🙂：先核对顺序，再摆出材料；不是旧正文。",
    )
    .await;
    let changed = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_ne!(changed["inputHash"], preview["inputHash"]);
    let previous = owner::latest(&db, dossier::DOMAIN, work, None)
        .await
        .unwrap();
    assert_eq!(
        previous["analysis"]["requestRef"],
        json!(receipt.request_ref)
    );
    assert_eq!(previous["analysis"]["sourceChanged"], true);
    assert_eq!(previous["analysis"]["result"], saved["analysis"]["result"]);
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
    let old_at_new_cutoff = owner::latest(
        &db,
        dossier::DOMAIN,
        work,
        Some(changed["knowledgeCutoff"].as_str().unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(
        old_at_new_cutoff["analysis"]["requestRef"],
        json!(receipt.request_ref)
    );
    assert_eq!(old_at_new_cutoff["analysis"]["sourceChanged"], true);
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
    assert_eq!(requests.len(), 2, "one synthetic probe and one L1 call");
    let (request, packet) = &l1_calls[0];
    assert_eq!(
        packet["draftProtocol"],
        linggan_intelligence::content_understanding::model_draft::PROTOCOL
    );
    for (i, source) in packet["material"]["texts"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(source["citationId"], format!("s{}", i + 1));
    }
    assert_eq!(request["reasoning_effort"], "high");
    assert_eq!(request["response_format"]["type"], "json_object");
    let schema = &packet["resultSchema"];
    let baseline = linggan_intelligence::content_understanding::response_schema();
    assert_eq!(schema["properties"]["core"], baseline["properties"]["core"]);
    assert_eq!(
        schema["properties"]["evidenceCandidates"],
        baseline["properties"]["evidenceCandidates"]
    );
    let image_count = packet["material"]["images"].as_array().unwrap().len();
    let max_chars =
        schema["properties"]["imageReadings"]["items"]["properties"]["text"]["maxLength"]
            .as_u64()
            .unwrap() as usize;
    assert!(image_count > 0 && max_chars * image_count <= 4096 / 3);
    assert!(schema["description"].as_str().unwrap().contains("deferred"));
    assert!(serde_json::from_str::<Value>(&raw).is_err());
    let repair: String = sqlx::query_scalar(
        "SELECT result->>'replyRepair' FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(repair, "removed_one_trailing_brace");
    let raw_draft: Value = serde_json::from_str(raw.strip_suffix('}').unwrap()).unwrap();
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
            .all(|evidence| evidence["id"] != "ev_unused")
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
    let mut restored_material = packet["material"].clone();
    for (index, source) in restored_material["texts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        source["citationId"] = frozen["snapshot"]["texts"][index]["citationId"].clone();
        assert!(source["structure"].get("ref").is_none());
        if let Some(reference) = frozen["snapshot"]["texts"][index]["structure"].get("ref") {
            source["structure"]["ref"] = reference.clone();
        }
    }
    assert_eq!(restored_material, frozen["snapshot"]);
    assert_eq!(packet["imageInventory"], frozen["inventory"]);
    let recovery = seed_recoverable_failure(&db, work).await;
    let before = owner::latest(&db, dossier::DOMAIN, work, None)
        .await
        .unwrap();
    assert_eq!(before["analysis"]["state"], "succeeded");
    assert_eq!(before["currentRequest"]["state"], "failed");
    let count_sql = "SELECT count(*) FROM linggan_model_invocation";
    let calls: i64 = sqlx::query_scalar(count_sql)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let revision = "b".repeat(40);
    let ids = [recovery];
    assert!(
        owner::recover_saved_replies(&db, &ids, &revision, false)
            .await
            .is_ok()
    );
    assert_eq!(
        owner::latest(&db, dossier::DOMAIN, work, None)
            .await
            .unwrap(),
        before
    );
    assert!(
        owner::recover_saved_replies(&db, &[recovery, Uuid::nil()], &revision, true)
            .await
            .is_err()
    );
    assert_eq!(
        owner::latest(&db, dossier::DOMAIN, work, None)
            .await
            .unwrap(),
        before
    );
    let repaired = owner::recover_saved_replies(&db, &ids, &revision, true)
        .await
        .unwrap();
    assert!(!repaired[0].already_saved);
    let after = owner::latest(&db, dossier::DOMAIN, work, None)
        .await
        .unwrap();
    assert_eq!(after["analysis"]["state"], "succeeded");
    assert_eq!(after["analysis"]["recoveredFromSavedReply"], true);
    assert_eq!(
        after["analysis"]["result"]["card"]["analysisRef"],
        json!(recovery)
    );
    assert_eq!(
        after["analysis"]["result"]["card"]["evidence"][0]["quote"],
        "先核对顺序，再摆出材料"
    );
    assert!(
        owner::recover_saved_replies(&db, &ids, &revision, true)
            .await
            .unwrap()[0]
            .already_saved
    );
    let reply_time: String = sqlx::query_scalar(
        "SELECT to_char(created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')
         FROM linggan_content_understanding_result WHERE request_ref=$1",
    )
    .bind(recovery)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        after["analysis"]["result"]["card"]["generation"]["createdAt"],
        reply_time
    );
    let original: (String, String, String) = sqlx::query_as(
        "SELECT b.state, i.state, o.failure_code FROM linggan_content_understanding_result o
         JOIN linggan_content_understanding_request r USING(request_ref)
         JOIN linggan_ai_batch_item_control b USING(item_ref)
         JOIN linggan_model_invocation i ON i.invocation_ref=o.invocation_ref WHERE r.request_ref=$1",
    ).bind(recovery).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        original,
        ("failed".into(), "failed".into(), "ai_output_invalid".into())
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(count_sql)
            .fetch_one(db.pool())
            .await
            .unwrap(),
        calls
    );
    prove_billing_pause(&db, work, &model, &endpoint, adapter).await;
    // A newly observed video without a transcript blocks new L1, but must not
    // make a previously saved result disappear.
    let uri = "https://media.example/synthetic-second-video";
    fixture::submit_package(&db, "media_slots", json!({"contentExternalId":"dossier-note"}),
        json!({"kind":"media_slot","slotKey":"xhs:dossier-note:video:2","observationRef":Uuid::new_v4(),
            "slot":{"role":"video","ordinal":2},
            "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-14T00:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"}})).await;
    let after_new_video = owner::latest(&db, dossier::DOMAIN, work, None)
        .await
        .unwrap();
    assert!(after_new_video["analysis"]["result"].is_object());
    assert_eq!(after_new_video["materialReadiness"]["state"], "blocked");
    let blocked = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(blocked["canStart"], false);
    // A later withdrawal must hide results based on those bytes, including
    // results recovered from an older material snapshot.
    linggan_evidence::record_blob_disposition(
        &db,
        &image_sha,
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "synthetic withdrawal",
    )
    .await
    .unwrap();
    let after_withdrawal = owner::latest(&db, dossier::DOMAIN, work, None)
        .await
        .unwrap();
    assert!(after_withdrawal["analysis"]["result"].is_null());
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned PostgreSQL + synthetic Node endpoint only"]
async fn optional_image_reading_failure_keeps_l1_and_invocation_receipt() {
    for scenario in ["new", "write_failure", "existing"] {
        let reject_reading = scenario == "write_failure";
        let db = fixture::proof_database(&format!("l1_img_{}", Uuid::new_v4().simple())).await;
        let endpoint = wire::Endpoint::start().await;
        endpoint
            .include_image_readings
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let adapter = PiAdapter::configured_with_test_command(
            std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
        );
        let mut session = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
            &db,
            "image-reading-synthetic-probe",
            4,
        )
        .await
        .unwrap();
        let model = setup::prepare_with_actual_probe(&db, &endpoint.url, &session, &adapter).await;
        l1_setup::configure_high(&db, &model).await;
        session.request_drain().await.unwrap();
        session.complete_drain().await.unwrap();
        drop(session);
        let package = submit_body(&db, BODY).await;
        let work: Uuid = sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id='dossier-note'").fetch_one(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
            .bind(Uuid::new_v4()).bind(work).bind(dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        let image = base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap();
        if scenario == "existing" {
            dossier::current_image_reading_with_bytes(&db, Some(image)).await;
        } else {
            dossier::acquired_image_source_with_bytes(&db, Some(image)).await;
        }
        if reject_reading {
            sqlx::query("CREATE FUNCTION reject_synthetic_l1_reading() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.rule_version='content-understanding.image-reading.v1' THEN RAISE EXCEPTION 'synthetic reading failure'; END IF; RETURN NEW; END $$")
                .execute(db.pool()).await.unwrap();
            sqlx::query("CREATE TRIGGER reject_synthetic_l1_reading BEFORE INSERT ON linggan_material_reading FOR EACH ROW EXECUTE FUNCTION reject_synthetic_l1_reading()")
                .execute(db.pool()).await.unwrap();
        }
        let preview = owner::preview(&db, dossier::DOMAIN, work).await.unwrap();
        assert_eq!(preview["canStart"], true);
        let receipt = owner::start(&db, &start_from(&preview, work))
            .await
            .unwrap();
        let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
        let worker = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
            db.clone(),
            "image-reading-synthetic",
            4,
            Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
            adapter,
            drain.clone(),
        ));
        let result = tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                let current = owner::latest(&db, dossier::DOMAIN, work, None)
                    .await
                    .unwrap();
                let state = current["analysis"]["state"].as_str().unwrap_or("");
                assert_ne!(state, "failed", "{current}");
                if state == "succeeded" {
                    break current;
                }
                assert!(!worker.is_finished());
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap();
        drain.request();
        worker.await.unwrap().unwrap();
        assert!(result["analysis"]["result"].is_object());
        let (raw, metadata, invocation_state): (Option<String>, Value, String) = sqlx::query_as(
            "SELECT r.raw_output,i.result,i.state FROM linggan_content_understanding_result r JOIN linggan_model_invocation i USING(invocation_ref) WHERE r.request_ref=$1"
        ).bind(receipt.request_ref).fetch_one(db.pool()).await.unwrap();
        assert!(raw.is_some());
        assert_eq!(invocation_state, "succeeded");
        let dossier = linggan_evidence::research_dossier::read(&db, work)
            .await
            .unwrap()
            .unwrap();
        if reject_reading {
            assert_eq!(metadata["imageReadings"]["state"], "failed");
            assert_eq!(
                metadata["imageReadings"]["code"],
                "image_reading_write_failed"
            );
            assert!(
                !dossier
                    .materials
                    .iter()
                    .any(|m| m["blocks"].to_string().contains("合成图片文字"))
            );
        } else if scenario == "existing" {
            assert_eq!(metadata["imageReadings"]["state"], "published");
            assert_eq!(metadata["imageReadings"]["receipts"][0]["current"], false);
            assert!(
                dossier
                    .materials
                    .iter()
                    .any(|m| m["blocks"].to_string().contains("连续的图片正文"))
            );
            assert!(
                !dossier
                    .materials
                    .iter()
                    .any(|m| m["blocks"].to_string().contains("合成图片文字"))
            );
        } else {
            assert_eq!(metadata["imageReadings"]["state"], "published");
            assert_eq!(metadata["imageReadings"]["receipts"][0]["current"], true);
            let image_material = dossier
                .materials
                .iter()
                .find(|m| m["blocks"].to_string().contains("合成图片文字"))
                .unwrap();
            assert_eq!(image_material["reading"]["generatedBy"], "cloud_vision");
            assert_eq!(image_material["reading"]["verification"], "unchecked");
            assert!(
                image_material["issues"]
                    .to_string()
                    .contains("model_formatted_reading")
            );
            assert!(
                linggan_evidence::research_dossier::markdown(&dossier)
                    .contains("图片文字由模型识读整理，引用前请对照原图。")
            );
            // A human may edit and resolve the issue without claiming verification.
            // The immutable model origin must still reach readers and exports.
            let unit_ref: Uuid =
                serde_json::from_value(metadata["imageReadings"]["receipts"][0]["unitRef"].clone())
                    .unwrap();
            let edit = linggan_evidence::material_reading::ReviewReadingRequest {
                command_ref: Uuid::new_v4(),
                unit_ref: unit_ref.to_string(),
                expected_current_revision_ref: image_material["currentRevisionRef"]
                    .as_str()
                    .map(|v| v.parse().unwrap()),
                source_version: image_material["sourceVersion"].as_str().unwrap().into(),
                action: "edit".into(),
                actor: "synthetic-editor".into(),
                reason: "整理后尚未核对".into(),
                edited_blocks: Some(vec![
                    json!({"kind":"paragraph","text":"编辑后的图片文字","speaker":"unconfirmed"}),
                ]),
                verified: false,
                resolved_issue_keys: vec!["model-formatted-image-reading".into()],
            };
            linggan_evidence::material_reading::review_reading(&db, unit_ref, &edit)
                .await
                .unwrap();
            let edited = linggan_evidence::research_dossier::read(&db, work)
                .await
                .unwrap()
                .unwrap();
            let edited_image = edited
                .materials
                .iter()
                .find(|m| m["blocks"].to_string().contains("编辑后的图片文字"))
                .unwrap();
            assert_eq!(edited_image["reading"]["formattedOrigin"], true);
            assert_eq!(edited_image["reading"]["verification"], "unchecked");
            assert!(
                !edited_image["issues"]
                    .to_string()
                    .contains("model_formatted_reading")
            );
            assert!(
                linggan_evidence::research_dossier::markdown(&edited)
                    .contains("图片文字由模型识读整理，引用前请对照原图。")
            );
            assert!(
                dossier
                    .materials
                    .iter()
                    .any(|m| m["blocks"].to_string().contains("合成图片文字"))
            );
        }
        endpoint.finish().await;
    }
}

async fn prove_billing_pause(
    db: &Database,
    work: Uuid,
    model: &setup::Configured,
    endpoint: &wire::Endpoint,
    adapter: PiAdapter,
) {
    use linggan_intelligence::ai_foundation::{
        batch_store::{BatchInput, BatchKind, CreateBatch, InputRef, ProbeOperation, create_batch},
        executor::service,
    };
    use std::sync::atomic::Ordering;

    submit_body(db, "新材料仍先摆出材料，再核对。本段只验证余额暂停。").await;
    let preview = owner::preview(db, dossier::DOMAIN, work).await.unwrap();
    endpoint.payment_required.store(true, Ordering::SeqCst);
    let request = owner::start(db, &start_from(&preview, work)).await.unwrap();
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let worker = tokio::spawn(service::run(
        db.clone(),
        "l1-billing-pause-proof",
        4,
        Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let status: Option<(bool, String, String)> = sqlx::query_as(
                "SELECT linggan_ai_billing_paused(p.connection_ref), i.result#>>'{diagnostic,httpStatus}', i.usage_state
                 FROM linggan_ai_connection_billing_pause p
                 JOIN linggan_model_invocation i USING(invocation_ref)
                 JOIN linggan_content_understanding_result r USING(invocation_ref)
                 WHERE p.connection_ref=$1 AND r.request_ref=$2",
            )
            .bind(model.connection).bind(request.request_ref)
            .fetch_optional(db.pool()).await.unwrap();
            if let Some(status) = status {
                assert_eq!(status, (true, "402".into(), "unknown_reserved".into()));
                break;
            }
            assert!(!worker.is_finished());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await.unwrap();
    let blocked = owner::preview(db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(blocked["canStart"], false);
    assert!(
        blocked["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x == "model_connection_billing_paused")
    );
    let (config, revision): (Uuid, i64) = sqlx::query_as(
        "SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='model_probe'",
    ).fetch_one(db.pool()).await.unwrap();
    endpoint.payment_required.store(false, Ordering::SeqCst);
    create_batch(
        db,
        &CreateBatch {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_task_revision: revision,
            task_config_ref: config,
            kind: BatchKind::Probe,
            items: vec![BatchInput {
                item_key: "billing-resume-probe".into(),
                input: InputRef::Probe {
                    sample_ref: Uuid::new_v4(),
                    connection_version_ref: model.version,
                    model_ref: Some(model.model),
                    operation: ProbeOperation::Probe,
                    profile: None,
                },
            }],
        },
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let paused: bool = sqlx::query_scalar("SELECT linggan_ai_billing_paused($1)")
                .bind(model.connection)
                .fetch_one(db.pool())
                .await
                .unwrap();
            if !paused {
                break;
            }
            assert!(!worker.is_finished());
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        owner::preview(db, dossier::DOMAIN, work).await.unwrap()["canStart"],
        true
    );
    drain.request();
    worker.await.unwrap().unwrap();
}

async fn seed_recoverable_failure(db: &linggan_storage_postgres::Database, work: Uuid) -> Uuid {
    let executor = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        db,
        "content-l1-recovery-owned-proof",
        4,
    )
    .await
    .unwrap();
    let preview = owner::preview(db, dossier::DOMAIN, work).await.unwrap();
    let receipt = owner::start(db, &start_from(&preview, work)).await.unwrap();
    let mut tx = executor.fence().begin(db, true).await.unwrap();
    let manifest: Value = sqlx::query_scalar(
        "SELECT input_manifest FROM linggan_content_understanding_request WHERE request_ref=$1",
    )
    .bind(receipt.request_ref)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let mut draft = wire::draft(&json!({"material":manifest["snapshot"]}));
    draft["type"] = json!("json_object");
    draft["evidenceCandidates"]["text"][0]["quote"] = json!("先核对顺序，再摆出材料");
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at,task_key,task_config_ref,batch_ref,dispatch_state,input_tokens,output_tokens,failure_code)
        SELECT $1,m.connection_version_ref,m.model_ref,c.config_ref,'analyze',repeat('a',64),'failed',1000,20,
        jsonb_build_object('ok',true,'promptVersion',t.prompt_version_ref,'diagnostic',jsonb_build_object('providerReturnedModel','synthetic-writing','terminalReceived',true,'finishReason','stop')),
        scope_001_now(),t.task_key,t.task_config_ref,r.batch_ref,'started',10,10,'ai_output_invalid'
        FROM linggan_content_understanding_request r JOIN linggan_ai_task_config t USING(task_config_ref)
        JOIN linggan_model_config c ON c.config_ref=t.model_config_ref JOIN linggan_model_entry m USING(model_ref)
        WHERE r.request_ref=$2")
        .bind(invocation).bind(receipt.request_ref).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=1,lease_token=$2,lease_instance_ref=$3,lease_instance_generation=$4,lease_expires_at=clock_timestamp()+interval '1 minute' WHERE item_ref=$1")
        .bind(receipt.item_ref).bind(Uuid::new_v4()).bind(executor.fence().instance_ref()).bind(executor.fence().generation()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control ctl SET invocation_ref=$2,attempt_count=1,executed_config_ref=cfg.task_config_ref,executed_skill_version_ref=cfg.skill_version_ref,execution_contract=cfg.execution_contract FROM linggan_ai_task_config cfg JOIN linggan_content_understanding_request r USING(task_config_ref) WHERE ctl.item_ref=$1 AND r.request_ref=$3")
        .bind(receipt.item_ref).bind(invocation).bind(receipt.request_ref).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_content_understanding_result(request_ref,work_ref,material_snapshot_identity,method_version,invocation_ref,raw_output,failure_code)
        SELECT request_ref,work_ref,material_snapshot_identity,method_version,$2,$3,'ai_output_invalid' FROM linggan_content_understanding_request WHERE request_ref=$1")
        .bind(receipt.request_ref).bind(invocation).bind(draft.to_string()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='failed',usable=false,failure_code='ai_output_invalid',finished_at=scope_001_now(),lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL WHERE item_ref=$1")
        .bind(receipt.item_ref).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    receipt.request_ref
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
    // A video title, body and cover are not a substitute for its speech transcript.
    let before_transcript = owner::preview(db, dossier::DOMAIN, work).await.unwrap();
    assert_eq!(before_transcript["materialReadiness"]["kind"], "video");
    assert_eq!(before_transcript["canStart"], false);
    assert!(
        before_transcript["materialReadiness"]["gaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|gap| gap["code"] == "video_transcript_required")
    );
    assert!(matches!(
        owner::start(db, &start_from(&before_transcript, work)).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
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
    tx.commit().await.unwrap();
}
