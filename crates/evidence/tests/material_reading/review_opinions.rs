//! Human opinions are command history, never implicit changes to the adopted reading.
use super::*;

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn review_opinions_preserve_selected_history_and_media_cleanup_blocks_new_commands() {
    let db = support::fixture::proof_database("dossier_review_opinions").await;
    let work = support::work(&db).await;
    support::ordinary_ocr(&db).await;
    let before = current(&db, work).await;
    let original = reference(&before["currentRevisionRef"]);
    let first = publish(
        &db,
        work,
        Uuid::new_v4(),
        paragraph("第一份完整云端候选"),
        vec![],
        Publication::Candidate,
    )
    .await;
    let mut adopt = request(first, Some(original), "adopt");
    adopt.verified = true;
    adopt.actor = "合成审核人甲".into();
    adopt.reason = "逐字核对数字、单位和否定".into();
    review_reading(&db, first.unit_ref, &adopt).await.unwrap();
    let selected = current(&db, work).await;
    assert_eq!(selected["reading"]["review"]["actor"], adopt.actor);
    assert_eq!(selected["reading"]["review"]["reason"], adopt.reason);
    let next = publish(
        &db,
        work,
        Uuid::new_v4(),
        paragraph("待核对的另一候选"),
        vec![],
        Publication::Candidate,
    )
    .await;
    let as_of = now(&db).await;
    let frozen = research_dossier::read_at(&db, work, Some(&as_of))
        .await
        .unwrap()
        .unwrap();
    let mut last = request(next, Some(first.reading_ref), "pending");
    for action in ["pending", "reject", "pending"] {
        last = request(next, Some(first.reading_ref), action);
        last.actor = "合成审核人乙".into();
        let receipt = review_reading(&db, next.unit_ref, &last).await.unwrap();
        assert_eq!(receipt["currentRevisionRef"], json!(first.reading_ref));
        assert_eq!(receipt["verification"], "verified");
        assert_eq!(receipt["adoption"], "human_adopted");
        assert_eq!(
            current(&db, work).await,
            selected,
            "opinion must not alter selection or its reviewer"
        );
        let candidates = read_candidates(&db, work).await.unwrap();
        assert_eq!(candidates.len(), usize::from(action == "pending"));
        let latest = research_dossier::read(&db, work).await.unwrap().unwrap();
        let plan = research_dossier::relayer_plan(&latest);
        assert_eq!(plan["pendingReviewCount"], usize::from(action == "pending"));
        assert_eq!(
            plan["layoutCount"], 1,
            "a cloud candidate must not hide the local target"
        );
        let historical = research_dossier::read_at(&db, work, Some(&as_of))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            historical.revision, frozen.revision,
            "later opinions do not change a same-cutoff plan"
        );
    }
    let plan = research_dossier::plan(&db, work).await.unwrap().unwrap();
    let rebuild =
        research_dossier::relayer_work(&db, work, plan["expectedRevision"].as_str().unwrap())
            .await
            .unwrap();
    assert_eq!(rebuild["scopeCount"], 1);
    assert_eq!(rebuild["updated"], 0);
    assert_eq!(rebuild["skipped"], 1);
    assert_eq!(current(&db, work).await, selected);
    let mut invalid = request(next, Some(first.reading_ref), "pending");
    invalid.verified = true;
    assert!(matches!(
        review_reading(&db, next.unit_ref, &invalid).await,
        Err(ReadingError::Invalid)
    ));
    invalid.verified = false;
    invalid.actor = " ".into();
    assert!(matches!(
        review_reading(&db, next.unit_ref, &invalid).await,
        Err(ReadingError::Invalid)
    ));
    invalid.actor = "合成审核人".into();
    invalid.reason = "因".repeat(301);
    assert!(matches!(
        review_reading(&db, next.unit_ref, &invalid).await,
        Err(ReadingError::Invalid)
    ));
    let source = reference(&before["sourceVersion"]);
    record_materialization_disposition(
        &db,
        source,
        MaterialMediaDisposition::BytesCleaned,
        "synthetic",
        "synthetic byte cleanup",
    )
    .await
    .unwrap();
    let readable = current(&db, work).await;
    assert_eq!(readable["blocks"], selected["blocks"]);
    assert!(readable.get("originalUrl").is_none());
    assert!(matches!(
        review_reading(
            &db,
            next.unit_ref,
            &request(next, Some(first.reading_ref), "adopt")
        )
        .await,
        Err(ReadingError::OriginalUnavailable)
    ));
    assert_eq!(
        review_reading(&db, next.unit_ref, &last).await.unwrap()["commandRef"],
        json!(last.command_ref),
        "an existing receipt is not a new human decision"
    );
    record_materialization_disposition(
        &db,
        source,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic",
        "synthetic current restriction",
    )
    .await
    .unwrap();
    assert!(
        matches!(
            review_reading(&db, next.unit_ref, &last).await,
            Err(ReadingError::SourceChanged)
        ),
        "receipt replay cannot bypass withdrawal"
    );
    assert!(read_candidates(&db, work).await.unwrap().is_empty());
    assert_eq!(current(&db, work).await["blocks"], json!([]));
}
