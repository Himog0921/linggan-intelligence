//! Record envelope of `capture.package.v1`. The payload subtree stays an opaque JSON value here;
//! only the per-record parser of a later slice interprets `content-detail.synthetic.v1` fields.

use serde::Deserialize;
use serde_json::Value;

/// One immutable record envelope carried by a capture package.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRecord {
    ordinal: u32,
    #[serde(rename = "recordKind")]
    _record_kind: RecordKind,
    target_external_id: String,
    #[serde(rename = "source")]
    _source: RecordSource,
    #[serde(rename = "observedAt")]
    _observed_at: ObservedAt,
    payload: Value,
    record_hash: String,
}

impl CaptureRecord {
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn target_external_id(&self) -> &str {
        &self.target_external_id
    }

    pub fn record_hash(&self) -> &str {
        &self.record_hash
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RecordKind {
    ContentDetail,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordSource {
    #[serde(rename = "system")]
    _system: String,
    #[serde(rename = "namespace")]
    _namespace: String,
    #[serde(rename = "objectType")]
    _object_type: String,
    #[serde(rename = "externalId")]
    _external_id: String,
    #[serde(rename = "channel")]
    _channel: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservedAt {
    #[serde(rename = "value")]
    _value: String,
    #[serde(rename = "precision")]
    _precision: String,
    #[serde(rename = "basis")]
    _basis: String,
}
