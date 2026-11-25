//! Verbatim material boundaries for draft output and adoption.
use super::{AiError, sources::ValidatedCitation};
use serde_json::Value;

pub(super) fn collect(restricted: &mut Vec<String>, source: &ValidatedCitation) {
    restricted.push(source.selected_text.clone());
    let content = &source.content;
    match content["kind"].as_str() {
        Some("list") => collect_strings(restricted, &content["items"]),
        Some("table") => {
            collect_strings(restricted, &content["columns"]);
            for row in content["rows"].as_array().into_iter().flatten() {
                collect_strings(restricted, row);
            }
        }
        Some("dialogue") => restricted.extend(
            content["turns"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|turn| turn["text"].as_str().map(str::to_owned)),
        ),
        _ => restricted.extend(content["text"].as_str().map(str::to_owned)),
    }
}

fn collect_strings(target: &mut Vec<String>, value: &Value) {
    target.extend(
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(str::to_owned)),
    );
}

fn sentences(text: &str) -> impl Iterator<Item = &str> {
    text.split(['。', '！', '？', '!', '?', '\n'])
        .flat_map(|line| line.split(". "))
        .map(|sentence| {
            sentence
                .trim()
                .trim_matches(['.', '"', '“', '”', '「', '」'])
                .trim()
        })
}

fn quoted_sentences(text: &str) -> impl Iterator<Item = &str> {
    [('「', '」'), ('『', '』'), ('“', '”'), ('"', '"')]
        .into_iter()
        .flat_map(move |(open, close)| {
            text.match_indices([open, close])
                .scan(None, move |start, (index, delimiter)| {
                    let ch = delimiter.chars().next()?;
                    let quoted = if ch == close && start.is_some() {
                        start.take().map(|from| &text[from..index])
                    } else {
                        if ch == open {
                            *start = Some(index + delimiter.len());
                        }
                        None
                    };
                    Some(quoted)
                })
                .flatten()
        })
        .flat_map(sentences)
}

fn contains_verbatim(text: &str, phrase: &str) -> bool {
    let starts_with_ascii_word = phrase.starts_with(|ch: char| ch.is_ascii_alphanumeric());
    let ends_with_ascii_word = phrase.ends_with(|ch: char| ch.is_ascii_alphanumeric());
    text.match_indices(phrase).any(|(start, matched)| {
        let end = start + matched.len();
        let attached_left = starts_with_ascii_word
            && text[..start].ends_with(|ch: char| ch.is_ascii_alphanumeric());
        let attached_right =
            ends_with_ascii_word && text[end..].starts_with(|ch: char| ch.is_ascii_alphanumeric());
        !attached_left && !attached_right
    })
}

pub(super) fn validate_material_quotes<'a>(
    restricted: &[String],
    output: impl IntoIterator<Item = &'a str>,
) -> Result<(), AiError> {
    let (short, long): (std::collections::HashSet<_>, std::collections::HashSet<_>) = restricted
        .iter()
        .flat_map(|raw| std::iter::once(raw.trim()).chain(sentences(raw)))
        .filter(|phrase| phrase.chars().any(char::is_alphanumeric))
        .partition(|phrase| phrase.chars().count() < 4);
    for text in output {
        if (!short.is_empty()
            && sentences(text)
                .chain(quoted_sentences(text))
                .any(|sentence| short.contains(sentence)))
            || long.iter().any(|phrase| contains_verbatim(text, phrase))
        {
            return Err(AiError::MaterialQuoteForbidden);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn structured_source_lines_are_checked_without_prefixes() {
        for content in [
            json!({"kind":"list","items":["摘录项", "另一项禁止原句"]}),
            json!({"kind":"dialogue","turns":[{"speaker":"other","text":"另一项禁止原句"}]}),
            json!({"kind":"table","columns":["标题"],"rows":[["另一项禁止原句"]]}),
        ] {
            let source = ValidatedCitation {
                selected_text: "摘录项".into(),
                text_hash: String::new(),
                speaker: String::new(),
                content,
                issues: vec![],
            };
            let mut restricted = Vec::new();
            collect(&mut restricted, &source);
            assert!(matches!(
                validate_material_quotes(&restricted, ["候选直接写了另一项禁止原句。"]),
                Err(AiError::MaterialQuoteForbidden)
            ));
        }
    }

    #[test]
    fn short_explicit_quotes_and_ascii_word_boundaries_are_respected() {
        let restricted = ["快走！".to_owned(), "time".to_owned()];
        assert!(validate_material_quotes(&restricted, ["我觉得要快走。", "sometimes"]).is_ok());
        for text in ["他说：「快走！」", "The time is here."] {
            assert!(matches!(
                validate_material_quotes(&restricted, [text]),
                Err(AiError::MaterialQuoteForbidden)
            ));
        }
    }

    #[test]
    fn short_fragments_match_only_as_complete_sentences() {
        let restricted = ["我".to_owned(), "温度是 2.5 度。全句不能照搬。".to_owned()];
        assert!(validate_material_quotes(&restricted, ["我先做自己的事情。数字是 5。"]).is_ok());
        assert!(matches!(
            validate_material_quotes(&restricted, ["第一句。」「我」。"]),
            Err(AiError::MaterialQuoteForbidden)
        ));
        assert!(matches!(
            validate_material_quotes(&restricted, ["另一种写法：全句不能照搬。"]),
            Err(AiError::MaterialQuoteForbidden)
        ));
    }
}
