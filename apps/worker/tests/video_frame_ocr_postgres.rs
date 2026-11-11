//! Video frame OCR reading publication through the current lease and dossier readers.
#[path = "../../../crates/evidence/tests/support/material_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "../src/bin/media_worker_support/mod.rs"]
#[allow(dead_code)]
mod support;

use linggan_evidence::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

async fn admit_video(db: &Database, work: &str, ordinal: u32, bytes: &[u8]) -> Uuid {
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_content WHERE content_external_id=$1)",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    if !present {
        fixture::submit_package(db,"content_detail",json!({"contentExternalId":work}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":work},"payload":{"title":"Synthetic replay metadata","bodyText":"Native body must remain independent."}})).await;
    }
    let observation = Uuid::new_v4();
    let slot = format!("xhs:{work}:video:{ordinal}");
    let uri = format!("https://media.example/{work}/{ordinal}.jpg");
    let record = json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"video","ordinal":ordinal},"observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-21T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":work}});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["media_slots"],
        json!({"contentExternalId":work}),
        "media_slots",
        "xhs",
        json!({"target":{"contentExternalId":work},"layers":[fixture::coverage_layer("media_slots",1)]}),
        vec![record],
    )
    .await;
    let hash = support::sha256_hex(bytes);
    let key = format!("blobs/{hash}");
    let path = support::prepare_output(&key).unwrap();
    std::fs::write(path, bytes).unwrap();
    admit_media_blob(
        db,
        observation,
        &hash,
        "video/mp4",
        bytes.len() as i64,
        &key,
    )
    .await
    .unwrap();
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "owned PostgreSQL + cached local Whisper/Paddle + macOS say; actual recognition"]
async fn local_video_recognition_preserves_text_times_and_empty_sources() {
    use std::time::Duration;
    use tokio::process::Command;

    let root = support::local_media_root().join("synthetic-video");
    std::fs::create_dir_all(&root).unwrap();
    let python = support::paddle_ocr_python();
    let mut panels = Command::new(&python);
    panels.args(["-I", "-c", "from PIL import Image,ImageDraw,ImageFont; import sys; from pathlib import Path; root=Path(sys.argv[1]); font=ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf',64); [(lambda im,name,text: (ImageDraw.Draw(im).text((50,140),text,font=font,fill='black'),im.save(root/name)))(Image.new('RGB',(960,400),'white'),name,text) for name,text in [('first.png','FIRST STEP 12'),('second.png','SECOND STEP 34'),('blank.png','')]]"])
        .arg(&root);
    support::require_success(
        support::run_command(&mut panels, Duration::from_secs(30))
            .await
            .unwrap(),
        "synthetic_panels",
    )
    .unwrap();
    let mut speech = Command::new("/usr/bin/say");
    speech
        .args(["-v", "Tingting", "-r", "150", "-o"])
        .arg(root.join("speech.aiff"))
        .arg("先准备材料，再检查步骤，每天练习十分钟。");
    support::require_success(
        support::run_command(&mut speech, Duration::from_secs(30))
            .await
            .unwrap(),
        "synthetic_speech",
    )
    .unwrap();

    let mut video = Command::new(support::ffmpeg_command());
    video.args(["-y", "-loop", "1", "-framerate", "5", "-i"])
        .arg(root.join("first.png"))
        .args(["-loop", "1", "-framerate", "5", "-i"])
        .arg(root.join("second.png"))
        .arg("-i").arg(root.join("speech.aiff"))
        .args(["-filter_complex", "[0:v]trim=duration=6,setpts=PTS-STARTPTS[a];[1:v]trim=duration=6,setpts=PTS-STARTPTS[b];[a][b]concat=n=2:v=1:a=0[v];[2:a]apad=whole_dur=12[s]", "-map", "[v]", "-map", "[s]", "-t", "12", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
        .arg(root.join("spoken.mp4"));
    support::require_success(
        support::run_command(&mut video, Duration::from_secs(60))
            .await
            .unwrap(),
        "synthetic_video",
    )
    .unwrap();
    let mut silent = Command::new(support::ffmpeg_command());
    silent
        .args(["-y", "-loop", "1", "-framerate", "5", "-i"])
        .arg(root.join("blank.png"))
        .args(["-t", "2", "-an", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
        .arg(root.join("silent.mp4"));
    support::require_success(
        support::run_command(&mut silent, Duration::from_secs(30))
            .await
            .unwrap(),
        "synthetic_silent_video",
    )
    .unwrap();

    let db = fixture::proof_database("video_local_chain").await;
    let spoken = std::fs::read(root.join("spoken.mp4")).unwrap();
    let silent = std::fs::read(root.join("silent.mp4")).unwrap();
    let spoken_work = admit_video(&db, "video-local-spoken", 1, &spoken).await;
    let silent_work = admit_video(&db, "video-local-silent", 1, &silent).await;
    ensure_media_processing_work(&db).await.unwrap();
    let jobs: Vec<Uuid> = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE blob_sha256=ANY($1) AND ((processor_kind='asr' AND processor_version=$2) OR (processor_kind='video_frame_ocr' AND processor_version=$3))")
        .bind(vec![support::sha256_hex(&spoken), support::sha256_hex(&silent)])
        .bind(processor_version_for_kind("asr").unwrap())
        .bind(processor_version_for_kind("video_frame_ocr").unwrap())
        .fetch_all(db.pool()).await.unwrap();
    assert_eq!(jobs.len(), 4);
    let jobs_file = root.join("jobs.json");
    std::fs::write(&jobs_file, serde_json::to_vec(&jobs).unwrap()).unwrap();
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    assert!(!url.contains('?'));
    let home = std::env::var_os("HOME").unwrap();
    let mut worker = Command::new(env!("CARGO_BIN_EXE_linggan-media-worker"));
    worker
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LANG", "en_US.UTF-8")
        .env(
            "LINGGAN_LOCAL_DATABASE_URL",
            format!("{url}?options=-csearch_path%3Dvideo_local_chain"),
        )
        .env("LINGGAN_LOCAL_MEDIA_ROOT", support::local_media_root())
        .env("LINGGAN_FFMPEG_COMMAND", support::ffmpeg_command())
        .env("LINGGAN_WHISPER_COMMAND", support::whisper_command())
        .env(
            "LINGGAN_WHISPER_MODEL",
            std::path::Path::new(&home).join(".cache/whisper/medium.pt"),
        )
        .env("LINGGAN_PADDLE_OCR_PYTHON", python)
        .env("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
        .env("HF_HUB_OFFLINE", "1")
        .env("LINGGAN_COLLECTION_UPGRADE_PHASE", "governance")
        .args(["--jobs-file"])
        .arg(&jobs_file);
    let output = support::run_command(&mut worker, Duration::from_secs(600))
        .await
        .unwrap();
    support::require_success(output, "local_video_worker").unwrap();
    let completed: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_media_processing_work WHERE job_ref=ANY($1) AND state='completed' AND attempt_count=1")
        .bind(&jobs).fetch_one(db.pool()).await.unwrap();
    assert_eq!(completed, 4);
    let dossier = research_dossier::read(&db, spoken_work)
        .await
        .unwrap()
        .unwrap();
    let frame = dossier
        .materials
        .iter()
        .find(|m| m["kind"] == "video_frame")
        .unwrap();
    let audio = dossier
        .materials
        .iter()
        .find(|m| m["kind"] == "audio")
        .unwrap();
    let frame_text: String = frame["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(research_dossier::content::block_text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(frame_text.contains("FIRST STEP 12"), "{frame_text}");
    assert!(frame_text.contains("SECOND STEP 34"), "{frame_text}");
    let times: std::collections::BTreeSet<_> = frame["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["time"]["startMs"].as_u64().unwrap())
        .collect();
    assert_eq!(times, [0, 10000].into_iter().collect());
    assert_eq!(frame["reading"]["generatedBy"], "frame_ocr");
    assert!(frame["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["code"] == "sampling_incomplete"));
    let transcript: String = audio["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(research_dossier::content::block_text)
        .collect::<Vec<_>>()
        .join("\n");
    for phrase in ["准备材料", "检查步骤", "每天练习"] {
        assert!(transcript.contains(phrase), "{transcript}");
    }
    assert!(
        transcript.contains("十分钟") || transcript.contains("10分钟"),
        "{transcript}"
    );
    assert_eq!(audio["reading"]["generatedBy"], "asr");
    for block in audio["blocks"].as_array().unwrap() {
        let start = block["time"]["startMs"].as_u64().unwrap();
        let end = block["time"]["endMs"].as_u64().unwrap();
        assert!(start < end && end <= 12000);
    }
    assert!(dossier
        .fragments
        .iter()
        .any(|f| f.field == "body" && f.text == "Native body must remain independent."));
    let empty = research_dossier::read(&db, silent_work)
        .await
        .unwrap()
        .unwrap();
    let empty_audio = empty
        .materials
        .iter()
        .find(|m| m["kind"] == "audio")
        .unwrap();
    assert_eq!(empty_audio["sourceState"], "not_applicable");
    assert_eq!(
        empty_audio["processing"]["reason"],
        "no_audio_stream_observed"
    );
    let empty_frame = empty
        .materials
        .iter()
        .find(|m| m["kind"] == "video_frame")
        .unwrap();
    assert_eq!(empty_frame["blocks"][0]["kind"], "gap");
    assert_eq!(empty_frame["blocks"][0]["time"]["startMs"], 0);
    assert!(empty_frame["blocks"][0]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["code"] == "recognition_empty"));
    let image_jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_media_processing_job WHERE processor_kind='image_ocr'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(image_jobs, 0);
    if let Some(directory) = std::env::var_os("OCR_AUDIT_PROOF_OUTPUT") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(std::path::Path::new(&directory).join("video-local-chain.json"), serde_json::to_vec_pretty(&json!({"synthetic":true,"recognizers":"Whisper medium and PaddleOCR local; no replay","spokenMaterials":dossier.materials,"silentMaterials":empty.materials,"completedJobs":completed})).unwrap()).unwrap();
    }
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL runner only"]
async fn latest_frame_completion_and_empty_outcome_publish_current_readings() {
    use linggan_evidence::material_reading::producers::ReadingContent;
    let db = fixture::proof_database("frame_current_completion").await;
    for (name, no_text) in [("frame-text", false), ("frame-empty", true)] {
        let work = admit_video(&db, name, 1, name.as_bytes()).await;
        let job: Uuid = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE processor_kind='video_frame_ocr' AND blob_sha256=$1 AND processor_version='local-v5'").bind(support::sha256_hex(name.as_bytes())).fetch_one(db.pool()).await.unwrap();
        ensure_media_processing_work(&db).await.unwrap();
        let worker = Uuid::new_v4();
        let MediaProcessingClaimOutcome::Claimed(claim) =
            claim_media_processing_work(&db, worker, &["video_frame_ocr".into()])
                .await
                .unwrap()
        else {
            panic!("current frame job must be claimable")
        };
        assert_eq!(claim.job_ref, job);
        if no_text {
            complete_media_processing_without_output(&db, &claim, worker, "no_frame_text_observed")
                .await
                .unwrap();
        } else {
            let text = "synthetic literal frame text";
            complete_media_processing_text(&db, &claim, worker, "frame_ocr_text", &support::sha256_hex(text.as_bytes()), text.len() as i64, "derivatives/frame-ocr/synthetic.txt", text, text, None, ReadingContent { blocks: vec![json!({"ref":"synthetic","kind":"paragraph","text":text,"speaker":"unconfirmed"})], issues: vec![], source_state:"present".into(), source_location:json!({"layerVersion":"rules-v3"}) }).await.unwrap();
        }
        let dossier = research_dossier::read(&db, work).await.unwrap().unwrap();
        let frame = dossier
            .materials
            .iter()
            .find(|m| m["kind"] == "video_frame")
            .unwrap();
        assert!(frame["currentRevisionRef"].is_string());
        assert_eq!(frame["reading"]["generatedBy"], "frame_ocr");
        assert_eq!(frame["blocks"].as_array().unwrap().is_empty(), no_text);
        assert_eq!(frame["processing"]["state"], "completed");
        if no_text {
            assert_eq!(frame["processing"]["reason"], "no_frame_text_observed");
        }
    }
}
