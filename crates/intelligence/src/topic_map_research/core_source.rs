//! Source-bound L1 consumption. Image observations remain derived text, never OCR.
use super::{
    ResearchInput,
    windows::{input_identity, update_source_coverage},
};
use linggan_evidence::creator_discovery::Fragment;
use serde_json::json;
use uuid::Uuid;

pub(crate) const CORE_INPUT_CONTRACT: &str = "topic-map.core-source.v1";

pub(crate) fn from_core(input: &ResearchInput, core_ref: Option<Uuid>) -> Option<ResearchInput> {
    let card = input.coverage["availableCores"]
        .as_array()?
        .iter()
        .find(|card| core_ref.is_none_or(|reference| card["analysisRef"] == json!(reference)))?;
    let reference: Uuid = card["analysisRef"].as_str()?.parse().ok()?;
    let method = card["generation"]["methodVersion"].as_str()?;
    let fragments = card_fragments(input.work.work_ref, card)?;
    let mut result = input.clone();
    result.fragments = fragments.clone();
    result.work.fragments = fragments;
    result.comment_study = json!([]);
    result.coverage.as_object_mut()?.remove("availableCores");
    result.coverage["inputContract"] = json!(CORE_INPUT_CONTRACT);
    result.coverage["coreRef"] = json!(reference);
    result.coverage["coreMethod"] = json!(method);
    result.coverage["coreSnapshot"] = card["sourceSnapshot"].clone();
    result.coverage["core"] = card["core"].clone();
    result.role_metadata["sourceRole"] = json!("author_work");
    result.role_metadata["authorTextAvailable"] = json!(true);
    result.role_metadata["commentOnly"] = json!(false);
    update_source_coverage(&mut result);
    result.hash = input_identity(&result);
    Some(result)
}

pub(crate) fn card_fragments(work: Uuid, card: &serde_json::Value) -> Option<Vec<Fragment>> {
    let reference: Uuid = card["analysisRef"].as_str()?.parse().ok()?;
    let method = card["generation"]["methodVersion"].as_str()?;
    let mut fragments = Vec::new();
    for evidence in card["evidence"].as_array()? {
        let id = evidence["id"].as_str()?;
        let units: Vec<_> = card["core"]["knowledgeUnits"]
            .as_array()?
            .iter()
            .filter(|unit| {
                unit["evidenceRefs"]
                    .as_array()
                    .is_some_and(|refs| refs.iter().any(|r| r == id))
            })
            .collect();
        if units.is_empty() {
            continue;
        }
        let image = evidence["anchorKind"] == "image";
        let speaker = if image {
            match evidence["speakerRole"].as_str() {
                Some("author") => "author",
                Some("quoted") => "other",
                _ => "unconfirmed",
            }
        } else if units.iter().all(|u| u["speakerRole"] == "author") {
            "author"
        } else if units.iter().all(|u| u["speakerRole"] == "quoted") {
            "other"
        } else {
            "unconfirmed"
        };
        for key in if image {
            &["modelQuote", "observation"][..]
        } else {
            &["quote"][..]
        } {
            let Some(text) = evidence[*key].as_str().filter(|s| !s.is_empty()) else {
                continue;
            };
            let start = if image {
                0
            } else {
                usize::try_from(evidence["start"].as_u64()?).ok()?
            };
            let source_ref = if image {
                reference
            } else {
                evidence["sourceRef"].as_str()?.parse().ok()?
            };
            let source_version = if image {
                method
            } else {
                evidence["sourceVersion"].as_str()?
            };
            fragments.push(Fragment {
                fragment_id: format!("{}.core.{}.{}.{}", work, reference, id, key),
                source_ref, source_version: source_version.into(),
                field: if image { "core_image_evidence" } else { "core_text_evidence" }.into(),
                start, end: start + text.chars().count(), text: text.into(), speaker: speaker.into(),
                structure: json!({"kind":if image {"core_image_evidence"} else {"core_text_evidence"},
                    "text":text,"coreRef":reference,"evidenceRef":id,"anchor":evidence,"knowledgeUnits":units}),
                ..Default::default()
            });
        }
    }
    if fragments.is_empty() {
        return None;
    }
    Some(fragments)
}
