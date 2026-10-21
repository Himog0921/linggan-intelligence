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
    icons(input).count() >= 3
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

pub(super) fn filter_text(input: &ImageInput, r: &mut RegionAssessment, screenshot: bool) {
    if !screenshot || r.usable_fragments.is_empty() {
        return;
    }
    let mut filtered = vec![];
    let mut changed = false;
    let mut unsupported_symbol = false;
    for f in &r.usable_fragments {
        let start = reply_prefix(input, &f.text).unwrap_or(0);
        let end = timestamp_suffix(&f.text).unwrap_or(f.text.len());
        let mut spans = vec![];
        let mut from = start.min(end);
        for (offset, c) in f
            .text
            .char_indices()
            .filter(|(i, _)| *i >= start && *i < end)
        {
            if emoji(c)
                && !input
                    .anchors
                    .lines
                    .iter()
                    .any(|l| r.support_line_refs.contains(&l.line_ref) && l.text.contains(c))
            {
                spans.push((from, offset));
                from = offset + c.len_utf8();
                unsupported_symbol = true;
            }
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
