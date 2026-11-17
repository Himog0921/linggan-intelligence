//! Parse a complete L1 reply without changing its substantive content.
use serde_json::Value;

pub(crate) const REMOVED_TRAILING_BRACE: &str = "removed_one_trailing_brace";

pub(crate) fn parse(
    text: &str,
    complete: bool,
) -> Result<(Value, Option<&'static str>), &'static str> {
    let text = text.trim_matches([' ', '\t', '\r', '\n']);
    if text.is_empty() {
        return Err("empty_reply");
    }
    if let Ok(value) = serde_json::from_str(text) {
        return Ok((value, None));
    }
    if complete
        && let Some(prefix) = text.strip_suffix('}')
        && let Ok(value) = serde_json::from_str::<Value>(prefix)
        && value.is_object()
    {
        return Ok((value, Some(REMOVED_TRAILING_BRACE)));
    }
    Err("invalid_json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_removes_one_brace_after_a_complete_object() {
        let draft = r#"{"core":{"text":"a literal } stays"}}"#;
        assert_eq!(parse(draft, true).unwrap().1, None);
        let extra = format!("{draft}}}\n");
        let (value, repair) = parse(&extra, true).unwrap();
        assert_eq!(value, json!({"core":{"text":"a literal } stays"}}));
        assert_eq!(repair, Some(REMOVED_TRAILING_BRACE));
        assert_eq!(parse(&extra, false), Err("invalid_json"));
        for bad in [r#"{"core":}"#, "{}}}", "[]}", "{}{}", "{} } explanation"] {
            assert_eq!(parse(bad, true), Err("invalid_json"));
        }
        assert_eq!(parse(" \n\t", true), Err("empty_reply"));
    }
}
