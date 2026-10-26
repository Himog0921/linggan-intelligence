//! Canonical media projection and live restriction proofs.
use super::{D, fixture};
use linggan_contracts::creator_discovery::CreatorScope;
use linggan_evidence::creator_discovery::load;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated PostgreSQL proof only"]
async fn batched_media_read_keeps_accepted_ocr_and_current_withdrawal() {
    use linggan_evidence::material_reading::{NewReading, Publication, publish_in};
    let db = fixture::proof_database("creator_discovery_batch_media").await;
    let mut refs = Vec::new();
    let mut image_readings = Vec::new();
    let mut image_versions = Vec::new();
    for (index, text) in [(1, "可用的 OCR 正文"), (2, "已经撤回的 OCR 正文")] {
        let content_id = format!("batch-media-{index}");
        let slot_key = format!("xhs:{content_id}:cover:1");
        let package = fixture::submit_package(
            &db,
            "content_detail",
            json!({"contentExternalId":content_id}),
            json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":content_id},"payload":{"title":format!("作品 {index}"),"authorId":format!("author-{index}")}}),
        ).await;
        let work_ref: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(&content_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        refs.push(work_ref);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
            .bind(Uuid::new_v4()).bind(work_ref).bind(D).bind(package).execute(db.pool()).await.unwrap();
        let observation_ref = Uuid::new_v4();
        fixture::submit_package(
            &db,
            "media_slots",
            json!({"contentExternalId":content_id}),
            json!({"kind":"media_slot","slotKey":slot_key,"observationRef":observation_ref,"slot":{"role":"cover","ordinal":1},"observation":{"externalUri":format!("https://media.example/{content_id}.jpg"),"candidateUris":[format!("https://media.example/{content_id}.jpg")],"observedAt":"2026-09-20T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":content_id}}),
        ).await;
        let blob = format!("{index:064x}");
        let admitted = linggan_evidence::admit_media_blob(
            &db,
            observation_ref,
            &blob,
            "image/jpeg",
            4096,
            &format!("blobs/batch/{index}"),
        )
        .await
        .unwrap();
        image_versions.push(admitted.materialization_ref);
        let job_ref: Uuid = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE slot_key=$1 AND processor_kind='image_ocr'")
            .bind(&slot_key).fetch_one(db.pool()).await.unwrap();
        let derivative_ref = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,24,$4)")
            .bind(derivative_ref).bind(job_ref).bind(format!("{:064x}", index + 10)).bind(format!("derivatives/batch/{index}.txt"))
            .execute(db.pool()).await.unwrap();
        let layout_ref = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr','batch-proof',1080,1440,$5,20,$6)")
            .bind(layout_ref).bind(derivative_ref).bind(work_ref).bind(&blob).bind(format!("{:064x}", index + 20)).bind(format!("derivatives/batch/{index}.json"))
            .execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_media_ocr_line(line_ref,layout_ref,ordinal,text_content,confidence,left_norm,top_norm,right_norm,bottom_norm) VALUES($1,$2,0,$3,0.98,0.1,0.2,0.9,0.3)")
            .bind(Uuid::new_v4()).bind(layout_ref).bind(text).execute(db.pool()).await.unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        let reading = publish_in(&mut tx,NewReading { content_public_ref:work_ref,unit_key:format!("image:{slot_key}"),kind:"image".into(),slot_key:Some(slot_key.clone()),page:None,source_version:admitted.materialization_ref.to_string(),source_state:"present".into(),derivative_ref:Some(derivative_ref),producer_ref:Some(job_ref),generated_by:"ordinary_ocr".into(),rule_version:"dossier-content.v2:synthetic-current-local".into(),blocks:vec![json!({"kind":"paragraph","text":text,"speaker":"unconfirmed"})],issues:vec![],source_location:json!({"materializationRef":admitted.materialization_ref,"blobSha256":blob}),publication:Publication::Automatic{expected_current:None} })
            .await
            .unwrap();
        assert!(reading.current);
        image_readings.push(reading.reading_ref);
        tx.commit().await.unwrap();
        sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason,occurred_at) VALUES($1,$2,'succeeded','batch proof',clock_timestamp())")
            .bind(Uuid::new_v4()).bind(job_ref).execute(db.pool()).await.unwrap();
        if index == 2 {
            sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','batch-proof','withdrawal',clock_timestamp())")
                .bind(Uuid::new_v4()).bind(derivative_ref).execute(db.pool()).await.unwrap();
        }
    }
    let scope: CreatorScope = serde_json::from_value(json!({"domain":D})).unwrap();
    let discovery = load(&db, &scope).await.unwrap();
    assert_eq!(discovery.works.len(), 2);
    let allowed = discovery
        .works
        .iter()
        .find(|work| work.work_ref == refs[0])
        .unwrap();
    assert!(allowed.search_text.contains("可用的 OCR 正文"));
    assert!(
        allowed
            .fragments
            .iter()
            .any(|fragment| fragment.field == "ocr" && fragment.text == "可用的 OCR 正文")
    );
    let image_fragment = allowed
        .fragments
        .iter()
        .find(|fragment| fragment.field == "ocr")
        .unwrap();
    assert_eq!(image_fragment.reading_ref, Some(image_readings[0]));
    assert_eq!(image_fragment.source_version, image_versions[0].to_string());
    assert_eq!(image_fragment.speaker, "unconfirmed");
    assert_eq!(image_fragment.structure["ref"], image_fragment.fragment_id);
    assert_eq!(image_fragment.structure["text"], "可用的 OCR 正文");
    assert!(
        image_fragment
            .fragment_id
            .starts_with(&format!("{}.{}.", refs[0], image_readings[0]))
    );
    let withdrawn = discovery
        .works
        .iter()
        .find(|work| work.work_ref == refs[1])
        .unwrap();
    assert!(!withdrawn.search_text.contains("已经撤回的 OCR 正文"));
    assert!(
        !withdrawn
            .fragments
            .iter()
            .any(|fragment| fragment.field == "ocr")
    );
    let detail = linggan_evidence::read_work_resource(&db, refs[1])
        .await
        .unwrap()
        .unwrap();
    assert!(
        detail.inspector["derivatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|derivative| derivative["dispositionState"] == "WITHDRAWN_OR_RESTRICTED")
    );
    assert!(
        !detail.inspector["derivatives"]
            .to_string()
            .contains("已经撤回的 OCR 正文"),
        "restricted OCR diagnostics expose metadata only, never a second transcription"
    );
    let available = linggan_evidence::read_work_resource(&db, refs[0])
        .await
        .unwrap()
        .unwrap();
    assert!(
        available.inspector["derivatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["processorKind"] == "image_ocr")
    );
    assert!(
        !available.inspector["derivatives"]
            .to_string()
            .contains("可用的 OCR 正文"),
        "OCR content is read from canonical, never derived diagnostics"
    );
    let first_slot = "xhs:batch-media-1:cover:1";
    let first_blob: String = sqlx::query_scalar(
        "SELECT blob_sha256 FROM linggan_media_processing_job WHERE slot_key=$1 LIMIT 1",
    )
    .bind(&first_slot)
    .fetch_one(db.pool())
    .await
    .unwrap();
    // ASR has a separately observed video source; image bytes cannot stand in for audio.
    let video_slot = "xhs:batch-media-1:video:1";
    let video_observation = Uuid::new_v4();
    fixture::submit_package(&db, "media_slots", json!({"contentExternalId":"batch-media-1"}), json!({
        "kind":"media_slot","slotKey":video_slot,"observationRef":video_observation,
        "slot":{"role":"video","ordinal":1},
        "observation":{"externalUri":"https://media.example/batch-media-1.mp4","candidateUris":["https://media.example/batch-media-1.mp4"],"observedAt":"2026-09-20T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":"batch-media-1"}
    })).await;
    let video_blob = "d".repeat(64);
    let video = linggan_evidence::admit_media_blob(
        &db,
        video_observation,
        &video_blob,
        "video/mp4",
        8192,
        "blobs/batch/video.mp4",
    )
    .await
    .unwrap();
    let asr_job: Uuid = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE slot_key=$1 AND processor_kind='asr'")
        .bind(video_slot).fetch_one(db.pool()).await.unwrap();
    let asr_derivative = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'asr_text',$3,24,'derivatives/batch/asr.txt')")
        .bind(asr_derivative).bind(asr_job).bind("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let audio_reading = publish_in(&mut tx, NewReading {
        content_public_ref: refs[0], unit_key: format!("audio:{video_slot}"), kind: "audio".into(),
        slot_key: Some(video_slot.into()), page: None, source_version: video.materialization_ref.to_string(),
        source_state: "present".into(), derivative_ref: Some(asr_derivative), producer_ref: Some(asr_job),
        generated_by: "asr".into(), rule_version: format!("{}:batch-asr", linggan_evidence::research_dossier::content::RULE_VERSION),
        blocks: vec![json!({"ref":"synthetic-asr","kind":"paragraph","text":"可用的 ASR 转录","speaker":"unconfirmed","time":{"startMs":0,"endMs":2500}})],
        issues: vec![], source_location: json!({"materializationRef":video.materialization_ref,"blobSha256":video_blob,"jobRef":asr_job}),
        publication: Publication::Automatic {expected_current:None},
    }).await.unwrap();
    assert!(audio_reading.current);
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,occurred_at) VALUES($1,$2,'succeeded',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(asr_job).execute(db.pool()).await.unwrap();
    let retired_job = Uuid::new_v4();
    let retired_derivative = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'image_ocr','batch-retired','full')")
        .bind(retired_job).bind(&first_blob).bind(&first_slot).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,24,'derivatives/batch/retired.txt')")
        .bind(retired_derivative).bind(retired_job).bind("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        .execute(db.pool()).await.unwrap();
    let retired_layout = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr','batch-proof',1080,1440,$5,20,'derivatives/batch/retired.json')")
        .bind(retired_layout).bind(retired_derivative).bind(refs[0]).bind(&first_blob).bind("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc")
        .execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_line(line_ref,layout_ref,ordinal,text_content,confidence,left_norm,top_norm,right_norm,bottom_norm) VALUES($1,$2,0,$3,0.98,0.1,0.2,0.9,0.3)")
        .bind(Uuid::new_v4()).bind(retired_layout).bind("不能出现的退役 OCR 文本").execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason) VALUES($1,'tesseract_replaced_by_paddleocr')")
        .bind(retired_job).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let retired_reading =
        publish_in(&mut tx,NewReading { content_public_ref:refs[0],unit_key:format!("image:{first_slot}"),kind:"image".into(),slot_key:Some(first_slot.to_owned()),page:None,source_version:image_versions[0].to_string(),source_state:"present".into(),derivative_ref:Some(retired_derivative),producer_ref:Some(retired_job),generated_by:"ordinary_ocr".into(),rule_version:"dossier-content.v2:synthetic-retired".into(),blocks:vec![json!({"kind":"paragraph","text":"不能出现的退役 OCR 文本","speaker":"unconfirmed"})],issues:vec![],source_location:json!({"materializationRef":image_versions[0],"blobSha256":first_blob}),publication:Publication::Automatic{expected_current:Some(image_readings[0])} })
            .await
            .unwrap();
    assert!(
        !retired_reading.current,
        "retirement prevents promotion even with the correct expected current reading"
    );
    tx.commit().await.unwrap();
    let with_asr = load(&db, &scope).await.unwrap();
    let first = with_asr
        .works
        .iter()
        .find(|w| w.work_ref == refs[0])
        .unwrap();
    assert!(first.search_text.contains("可用的 ASR 转录"));
    assert!(
        first
            .fragments
            .iter()
            .any(|fragment| fragment.field == "transcript" && fragment.text == "可用的 ASR 转录")
    );
    let audio_fragment = first
        .fragments
        .iter()
        .find(|fragment| fragment.field == "transcript")
        .unwrap();
    assert_eq!(audio_fragment.reading_ref, Some(audio_reading.reading_ref));
    assert_eq!(
        audio_fragment.source_version,
        video.materialization_ref.to_string()
    );
    assert_eq!(
        audio_fragment.structure["time"],
        json!({"startMs":0,"endMs":2500})
    );
    assert!(first.search_text.contains("可用的 OCR 正文"));
    assert!(!first.search_text.contains("不能出现的退役 OCR 文本"));
    let retired_detail = linggan_evidence::read_work_resource(&db, refs[0])
        .await
        .unwrap()
        .unwrap();
    assert!(
        retired_detail.inspector["derivatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["jobRef"] == retired_job.to_string() && d["state"] == "RETIRED")
    );
    // A new job after this read clock is not allowed to leak backward into the projection.
    let future_job = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope,created_at) VALUES($1,$2,$3,'asr','future-proof','full','2099-01-01')")
        .bind(future_job).bind(&video_blob).bind(video_slot).execute(db.pool()).await.unwrap();
    let unchanged = linggan_evidence::read_work_resource(&db, refs[0])
        .await
        .unwrap()
        .unwrap();
    assert!(
        !unchanged.inspector["derivatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["jobRef"] == future_job.to_string())
    );

    // The same slot can have multiple origin generations; that does not duplicate a job row.
    fixture::submit_package(&db, "media_slots", json!({"contentExternalId":"batch-media-1"}), json!({
        "kind":"media_slot","slotKey":first_slot,"observationRef":Uuid::new_v4(),
        "slot":{"role":"cover","ordinal":1},
        "observation":{"externalUri":"https://media.example/batch-media-1-v2.jpg","candidateUris":["https://media.example/batch-media-1-v2.jpg"],"observedAt":"2026-09-21T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":"batch-media-1"}
    })).await;
    let after_reobservation = linggan_evidence::read_work_resource(&db, refs[0])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after_reobservation.inspector["derivativesReceipt"]["total"],
        unchanged.inspector["derivativesReceipt"]["total"]
    );
    let after_reobservation_discovery = load(&db, &scope).await.unwrap();
    let after_reobservation_work = after_reobservation_discovery
        .works
        .iter()
        .find(|work| work.work_ref == refs[0])
        .unwrap();
    assert!(
        !after_reobservation_work
            .search_text
            .contains("可用的 OCR 正文"),
        "a new image generation cannot fall back to historical OCR"
    );
    assert!(
        after_reobservation_work
            .search_text
            .contains("可用的 ASR 转录"),
        "the independent video source remains current"
    );

    // 260 nonempty jobs force the exact per-work 256 + 1 page boundary.
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) SELECT gen_random_uuid(),$1,$2,'asr','batch-scale-'||n::text,'full' FROM generate_series(1,260) AS n")
        .bind(&video_blob).bind(video_slot).execute(db.pool()).await.unwrap();
    let page = linggan_evidence::read_work_resource(&db, refs[0])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        page.inspector["derivativesReceipt"]["total"]
            .as_u64()
            .unwrap(),
        unchanged.inspector["derivativesReceipt"]["total"]
            .as_u64()
            .unwrap()
            + 260
    );
    assert_eq!(page.inspector["derivativesReceipt"]["returned"], json!(256));
    assert_eq!(page.inspector["derivativesReceipt"]["truncated"], true);
    assert!(
        page.inspector["derivativesReceipt"]["nextCursor"]
            .as_str()
            .is_some_and(|cursor| cursor.starts_with("job:"))
    );
    assert_eq!(page.inspector["derivatives"].as_array().unwrap().len(), 256);
    let scale_order: Vec<Uuid> = sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE processor_version LIKE 'batch-scale-%' ORDER BY created_at,job_ref")
        .fetch_all(db.pool()).await.unwrap();
    let scale_set: std::collections::HashSet<_> = scale_order.iter().copied().collect();
    let actual_scale_order: Vec<Uuid> = page.inspector["derivatives"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| {
            d["jobRef"]
                .as_str()
                .and_then(|value| value.parse::<Uuid>().ok())
        })
        .filter(|job_ref| scale_set.contains(job_ref))
        .collect();
    assert!(!actual_scale_order.is_empty());
    assert_eq!(
        actual_scale_order,
        scale_order[..actual_scale_order.len()],
        "same-timestamp jobs use stable job-ref ordering"
    );
    assert_eq!(
        load(&db, &scope).await.unwrap().works.len(),
        2,
        "pagination within one work must not remove another work"
    );

    // Live disposition is key-wide: derivative, slot, and blob keys can each restrict a read.
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','batch-proof','slot withdrawn',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(video_slot).execute(db.pool()).await.unwrap();
    let slot_restricted = load(&db, &scope).await.unwrap();
    assert!(
        !slot_restricted
            .works
            .iter()
            .find(|w| w.work_ref == refs[0])
            .unwrap()
            .search_text
            .contains("可用的 ASR 转录")
    );
    let second_blob: String = sqlx::query_scalar("SELECT blob_sha256 FROM linggan_media_processing_job WHERE slot_key='xhs:batch-media-2:cover:1' LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,blob_sha256,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','batch-proof','blob withdrawn',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(second_blob).execute(db.pool()).await.unwrap();
    let blob_restricted = linggan_evidence::read_work_resource(&db, refs[1])
        .await
        .unwrap()
        .unwrap();
    assert!(
        blob_restricted.inspector["derivatives"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["dispositionState"] == "WITHDRAWN_OR_RESTRICTED")
    );
}
