//! Local read scope shared by creator discovery and precise Corpus backlinks.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatorScope {
    pub domain: Uuid,
    #[serde(default = "primary")]
    pub usage_role: String,
    #[serde(default = "platform")]
    pub platform: String,
    pub creator_key: Option<String>,
    pub published_from: Option<String>,
    pub published_to: Option<String>,
    pub query: Option<String>,
    #[serde(default = "author")]
    pub search_mode: String,
    pub relevance: Option<String>,
    pub traits: Option<String>,
    /// Exact same-work analysis phrase; this is not a formal Topic identity.
    pub topic_hint: Option<String>,
    pub focus: Option<String>,
    pub observation: Option<String>,
    #[serde(default = "recent")]
    pub sort: String,
    pub min_likes: Option<i64>,
    #[serde(default)]
    pub viral: bool,
    #[serde(default)]
    pub high_likes: bool,
    #[serde(default)]
    pub unknown_author: bool,
    #[serde(default)]
    pub page: usize,
    #[serde(default = "page_size")]
    pub page_size: usize,
}
fn primary() -> String {
    "primary".into()
}
fn platform() -> String {
    "xhs".into()
}
fn author() -> String {
    "author".into()
}
fn recent() -> String { "recent".into() }
fn page_size() -> usize {
    50
}
impl CreatorScope {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !matches!(self.usage_role.as_str(), "primary" | "reference")
            || self.platform != "xhs"
            || !matches!(self.search_mode.as_str(), "author" | "work")
            || ![25, 50, 100].contains(&self.page_size)
            || self.page > 1_000_000
            || self.min_likes.is_some_and(|x| x < 0)
            || self.query.as_ref().is_some_and(|x| x.chars().count() > 200)
            || self.topic_hint.as_ref().is_some_and(|x| x.trim().is_empty() || x.chars().count() > 120)
            || !matches!(self.sort.as_str(), "recent" | "high_likes" | "related_works" | "viral_works")
            || self
                .relevance
                .as_deref()
                .is_some_and(|x| !matches!(x, "related" | "unrelated" | "unknown"))
            || self
                .focus
                .as_deref()
                .is_some_and(|x| !matches!(x, "vertical_tendency" | "multi_topic" | "unknown"))
            || self
                .observation
                .as_deref()
                .is_some_and(|x| !matches!(x, "outside" | "inside" | "monitoring" | "paused" | "other_domains_only" | "dismissed"))
            || self.traits.as_deref().is_some_and(|x| {
                x.split(',').any(|v| {
                    !matches!(
                        v,
                        "personal_experience" | "professional_output" | "explicit_promotion" | "institution_or_brand"
                    )
                })
            })
        {
            return Err("invalid_creator_scope");
        }
        for date in [&self.published_from, &self.published_to]
            .into_iter()
            .flatten()
        {
            if date.len() != 10
                || date.as_bytes()[4] != b'-'
                || date.as_bytes()[7] != b'-'
                || !date
                    .chars()
                    .enumerate()
                    .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
            {
                return Err("invalid_publication_date");
            }
        }
        if self
            .published_from
            .as_ref()
            .zip(self.published_to.as_ref())
            .is_some_and(|(a, b)| a >= b)
        {
            return Err("invalid_publication_range");
        }
        if let Some(key) = &self.creator_key {
            decode_creator_key(key).ok_or("invalid_creator_key")?;
        }
        Ok(())
    }
    pub fn base(&self) -> Self {
        let mut q = self.clone();
        q.creator_key = None;
        q.query = None;
        q.relevance = None;
        q.traits = None;
        q.topic_hint = None;
        q.focus = None;
        q.observation = None;
        q.sort = recent();
        q.min_likes = None;
        q.viral = false;
        q.high_likes = false;
        q.unknown_author = false;
        q.page = 0;
        q
    }
}
pub fn creator_key(platform: &str, id: &str) -> String {
    format!(
        "{platform}:{}",
        id.as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
pub fn decode_creator_key(key: &str) -> Option<(String, String)> {
    let (platform, hex) = key.split_once(':')?;
    if platform != "xhs"
        || hex.is_empty()
        || hex.len() > 1024
        || !hex.len().is_multiple_of(2)
        || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    let id = String::from_utf8(bytes).ok()?;
    if id.trim().is_empty() || id.chars().any(char::is_control) {
        return None;
    }
    Some((platform.into(), id))
}
