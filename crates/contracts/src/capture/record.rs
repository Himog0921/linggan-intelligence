//! Record envelope of `capture.package.v1`.
//!
//! The envelope is a closed contract: every field below must be present, and the fixed v1 values
//! are enums so an unknown one fails the schema instead of degrading to a string comparison. The
//! `payload` subtree stays an opaque JSON value here; only the per-record processor of a later
//! stage interprets `content-detail.synthetic.v1` fields.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// One immutable record envelope carried by a capture package.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureRecord {
    ordinal: u32,
    record_kind: RecordKind,
    target_external_id: String,
    source: RecordSource,
    observed_at: ObservedAt,
    payload: Value,
    record_hash: String,
}

impl CaptureRecord {
    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn record_kind(&self) -> &'static str {
        self.record_kind.as_str()
    }

    pub fn target_external_id(&self) -> &str {
        &self.target_external_id
    }

    pub fn source_system(&self) -> &'static str {
        self.source.system.as_str()
    }

    pub fn source_namespace(&self) -> &'static str {
        self.source.namespace.as_str()
    }

    pub fn source_object_type(&self) -> &'static str {
        self.source.object_type.as_str()
    }

    /// `None` means the envelope explicitly stated it has no stable external id, which is a legal
    /// contract value, not a missing field. It is the only statement that may establish an
    /// identity; the target and payload statements never substitute for it.
    pub fn source_external_id(&self) -> Option<&str> {
        self.source.external_id.as_deref()
    }

    pub fn source_channel(&self) -> &'static str {
        self.source.channel.as_str()
    }

    /// The producer's observation instant as a strict UTC RFC 3339 string, e.g.
    /// `2026-08-20T08:00:00Z`. It is never a receive, accept, insert or process time.
    pub fn observed_at_value(&self) -> &str {
        &self.observed_at.value
    }

    pub fn observed_at_precision(&self) -> &'static str {
        self.observed_at.precision.as_str()
    }

    pub fn observed_at_basis(&self) -> &'static str {
        self.observed_at.basis.as_str()
    }

    pub fn record_hash(&self) -> &str {
        &self.record_hash
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }
}

/// The envelope's own statement about which stable platform object this record describes. It is
/// the only statement that may create a Source Identity; `targetExternalId` and
/// `payload.sourceExternalId` never substitute for it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordSource {
    system: SourceSystem,
    namespace: SourceNamespace,
    object_type: SourceObjectType,
    /// The field is mandatory; its value may be an explicit JSON null. `deserialize_with` keeps
    /// serde from treating a missing field as `None`.
    #[serde(deserialize_with = "required_nullable_string")]
    external_id: Option<String>,
    channel: SourceChannel,
}

/// When the producer actually observed this state at the source. It is never the time the
/// delivery was received, accepted, inserted or processed.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservedAt {
    #[serde(deserialize_with = "strict_utc_rfc3339")]
    value: String,
    precision: ObservedAtPrecision,
    basis: ObservedAtBasis,
}

macro_rules! fixed_value_enum {
    ($name:ident, $variant:ident, $wire:literal) => {
        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum $name {
            $variant,
        }

        impl $name {
            fn as_str(&self) -> &'static str {
                $wire
            }
        }
    };
}

fixed_value_enum!(RecordKind, ContentDetail, "content_detail");
fixed_value_enum!(SourceSystem, Synthetic, "synthetic");
fixed_value_enum!(SourceObjectType, Content, "content");
fixed_value_enum!(SourceChannel, SyntheticPage, "synthetic_page");
fixed_value_enum!(ObservedAtPrecision, Exact, "exact");
fixed_value_enum!(ObservedAtBasis, Fixture, "fixture");

#[derive(Debug, Deserialize)]
enum SourceNamespace {
    #[serde(rename = "scope-001")]
    Scope001,
}

impl SourceNamespace {
    fn as_str(&self) -> &'static str {
        "scope-001"
    }
}

/// Accepts a JSON string or an explicit null, and rejects a missing field.
fn required_nullable_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

/// SCOPE-001 freezes one lexical form: `YYYY-MM-DDTHH:MM:SSZ` in UTC. A legal RFC 3339 value
/// with an offset, lowercase designators or fractional seconds is a different lexical contract
/// and fails closed rather than being normalized.
fn strict_utc_rfc3339<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    verify_strict_utc_rfc3339(&value)
        .map_err(|reason| serde::de::Error::custom(format!("{reason}: {value}")))?;
    Ok(value)
}

fn verify_strict_utc_rfc3339(value: &str) -> Result<(), &'static str> {
    let bytes = value.as_bytes();
    if bytes.len() != 20 {
        return Err("observedAt.value must be exactly YYYY-MM-DDTHH:MM:SSZ");
    }
    for (index, byte) in bytes.iter().enumerate() {
        let expected_digit = matches!(index, 0..=3 | 5 | 6 | 8 | 9 | 11 | 12 | 14 | 15 | 17 | 18);
        let ok = match index {
            _ if expected_digit => byte.is_ascii_digit(),
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => false,
        };
        if !ok {
            return Err("observedAt.value must be exactly YYYY-MM-DDTHH:MM:SSZ");
        }
    }

    let number = |from: usize, to: usize| value[from..to].parse::<u32>().unwrap_or(u32::MAX);
    let (year, month, day) = (number(0, 4), number(5, 7), number(8, 10));
    let (hour, minute, second) = (number(11, 13), number(14, 16), number(17, 19));

    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return Err("observedAt.value must be a real calendar date");
    }
    if hour > 23 || minute > 59 || second > 59 {
        return Err("observedAt.value must be a real UTC time of day");
    }
    Ok(())
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}
