//! Small deterministic text primitives shared by the V1 comment-research kernel.
//! They carry no legacy research state or database behaviour.

use sha2::{Digest, Sha256};

pub fn content_hash(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn valid_text(value: &str, maximum_chars: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum_chars
}
