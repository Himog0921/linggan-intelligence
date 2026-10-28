//! The Xiaohongshu copy format (spec A.4) and the paragraph order shared with the checks.
//! Paragraphs are separated by a line holding only U+3164, followed by one line of topics; the
//! title is copied on its own.
use super::subject::Subject;
use uuid::Uuid;

/// HANGUL FILLER: the only character on the line between two paragraphs.
pub const SPACER: char = '\u{3164}';

pub enum Block<'a> {
    Paragraph(Uuid, &'a str),
    Bridge(&'a str),
}
impl Block<'_> {
    pub fn text(&self) -> &str {
        match self {
            Self::Paragraph(_, text) | Self::Bridge(text) => text,
        }
    }
}

/// The note body in reading order. Empty paragraphs are dropped. The bridge follows its anchor
/// paragraph; with no usable anchor it goes before the last paragraph (or at the end when there
/// is only one).
pub fn blocks<'a>(
    paragraphs: &'a [(Uuid, String)],
    bridge: Option<(&'a str, Option<Uuid>)>,
) -> Vec<Block<'a>> {
    let mut out: Vec<Block<'a>> = paragraphs
        .iter()
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(id, text)| Block::Paragraph(*id, text.as_str()))
        .collect();
    if let Some((text, anchor)) = bridge {
        let after_anchor = anchor.and_then(|id| {
            out.iter()
                .position(|block| matches!(block, Block::Paragraph(p, _) if *p == id))
                .map(|index| index + 1)
        });
        let at = after_anchor.unwrap_or(if out.len() >= 2 {
            out.len() - 1
        } else {
            out.len()
        });
        out.insert(at, Block::Bridge(text));
    }
    out
}

pub fn tag_key(tag: &str) -> String {
    tag.trim()
        .trim_start_matches(['#', '＃'])
        .trim()
        .to_ascii_lowercase()
}

/// Topics as `#话题`, in order, with repeats removed.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for tag in tags {
        let key = tag_key(tag);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        out.push(format!(
            "#{}",
            tag.trim().trim_start_matches(['#', '＃']).trim()
        ));
    }
    out
}

pub struct CopyText {
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    /// Body followed by the topic line, what goes to the clipboard.
    pub text: String,
}

/// `None` when the note has no body to copy.
pub fn format_copy(subject: &Subject, bridge: Option<(&str, Option<Uuid>)>) -> Option<CopyText> {
    let parts: Vec<String> = blocks(&subject.paragraphs, bridge)
        .iter()
        .map(|block| block.text().replace("\r\n", "\n").trim_end().to_string())
        .collect();
    if parts.is_empty() {
        return None;
    }
    let body = parts.join(&format!("\n{SPACER}\n"));
    let tags = normalize_tags(&subject.tags);
    let text = if tags.is_empty() {
        body.clone()
    } else {
        format!("{body}\n{}", tags.join(" "))
    };
    Some(CopyText {
        title: subject.title.trim().to_string(),
        body,
        tags,
        text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(paragraphs: &[&str], tags: &[&str]) -> Subject {
        Subject {
            has_account: true,
            title: "  标题  ".into(),
            cover_title: String::new(),
            paragraphs: paragraphs
                .iter()
                .map(|text| (Uuid::new_v4(), (*text).to_string()))
                .collect(),
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            bridge: None,
            cover_ai: None,
        }
    }

    #[test]
    fn paragraphs_are_separated_by_a_line_with_only_the_filler() {
        let s = subject(
            &["第一段。", "第二段。", "第三段。"],
            &["ADHD", "#A娃成长", "放手"],
        );
        let out = format_copy(&s, None).unwrap();
        assert_eq!(
            out.text,
            "第一段。\n\u{3164}\n第二段。\n\u{3164}\n第三段。\n#ADHD #A娃成长 #放手"
        );
        assert_eq!(out.title, "标题");
        assert!(out.body.lines().any(|line| line == "\u{3164}"));
        assert_eq!(out.body.matches('\u{3164}').count(), 2);
    }

    #[test]
    fn repeated_and_empty_topics_are_dropped_and_none_means_no_topic_line() {
        let s = subject(&["正文"], &["ADHD", "#adhd", "＃ADHD ", "  ", "#"]);
        assert_eq!(format_copy(&s, None).unwrap().tags, vec!["#ADHD"]);
        let none = subject(&["正文"], &[]);
        assert_eq!(format_copy(&none, None).unwrap().text, "正文");
    }

    #[test]
    fn empty_paragraphs_and_trailing_spaces_do_not_leak_into_the_copy() {
        let s = subject(&["甲  ", "", "   ", "乙\r\n丙"], &[]);
        assert_eq!(format_copy(&s, None).unwrap().body, "甲\n\u{3164}\n乙\n丙");
        assert!(format_copy(&subject(&["", " "], &["x"]), None).is_none());
    }

    #[test]
    fn the_bridge_defaults_to_before_the_last_paragraph_or_follows_its_anchor() {
        let s = subject(&["开头", "观点", "收尾"], &[]);
        let default = format_copy(&s, Some(("衔接", None))).unwrap().body;
        assert_eq!(
            default,
            "开头\n\u{3164}\n观点\n\u{3164}\n衔接\n\u{3164}\n收尾"
        );
        let anchor = s.paragraphs[0].0;
        let anchored = format_copy(&s, Some(("衔接", Some(anchor)))).unwrap().body;
        assert_eq!(
            anchored,
            "开头\n\u{3164}\n衔接\n\u{3164}\n观点\n\u{3164}\n收尾"
        );
        let dangling = format_copy(&s, Some(("衔接", Some(Uuid::new_v4()))))
            .unwrap()
            .body;
        assert_eq!(dangling, default);
        let single = subject(&["只有一段"], &[]);
        assert_eq!(
            format_copy(&single, Some(("衔接", None))).unwrap().body,
            "只有一段\n\u{3164}\n衔接"
        );
    }
}
