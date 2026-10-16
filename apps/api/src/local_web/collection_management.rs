//! Validated return navigation for collection management commands.
use super::target_drawer::percent_encode_component;
const MAX_RETURN_BYTES: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ManagementContext {
    return_to: String,
}

impl ManagementContext {
    pub(super) fn new(return_to: Option<&str>, domain: Option<&str>) -> Self {
        let return_to = valid_return_to(return_to)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let mut fallback = "/collection/targets".to_owned();
                if let Some(domain) = domain.map(str::trim).filter(|value| {
                    !value.is_empty()
                        && value.len() <= 128
                        && !value.chars().any(|c| c.is_control() || c == '\\')
                }) {
                    fallback.push_str("?domain=");
                    fallback.push_str(&percent_encode_component(domain));
                }
                fallback
            });
        Self { return_to }
    }

    pub(super) fn return_to(&self) -> &str {
        &self.return_to
    }
}

fn path_only(value: &str) -> &str {
    value.split(['?', '#']).next().unwrap_or(value)
}

fn valid_return_to(value: Option<&str>) -> Option<&str> {
    let value = value?;
    if value.len() > MAX_RETURN_BYTES
        || !matches!(
            path_only(value),
            "/collection/targets" | "/collection/tasks"
        )
        || value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '\\')
    {
        return None;
    }
    // Encoded separators in the path already fail the exact path comparison. Reject encoded
    // controls/backslashes in query and fragment too, without normalizing the original query.
    let bytes = value.as_bytes();
    let mut index = 0;
    let mut decoded_value = Vec::with_capacity(bytes.len());
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = char::from(*bytes.get(index + 1)?).to_digit(16)?;
            let low = char::from(*bytes.get(index + 2)?).to_digit(16)?;
            let decoded = (high * 16 + low) as u8;
            if decoded.is_ascii_control() || decoded == b'\\' {
                return None;
            }
            decoded_value.push(decoded);
            index += 3;
        } else {
            decoded_value.push(bytes[index]);
            index += 1;
        }
    }
    if std::str::from_utf8(&decoded_value)
        .ok()?
        .chars()
        .any(|c| c.is_control() || c == '\\')
    {
        return None;
    }
    Some(value)
}

pub(super) fn redirect_path(base: &str, error: Option<&str>, return_to: Option<&str>) -> String {
    let mut location = base.to_owned();
    if let Some(error) = error.filter(|value| !value.is_empty()) {
        location = with_query_parameter(&location, "error", error);
    }
    if let Some(return_to) = valid_return_to(return_to) {
        location = with_query_parameter(&location, "return_to", return_to);
    }
    location
}

fn with_query_parameter(url: &str, name: &str, value: &str) -> String {
    let (before_fragment, fragment) = url
        .split_once('#')
        .map_or((url, None), |(a, b)| (a, Some(b)));
    let (path, query) = before_fragment
        .split_once('?')
        .unwrap_or((before_fragment, ""));
    let mut parameters: Vec<String> = query
        .split('&')
        .filter(|part| !part.is_empty() && part.split('=').next() != Some(name))
        .map(str::to_owned)
        .collect();
    parameters.push(format!("{name}={}", percent_encode_component(value)));
    let mut result = format!("{path}?{}", parameters.join("&"));
    if let Some(fragment) = fragment {
        result.push('#');
        result.push_str(fragment);
    }
    result
}

#[cfg(test)]
#[path = "collection_management_tests.rs"]
mod tests;
