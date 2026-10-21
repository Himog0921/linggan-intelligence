//! Screenshot context and exact-source omission of interface text. No raw text is edited.
use super::quality::normalized;
use super::*;

fn icons(input: &ImageInput) -> impl Iterator<Item = &Block> {
    input.layout.blocks.iter().filter(|b| {
        matches!(b.label.as_str(), "image" | "header_image")
            && (b.bbox[2] - b.bbox[0]) / f64::from(input.image.width) <= 0.18
            && (b.bbox[3] - b.bbox[1]) / f64::from(input.image.height) <= 0.16
    })
}

pub(super) fn is_screenshot(input: &ImageInput) -> bool {
    // Checkboxes and decorative icons alone do not establish a comment interface.
    let comments = input.layout.blocks.iter().any(|b| {
        b.label == "content"
            && b.text
                .lines()
                .filter(|s| timestamp_suffix(s).is_some())
                .count()
                >= 2
    });
    comments
        || icons(input).count() >= 3
            && input.layout.blocks.iter().any(|b| {
                control(&b.text)
                    || timestamp_suffix(&b.text).is_some()
                    || date_control(&b.text) && b.text.contains("浏览量")
            })
}

fn name_aligned(input: &ImageInput, b: &Block) -> bool {
    let w = f64::from(input.image.width);
    b.text.chars().count() <= 48
        && (b.bbox[3] - b.bbox[1]) / f64::from(input.image.height) <= 0.06
        && icons(input).any(|i| {
            i.bbox[2] <= b.bbox[0] + w * 0.02
                && b.bbox[0] - i.bbox[2] < w * 0.06
                && i.bbox[3].min(b.bbox[3]) > i.bbox[1].max(b.bbox[1])
                && b.bbox[1] < (i.bbox[1] + i.bbox[3]) * 0.5
                && b.bbox[3] <= i.bbox[3]
        })
}

fn control(text: &str) -> bool {
    let text = text.trim();
    let compact = normalized(text);
    let replies = compact
        .strip_prefix("展开")
        .and_then(|t| t.strip_suffix("条回复"))
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    text == "作者赞过" || replies || text.starts_with("译自") && text.contains("显示原文")
}

fn date_control(text: &str) -> bool {
    text.chars().count() <= 48
        && text.chars().filter(char::is_ascii_digit).count() >= 4
        && (text.contains(':') || text.contains('-') || text.contains('年'))
}

/// Byte offset of a short terminal time/location/reply control, not a prose mention.
fn timestamp_suffix(text: &str) -> Option<usize> {
    let trimmed = text.trim_end();
    if !trimmed.ends_with("回复") {
        return None;
    }
    let from = trimmed.char_indices().rev().nth(47).map_or(0, |(i, _)| i);
    trimmed[from..].char_indices().find_map(|(index, c)| {
        let offset = from + index;
        let suffix = &trimmed[offset..];
        if !c.is_ascii_digit() {
            return None;
        }
        let digits = suffix.bytes().take_while(u8::is_ascii_digit).count();
        let tail = &suffix[digits..];
        let relative = ["分钟前", "小时前", "天前", "周前", "月前", "年前"]
            .iter()
            .any(|unit| tail.starts_with(unit));
        let date = digits == 4 && (tail.starts_with('-') || tail.starts_with('年'));
        (relative || date).then_some(offset)
    })
}

pub(super) fn classify(input: &ImageInput, b: &Block, r: &mut RegionAssessment, screenshot: bool) {
    if !screenshot {
        return;
    }
    let compact = normalized(&b.text);
    if compact.starts_with(|c: char| c.is_ascii_digit())
        && compact.chars().count() <= 8
        && compact
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'k' | 'm' | 'w' | '万'))
    {
        r.role = Role::Uncertain;
        r.adoption_state = AdoptionState::Pending;
        r.reasons.push(Reason::ScreenshotUiContext);
    } else if control(&b.text)
        || date_control(&b.text)
        || timestamp_suffix(b.text.trim()) == Some(0)
        || name_aligned(input, b)
    {
        r.role = Role::LayoutMetadata;
        r.reasons.push(Reason::ScreenshotUiContext);
    }
}

fn reply_prefix(input: &ImageInput, text: &str) -> Option<usize> {
    let text = text.strip_prefix("回复")?;
    let colon = text.find([':', '：'])?;
    let name = text[..colon].trim();
    if input
        .layout
        .blocks
        .iter()
        .any(|b| name_aligned(input, b) && b.text.trim() == name)
    {
        Some("回复".len() + colon + text[colon..].chars().next()?.len_utf8())
    } else {
        None
    }
}

fn emoji(c: char) -> bool {
    matches!(c as u32, 0x1f000..=0x1faff | 0x2600..=0x27bf)
}

fn unsupported_ranges(text: &str, support: &str) -> Vec<(usize, usize)> {
    let mut ranges = vec![];
    for (open, close) in [("$", "$"), ("\\(", "\\)")] {
        let mut cursor = 0;
        while let Some(a) = text[cursor..].find(open).map(|n| cursor + n) {
            let from = a + open.len();
            let Some(b) = text[from..].find(close).map(|n| from + n + close.len()) else {
                break;
            };
            if !support.contains(&text[a..b]) {
                // LaTeX text wrappers are formatting, not image characters. Keep
                // an already-qualified literal prose interior; omit its wrapper.
                let interior = text[a..b].find("\\text{").and_then(|n| {
                    let s = a + n + "\\text{".len();
                    let e = s + text[s..b].find('}')?;
                    text[s..e]
                        .chars()
                        .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c))
                        .then_some((s, e))
                });
                if let Some((s, e)) = interior {
                    ranges.extend([(a, s), (e, b)]);
                } else {
                    ranges.push((a, b));
                }
            }
            cursor = b;
        }
    }
    let chars: Vec<_> = text.char_indices().collect();
    for (n, &(offset, c)) in chars.iter().enumerate() {
        let attachment = matches!(c as u32, 0xfe00..=0xfe0f | 0x200d | 0x20e3);
        let preceding_removed = n.checked_sub(1).is_some_and(|i| {
            ranges
                .iter()
                .any(|&(a, b)| chars[i].0 >= a && chars[i].0 < b)
        });
        if (emoji(c) && !support.contains(c))
            || attachment && (n == 0 || preceding_removed || !emoji(chars[n - 1].1))
        {
            ranges.push((offset, offset + c.len_utf8()));
        }
    }
    ranges
}

pub(super) fn filter_text(input: &ImageInput, r: &mut RegionAssessment, screenshot: bool) {
    if r.usable_fragments.is_empty() {
        return;
    }
    let mut filtered = vec![];
    let mut changed = false;
    let mut unsupported_symbol = false;
    let support = input
        .anchors
        .lines
        .iter()
        .filter(|l| r.support_line_refs.contains(&l.line_ref))
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for f in &r.usable_fragments {
        let start = if screenshot {
            reply_prefix(input, &f.text).unwrap_or(0)
        } else {
            0
        };
        let end = if screenshot {
            timestamp_suffix(&f.text).unwrap_or(f.text.len())
        } else {
            f.text.len()
        };
        let mut spans = vec![];
        let mut from = start.min(end);
        // Inspect the entire original source, then intersect with the fragment.
        // Otherwise a fragment cut through a formula could retain an orphan '$'.
        let original = match f.source_kind {
            SourceKind::Block => input
                .layout
                .blocks
                .iter()
                .find(|b| b.block_ref == f.source_ref)
                .map(|b| b.text.as_str()),
            SourceKind::Line => input
                .anchors
                .lines
                .iter()
                .find(|l| l.line_ref == f.source_ref)
                .map(|l| l.text.as_str()),
        }
        .unwrap_or(&f.text);
        let source_start = original
            .chars()
            .take(f.start)
            .map(char::len_utf8)
            .sum::<usize>();
        let source_end = source_start + f.text.len();
        let mut omitted: Vec<_> = unsupported_ranges(original, &support)
            .into_iter()
            .filter(|&(a, b)| a < source_end && b > source_start)
            .map(|(a, b)| {
                (
                    a.max(source_start) - source_start,
                    b.min(source_end) - source_start,
                )
            })
            .collect();
        omitted.sort_unstable();
        for (a, b) in omitted {
            if a >= end || b <= start {
                continue;
            }
            if a > from {
                spans.push((from, a));
            }
            from = from.max(b.min(end));
            unsupported_symbol = true;
        }
        spans.push((from, end));
        for (a, b) in spans {
            let text = f.text[a..b].trim();
            if text.is_empty() {
                continue;
            }
            let a = a + f.text[a..b].len() - f.text[a..b].trim_start().len();
            let offset = f.text[..a].chars().count();
            filtered.push(Fragment {
                source_kind: f.source_kind,
                source_ref: f.source_ref.clone(),
                start: f.start + offset,
                end: f.start + offset + text.chars().count(),
                bbox: f.bbox,
                text: text.into(),
            });
        }
        changed |= start != 0 || end != f.text.len() || unsupported_symbol;
    }
    if changed {
        r.usable_fragments = filtered;
        r.adoption_state = if r.usable_fragments.is_empty() {
            AdoptionState::Pending
        } else {
            AdoptionState::AdoptedPartialBlock
        };
        if r.quality != Quality::Rejected {
            r.quality = Quality::NeedsReview;
        }
        r.reasons.push(Reason::ScreenshotTextFiltered);
        if unsupported_symbol {
            r.reasons.push(Reason::UnverifiedSymbol);
        }
    }
}
