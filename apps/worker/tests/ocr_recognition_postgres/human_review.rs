//! Current cloud output is readable immediately; a later run cannot replace a human correction.
use super::*;
use linggan_evidence::{material_reading::*, research_dossier, *};
use serde_json::{Value, json};

async fn current_image(db: &linggan_storage_postgres::Database, work: Uuid) -> Value {
    research_dossier::read(db, work)
        .await
        .unwrap()
        .unwrap()
        .materials
        .into_iter()
        .find(|unit| unit["kind"] == "image")
        .unwrap()
}
async fn recognize(
    db: &linggan_storage_postgres::Database,
    work: Uuid,
    owner: &ExecutorSession,
    adapter: &PiAdapter,
) {
    let ready = read_run(db, work).await.unwrap();
    start_run(
        db,
        work,
        &StartRun {
            command_ref: Uuid::new_v4(),
            expected_scope_sha256: ready["scope"]["sha256"].as_str().unwrap().into(),
            slot_keys: selected_slots(&ready),
        },
    )
    .await
    .unwrap();
    let item = queue::claim_next(db, owner.fence()).await.unwrap().unwrap();
    ocr_recognition::execute(db, &item, &SyntheticModelSecrets, adapter)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "owned disposable PostgreSQL, synthetic image and loopback Pi only"]
async fn cloud_current_human_edit_replay_and_later_machine_run_preserve_human_priority() {
    let db = fixture::db("ocr_manual_current_edit").await;
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
    let owner = ExecutorSession::acquire(&db, "synthetic-canonical-edit", 4)
        .await
        .unwrap();
    configure_recognition(&db, &wire.url, &owner, &adapter).await;
    recognize(&db, work, &owner, &adapter).await;
    let run = read_run(&db, work).await.unwrap();
    assert_eq!(run["state"], "succeeded");
    assert_eq!(run["images"][0]["isCurrent"], true);
    let current = current_image(&db, work).await;
    assert_eq!(
        current["currentRevisionRef"],
        run["images"][0]["readingRef"]
    );
    assert!(!current["blocks"].as_array().unwrap().is_empty());
    let unit: Uuid = serde_json::from_value(current["ref"].clone()).unwrap();
    let base: Uuid = serde_json::from_value(current["currentRevisionRef"].clone()).unwrap();
    let mut blocks = current["blocks"].as_array().unwrap().clone();
    let manual = format!("{}结尾🙂：并不是 30 mg。", "完整人工转写。".repeat(3000));
    let paragraph = blocks
        .iter_mut()
        .find(|block| block["kind"] == "paragraph")
        .unwrap();
    paragraph["text"] = json!(manual);
    let request = ReviewReadingRequest {
        command_ref: Uuid::new_v4(),
        unit_ref: unit.to_string(),
        source_version: current["sourceVersion"].as_str().unwrap().into(),
        expected_current_revision_ref: Some(base),
        action: "edit".into(),
        actor: "synthetic-reviewer".into(),
        reason: "核对完整合成原图后修正文字".into(),
        edited_blocks: Some(blocks),
        verified: true,
        resolved_issue_keys: vec![],
    };
    let saved = review_reading(&db, unit, &request).await.unwrap();
    assert_ne!(saved["currentRevisionRef"], json!(base));
    assert_eq!(saved["adoption"], "human_edited");
    assert_eq!(
        review_reading(&db, unit, &request).await.unwrap(),
        saved,
        "lost receipt reuses one command"
    );
    let mut altered = request.clone();
    altered.reason = "same UUID with different payload".into();
    assert!(matches!(
        review_reading(&db, unit, &altered).await,
        Err(ReadingError::Conflict)
    ));
    let mut stale = request.clone();
    stale.command_ref = Uuid::new_v4();
    assert!(matches!(
        review_reading(&db, unit, &stale).await,
        Err(ReadingError::Conflict)
    ));
    let human = current_image(&db, work).await;
    assert!(
        human["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|block| block["text"] == manual)
    );
    recognize(&db, work, &owner, &adapter).await;
    let later = read_run(&db, work).await.unwrap();
    assert_eq!(later["state"], "succeeded");
    assert_eq!(later["images"][0]["isCurrent"], false);
    let kept = current_image(&db, work).await;
    assert_eq!(kept["currentRevisionRef"], saved["currentRevisionRef"]);
    assert_eq!(kept["blocks"], human["blocks"]);
    let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading_command")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(commands, 1);
    let calls: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE task_key='ocr_recognition'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(calls, 2);
    let source: Uuid = request.source_version.parse().unwrap();
    record_materialization_disposition(
        &db,
        source,
        MaterialMediaDisposition::BytesCleaned,
        "synthetic",
        "synthetic byte cleanup",
    )
    .await
    .unwrap();
    let mut blocked = request.clone();
    blocked.command_ref = Uuid::new_v4();
    blocked.expected_current_revision_ref =
        serde_json::from_value(saved["currentRevisionRef"].clone()).unwrap();
    assert!(matches!(
        review_reading(&db, unit, &blocked).await,
        Err(ReadingError::OriginalUnavailable)
    ));
    assert_eq!(review_reading(&db, unit, &request).await.unwrap(), saved);
    record_materialization_disposition(
        &db,
        source,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic",
        "synthetic source restriction",
    )
    .await
    .unwrap();
    assert!(matches!(
        review_reading(&db, unit, &request).await,
        Err(ReadingError::SourceChanged)
    ));
    assert_eq!(current_image(&db, work).await["blocks"], json!([]));
}
