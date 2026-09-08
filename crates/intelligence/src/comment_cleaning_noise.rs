//! Deterministic, bounded text noise handling. No quality or spam inference.
use super::{CleanComment, MentionSpan};
use unicode_properties::UnicodeEmoji;

pub(super) fn has_meaning(c: char) -> bool {
    c.is_alphanumeric() || "∑∫√∞≠≈≤≥±×÷".contains(c)
}

// Unicode 17.0 Emoji / Emoji_Component properties, pinned by unicode-properties 0.1.4.
// ASCII numbers and operators are interpreted as emoji only in complete keycap sequences.
fn pictograph(c: char) -> bool {
    !c.is_ascii() && (c.is_emoji_char_or_emoji_component() || c == '\u{fe0e}')
}

fn flag(out: &mut CleanComment, reason: &str) {
    if !out.reasons.iter().any(|r| r == reason) {
        out.reasons.push(reason.into());
    }
}

pub(super) fn remove(out: &mut CleanComment, raw: &str, mentions: &[MentionSpan]) {
    let raw_chars: Vec<_> = raw.chars().collect();
    let chars: Vec<_> = out.text.chars().collect();
    let mut removed = vec![false; chars.len()];
    for span in mentions {
        // Only explicit platform spans beginning at @ are trusted. Never accept
        // a boundary cutting through a transformed scalar or crossing a newline.
        if span.start >= span.end
            || span.end > raw_chars.len()
            || !matches!(raw_chars[span.start], '@' | '＠')
            || raw_chars[span.start..span.end]
                .iter()
                .any(|c| matches!(c, '\n' | '\r'))
        {
            flag(out, "mention_boundary_invalid");
            continue;
        }
        for (i, (start, end)) in out.offsets.iter().enumerate() {
            if *start >= span.start && *end <= span.end {
                removed[i] = true;
            }
        }
        flag(out, "mention_removed");
    }
    for i in 0..chars.len() {
        if pictograph(chars[i]) {
            removed[i] = true;
            flag(out, "emoji_removed");
        }
        // A digit/#/* is emoji only when followed by a complete keycap sequence.
        if chars[i].is_ascii_digit() || matches!(chars[i], '#' | '*') {
            let end = i + 1 + usize::from(chars.get(i + 1) == Some(&'\u{fe0f}'));
            if chars.get(end) == Some(&'\u{20e3}') {
                removed[i..=end].fill(true);
                flag(out, "emoji_removed");
            }
        }
        if chars[i] != '@' || removed[i] || i > 0 && !chars[i - 1].is_whitespace() {
            continue;
        }
        // Plain text fallback recognizes one whitespace-delimited @token.
        // Embedded email addresses and punctuation-attached prose are preserved.
        let mut end = i + 1;
        while end < chars.len() && (chars[end].is_alphanumeric() || matches!(chars[end], '_' | '-'))
        {
            end += 1;
        }
        if end == i + 1 || end - i > 33 {
            flag(out, "mention_boundary_uncertain");
            continue;
        }
        if end == chars.len() || chars[end].is_whitespace() || pictograph(chars[end]) {
            removed[i..end].fill(true);
            flag(out, "mention_removed");
        } else {
            flag(out, "mention_boundary_uncertain");
        }
    }
    let offsets = out.offsets.clone();
    out.text.clear();
    out.offsets.clear();
    for (i, c) in chars.iter().copied().enumerate() {
        if removed[i] || c.is_whitespace() && out.text.is_empty() {
            continue;
        }
        if c.is_whitespace() && out.text.ends_with(' ') {
            if let Some(last) = out.offsets.last_mut() {
                last.1 = offsets[i].1;
            }
            continue;
        }
        out.text.push(c);
        out.offsets.push(offsets[i]);
    }
    if out.text.ends_with(' ') {
        out.text.pop();
        out.offsets.pop();
    }
}
