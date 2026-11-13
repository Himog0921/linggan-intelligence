//! Commenter HMAC digests (D-013). The key is read from the same support directory as model
//! secrets; this module never logs an author identifier.

use crate::model_secrets::{private_dir, private_file, support_dir};
use linggan_evidence::hmac_sha256_hex;
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

pub const COMMENTER_DIGEST_VERSION: &str = "commenter-hmac-sha256-v1";
const MIN_KEY_BYTES: usize = 32;
const KEY_DIRECTORY: &str = "opportunity";
const KEY_FILE: &str = "commenter-digest.key";

#[derive(Debug)]
pub enum CommenterDigestError {
    SupportDirUnset,
    SupportDirUnavailable { path: PathBuf },
    KeyUnavailable { path: PathBuf },
    KeyTooShort { path: Option<PathBuf>, bytes: usize },
}

impl fmt::Display for CommenterDigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SupportDirUnset => write!(
                formatter,
                "commenter digest support directory is not configured"
            ),
            Self::SupportDirUnavailable { path } => write!(
                formatter,
                "commenter digest support directory is unavailable or not private: {}",
                path.display()
            ),
            Self::KeyUnavailable { path } => write!(
                formatter,
                "commenter digest key is unavailable or not private: {}",
                path.display()
            ),
            Self::KeyTooShort { path, bytes } => match path {
                Some(path) => write!(
                    formatter,
                    "commenter digest key has {bytes} bytes, minimum is {MIN_KEY_BYTES}: {}",
                    path.display()
                ),
                None => write!(
                    formatter,
                    "commenter digest key has {bytes} bytes, minimum is {MIN_KEY_BYTES}"
                ),
            },
        }
    }
}

impl std::error::Error for CommenterDigestError {}

pub struct CommenterDigest {
    key: Vec<u8>,
}

impl CommenterDigest {
    pub fn load() -> Result<Self, CommenterDigestError> {
        let support = support_dir().ok_or(CommenterDigestError::SupportDirUnset)?;
        Self::load_from_support_dir(&support)
    }

    pub fn load_from_support_dir(support: &Path) -> Result<Self, CommenterDigestError> {
        let key_path = support.join(KEY_DIRECTORY).join(KEY_FILE);
        private_dir(support).map_err(|_| CommenterDigestError::SupportDirUnavailable {
            path: support.to_path_buf(),
        })?;
        private_dir(key_path.parent().unwrap_or(support)).map_err(|_| {
            CommenterDigestError::KeyUnavailable {
                path: key_path.clone(),
            }
        })?;
        private_file(&key_path).map_err(|_| CommenterDigestError::KeyUnavailable {
            path: key_path.clone(),
        })?;
        let key = fs::read(&key_path).map_err(|_| CommenterDigestError::KeyUnavailable {
            path: key_path.clone(),
        })?;
        Self::from_key_at(key, Some(key_path))
    }

    pub fn from_key(key: Vec<u8>) -> Result<Self, CommenterDigestError> {
        Self::from_key_at(key, None)
    }

    fn from_key_at(key: Vec<u8>, path: Option<PathBuf>) -> Result<Self, CommenterDigestError> {
        if key.len() < MIN_KEY_BYTES {
            return Err(CommenterDigestError::KeyTooShort {
                path,
                bytes: key.len(),
            });
        }
        Ok(Self { key })
    }

    pub fn hash(&self, author_external_id: Option<&str>) -> Option<String> {
        author_external_id.map(|author| hmac_sha256_hex(&self.key, author.as_bytes()))
    }

    pub fn digest_version(&self) -> &'static str {
        COMMENTER_DIGEST_VERSION
    }

    pub(super) fn key_verifier(&self) -> String {
        hmac_sha256_hex(&self.key, b"opportunity-commenter-key-verifier-v1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{DirBuilderExt, OpenOptionsExt},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_support() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "linggan-commenter-digest-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path.join(KEY_DIRECTORY))
            .unwrap();
        path
    }

    fn write_key(support: &Path, key: &[u8]) {
        let path = support.join(KEY_DIRECTORY).join(KEY_FILE);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        use std::io::Write;
        file.write_all(key).unwrap();
    }

    #[test]
    fn same_identifier_and_key_produce_the_same_digest() {
        let digest = CommenterDigest::from_key(vec![b'k'; 32]).unwrap();
        assert_eq!(
            digest.hash(Some("commenter-1")),
            digest.hash(Some("commenter-1"))
        );
        assert_eq!(digest.digest_version(), "commenter-hmac-sha256-v1");
    }

    #[test]
    fn different_keys_produce_different_digests() {
        let first = CommenterDigest::from_key(vec![b'a'; 32]).unwrap();
        let second = CommenterDigest::from_key(vec![b'b'; 32]).unwrap();
        assert_ne!(
            first.hash(Some("commenter-1")),
            second.hash(Some("commenter-1"))
        );
    }

    #[test]
    fn unknown_identifier_stays_null() {
        let digest = CommenterDigest::from_key(vec![b'k'; 32]).unwrap();
        assert_eq!(digest.hash(None), None);
    }

    fn short_key_error() -> CommenterDigestError {
        match CommenterDigest::from_key(vec![b'k'; 31]) {
            Ok(_) => panic!("short key must be refused"),
            Err(error) => error,
        }
    }

    #[test]
    fn short_keys_are_refused() {
        let error = short_key_error();
        assert!(matches!(
            error,
            CommenterDigestError::KeyTooShort { bytes: 31, .. }
        ));
        assert!(error.to_string().contains("minimum is 32"));
    }

    fn missing_key_error(support: &Path) -> CommenterDigestError {
        match CommenterDigest::load_from_support_dir(support) {
            Ok(_) => panic!("missing key must be refused"),
            Err(error) => error,
        }
    }

    #[test]
    fn missing_key_is_refused_with_the_resolved_path() {
        let support = temp_support();
        let error = missing_key_error(&support);
        let expected = support
            .join(KEY_DIRECTORY)
            .join(KEY_FILE)
            .display()
            .to_string();
        assert!(error.to_string().contains(&expected));
        let _ = fs::remove_dir_all(&support);
    }

    #[test]
    fn key_loads_from_the_private_support_directory() {
        let support = temp_support();
        write_key(&support, &[b'z'; 32]);
        let digest = CommenterDigest::load_from_support_dir(&support).unwrap();
        assert_eq!(
            digest.hash(Some("commenter-1")),
            CommenterDigest::from_key(vec![b'z'; 32])
                .unwrap()
                .hash(Some("commenter-1"))
        );
        let _ = fs::remove_dir_all(&support);
    }

    #[test]
    fn raw_identifier_never_appears_in_hash_or_error_text() {
        let digest = CommenterDigest::from_key(vec![b'k'; 32]).unwrap();
        let hash = digest.hash(Some("raw-platform-id-123")).unwrap();
        assert!(!hash.contains("raw-platform-id-123"));
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|value| value.is_ascii_hexdigit()));
        assert!(!digest.digest_version().contains("raw-platform-id-123"));
        let error = short_key_error();
        assert!(!error.to_string().contains("raw-platform-id-123"));
        assert!(!format!("{error:?}").contains("raw-platform-id-123"));
    }

    // The module has no log facility, so this asserts the stronger condition that no output
    // statement exists at all and that every produced value stays free of the raw identifier.
    #[test]
    fn module_emits_no_log_statement_and_no_output_carries_the_raw_identifier() {
        let source = include_str!("commenter_hash.rs");
        for forbidden in [
            concat!("print", "ln!"),
            concat!("eprint", "ln!"),
            concat!("log", "::"),
            concat!("tracing", "::"),
            concat!("dbg", "!"),
        ] {
            assert!(!source.contains(forbidden), "unexpected output call");
        }
    }
}
