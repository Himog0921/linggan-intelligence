//! Legacy motion text is prepared from immutable local results, never by replaying a worker.
#[path = "support/standard_dossier_fixture.rs"]
mod support;
use linggan_evidence::{
    dossier_input as input, material_reading as reading, research_dossier as dossier, *,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::{DOMAIN, fixture};
use uuid::Uuid;

fn hash(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
async fn source(db: &Database, work: Uuid) -> (String, String, Uuid) {
    sqlx::query_as("SELECT o.slot_key,m.blob_sha256,m.materialization_ref FROM linggan_material_media_origin o JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m USING(download_attempt_ref) WHERE o.content_public_ref=$1 ORDER BY o.source_generation DESC,m.verified_at DESC,m.materialization_ref DESC LIMIT 1")
        .bind(work).fetch_one(db.pool()).await.unwrap()
}
async fn legacy(
    db: &Database,
    work: Uuid,
    kind: &str,
    version: &str,
    text: Option<&str>,
    reason: Option<&str>,
) -> (Uuid, Option<Uuid>) {
    let (slot, blob, _) = source(db, work).await;
    let job = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,$4,$5,'full_blob')")
        .bind(job).bind(&blob).bind(slot).bind(kind).bind(version).execute(db.pool()).await.unwrap();
    ensure_media_processing_work(db).await.unwrap();
    let derivative = if let Some(text) = text {
        let derivative = Uuid::new_v4();
        let kind = if kind == "asr" {
            "asr_text"
        } else {
            "frame_ocr_text"
        };
        sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(derivative).bind(job).bind(kind).bind(hash(text)).bind(text.len() as i64).bind(format!("legacy/{derivative}.txt")).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text,source_location) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(derivative).bind(work).bind(kind).bind(text).bind(text.chars().take(30).collect::<String>()).bind(json!({"blobSha256":blob,"processorVersion":version})).execute(db.pool()).await.unwrap();
        Some(derivative)
    } else {
        None
    };
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason,occurred_at) VALUES($1,$2,'succeeded',$3,clock_timestamp())")
        .bind(Uuid::new_v4()).bind(job).bind(reason).execute(db.pool()).await.unwrap();
    (job, derivative)
}
async fn plan(db: &Database, work: Uuid) -> input::BatchPlan {
    input::plan(
        db,
        DOMAIN,
        &[work],
        None,
        input::BatchOperation::HistoricalMedia,
    )
    .await
    .unwrap()
}
async fn counters(db: &Database) -> (i64, i64, i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_media_processing_job_event),(SELECT count(*) FROM linggan_media_derivative),(SELECT count(*) FROM linggan_model_invocation),(SELECT count(*) FROM linggan_material_reading)")
        .fetch_one(db.pool()).await.unwrap()
}
async fn current(db: &Database, work: Uuid, kind: &str) -> Value {
    dossier::read(db, work)
        .await
        .unwrap()
        .unwrap()
        .materials
        .into_iter()
        .find(|m| m["kind"] == kind)
        .unwrap()
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn legacy_null_freeze_retains_full_text_unknown_time_and_actual_provenance_without_jobs() {
    let db = fixture::proof_database("historical_media_full").await;
    let work = support::scenario(&db, "legacy-full", None, 0, true, false, true).await;
    let full = format!(
        "{}否定和0.5 mg不能丢失🙂",
        "历史口播完整段落。\n".repeat(3500)
    );
    let frame = "疑似水印 / 合成乱码 abc123 / 画面文字均保留";
    let (asr, asr_derivative) = legacy(&db, work, "asr", "legacy-asr.v1", Some(&full), None).await;
    let (ocr, frame_derivative) = legacy(
        &db,
        work,
        "video_frame_ocr",
        "legacy-frame.v1",
        Some(frame),
        None,
    )
    .await;
    let (_, blob, materialization) = source(&db, work).await;
    let before = counters(&db).await;
    let plan = plan(&db, work).await;
    assert_eq!(plan.entries[0].historical.as_ref().unwrap()["eligible"], 2);
    assert_eq!(counters(&db).await, before, "planning is read only");
    let applied = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(applied.updated, 1);
    assert!(applied.complete());
    assert_eq!(applied.results[0].result["updated"], 2);
    for (kind, text, job, derivative, generated) in [
        ("audio", full.as_str(), asr, asr_derivative.unwrap(), "asr"),
        (
            "video_frame",
            frame,
            ocr,
            frame_derivative.unwrap(),
            "frame_ocr",
        ),
    ] {
        let unit = current(&db, work, kind).await;
        assert_eq!(unit["sourceVersion"], materialization.to_string());
        assert_eq!(
            unit["blocks"][0]["text"], text,
            "bounded display_text cannot truncate the imported original"
        );
        assert_eq!(unit["blocks"][0]["kind"], "paragraph");
        assert_eq!(unit["blocks"][0]["speaker"], "unconfirmed");
        assert!(
            unit["blocks"][0].get("time").is_none(),
            "unknown is not zero milliseconds"
        );
        assert_eq!(unit["reading"]["generatedBy"], generated);
        assert_eq!(unit["reading"]["adoption"], "automatic");
        assert_ne!(unit["reading"]["verification"], "verified");
        let stored: (Uuid, Uuid, Value) = sqlx::query_as("SELECT producer_ref,derivative_ref,source_location FROM linggan_material_reading WHERE reading_ref=$1")
            .bind(unit["currentRevisionRef"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(db.pool()).await.unwrap();
        assert_eq!((stored.0, stored.1), (job, derivative));
        assert_eq!(stored.2["materializationRef"], materialization.to_string());
        assert_eq!(stored.2["blobSha256"], blob);
        assert_eq!(stored.2["timeUnknown"], true);
        assert_eq!(stored.2["fullTextHash"], hash(text));
        assert!(
            unit["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["code"] == "time_unknown")
        );
        assert!(
            unit["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["code"] == "machine_unchecked")
        );
    }
    let after = counters(&db).await;
    assert_eq!(
        (after.0, after.1, after.2, after.3),
        (before.0, before.1, before.2, before.3)
    );
    assert_eq!(after.4 - before.4, 2);
    let frozen: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_media_processing_work WHERE job_ref=ANY($1) AND reading_source_version IS NOT NULL")
        .bind(&[asr, ocr][..]).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        frozen, 0,
        "legacy jobs never acquire an invented frozen source"
    );
    let replay = input::apply_plan(&db, &plan).await.unwrap();
    assert_eq!(replay.already_current, 1);
    assert_eq!(replay.results[0].result["readOnlyReplay"], true);
    assert_eq!(replay.results[0].result["alreadyCurrent"], 2);
    assert_eq!(counters(&db).await, after);
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn human_current_survives_same_bytes_reobservation_and_legacy_repreparation() {
    let db = fixture::proof_database("historical_media_human").await;
    let name = "legacy-human";
    let work = support::scenario(&db, name, None, 0, true, false, true).await;
    legacy(
        &db,
        work,
        "asr",
        "legacy-asr.v1",
        Some("原始完整口播，不能被重接入覆盖。"),
        None,
    )
    .await;
    legacy(
        &db,
        work,
        "video_frame_ocr",
        "legacy-frame.v1",
        None,
        Some("no_frame_text_observed"),
    )
    .await;
    input::apply_plan(&db, &plan(&db, work).await)
        .await
        .unwrap();
    let audio = current(&db, work, "audio").await;
    let unit: Uuid = serde_json::from_value(audio["ref"].clone()).unwrap();
    let old: Uuid = serde_json::from_value(audio["currentRevisionRef"].clone()).unwrap();
    let edited = reading::review_reading(
        &db,
        unit,
        &reading::ReviewReadingRequest {
            command_ref: Uuid::new_v4(),
            expected_current_revision_ref: Some(old),
            candidate_revision_ref: old,
            action: "edit".into(),
            actor: "synthetic-reviewer".into(),
            reason: "核对合成视频原件".into(),
            edited_blocks: Some(vec![
                json!({"kind":"paragraph","text":"人工核对后的口播。","speaker":"unconfirmed"}),
            ]),
            verified: false,
            resolved_issue_keys: vec![],
        },
    )
    .await
    .unwrap();
    let (slot, blob, old_materialization) = source(&db, work).await;
    let observation = Uuid::new_v4();
    fixture::submit_custom_package(&db,"xhs",&["media_slots"],json!({"contentExternalId":name}),"media_slots","xhs",json!({"target":{"contentExternalId":name},"layers":[fixture::coverage_layer("media_slots",1)]}),vec![json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"video","ordinal":1},"observation":{"externalUri":"https://media.example/reobserved.mp4","candidateUris":["https://media.example/reobserved.mp4"],"observedAt":"2026-10-20T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":name}})]).await;
    let (size, key): (i64, String) =
        sqlx::query_as("SELECT byte_size,storage_key FROM linggan_media_blob WHERE sha256=$1")
            .bind(&blob)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let new_materialization = admit_media_blob(&db, observation, &blob, "video/mp4", size, &key)
        .await
        .unwrap()
        .materialization_ref;
    assert_ne!(old_materialization, new_materialization);
    let before = counters(&db).await;
    let response = input::apply_plan(&db, &plan(&db, work).await)
        .await
        .unwrap();
    assert!(response.complete());
    assert_eq!(response.results[0].result["alreadyCurrent"], 2);
    assert_eq!(
        response.results[0].result["results"][0]["humanProtected"],
        true
    );
    let preserved = current(&db, work, "audio").await;
    assert_eq!(
        preserved["currentRevisionRef"],
        edited["currentRevisionRef"]
    );
    assert_eq!(preserved["blocks"][0]["text"], "人工核对后的口播。");
    assert_eq!(preserved["reading"]["adoption"], "human_edited");
    assert_eq!(counters(&db).await, before);
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn legacy_source_change_conflicts_before_writes_and_retired_or_restricted_text_is_excluded() {
    let db = fixture::proof_database("historical_media_conflict").await;
    let work = support::scenario(&db, "legacy-conflict", None, 0, true, false, true).await;
    let (_, old_asr) = legacy(&db, work, "asr", "legacy-asr.v1", Some("旧口播"), None).await;
    let (frame_job, _) = legacy(
        &db,
        work,
        "video_frame_ocr",
        "legacy-frame.v1",
        Some("画面原文"),
        None,
    )
    .await;
    let frozen = plan(&db, work).await;
    let (_, new_asr) = legacy(
        &db,
        work,
        "asr",
        "legacy-asr.v2",
        Some("来源全文变更，旧预览不得写入"),
        None,
    )
    .await;
    let before = counters(&db).await;
    let rejected = input::apply_plan(&db, &frozen).await.unwrap();
    assert_eq!(rejected.failed, 1);
    assert_eq!(rejected.results[0].result["reason"], "material_changed");
    assert_eq!(
        counters(&db).await,
        before,
        "source conflict cannot partially publish the frame"
    );
    sqlx::query("INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason) VALUES($1,'tesseract_replaced_by_paddleocr')")
        .bind(frame_job).execute(db.pool()).await.unwrap();
    for derivative in [old_asr.unwrap(), new_asr.unwrap()] {
        record_derivative_disposition(
            &db,
            derivative,
            MaterialMediaDisposition::WithdrawnOrRestricted,
            "synthetic-policy",
            "legacy source restriction",
        )
        .await
        .unwrap();
    }
    let next = plan(&db, work).await;
    assert_eq!(next.entries[0].historical.as_ref().unwrap()["eligible"], 0);
    let before = counters(&db).await;
    let skipped = input::apply_plan(&db, &next).await.unwrap();
    assert_eq!(skipped.partial, 1);
    assert_eq!(skipped.results[0].result["updated"], 0);
    assert_eq!(skipped.results[0].result["unavailable"], 2);
    assert_eq!(counters(&db).await, before);
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn explicit_empty_receipts_close_source_and_coverage_without_text_and_missing_images_are_acquisition_not_ocr()
 {
    let db = fixture::proof_database("historical_media_empty").await;
    let work = support::scenario(&db, "legacy-empty", None, 0, true, false, true).await;
    let (asr, _) = legacy(
        &db,
        work,
        "asr",
        "legacy-asr.v1",
        None,
        Some("no_speech_observed"),
    )
    .await;
    let (frame, _) = legacy(
        &db,
        work,
        "video_frame_ocr",
        "legacy-frame.v1",
        None,
        Some("no_frame_text_observed"),
    )
    .await;
    let unprepared = plan(&db, work).await;
    fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"legacy-empty"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"legacy-empty"},"payload":{"bodyText":"无关正文修订，不得被历史空结果伪装成只读回放"}})).await;
    let frozen = plan(&db, work).await;
    assert_ne!(
        frozen.entries[0].expected_revision,
        unprepared.entries[0].expected_revision
    );
    assert_eq!(
        frozen.entries[0].historical_source_hash,
        unprepared.entries[0].historical_source_hash
    );
    let before = counters(&db).await;
    let drift = input::apply_plan(&db, &unprepared).await.unwrap();
    assert_eq!(
        drift.failed, 1,
        "unprepared empty sources require the original revision guard"
    );
    assert_eq!(drift.results[0].result["reason"], "material_changed");
    assert_eq!(
        counters(&db).await,
        before,
        "a claimed read-only replay cannot publish empty readings"
    );
    let receipt = input::apply_plan(&db, &frozen).await.unwrap();
    assert!(receipt.complete());
    assert_eq!(receipt.results[0].result["knownEmpty"], 2);
    let empty = dossier::read(&db, work).await.unwrap().unwrap();
    for kind in ["audio", "video_frame"] {
        let material = empty.materials.iter().find(|m| m["kind"] == kind).unwrap();
        assert_eq!(material["sourceState"], "known_empty");
        assert!(material["currentRevisionRef"].is_string());
        let stored: (Option<Uuid>, Uuid, Value) = sqlx::query_as("SELECT derivative_ref,producer_ref,source_location FROM linggan_material_reading WHERE reading_ref=$1")
            .bind(material["currentRevisionRef"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(db.pool()).await.unwrap();
        assert!(stored.0.is_none());
        assert_eq!(stored.1, if kind == "audio" { asr } else { frame });
        assert_eq!(
            stored.2["materializationRef"],
            source(&db, work).await.2.to_string()
        );
        assert_eq!(
            stored.2["recognitionOutcome"],
            if kind == "audio" {
                "no_speech_observed"
            } else {
                "no_frame_text_observed"
            }
        );
        assert!(stored.2.get("fullTextHash").is_none());
        assert_eq!(
            material["blocks"],
            json!([]),
            "empty recognition cannot invent words or times"
        );
        assert!(
            material["issues"]
                .as_array()
                .unwrap()
                .iter()
                .all(|i| i["code"] != "not_processed")
        );
    }
    assert_eq!(empty.coverage["sources"]["audio"]["state"], "known_empty");
    assert_eq!(
        empty.coverage["sources"]["videoFrames"]["state"],
        "known_empty"
    );
    assert_eq!(receipt.results[0].result["updated"], 2);
    let after = counters(&db).await;
    assert_eq!(
        (after.0, after.1, after.2, after.3),
        (before.0, before.1, before.2, before.3)
    );
    assert_eq!(
        after.4 - before.4,
        2,
        "only canonical empty observations are appended"
    );
    let replay = input::apply_plan(&db, &frozen).await.unwrap();
    assert!(replay.complete());
    assert_eq!(replay.results[0].result["readOnlyReplay"], true);
    assert_eq!(replay.results[0].result["alreadyCurrent"], 2);
    assert_eq!(replay.results[0].result["updated"], 0);
    assert_eq!(counters(&db).await, after);
    let no_audio = support::scenario(&db, "legacy-no-audio", None, 0, true, false, true).await;
    legacy(
        &db,
        no_audio,
        "asr",
        "legacy-asr.v1",
        None,
        Some("no_audio_stream_observed"),
    )
    .await;
    legacy(
        &db,
        no_audio,
        "video_frame_ocr",
        "legacy-frame.v1",
        None,
        Some("no_frame_text_observed"),
    )
    .await;
    assert!(
        input::apply_plan(&db, &plan(&db, no_audio).await)
            .await
            .unwrap()
            .complete()
    );
    let no_audio = dossier::read(&db, no_audio).await.unwrap().unwrap();
    assert_eq!(
        no_audio
            .materials
            .iter()
            .find(|m| m["kind"] == "audio")
            .unwrap()["sourceState"],
        "not_applicable"
    );
    assert_eq!(
        no_audio.coverage["sources"]["audio"]["state"],
        "not_applicable"
    );
    let missing = support::scenario(&db, "relayer-acquisition", None, 2, false, true, true).await;
    let relayer = input::plan(
        &db,
        DOMAIN,
        &[missing],
        None,
        input::BatchOperation::Relayer,
    )
    .await
    .unwrap();
    assert_eq!(relayer.entries[0].relayer["needsAcquisition"], 1);
    assert_eq!(relayer.entries[0].relayer["needsOcr"], 1);
    let receipt = input::apply_plan(&db, &relayer).await.unwrap();
    assert_eq!(receipt.partial, 1);
    assert_eq!(receipt.needs_acquisition, 1);
    assert_eq!(receipt.needs_ocr, 1);
    assert_eq!(receipt.results[0].result["needsAcquisition"], 1);
    assert_eq!(receipt.results[0].result["needsOcr"], 1);
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn image_slots_with_abnormal_motion_bytes_are_not_invisible_historical_media() {
    let db = fixture::proof_database("historical_media_component").await;
    let work = support::scenario(&db, "legacy-image-motion", None, 1, false, false, true).await;
    let observation: Uuid = sqlx::query_scalar("SELECT observation_ref FROM linggan_material_media_origin WHERE content_public_ref=$1 ORDER BY source_generation DESC LIMIT 1")
        .bind(work).fetch_one(db.pool()).await.unwrap();
    let bytes = "abnormal-motion-on-image-slot";
    admit_media_blob(
        &db,
        observation,
        &hash(bytes),
        "video/mp4",
        bytes.len() as i64,
        "legacy/abnormal-image-motion.mp4",
    )
    .await
    .unwrap();
    let (job, derivative) = legacy(
        &db,
        work,
        "asr",
        "legacy-asr.v1",
        Some("不属于图像组件的隐藏口播"),
        None,
    )
    .await;
    let (_, _, materialization) = source(&db, work).await;
    let mut tx = db.pool().begin().await.unwrap();
    assert!(
        reading::producers::publish_historical_in(&mut tx, work, job, materialization, derivative)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let before = counters(&db).await;
    let frozen = plan(&db, work).await;
    assert_eq!(frozen.entries[0].historical.as_ref().unwrap()["units"], 0);
    let result = input::apply_plan(&db, &frozen).await.unwrap();
    assert_eq!(result.partial, 1);
    assert_eq!(result.results[0].result["state"], "source_not_available");
    assert_eq!(result.results[0].result["sourceUnits"], 0);
    assert_eq!(counters(&db).await, before);
}
