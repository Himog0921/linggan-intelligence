use super::*;
use unicode_normalization::UnicodeNormalization;

pub(super) fn normalized(text: &str) -> String {
    text.nfkc()
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub(super) fn repeated(text: &str) -> bool {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    let mut run = 0;
    let mut previous = None;
    for &c in &chars {
        if Some(c) == previous {
            run += 1;
        } else {
            run = 1;
            previous = Some(c);
        }
        if run >= 32 {
            return true;
        }
    }
    // Adjacent cycles, not ordinary repeated words appearing at distant positions.
    for period in 2..=12 {
        let mut matched = 0;
        for i in period..chars.len() {
            if chars[i] == chars[i - period] {
                matched += 1;
            } else {
                matched = 0;
            }
            if matched + period >= (period * 8).max(64) {
                return true;
            }
        }
    }
    false
}

pub(super) fn fragment(kind: SourceKind, id: &str, text: &str, bbox: [f64; 4]) -> Fragment {
    Fragment {
        source_kind: kind,
        source_ref: id.into(),
        start: 0,
        end: text.chars().count(),
        bbox,
        text: text.into(),
    }
}

pub(super) fn supported_lines<'a>(input: &'a ImageInput, b: &Block) -> Vec<&'a Line> {
    let mut lines: Vec<_> = input
        .anchors
        .lines
        .iter()
        .filter(|l| {
            let overlap = (l.bbox[2].min(b.bbox[2]) - l.bbox[0].max(b.bbox[0])).max(0.0)
                * (l.bbox[3].min(b.bbox[3]) - l.bbox[1].max(b.bbox[1])).max(0.0);
            let area = (l.bbox[2] - l.bbox[0]) * (l.bbox[3] - l.bbox[1]);
            l.confidence >= 0.70 && overlap / area >= 0.60
        })
        .collect();
    // The saved line order is source evidence; do not invent cross-column reading order.
    lines.sort_by_key(|l| l.reading_order);
    lines
}

pub(super) fn assess_regions(input: &ImageInput) -> Vec<RegionAssessment> {
    input
        .layout
        .blocks
        .iter()
        .map(|b| assess_region(input, b))
        .collect()
}

fn assess_region(input: &ImageInput, b: &Block) -> RegionAssessment {
    let lines = supported_lines(input, b);
    let line_text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let norm = normalized(&b.text);
    let matches = !norm.is_empty() && norm == normalized(&line_text);
    let finish = input
        .layout
        .terminations
        .iter()
        .find(|r| r.block_ref == b.block_ref)
        .and_then(|r| r.finish_reason.clone());
    let mut r = RegionAssessment {
        block_ref: b.block_ref.clone(),
        reading_order: b.reading_order,
        bbox: b.bbox,
        model_label: b.label.clone(),
        quality: Quality::Supported,
        role: if matches!(b.label.as_str(), "doc_title" | "paragraph_title") {
            Role::ImageHeading
        } else {
            Role::TextCandidate
        },
        reasons: vec![],
        finish_reason: finish,
        support_line_refs: lines.iter().map(|l| l.line_ref.clone()).collect(),
        normalized_text_matches: matches,
        median_line_height_norm: median_height(input, &lines),
        usable_fragments: vec![],
    };
    execution_quality(input, b, &mut r);
    content_quality(b, &mut r);
    if !b.text.trim().is_empty() && !matches {
        if r.quality != Quality::Rejected {
            r.quality = Quality::NeedsReview;
        }
        r.reasons.push(if lines.is_empty() {
            Reason::NoLineSupport
        } else {
            Reason::AnchorDisagreement
        });
    } else if matches {
        r.reasons.push(Reason::MatchingLineText);
    }
    r.usable_fragments = usable_fragments(input, b, &r, &lines);
    if r.usable_fragments
        .iter()
        .any(|f| f.source_kind == SourceKind::Line)
    {
        r.reasons.push(Reason::LineFallback);
    }
    r
}

fn median_height(input: &ImageInput, lines: &[&Line]) -> Option<f64> {
    let mut heights: Vec<_> = lines
        .iter()
        .map(|l| (l.bbox[3] - l.bbox[1]) / f64::from(input.image.height))
        .collect();
    heights.sort_by(f64::total_cmp);
    heights.get(heights.len() / 2).copied()
}

fn execution_quality(input: &ImageInput, b: &Block, r: &mut RegionAssessment) {
    match r.finish_reason.as_deref() {
        None => r.reasons.push(Reason::FinishReasonMissing),
        Some("stop") => {}
        Some(other) => {
            r.quality = Quality::Rejected;
            r.reasons.push(if other == "length" {
                Reason::OutputTruncated
            } else {
                Reason::TerminationIncomplete
            });
        }
    }
    if !input.layout.completed {
        r.quality = Quality::Rejected;
        r.reasons.push(Reason::ExecutionIncomplete);
    }
    if repeated(&b.text) {
        r.quality = Quality::Rejected;
        r.reasons.push(Reason::ExcessiveRepetition);
    }
    let compact: Vec<_> = b.text.chars().filter(|c| !c.is_whitespace()).collect();
    let decoration = !r.normalized_text_matches
        && compact.len() >= 8
        && compact.iter().all(|c| *c == compact[0])
        && (b.bbox[3] - b.bbox[1]) / f64::from(input.image.height) < 0.035;
    if decoration {
        r.quality = Quality::Rejected;
        r.reasons.push(Reason::DecorationLike);
        r.role = Role::NonNarrative;
    }
}

fn content_quality(b: &Block, r: &mut RegionAssessment) {
    if b.text.trim().is_empty() {
        r.role = Role::NonNarrative;
        r.reasons.push(Reason::EmptyText);
    } else if matches!(b.label.as_str(), "image" | "chart" | "seal") {
        if r.quality != Quality::Rejected {
            r.quality = Quality::NeedsReview;
        }
        r.role = Role::Background;
        r.reasons.push(Reason::ImageRegionText);
    } else if b.label == "table" || b.text.contains("<table") {
        if r.quality != Quality::Rejected {
            r.quality = Quality::NeedsReview;
        }
        r.role = Role::Context;
        r.reasons.push(Reason::StructuredLayoutUnverified);
    } else if !matches!(b.label.as_str(), "text" | "doc_title" | "paragraph_title") {
        r.role = Role::LayoutMetadata;
        r.reasons.push(Reason::NonNarrativeLabel);
    }
}

fn usable_fragments(
    input: &ImageInput,
    b: &Block,
    r: &RegionAssessment,
    lines: &[&Line],
) -> Vec<Fragment> {
    if !input.layout.completed
        || b.text.trim().is_empty()
        || !matches!(
            r.role,
            Role::TextCandidate | Role::ImageHeading | Role::Context
        )
    {
        return vec![];
    }
    if r.quality == Quality::Supported {
        return vec![fragment(SourceKind::Block, &b.block_ref, &b.text, b.bbox)];
    }
    // Explicit, separately identified line fallback. No trimming, correcting or pretending
    // a shortened VL result is its raw output. Missing line support stays missing.
    lines
        .iter()
        .filter(|l| l.confidence >= 0.75 && !repeated(&l.text))
        .map(|l| fragment(SourceKind::Line, &l.line_ref, &l.text, l.bbox))
        .collect()
}
