//! Canonical upload bytes; no paths from this packet are ever opened on the server filesystem.
use super::store::FoundationError;
use base64::{Engine, engine::general_purpose::STANDARD};
use linggan_contracts::ai_limits::AI_LIMITS;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Utf8,
    Base64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UploadFile {
    pub path: String,
    pub encoding: Encoding,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillPackage {
    pub directory: String,
    pub files: Vec<UploadFile>,
}
#[derive(Debug)]
pub struct CanonicalFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub text: Option<String>,
    pub sha256: String,
}
#[derive(Debug)]
pub struct PreparedPackage {
    pub directory: String,
    pub files: Vec<CanonicalFile>,
    pub source_sha256: String,
    pub package_sha256: String,
    pub source_manifest: Vec<SourceFile>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}
fn normalized_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|p| {
            !p.is_empty() && !matches!(p, "." | "..") && !p.eq_ignore_ascii_case("scripts")
        })
}
fn digest_entries<'a>(
    directory: &str,
    entries: impl Iterator<Item = (&'a str, &'a [u8])>,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"ai-skill-package.v1\0");
    hash.update((directory.len() as u64).to_be_bytes());
    hash.update(directory.as_bytes());
    for (path, bytes) in entries {
        for value in [path.as_bytes(), bytes] {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value);
        }
    }
    hex(&hash.finalize())
}
impl SkillPackage {
    pub fn prepare(&self) -> Result<PreparedPackage, FoundationError> {
        let directory: String = self.directory.nfc().collect();
        // The official loader applies the same 64-character lowercase directory/name rule.
        if directory.is_empty()
            || directory.len() > 64
            || directory.starts_with('-')
            || directory.ends_with('-')
            || directory.contains("--")
            || !directory
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || self.files.is_empty()
            || self.files.len() > AI_LIMITS.max_skill_files as usize
        {
            return Err(FoundationError::Invalid);
        }
        let mut source = BTreeMap::new();
        let mut canonical = BTreeMap::new();
        let mut source_total = 0u64;
        let mut canonical_total = 0u64;
        for file in &self.files {
            let path: String = file.path.nfc().collect();
            if !valid_path(&path) || file.content.len() as u64 > AI_LIMITS.max_skill_file_bytes * 2
            {
                return Err(FoundationError::Invalid);
            }
            let bytes = match file.encoding {
                Encoding::Utf8 => file.content.as_bytes().to_vec(),
                Encoding::Base64 => STANDARD
                    .decode(&file.content)
                    .map_err(|_| FoundationError::Invalid)?,
            };
            source_total = source_total
                .checked_add(bytes.len() as u64)
                .ok_or(FoundationError::Invalid)?;
            if bytes.len() as u64 > AI_LIMITS.max_skill_file_bytes
                || source_total > AI_LIMITS.max_skill_total_bytes
            {
                return Err(FoundationError::Invalid);
            }
            let text = std::str::from_utf8(&bytes)
                .ok()
                .filter(|s| {
                    !s.chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                })
                .map(normalized_text);
            let normalized = text
                .as_ref()
                .map_or_else(|| bytes.clone(), |s| s.as_bytes().to_vec());
            canonical_total = canonical_total
                .checked_add(normalized.len() as u64)
                .ok_or(FoundationError::Invalid)?;
            if normalized.len() as u64 > AI_LIMITS.max_skill_file_bytes
                || canonical_total > AI_LIMITS.max_skill_total_bytes
            {
                return Err(FoundationError::Invalid);
            }
            let hash = hex(&Sha256::digest(&normalized));
            if source.insert(file.path.clone(), bytes).is_some()
                || canonical
                    .insert(
                        path.clone(),
                        CanonicalFile {
                            path,
                            bytes: normalized,
                            text,
                            sha256: hash,
                        },
                    )
                    .is_some()
            {
                return Err(FoundationError::Invalid);
            }
        }
        if canonical.get("SKILL.md").is_none_or(|f| f.text.is_none()) {
            return Err(FoundationError::Invalid);
        }
        let source_sha256 = digest_entries(
            &self.directory,
            source.iter().map(|(p, b)| (p.as_str(), b.as_slice())),
        );
        let package_sha256 = digest_entries(
            &directory,
            canonical
                .values()
                .map(|f| (f.path.as_str(), f.bytes.as_slice())),
        );
        let source_manifest = source
            .iter()
            .map(|(path, bytes)| SourceFile {
                path: path.clone(),
                sha256: hex(&Sha256::digest(bytes)),
                bytes: bytes.len(),
            })
            .collect();
        Ok(PreparedPackage {
            directory,
            files: canonical.into_values().collect(),
            source_sha256,
            package_sha256,
            source_manifest,
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillInspection {
    pub contract: String,
    pub name: String,
    pub description: String,
    pub body: String,
    pub frontmatter: String,
    pub disable_model_invocation: bool,
    pub invocation: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: &str, content: &str) -> UploadFile {
        UploadFile {
            path: path.into(),
            encoding: Encoding::Utf8,
            content: content.into(),
        }
    }
    #[test]
    fn newline_nfc_path_order_and_upload_encoding_share_one_canonical_hash_but_preserve_source() {
        let one = SkillPackage {
            directory: "example".into(),
            files: vec![
                file(
                    "SKILL.md",
                    "---\nname: example\ndescription: example\n---\nCafé\n",
                ),
                file("references/café.md", "Café\n"),
            ],
        };
        let mut two = one.clone();
        two.files.reverse();
        assert_eq!(
            one.prepare().unwrap().source_sha256,
            two.prepare().unwrap().source_sha256
        );
        two.files[0] = file("references/cafe\u{301}.md", "Cafe\u{301}\r\n");
        two.files[1].content = two.files[1].content.replace('\n', "\r\n");
        let a = one.prepare().unwrap();
        let b = two.prepare().unwrap();
        assert_eq!(a.package_sha256, b.package_sha256);
        assert_ne!(a.source_sha256, b.source_sha256);
        two.files[0].content = STANDARD.encode(two.files[0].content.as_bytes());
        two.files[0].encoding = Encoding::Base64;
        assert_eq!(b.source_sha256, two.prepare().unwrap().source_sha256);
    }
    #[test]
    fn duplicate_canonical_paths_traversal_scripts_and_nontext_manifests_are_rejected() {
        let mut package = SkillPackage {
            directory: "example".into(),
            files: vec![file("SKILL.md", "body")],
        };
        for path in [
            "../escape",
            "/absolute",
            "C:/escape",
            "scripts/run",
            "references/Scripts/run",
            "a\\b",
            "a//b",
        ] {
            package.files.push(file(path, "x"));
            assert!(package.prepare().is_err(), "{path}");
            package.files.pop();
        }
        package
            .files
            .extend([file("café.md", "1"), file("cafe\u{301}.md", "2")]);
        assert!(package.prepare().is_err());
        assert!(
            serde_json::from_value::<SkillPackage>(serde_json::json!({"directory":"example",
            "files":[{"path":"SKILL.md","encoding":"symlink","content":"/outside/secret"}]}))
            .is_err(),
            "the upload protocol accepts byte payloads, never filesystem links"
        );
        package.files.truncate(1);
        package.files[0].encoding = Encoding::Base64;
        package.files[0].content = STANDARD.encode([0xff, 0]);
        assert!(package.prepare().is_err());
    }
    #[test]
    fn normalization_and_aggregate_bounds_apply_before_loading_any_skill() {
        let mut package = SkillPackage {
            directory: "example".into(),
            files: vec![file("SKILL.md", "body")],
        };
        package
            .files
            .push(file("expansion.md", &"\u{0344}".repeat(131072)));
        assert!(
            package.prepare().is_err(),
            "NFC expansion cannot escape the per-file limit"
        );
        package.files.truncate(1);
        for index in 0..64 {
            package.files.push(file(&format!("file-{index}.md"), "x"));
        }
        assert!(
            package.prepare().is_err(),
            "the manifest counts toward the 64-file bound"
        );
        package.files.truncate(1);
        for index in 0..4 {
            package
                .files
                .push(file(&format!("file-{index}.md"), &"x".repeat(262144)));
        }
        assert!(
            package.prepare().is_err(),
            "individually bounded files can exceed the total package limit"
        );
    }
}
