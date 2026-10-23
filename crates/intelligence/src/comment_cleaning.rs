//! Versioned research text; source facts remain immutable. Every retained Unicode scalar
//! maps to its original span, including after emoji and reliable mention removal.
use serde::{Deserialize, Serialize};
pub const CLEANER_VERSION: &str = "comment-clean.v2";

#[path = "comment_cleaning_noise.rs"]
mod noise;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CleanComment {
    pub text: String,
    pub offsets: Vec<(usize, usize)>,
    pub state: String,
    pub reasons: Vec<String>,
}
impl CleanComment {
    /// Maps one unambiguous retained quote back to the immutable source text.  This remains a
    /// generic cleaner guarantee; V1's Atom admission uses the same offset contract.
    pub fn resolve(&self, quote: &str, raw: &str) -> Result<(i32, i32, String), &'static str> {
        if quote.is_empty() || quote.contains('█') {
            return Err("quote_redacted_or_empty");
        }
        let text: Vec<_> = self.text.chars().collect();
        let needle: Vec<_> = quote.chars().collect();
        if needle.len() > text.len() {
            return Err("quote_missing_or_ambiguous");
        }
        let matches: Vec<_> = text
            .windows(needle.len())
            .enumerate()
            .filter(|(_, window)| *window == needle.as_slice())
            .map(|(index, _)| index)
            .collect();
        if matches.len() != 1 {
            return Err("quote_missing_or_ambiguous");
        }
        let start = matches[0];
        let end = start + needle.len();
        let source_start = self.offsets.get(start).ok_or("quote_out_of_bounds")?.0;
        let source_end = self.offsets.get(end - 1).ok_or("quote_out_of_bounds")?.1;
        let original = raw
            .chars()
            .skip(source_start)
            .take(source_end - source_start)
            .collect();
        Ok((source_start as i32, source_end as i32, original))
    }
}
/// A platform-supplied mention boundary, expressed in original Unicode scalar offsets.
/// Invalid spans are ignored rather than guessing at a username boundary.
#[derive(Debug, Clone, Copy)]
pub struct MentionSpan {
    pub start: usize,
    pub end: usize,
}
pub fn clean(raw: &str) -> CleanComment {
    clean_with_mentions(raw, &[])
}
pub fn clean_with_mentions(raw: &str, mentions: &[MentionSpan]) -> CleanComment {
    let chars: Vec<_> = raw.chars().collect();
    let mut out = CleanComment {
        text: String::new(),
        offsets: vec![],
        state: "direct".into(),
        reasons: vec![],
    };
    if chars.len() > 16000 {
        out.state = "anomaly".into();
        out.reasons.push("source_too_long".into());
        return out;
    }
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let mut c = chars[i];
        i += 1;
        if c.is_control() && !c.is_whitespace()
            || matches!(c, '\u{200b}' | '\u{feff}' | '\u{200c}' | '\u{200d}')
        {
            continue;
        }
        if c == '&' {
            let rest: String = chars[start..].iter().take(12).collect();
            if let Some(end) = rest.find(';') {
                let entity = &rest[1..end];
                let decoded = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" | "#39" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                };
                if let Some(d) = decoded {
                    c = d;
                    i = start + end + 1;
                }
            }
        }
        if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
            c = char::from_u32(c as u32 - 0xfee0).unwrap_or(c);
        }
        if c.is_whitespace() {
            if out.text.is_empty() {
                continue;
            }
            if out.text.ends_with(' ') {
                if let Some(last) = out.offsets.last_mut() {
                    last.1 = i;
                }
                continue;
            }
            c = ' ';
        }
        out.text.push(c);
        out.offsets.push((start, i));
    }
    if out.text.ends_with(' ') {
        out.text.pop();
        out.offsets.pop();
    }
    if out.text != raw {
        out.reasons.push("whitespace_or_encoding".into());
    }
    noise::remove(&mut out, raw, mentions);
    classify_cleaned(raw, &mut out);
    out
}
fn classify_cleaned(raw: &str, out: &mut CleanComment) {
    if raw.chars().any(|c| c == '�') && !out.text.chars().any(noise::has_meaning) {
        out.state = "anomaly".into();
        out.reasons.push("damaged_encoding".into());
    } else if !out.text.chars().any(noise::has_meaning) {
        out.state = "dropped".into();
        out.reasons.push(
            if raw.trim().is_empty() {
                "empty_text"
            } else if out.text.is_empty() {
                match (
                    out.reasons.iter().any(|r| r == "mention_removed"),
                    out.reasons.iter().any(|r| r == "emoji_removed"),
                ) {
                    (true, true) => "mention_emoji_only",
                    (true, false) => "mention_only",
                    (false, true) => "emoji_only",
                    (false, false) => "invisible_only",
                }
            } else {
                "punctuation_only"
            }
            .into(),
        );
        out.text.clear();
        out.offsets.clear();
    } else if [
        "我也是",
        "我们也是",
        "我家也是",
        "我们家也是",
        "真的吗",
        "这个有用吗",
        "有效果吗",
        "不行,我试过了",
        "怎么呼吸",
        "同问",
        "同感",
        "是的",
        "真的",
        "对",
        "嗯",
        "求",
        "怎么做",
    ]
    .iter()
    .any(|t| {
        out.text
            .trim_matches(|c: char| c.is_ascii_punctuation() || "！？。～".contains(c))
            == *t
    }) || out.text.chars().count() < 4
    {
        out.state = "context".into();
        out.reasons.push("context_dependent".into());
    }
}
#[cfg(test)]
fn clean_optional(raw: Option<&str>) -> CleanComment {
    raw.map(clean).unwrap_or_else(|| CleanComment {
        text: String::new(),
        offsets: Vec::new(),
        state: "anomaly".into(),
        reasons: vec!["missing_body".into()],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_mapping_preserves_negation_and_family_emoji() {
        let raw = "  我👨‍👩‍👧不想\n  催促 &amp; 监督  ";
        let c = clean(raw);
        assert!(c.text.contains("不想 催促 & 监督"));
        assert_eq!(c.offsets.len(), c.text.chars().count());
    }
    #[test]
    fn deterministic_noise_is_dropped_and_short_replies_need_context() {
        assert_eq!(clean("😭😭😭").state, "dropped");
        assert_eq!(clean("我也是！！").state, "context");
        assert_eq!(clean("没有效果，一催就吵").state, "direct");
    }
}

#[cfg(test)]
#[path = "comment_cleaning_v2_tests.rs"]
mod v2_tests;
