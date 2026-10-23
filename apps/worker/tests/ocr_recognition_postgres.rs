//! Owned PostgreSQL and loopback Pi proof for the manual three-image OCR path.
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
        batch_control::{CancelBatch, cancel_batch},
        control::BudgetPolicy,
        control_store::ControlScope,
        executor::{ExecutorSession, ocr_recognition, queue},
        ocr_manual_adoption::{Adopt, adopt},
        ocr_recognition::{StartRun, read_run, start_run},
        provider_store::MaterialClass,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;
fn adapter() -> PiAdapter {
    PiAdapter::configured_with_test_command(
        PathBuf::from(std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap()),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pi-adapter/src/adapter.mjs"),
    )
}
fn image(seed: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 2, 2);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&[seed, 0, 0, 0, seed, 0, 0, 0, seed, seed, seed, seed])
        .unwrap();
    drop(writer);
    out
}
async fn configure_recognition(
    db: &linggan_storage_postgres::Database,
    endpoint: &str,
    owner: &ExecutorSession,
    adapter: &PiAdapter,
) -> setup::Configured {
    let model = setup::prepare_with_actual_probe(db, endpoint, owner, adapter).await;
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='ocr_recognition'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::OcrRecognition,
            expected_revision: revision,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: model.capability,
                limits: model.limits.clone(),
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    setup::enable(db, ControlScope::Task(TaskKey::OcrRecognition)).await;
    setup::allow(
        db,
        model.endpoint,
        TaskKey::OcrRecognition,
        MaterialClass::ManualOcrImage,
    )
    .await;
    model
}
async fn apply_manual_ocr_migrations(db: &linggan_storage_postgres::Database) {
    for migration in [
        include_str!("../../../database/migrations/0142_ocr_adoption.sql"),
        include_str!("../../../database/migrations/0143_ocr_quality_audit.sql"),
        include_str!("../../../database/migrations/0144_ocr_manual_recognition.sql"),
        include_str!("../../../database/migrations/0145_ocr_manual_human_adoption.sql"),
    ] {
        sqlx::raw_sql(migration).execute(db.pool()).await.unwrap();
    }
}
#[tokio::test]
#[ignore = "owned disposable PostgreSQL and loopback Pi only"]
async fn manual_ocr_three_synthetic_images_are_frozen_sequential_and_readable() {
    let db = fixture::db("ocr_recognition_owned").await;
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0142_ocr_adoption.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0143_ocr_quality_audit.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0144_ocr_manual_recognition.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0145_ocr_manual_human_adoption.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    let work_key = "ocr-recognition-three-synthetic";
    let work = ocr::admit(&db, work_key, 1, &image(1), "image/png").await;
    ocr::admit(&db, work_key, 2, &image(2), "image/png").await;
    ocr::admit(&db, work_key, 3, &image(3), "image/png").await;
    let unconfigured = read_run(&db, work).await.unwrap();
    assert_eq!(unconfigured["state"], "unavailable");
    assert_eq!(
        unconfigured["unavailableReason"], "图片文字识别尚未配置可用模型或端点。",
        "an empty model selection is configuration state, not a database read failure"
    );
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0, "reading unconfigured OCR never calls a model");
    let wire = endpoint::Endpoint::start(false).await;
    let adapter = adapter();
    let mut owner = ExecutorSession::acquire(&db, "synthetic-manual-ocr", 4)
        .await
        .unwrap();
    let model = setup::prepare_with_actual_probe(&db, &wire.url, &owner, &adapter).await;
    let configured = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::OcrRecognition,
            expected_revision: 0,
            model: Some(ModelSelection {
                model_ref: model.model,
                capability_ref: model.capability,
                limits: model.limits.clone(),
            }),
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let disabled = read_run(&db, work).await.unwrap();
    assert_eq!(disabled["state"], "unavailable");
    assert!(
        disabled["unavailableReason"]
            .as_str()
            .unwrap()
            .contains("开关"),
        "GET must expose the current disabled task switch before offering a start action"
    );
    setup::enable(&db, ControlScope::Task(TaskKey::OcrRecognition)).await;
    setup::allow(
        &db,
        model.endpoint,
        TaskKey::OcrRecognition,
        MaterialClass::ManualOcrImage,
    )
    .await;
    let ready = read_run(&db, work).await.unwrap();
    assert_eq!(ready["state"], "ready");
    let scope = ready["scope"]["sha256"].as_str().unwrap().to_owned();
    let request = StartRun {
        command_ref: Uuid::new_v4(),
        expected_scope_sha256: scope,
    };
    let receipt = start_run(&db, work, &request).await.unwrap();
    assert_eq!(
        start_run(&db, work, &request).await.unwrap(),
        receipt,
        "same command only replays"
    );
    assert_eq!(read_run(&db, work).await.unwrap()["completedImages"], 0);
    for expected in 0..3 {
        let item = queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .expect("next sequential item");
        assert_eq!(item.task_key, TaskKey::OcrRecognition);
        assert_eq!(item.position, expected);
        ocr_recognition::execute(&db, &item, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
        let progress = read_run(&db, work).await.unwrap();
        assert_eq!(progress["completedImages"], expected + 1);
        assert_eq!(progress["totalImages"], 3);
        assert_eq!(
            progress["state"],
            if expected == 2 {
                "succeeded"
            } else {
                "running"
            },
            "100% is observable only after the third receipt transaction"
        );
    }
    assert!(
        queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .is_none()
    );
    let run = read_run(&db, work).await.unwrap();
    assert_eq!(run["state"], "succeeded");
    assert_eq!(run["completedImages"], 3);
    assert_eq!(run["totalImages"], 3);
    assert_eq!(run["images"].as_array().unwrap().len(), 3);
    assert_eq!(run["coverTitle"], "合成封面标题");
    assert_eq!(run["coverTitleStatus"], "adopted");
    let images = run["images"].as_array().unwrap();
    let adoption: Uuid = serde_json::from_value(images[1]["adoptionRef"].clone()).unwrap();
    let raw_before: serde_json::Value = sqlx::query_scalar(
        "SELECT observation FROM linggan_media_ocr_adoption WHERE adoption_ref=$1",
    )
    .bind(adoption)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let manual = Adopt {
        command_ref: Uuid::new_v4(),
        adoption_ref: adoption,
        expected_revision: 0,
        decision: "adopt".into(),
        field: "adopted_body".into(),
        text: Some("内页章节标题".into()),
        actor: "SYNTHETIC-HUMAN-NOT-MOG".into(),
        reason: "合成原图逐字核对".into(),
    };
    let manual_receipt = adopt(&db, work, &manual).await.unwrap();
    assert_eq!(manual_receipt["auditSampleRef"], serde_json::Value::Null);
    assert_eq!(adopt(&db, work, &manual).await.unwrap(), manual_receipt);
    assert!(matches!(
        adopt(
            &db,
            work,
            &Adopt {
                expected_revision: 0,
                command_ref: Uuid::new_v4(),
                ..manual.clone()
            }
        )
        .await,
        Err(linggan_intelligence::ai_foundation::store::FoundationError::Conflict)
    ));
    let review: (Option<Uuid>, Option<Uuid>, String, String) = sqlx::query_as(
        "SELECT sample_ref,issue_ref,source_kind,after_text FROM linggan_media_ocr_review WHERE review_ref=$1",
    )
    .bind(manual.command_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(review.0, None);
    assert_eq!(review.1, None);
    assert_eq!(review.2, "ocr_block");
    assert_eq!(review.3, "内页章节标题");
    let updated = read_run(&db, work).await.unwrap();
    assert_eq!(updated["images"][1]["reviewRevision"], 1);
    assert_eq!(updated["images"][1]["humanBody"]["text"], "内页章节标题");
    let raw_after: serde_json::Value = sqlx::query_scalar(
        "SELECT observation FROM linggan_media_ocr_adoption WHERE adoption_ref=$1",
    )
    .bind(adoption)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        raw_after, raw_before,
        "human adoption never mutates the machine observation"
    );
    assert!(
        images[1]["regions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|region| region["role"] == "chapter_heading" && region["text"] == "内页章节标题"),
        "an interior heading remains an image region and is never promoted to coverTitle"
    );
    assert!(
        images[1]["regions"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|region| region["spans"].as_array().unwrap())
            .any(|span| span["status"] == "pending"),
        "partial image text is a pending Unicode range rather than fabricated certainty"
    );
    assert!(
        run["images"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["rawJson"].is_string() && i["usage"]["measuredTokens"].is_number())
    );
    assert_eq!(
        wire.requests.lock().unwrap().len(),
        4,
        "one provider probe plus exact three images"
    );
    let raw_paths:Vec<String>=sqlx::query_scalar("SELECT storage_key FROM linggan_media_derivative WHERE storage_key LIKE 'ocr/manual/%' ORDER BY storage_key").fetch_all(db.pool()).await.unwrap();
    assert_eq!(raw_paths.len(), 3);
    for path in raw_paths {
        let raw: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                PathBuf::from(std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT").unwrap()).join(path),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(
            raw["regions"].is_array(),
            "raw provider JSON is actually persisted"
        );
    }
    let layouts:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_media_ocr_layout WHERE engine='cloud_manual' AND content_public_ref=$1").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(layouts, 3);
    assert_eq!(configured.task_key, TaskKey::OcrRecognition);
    if let Some(binary) = std::env::var_os("OCR_AUDIT_BROWSER_PROOF_BINARY") {
        let out = PathBuf::from(std::env::var_os("OCR_AUDIT_PROOF_OUTPUT").unwrap());
        std::fs::create_dir_all(&out).unwrap();
        let release = out.join("release-second");
        let _ = std::fs::remove_file(&release);
        let browser_wire = endpoint::Endpoint::start_browser_second_gate(release).await;
        let browser_work_key = "ocr-recognition-browser-synthetic";
        let browser_work = ocr::admit(&db, browser_work_key, 1, &image(11), "image/png").await;
        ocr::admit(&db, browser_work_key, 2, &image(12), "image/png").await;
        ocr::admit(&db, browser_work_key, 3, &image(13), "image/png").await;
        let browser_package: Uuid = sqlx::query_scalar(
            "SELECT package_ref FROM linggan_material_media_origin
             WHERE content_public_ref=$1 ORDER BY source_generation DESC LIMIT 1",
        )
        .bind(browser_work)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)
             VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)",
        )
        .bind(Uuid::new_v4())
        .bind(browser_work)
        .bind(fixture::dossier::DOMAIN)
        .bind(browser_package)
        .execute(db.pool())
        .await
        .unwrap();
        // The model is reprobed against this new loopback endpoint; it is still entirely
        // synthetic, and it proves the browser's settings save does not create the OCR run.
        let browser_model =
            setup::prepare_with_actual_probe(&db, &browser_wire.url, &owner, &adapter).await;
        let revision: i64 = sqlx::query_scalar(
            "SELECT revision FROM linggan_ai_task_setting WHERE task_key='ocr_recognition'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        save_task_config(
            &db,
            &SaveTaskConfig {
                command_ref: Uuid::new_v4(),
                task_key: TaskKey::OcrRecognition,
                expected_revision: revision,
                model: Some(ModelSelection {
                    model_ref: browser_model.model,
                    capability_ref: browser_model.capability,
                    limits: browser_model.limits.clone(),
                }),
                skill_version_ref: None,
                material_columns: Default::default(),
                budget: BudgetPolicy::Unlimited,
            },
        )
        .await
        .unwrap();
        setup::allow(
            &db,
            browser_model.endpoint,
            TaskKey::OcrRecognition,
            MaterialClass::ManualOcrImage,
        )
        .await;
        assert_eq!(read_run(&db, browser_work).await.unwrap()["state"], "ready");
        std::fs::write(
            out.join("recognition-fixture.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema":"ocr_recognition_owned",
                "domain":fixture::dossier::DOMAIN,
                "workRef":browser_work,
            }))
            .unwrap(),
        )
        .unwrap();
        let worker_db = db.clone();
        let worker_fence = owner.fence();
        let worker_adapter = adapter.clone();
        let mut worker = tokio::spawn(async move {
            for position in 0..3 {
                let item = loop {
                    if let Some(item) = queue::claim_next(&worker_db, worker_fence).await.unwrap() {
                        break item;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                };
                assert_eq!(item.position, position);
                ocr_recognition::execute(
                    &worker_db,
                    &item,
                    &SyntheticModelSecrets,
                    &worker_adapter,
                )
                .await
                .unwrap();
            }
        });
        let mut browser = tokio::process::Command::new(binary)
            .arg("local_web::ocr_recognition_browser_tests::owned_manual_ocr_real_ui")
            .arg("--ignored")
            .arg("--nocapture")
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let waited = tokio::time::timeout(std::time::Duration::from_secs(90), async {
            loop {
                if let Some(status) = browser.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                owner.heartbeat().await.unwrap();
            }
        })
        .await;
        let status = match waited {
            Ok(status) if status.success() => status,
            Ok(status) => {
                worker.abort();
                let _ = worker.await;
                panic!("owned OCR browser proof failed: {status}");
            }
            Err(_) => {
                let _ = browser.kill().await;
                let _ = browser.wait().await;
                worker.abort();
                let _ = worker.await;
                panic!("owned OCR browser proof timed out before release");
            }
        };
        assert!(status.success());
        let worker_result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                tokio::select! {
                    result = &mut worker => return result,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => owner.heartbeat().await.unwrap(),
                }
            }
        }).await;
        match worker_result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => panic!("owned OCR browser executor join failed: {error}"),
            Err(_) => {
                worker.abort();
                let _ = worker.await;
                panic!("owned OCR browser executor timed out");
            }
        }
        assert_eq!(
            read_run(&db, browser_work).await.unwrap()["completedImages"],
            3
        );
        browser_wire.finish().await;
    }
    let first_materialization: Uuid = sqlx::query_scalar(
        "SELECT materialization_ref FROM linggan_ocr_recognition_item
         WHERE item_ref=$1",
    )
    .bind(serde_json::from_value::<Uuid>(run["images"][0]["imageRef"].clone()).unwrap())
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,materialization_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-manual-ocr-proof','synthetic materialization withdrawal',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(first_materialization).execute(db.pool()).await.unwrap();
    let withdrawn = read_run(&db, work).await.unwrap();
    assert!(withdrawn["coverTitle"].is_null());
    assert_eq!(withdrawn["images"][0]["state"], "unknown");
    assert!(withdrawn["images"][0]["originalUrl"].is_null());
    assert!(withdrawn["images"][0]["rawJson"].is_null());
    assert_eq!(withdrawn["images"][0]["regions"], serde_json::json!([]));
    wire.finish().await;
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL only; no model or image egress"]
async fn cloud_manual_human_adoption_is_append_only_and_not_an_audit_sample() {
    let db = fixture::db("ocr_manual_adoption_owned").await;
    apply_manual_ocr_migrations(&db).await;
    let work = ocr::admit(
        &db,
        "ocr-manual-adoption-synthetic",
        1,
        &image(19),
        "image/png",
    )
    .await;
    let row = sqlx::query(
        "SELECT o.slot_key,m.blob_sha256,m.materialization_ref,m.local_asset_path,
                b.storage_key,b.mime_type,b.byte_size
           FROM linggan_media_observation o
           JOIN linggan_media_download_attempt attempt
             ON attempt.media_observation_ref=o.observation_ref
           JOIN linggan_media_materialization m USING(download_attempt_ref)
           JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
          WHERE o.slot_key=$1",
    )
    .bind("xhs:ocr-manual-adoption-synthetic:image:1")
    .fetch_one(db.pool())
    .await
    .unwrap();
    let job = Uuid::new_v4();
    let derivative = Uuid::new_v4();
    let layout = Uuid::new_v4();
    let adoption = Uuid::new_v4();
    let processor_version = format!("cloud-manual-v1:{job}");
    let blob: String = row.get("blob_sha256");
    let slot: String = row.get("slot_key");
    sqlx::query(
        "INSERT INTO linggan_media_processing_job
          (job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope)
         VALUES($1,$2,$3,'image_ocr',$4,'full')",
    )
    .bind(job)
    .bind(&blob)
    .bind(&slot)
    .bind(&processor_version)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason)
         VALUES($1,$2,'succeeded','synthetic saved cloud result')",
    )
    .bind(Uuid::new_v4())
    .bind(job)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_media_derivative
          (derivative_ref,job_ref,derivative_kind,content_hash,storage_key)
         VALUES($1,$2,'ocr_text',$3,$4)",
    )
    .bind(derivative)
    .bind(job)
    .bind("a".repeat(64))
    .bind("ocr/manual/synthetic.json")
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_media_ocr_layout
          (layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,
           image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key)
         VALUES($1,$2,$3,$4,'cloud_manual','cloud-manual-v1:synthetic',2,2,$5,1,$6)",
    )
    .bind(layout)
    .bind(derivative)
    .bind(work)
    .bind(&blob)
    .bind("b".repeat(64))
    .bind("ocr/manual/synthetic-layout.json")
    .execute(db.pool())
    .await
    .unwrap();
    let source = serde_json::json!({
        "image":{"workRef":work,"imageRef":slot,"blobSha256":blob,"width":2,"height":2},
        "imageRole":"first_image","sequenceOrdinal":1
    });
    let observation = serde_json::json!({
        "producer":"cloud_vision","source":source,"rawSha256":"e".repeat(64),"result":{}
    });
    let fingerprint =
        linggan_evidence::ocr_adoption::content_hash(observation.to_string().as_bytes());
    let assessment = serde_json::json!({
        "producer":"cloud_vision","ruleVersion":"cloud-ocr-adoption.v1",
        "image":source["image"],"imageRole":"first_image","ordinal":1,
        "coverTitle":null,"coverTitleStatus":"pending","inputFingerprint":fingerprint,
        "imageText":"合成人工采用正文","humanVerified":false,
        "regions":[{"regionRef":"body-1","text":"合成人工采用正文","bbox":[0.1,0.1,0.9,0.9],
          "spans":[{"start":0,"end":8,"status":"adopted","reason":null}]}]
    });
    sqlx::query(
        "INSERT INTO linggan_media_ocr_adoption
          (adoption_ref,derivative_ref,layout_ref,rule_version,input_fingerprint,observation,
           assessment,line_ref_map,image_text,cover_title,state)
         VALUES($1,$2,$3,'cloud-ocr-adoption.v1',$4,$5,$6,'[]'::jsonb,$7,NULL,'NEEDS_REVIEW')",
    )
    .bind(adoption)
    .bind(derivative)
    .bind(layout)
    .bind(&fingerprint)
    .bind(&observation)
    .bind(&assessment)
    .bind("合成人工采用正文")
    .execute(db.pool())
    .await
    .unwrap();
    let config: Uuid = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='ocr_recognition'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let run = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ocr_recognition_run
          (run_ref,command_ref,content_public_ref,task_config_ref,state,model_label,endpoint_url,
           scope_sha256,total_images,completed_images)
         VALUES($1,$2,$3,$4,'succeeded','synthetic-cloud-model','https://loopback.invalid/ocr',$5,1,1)",
    )
    .bind(run)
    .bind(Uuid::new_v4())
    .bind(work)
    .bind(config)
    .bind("d".repeat(64))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ocr_recognition_item
          (item_ref,run_ref,position,slot_key,blob_sha256,materialization_ref,ordinal,role,state,
           original_url,storage_key,mime_type,byte_size,adoption_ref)
         VALUES($1,$2,0,$3,$4,$5,1,'cover','succeeded',$6,$7,$8,$9,$10)",
    )
    .bind(Uuid::new_v4())
    .bind(run)
    .bind(&slot)
    .bind(&blob)
    .bind(row.get::<Uuid, _>("materialization_ref"))
    .bind(row.get::<String, _>("local_asset_path"))
    .bind(row.get::<String, _>("storage_key"))
    .bind(row.get::<String, _>("mime_type"))
    .bind(row.get::<i64, _>("byte_size"))
    .bind(adoption)
    .execute(db.pool())
    .await
    .unwrap();

    let unadopted = research_dossier::read(&db, work).await.unwrap().unwrap();
    let machine_preview = unadopted
        .media
        .iter()
        .find(|media| media["ocrLayering"]["decisionSource"] == "cloud_vision_unverified")
        .unwrap();
    assert!(machine_preview["researchText"].is_null());
    assert!(machine_preview["ocrLayering"]["imageSubstantiveText"].is_null());
    assert!(machine_preview["ocrLayering"]["cleanCorpusEligible"] == false);

    // This row shape is permitted by the old recognition-item foreign key but is corrupt:
    // it points another work's run at this cloud adoption. The command must reject it.
    let other_work = ocr::admit(
        &db,
        "ocr-manual-adoption-other-work",
        1,
        &image(20),
        "image/png",
    )
    .await;
    let other_run = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ocr_recognition_run
          (run_ref,command_ref,content_public_ref,task_config_ref,state,model_label,endpoint_url,
           scope_sha256,total_images,completed_images)
         VALUES($1,$2,$3,$4,'succeeded','synthetic-cloud-model','https://loopback.invalid/ocr',$5,1,1)",
    )
    .bind(other_run)
    .bind(Uuid::new_v4())
    .bind(other_work)
    .bind(config)
    .bind("f".repeat(64))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ocr_recognition_item
          (item_ref,run_ref,position,slot_key,blob_sha256,materialization_ref,ordinal,role,state,
           original_url,storage_key,mime_type,byte_size,adoption_ref)
         SELECT $1,$2,0,slot_key,blob_sha256,materialization_ref,ordinal,role,state,
                original_url,storage_key,mime_type,byte_size,adoption_ref
           FROM linggan_ocr_recognition_item WHERE run_ref=$3",
    )
    .bind(Uuid::new_v4())
    .bind(other_run)
    .bind(run)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        adopt(
            &db,
            other_work,
            &Adopt {
                command_ref: Uuid::new_v4(),
                adoption_ref: adoption,
                expected_revision: 0,
                decision: "adopt".into(),
                field: "adopted_body".into(),
                text: Some("不得跨作品采用".into()),
                actor: "synthetic-reviewer".into(),
                reason: "owned proof: reject corrupt cross-work recognition item".into(),
            }
        )
        .await,
        Err(linggan_intelligence::ai_foundation::store::FoundationError::NotFound)
    ));

    let request = Adopt {
        command_ref: Uuid::new_v4(),
        adoption_ref: adoption,
        expected_revision: 0,
        decision: "adopt".into(),
        field: "adopted_body".into(),
        text: Some("合成人工采用正文".into()),
        actor: "synthetic-reviewer".into(),
        reason: "owned proof: compare saved cloud result with synthetic image".into(),
    };
    let receipt = adopt(&db, work, &request).await.unwrap();
    assert!(receipt["auditSampleRef"].is_null());
    assert_eq!(adopt(&db, work, &request).await.unwrap(), receipt);
    let conflict = Adopt {
        command_ref: Uuid::new_v4(),
        ..request.clone()
    };
    assert!(matches!(
        adopt(&db, work, &conflict).await,
        Err(linggan_intelligence::ai_foundation::store::FoundationError::Conflict)
    ));
    let review = sqlx::query(
        "SELECT sample_ref,issue_ref,source_kind,after_text
           FROM linggan_media_ocr_review WHERE adoption_ref=$1",
    )
    .bind(adoption)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(review.get::<Option<Uuid>, _>("sample_ref").is_none());
    assert!(review.get::<Option<Uuid>, _>("issue_ref").is_none());
    assert_eq!(review.get::<String, _>("source_kind"), "ocr_block");
    assert_eq!(
        review.get::<Option<String>, _>("after_text").unwrap(),
        "合成人工采用正文"
    );
    let stored_observation: serde_json::Value = sqlx::query_scalar(
        "SELECT observation FROM linggan_media_ocr_adoption WHERE adoption_ref=$1",
    )
    .bind(adoption)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(stored_observation, observation);
    let visible = read_run(&db, work).await.unwrap();
    assert_eq!(visible["images"][0]["reviewRevision"], 1);
    assert_eq!(
        visible["images"][0]["humanBody"]["text"],
        "合成人工采用正文"
    );
    let dossier = research_dossier::read(&db, work).await.unwrap().unwrap();
    let media = dossier
        .media
        .iter()
        .find(|media| media["ocrHumanAdoption"]["adopted_body"].is_object())
        .unwrap();
    assert_eq!(media["researchText"], "合成人工采用正文");
    assert_eq!(media["ocrLayering"]["decisionSource"], "human_ocr_review");
    assert_eq!(
        dossier.image_text_projection["projection"]["imageText"],
        "合成人工采用正文"
    );
    assert_eq!(
        dossier.image_text_projection["projection"]["fragments"][0]["source"]["sourceKind"],
        "block"
    );
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL and loopback Pi only"]
async fn second_image_schema_failure_stops_without_sending_the_third_image() {
    let db = fixture::db("ocr_recognition_second_failure_owned").await;
    apply_manual_ocr_migrations(&db).await;
    let work_key = "ocr-recognition-second-failure";
    let work = ocr::admit(&db, work_key, 1, &image(31), "image/png").await;
    ocr::admit(&db, work_key, 2, &image(32), "image/png").await;
    ocr::admit(&db, work_key, 3, &image(33), "image/png").await;
    let wire = endpoint::Endpoint::start_second_image_schema_failure().await;
    let adapter = adapter();
    let owner = ExecutorSession::acquire(&db, "synthetic-manual-ocr-failure", 4)
        .await
        .unwrap();
    configure_recognition(&db, &wire.url, &owner, &adapter).await;
    let scope = read_run(&db, work).await.unwrap()["scope"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let receipt = start_run(
        &db,
        work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: scope,
        },
    )
    .await
    .unwrap();
    for position in 0..2 {
        let item = queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .expect("only the first two images may be claimed");
        assert_eq!(item.position, position);
        ocr_recognition::execute(&db, &item, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
    }
    assert!(
        queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .is_none(),
        "the third source cannot be dispatched after the second receipt fails"
    );
    let run = read_run(&db, work).await.unwrap();
    assert_eq!(run["state"], "stopped");
    assert_eq!(run["completedImages"], 1);
    assert_eq!(run["failureCode"], "ocr_visual_schema_invalid");
    let images = run["images"].as_array().unwrap();
    assert_eq!(images[0]["state"], "succeeded");
    assert_eq!(images[1]["state"], "failed");
    assert_eq!(images[2]["state"], "unattempted");
    assert!(images[1]["rawJson"].as_str().unwrap().contains("malformed"));
    let raw_ref = images[1]["rawResultRef"].as_str().unwrap();
    assert!(
        PathBuf::from(std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT").unwrap())
            .join(raw_ref)
            .is_file(),
        "even schema-invalid bytes are retained as an inspectable raw receipt"
    );
    let skipped: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ai_batch_item_control c JOIN linggan_ai_batch_item i USING(item_ref)
         WHERE i.batch_ref=$1 AND c.state='skipped' AND c.failure_code='ocr_prior_item_failed'",
    )
    .bind(serde_json::from_value::<Uuid>(receipt["batchRef"].clone()).unwrap())
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(skipped, 1);
    assert_eq!(
        wire.requests.lock().unwrap().len(),
        3,
        "one actual loopback probe plus two images; no third egress"
    );
    wire.finish().await;
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL and loopback Pi only"]
async fn manual_ocr_cancel_and_attempted_lease_recovery_are_read_without_resend() {
    let db = fixture::db("ocr_recognition_control_projection_owned").await;
    apply_manual_ocr_migrations(&db).await;
    let adapter = adapter();
    let owner = ExecutorSession::acquire(&db, "synthetic-manual-ocr-control", 4)
        .await
        .unwrap();

    // Cancellation touches only the unified batch controls. The work read must reflect it
    // without manufacturing an OCR receipt or attempting any image.
    let cancelled_work =
        ocr::admit(&db, "ocr-recognition-cancelled", 1, &image(41), "image/png").await;
    ocr::admit(&db, "ocr-recognition-cancelled", 2, &image(42), "image/png").await;
    ocr::admit(&db, "ocr-recognition-cancelled", 3, &image(43), "image/png").await;
    let cancellation_wire = endpoint::Endpoint::start(false).await;
    configure_recognition(&db, &cancellation_wire.url, &owner, &adapter).await;
    let scope = read_run(&db, cancelled_work).await.unwrap()["scope"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let receipt = start_run(
        &db,
        cancelled_work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: scope,
        },
    )
    .await
    .unwrap();
    let batch: Uuid = serde_json::from_value(receipt["batchRef"].clone()).unwrap();
    let cancelled = cancel_batch(
        &db,
        &CancelBatch {
            command_ref: Uuid::new_v4(),
            batch_ref: batch,
            expected_revision: 0,
            reason: "synthetic cancellation proof".into(),
        },
    )
    .await
    .unwrap();
    assert!(cancelled.cancel_requested);
    let read_cancelled = read_run(&db, cancelled_work).await.unwrap();
    assert_eq!(read_cancelled["state"], "stopped");
    assert_eq!(read_cancelled["cancelRequested"], true);
    assert!(
        read_cancelled["images"]
            .as_array()
            .unwrap()
            .iter()
            .all(|image| image["state"] == "unattempted"),
        "cancelled control rows must not leave a queued or running image in the reader"
    );
    assert!(
        queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .is_none()
    );
    let resumed = start_run(
        &db,
        cancelled_work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: read_cancelled["scope"]["sha256"]
                .as_str()
                .unwrap()
                .to_owned(),
        },
    )
    .await
    .unwrap();
    assert_ne!(resumed["runRef"], receipt["runRef"]);
    let resumed_batch: Uuid = serde_json::from_value(resumed["batchRef"].clone()).unwrap();
    cancel_batch(
        &db,
        &CancelBatch {
            command_ref: Uuid::new_v4(),
            batch_ref: resumed_batch,
            expected_revision: 0,
            reason: "synthetic rerun cleanup".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        cancellation_wire.requests.lock().unwrap().len(),
        1,
        "only model probe reaches the loopback endpoint; cancellation sends no source image"
    );
    cancellation_wire.finish().await;

    // Once an invocation is durably marked attempted, expiry is unknown rather than retryable:
    // the executor cannot prove that a provider did not receive the image.
    let recovery_work =
        ocr::admit(&db, "ocr-recognition-recovery", 1, &image(51), "image/png").await;
    ocr::admit(&db, "ocr-recognition-recovery", 2, &image(52), "image/png").await;
    ocr::admit(&db, "ocr-recognition-recovery", 3, &image(53), "image/png").await;
    let mut recovery_wire = endpoint::Endpoint::start(true).await;
    configure_recognition(&db, &recovery_wire.url, &owner, &adapter).await;
    let recovery_scope = read_run(&db, recovery_work).await.unwrap()["scope"]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    start_run(
        &db,
        recovery_work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: recovery_scope,
        },
    )
    .await
    .unwrap();
    let first = queue::claim_next(&db, owner.fence())
        .await
        .unwrap()
        .expect("first recovery image claim");
    let execute_db = db.clone();
    let execute_adapter = adapter.clone();
    let execution = tokio::spawn(async move {
        ocr_recognition::execute(
            &execute_db,
            &first,
            &SyntheticModelSecrets,
            &execute_adapter,
        )
        .await
    });
    recovery_wire
        .seen
        .take()
        .expect("first image must reach the owned endpoint")
        .await
        .expect("endpoint observed first image request");
    let mut expire = db.pool().begin().await.unwrap();
    sqlx::query(
        "SELECT set_config('linggan.ai_executor_instance',$1,true),
                set_config('linggan.ai_executor_generation',$2,true),
                set_config('linggan.ai_executor_recovery','false',true)",
    )
    .bind(owner.fence().instance_ref().to_string())
    .bind(owner.fence().generation().to_string())
    .execute(&mut *expire)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_ai_batch_item_control SET lease_expires_at=clock_timestamp()-interval '1 second'
         WHERE state='running' AND invocation_ref IS NOT NULL",
    )
    .execute(&mut *expire)
    .await
    .unwrap();
    expire.commit().await.unwrap();
    assert_eq!(queue::recover_expired(&db, owner.fence()).await.unwrap(), 1);
    let unknown = read_run(&db, recovery_work).await.unwrap();
    assert_eq!(unknown["state"], "unknown");
    assert_eq!(unknown["failureCode"], "ai_dispatch_unknown");
    assert_eq!(unknown["completedImages"], 0);
    assert_eq!(unknown["images"][0]["state"], "unknown");
    assert_eq!(unknown["images"][1]["state"], "pending");
    assert!(
        queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .is_none(),
        "a later image must remain blocked after an uncertain earlier dispatch"
    );
    recovery_wire
        .release
        .take()
        .expect("gate release sender")
        .send(())
        .expect("release first synthetic response");
    assert!(
        execution.await.unwrap().is_err(),
        "the expired executor cannot settle a receipt after recovery claimed it unknown"
    );
    assert_eq!(
        recovery_wire.requests.lock().unwrap().len(),
        2,
        "one probe and one first image; neither recovery nor queue retry dispatches later images"
    );
    recovery_wire.finish().await;
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL and loopback Pi only"]
async fn manual_ocr_freezes_only_current_work_images_not_context_or_history() {
    let db = fixture::db("ocr_recognition_source_scope_owned").await;
    apply_manual_ocr_migrations(&db).await;
    let work_key = "ocr-recognition-source-scope";
    let work = ocr::admit(&db, work_key, 1, &image(61), "image/png").await;
    // The same slot receives a newer verified replica. The prior blob must never enter a
    // later manual OCR run merely because it remains in immutable media history.
    ocr::admit(&db, work_key, 1, &image(62), "image/png").await;
    ocr::admit(&db, work_key, 2, &image(63), "image/png").await;
    ocr::admit_with_slot_role(&db, work_key, 3, &image(64), "image/png", "avatar").await;
    ocr::admit_with_slot_role(&db, work_key, 4, &image(65), "image/png", "comment_image").await;
    let wire = endpoint::Endpoint::start(false).await;
    let adapter = adapter();
    let owner = ExecutorSession::acquire(&db, "synthetic-manual-ocr-source-scope", 4)
        .await
        .unwrap();
    configure_recognition(&db, &wire.url, &owner, &adapter).await;
    let ready = read_run(&db, work).await.unwrap();
    assert_eq!(ready["state"], "ready");
    assert_eq!(ready["scope"]["imageCount"], 2);
    let scope = ready["scope"]["sha256"].as_str().unwrap().to_owned();
    start_run(
        &db,
        work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: scope,
        },
    )
    .await
    .unwrap();
    let frozen: Vec<(String, String)> = sqlx::query_as(
        "SELECT slot_key,blob_sha256 FROM linggan_ocr_recognition_item
         WHERE run_ref=(SELECT run_ref FROM linggan_ocr_recognition_run
                        WHERE content_public_ref=$1 ORDER BY created_at DESC LIMIT 1)
         ORDER BY position",
    )
    .bind(work)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(frozen.len(), 2);
    assert!(
        frozen
            .iter()
            .all(|(slot, _)| !slot.ends_with(":3") && !slot.ends_with(":4"))
    );
    assert!(
        frozen
            .iter()
            .all(|(_, blob)| blob != &linggan_evidence::ocr_adoption::content_hash(&image(61))),
        "a historical replica for the same slot is outside the frozen dispatch source set"
    );
    for expected in 0..2 {
        let item = queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .expect("only current eligible work images are claimed");
        assert_eq!(item.position, expected);
        ocr_recognition::execute(&db, &item, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap();
    }
    assert!(
        queue::claim_next(&db, owner.fence())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        wire.requests.lock().unwrap().len(),
        3,
        "one probe plus exactly two current eligible images; avatar, comment image, and old replica stay local"
    );
    wire.finish().await;
}
