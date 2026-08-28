//! Query-bound keyset cursors for the local material read projection.
//!
//! The checksum catches accidental corruption only. The payload is always treated as untrusted
//! input and is not an authorization boundary.

use linggan_contracts::{EvidenceQuery, EvidenceQuerySort, EvidenceTimeView};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CURSOR_VERSION: &str = "material-keyset-v1";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialCursor {
    pub as_of: String,
    pub last_observed_at: String,
    pub last_platform: String,
    pub last_content_external_id: String,
    query_fingerprint: String,
}

pub(crate) fn encode(query: &EvidenceQuery, cursor: MaterialCursor) -> String {
    let payload = serde_json::to_vec(&cursor).expect("material cursor serializes");
    let encoded = hex_encode(&payload);
    let checksum = digest(&format!("{CURSOR_VERSION}:{encoded}"));
    debug_assert_eq!(cursor.query_fingerprint, fingerprint(query));
    format!("m1.{encoded}.{checksum}")
}

pub(crate) fn decode(query: &EvidenceQuery, value: &str) -> Option<MaterialCursor> {
    let mut parts = value.split('.');
    if parts.next()? != "m1" {
        return None;
    }
    let encoded = parts.next()?;
    let checksum = parts.next()?;
    if parts.next().is_some() || checksum != digest(&format!("{CURSOR_VERSION}:{encoded}")) {
        return None;
    }
    let cursor: MaterialCursor = serde_json::from_slice(&hex_decode(encoded)?).ok()?;
    (cursor.query_fingerprint == fingerprint(query)
        && cursor.as_of.len() <= 64
        && cursor.last_observed_at.len() <= 256
        && matches!(cursor.last_platform.as_str(), "xhs" | "douyin")
        && !cursor.last_content_external_id.is_empty()
        && cursor.last_content_external_id.len() <= 512)
        .then_some(cursor)
}

pub(crate) fn for_last_item(
    query: &EvidenceQuery,
    as_of: String,
    observed_at: String,
    platform: String,
    content_external_id: String,
) -> MaterialCursor {
    MaterialCursor {
        as_of,
        last_observed_at: observed_at,
        last_platform: platform,
        last_content_external_id: content_external_id,
        query_fingerprint: fingerprint(query),
    }
}

fn fingerprint(query: &EvidenceQuery) -> String {
    let time_view = match query.time_view() {
        EvidenceTimeView::LatestAcceptedDiscovery => "latest_accepted_discovery",
        EvidenceTimeView::PublishedLast7Days => "published_last_7_days",
        EvidenceTimeView::PublishedLast30Days => "published_last_30_days",
    };
    let sort = match query.sort() {
        EvidenceQuerySort::LatestDiscovery => "latest_discovery",
        EvidenceQuerySort::Relevance => "relevance",
    };
    digest(&format!(
        "q={:?}|time={time_view}|sort={sort}|lane={:?}|state={:?}|media={:?}|restriction={:?}",
        query.text(),
        query.lane(),
        query.lane_state(),
        query.media_kind(),
        query.restriction()
    ))
}

fn digest(value: &str) -> String {
    hex_encode(&Sha256::digest(value.as_bytes()))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}
