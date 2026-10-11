use super::*;
use std::collections::BTreeMap;

pub(super) fn merge_spans(spans: impl IntoIterator<Item = Span>) -> Vec<Span> {
    let mut sources = BTreeMap::<(Uuid, String, Option<String>), Vec<Span>>::new();
    for span in spans {
        sources
            .entry((span.work, span.origin.clone(), span.context_field.clone()))
            .or_default()
            .push(span);
    }
    let mut merged: Vec<Span> = Vec::new();
    for (_, mut spans) in sources {
        spans.sort_by_key(|s| (s.start, s.end));
        for span in spans {
            if let Some(previous) = merged.last_mut()
                && previous.work == span.work
                && previous.origin == span.origin
                && previous.context_field == span.context_field
                && span.start <= previous.end
            {
                previous.end = previous.end.max(span.end);
            } else {
                merged.push(span);
            }
        }
    }
    merged
}

pub(super) fn selected_spans(selected: &[Candidate]) -> Vec<Span> {
    merge_spans(selected.iter().flat_map(|c| c.spans.iter().cloned()))
}

pub(super) fn cost(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.end - s.start).sum()
}

pub(super) fn shared_topics(candidates: &[Candidate]) -> BTreeSet<String> {
    let mut members = BTreeMap::<String, BTreeSet<Uuid>>::new();
    for candidate in candidates {
        if !["matched", "new"].contains(&candidate.unit["status"].as_str().unwrap_or("")) {
            continue;
        }
        for assignment in candidate.unit["assignments"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(topic) = assignment["topicRef"].as_str() {
                members
                    .entry(topic.into())
                    .or_default()
                    .insert(candidate.work);
            }
        }
    }
    members
        .into_iter()
        .filter(|(_, works)| works.len() > 1)
        .map(|(topic, _)| topic)
        .collect()
}

fn rank(candidate: &Candidate, common: &BTreeSet<String>) -> (bool, bool, bool, bool) {
    (
        candidate.unit["assignments"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|a| a["topicRef"].as_str().is_some_and(|t| common.contains(t))),
        candidate.unit["evidenceRole"] == "challenge",
        candidate.unit["assignments"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        matches!(
            candidate.unit["speakerRole"].as_str(),
            Some("author" | "commenter")
        ),
    )
}

pub(super) fn choose(candidates: &[Candidate], requested: &[Uuid]) -> Vec<Candidate> {
    if requested.len() == 1 {
        return choose_author_response(candidates, requested[0]);
    }
    let common = shared_topics(candidates);
    let mut by_work = BTreeMap::<Uuid, Vec<Candidate>>::new();
    for candidate in candidates {
        by_work
            .entry(candidate.work)
            .or_default()
            .push(candidate.clone());
    }
    for rows in by_work.values_mut() {
        rows.sort_by(|a, b| {
            rank(b, &common)
                .cmp(&rank(a, &common))
                .then(cost(&a.spans).cmp(&cost(&b.spans)))
                .then(a.unit["unitId"].as_str().cmp(&b.unit["unitId"].as_str()))
        });
    }
    let mut selected: Vec<Candidate> = Vec::new();
    // One fair share first; large whole citations can use spare capacity in the next pass.
    let fair_share = MAX_CHARS / requested.len().max(1);
    for limit in [fair_share, MAX_CHARS] {
        for work in requested {
            if selected.iter().any(|c| c.work == *work) {
                continue;
            }
            let Some(rows) = by_work.get(work) else {
                continue;
            };
            if let Some(candidate) = rows.iter().find(|c| {
                cost(&c.spans) <= limit
                    && cost(&merge_spans(
                        selected_spans(&selected)
                            .into_iter()
                            .chain(c.spans.iter().cloned()),
                    )) <= MAX_CHARS
            }) {
                selected.push(candidate.clone());
            }
        }
    }
    for work in requested {
        let existing: Vec<_> = selected
            .iter()
            .filter(|c| c.work == *work)
            .cloned()
            .collect();
        if existing.is_empty() || existing.len() >= MAX_UNITS_PER_WORK {
            continue;
        }
        let Some(rows) = by_work.get(work) else {
            continue;
        };
        let mut remaining: Vec<_> = rows
            .iter()
            .filter(|c| {
                !existing
                    .iter()
                    .any(|e| e.unit["unitId"] == c.unit["unitId"])
            })
            .collect();
        remaining.sort_by_key(|c| {
            existing.iter().any(|e| {
                e.unit["speakerRole"] == c.unit["speakerRole"]
                    && e.unit["evidenceRole"] == c.unit["evidenceRole"]
            })
        });
        if let Some(candidate) = remaining.into_iter().find(|c| {
            cost(&merge_spans(
                selected_spans(&selected)
                    .into_iter()
                    .chain(c.spans.iter().cloned()),
            )) <= MAX_CHARS
        }) {
            selected.push(candidate.clone());
        }
    }
    selected
}

fn choose_author_response(candidates: &[Candidate], work: Uuid) -> Vec<Candidate> {
    let mut roles = BTreeMap::<&str, Vec<&Candidate>>::new();
    for candidate in candidates.iter().filter(|candidate| candidate.work == work) {
        if let Some(role @ ("author" | "commenter")) = candidate.unit["speakerRole"].as_str() {
            roles.entry(role).or_default().push(candidate);
        }
    }
    let common = BTreeSet::new();
    for rows in roles.values_mut() {
        rows.sort_by(|a, b| {
            rank(b, &common)
                .cmp(&rank(a, &common))
                .then(cost(&a.spans).cmp(&cost(&b.spans)))
                .then(a.unit["unitId"].as_str().cmp(&b.unit["unitId"].as_str()))
        });
    }
    let Some(authors) = roles.get("author") else {
        return Vec::new();
    };
    let Some(commenters) = roles.get("commenter") else {
        return Vec::new();
    };
    // A response may expose an unmatched need. Prefer shared canonical topics,
    // then retain an honest author/commenter pair even if no topic is shared.
    for require_shared in [true, false] {
        for author in authors {
            for commenter in commenters {
                let shared = author.unit["assignments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|a| {
                        commenter.unit["assignments"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|b| {
                                a["topicRef"] == b["topicRef"]
                                    && a["definitionRef"] == b["definitionRef"]
                            })
                    });
                if require_shared && !shared {
                    continue;
                }
                if cost(&merge_spans(
                    author
                        .spans
                        .iter()
                        .cloned()
                        .chain(commenter.spans.iter().cloned()),
                )) <= MAX_CHARS
                {
                    return vec![(*author).clone(), (*commenter).clone()];
                }
            }
        }
    }
    Vec::new()
}
