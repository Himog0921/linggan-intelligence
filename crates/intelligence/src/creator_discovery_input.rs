//! Exact creator request preparation shared by preview and dispatch. No I/O or model calls.
use linggan_evidence::{
    creator_discovery::{DiscoveryWork, Fragment},
    research_dossier,
};
use serde_json::{Value, json};

pub fn prepare(domain: &Value, work: &DiscoveryWork, as_of: &str) -> (Value, Vec<Fragment>) {
    let fragments = work.fragments.clone();
    let materials = provider_materials(&fragments);
    let input = json!({"domain":domain,"workRef":work.work_ref,"asOf":as_of,"sourceFingerprint":work.fingerprint,
        "materialContract":research_dossier::VERSION,"readingTitle":work.reading_title,"materials":materials,
        "dossierRevision":work.dossier_revision,"dossierMaterialRevision":work.dossier_material_revision,
        "coverage":work.dossier_coverage,
        "acquisitionContext":{"kind":work.acquisition_kind,"selectionBiased":true}});
    (input, fragments)
}

pub(crate) fn provider_materials(fragments: &[Fragment]) -> Vec<Value> {
    fragments.iter().enumerate().map(|(index, f)| json!({
        "citation":format!("S{}",index+1),"blockRef":f.fragment_id,"field":f.field,
        "sourceRef":f.source_ref,"sourceVersion":f.source_version,"readingRef":f.reading_ref,
        "unitRef":f.unit_ref,"speaker":f.speaker,"content":f.model_content(),"issues":f.issues
    })).collect()
}

/// Audit metadata only. This map is kept locally and is not part of the model prompt.
pub fn citation_map(fragments: &[Fragment], as_of: &str) -> Value {
    json!(fragments.iter().enumerate().map(|(i,f)| json!({
        "citation":format!("S{}",i+1),"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"asOf":as_of,
        "sourceVersion":f.source_version,"blockRef":f.fragment_id,"readingRef":f.reading_ref,"unitRef":f.unit_ref,"start":f.start,"end":f.end
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
            ..Fragment::default()
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
    #[test]
    fn provider_receives_complete_canonical_content_once_with_source_metadata() {
        let text = format!("{}尾部限制不能省略", "原始正文😀\n".repeat(2000));
        let fragment = Fragment {
            fragment_id: "work.reading.1".into(),
            source_ref: uuid::Uuid::from_u128(1),
            source_version: "exact-source-version".into(),
            reading_ref: Some(uuid::Uuid::from_u128(2)),
            unit_ref: Some(uuid::Uuid::from_u128(3)),
            speaker: "other".into(),
            field: "ocr".into(),
            end: text.chars().count(),
            text: text.clone(),
            structure: json!({"kind":"quote","text":text}),
            issues: vec![json!({"code":"unclear"})],
            ..Fragment::default()
        };
        let value = &provider_materials(std::slice::from_ref(&fragment))[0];
        assert_eq!(value["content"]["text"], text);
        assert_eq!(value["sourceRef"], json!(fragment.source_ref));
        assert_eq!(value["readingRef"], json!(fragment.reading_ref));
        assert_eq!(value["blockRef"], fragment.fragment_id);
        assert_eq!(value["speaker"], "other");
        assert_eq!(value["issues"], json!(fragment.issues));
        assert!(value.get("text").is_none());
    }
}
