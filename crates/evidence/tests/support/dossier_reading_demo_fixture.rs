//! Approved reading demo replay, only in isolated tests. No recognizer is invoked.
use linggan_evidence::{material_reading as reading, research_dossier as dossier};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

pub async fn seed(db: &Database) -> Uuid {
    let data: Value = serde_json::from_str(include_str!("dossier_reading_demo.json")).unwrap();
    assert_eq!(data["synthetic"], true);
    let name = "approved-reading-demo";
    let body = data["body"].as_str().unwrap();
    let work = super::scenario(db, name, Some(body), 2, true, false, false).await;
    super::fixture::submit_package(db, "content_detail", json!({"contentExternalId":name}), json!({
        "kind":"content_detail", "sourceObject":{"platform":"xhs","type":"content","externalId":name},
        "payload":{"title":data["title"],"bodyText":body,"hashtags":data["tags"],"authorId":"synthetic-author"}
    })).await;
    let current = dossier::read(db, work).await.unwrap().unwrap();
    for unit in &current.materials {
        let kind = unit["kind"].as_str().unwrap();
        let blocks = match kind {
            "image" => {
                let page = unit["page"].as_u64().unwrap();
                data["images"][usize::try_from(page - 1).unwrap()].clone()
            }
            "audio" => data["audio"].clone(),
            "video_frame" => data["frames"].clone(),
            _ => continue,
        };
        let slot = unit["slotKey"].as_str().unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        let receipt = reading::publish_in(
            &mut tx,
            reading::NewReading {
                content_public_ref: work,
                unit_key: format!("{kind}:{slot}"),
                kind: kind.into(),
                slot_key: Some(slot.into()),
                page: unit["page"].as_i64().map(|n| i32::try_from(n).unwrap()),
                source_version: unit["sourceVersion"].as_str().unwrap().into(),
                source_state: "present".into(),
                derivative_ref: None,
                producer_ref: Some(Uuid::new_v4()),
                generated_by: match kind {
                    "audio" => "asr",
                    "video_frame" => "frame_ocr",
                    _ => "cloud_vision",
                }
                .into(),
                rule_version: "approved-reading-demo.v1:synthetic-replay".into(),
                blocks: serde_json::from_value(blocks).unwrap(),
                issues: vec![],
                source_location: json!({"synthetic":true,"fixture":data["fixture"]}),
                publication: reading::Publication::Automatic {
                    expected_current: serde_json::from_value(unit["currentRevisionRef"].clone())
                        .unwrap(),
                },
            },
        )
        .await
        .unwrap();
        assert!(receipt.current);
        tx.commit().await.unwrap();
    }
    work
}
