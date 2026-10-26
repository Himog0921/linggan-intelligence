//! Rule `ops-link.v1`: platform identities that can be read from a link without network access.
//! Only Xiaohongshu full links carry an identity; short links and every Douyin link wait for the
//! D5 homepage patrol to be matched.

use super::export_rows::Platform;

/// Where a publication's platform content id came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ContentIdState {
    ParsedPublicationUrl,
    ParsedCanonicalUrl,
    PendingMatch,
    Conflict,
}

impl ContentIdState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ParsedPublicationUrl => "parsed_publication_url",
            Self::ParsedCanonicalUrl => "parsed_canonical_url",
            Self::PendingMatch => "pending_match",
            Self::Conflict => "conflict",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        [
            Self::ParsedPublicationUrl,
            Self::ParsedCanonicalUrl,
            Self::PendingMatch,
            Self::Conflict,
        ]
        .into_iter()
        .find(|state| state.as_str() == value)
    }
}

/// Reads a publication's content id: the publication link first, then the canonical link.
/// Two different ids are a conflict and neither is trusted.
pub(super) fn publication_content_id(
    platform: Platform,
    publication_url: &str,
    canonical_url: &str,
) -> (ContentIdState, Option<String>) {
    if platform != Platform::Xhs {
        return (ContentIdState::PendingMatch, None);
    }
    match (xhs_note_id(publication_url), xhs_note_id(canonical_url)) {
        (Some(first), Some(second)) if first != second => (ContentIdState::Conflict, None),
        (Some(id), _) => (ContentIdState::ParsedPublicationUrl, Some(id)),
        (None, Some(id)) => (ContentIdState::ParsedCanonicalUrl, Some(id)),
        (None, None) => (ContentIdState::PendingMatch, None),
    }
}

/// Account homepage identity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ProfileLinkState {
    Parsed,
    PendingMatch,
    Missing,
}

impl ProfileLinkState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::PendingMatch => "pending_match",
            Self::Missing => "missing",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        [Self::Parsed, Self::PendingMatch, Self::Missing]
            .into_iter()
            .find(|state| state.as_str() == value)
    }
}

pub(super) fn profile_identity(
    platform: Platform,
    profile_url: Option<&str>,
) -> (ProfileLinkState, Option<String>) {
    let Some(url) = profile_url else {
        return (ProfileLinkState::Missing, None);
    };
    let user_id = (platform == Platform::Xhs)
        .then(|| xhs_path_id(url, &[&["user", "profile"]]))
        .flatten();
    match user_id {
        Some(id) => (ProfileLinkState::Parsed, Some(id)),
        None => (ProfileLinkState::PendingMatch, None),
    }
}

/// Parses the stable Xiaohongshu note identity from a full note URL without visiting the
/// platform. Short links deliberately do not pass this boundary.
pub fn xhs_note_id(url: &str) -> Option<String> {
    xhs_path_id(url, &[&["explore"], &["discovery", "item"]])
}

/// Returns the 24-character lowercase hex id that directly follows one of the path prefixes on a
/// xiaohongshu.com host. Query strings and fragments are ignored; anything else is not an id.
fn xhs_path_id(url: &str, prefixes: &[&[&str]]) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?;
    if host != "xiaohongshu.com" && host != "www.xiaohongshu.com" {
        return None;
    }
    let segments: Vec<&str> = parsed
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    prefixes.iter().find_map(|prefix| {
        let rest = segments.strip_prefix(*prefix)?;
        let [id] = rest else { return None };
        let is_id = id.len() == 24
            && id
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        is_id.then(|| (*id).to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::{ContentIdState, ProfileLinkState, profile_identity, publication_content_id};
    use crate::ops_ledger::export_rows::Platform;

    const NOTE: &str = "6650a1b2c3d4e5f60718293a";
    const OTHER: &str = "6650a1b2c3d4e5f60718293b";

    #[test]
    fn xiaohongshu_full_links_carry_the_note_id() {
        let explore =
            format!("https://www.xiaohongshu.com/explore/{NOTE}?xsec_token=x&xsec_source=pc");
        let item = format!("http://xiaohongshu.com/discovery/item/{NOTE}/");
        for url in [explore.as_str(), item.as_str()] {
            assert_eq!(
                publication_content_id(Platform::Xhs, url, "https://example.invalid/"),
                (ContentIdState::ParsedPublicationUrl, Some(NOTE.to_owned()))
            );
        }
        let canonical = format!("https://www.xiaohongshu.com/explore/{NOTE}");
        assert_eq!(
            publication_content_id(Platform::Xhs, "https://xhslink.com/a/AbC", &canonical),
            (ContentIdState::ParsedCanonicalUrl, Some(NOTE.to_owned()))
        );
    }

    #[test]
    fn short_links_lookalikes_and_douyin_wait_for_matching() {
        let pending = (ContentIdState::PendingMatch, None);
        for url in [
            "https://xhslink.com/a/AbC".to_owned(),
            format!(
                "https://www.xiaohongshu.com/explore/{}",
                NOTE.to_uppercase()
            ),
            format!("https://www.xiaohongshu.com/explore/{NOTE}/comments"),
            format!("https://evil.example/www.xiaohongshu.com/explore/{NOTE}"),
            format!("https://xiaohongshu.com.evil.example/explore/{NOTE}"),
            "not a url".to_owned(),
        ] {
            assert_eq!(
                publication_content_id(Platform::Xhs, &url, &url),
                pending,
                "{url}"
            );
        }
        let douyin = "https://www.douyin.com/video/7300000000000000000";
        assert_eq!(
            publication_content_id(Platform::Douyin, douyin, douyin),
            pending
        );
    }

    #[test]
    fn two_different_ids_are_a_conflict() {
        let first = format!("https://www.xiaohongshu.com/explore/{NOTE}");
        let second = format!("https://www.xiaohongshu.com/explore/{OTHER}");
        assert_eq!(
            publication_content_id(Platform::Xhs, &first, &second),
            (ContentIdState::Conflict, None)
        );
    }

    #[test]
    fn profile_links_are_parsed_only_for_xiaohongshu_profiles() {
        let profile = format!("https://www.xiaohongshu.com/user/profile/{NOTE}?xsec_token=1");
        assert_eq!(
            profile_identity(Platform::Xhs, Some(&profile)),
            (ProfileLinkState::Parsed, Some(NOTE.to_owned()))
        );
        assert_eq!(
            profile_identity(Platform::Xhs, Some("https://xhslink.com/m/AbC")),
            (ProfileLinkState::PendingMatch, None)
        );
        assert_eq!(
            profile_identity(
                Platform::Douyin,
                Some("https://www.douyin.com/user/MS4wLjABAAAA")
            ),
            (ProfileLinkState::PendingMatch, None)
        );
        assert_eq!(
            profile_identity(Platform::Xhs, None),
            (ProfileLinkState::Missing, None)
        );
    }
}
