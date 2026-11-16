//! Deterministic L2 input manifest and fingerprint; generation-time only, no I/O.
use super::source::Packet;
use crate::research_text::content_hash;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

/// Product cap confirmed 2026-11-16 (D7); exposed through /deep/modes.
pub(crate) const MAX_IMAGES: usize = 9;
pub(crate) const MODES: [&str; 3] = ["content_design", "content_value", "creation_method"];
pub(crate) const FINGERPRINT_CONTRACT: &str = "content-understanding.deep-input.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SubQuestion {
    pub id: String,
    pub text: String,
}

pub(crate) fn default_sub_questions(mode: &str) -> Option<Vec<SubQuestion>> {
    let questions: [&str; 3] = match mode {
        "content_design" => [
            "标题和开头提出了什么，正文怎样承接？",
            "正文按什么顺序展开信息？",
            "作者用了哪些具体的表达手法或图文配合？",
        ],
        "content_value" => [
            "标题、封面或开头承诺了什么？",
            "正文实际交付了哪些信息、解释、步骤或判断方法？",
            "承诺与交付对上了哪些、哪些没有交付？",
        ],
        "creation_method" => [
            "这篇作品里有哪些具体做法值得迁移？",
            "每个做法在原作品中怎样使用？",
            "换一个选题时需要怎样调整，什么情况下不适用？",
        ],
        _ => return None,
    };
    Some(
        questions
            .into_iter()
            .enumerate()
            .map(|(index, text)| SubQuestion {
                id: format!("Q{}", index + 1),
                text: text.into(),
            })
            .collect(),
    )
}

/// Mode/base-mode/sub-question contract and the frozen `question` column value.
pub(crate) fn validate_request(
    research_mode: &str,
    base_mode: Option<&str>,
    sub_questions: &[SubQuestion],
) -> Result<String, &'static str> {
    let standard = MODES.contains(&research_mode);
    if !standard && research_mode != "custom" {
        return Err("unknown_research_mode");
    }
    match (research_mode == "custom", base_mode) {
        (true, Some(base)) if MODES.contains(&base) => {}
        (true, _) => return Err("invalid_base_mode"),
        (false, None) => {}
        (false, Some(_)) => return Err("invalid_base_mode"),
    }
    if sub_questions.is_empty() || sub_questions.len() > 4 {
        return Err("invalid_sub_questions");
    }
    let mut ids = std::collections::BTreeSet::new();
    for question in sub_questions {
        if question.id.trim().is_empty()
            || !ids.insert(question.id.trim())
            || question.text.trim().is_empty()
            || question.text.chars().count() > 120
        {
            return Err("invalid_sub_questions");
        }
    }
    let question = sub_questions
        .iter()
        .map(|question| question.text.trim())
        .collect::<Vec<_>>()
        .join("；");
    if question.chars().count() > 300 {
        return Err("invalid_sub_questions");
    }
    Ok(question)
}

fn normalized_text(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\r', "\n").nfc().collect()
}

/// Build the frozen material manifest and its fingerprint.
///
/// Image order follows `snapshot.images`; the request sends exactly these images
/// in this order (adapter labels them image_1..image_n).
pub(crate) fn build(
    packet: &Packet,
    card: &Value,
    research_mode: &str,
    base_mode: Option<&str>,
    sub_questions: &[SubQuestion],
    method_version: &str,
    core_basis: &Value,
) -> Result<(Value, String), &'static str> {
    let mut materials: Vec<Value> = Vec::new();

    for kind in ["title", "body", "tag", "asr"] {
        let source_kind = if kind == "asr" { "transcript" } else if kind == "tag" { "tags" } else { kind };
        let blocks: Vec<&crate::content_understanding::TextSource> = packet
            .snapshot
            .texts
            .iter()
            .filter(|text| text.kind == source_kind)
            .collect();
        if blocks.is_empty() {
            continue;
        }
        let text = blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            continue;
        }
        let time_start = if kind == "asr" {
            blocks
                .iter()
                .find_map(|block| block.structure["time"]["startMs"].as_u64())
        } else {
            None
        };
        let time_end = if kind == "asr" {
            blocks
                .iter()
                .rev()
                .find_map(|block| block.structure["time"]["endMs"].as_u64())
        } else {
            None
        };
        let first = blocks[0];
        let id = format!("M{:02}", materials.len() + 1);
        materials.push(json!({
            "id":id,"materialKind":kind,
            "sourceRef":first.source_ref,"sourceVersion":first.source_version,
            "inputCitationId":first.citation_id,
            "text":text,"timeStartMs":time_start,"timeEndMs":time_end,"mediaSha256":null
        }));
    }

    let mut purposes: BTreeMap<&str, &str> = BTreeMap::new();
    for slot in packet.inventory.as_array().into_iter().flatten() {
        if let (Some(blob), Some(purpose)) = (slot["blobSha256"].as_str(), slot["purpose"].as_str()) {
            purposes.insert(blob, purpose);
        }
    }
    let referenced: std::collections::BTreeSet<&str> = card["evidence"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry.get("quote").is_none())
        .filter_map(|entry| entry["inputCitationId"].as_str())
        .collect();
    let mut images = 0usize;
    for image in &packet.snapshot.images {
        let cover = purposes
            .get(image.blob_sha256.as_str())
            .is_some_and(|purpose| *purpose == "cover");
        if !cover && !referenced.contains(image.citation_id.as_str()) {
            continue;
        }
        if images == MAX_IMAGES {
            break;
        }
        images += 1;
        // The adapter labels sent images image_1..image_n, so the manifest uses
        // the same identity instead of an unrelated sequence number.
        let id = format!("image_{images}");
        let material_kind = if cover { "cover" } else { "image" };
        materials.push(json!({
            "id":id,"materialKind":material_kind,
            "sourceRef":image.source_ref,"sourceVersion":image.source_version,
            "inputCitationId":image.citation_id,
            "text":null,"timeStartMs":null,"timeEndMs":null,"mediaSha256":image.blob_sha256
        }));
    }

    for unit in card["core"]["knowledgeUnits"].as_array().into_iter().flatten() {
        let Some(unit_id) = unit["id"].as_str() else {
            continue;
        };
        let id = format!("M{:02}", materials.len() + 1);
        materials.push(json!({
            "id":id,"materialKind":"core_unit","sourceRef":unit_id,"sourceVersion":null,
            "inputCitationId":unit_id,"text":unit["statement"],"timeStartMs":null,
            "timeEndMs":null,"mediaSha256":null
        }));
    }
    for evidence in card["evidence"].as_array().into_iter().flatten() {
        if evidence["id"].as_str().is_none() {
            continue;
        }
        let (text, evidence_kind) = if let Some(quote) = evidence["quote"].as_str() {
            (quote, "verbatim")
        } else if let Some(observation) = evidence["modelQuote"]
            .as_str()
            .or_else(|| evidence["observation"].as_str())
        {
            (observation, "model_read")
        } else {
            continue;
        };
        let id = format!("M{:02}", materials.len() + 1);
        materials.push(json!({
            "id":id,"materialKind":"core_evidence",
            "sourceRef":evidence["sourceRef"],"sourceVersion":evidence["sourceVersion"],
            "inputCitationId":evidence["inputCitationId"],"text":text,
            "timeStartMs":null,"timeEndMs":null,"mediaSha256":null,
            "evidenceKind":evidence_kind
        }));
    }

    let canonical_materials: Vec<Value> = materials
        .iter()
        .map(|material| {
            let text = material["text"].as_str().unwrap_or_default();
            let text_sha256 = if text.is_empty() {
                Value::Null
            } else {
                json!(content_hash(&normalized_text(text)))
            };
            json!({
                "id":material["id"],"materialKind":material["materialKind"],
                "sourceRef":material["sourceRef"],"sourceVersion":material["sourceVersion"],
                "inputCitationId":material["inputCitationId"],
                "timeStartMs":material["timeStartMs"],"timeEndMs":material["timeEndMs"],
                "mediaSha256":material["mediaSha256"],"textSha256":text_sha256
            })
        })
        .collect();
    let canonical = json!({
        "contract":FINGERPRINT_CONTRACT,"researchMode":research_mode,
        "baseMode":base_mode,"methodVersion":method_version,"coreBasis":core_basis,
        "subQuestions":sub_questions,"materials":canonical_materials
    });
    let fingerprint = content_hash(
        &serde_json::to_string(&canonical).map_err(|_| "invalid_manifest")?,
    );
    Ok((Value::Array(materials), fingerprint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_understanding::{Snapshot, TextSource};
    use uuid::Uuid;

    fn question(id: &str, text: &str) -> SubQuestion {
        SubQuestion {
            id: id.into(),
            text: text.into(),
        }
    }

    fn text(citation: &str, kind: &str, body: &str) -> TextSource {
        TextSource {
            citation_id: citation.into(),
            source_ref: Uuid::nil(),
            source_version: "v1".into(),
            start: 0,
            text: body.into(),
            kind: kind.into(),
            speaker: "unconfirmed".into(),
            structure: serde_json::json!({}),
            issues: vec![],
        }
    }

    fn packet() -> Packet {
        let mut asr = text("s3", "transcript", "口播");
        asr.structure = serde_json::json!({"time":{"startMs":100,"endMs":900}});
        Packet {
            snapshot: Snapshot {
                work_ref: Uuid::nil(),
                material_snapshot_identity: "identity".into(),
                material_as_of: "2026-11-16T00:00:00.000000Z".into(),
                included_kinds: vec!["body".into()],
                coverage_mode: "complete".into(),
                texts: vec![
                    text("s1", "title", "标题"),
                    text("s2", "body", "先摆材料。"),
                    asr,
                    text("s4", "frame_text", "已下线的画面文字"),
                ],
                images: vec![],
            },
            inventory: serde_json::json!([]),
            coverage: serde_json::json!({}),
        }
    }

    #[test]
    fn request_shape_is_validated_and_question_is_joined() {
        assert!(validate_request("content_design", Some("content_value"), &[question("Q1", "甲")]).is_err());
        assert!(validate_request("custom", None, &[question("Q1", "甲")]).is_err());
        assert!(validate_request("content_design", None, &[]).is_err());
        let five: Vec<_> = (1..=5)
            .map(|index| question(&format!("Q{index}"), "甲"))
            .collect();
        assert!(validate_request("content_design", None, &five).is_err());
        let long = question("Q1", &"字".repeat(121));
        assert!(validate_request("content_design", None, &[long]).is_err());
        let joined = validate_request(
            "content_design",
            None,
            &[question("Q1", "甲"), question("Q2", "乙")],
        )
        .unwrap();
        assert_eq!(joined, "甲；乙");
        assert!(validate_request("custom", Some("content_value"), &[question("Q1", "甲")]).is_ok());
    }

    #[test]
    fn manifest_retires_frame_text_and_fingerprint_tracks_text() {
        let card = serde_json::json!({"core":{"knowledgeUnits":[]},"evidence":[]});
        let basis = serde_json::json!({"kind":"base"});
        let questions = vec![question("Q1", "测什么？")];
        let (manifest, first) = build(
            &packet(),
            &card,
            "content_design",
            None,
            &questions,
            "content-understanding.l2.model.v1",
            &basis,
        )
        .unwrap();
        let kinds: Vec<&str> = manifest
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|material| material["materialKind"].as_str())
            .collect();
        assert_eq!(kinds, vec!["title", "body", "asr"]);
        assert_eq!(manifest[2]["timeStartMs"], serde_json::json!(100));
        assert_eq!(manifest[2]["timeEndMs"], serde_json::json!(900));

        let mut changed = packet();
        changed.snapshot.texts[1].text = "换了正文。".into();
        let (_, second) = build(
            &changed,
            &card,
            "content_design",
            None,
            &questions,
            "content-understanding.l2.model.v1",
            &basis,
        )
        .unwrap();
        assert_ne!(first, second);
    }
}
