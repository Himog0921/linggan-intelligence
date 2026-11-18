use crate::ai_foundation::content_understanding::Packet;
use linggan_evidence::{
    material_reading::{NewReading, Publication, publish_in},
    research_dossier::content::{blocks_from_text, has_source_text, validate_blocks},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Readable,
    NoText,
    Unreadable,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftReading {
    input_citation_id: String,
    state: State,
    text: String,
}

/// Connect model output only to image hashes and slots from the frozen L1 request.
pub(super) fn prepare(draft: &Value, packet: &Packet) -> Result<Vec<NewReading>, &'static str> {
    let rows: Vec<DraftReading> = serde_json::from_value(draft["imageReadings"].clone())
        .map_err(|_| "l1_image_readings_invalid")?;
    let mut images: BTreeMap<_, _> = packet
        .snapshot
        .images
        .iter()
        .map(|image| (image.citation_id.as_str(), image.blob_sha256.as_str()))
        .collect();
    let inventory = packet
        .inventory
        .as_array()
        .ok_or("l1_image_inventory_invalid")?;
    let mut readings = Vec::new();
    for row in rows {
        let sha = images
            .remove(row.input_citation_id.as_str())
            .ok_or("l1_image_reading_id_invalid")?;
        let text = row.text.trim();
        if row.text.chars().count() > 30_000 {
            return Err("l1_image_reading_too_long");
        }
        let (source_state, blocks) = match row.state {
            State::Readable => {
                let blocks = blocks_from_text(text, "unconfirmed");
                validate_blocks(&blocks).map_err(|_| "l1_image_reading_blocks_invalid")?;
                if !blocks.iter().any(has_source_text) {
                    return Err("l1_image_reading_text_empty");
                }
                ("present", blocks)
            }
            State::NoText if text.is_empty() => ("known_empty", Vec::new()),
            State::Unreadable if text.is_empty() => continue,
            _ => return Err("l1_image_reading_state_invalid"),
        };
        for slot in inventory.iter().filter(|slot| {
            slot["component"] == "image" && slot["available"] == true && slot["blobSha256"] == sha
        }) {
            let slot_key = slot["slotKey"].as_str().ok_or("l1_image_slot_invalid")?;
            let version = slot["sourceVersion"]
                .as_str()
                .ok_or("l1_image_version_invalid")?;
            let page = slot["page"].as_i64().and_then(|p| i32::try_from(p).ok());
            readings.push(NewReading {
                content_public_ref: packet.snapshot.work_ref,
                unit_key: format!("image:{slot_key}"),
                kind: "image".into(),
                slot_key: Some(slot_key.into()),
                page,
                source_version: version.into(),
                source_state: source_state.into(),
                derivative_ref: None,
                producer_ref: None,
                generated_by: "cloud_vision".into(),
                rule_version: "content-understanding.image-reading.v1".into(),
                blocks: blocks.clone(),
                issues: Vec::new(),
                source_location: json!({"blobSha256":sha,"materializationRef":version,
                    "inputCitationId":row.input_citation_id,"formattedReading":true}),
                publication: Publication::Automatic {
                    expected_current: None,
                },
            });
        }
    }
    if !images.is_empty() {
        return Err("l1_image_readings_incomplete");
    }
    Ok(readings)
}

pub(super) async fn publish(
    conn: &mut PgConnection,
    request: Uuid,
    readings: Vec<NewReading>,
) -> Result<(), sqlx::Error> {
    for mut reading in readings {
        reading.producer_ref = Some(request);
        reading.source_location["analysisRequestRef"] = json!(request);
        publish_in(conn, reading).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_understanding::{ImageSource, Snapshot};

    #[test]
    fn formatted_reading_uses_only_frozen_available_image_slots() {
        let sha = "abc";
        let version = Uuid::new_v4();
        let packet = Packet {
            snapshot: Snapshot {
                work_ref: Uuid::new_v4(),
                material_snapshot_identity: String::new(),
                material_as_of: String::new(),
                included_kinds: vec!["image".into()],
                coverage_mode: "complete".into(),
                texts: vec![],
                images: vec![ImageSource {
                    citation_id: "image_1".into(),
                    source_ref: version,
                    source_version: version.to_string(),
                    blob_sha256: sha.into(),
                }],
            },
            inventory: json!([
                {"component":"image","available":true,"blobSha256":sha,
                 "slotKey":"page-1","sourceVersion":version,"page":1},
                {"component":"image","available":false,"blobSha256":sha,
                 "slotKey":"restricted","sourceVersion":version,"page":2}
            ]),
            coverage: Value::Null,
        };
        let page = json!({"inputCitationId":"image_1","state":"readable",
                         "text":"## 小标题\n\n1. 第一项\n2. 第二项"});
        let parsed = prepare(&json!({"imageReadings":[page.clone()]}), &packet).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].slot_key.as_deref(), Some("page-1"));
        assert_eq!(parsed[0].blocks[0]["kind"], "heading");
        assert_eq!(parsed[0].blocks[1]["kind"], "list");
        assert!(prepare(&json!({"imageReadings":[page.clone(), page]}), &packet).is_err());
        assert!(prepare(&json!({"imageReadings":[]}), &packet).is_err());
    }
}
