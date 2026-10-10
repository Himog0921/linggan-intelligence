//! Evidence units and explicit concept decisions. Recall is never an admission rule.
#[path = "topic_map_core/acceptance.rs"]
mod acceptance;
pub(crate) use acceptance::{Acceptance, accept_in};
#[path = "topic_map_core/recall.rs"]
mod recall;
pub(crate) use recall::{catalog, catalog_in, catalog_version, recall_topics};
#[path = "topic_map_core/source_qualification.rs"]
mod source_qualification;
pub(crate) use source_qualification::{definition_dependencies, unavailable_definitions};
#[path = "topic_map_core/backfill.rs"]
pub(crate) mod backfill;
#[path = "topic_map_core/comparison.rs"]
pub(crate) mod comparison;

use crate::{
    topic_map_research::ResearchInput,
    topic_map_research_analysis::{Discussion, ResearchOutput},
};
use linggan_evidence::creator_discovery::hash;
use serde_json::{Value, json};
use uuid::Uuid;

pub const METHOD_VERSION: &str = "topic-map.core.v1";

/// Source coordinates, roles and statement identify a discussion, not its batch or display name.
pub(crate) fn units(input: &ResearchInput, output: &ResearchOutput) -> Vec<(String, Discussion)> {
    let mut seen = std::collections::HashSet::new();
    output
        .discussions
        .iter()
        .filter_map(|d| {
            let mut evidence: Vec<String> = d
                .evidence
                .iter()
                .filter_map(|c| {
                    input.fragments.iter().find(|f| f.fragment_id == c.fragment_id).map(|f| {
                let quote: String = f.text.chars().skip(c.start.saturating_sub(f.start)).take(c.end.saturating_sub(c.start)).collect();
                json!({"source":crate::topic_map_research::semantic_source_identity(input,f),
                    "start":c.start,"end":c.end,"textHash":hash(&quote)}).to_string()
            })
                })
                .collect();
            evidence.sort();
            evidence.dedup();
            let key = hash(
                &json!({"domain":input.domain["domainRef"],"work":input.work.work_ref,"evidence":evidence,
            "statement":d.statement.split_whitespace().collect::<Vec<_>>().join(" "),
            "speaker":d.speaker_role,"role":d.evidence_role,"method":METHOD_VERSION})
                .to_string(),
            );
            seen.insert(key.clone()).then(|| (key, d.clone()))
        })
        .collect()
}

pub(crate) fn concept_identity(
    definition: &str,
    included: &[String],
    excluded: &[String],
) -> String {
    let normalize = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let mut included: Vec<_> = included.iter().map(|s| normalize(s)).collect();
    let mut excluded: Vec<_> = excluded.iter().map(|s| normalize(s)).collect();
    included.sort();
    included.dedup();
    excluded.sort();
    excluded.dedup();
    hash(
        &json!({"definition":normalize(definition),"included":included,"excluded":excluded})
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_boundary_deduplicates_across_labels_but_different_boundaries_do_not() {
        let a = concept_identity(
            "开始任务遇到困难",
            &["开始前迟迟无法行动".into()],
            &["已经开始后分心".into()],
        );
        assert_eq!(
            a,
            concept_identity(
                " 开始任务遇到困难 ",
                &["开始前迟迟无法行动".into()],
                &["已经开始后分心".into()]
            )
        );
        assert_ne!(
            a,
            concept_identity(
                "任务持续困难",
                &["已经开始后分心".into()],
                &["开始前迟迟无法行动".into()]
            )
        );
        assert_ne!(
            concept_identity(
                "not able",
                &["explicit condition".into()],
                &["other condition".into()]
            ),
            concept_identity(
                "notable",
                &["explicit condition".into()],
                &["other condition".into()]
            )
        );
    }
}
