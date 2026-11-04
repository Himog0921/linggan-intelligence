//! Canonical current reading, edit-only CAS, source guards and human priority.
#[path = "support/research_dossier_fixture.rs"]
mod support;
use linggan_evidence::{material_reading::*, *};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

async fn current(db: &Database, work: Uuid) -> Value {
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let mut c = db.pool().acquire().await.unwrap();
    read_current_in(&mut c, &[work], &now).await.unwrap()[&work]
        .iter()
        .find(|u| u["kind"] == "image")
        .unwrap()
        .clone()
}
fn id(v: &Value) -> Uuid {
    serde_json::from_value(v.clone()).unwrap()
}
fn paragraph(text: &str) -> Vec<Value> {
    vec![json!({"ref":"synthetic","kind":"paragraph","text":text,"speaker":"unconfirmed"})]
}
fn edit(unit: &Value, text: &str) -> ReviewReadingRequest {
    ReviewReadingRequest {
        command_ref: Uuid::new_v4(),
        unit_ref: unit["ref"].as_str().unwrap().into(),
        source_version: unit["sourceVersion"].as_str().unwrap().into(),
        expected_current_revision_ref: unit["currentRevisionRef"]
            .as_str()
            .map(|s| Uuid::parse_str(s).unwrap()),
        action: "edit".into(),
        actor: "synthetic-editor".into(),
        reason: "Compare with original source".into(),
        edited_blocks: Some(paragraph(text)),
        verified: true,
        resolved_issue_keys: vec![],
    }
}
async fn publish(
    db: &Database,
    work: Uuid,
    unit: &Value,
    expected: Option<Uuid>,
    text: &str,
) -> PublishedReading {
    let mut tx = db.pool().begin().await.unwrap();
    let p=publish_in(&mut tx,NewReading{content_public_ref:work,unit_key:format!("image:{}",unit["slotKey"].as_str().unwrap()),kind:"image".into(),slot_key:Some(unit["slotKey"].as_str().unwrap().into()),page:Some(1),source_version:unit["sourceVersion"].as_str().unwrap().into(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:"cloud_vision".into(),rule_version:"synthetic-current.v1".into(),blocks:paragraph(text),issues:vec![],source_location:json!({"slotKey":unit["slotKey"],"sourceVersion":unit["sourceVersion"]}),publication:Publication::Automatic{expected_current:expected}}).await.unwrap();
    tx.commit().await.unwrap();
    p
}
#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn current_cloud_edit_is_cas_idempotent_and_keeps_human_priority() {
    let db = support::fixture::proof_database("dossier_current_edit").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    let before = current(&db, work).await;
    assert_eq!(before["reading"]["generatedBy"], "cloud_vision");
    let next = publish(
        &db,
        work,
        &before,
        Some(id(&before["currentRevisionRef"])),
        "新的完整文字，不删除否定。",
    )
    .await;
    assert!(next.current);
    let unit = current(&db, work).await;
    assert_eq!(unit["currentRevisionRef"], json!(next.reading_ref));
    let request = edit(&unit, "人工逐字核对后的原文。");
    let receipt = review_reading(&db, Uuid::parse_str(&request.unit_ref).unwrap(), &request)
        .await
        .unwrap();
    assert_eq!(receipt["adoption"], "human_edited");
    assert_eq!(receipt["verification"], "verified");
    assert!(receipt.get("candidateRevisionRef").is_none());
    assert_eq!(
        review_reading(&db, Uuid::parse_str(&request.unit_ref).unwrap(), &request)
            .await
            .unwrap(),
        receipt
    );
    let mut conflicting = request.clone();
    conflicting.reason = "same UUID with changed body".into();
    assert!(matches!(
        review_reading(
            &db,
            Uuid::parse_str(&request.unit_ref).unwrap(),
            &conflicting
        )
        .await,
        Err(ReadingError::Conflict)
    ));
    let stale = edit(&unit, "过期编辑不可覆盖当前文字");
    assert!(matches!(
        review_reading(&db, Uuid::parse_str(&stale.unit_ref).unwrap(), &stale).await,
        Err(ReadingError::Conflict)
    ));
    let protected = publish(
        &db,
        work,
        &unit,
        Some(id(&receipt["currentRevisionRef"])),
        "机器不得覆盖人工",
    )
    .await;
    assert!(!protected.current);
    assert_eq!(
        current(&db, work).await["blocks"][0]["text"],
        "人工逐字核对后的原文。"
    );
}
#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn source_and_original_guard_apply_to_edits_and_lost_receipts() {
    let db = support::fixture::proof_database("dossier_edit_source").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    let unit = current(&db, work).await;
    let r = edit(&unit, "已核对原件");
    review_reading(&db, Uuid::parse_str(&r.unit_ref).unwrap(), &r)
        .await
        .unwrap();
    let mut source_changed = r.clone();
    source_changed.command_ref = Uuid::new_v4();
    source_changed.source_version = Uuid::new_v4().to_string();
    assert!(matches!(
        review_reading(&db, Uuid::parse_str(&r.unit_ref).unwrap(), &source_changed).await,
        Err(ReadingError::SourceChanged)
    ));
    let m = Uuid::parse_str(unit["sourceVersion"].as_str().unwrap()).unwrap();
    record_materialization_disposition(
        &db,
        m,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "synthetic withdrawal",
    )
    .await
    .unwrap();
    assert!(matches!(
        review_reading(&db, Uuid::parse_str(&r.unit_ref).unwrap(), &r).await,
        Err(ReadingError::SourceChanged)
    ));
}
#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn edits_keep_unresolved_issues_and_reject_old_actions() {
    let db = support::fixture::proof_database("dossier_edit_issues").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    let unit = current(&db, work).await;
    let mut r = edit(&unit, "人工修正并保留缺口");
    for action in ["adopt", "keep", "pending", "reject"] {
        r.action = action.into();
        assert!(matches!(
            review_reading(&db, Uuid::parse_str(&r.unit_ref).unwrap(), &r).await,
            Err(ReadingError::Invalid)
        ));
    }
    r.action = "edit".into();
    let receipt = review_reading(&db, Uuid::parse_str(&r.unit_ref).unwrap(), &r)
        .await
        .unwrap();
    let changed = current(&db, work).await;
    assert_eq!(changed["currentRevisionRef"], receipt["currentRevisionRef"]);
    assert!(
        changed["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["key"] == "background")
    );
    let mut resolved = edit(&changed, "已逐字核对背景文字");
    resolved.resolved_issue_keys = vec!["background".into()];
    review_reading(&db, Uuid::parse_str(&resolved.unit_ref).unwrap(), &resolved)
        .await
        .unwrap();
    assert!(
        current(&db, work).await["issues"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn first_human_transcription_and_same_bytes_reobservation_remain_editable_and_protected() {
    let db = support::fixture::proof_database("dossier_first_human").await;
    let work = support::work(&db).await;
    let (_, _, _) = support::acquired_image_source(&db).await;
    let first = research_dossier::read(&db, work).await.unwrap().unwrap();
    let image = first
        .materials
        .iter()
        .find(|u| u["kind"] == "image")
        .unwrap();
    assert!(image["currentRevisionRef"].is_null());
    assert!(Uuid::parse_str(image["ref"].as_str().unwrap()).is_err());
    let r = edit(image, "人工从原图逐字录入，无需先识别。");
    let receipt = review_work_reading(&db, work, &r.unit_ref, &r)
        .await
        .unwrap();
    assert_eq!(receipt["unitRef"], r.unit_ref);
    assert_eq!(receipt["adoption"], "human_edited");
    assert_eq!(
        review_work_reading(&db, work, &r.unit_ref, &r)
            .await
            .unwrap(),
        receipt
    );
    let (_, new_version, _) = support::acquired_image_source(&db).await;
    assert_ne!(
        new_version.to_string(),
        r.source_version,
        "same bytes must have a distinct observed materialization in this regression"
    );
    let observed = research_dossier::read(&db, work).await.unwrap().unwrap();
    let image = observed
        .materials
        .iter()
        .find(|u| u["kind"] == "image")
        .unwrap();
    assert_eq!(image["currentRevisionRef"], receipt["currentRevisionRef"]);
    let r2 = edit(image, "同图再次观察后，人工仍能修正。");
    let receipt2 = review_work_reading(&db, work, &r2.unit_ref, &r2)
        .await
        .unwrap();
    let mut fresh = image.clone();
    fresh["sourceVersion"] = json!(new_version);
    let machine = publish(
        &db,
        work,
        &fresh,
        Some(id(&receipt2["currentRevisionRef"])),
        "同图机器结果不能覆盖人工",
    )
    .await;
    assert!(!machine.current);
    assert_eq!(
        current(&db, work).await["currentRevisionRef"],
        receipt2["currentRevisionRef"]
    );
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn changed_bytes_can_be_transcribed_without_new_machine_reading_and_conflicts_roll_back_source_change()
 {
    let db = support::fixture::proof_database("dossier_new_bytes_human").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    let old = current(&db, work).await;
    let old_edit = edit(&old, "旧原件人工稿");
    let old_receipt = review_work_reading(&db, work, &old_edit.unit_ref, &old_edit)
        .await
        .unwrap();
    support::acquired_image_source_with_bytes(
        &db,
        Some(b"synthetic different original bytes".to_vec()),
    )
    .await;
    let dossier = research_dossier::read(&db, work).await.unwrap().unwrap();
    let empty = dossier
        .materials
        .iter()
        .find(|m| m["kind"] == "image")
        .unwrap();
    assert!(empty["currentRevisionRef"].is_null());
    assert_ne!(empty["sourceVersion"], old["sourceVersion"]);
    let mut request = edit(empty, "新原件直接人工录入");
    request.command_ref = old_edit.command_ref;
    assert!(matches!(
        review_work_reading(&db, work, &request.unit_ref, &request).await,
        Err(ReadingError::Conflict)
    ));
    let source: String =
        sqlx::query_scalar("SELECT source_version FROM linggan_material_unit WHERE unit_ref=$1")
            .bind(id(&old["ref"]))
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        source, old_edit.source_version,
        "rejected reused command must roll back source advance"
    );
    request.command_ref = Uuid::new_v4();
    let saved = review_work_reading(&db, work, &request.unit_ref, &request)
        .await
        .unwrap();
    assert_eq!(
        current(&db, work).await["currentRevisionRef"],
        saved["currentRevisionRef"]
    );
    let old_text: Value =
        sqlx::query_scalar("SELECT blocks FROM linggan_material_reading WHERE reading_ref=$1")
            .bind(id(&old_receipt["currentRevisionRef"]))
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(old_text[0]["text"], "旧原件人工稿");
    assert_eq!(
        review_work_reading(&db, work, &request.unit_ref, &request)
            .await
            .unwrap(),
        saved
    );
}

#[path = "support/standard_dossier_fixture.rs"]
mod online_note_fixture;

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn online_note_keeps_legacy_history_without_mixing_automatic_text() {
    let db = support::fixture::proof_database("online_note_legacy_history").await;
    let work = online_note_fixture::scenario(
        &db,
        "online-note-history",
        Some("发布正文"),
        2,
        false,
        false,
        true,
    )
    .await;
    let sources: Vec<(String,Uuid)> = sqlx::query_as("SELECT o.slot_key,m.materialization_ref FROM linggan_material_media_origin o JOIN linggan_media_download_attempt a ON a.media_observation_ref=o.observation_ref JOIN linggan_media_materialization m USING(download_attempt_ref) WHERE o.content_public_ref=$1 ORDER BY o.producer_ordinal")
        .bind(work).fetch_all(db.pool()).await.unwrap();
    let mut old = vec![];
    for (index, (slot, version)) in sources.iter().enumerate() {
        let mut tx = db.pool().begin().await.unwrap();
        let result = publish_in(
            &mut tx,
            NewReading {
                content_public_ref: work,
                unit_key: format!("image:{slot}"),
                kind: "image".into(),
                slot_key: Some(slot.clone()),
                page: Some(index as i32 + 1),
                source_version: version.to_string(),
                source_state: "present".into(),
                derivative_ref: None,
                producer_ref: Some(Uuid::new_v4()),
                generated_by: "ordinary_ocr".into(),
                rule_version: "synthetic-legacy.v1".into(),
                blocks: paragraph(&format!("历史机器页{index}")),
                issues: vec![],
                source_location: json!({"slotKey":slot,"materializationRef":version}),
                publication: Publication::Automatic {
                    expected_current: None,
                },
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        old.push(result.reading_ref);
    }
    let before = research_dossier::read(&db, work).await.unwrap().unwrap();
    assert!(research_dossier::markdown(&before).contains("历史机器页1"));
    let first = before
        .materials
        .iter()
        .find(|m| m["slotKey"] == sources[0].0)
        .unwrap();
    publish(&db, work, first, Some(old[0]), "在线第一页完整内容").await;
    let after = research_dossier::read(&db, work).await.unwrap().unwrap();
    let text = research_dossier::markdown(&after);
    assert!(text.contains("在线第一页完整内容"));
    assert!(!text.contains("历史机器页"));
    assert_eq!(after.coverage["sources"]["images"]["state"], "partial");
    assert_eq!(after.coverage["imageInventory"]["processedPages"], 1);
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let mut conn = db.pool().acquire().await.unwrap();
    let historical = read_versions_in(&mut conn, &[work], &now, &old)
        .await
        .unwrap();
    assert_eq!(historical[&work].len(), 2);
    assert!(
        historical[&work]
            .iter()
            .any(|m| m["blocks"].to_string().contains("历史机器页1"))
    );
}

#[tokio::test]
#[ignore = "owned isolated PostgreSQL only"]
async fn requested_online_reading_replaces_frozen_human_reading_and_preserves_history() {
    let db = support::fixture::proof_database("dossier_requested_replace").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    let original = current(&db, work).await;
    let request = edit(&original, "人工旧读本");
    review_reading(&db, id(&original["ref"]), &request).await.unwrap();
    let human = current(&db, work).await;
    let human_ref = id(&human["currentRevisionRef"]);
    let make = |expected| NewReading {
        content_public_ref:work, unit_key:format!("image:{}", human["slotKey"].as_str().unwrap()),
        kind:"image".into(), slot_key:Some(human["slotKey"].as_str().unwrap().into()), page:Some(1),
        source_version:human["sourceVersion"].as_str().unwrap().into(), source_state:"present".into(),
        derivative_ref:None, producer_ref:Some(Uuid::new_v4()), generated_by:"cloud_vision".into(),
        rule_version:"synthetic-requested.v1".into(), blocks:paragraph("在线新读本"), issues:vec![],
        source_location:json!({"slotKey":human["slotKey"]}), publication:Publication::Requested{expected_current:Some(expected)}
    };
    let mut tx = db.pool().begin().await.unwrap();
    let published = publish_in(&mut tx, make(human_ref)).await.unwrap();
    tx.commit().await.unwrap();
    assert!(published.current);
    let replaced = current(&db, work).await;
    assert_eq!(replaced["currentRevisionRef"], json!(published.reading_ref));
    assert_eq!(replaced["reading"]["adoption"], "automatic");
    let historical: Value = sqlx::query_scalar("SELECT blocks FROM linggan_material_reading WHERE reading_ref=$1").bind(human_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(historical[0]["text"], "人工旧读本");
    let newer = edit(&replaced, "识别期间的人工修改");
    review_reading(&db, id(&replaced["ref"]), &newer).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(!publish_in(&mut tx, make(published.reading_ref)).await.unwrap().current);
    tx.commit().await.unwrap();
    assert_eq!(current(&db, work).await["blocks"][0]["text"], "识别期间的人工修改");
}
