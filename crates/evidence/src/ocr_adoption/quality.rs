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
    let lines: Vec<_> = input
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
    visual_order(lines)
}

pub(super) fn visual_order(mut lines: Vec<&Line>) -> Vec<&Line> {
    // Preserve raw ordinals in the input. For support inside one VL block, derive visual
    // rows so a differently coloured word cannot jump ahead of its left-hand prefix.
    lines.sort_by(|a, b| a.bbox[1].total_cmp(&b.bbox[1]));
    let mut rows: Vec<Vec<&Line>> = vec![];
    for l in lines {
        let same = rows.last().is_some_and(|row| {
            let a = row[0].bbox;
            let b = l.bbox;
            a[3].min(b[3]) - a[1].max(b[1]) >= (a[3] - a[1]).min(b[3] - b[1]) * 0.5
        });
        if same {
            if let Some(row) = rows.last_mut() {
                row.push(l);
            }
        } else {
            rows.push(vec![l]);
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| a.bbox[0].total_cmp(&b.bbox[0]));
    }
    let lines = rows.into_iter().flatten().collect();
    lines
}

pub(super) fn assess_regions(input: &ImageInput) -> Vec<RegionAssessment> {
    let screenshot = super::ui::is_screenshot(input);
    input
        .layout
        .blocks
        .iter()
        .map(|b| assess_region(input, b, screenshot))
        .collect()
}

fn assess_region(input: &ImageInput, b: &Block, screenshot: bool) -> RegionAssessment {
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
        adoption_state: AdoptionState::Excluded,
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
        fallback_candidates: vec![],
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
    super::ui::classify(input, b, &mut r, screenshot);
    qualify_fragments(input, b, &lines, &mut r);
    super::ui::filter_text(input, &mut r, screenshot);

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
        None => {
            r.reasons.push(Reason::FinishReasonMissing);
            if input.layout.require_termination && !b.text.trim().is_empty() {
                r.quality = Quality::Rejected;
            }
        }
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
    let raw_repeated = input
        .layout
        .terminations
        .iter()
        .find(|t| t.block_ref == b.block_ref)
        .and_then(|t| t.raw_text.as_deref())
        .is_some_and(repeated);
    if repeated(&b.text) || raw_repeated {
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
    } else if mathematical_expression(&b.text) {
        if r.quality != Quality::Rejected {
            r.quality = Quality::NeedsReview;
        }
        r.role = Role::StructuredText;
        r.reasons.push(Reason::MathematicalLayoutUnverified);
    } else if b.label == "table" || b.text.contains("<table") {
        if r.quality != Quality::Rejected {
            r.quality = Quality::NeedsReview;
        }
        r.role = Role::StructuredText;
        r.reasons.push(Reason::StructuredLayoutUnverified);
    } else if !matches!(b.label.as_str(), "text" | "doc_title" | "paragraph_title") {
        r.role = Role::LayoutMetadata;
        r.reasons.push(Reason::NonNarrativeLabel);
    }
}

fn qualify_fragments(input: &ImageInput, b: &Block, lines: &[&Line], r: &mut RegionAssessment) {
    if !input.layout.completed
        || !matches!(
            r.role,
            Role::TextCandidate | Role::ImageHeading | Role::StructuredText
        )
    {
        return;
    }
    if r.quality == Quality::Supported && !b.text.trim().is_empty() {
        r.usable_fragments = vec![fragment(SourceKind::Block, &b.block_ref, &b.text, b.bbox)];
        r.adoption_state = AdoptionState::AdoptedBlock;
        return;
    }
    r.fallback_candidates = lines
        .iter()
        .filter(|l| l.confidence >= 0.75 && !repeated(&l.text))
        .map(|l| fragment(SourceKind::Line, &l.line_ref, &l.text, l.bbox))
        .collect();
    r.adoption_state = AdoptionState::Pending;
    if !r.fallback_candidates.is_empty() {
        r.reasons.push(Reason::FallbackNotConfirmed);
    }
    // Agreement on flattened characters cannot prove superscripts or equation structure.
    // Keep both observations without treating a matching crop as a mathematical proof.
    if r.reasons.contains(&Reason::MathematicalLayoutUnverified) {
        return;
    }
    let anchors = normalized(
        &lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    let original = normalized(&b.text);
    for check in input.rechecks.iter().filter(|c| c.block_ref == b.block_ref) {
        let reread = normalized(&check.text);
        let ended =
            check.kind == RecheckKind::LineCrop || check.finish_reason.as_deref() == Some("stop");
        if !ended || reread.is_empty() || repeated(&check.text) {
            continue;
        }
        if r.quality != Quality::Rejected
            && r.role != Role::StructuredText
            && reread == original
            && (check.kind == RecheckKind::LineCrop || supported_extension(&anchors, &original))
        {
            r.usable_fragments = vec![fragment(SourceKind::Block, &b.block_ref, &b.text, b.bbox)];
            r.adoption_state = AdoptionState::AdoptedBlock;
            r.reasons.push(if check.kind == RecheckKind::LineCrop {
                Reason::CropLineAgreement
            } else {
                Reason::CropVlAgreement
            });
            return;
        }
        let fallback = normalized(
            &r.fallback_candidates
                .iter()
                .map(|f| f.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        );
        if !fallback.is_empty() && reread == fallback {
            // A repeated line-engine reading cannot resolve a conflicting, completed
            // VL crop. Keep the candidate instead of promoting an icon-shaped letter.
            if input.rechecks.iter().any(|other| {
                other.block_ref == b.block_ref
                    && other.kind == RecheckKind::VlCrop
                    && other.finish_reason.as_deref() == Some("stop")
                    && !normalized(&other.text).is_empty()
                    && normalized(&other.text) != fallback
            }) {
                r.reasons.push(Reason::ConflictingLocalReadings);
                continue;
            }
            r.usable_fragments = r.fallback_candidates.clone();
            r.adoption_state = AdoptionState::AdoptedLineFallback;
            r.reasons.push(Reason::CropSupportsFallback);
            return;
        }
    }
}

// VL may add visible letters missed by the old recognizer. It may not silently remove or
// rewrite supported letters. Same-model re-reading alone has a deliberately limited ceiling.
fn supported_extension(anchors: &str, vl: &str) -> bool {
    let a: Vec<_> = anchors.chars().collect();
    let v: Vec<_> = vl.chars().collect();
    if a.is_empty() || (a.len() as f64) / (v.len().max(1) as f64) < 0.80 {
        return false;
    }
    let mut i = 0;
    for c in v {
        if a.get(i) == Some(&c) {
            i += 1;
        }
    }
    i == a.len()
}

fn mathematical_expression(text: &str) -> bool {
    let text: String = text.nfkc().filter(|c| !c.is_whitespace()).collect();
    text.chars().any(|c| c.is_ascii_digit())
        && text
            .chars()
            .any(|c| matches!(c, '=' | '≈' | '≠' | '≤' | '≥'))
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '=' | '≈'
                        | '≠'
                        | '≤'
                        | '≥'
                        | '+'
                        | '-'
                        | '−'
                        | '*'
                        | '/'
                        | '×'
                        | '÷'
                        | '^'
                        | '_'
                        | '.'
                        | ','
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '\\'
                        | '$'
                        | '%'
                )
        })
}
