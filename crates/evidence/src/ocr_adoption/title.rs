use super::quality::{fragment, normalized};
use super::*;

pub(super) fn select(input: &ImageInput, regions: &mut [RegionAssessment]) -> CoverTitle {
    let mut result = CoverTitle {
        state: TitleState::None,
        text: None,
        fragments: vec![],
        reasons: vec![],
        candidates: vec![],
    };
    if !input.layout.completed {
        result.state = TitleState::Unavailable;
        result.reasons.push(Reason::ExecutionIncomplete);
        return result;
    }
    if input.image_role == ImageRole::Interior {
        result.state = TitleState::NotCover;
        result.reasons.push(Reason::InteriorImage);
        return result;
    }
    let heights: Vec<_> = input
        .layout
        .blocks
        .iter()
        .zip(regions.iter())
        .map(|(b, r)| {
            r.median_line_height_norm.unwrap_or_else(|| {
                (b.bbox[3] - b.bbox[1])
                    / f64::from(input.image.height)
                    / b.text.lines().count().max(1) as f64
            })
        })
        .collect();
    result.candidates = candidate_groups(input, regions, &heights)
        .iter()
        .map(|g| candidate(input, regions, &heights, g))
        .collect();
    // Area separates a dominant group from a small distant one. Ties stay uncertain;
    // this is a visible feature, not a probability or an opaque title score.
    result
        .candidates
        .sort_by(|a, b| area(b.bbox).total_cmp(&area(a.bbox)));
    let Some(first) = result.candidates.first() else {
        result.reasons.push(Reason::NoProminentGroup);
        if input.image_role == ImageRole::Unknown {
            result.state = TitleState::Uncertain;
            result.reasons.push(Reason::UnknownCoverIdentity);
        }
        return result;
    };
    let uncertainty = if input.image_role == ImageRole::Unknown {
        Some(Reason::UnknownCoverIdentity)
    } else if result
        .candidates
        .get(1)
        .is_some_and(|c| area(first.bbox) < area(c.bbox) * 1.8)
    {
        Some(Reason::CompetingTitleGroups)
    } else if !first.supported {
        Some(
            if regions.iter().any(|r| {
                first.block_refs.contains(&r.block_ref)
                    && r.reasons.contains(&Reason::MathematicalLayoutUnverified)
            }) {
                Reason::MathematicalLayoutUnverified
            } else {
                Reason::UnsupportedCandidate
            },
        )
    } else {
        None
    };
    if let Some(reason) = uncertainty {
        result.state = TitleState::Uncertain;
        result.reasons.push(reason);
    } else {
        result.state = TitleState::Usable;
        result.text = Some(first.text.clone());
        result.fragments = first.fragments.clone();
        result.reasons.push(Reason::ProminentConnectedGroup);
    }
    update_roles(input, regions, &heights, &result);
    result
}

fn candidate_groups(
    input: &ImageInput,
    regions: &[RegionAssessment],
    heights: &[f64],
) -> Vec<Vec<usize>> {
    let eligible = |i: usize| {
        let b = &input.layout.blocks[i];
        let n = normalized(&b.text).chars().count();
        matches!(b.label.as_str(), "text" | "doc_title" | "paragraph_title")
            && !b.text.contains('<')
            && !matches!(
                regions[i].role,
                Role::LayoutMetadata | Role::Background | Role::NonNarrative | Role::Uncertain
            )
            && (2..=160).contains(&n)
            && heights[i] >= 0.04
    };
    let max_height = (0..regions.len())
        .filter(|i| eligible(*i))
        .map(|i| heights[i])
        .fold(0.0, f64::max);
    let mut remaining: Vec<_> = (0..regions.len())
        .filter(|i| eligible(*i) && heights[*i] >= max_height * 0.45)
        .collect();
    remaining.sort_by_key(|i| input.layout.blocks[*i].reading_order);
    let mut groups = vec![];
    while let Some(seed) = remaining
        .iter()
        .position(|i| heights[*i] >= max_height * 0.70)
    {
        let first = remaining.remove(seed);
        let mut group = vec![first];
        loop {
            let mut additions = vec![];
            remaining.retain(|i| {
                if group.iter().any(|j| {
                    if heights[*i].min(heights[*j]) >= max_height * 0.70 {
                        connects(input, *i, *j, heights)
                    } else {
                        title_continuation(input, *i, *j, heights)
                    }
                }) {
                    additions.push(*i);
                    false
                } else {
                    true
                }
            });
            if additions.is_empty() {
                break;
            }
            group.extend(additions);
        }
        group.sort_by_key(|i| input.layout.blocks[*i].reading_order);
        groups.push(group);
    }
    groups
}

fn candidate(
    input: &ImageInput,
    regions: &[RegionAssessment],
    heights: &[f64],
    group: &[usize],
) -> TitleCandidate {
    let mut bbox = [f64::MAX, f64::MAX, 0.0_f64, 0.0_f64];
    for i in group {
        let b = input.layout.blocks[*i].bbox;
        bbox = [
            bbox[0].min(b[0]),
            bbox[1].min(b[1]),
            bbox[2].max(b[2]),
            bbox[3].max(b[3]),
        ];
    }
    TitleCandidate {
        block_refs: group
            .iter()
            .map(|i| input.layout.blocks[*i].block_ref.clone())
            .collect(),
        text: group
            .iter()
            .map(|i| input.layout.blocks[*i].text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        fragments: group
            .iter()
            .map(|i| {
                let b = &input.layout.blocks[*i];
                fragment(SourceKind::Block, &b.block_ref, &b.text, b.bbox)
            })
            .collect(),
        bbox,
        line_height_norm: group.iter().map(|i| heights[*i]).fold(0.0, f64::max),
        supported: group
            .iter()
            .all(|i| regions[*i].adoption_state == AdoptionState::AdoptedBlock),
    }
}

fn update_roles(
    input: &ImageInput,
    regions: &mut [RegionAssessment],
    heights: &[f64],
    result: &CoverTitle,
) {
    let Some(first) = result.candidates.first() else {
        return;
    };
    for (i, r) in regions.iter_mut().enumerate() {
        if first.block_refs.contains(&r.block_ref) {
            r.role = if result.state == TitleState::Usable {
                Role::CoverTitle
            } else {
                Role::Uncertain
            };
        } else if result
            .candidates
            .iter()
            .skip(1)
            .any(|c| c.block_refs.contains(&r.block_ref))
        {
            r.role = Role::OtherTitleGroup;
            r.reasons.push(Reason::SecondaryDistantGroup);
        } else if r.role == Role::ImageHeading && heights[i] < first.line_height_norm * 0.70 {
            // Context is deliberately broader than "subtitle": geometry cannot prove
            // whether small distant text is dialogue, a reference or an app control.
            r.role = Role::Context;
            r.reasons.push(
                if input.layout.blocks[i].bbox[1] / f64::from(input.image.height) < 0.08 {
                    Reason::SmallEdgeMetadata
                } else {
                    Reason::SecondaryDistantGroup
                },
            );
        }
    }
}

fn area(b: [f64; 4]) -> f64 {
    (b[2] - b[0]) * (b[3] - b[1])
}
fn connects(input: &ImageInput, i: usize, j: usize, heights: &[f64]) -> bool {
    let a = input.layout.blocks[i].bbox;
    let b = input.layout.blocks[j].bbox;
    let scale = heights[i].min(heights[j]) * f64::from(input.image.height);
    let vertical_gap = (a[1] - b[3]).max(b[1] - a[3]);
    let horizontal_overlap = a[2].min(b[2]) - a[0].max(b[0]);
    vertical_gap >= -scale * 0.5 && vertical_gap <= scale * 1.5 && horizontal_overlap > 0.0
}

// A smaller, tightly adjacent line can complete the same visual title. It cannot seed
// a title by itself or bridge the larger whitespace before a separate body paragraph.
fn title_continuation(input: &ImageInput, i: usize, j: usize, heights: &[f64]) -> bool {
    let a = input.layout.blocks[i].bbox;
    let b = input.layout.blocks[j].bbox;
    let scale = heights[i].min(heights[j]) * f64::from(input.image.height);
    let gap = (a[1] - b[3]).max(b[1] - a[3]);
    let aligned = (a[0] - b[0]).abs() <= f64::from(input.image.width) * 0.05
        || ((a[0] + a[2]) - (b[0] + b[2])).abs() <= f64::from(input.image.width) * 0.10;
    gap >= 0.0 && gap <= scale * 0.85 && aligned
}
