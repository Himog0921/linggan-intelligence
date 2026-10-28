//! Complete selected dossiers and profile; conservative account claims retain sample limits.
use linggan_evidence::creator_discovery::{self, DiscoveryData, Fragment};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(super) struct AccountInput {
    pub packet: Value,
    fragments: Vec<Fragment>,
    related: HashMap<String, Uuid>,
    broad: HashSet<Uuid>,
}
pub(super) fn prepare(data: &DiscoveryData) -> AccountInput {
    let mut fragments = Vec::new();
    if let Some(w) = data.works.first() {
        if let Some(profile) = creator_discovery::fragment(
            w.work_ref,
            "biography",
            w.profile["biography"].as_str(),
            w.profile["sourceRef"].as_str().and_then(|s| s.parse().ok()),
            None,
        ) {
            fragments.push(profile);
        }
    }
    let mut related = HashMap::new();
    let mut broad = HashSet::new();
    for w in &data.works {
        if w.relevance == "related" {
            for f in &w.fragments {
                related.insert(f.fragment_id.clone(), w.work_ref);
            }
        }
        if w.acquisition_kind == "profile_discovery" {
            broad.insert(w.work_ref);
        }
        fragments.extend(w.fragments.clone());
    }
    let packet = json!({"accountResearch":true,"domain":data.domain,"asOf":data.as_of,
        "materials":crate::creator_discovery_input::provider_materials(&fragments),
        "works":data.works.iter().map(|w|json!({"workRef":w.work_ref,"relevance":w.relevance,"topicHints":w.topic_hints,"acquisitionKind":w.acquisition_kind,"coverage":w.dossier_coverage})).collect::<Vec<_>>(),
        "sampleBasis":if broad.len()>=2 {"profile_discovery_sample"}else{"limited_domain_sample"},"limitations":["仅使用明确选定的领域材料，不代表账号全部作品"]});
    AccountInput {
        packet,
        fragments,
        related,
        broad,
    }
}
pub(super) fn schema() -> Value {
    let field = |values: Vec<&str>| json!({"type":"object","additionalProperties":false,"required":["value","reason","evidenceFragmentIds"],"properties":{"value":{"type":"string","enum":values},"reason":{"type":"string","maxLength":300},"evidenceFragmentIds":{"type":"array","maxItems":12,"items":{"type":"string"}}}});
    json!({"type":"object","additionalProperties":false,"required":["focus","institution_or_brand"],"properties":{"focus":field(vec!["vertical_tendency","multi_topic","unknown"]),"institution_or_brand":field(vec!["yes","no","unknown"])}})
}
pub(super) fn validate(value: &Value, input: &AccountInput) -> Option<Value> {
    if !value.is_object()
        || !value["focus"].is_object()
        || !value["institution_or_brand"].is_object()
    {
        return None;
    }
    let mut aliases = input.fragments.clone();
    for (index, f) in aliases.iter_mut().enumerate() {
        f.fragment_id = format!("S{}", index + 1);
    }
    // First validate alias ownership. Restore canonical IDs before the sample-coverage checks.
    let mut restored = value.clone();
    for key in ["focus", "institution_or_brand"] {
        let Some(ids) = restored[key]["evidenceFragmentIds"].as_array_mut() else {
            return None;
        };
        for id in ids {
            let reference = id.as_str()?;
            let index = aliases.iter().position(|f| f.fragment_id == reference)?;
            *id = json!(input.fragments[index].fragment_id);
        }
    }
    let mut output = crate::creator_discovery_analysis::validate_focus(
        &restored,
        &input.fragments,
        &input.related,
        &input.broad,
    );
    output.as_object_mut()?.remove("evidence");
    output["workRefs"] = json!(
        input.packet["works"]
            .as_array()?
            .iter()
            .map(|w| w["workRef"].clone())
            .collect::<Vec<_>>()
    );
    Some(output)
}
