//! Frozen work-first sampling. Pools are disjoint and never silently filled from another pool.
use super::Settings;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub reading_ref: Uuid,
    pub work_ref: Uuid,
    pub blob_sha256: String,
    pub risk: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    pub candidate: Candidate,
    pub pool: String,
}
pub fn select(seed: &str, candidates: &[Candidate], s: &Settings) -> Vec<Selection> {
    let hash = |key: String| crate::research_text::content_hash(&format!("{seed}:{key}"));
    let mut used = std::collections::HashSet::new();
    let mut out = vec![];
    // Fixed regression is reserved before random; its excluded denominator is frozen explicitly.
    for id in s.regression_refs.iter().take(s.regression_count as usize) {
        if let Some(c) = candidates.iter().find(|c| &c.reading_ref == id) {
            if used.insert(c.reading_ref) {
                out.push(Selection {
                    candidate: c.clone(),
                    pool: "regression".into(),
                });
            }
        }
    }
    let mut works = std::collections::BTreeMap::<Uuid, Vec<&Candidate>>::new();
    for c in candidates
        .iter()
        .filter(|c| !s.regression_refs.contains(&c.reading_ref))
    {
        works.entry(c.work_ref).or_default().push(c);
    }
    for group in works.values_mut() {
        group.sort_by_key(|c| hash(c.reading_ref.to_string()));
    }
    let mut works: Vec<_> = works.into_iter().collect();
    works.sort_by_key(|(w, _)| hash(w.to_string()));
    let mut random = 0;
    'rounds: for round in 0..candidates.len() {
        let mut found = false;
        for (_, group) in &works {
            if random >= s.random_count {
                break 'rounds;
            }
            if let Some(c) = group.get(round) {
                found = true;
                if used.insert(c.reading_ref) {
                    out.push(Selection {
                        candidate: (*c).clone(),
                        pool: "random".into(),
                    });
                    random += 1;
                }
            }
        }
        if !found {
            break;
        }
    }
    let mut risk: Vec<_> = candidates
        .iter()
        .filter(|c| {
            c.risk && !used.contains(&c.reading_ref) && !s.regression_refs.contains(&c.reading_ref)
        })
        .collect();
    risk.sort_by_key(|c| hash(c.reading_ref.to_string()));
    out.extend(
        risk.into_iter()
            .take(s.risk_count as usize)
            .map(|c| Selection {
                candidate: c.clone(),
                pool: "risk".into(),
            }),
    );
    out
}
