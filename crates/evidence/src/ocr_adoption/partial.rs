//! Conservative recovery of exact primary-source ranges, never rewritten consensus text.
use super::quality::{normalized, repeated, supported_lines};
use super::*;
use std::collections::HashMap;
use unicode_normalization::UnicodeNormalization;

pub(super) fn covers(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[0] && a[1] <= b[1] && a[2] >= b[2] && a[3] >= b[3]
}

pub(super) fn recover(input: &ImageInput, b: &Block, r: &mut RegionAssessment) {
    if r.adoption_state != AdoptionState::Pending
        || r.quality == Quality::Rejected
        || !matches!(r.role, Role::TextCandidate | Role::ImageHeading)
        || b.text.chars().count() > 8000
    {
        return;
    }
    let mut ranges = vec![];
    let screenshot = super::ui::is_screenshot(input);
    let clean = |text: &str| {
        text.lines()
            .filter(|s| !(screenshot && s.trim().chars().all(|c| c.is_ascii_digit())))
            .collect::<Vec<_>>()
            .join("\n")
    };
    for vl in input.rechecks.iter().filter(|c| {
        c.block_ref == b.block_ref
            && c.kind == RecheckKind::VlCrop
            && c.finish_reason.as_deref() == Some("stop")
            && !repeated(&c.text)
            && !(b.label == "content" && covers(c.crop_bbox, b.bbox))
    }) {
        let mut support = vec![];
        for line in input.rechecks.iter().filter(|c| {
            c.block_ref == b.block_ref
                && c.kind == RecheckKind::LineCrop
                && c.crop_bbox == vl.crop_bbox
                && !repeated(&c.text)
        }) {
            support.push(clean(&line.text));
        }
        // A line observation must be inside the VL crop. An unrelated phrase in
        // another part of a large block is not support for this local reading.
        let old = supported_lines(input, b)
            .into_iter()
            .filter(|l| l.confidence >= 0.75 && covers(vl.crop_bbox, l.bbox))
            .filter(|l| !(screenshot && l.text.trim().chars().all(|c| c.is_ascii_digit())))
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        support.push(old);
        for text in support {
            for (start, end) in literal_ranges(&b.text, &text) {
                let candidate: String = b.text.chars().skip(start).take(end - start).collect();
                // A disagreement in either reading removes that local clause,
                // not every independently supported clause in the same block.
                for (local_start, local_end) in literal_ranges(&candidate, &vl.text) {
                    ranges.push((start + local_start, start + local_end));
                }
            }
        }
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = vec![];
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    r.usable_fragments = merged
        .into_iter()
        .map(|(start, end)| Fragment {
            source_kind: SourceKind::Block,
            source_ref: b.block_ref.clone(),
            start,
            end,
            bbox: b.bbox,
            text: b.text.chars().skip(start).take(end - start).collect(),
        })
        .collect();
    if !r.usable_fragments.is_empty() {
        r.adoption_state = AdoptionState::AdoptedPartialBlock;
        r.quality = Quality::NeedsReview;
        r.reasons.push(Reason::PartialLiteralAgreement);
    }
}

// Preserve a mapping to Unicode scalar offsets. Normalization is only used to
// locate agreement; the adopted text is always sliced from the saved primary.
fn indexed(text: &str) -> Vec<(char, usize)> {
    text.chars()
        .enumerate()
        .flat_map(|(i, c)| {
            c.to_string()
                .nfkc()
                .flat_map(char::to_lowercase)
                .filter(|c| c.is_alphanumeric())
                .map(|c| (c, i))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn seeds(chars: &[(char, usize)]) -> HashMap<[char; 4], Option<usize>> {
    let mut result = HashMap::new();
    for (i, w) in chars.windows(4).enumerate() {
        let key = [w[0].0, w[1].0, w[2].0, w[3].0];
        result
            .entry(key)
            .and_modify(|v| *v = None)
            .or_insert(Some(i));
    }
    result
}

fn boundary(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '，' | '。' | '、' | '；' | '！' | '？' | ',' | ';' | '!' | '?' | ':' | '：'
        )
}

fn literal_ranges(source: &str, evidence: &str) -> Vec<(usize, usize)> {
    if evidence.chars().count() > 8000 {
        return vec![];
    }
    let a = indexed(source);
    let b = indexed(evidence);
    let a_seeds = seeds(&a);
    let b_seeds = seeds(&b);
    let raw: Vec<_> = source.chars().collect();
    let mut ranges = vec![];
    let mut covered = 0;
    for (i, w) in a.windows(4).enumerate() {
        if i < covered {
            continue;
        }
        let key = [w[0].0, w[1].0, w[2].0, w[3].0];
        let Some(Some(mut j)) = b_seeds.get(&key).copied() else {
            continue;
        };
        if a_seeds.get(&key) != Some(&Some(i)) {
            continue;
        }
        let mut left = i;
        while left > 0 && j > 0 && a[left - 1].0 == b[j - 1].0 {
            left -= 1;
            j -= 1;
        }
        let mut right = i + 4;
        let mut k = j + (i - left) + 4;
        while right < a.len() && k < b.len() && a[right].0 == b[k].0 {
            right += 1;
            k += 1;
        }
        covered = right;
        let mut start = a[left].1;
        let mut end = a[right - 1].1 + 1;
        // Do not publish a dangling half-word beside a disputed character. Keep
        // only complete local clauses; leave the disputed clause in the raw block.
        if left > 0 && start > 0 && !boundary(raw[start - 1]) && raw[start - 1] != '{' {
            start = (start..end)
                .find(|&p| boundary(raw[p]))
                .map_or(end, |p| p + 1);
        }
        if right < a.len() && end < raw.len() && !boundary(raw[end]) && raw[end] != '}' {
            end = (start..end)
                .rev()
                .find(|&p| boundary(raw[p]))
                .map_or(start, |p| p + 1);
        } else {
            while end < raw.len() && boundary(raw[end]) {
                end += 1;
            }
        }
        while start < end && raw[start].is_whitespace() {
            start += 1;
        }
        while end > start && raw[end - 1].is_whitespace() {
            end -= 1;
        }
        let text: String = raw[start..end].iter().collect();
        if normalized(&text).chars().count() >= 4
            && !text
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, ':' | '.' | '-' | ' '))
        {
            ranges.push((start, end));
        }
    }
    ranges
}
