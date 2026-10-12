//! Shared navigation context for the existing Collection management pages.
//!
//! A return location is navigation only: it never scopes stations or changes a command.
//! Only the two daily Collection pages may be return destinations.

use super::target_drawer::percent_encode_component;

const MAX_RETURN_BYTES: usize = 2048;
const BODY_START: &str = "<!-- collection-body:start -->";
const RETURN_MARKER: &str = "data-collection-management-context";

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

    fn label(&self) -> &'static str {
        if path_only(&self.return_to) == "/collection/tasks" {
            "返回采集任务"
        } else {
            "返回观察目标"
        }
    }
}

fn path_only(value: &str) -> &str {
    value.split(['?', '#']).next().unwrap_or(value)
}

fn valid_return_to(value: Option<&str>) -> Option<&str> {
    let value = value?;
    if value.len() > MAX_RETURN_BYTES
        || !matches!(path_only(value), "/collection/targets" | "/collection/tasks")
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

fn management_path(value: &str) -> bool {
    matches!(path_only(value), "/collection/runtime" | "/collection/domains")
}

fn management_action(value: &str) -> bool {
    management_path(value)
        || path_only(value).starts_with("/collection/runtime/")
        || path_only(value).starts_with("/collection/domains/")
}

/// `base` is a server-owned route, optionally with its existing domain query/fragment.
/// Old callers without a valid return context keep their original redirect location.
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

/// Decorate the original SSR document, including empty/read-failed and POST-draft responses.
/// Reapplying it adds context to a newly replaced draft without duplicating existing controls.
pub(super) fn decorate(html: &str, return_to: Option<&str>, domain: Option<&str>) -> String {
    let context = ManagementContext::new(return_to, domain);
    let mut rendered = preserve_management_context(html, context.return_to());
    let scope = if html.contains("class=\"c-runtime")
        || html.contains("<title>执行工位 ·")
        || html.contains("<title>工位管理 ·")
    {
        "<span>工位是全局共享资源，当前领域仅用于返回工作页。</span>"
    } else {
        ""
    };
    let bar = format!(
        r#"<nav class="c-management-return" {RETURN_MARKER} aria-label="返回采集工作页"><a class="c-btn c-btn-quiet" data-collection-management-return href="{}">{}</a>{scope}</nav>"#,
        escape(context.return_to()),
        context.label(),
    );
    let bar_start = format!(r#"<nav class="c-management-return" {RETURN_MARKER}"#);
    if let Some(at) = rendered.find(&bar_start) {
        if let Some(end) = rendered[at..].find("</nav>") {
            rendered.replace_range(at..at + end + "</nav>".len(), &bar);
        }
    } else if let Some(at) = rendered.find(BODY_START) {
        rendered.insert_str(at + BODY_START.len(), &bar);
    } else if let Some(at) = rendered.find("<main") {
        if let Some(end) = tag_end(&rendered, at) {
            rendered.insert_str(end, &bar);
        }
    }
    rendered
}

fn preserve_management_context(html: &str, return_to: &str) -> String {
    let hidden = format!(
        r#"<input type="hidden" name="return_to" value="{}">"#,
        escape(return_to),
    );
    let mut result = String::with_capacity(html.len() + hidden.len());
    let mut cursor = 0;
    let mut management_form = false;
    while let Some(offset) = html[cursor..].find('<') {
        let start = cursor + offset;
        result.push_str(&html[cursor..start]);
        let Some(end) = tag_end(html, start) else {
            result.push_str(&html[start..]);
            return result;
        };
        let tag = &html[start..end];
        if is_tag(tag, "form") {
            management_form = attribute(tag, "action").is_some_and(management_action);
            result.push_str(tag);
            if management_form {
                result.push_str(&hidden);
            }
        } else if is_tag(tag, "/form") {
            management_form = false;
            result.push_str(tag);
        } else if management_form
            && is_tag(tag, "input")
            && attribute(tag, "name") == Some("return_to")
        {
            // Replace an old hidden context with the single canonical value above.
        } else if is_tag(tag, "a") {
            if let Some(range) = attribute_range(tag, "href") {
                let href = unescape(&tag[range.clone()]);
                if management_path(&href) {
                    result.push_str(&tag[..range.start]);
                    result.push_str(&escape(&redirect_path(&href, None, Some(return_to))));
                    result.push_str(&tag[range.end..]);
                } else {
                    result.push_str(tag);
                }
            } else {
                result.push_str(tag);
            }
        } else {
            result.push_str(tag);
        }
        cursor = end;
    }
    result.push_str(&html[cursor..]);
    result
}

fn is_tag(tag: &str, name: &str) -> bool {
    tag.strip_prefix('<').is_some_and(|body| {
        body.get(..name.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
            && body
                .as_bytes()
                .get(name.len())
                .is_some_and(|next| next.is_ascii_whitespace() || matches!(*next, b'>' | b'/'))
    })
}

fn tag_end(html: &str, start: usize) -> Option<usize> {
    if html[start..].starts_with("<!--") {
        return html[start..].find("-->").map(|end| start + end + 3);
    }
    let mut quote = None;
    for (offset, byte) in html.as_bytes()[start..].iter().copied().enumerate() {
        match (quote, byte) {
            (Some(expected), actual) if expected == actual => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Some(start + offset + 1),
            _ => {}
        }
    }
    None
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    attribute_range(tag, name).map(|range| &tag[range])
}

// The input is our escaped SSR markup. Read attributes instead of broad replacement so a
// data-action attribute or a submitted value cannot be mistaken for a form action.
fn attribute_range(tag: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let bytes = tag.as_bytes();
    let mut index = bytes.iter().position(u8::is_ascii_whitespace)?;
    while index < bytes.len() {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        let start = index;
        while bytes
            .get(index)
            .is_some_and(|c| !c.is_ascii_whitespace() && !matches!(*c, b'=' | b'>' | b'/'))
        {
            index += 1;
        }
        if start == index {
            return None;
        }
        let matches = tag[start..index].eq_ignore_ascii_case(name);
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            continue;
        }
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        let quote = *bytes.get(index)?;
        let quoted = matches!(quote, b'\'' | b'"');
        if quoted {
            index += 1;
        }
        let value_start = index;
        while bytes.get(index).is_some_and(|c| {
            if quoted {
                *c != quote
            } else {
                !c.is_ascii_whitespace() && *c != b'>'
            }
        }) {
            index += 1;
        }
        if matches {
            return Some(value_start..index);
        }
        if quoted {
            index += 1;
        }
    }
    None
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
#[path = "collection_management_tests.rs"]
mod tests;
