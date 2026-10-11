//! Coverage unions and author-only journey summaries across source windows.
use super::*;

#[derive(Default)]
pub(super) struct Coverage {
    spans: HashMap<String, Vec<(usize, usize)>>,
    completed: HashSet<String>,
    partial: HashSet<String>,
    seen: HashSet<String>,
    source_chars: usize,
    total_windows: usize,
}
impl Coverage {
    pub(super) fn add(&mut self, part: &Window) {
        if part.is_comparison() {
            return;
        }
        self.source_chars = self
            .source_chars
            .max(part.input.coverage["sourceChars"].as_u64().unwrap_or(0) as usize);
        self.total_windows = self
            .total_windows
            .max(part.input.coverage["windowCount"].as_u64().unwrap_or(0) as usize);
        // Model/config identities audit a run; they cannot multiply source
        // coverage. Include parent ranges so repeated child text does not hide
        // unprocessed pages of its surrounding conversation.
        let key = semantic_window_key(&part.input);
        // The caller supplies newest first. A new partial revision must not
        // inherit the completed state of its previous analysis.
        if !self.seen.insert(key.clone()) {
            return;
        }
        if part.is_partial() {
            self.partial.insert(key);
        } else {
            self.completed.insert(key);
        }
        for f in part
            .input
            .fragments
            .iter()
            .filter(|f| crate::topic_map_research::is_research_evidence(f))
        {
            // The same source span has one identity even when v1/v2 window IDs differ.
            let origin = crate::topic_map_research::semantic_source_identity(&part.input, f);
            let ranges = self.spans.entry(origin).or_default();
            if part.is_partial() {
                ranges.extend(
                    part.core["units"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .flat_map(|u| u["evidence"].as_array().into_iter().flatten())
                        .filter(|c| c["fragmentId"].as_str() == Some(&f.fragment_id))
                        .filter_map(|c| {
                            Some((c["start"].as_u64()? as usize, c["end"].as_u64()? as usize))
                        }),
                );
            } else {
                ranges.push((f.start, f.end));
            }
        }
    }
    pub(super) fn value(mut self) -> Value {
        let covered = self
            .spans
            .values_mut()
            .map(|ranges| union_chars(ranges))
            .sum::<usize>()
            .min(self.source_chars);
        json!({"sourceChars":self.source_chars,"coveredChars":covered,"totalWindows":self.total_windows,
            "completedWindows":self.completed.len().min(self.total_windows),
            "partialWindows":self.partial.len(),
            "state":if self.partial.is_empty() && self.total_windows>0 && self.completed.len()>=self.total_windows && covered>=self.source_chars{"complete"}else{"partial"},"countsArePeople":false})
    }
}

pub(super) fn semantic_window_key(input: &ResearchInput) -> String {
    let mut fragments: Vec<_> = input
        .fragments
        .iter()
        .map(|fragment| {
            json!({"source":crate::topic_map_research::semantic_source_identity(input,fragment),
            "start":fragment.start,"end":fragment.end,
            "textHash":linggan_evidence::creator_discovery::hash(&fragment.text),
            "contextOnly":!crate::topic_map_research::is_research_evidence(fragment)})
            .to_string()
        })
        .collect();
    fragments.sort();
    fragments.dedup();
    linggan_evidence::creator_discovery::hash(&json!(fragments).to_string())
}

pub(super) fn author_journey(part: &Window) -> Option<&Value> {
    if part.is_comparison() || part.is_partial() {
        return None;
    }
    let journey = &part.output["journey"];
    let author_only = journey["evidence"].as_array().is_some_and(|refs| {
        !refs.is_empty()
            && refs.iter().all(|c| {
                part.input.fragments.iter().any(|f| {
                    c["fragmentId"].as_str() == Some(&f.fragment_id)
                        && ["title", "body", "ocr", "transcript"].contains(&f.field.as_str())
                })
            })
    });
    (author_only
        && journey["mainStage"]
            .as_str()
            .is_some_and(|s| s != "unclear"))
    .then_some(journey)
}
fn union_chars(ranges: &mut [(usize, usize)]) -> usize {
    ranges.sort();
    let (mut total, mut end) = (0, 0);
    for &(a, b) in ranges.iter() {
        if b > end {
            total += b - a.max(end);
            end = b;
        }
    }
    total
}

#[derive(Default)]
pub(super) struct Journey {
    stages: BTreeSet<String>,
    involved: BTreeSet<String>,
    overlays: BTreeSet<String>,
    paths: BTreeSet<String>,
}
impl Journey {
    pub(super) fn add(&mut self, part: &Window) {
        // A commenter's stage and an unfinished draft are not the author's stage.
        let Some(journey) = author_journey(part) else {
            return;
        };
        if let Some(stage) = journey["mainStage"].as_str() {
            self.stages.insert(stage.to_owned());
        }
        for s in journey["involvedStages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            self.involved.insert(s.to_owned());
        }
        for s in journey["overlays"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            self.overlays.insert(s.to_owned());
        }
        if let Some(path) = journey["path"].as_str().filter(|p| *p != "unknown") {
            self.paths.insert(path.to_owned());
        }
    }
    pub(super) fn apply(self, work: &mut TopicMapWork) {
        if self.stages.is_empty() {
            return;
        }
        if work.annotation.as_ref().is_some_and(|a| {
            !a["method_version"]
                .as_str()
                .is_some_and(|s| s.starts_with("topic-map.research"))
        }) {
            return;
        }
        if !self.stages.is_empty() {
            work.main_stage = if self.stages.len() == 1 {
                self.stages
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "unclear".into())
            } else {
                "cross_stage".into()
            };
        }
        work.involved_stages = self.involved.into_iter().collect();
        work.overlays = self.overlays.into_iter().collect();
        work.path = if self.paths.len() > 1 {
            "both".into()
        } else {
            self.paths
                .into_iter()
                .next()
                .unwrap_or_else(|| "unknown".into())
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_unicode_source_ranges_count_once() {
        assert_eq!(union_chars(&mut [(0, 8), (5, 12), (20, 25), (12, 15)]), 20);
        assert_eq!(union_chars(&mut [(100, 112), (100, 112), (104, 108)]), 12);
    }
}
