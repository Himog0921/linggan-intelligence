//! The cloud OCR entry uses exactly the canonical material review command and history.
use super::*;
use linggan_evidence::{material_reading::*, research_dossier};
use serde_json::{Value, json};

fn command(candidate: &Value, action: &str, expected: Option<Uuid>) -> ReviewReadingRequest {
    ReviewReadingRequest {
        command_ref: Uuid::new_v4(),
        expected_current_revision_ref: expected,
        candidate_revision_ref: serde_json::from_value(candidate["candidateRevisionRef"].clone())
            .unwrap(),
        action: action.into(),
        actor: "synthetic-reviewer".into(),
        reason: "核对完整合成原图与已保存云端候选".into(),
        edited_blocks: None,
        verified: false,
        resolved_issue_keys: vec![],
    }
}
async fn selection_count(db: &linggan_storage_postgres::Database, unit: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading_selection WHERE unit_ref=$1")
        .bind(unit)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn withdraw_independent_local_reading(
    db: &linggan_storage_postgres::Database,
    unit: Uuid,
) -> Uuid {
    use linggan_evidence::*;
    ensure_media_processing_work(db).await.unwrap();
    let worker = Uuid::new_v4();
    let MediaProcessingClaimOutcome::Claimed(claim) =
        claim_media_processing_work(db, worker, &["image_ocr".into()])
            .await
            .unwrap()
    else {
        panic!("ordinary OCR claim");
    };
    complete_media_processing_ocr(
        db,
        &claim,
        worker,
        &OcrCompletionInput {
            engine_version: "synthetic-independent-ordinary".into(),
            image_width: 2,
            image_height: 2,
            raw_text: "随后仅撤回这份本地衍生读本".into(),
            raw_content_hash: "d".repeat(64),
            raw_storage_key: "derived/independent-current.txt".into(),
            layout_content_hash: "e".repeat(64),
            layout_byte_size: 12,
            layout_storage_key: "derived/independent-current.json".into(),
            lines: vec![OcrLineInput {
                text: "随后仅撤回这份本地衍生读本".into(),
                confidence: 0.98,
                bbox_norm: [0.0, 0.0, 1.0, 1.0],
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
    let (reading, derivative): (Uuid, Uuid) = sqlx::query_as(
        "SELECT r.reading_ref,r.derivative_ref FROM linggan_material_unit u JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref WHERE u.unit_ref=$1",
    ).bind(unit).fetch_one(db.pool()).await.unwrap();
    record_derivative_disposition(
        db,
        derivative,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-reviewer",
        "Only the independent local derivative is withdrawn",
    )
    .await
    .unwrap();
    reading
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL, synthetic image and loopback Pi only"]
async fn cloud_manual_human_adoption_is_append_only_and_not_an_audit_sample() {
    let db = fixture::db("ocr_manual_review_canonical").await;
    let work = ocr::admit(
        &db,
        "ocr-canonical-human-review",
        1,
        &image(71),
        "image/png",
    )
    .await;
    let wire = endpoint::Endpoint::start(false).await;
    let adapter = adapter();
    let owner = ExecutorSession::acquire(&db, "synthetic-canonical-review", 4)
        .await
        .unwrap();
    configure_recognition(&db, &wire.url, &owner, &adapter).await;
    let ready = read_run(&db, work).await.unwrap();
    start_run(
        &db,
        work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: ready["scope"]["sha256"].as_str().unwrap().into(),
            slot_keys: selected_slots(&ready),
        },
    )
    .await
    .unwrap();
    let item = queue::claim_next(&db, owner.fence())
        .await
        .unwrap()
        .unwrap();
    ocr_recognition::execute(&db, &item, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let saved = read_run(&db, work).await.unwrap();
    let candidate = read_candidates(&db, work).await.unwrap().remove(0);
    let unit = serde_json::from_value(candidate["unitRef"].clone()).unwrap();
    let candidate_ref = serde_json::from_value(candidate["candidateRevisionRef"].clone()).unwrap();
    assert!(candidate["currentRevisionRef"].is_null());
    let plan = research_dossier::plan(&db, work).await.unwrap().unwrap();
    assert_eq!(plan["layoutCount"], 0);
    assert_eq!(plan["pendingLayouts"], 0);
    assert_eq!(
        plan["needsOcr"], 0,
        "saved cloud text awaits review, not another OCR call"
    );
    assert_eq!(plan["pendingReviewCount"], 1);
    let rebuilt =
        research_dossier::relayer_work(&db, work, plan["expectedRevision"].as_str().unwrap())
            .await
            .unwrap();
    assert_eq!(
        rebuilt["scopeCount"], 0,
        "cloud layouts cannot enter the local relayer"
    );
    let withdrawn = withdraw_independent_local_reading(&db, unit).await;
    let exact = research_dossier::read_version(&db, work, candidate_ref)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exact.current_revision_ref, None);
    assert_eq!(
        exact.material["currentRevisionRef"],
        candidate["candidateRevisionRef"]
    );
    assert!(
        research_dossier::read_version(&db, work, withdrawn)
            .await
            .unwrap()
            .is_none()
    );
    let initial = selection_count(&db, unit).await;
    for action in ["pending", "reject", "pending"] {
        let opinion = command(&candidate, action, None);
        let receipt = review_reading(&db, unit, &opinion).await.unwrap();
        assert!(receipt["currentRevisionRef"].is_null());
        assert!(receipt["adoption"].is_null() && receipt["verification"].is_null());
        assert_eq!(selection_count(&db, unit).await, initial);
        let stored_current: Option<Uuid> = sqlx::query_scalar(
            "SELECT current_reading_ref FROM linggan_material_unit WHERE unit_ref=$1",
        )
        .bind(unit)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            stored_current,
            Some(withdrawn),
            "an opinion cannot mutate the underlying selection history"
        );
        let visible = read_candidates(&db, work).await.unwrap();
        assert_eq!(visible.len(), usize::from(action == "pending"));
        if action == "pending" {
            assert!(visible[0]["currentRevisionRef"].is_null());
            assert_eq!(
                visible[0]["review"]["commandRef"],
                json!(opinion.command_ref)
            );
            assert_eq!(visible[0]["review"]["reason"], opinion.reason);
        }
    }
    let mut adopt = command(&candidate, "adopt", None);
    adopt.verified = true;
    let receipt = review_reading(&db, unit, &adopt).await.unwrap();
    assert_eq!(
        receipt["currentRevisionRef"],
        candidate["candidateRevisionRef"]
    );
    assert_eq!(review_reading(&db, unit, &adopt).await.unwrap(), receipt);
    let mut changed = adopt.clone();
    changed.reason = "different reason on an already committed command".into();
    assert!(matches!(
        review_reading(&db, unit, &changed).await,
        Err(ReadingError::Conflict)
    ));
    assert!(matches!(
        review_reading(&db, Uuid::new_v4(), &adopt).await,
        Err(ReadingError::Conflict)
    ));
    let mut stale = adopt.clone();
    stale.command_ref = Uuid::new_v4();
    assert!(matches!(
        review_reading(&db, unit, &stale).await,
        Err(ReadingError::Conflict)
    ));
    let selected = research_dossier::read(&db, work).await.unwrap().unwrap();
    let current = selected
        .materials
        .iter()
        .find(|m| m["ref"] == json!(unit))
        .unwrap();
    assert_eq!(current["reading"]["review"]["actor"], adopt.actor);
    assert_eq!(current["reading"]["review"]["reason"], adopt.reason);
    assert_eq!(current["reading"]["verification"], "verified");
    let mut edit = command(&candidate, "edit", Some(candidate_ref));
    let manual = format!("{}结尾🙂：并不是 30 mg。", "完整人工转写。".repeat(3000));
    edit.edited_blocks = Some(vec![
        json!({"kind":"paragraph","speaker":"unconfirmed","text":manual}),
    ]);
    edit.verified = true;
    let edited = review_reading(&db, unit, &edit).await.unwrap();
    assert_ne!(
        edited["currentRevisionRef"],
        candidate["candidateRevisionRef"]
    );
    let result = research_dossier::read(&db, work).await.unwrap().unwrap();
    let current = result
        .materials
        .iter()
        .find(|m| m["ref"] == json!(unit))
        .unwrap();
    assert_eq!(
        current["blocks"][0]["text"], manual,
        "manual content has no old 16,000-character cap"
    );
    assert!(current["blocks"][0].get("bbox").is_none());
    assert_eq!(
        current["reading"]["review"]["commandRef"],
        json!(edit.command_ref)
    );
    let original = research_dossier::read_version(&db, work, candidate_ref)
        .await
        .unwrap()
        .unwrap()
        .material;
    assert_eq!(original["blocks"], candidate["blocks"]);
    assert_eq!(
        original["reading"]["review"]["commandRef"],
        json!(adopt.command_ref)
    );
    let visible = read_run(&db, work).await.unwrap();
    assert_eq!(
        visible["images"][0]["rawJson"],
        saved["images"][0]["rawJson"]
    );
    assert_eq!(visible["images"][0]["blocks"], saved["images"][0]["blocks"]);
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM linggan_ocr_audit_sample),(SELECT count(*) FROM linggan_ocr_audit_issue)")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(counts, (0, 0));
    assert_eq!(
        wire.requests.lock().unwrap().len(),
        2,
        "only the explicit probe and one image were sent"
    );
    wire.finish().await;
}
