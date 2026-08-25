use serde::Deserialize;

use super::query::DiscoverySort;

/// One platform content identity together with a distinct discovery occurrence. The occurrence
/// records why, when, and at which result position this card was displayed; position is not a
/// ContentItem or Observation field.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryCard {
    pub(super) content: VisibleContentCard,
    pub(super) occurrence: DiscoveryOccurrence,
}

impl DiscoveryCard {
    pub fn content(&self) -> &VisibleContentCard {
        &self.content
    }

    pub fn occurrence(&self) -> &DiscoveryOccurrence {
        &self.occurrence
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisibleContentCard {
    pub(super) platform_content_id: String,
    title: Option<String>,
    creator_display_name: Option<String>,
    published_at_source_text: Option<String>,
    cover_candidate: Option<MediaCandidate>,
}

impl VisibleContentCard {
    pub fn platform_content_id(&self) -> &str {
        &self.platform_content_id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn creator_display_name(&self) -> Option<&str> {
        self.creator_display_name.as_deref()
    }

    pub fn published_at_source_text(&self) -> Option<&str> {
        self.published_at_source_text.as_deref()
    }

    pub fn cover_candidate(&self) -> Option<&MediaCandidate> {
        self.cover_candidate.as_ref()
    }
}

/// A visible external media address is provenance only. It cannot produce a page display URL;
/// a later media-acquisition contract must first obtain and verify a MediaBlob and local replica.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaCandidate {
    observed_external_uri: String,
}

impl MediaCandidate {
    pub fn observed_external_uri(&self) -> &str {
        &self.observed_external_uri
    }

    pub fn presentation_state(&self) -> CoverPresentationState {
        CoverPresentationState::MediaNotAcquired
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverPresentationState {
    MediaNotAcquired,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveryOccurrence {
    pub(super) query: String,
    pub(super) sort: DiscoverySort,
    pub(super) observed_at: String,
    pub(super) result_position: u16,
}

impl DiscoveryOccurrence {
    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn observed_at(&self) -> &str {
        &self.observed_at
    }

    pub fn result_position(&self) -> u16 {
        self.result_position
    }
}
