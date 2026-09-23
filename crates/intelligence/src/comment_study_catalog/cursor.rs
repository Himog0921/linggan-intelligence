//! Bounded resource/query-bound cursors; never authorization tokens.
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::StudyCatalogError;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const MAX_CURSOR_BYTES: usize = 2048;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CommentPosition {
    pub received_at: String,
    pub work_ref: Uuid,
    pub comment_external_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EntryPosition {
    pub created_at: String,
    pub reference: Uuid,
}

pub(super) trait CursorPosition {
    fn timestamp(&self) -> &str;
    fn valid_key(&self) -> bool;
}

impl CursorPosition for CommentPosition {
    fn timestamp(&self) -> &str { &self.received_at }
    fn valid_key(&self) -> bool {
        !self.work_ref.is_nil() && !self.comment_external_id.is_empty()
            && self.comment_external_id.len() <= 512 && !self.comment_external_id.contains('\0')
    }
}

impl CursorPosition for EntryPosition {
    fn timestamp(&self) -> &str { &self.created_at }
    fn valid_key(&self) -> bool { !self.reference.is_nil() }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Cursor<P = CommentPosition> {
    v: u8,
    resource: String,
    scope_hash: String,
    pub as_of: String,
    pub last: P,
}

pub(super) fn scope_hash(scope: &Value) -> Result<String, StudyCatalogError> {
    let bytes = serde_json::to_vec(scope).map_err(|_| StudyCatalogError::InvalidQuery)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(super) fn encode(
    scope_hash: &str,
    as_of: &str,
    last: CommentPosition,
) -> Result<String, StudyCatalogError> {
    encode_for("comments", scope_hash, as_of, last)
}

pub(super) fn decode(value: &str, expected_scope: &str) -> Result<Cursor, StudyCatalogError> {
    decode_for("comments", value, expected_scope)
}

pub(super) fn encode_for<P: Serialize + CursorPosition>(
    resource: &str,
    scope_hash: &str,
    as_of: &str,
    last: P,
) -> Result<String, StudyCatalogError> {
    let cursor = Cursor {
        v: 1, resource: resource.to_owned(), scope_hash: scope_hash.to_owned(),
        as_of: as_of.to_owned(), last,
    };
    validate(&cursor)?;
    let bytes = serde_json::to_vec(&cursor).map_err(|_| StudyCatalogError::InvalidCursor)?;
    let encoded = encode_bytes(&bytes);
    if encoded.len() > MAX_CURSOR_BYTES { return Err(StudyCatalogError::InvalidCursor); }
    Ok(encoded)
}

pub(super) fn decode_for<P: DeserializeOwned + CursorPosition>(
    resource: &str,
    value: &str,
    expected_scope: &str,
) -> Result<Cursor<P>, StudyCatalogError> {
    let bytes = decode_bytes(value).ok_or(StudyCatalogError::InvalidCursor)?;
    // Check the resource before decoding its differently-shaped position. A history cursor must
    // not be accepted as a comment cursor, even when a caller supplies a matching checksum.
    let envelope: Cursor<Value> = serde_json::from_slice(&bytes)
        .map_err(|_| StudyCatalogError::InvalidCursor)?;
    if envelope.resource != resource || envelope.scope_hash != expected_scope {
        return Err(StudyCatalogError::CursorScopeMismatch);
    }
    let cursor = Cursor {
        v: envelope.v, resource: envelope.resource, scope_hash: envelope.scope_hash,
        as_of: envelope.as_of,
        last: serde_json::from_value(envelope.last).map_err(|_| StudyCatalogError::InvalidCursor)?,
    };
    validate(&cursor)?;
    Ok(cursor)
}

fn validate<P: CursorPosition>(cursor: &Cursor<P>) -> Result<(), StudyCatalogError> {
    if cursor.v != 1
        || !matches!(cursor.resource.as_str(), "comments" | "comment-history" | "comment-versions" | "works")
        || cursor.scope_hash.len() != 64
        || !cursor.scope_hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !utc_timestamp_shape(&cursor.as_of)
        || !utc_timestamp_shape(cursor.last.timestamp())
        || cursor.last.timestamp() > cursor.as_of.as_str()
        || !cursor.last.valid_key()
    {
        return Err(StudyCatalogError::InvalidCursor);
    }
    Ok(())
}

// PostgreSQL performs calendar validation on the bound value. No date enters SQL text.
fn utc_timestamp_shape(value: &str) -> bool {
    value.len() == 27 && value.bytes().enumerate().all(|(index, byte)| match index {
        4 | 7 => byte == b'-', 10 => byte == b'T', 13 | 16 => byte == b':',
        19 => byte == b'.', 26 => byte == b'Z', _ => byte.is_ascii_digit(),
    })
}

fn encode_bytes(bytes: &[u8]) -> String {
    let mut output = String::new();
    let (mut bits, mut available) = (0_u32, 0_u8);
    for byte in bytes {
        bits = (bits << 8) | u32::from(*byte);
        available += 8;
        while available >= 6 {
            available -= 6;
            output.push(ALPHABET[((bits >> available) & 63) as usize] as char);
        }
        bits &= (1_u32 << available) - 1;
    }
    if available != 0 { output.push(ALPHABET[((bits << (6 - available)) & 63) as usize] as char); }
    output
}

fn decode_bytes(value: &str) -> Option<Vec<u8>> {
    if value.is_empty() || value.len() > MAX_CURSOR_BYTES || value.len() % 4 == 1 { return None; }
    let (mut bits, mut available) = (0_u32, 0_u8);
    let mut bytes = Vec::with_capacity(value.len() * 3 / 4);
    for byte in value.bytes() {
        let number = ALPHABET.iter().position(|candidate| *candidate == byte)?;
        bits = (bits << 6) | number as u32;
        available += 6;
        if available >= 8 {
            available -= 8;
            bytes.push((bits >> available) as u8);
        }
        bits &= (1_u32 << available) - 1;
    }
    (bits == 0 && encode_bytes(&bytes) == value).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn position() -> CommentPosition {
        CommentPosition {
            received_at: "2026-09-22T01:02:03.000001Z".to_owned(),
            work_ref: Uuid::from_u128(1), comment_external_id: "评论_1".to_owned(),
        }
    }

    #[test]
    fn base64url_is_canonical_and_round_trips_unicode() {
        for (raw, expected) in [("f", "Zg"), ("fo", "Zm8"), ("foo", "Zm9v"), ("你好", "5L2g5aW9")] {
            assert_eq!(encode_bytes(raw.as_bytes()), expected);
            assert_eq!(decode_bytes(expected).unwrap(), raw.as_bytes());
        }
        for invalid in ["", "A", "Zh", "Zg=", "Zg==", "é", "____ "] {
            assert!(decode_bytes(invalid).is_none(), "accepted {invalid}");
        }
        assert!(decode_bytes(&"A".repeat(2049)).is_none());
    }

    #[test]
    fn cursor_binds_scope_but_does_not_encode_page_size() {
        let first = scope_hash(&json!({"domain":1,"q":"药","voiceRole":"reader"})).unwrap();
        let other = scope_hash(&json!({"domain":1,"q":"作业","voiceRole":"reader"})).unwrap();
        let encoded = encode(&first, "2026-09-23T00:00:00.000000Z", position()).unwrap();
        assert_eq!(decode(&encoded, &first).unwrap().last.comment_external_id, "评论_1");
        assert!(matches!(decode(&encoded, &other), Err(StudyCatalogError::CursorScopeMismatch)));
        let mut value: Value = serde_json::from_slice(&decode_bytes(&encoded).unwrap()).unwrap();
        value["extra"] = json!(true);
        assert!(decode(&encode_bytes(&serde_json::to_vec(&value).unwrap()), &first).is_err());
    }

    #[test]
    fn entry_cursors_reject_other_resources_and_other_comment_keys() {
        let hash = scope_hash(&json!({"comment":"one"})).unwrap();
        let other = scope_hash(&json!({"comment":"two"})).unwrap();
        let last = EntryPosition { created_at: position().received_at, reference: Uuid::from_u128(2) };
        let encoded = encode_for("comment-history", &hash, "2026-09-23T00:00:00.000000Z", last).unwrap();
        assert_eq!(decode_for::<EntryPosition>("comment-history", &encoded, &hash).unwrap().last.reference, Uuid::from_u128(2));
        assert!(matches!(decode(&encoded, &hash), Err(StudyCatalogError::CursorScopeMismatch)));
        assert!(matches!(decode_for::<EntryPosition>("comment-versions", &encoded, &hash), Err(StudyCatalogError::CursorScopeMismatch)));
        assert!(matches!(decode_for::<EntryPosition>("comment-history", &encoded, &other), Err(StudyCatalogError::CursorScopeMismatch)));
    }

    #[test]
    fn invalid_position_cannot_create_a_cursor() {
        let hash = "0".repeat(64);
        let last = EntryPosition { created_at: position().received_at, reference: Uuid::nil() };
        assert!(encode_for("comment-history", &hash, "2026-09-23T00:00:00.000000Z", last).is_err());
        let last = EntryPosition { created_at: "2026-09-23".into(), reference: Uuid::from_u128(2) };
        assert!(encode_for("comment-history", &hash, "2026-09-23T00:00:00.000000Z", last).is_err());
    }
}
