//! Counts describe only the explicitly supplied offline set. Secondary directions never add counts.
use super::{Confidence, Input, Invalid, Output, Perspective, Relevance, validate_output};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub struct NoteStatistics<'a> {
    pub input: &'a Input,
    pub output: &'a Output,
    pub likes: Option<u64>,
    /// Caller-declared publication week, not an inferred platform timestamp.
    pub published_week: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DirectionStatistics {
    pub direction_ref: Uuid,
    pub count: usize,
    pub possible: usize,
    pub over1000: usize,
    pub over5000: usize,
    pub unknown_likes: usize,
    pub unknown_week: usize,
    pub weeks: BTreeMap<String, usize>,
    pub perspectives: BTreeMap<String, usize>,
    pub keywords: Vec<KeywordCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeywordCount {
    pub text: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub total: usize,
    pub unknown: usize,
    pub not_related: usize,
    pub no_suitable_direction: usize,
    pub unknown_likes: usize,
    pub unknown_week: usize,
    pub directions: Vec<DirectionStatistics>,
}

fn perspective_name(p: Perspective) -> &'static str {
    match p {
        Perspective::ParentLivedExperience => "家长亲历",
        Perspective::AdultSelfReport => "成人自述",
        Perspective::ProfessionalEducation => "专业科普",
        Perspective::OrganizationPromotion => "机构推广",
        Perspective::Other => "其他",
        Perspective::Unknown => "看不出",
    }
}

pub fn summarize(notes: &[NoteStatistics<'_>]) -> Result<Summary, Invalid> {
    let mut summary = Summary::default();
    let mut seen = BTreeSet::new();
    let mut groups = BTreeMap::<Uuid, DirectionStatistics>::new();
    let mut keywords = BTreeMap::<Uuid, BTreeMap<String, usize>>::new();
    let scope = notes.first().map(|n| {
        (
            n.input.material.domain_ref,
            n.input.direction_version_ref,
            n.input.skill_version_ref,
        )
    });
    for note in notes {
        validate_output(note.input, note.output)?;
        if Some((
            note.input.material.domain_ref,
            note.input.direction_version_ref,
            note.input.skill_version_ref,
        )) != scope
            || !seen.insert(note.input.material.work_ref)
        {
            return Err(Invalid("offline_statistics_scope_or_duplicate"));
        }
        summary.total += 1;
        summary.unknown_likes += usize::from(note.likes.is_none());
        summary.unknown_week += usize::from(note.published_week.is_none());
        match note.output.relevance {
            Relevance::Unknown => summary.unknown += 1,
            Relevance::NotRelated => summary.not_related += 1,
            Relevance::Related => {
                let Some(reference) = note.output.primary_direction else {
                    summary.no_suitable_direction += 1;
                    continue;
                };
                let group = groups
                    .entry(reference)
                    .or_insert_with(|| DirectionStatistics {
                        direction_ref: reference,
                        ..Default::default()
                    });
                group.count += 1;
                group.possible += usize::from(note.output.confidence == Confidence::Possible);
                group.over1000 += usize::from(note.likes.is_some_and(|n| n > 1000));
                group.over5000 += usize::from(note.likes.is_some_and(|n| n > 5000));
                group.unknown_likes += usize::from(note.likes.is_none());
                if let Some(week) = note.published_week {
                    *group.weeks.entry(week.into()).or_default() += 1;
                } else {
                    group.unknown_week += 1;
                }
                *group
                    .perspectives
                    .entry(perspective_name(note.output.perspective).into())
                    .or_default() += 1;
                for word in note
                    .output
                    .keywords
                    .iter()
                    .map(|k| &k.text)
                    .collect::<BTreeSet<_>>()
                {
                    *keywords
                        .entry(reference)
                        .or_default()
                        .entry(word.clone())
                        .or_default() += 1;
                }
            }
        }
    }
    for (reference, mut group) in groups {
        group.keywords = keywords
            .remove(&reference)
            .unwrap_or_default()
            .into_iter()
            .map(|(text, count)| KeywordCount { text, count })
            .collect();
        group
            .keywords
            .sort_by(|a, b| b.count.cmp(&a.count).then(a.text.cmp(&b.text)));
        group.keywords.truncate(5);
        summary.directions.push(group);
    }
    Ok(summary)
}
