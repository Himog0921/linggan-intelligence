use std::collections::HashSet;

use serde::de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) enum JsonIngressError {
    InvalidJson(serde_json::Error),
    DuplicateKey(String),
}

pub(crate) fn reject_duplicate_keys(input: &str) -> Result<(), JsonIngressError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let mut detector = DuplicateKeyDetector {
        duplicate_key: None,
    };

    match (&mut detector).deserialize(&mut deserializer) {
        Ok(()) => {
            deserializer.end().map_err(JsonIngressError::InvalidJson)?;
            match detector.duplicate_key {
                Some(key) => Err(JsonIngressError::DuplicateKey(key)),
                None => Ok(()),
            }
        }
        Err(error) => Err(JsonIngressError::InvalidJson(error)),
    }
}

pub(crate) fn first_unsafe_integer_token(input: &str) -> Option<&str> {
    let bytes = input.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = skip_json_string(bytes, index),
            b'-' | b'0'..=b'9' => {
                let start = index;
                while index < bytes.len() && is_json_number_character(bytes[index]) {
                    index += 1;
                }
                let token = &input[start..index];
                if is_unsafe_integer_token(token) {
                    return Some(token);
                }
            }
            _ => index += 1,
        }
    }

    None
}

pub(crate) fn sanitized_surrogate_input(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut offsets = Vec::new();

    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        index += 1;
        while index < bytes.len() && bytes[index] != b'"' {
            if let Some(code_unit) = unicode_escape_at(bytes, index) {
                if is_high_surrogate(code_unit) {
                    if let Some(low_surrogate) = unicode_escape_at(bytes, index + 6)
                        && is_low_surrogate(low_surrogate)
                    {
                        index += 12;
                        continue;
                    }
                    offsets.push(index);
                }
                if is_low_surrogate(code_unit) {
                    offsets.push(index);
                }
                index += 6;
                continue;
            }

            if bytes[index] == b'\\' {
                index += 2;
            } else {
                index += 1;
            }
        }
        index += 1;
    }

    if offsets.is_empty() {
        return None;
    }

    let mut sanitized = bytes.to_vec();
    for offset in offsets {
        sanitized[offset + 2..offset + 6].copy_from_slice(b"FFFD");
    }
    Some(String::from_utf8(sanitized).expect("surrogate replacement preserves UTF-8"))
}

fn unicode_escape_at(bytes: &[u8], index: usize) -> Option<u16> {
    let escape = bytes.get(index..index + 6)?;
    if escape[0] != b'\\' || escape[1] != b'u' {
        return None;
    }

    let mut value = 0_u16;
    for digit in &escape[2..] {
        value = value.checked_mul(16)? + hex_value(*digit)?;
    }
    Some(value)
}

fn hex_value(byte: u8) -> Option<u16> {
    match byte {
        b'0'..=b'9' => Some(u16::from(byte - b'0')),
        b'a'..=b'f' => Some(u16::from(byte - b'a' + 10)),
        b'A'..=b'F' => Some(u16::from(byte - b'A' + 10)),
        _ => None,
    }
}

fn is_high_surrogate(code_unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&code_unit)
}

fn is_low_surrogate(code_unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&code_unit)
}

fn skip_json_string(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;

    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }

    bytes.len()
}

fn is_json_number_character(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')
}

fn is_unsafe_integer_token(token: &str) -> bool {
    if token.contains(['.', 'e', 'E']) {
        return false;
    }

    let magnitude = token.strip_prefix('-').unwrap_or(token);
    const MAX_SAFE_INTEGER: &str = "9007199254740991";

    magnitude.len() > MAX_SAFE_INTEGER.len()
        || (magnitude.len() == MAX_SAFE_INTEGER.len() && magnitude > MAX_SAFE_INTEGER)
}

struct DuplicateKeyDetector {
    duplicate_key: Option<String>,
}

impl<'de> DeserializeSeed<'de> for &mut DuplicateKeyDetector {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for &mut DuplicateKeyDetector {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element_seed(&mut *self)?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = HashSet::new();

        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                self.duplicate_key.get_or_insert(key);
            }
            map.next_value_seed(&mut *self)?;
        }

        Ok(())
    }
}

pub(crate) fn canonical_json_sha256(value: &Value) -> Result<String, serde_json::Error> {
    let canonical_bytes = serde_jcs::to_vec(value)?;
    let digest = Sha256::digest(canonical_bytes);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    Ok(format!("sha256:{hex}"))
}
