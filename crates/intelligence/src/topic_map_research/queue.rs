//! Incremental source work, explicit retry rules and one frozen run budget.
use super::ResearchInput;
use super::windows::{is_research_evidence, origin_id};
use linggan_evidence::creator_discovery;
use serde_json::{Value, json};

/// A different config, method or window size cannot authorize replaying a source
/// range whose provider dispatch is still unknown. New qualified text can advance.
pub(crate) fn overlaps_unknown_dispatch(
    window: &ResearchInput,
    full: &ResearchInput,
    refs: &[Value],
) -> bool {
    fn same_field(a: &str, b: &str) -> bool {
        a == b
            || (["studied_comment", "unresearched_comment"].contains(&a)
                && ["studied_comment", "unresearched_comment"].contains(&b))
    }
    window
        .fragments
        .iter()
        .filter(|f| is_research_evidence(f))
        .any(|fragment| {
            let origin = origin_id(window, fragment);
            let Some(source) = full
                .fragments
                .iter()
                .find(|source| source.fragment_id == origin)
            else {
                return false;
            };
            refs.iter().any(|reference| {
                let Some(field) = reference["field"].as_str() else {
                    return false;
                };
                let owner = reference["workRef"]
                    .as_str()
                    .or_else(|| reference["fragmentId"].as_str()?.split('.').next());
                if owner != origin.split('.').next() {
                    return false;
                }
                if !same_field(field, &source.field)
                    || !(reference["sourceRef"] == json!(source.source_ref)
                        || reference["sourceFragmentId"].as_str() == Some(origin.as_str())
                        || reference["fragmentId"].as_str() == Some(origin.as_str())
                        || (matches!(field, "body" | "title")
                            && reference["workRef"].as_str() == origin.split('.').next()))
                {
                    return false;
                }
                let Some(start) = reference["start"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    return false;
                };
                let Some(end) = reference["end"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                else {
                    return false;
                };
                if start >= end
                    || start < source.start
                    || end > source.end
                    || fragment.start >= end
                    || fragment.end <= start
                {
                    return false;
                }
                let text: String = source
                    .text
                    .chars()
                    .skip(start - source.start)
                    .take(end - start)
                    .collect();
                reference["textHash"].as_str() == Some(creator_discovery::hash(&text).as_str())
            })
        })
}
