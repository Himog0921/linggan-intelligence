//! Saved v1 decisions retain only the identity that was present in their manifest.
use super::*;

pub(super) fn definitions_current(wrapper: &Value, unavailable: &HashSet<Uuid>) -> bool {
    crate::topic_map_core::definition_dependencies(wrapper).is_disjoint(unavailable)
}

pub(super) fn legacy_units(
    result: Uuid,
    typed: &crate::topic_map_research_analysis::ResearchOutput,
    manifest: &Value,
) -> Value {
    json!({"units":typed.discussions.iter().enumerate().map(|(i,d)|json!({
        "unitId":format!("legacy:{result}:{i}"),"label":d.label,"statement":d.label,
        "speakerRole":"unknown","evidenceRole":"context","rationale":"旧版研究尚未补齐主题边界与观点角色。",
        "evidence":d.evidence,"status":if d.topic_ref.is_some(){"matched"}else{"uncertain"},
        "assignments":d.topic_ref.map(|t|json!([{"topicRef":t,"definitionRef":manifest["topics"].as_array().into_iter().flatten().find(|topic|topic["topicRef"].as_str()==Some(&t.to_string())).map(|topic|topic["definitionRef"].clone()),
            "label":d.label,"reason":"旧版主题引用；边界待补齐"}])).unwrap_or(json!([])),
        "relations":[],"reason":"旧版结果继续可读；不作为新方法完整覆盖。"})).collect::<Vec<_>>()})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_output_requires_every_definition_in_its_frozen_prompt_to_remain_qualified() {
        let (restricted, manual) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let unavailable = HashSet::from([restricted]);
        for wrapper in [
            json!({"topics":[{"definitionRef":restricted}]}),
            json!({"source":{"topics":[{"definitionRef":restricted}]}}),
        ] {
            assert!(!definitions_current(&wrapper, &unavailable));
        }
        assert!(definitions_current(
            &json!({"topics":[{"definitionRef":manual}],"label":restricted.to_string()}),
            &unavailable,
        ));
    }
}
