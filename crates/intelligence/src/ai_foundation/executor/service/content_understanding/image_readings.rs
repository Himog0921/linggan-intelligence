use crate::ai_foundation::content_understanding::Packet;
use linggan_evidence::{
    material_reading::{NewReading, Publication, publish_in},
    research_dossier::content::{blocks_from_text, has_source_text, validate_blocks},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Readable,
    NoText,
    Unreadable,
    Deferred,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftReading {
    input_citation_id: String,
    state: State,
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReadingIssue {
    input_citation_id: Option<String>,
    code: &'static str,
}

pub(super) struct PreparedReadings {
    pub readings: Vec<NewReading>,
    pub unreadable: Vec<String>,
    pub no_text: Vec<String>,
    pub deferred: Vec<String>,
    pub missing: Vec<String>,
    pub invalid: Vec<ReadingIssue>,
}

/// Validate each image independently against the frozen input. Bad or missing
/// rows are reported without discarding other images or the L1 result.
pub(super) fn prepare(draft: &Value, packet: &Packet) -> Result<PreparedReadings, &'static str> {
    let rows = draft["imageReadings"]
        .as_array()
        .ok_or("l1_image_readings_invalid")?;
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
    let mut counts = BTreeMap::new();
    for id in rows
        .iter()
        .filter_map(|row| row["inputCitationId"].as_str())
    {
        *counts.entry(id).or_insert(0usize) += 1;
    }
    let mut handled = BTreeSet::new();
    let mut prepared = PreparedReadings {
        readings: Vec::new(),
        unreadable: Vec::new(),
        no_text: Vec::new(),
        deferred: Vec::new(),
        missing: Vec::new(),
        invalid: Vec::new(),
    };
    for raw in rows {
        let Some(id) = raw["inputCitationId"].as_str() else {
            prepared.invalid.push(ReadingIssue {
                input_citation_id: None,
                code: "l1_image_reading_id_invalid",
            });
            continue;
        };
        if !handled.insert(id) {
            continue;
        }
        let Some(sha) = images.remove(id) else {
            prepared.invalid.push(ReadingIssue {
                input_citation_id: None,
                code: "l1_image_reading_id_invalid",
            });
            continue;
        };
        let make_issue = |code| ReadingIssue {
            input_citation_id: Some(id.to_owned()),
            code,
        };
        if counts[id] != 1 {
            prepared
                .invalid
                .push(make_issue("l1_image_reading_duplicate"));
            continue;
        }
        let row: DraftReading = match serde_json::from_value(raw.clone()) {
            Ok(row) => row,
            Err(_) => {
                prepared
                    .invalid
                    .push(make_issue("l1_image_reading_invalid"));
                continue;
            }
        };
        let text = row.text.trim();
        if row.text.chars().count() > 30_000 {
            prepared
                .invalid
                .push(make_issue("l1_image_reading_too_long"));
            continue;
        }
        let (source_state, blocks) = match row.state {
            State::Readable => {
                let blocks = blocks_from_text(text, "unconfirmed");
                if validate_blocks(&blocks).is_err() || !blocks.iter().any(has_source_text) {
                    prepared
                        .invalid
                        .push(make_issue("l1_image_reading_text_invalid"));
                    continue;
                }
                ("present", blocks)
            }
            // The model's observation is not proof that the source is empty.
            State::NoText if text.is_empty() => {
                prepared.no_text.push(id.to_owned());
                continue;
            }
            State::Unreadable if text.is_empty() => {
                prepared.unreadable.push(id.to_owned());
                continue;
            }
            State::Deferred if text.is_empty() => {
                prepared.deferred.push(id.to_owned());
                continue;
            }
            _ => {
                prepared
                    .invalid
                    .push(make_issue("l1_image_reading_state_invalid"));
                continue;
            }
        };
        let matching: Vec<_> = inventory
            .iter()
            .filter(|slot| {
                slot["component"] == "image"
                    && slot["available"] == true
                    && slot["blobSha256"] == sha
            })
            .collect();
        if matching.is_empty()
            || matching.iter().any(|slot| {
                slot["slotKey"].as_str().is_none() || slot["sourceVersion"].as_str().is_none()
            })
        {
            prepared.invalid.push(make_issue("l1_image_slot_invalid"));
            continue;
        }
        for slot in matching {
            let slot_key = slot["slotKey"].as_str().expect("checked image slot");
            let version = slot["sourceVersion"]
                .as_str()
                .expect("checked source version");
            let page = slot["page"].as_i64().and_then(|p| i32::try_from(p).ok());
            prepared.readings.push(NewReading {
                content_public_ref: packet.snapshot.work_ref, unit_key: format!("image:{slot_key}"), kind: "image".into(),
                slot_key: Some(slot_key.into()), page, source_version: version.into(), source_state: source_state.into(),
                derivative_ref: None, producer_ref: None, generated_by: "cloud_vision".into(),
                rule_version: "content-understanding.image-reading.v1".into(), blocks: blocks.clone(),
                issues: vec![json!({"key":"model-formatted-image-reading","code":"model_formatted_reading","message":"图片文字由模型识读并整理，引用前请对照原图。","resolution":"reading_edit"})],
                source_location: json!({"blobSha256":sha,"materializationRef":version,"inputCitationId":row.input_citation_id,"formattedReading":true}),
                publication: Publication::Automatic { expected_current: None },
            });
        }
    }
    prepared.missing = images.keys().map(|id| (*id).to_owned()).collect();
    Ok(prepared)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReadingReceipt {
    input_citation_id: String,
    slot_key: Option<String>,
    source_state: String,
    #[serde(flatten)]
    publication: linggan_evidence::material_reading::PublishedReading,
}

pub(super) async fn publish(
    conn: &mut PgConnection,
    request: Uuid,
    readings: Vec<NewReading>,
) -> Result<Vec<ReadingReceipt>, sqlx::Error> {
    let mut receipts = Vec::with_capacity(readings.len());
    for mut reading in readings {
        reading.producer_ref = Some(request);
        reading.source_location["analysisRequestRef"] = json!(request);
        let input_citation_id = reading.source_location["inputCitationId"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let slot_key = reading.slot_key.clone();
        let source_state = reading.source_state.clone();
        let publication = publish_in(conn, reading).await?;
        receipts.push(ReadingReceipt {
            input_citation_id,
            slot_key,
            source_state,
            publication,
        });
    }
    Ok(receipts)
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
        assert_eq!(parsed.readings.len(), 1);
        assert_eq!(parsed.readings[0].slot_key.as_deref(), Some("page-1"));
        assert_eq!(parsed.readings[0].blocks[0]["kind"], "heading");
        assert_eq!(parsed.readings[0].blocks[1]["kind"], "list");
        let unreadable = prepare(&json!({"imageReadings":[{"inputCitationId":"image_1","state":"unreadable","text":""}]}), &packet).unwrap();
        assert!(unreadable.readings.is_empty());
        assert_eq!(unreadable.unreadable, vec!["image_1"]);
        let no_text = prepare(
            &json!({"imageReadings":[{"inputCitationId":"image_1","state":"no_text","text":""}]}),
            &packet,
        )
        .unwrap();
        assert!(no_text.readings.is_empty());
        assert_eq!(no_text.no_text, vec!["image_1"]);
        let duplicate = prepare(&json!({"imageReadings":[page.clone(), page]}), &packet).unwrap();
        assert!(duplicate.readings.is_empty());
        assert_eq!(duplicate.invalid[0].code, "l1_image_reading_duplicate");
        let missing = prepare(&json!({"imageReadings":[]}), &packet).unwrap();
        assert_eq!(missing.missing, vec!["image_1"]);
        let mut multi = packet.clone();
        multi.snapshot.images.push(ImageSource {
            citation_id: "image_2".into(),
            source_ref: version,
            source_version: version.to_string(),
            blob_sha256: "def".into(),
        });
        multi.inventory.as_array_mut().unwrap().push(json!({"component":"image","available":true,"blobSha256":"def","slotKey":"page-2","sourceVersion":version,"page":2}));
        let good = json!({"inputCitationId":"image_1","state":"readable","text":"第一页正文"});
        let partial = prepare(&json!({"imageReadings":[good.clone()]}), &multi).unwrap();
        assert_eq!(partial.readings.len(), 1);
        assert_eq!(partial.missing, vec!["image_2"]);
        let partial = prepare(&json!({"imageReadings":[good.clone(),{"inputCitationId":"image_2","state":"unreadable","text":""}]}), &multi).unwrap();
        assert_eq!(partial.readings.len(), 1);
        assert_eq!(partial.unreadable, vec!["image_2"]);
        let deferred = prepare(&json!({"imageReadings":[{"inputCitationId":"image_1","state":"readable","text":"第一页正文"},{"inputCitationId":"image_2","state":"deferred","text":""}]}), &multi).unwrap();
        assert_eq!(deferred.readings.len(), 1);
        assert_eq!(deferred.deferred, vec!["image_2"]);
        let partial = prepare(&json!({"imageReadings":[good,{"inputCitationId":"image_2","state":"invalid","text":""}]}), &multi).unwrap();
        assert_eq!(partial.readings.len(), 1);
        assert_eq!(
            partial.invalid[0].input_citation_id.as_deref(),
            Some("image_2")
        );
    }
}
