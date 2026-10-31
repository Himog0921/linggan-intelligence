//! Closed, source-qualified title suggestions. Suggestions are never automatically adopted.
use super::AiError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TitleSuggestion {
    pub cover_titles: Vec<String>,
    pub note_titles: Vec<String>,
}

pub(super) fn validate(text: &str) -> Result<TitleSuggestion, AiError> {
    let output: TitleSuggestion = serde_json::from_str(text).map_err(|_| AiError::Invalid)?;
    fn valid(items: &[String], limit: usize) -> bool {
        let mut seen = HashSet::new();
        items.len() <= 10
            && items.iter().all(|s| {
                !s.trim().is_empty()
                    && s == s.trim()
                    && s.chars().count() <= limit
                    && !s.chars().any(char::is_control)
                    && seen.insert(s)
            })
    }
    if output.cover_titles.is_empty()
        || output.note_titles.is_empty()
        || !valid(&output.cover_titles, 10)
        || !valid(&output.note_titles, 20)
    {
        return Err(AiError::Invalid);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_both_title_kinds_and_rejects_bad_shapes_or_repetitions() {
        assert!(validate(r#"{"coverTitles":["陪孩子迈出第一步"],"noteTitles":["孩子启动困难时先做这一件小事"]}"#).is_ok());
        assert!(
            validate(r#"{"coverTitles":["这个标题已经超过十个汉字了"],"noteTitles":["测试"]}"#)
                .is_err()
        );
        assert!(validate(r#"{"coverTitles":["测试","测试"],"noteTitles":["测试"]}"#).is_err());
        assert!(validate(r#"{"coverTitles":["测试"],"noteTitles":["测试"],"extra":1}"#).is_err());
    }
}
