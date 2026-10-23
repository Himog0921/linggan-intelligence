//! Real PostgreSQL proofs for the canonical producer/selection/review boundary.
#[path = "material_reading/review_opinions.rs"]
mod review_opinions;
#[path = "support/research_dossier_fixture.rs"]
mod support;
use linggan_evidence::{material_reading::*, *};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn now(db: &Database) -> String {
    sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap()
}
async fn current(db: &Database, work: Uuid) -> Value {
    let at = now(db).await;
    let mut conn = db.pool().acquire().await.unwrap();
    read_current_in(&mut conn, &[work], &at).await.unwrap()[&work]
        .iter()
        .find(|m| m["kind"] == "image")
        .unwrap()
        .clone()
}
fn reference(value: &Value) -> Uuid {
    serde_json::from_value(value.clone()).unwrap()
}
async fn publish(
    db: &Database,
    work: Uuid,
    producer: Uuid,
    blocks: Vec<Value>,
    issues: Vec<Value>,
    publication: Publication,
) -> PublishedReading {
    let row = sqlx::query("SELECT u.*,r.derivative_ref,r.source_location FROM linggan_material_unit u JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref WHERE u.content_public_ref=$1 AND u.kind='image'")
        .bind(work).fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let result = publish_in(
        &mut tx,
        NewReading {
            content_public_ref: work,
            unit_key: row.get("unit_key"),
            kind: "image".into(),
            slot_key: row.get("slot_key"),
            page: row.get("page_ordinal"),
            source_version: row.get("source_version"),
            source_state: "present".into(),
            derivative_ref: row.get("derivative_ref"),
            producer_ref: Some(producer),
            generated_by: if matches!(publication, Publication::Candidate) {
                "cloud_vision"
            } else {
                "ordinary_ocr"
            }
            .into(),
            rule_version: "dossier-content.v2:synthetic-producer".into(),
            blocks,
            issues,
            source_location: row.get("source_location"),
            publication,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    result
}
fn request(
    candidate: PublishedReading,
    expected: Option<Uuid>,
    action: &str,
) -> ReviewReadingRequest {
    ReviewReadingRequest {
        command_ref: Uuid::new_v4(),
        expected_current_revision_ref: expected,
        candidate_revision_ref: candidate.reading_ref,
        action: action.into(),
        edited_blocks: None,
        verified: false,
        resolved_issue_keys: vec![],
        actor: "synthetic-reviewer".into(),
        reason: "Compare the complete synthetic source image".into(),
    }
}
fn paragraph(text: &str) -> Vec<Value> {
    vec![json!({"kind":"paragraph","text":text,"speaker":"unconfirmed"})]
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn ordinary_ocr_is_current_and_manual_candidates_require_explicit_cas_review() {
    let db = support::fixture::proof_database("dossier_reading_cas").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let original = current(&db, work).await;
    let original_ref = reference(&original["currentRevisionRef"]);
    assert_eq!(original["reading"]["generatedBy"], "ordinary_ocr");
    assert_eq!(original["reading"]["adoption"], "automatic");
    assert_eq!(original["blocks"].as_array().unwrap().len(), 6);
    assert!(
        original["blocks"]
            .to_string()
            .contains("用途不清的背景文字"),
        "source text is retained even when an old layering decision excluded it"
    );
    let producer = Uuid::new_v4();
    let candidate = publish(
        &db,
        work,
        producer,
        paragraph("人可以核对的完整候选，不删除 30 mg 的否定。"),
        vec![],
        Publication::Candidate,
    )
    .await;
    assert!(!candidate.current);
    assert_eq!(
        current(&db, work).await["currentRevisionRef"],
        json!(original_ref)
    );
    assert_eq!(
        publish(
            &db,
            work,
            producer,
            paragraph("人可以核对的完整候选，不删除 30 mg 的否定。"),
            vec![],
            Publication::Candidate
        )
        .await
        .reading_ref,
        candidate.reading_ref,
        "producer retries reuse exactly one immutable candidate"
    );
    assert_eq!(read_candidates(&db, work).await.unwrap().len(), 1);
    let mut adopt = request(candidate, Some(original_ref), "adopt");
    adopt.verified = true;
    let accepted = review_reading(&db, candidate.unit_ref, &adopt)
        .await
        .unwrap();
    assert_eq!(accepted["currentRevisionRef"], json!(candidate.reading_ref));
    assert_eq!(
        review_reading(&db, candidate.unit_ref, &adopt)
            .await
            .unwrap(),
        accepted
    );
    let mut changed_actor = adopt.clone();
    changed_actor.actor = "another-actor".into();
    assert!(matches!(
        review_reading(&db, candidate.unit_ref, &changed_actor).await,
        Err(ReadingError::Conflict)
    ));
    assert!(matches!(
        review_reading(
            &db,
            candidate.unit_ref,
            &request(candidate, Some(original_ref), "adopt")
        )
        .await,
        Err(ReadingError::Conflict)
    ));
    assert!(read_candidates(&db, work).await.unwrap().is_empty());
    let late = publish(
        &db,
        work,
        Uuid::new_v4(),
        paragraph("晚到的自动识别"),
        vec![],
        Publication::Automatic {
            expected_current: Some(candidate.reading_ref),
        },
    )
    .await;
    assert!(
        !late.current,
        "even a correct expected pointer cannot overwrite an explicit human selection"
    );
    let selected = current(&db, work).await;
    assert_eq!(selected["reading"]["adoption"], "human_adopted");
    assert_eq!(selected["reading"]["verification"], "verified");
    let frozen = now(&db).await;
    let mut conn = db.pool().acquire().await.unwrap();
    let exact = read_versions_in(&mut conn, &[work], &frozen, &[original_ref])
        .await
        .unwrap();
    assert_eq!(exact[&work][0]["blocks"], original["blocks"]);
    assert_eq!(
        exact[&work][0]["reading"]["adoption"], "automatic",
        "later unit metadata cannot relabel an old immutable reading"
    );
    drop(conn);
    assert!(
        sqlx::query("UPDATE linggan_material_reading SET blocks='[]' WHERE reading_ref=$1")
            .bind(original_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM linggan_material_reading_command WHERE command_ref=$1")
            .bind(adopt.command_ref)
            .execute(db.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn edits_preserve_observed_locations_and_source_issues_with_new_immutable_references() {
    let db = support::fixture::proof_database("dossier_reading_edit").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let old = reference(&current(&db, work).await["currentRevisionRef"]);
    let blocks = vec![
        json!({"ref":"candidate.block","kind":"paragraph","speaker":"unconfirmed","text":"不是 30 mg",
        "bbox":[0.1,0.2,0.4,0.1],"issues":[{"key":"unclear","code":"uncertain_reading","message":"请对照原图","resolution":"reading_edit","blockRef":"candidate.block"}]}),
    ];
    let issues = vec![
        json!({"key":"missing_source","code":"sampling_incomplete","message":"来源尚未完整取得","resolution":"source_refresh"}),
    ];
    let candidate = publish(
        &db,
        work,
        Uuid::new_v4(),
        blocks,
        issues,
        Publication::Candidate,
    )
    .await;
    let candidate_data = read_candidates(&db, work)
        .await
        .unwrap()
        .into_iter()
        .find(|c| c["candidateRevisionRef"] == json!(candidate.reading_ref))
        .unwrap();
    let base: Vec<Value> = serde_json::from_value(candidate_data["blocks"].clone()).unwrap();
    let mut edit = request(candidate, Some(old), "edit");
    edit.edited_blocks = Some(base.clone());
    edit.edited_blocks.as_mut().unwrap()[0]["bbox"] = json!([0.5, 0.2, 0.4, 0.1]);
    assert!(matches!(
        review_reading(&db, candidate.unit_ref, &edit).await,
        Err(ReadingError::Invalid)
    ));
    edit.edited_blocks = Some(base.clone());
    edit.edited_blocks.as_mut().unwrap()[0]["issues"][0]["resolution"] = json!("source_refresh");
    assert!(matches!(
        review_reading(&db, candidate.unit_ref, &edit).await,
        Err(ReadingError::Invalid)
    ));
    edit.edited_blocks = Some(base.clone());
    edit.resolved_issue_keys = vec!["missing_source".into()];
    assert!(matches!(
        review_reading(&db, candidate.unit_ref, &edit).await,
        Err(ReadingError::Invalid)
    ));
    edit.resolved_issue_keys = vec!["unclear".into()];
    edit.edited_blocks.as_mut().unwrap()[0]["text"] = json!("核对原图：不是 30 mg。");
    edit.edited_blocks.as_mut().unwrap().push(json!({"kind":"table","columns":["项目","数值"],"rows":[["剂量","30 mg"]],"speaker":"unconfirmed"}));
    edit.verified = true;
    let receipt = review_reading(&db, candidate.unit_ref, &edit)
        .await
        .unwrap();
    let selected = current(&db, work).await;
    assert_eq!(
        selected["currentRevisionRef"],
        receipt["currentRevisionRef"]
    );
    assert_ne!(selected["currentRevisionRef"], json!(candidate.reading_ref));
    assert_eq!(selected["reading"]["generatedBy"], "human_edit");
    assert!(
        selected["blocks"][0].get("bbox").is_none(),
        "changed text cannot inherit an unproven location"
    );
    assert_eq!(selected["blocks"][1]["kind"], "table");
    assert_eq!(selected["issues"][0]["resolution"], "source_refresh");
    assert!(
        selected["blocks"][0]["issues"]
            .as_array()
            .is_none_or(Vec::is_empty)
    );
    let new_ref = selected["currentRevisionRef"].as_str().unwrap();
    for (index, block) in selected["blocks"].as_array().unwrap().iter().enumerate() {
        assert_eq!(block["ref"], format!("{work}.{new_ref}.{index}"));
    }
    let original: Value =
        sqlx::query_scalar("SELECT blocks FROM linggan_material_reading WHERE reading_ref=$1")
            .bind(candidate.reading_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(original, candidate_data["blocks"]);
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn keep_is_a_human_decision_and_current_restrictions_apply_to_exact_history() {
    let db = support::fixture::proof_database("dossier_reading_restriction").await;
    let work = support::work(&db).await;
    let derivative = support::ordinary_ocr(&db).await;
    let original = current(&db, work).await;
    let old = reference(&original["currentRevisionRef"]);
    let candidate = publish(
        &db,
        work,
        Uuid::new_v4(),
        paragraph("另一候选"),
        vec![],
        Publication::Candidate,
    )
    .await;
    let keep = request(candidate, Some(old), "keep");
    let receipt = review_reading(&db, candidate.unit_ref, &keep)
        .await
        .unwrap();
    assert_eq!(receipt["currentRevisionRef"], json!(old));
    assert_eq!(receipt["adoption"], "human_adopted");
    assert!(
        read_candidates(&db, work).await.unwrap().is_empty(),
        "a deliberately kept candidate is not offered repeatedly"
    );
    let frozen = now(&db).await;
    record_derivative_disposition(
        &db,
        derivative,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-policy",
        "synthetic current restriction proof",
    )
    .await
    .unwrap();
    let restricted = current(&db, work).await;
    assert_eq!(restricted["sourceState"], "restricted");
    assert_eq!(restricted["blocks"], json!([]));
    assert!(restricted.get("originalUrl").is_none());
    let mut conn = db.pool().acquire().await.unwrap();
    assert!(
        read_versions_in(&mut conn, &[work], &frozen, &[old, candidate.reading_ref])
            .await
            .unwrap()[&work]
            .is_empty()
    );
    drop(conn);
    assert!(read_candidates(&db, work).await.unwrap().is_empty());
    assert!(matches!(
        review_reading(
            &db,
            candidate.unit_ref,
            &request(candidate, Some(old), "adopt")
        )
        .await,
        Err(ReadingError::SourceChanged)
    ));
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn new_source_generation_blocks_stale_adoption_without_substituting_historical_text() {
    let db = support::fixture::proof_database("dossier_reading_source").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let original = current(&db, work).await;
    let old = reference(&original["currentRevisionRef"]);
    let candidate = publish(
        &db,
        work,
        Uuid::new_v4(),
        paragraph("上一代图片候选"),
        vec![],
        Publication::Candidate,
    )
    .await;
    let frozen = now(&db).await;
    support::fixture::submit_package(&db,"media_slots",json!({"contentExternalId":"dossier-note"}),json!({"kind":"media_slot","slotKey":"xhs:dossier-note:image:1","observationRef":Uuid::new_v4(),"slot":{"role":"image","ordinal":1},"observation":{"externalUri":"https://media.example/new-generation.jpg","candidateUris":["https://media.example/new-generation.jpg"],"observedAt":"2026-10-23T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"}})).await;
    assert!(matches!(
        review_reading(
            &db,
            candidate.unit_ref,
            &request(candidate, Some(old), "adopt")
        )
        .await,
        Err(ReadingError::SourceChanged)
    ));
    assert!(read_candidates(&db, work).await.unwrap().is_empty());
    let historical_version = research_dossier::read_version(&db, work, old)
        .await
        .unwrap()
        .unwrap();
    assert!(
        historical_version.current_revision_ref.is_none(),
        "a stored pointer from an older generation is not current"
    );
    assert_eq!(historical_version.material["blocks"], original["blocks"]);
    let mut conn = db.pool().acquire().await.unwrap();
    let exact = read_versions_in(&mut conn, &[work], &frozen, &[old])
        .await
        .unwrap();
    assert_eq!(exact[&work][0]["blocks"], original["blocks"]);
    drop(conn);
    let dossier = research_dossier::read(&db, work).await.unwrap().unwrap();
    assert!(
        !dossier
            .materials
            .iter()
            .filter(|m| m["kind"] == "image")
            .any(|m| m["currentRevisionRef"] == json!(old)),
        "new source generation never falls back to old OCR"
    );
}

async fn complete_claim(db: &Database, kind: &str, expected_source: Uuid) {
    let worker = Uuid::new_v4();
    let MediaProcessingClaimOutcome::Claimed(claim) =
        claim_media_processing_work(db, worker, &[kind.into()])
            .await
            .unwrap()
    else {
        panic!("matching component must remain claimable")
    };
    let frozen: String = sqlx::query_scalar(
        "SELECT reading_source_version FROM linggan_media_processing_work WHERE work_ref=$1",
    )
    .bind(claim.work_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(frozen, expected_source.to_string());
    let text = "完整组件文字，不能丢失否定与数值 30。";
    let hash = ocr_adoption::content_hash(text.as_bytes());
    if kind == "image_ocr" {
        complete_media_processing_ocr(
            db,
            &claim,
            worker,
            &OcrCompletionInput {
                engine_version: "synthetic-component-proof".into(),
                image_width: 100,
                image_height: 100,
                raw_text: text.into(),
                raw_content_hash: hash.clone(),
                raw_storage_key: format!("synthetic/{}/raw.txt", claim.job_ref),
                layout_content_hash: hash,
                layout_byte_size: 12,
                layout_storage_key: format!("synthetic/{}/layout.json", claim.job_ref),
                lines: vec![OcrLineInput {
                    text: text.into(),
                    confidence: 0.99,
                    bbox_norm: [0.1, 0.1, 0.8, 0.2],
                }],
                layering: OcrLayeringInput {
                    layer_version: "rules-v1".into(),
                    state: "PARTIAL".into(),
                    cover_headline: None,
                    image_substantive_text: None,
                    retained_ordinals: vec![0],
                    headline_ordinals: vec![],
                    substantive_ordinals: vec![],
                    excluded_lines: vec![],
                },
            },
        )
        .await
        .unwrap();
    } else {
        let derivative = if kind == "asr" {
            "asr_text"
        } else {
            "frame_ocr_text"
        };
        let time = if kind == "asr" {
            json!({"startMs":1250,"endMs":3820})
        } else {
            json!({"startMs":11600,"endMs":11600})
        };
        complete_media_processing_text(
            db,
            &claim,
            worker,
            derivative,
            &hash,
            text.len() as i64,
            &format!("synthetic/{}/text.txt", claim.job_ref),
            text,
            text,
            Some("zh-Hans"),
            material_reading::producers::ReadingContent {
                blocks: vec![
                    json!({"kind":"paragraph","text":text,"speaker":"unconfirmed","time":time}),
                ],
                issues: vec![],
                source_state: "present".into(),
                source_location: json!({"timing":"synthetic_observed_times"}),
            },
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn live_photo_components_are_independent_in_both_completion_orders_and_byte_cleanup_preserves_text()
 {
    let db = support::fixture::proof_database("dossier_reading_components").await;
    for (index, still_last) in [false, true].into_iter().enumerate() {
        let key = format!("live-proof-{index}");
        support::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":key}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":key},"payload":{"title":"Live Photo component proof","bodyText":"组件互不替代。"}})).await;
        let observation = Uuid::new_v4();
        let slot = format!("xhs:{key}:live_photo:1");
        let still_uri = format!("https://media.example/{key}/still.jpg");
        let motion_uri = format!("https://media.example/{key}/motion.mp4");
        let target = json!({"contentExternalId":key});
        support::fixture::submit_custom_package(&db,"xhs",&["media_slots"],target.clone(),"media_slots","xhs",
            json!({"target":{"contentExternalId":key,"imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":1}},"layers":[support::fixture::coverage_layer("media_slots",1)]}),
            vec![json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"live_photo","ordinal":1,"displayOrdinal":1,"displayOrderBasis":"platform_explicit"},"observation":{"externalUri":motion_uri,"candidateUris":[motion_uri,still_uri],"components":{"still":{"candidateUris":[still_uri]},"motion":{"candidateUris":[motion_uri]}},"observedAt":"2026-10-23T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":key}})]).await;
        let mut still = None;
        let mut motion = None;
        for image in if still_last {
            [false, true]
        } else {
            [true, false]
        } {
            let hash = format!("{:064x}", 100 + index * 2 + usize::from(image));
            let admission = admit_media_blob(
                &db,
                observation,
                &hash,
                if image { "image/jpeg" } else { "video/mp4" },
                12,
                &format!("synthetic/{hash}"),
            )
            .await
            .unwrap();
            if image {
                still = Some(admission.materialization_ref)
            } else {
                motion = Some(admission.materialization_ref)
            };
        }
        ensure_media_processing_work(&db).await.unwrap();
        complete_claim(&db, "image_ocr", still.unwrap()).await;
        complete_claim(&db, "asr", motion.unwrap()).await;
        complete_claim(&db, "video_frame_ocr", motion.unwrap()).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(&key)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let at = now(&db).await;
        let mut conn = db.pool().acquire().await.unwrap();
        let readings = read_current_in(&mut conn, &[work], &at).await.unwrap();
        for (kind, source) in [
            ("image", still.unwrap()),
            ("audio", motion.unwrap()),
            ("video_frame", motion.unwrap()),
        ] {
            let material = readings[&work].iter().find(|m| m["kind"] == kind).unwrap();
            assert_eq!(material["sourceVersion"], source.to_string());
            assert!(material["currentRevisionRef"].is_string());
            assert!(material["blocks"].to_string().contains("不能丢失"));
        }
        drop(conn);
        record_materialization_disposition(
            &db,
            still.unwrap(),
            MaterialMediaDisposition::BytesCleaned,
            "synthetic-retention",
            "bytes removed, immutable transcription retained",
        )
        .await
        .unwrap();
        let material = current(&db, work).await;
        assert_eq!(material["sourceState"], "present");
        assert!(!material["blocks"].as_array().unwrap().is_empty());
        assert!(material.get("originalUrl").is_none());
        let blob: String = sqlx::query_scalar(
            "SELECT blob_sha256 FROM linggan_media_materialization WHERE materialization_ref=$1",
        )
        .bind(still.unwrap())
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'image_ocr',$4,'full_blob')")
            .bind(Uuid::new_v4()).bind(blob).bind(&slot).bind(ocr_adoption_persistence::PROCESSOR_VERSION).execute(db.pool()).await.unwrap();
        ensure_media_processing_work(&db).await.unwrap();
        assert!(
            matches!(
                claim_media_processing_work_for_version(
                    &db,
                    Uuid::new_v4(),
                    &["image_ocr".into()],
                    Some(ocr_adoption_persistence::PROCESSOR_VERSION)
                )
                .await
                .unwrap(),
                MediaProcessingClaimOutcome::Retired {
                    reason: "source_media_unavailable",
                    ..
                }
            ),
            "byte cleanup preserves text but prevents a new image-processing lease"
        );
    }
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn complete_manifest_removes_old_candidates_and_later_partial_observation_only_adds_itself() {
    let db = support::fixture::proof_database("dossier_reading_manifest").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let original = current(&db, work).await;
    let old = reference(&original["currentRevisionRef"]);
    let candidate = publish(
        &db,
        work,
        Uuid::new_v4(),
        paragraph("旧页候选"),
        vec![],
        Publication::Candidate,
    )
    .await;
    let frozen = now(&db).await;
    let target = json!({"contentExternalId":"dossier-note"});
    support::fixture::submit_custom_package(&db,"xhs",&["media_slots"],target.clone(),"media_slots","xhs",
        json!({"target":{"contentExternalId":"dossier-note","imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":0}},"layers":[support::fixture::coverage_layer("media_slots",0)]}),vec![]).await;
    assert!(read_candidates(&db, work).await.unwrap().is_empty());
    assert!(matches!(
        review_reading(
            &db,
            candidate.unit_ref,
            &request(candidate, Some(old), "adopt")
        )
        .await,
        Err(ReadingError::SourceChanged)
    ));
    let removed_version = research_dossier::read_version(&db, work, old)
        .await
        .unwrap()
        .unwrap();
    assert!(
        removed_version.current_revision_ref.is_none(),
        "a removed slot cannot expose its historical reading as current"
    );
    let current_active: bool = sqlx::query_scalar(
        "SELECT linggan_dossier_slot_active($1,'xhs:dossier-note:image:1',scope_001_now())",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!current_active);
    let mut conn = db.pool().acquire().await.unwrap();
    let current_readings = read_current_in(&mut conn, &[work], &now(&db).await)
        .await
        .unwrap();
    assert!(
        !current_readings[&work]
            .iter()
            .any(|m| m["currentRevisionRef"] == json!(old))
    );
    drop(conn);
    support::fixture::submit_package(&db,"media_slots",target,json!({"kind":"media_slot","slotKey":"xhs:dossier-note:image:2","observationRef":Uuid::new_v4(),"slot":{"role":"image","ordinal":2},"observation":{"externalUri":"https://media.example/partial-new-page.jpg","candidateUris":["https://media.example/partial-new-page.jpg"],"observedAt":"2026-10-23T00:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"}})).await;
    let active:(bool,bool)=sqlx::query_as("SELECT linggan_dossier_slot_active($1,'xhs:dossier-note:image:1',scope_001_now()),linggan_dossier_slot_active($1,'xhs:dossier-note:image:2',scope_001_now())").bind(work).fetch_one(db.pool()).await.unwrap();
    assert_eq!(active, (false, true));
    assert!(read_candidates(&db, work).await.unwrap().is_empty());
    let mut conn = db.pool().acquire().await.unwrap();
    assert_eq!(
        read_versions_in(&mut conn, &[work], &frozen, &[old])
            .await
            .unwrap()[&work][0]["blocks"],
        original["blocks"]
    );
}
