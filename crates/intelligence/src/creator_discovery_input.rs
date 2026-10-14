//! Exact creator request preparation shared by preview and dispatch. No I/O or model calls.
use crate::{creator_discovery_analysis::SYSTEM, model_settings::ModelError};
use linggan_evidence::{
    creator_discovery::{DiscoveryWork, Fragment},
    research_dossier,
};
use serde_json::{Value, json};

pub(crate) const ENVELOPE_BYTES: usize = 128;

pub fn prepare(
    domain: &Value,
    work: &DiscoveryWork,
    input_budget: usize,
) -> Result<(Value, Vec<Fragment>), ModelError> {
    let mut fragments = work.fragments.clone();
    loop {
        let manifest: Vec<_> = fragments
            .iter()
            .enumerate()
            .map(|(i, f)| json!({"fragmentId":format!("S{}",i+1),"field":f.field}))
            .collect();
        let input = json!({"domain":domain,"workRef":work.work_ref,"sourceFingerprint":work.fingerprint,
            "materialContract":research_dossier::VERSION,"materials":research_dossier::render_materials(&fragments),
            "fragments":manifest,"acquisitionContext":{"kind":work.acquisition_kind,"selectionBiased":true}});
        if input.to_string().len() + SYSTEM.len() + ENVELOPE_BYTES <= input_budget {
            return Ok((input, fragments));
        }
        let Some(longest) = fragments.iter_mut().max_by_key(|f| f.text.chars().count()) else {
            return Err(ModelError::InputLimit);
        };
        let length = longest.text.chars().count();
        if length <= 1 {
            return Err(ModelError::InputLimit);
        }
        longest.text = longest.text.chars().take(length / 2).collect();
        longest.end = longest.start + longest.text.chars().count();
    }
}

/// Audit metadata only. This map is kept locally and is not part of the model prompt.
pub fn citation_map(fragments: &[Fragment]) -> Value {
    json!(fragments.iter().enumerate().map(|(i,f)| json!({
        "citation":format!("S{}",i+1),"fragmentId":f.fragment_id,"sourceRef":f.source_ref,
        "sourceVersion":f.source_version,"start":f.start,"end":f.end
    })).collect::<Vec<_>>())
}

/// Validate only aliases actually supplied to this call, then restore canonical IDs for
/// stored analyses/manual support. Provider output never chooses a source mapping.
pub(crate) fn validate_response(raw: &Value, fragments: &[Fragment]) -> Value {
    let aliases: Vec<_> = fragments
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let mut alias = f.clone();
            alias.fragment_id = format!("S{}", i + 1);
            alias
        })
        .collect();
    let mut validated = crate::creator_discovery_analysis::validate_work(raw, &aliases);
    fn restore(value: &mut Value, originals: &[Fragment]) {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    if key == "evidenceFragmentIds" {
                        if let Some(ids) = child.as_array_mut() {
                            for id in ids {
                                if let Some(index) = id
                                    .as_str()
                                    .and_then(|s| s.strip_prefix('S'))
                                    .and_then(|s| s.parse::<usize>().ok())
                                    .and_then(|n| n.checked_sub(1))
                                {
                                    *id = json!(originals[index].fragment_id);
                                }
                            }
                        }
                    } else {
                        restore(child, originals);
                    }
                }
            }
            Value::Array(values) => {
                for child in values {
                    restore(child, originals);
                }
            }
            _ => {}
        }
    }
    restore(&mut validated, fragments);
    validated
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_citations_restore_only_supplied_sources() {
        let f = Fragment {
            fragment_id: "canonical.body.hash".into(),
            source_ref: uuid::Uuid::nil(),
            source_version: "v1".into(),
            field: "body".into(),
            start: 0,
            end: 2,
            text: "材料".into(),
        };
        let output = |id: &str| json!({"relevance":{"value":"related","reason":"材料支持","evidenceFragmentIds":[id]}});
        let fragments = vec![f];
        assert_eq!(
            validate_response(&output("S1"), &fragments)["relevance"]["evidenceFragmentIds"][0],
            "canonical.body.hash"
        );
        for invalid in ["S2", "S0", "S01", "canonical.body.hash"] {
            assert_eq!(
                validate_response(&output(invalid), &fragments)["relevance"]["value"],
                "unknown"
            );
        }
    }
}
