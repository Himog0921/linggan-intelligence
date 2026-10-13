//! OPS-MIGRATION-001 D2: historical publication covers that Mog copied from the content
//! workbench machine. Only files named by the workbench export are read; each is checked to be a
//! real image, stored content-addressed under the local media root and registered to its
//! publication. Preview first, then apply with the preview hash (as for the ledger import).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use linggan_storage_postgres::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

use super::OpsImportError;
use super::export_rows::{ExportFile, PUBLICATION_KEYS, PublicationExportRow, parse_rows};

pub const OPS_COVER_RULE_VERSION: &str = "ops-cover.v1";
const LOCAL_MEDIA_PREFIX: &str = "/api/local-media/";
const MAX_COVER_BYTES: u64 = 20 * 1024 * 1024;

pub struct CoverImportRequest {
    /// The same `data-management-publications.json` the ledger was imported from.
    pub publications_export: Vec<u8>,
    /// Folder holding the copied `workspaces/<id>/media/...` tree.
    pub covers_dir: PathBuf,
    /// Linggan's local media root (`LINGGAN_LOCAL_MEDIA_ROOT`).
    pub media_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedCover {
    publication_ref: Uuid,
    source_path: String,
    sha256: String,
    byte_size: i64,
    mime_type: &'static str,
}

impl PlannedCover {
    fn storage_key(&self) -> String {
        format!("ops-covers/{}.{}", self.sha256, extension(self.mime_type))
    }
}

/// Counts only; no names, titles or paths.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverFigures {
    pub covers_in_export: usize,
    pub not_in_ledger: usize,
    pub missing_files: usize,
    pub not_an_image: usize,
    pub already_registered: usize,
    pub conflicting: usize,
    pub to_register: usize,
    pub webp: usize,
    pub jpeg: usize,
    pub png: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverPreview {
    pub preview_hash: String,
    pub rule_version: &'static str,
    pub figures: CoverFigures,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverReceipt {
    pub import_ref: Option<Uuid>,
    pub registered: usize,
    pub files_written: usize,
    pub files_already_present: usize,
    pub figures: CoverFigures,
}

pub async fn preview_cover_import(
    database: &Database,
    request: &CoverImportRequest,
) -> Result<CoverPreview, OpsImportError> {
    let (plan, figures) = plan_covers(database, request).await?;
    Ok(CoverPreview {
        preview_hash: plan_hash(&plan),
        rule_version: OPS_COVER_RULE_VERSION,
        figures,
    })
}

/// Files are written before the rows: a failed transaction leaves only content-addressed files
/// that the next apply reuses, never a row pointing at a missing file.
pub async fn apply_cover_import(
    database: &Database,
    request: &CoverImportRequest,
    expected_preview_hash: &str,
) -> Result<CoverReceipt, OpsImportError> {
    let (plan, figures) = plan_covers(database, request).await?;
    if plan_hash(&plan) != expected_preview_hash {
        return Err(OpsImportError::PreviewHashMismatch);
    }
    if plan.is_empty() {
        return Ok(CoverReceipt {
            import_ref: None,
            registered: 0,
            files_written: 0,
            files_already_present: 0,
            figures,
        });
    }
    let (mut files_written, mut files_already_present) = (0, 0);
    for cover in &plan {
        let written = store_file(request, cover)?;
        if written {
            files_written += 1
        } else {
            files_already_present += 1
        }
    }
    let import_ref = Uuid::new_v4();
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('linggan_ops_cover_import', 0))")
        .execute(&mut *transaction)
        .await?;
    let mut registered = 0;
    for cover in &plan {
        registered += sqlx::query(
            "INSERT INTO linggan_ops_publication_cover \
               (publication_ref, source_path, content_sha256, byte_size, mime_type, storage_key, import_ref) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (publication_ref) DO NOTHING",
        )
        .bind(cover.publication_ref)
        .bind(&cover.source_path)
        .bind(&cover.sha256)
        .bind(cover.byte_size)
        .bind(cover.mime_type)
        .bind(cover.storage_key())
        .bind(import_ref)
        .execute(&mut *transaction)
        .await?
        .rows_affected() as usize;
    }
    transaction.commit().await?;
    Ok(CoverReceipt {
        import_ref: Some(import_ref),
        registered,
        files_written,
        files_already_present,
        figures,
    })
}

async fn plan_covers(
    database: &Database,
    request: &CoverImportRequest,
) -> Result<(Vec<PlannedCover>, CoverFigures), OpsImportError> {
    let rows = parse_rows::<PublicationExportRow>(
        ExportFile::Publications,
        &request.publications_export,
        &PUBLICATION_KEYS,
    )?;
    let ledger = ledger_refs(database).await?;
    let mut figures = CoverFigures::default();
    let mut plan = Vec::new();
    for (index, parsed) in rows.iter().enumerate() {
        let Some(url) = cover_url(&parsed.raw) else {
            continue;
        };
        figures.covers_in_export += 1;
        let source_path = relative_media_path(url).ok_or_else(|| {
            OpsImportError::invalid(
                ExportFile::Publications,
                Some(index + 1),
                "media.cover.url",
                "封面地址不是工作台本机媒体路径",
            )
        })?;
        let Some((publication_ref, existing)) = ledger.get(&parsed.row.id) else {
            figures.not_in_ledger += 1;
            continue;
        };
        let Some(file) = read_cover(&request.covers_dir, &source_path)? else {
            figures.missing_files += 1;
            continue;
        };
        let Some(mime_type) = image_type(&file) else {
            figures.not_an_image += 1;
            continue;
        };
        let sha256 = Sha256::digest(&file)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        match existing {
            Some(stored) if *stored == sha256 => figures.already_registered += 1,
            Some(_) => figures.conflicting += 1,
            None => {
                count_type(&mut figures, mime_type);
                plan.push(PlannedCover {
                    publication_ref: *publication_ref,
                    source_path,
                    sha256,
                    byte_size: i64::try_from(file.len()).unwrap_or(i64::MAX),
                    mime_type,
                });
            }
        }
    }
    figures.to_register = plan.len();
    plan.sort_by_key(|cover| cover.publication_ref);
    Ok((plan, figures))
}

/// Publication registrations by original record id, with the hash of a cover already registered.
async fn ledger_refs(
    database: &Database,
) -> Result<HashMap<String, (Uuid, Option<String>)>, OpsImportError> {
    let rows = sqlx::query(
        "SELECT p.source_record_id, p.publication_ref, c.content_sha256 \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_publication_cover c ON c.publication_ref = p.publication_ref \
          WHERE p.source_system = 'content_workbench'",
    )
    .fetch_all(database.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get("source_record_id")?,
                (
                    row.try_get("publication_ref")?,
                    row.try_get("content_sha256")?,
                ),
            ))
        })
        .collect()
}

fn cover_url(raw: &serde_json::Map<String, serde_json::Value>) -> Option<&str> {
    raw.get("media")?.get("cover")?.get("url")?.as_str()
}

/// `/api/local-media/<path>` → the decoded relative path, refusing anything that could leave
/// the copied folder.
fn relative_media_path(url: &str) -> Option<String> {
    let encoded = url.strip_prefix(LOCAL_MEDIA_PREFIX)?;
    let decoded = percent_decode(encoded)?;
    let safe = !decoded.is_empty()
        && decoded.len() <= 1024
        && !decoded.contains(['\\', '\0'])
        && decoded
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    safe.then_some(decoded)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            output.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

/// Reads a regular file under `covers_dir`. Symbolic links and oversize files are refused.
fn read_cover(covers_dir: &Path, relative: &str) -> Result<Option<Vec<u8>>, OpsImportError> {
    let path = covers_dir.join(relative);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(OpsImportError::CoverFileUnreadable),
    };
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_COVER_BYTES {
        return Ok(None);
    }
    std::fs::read(&path)
        .map(Some)
        .map_err(|_| OpsImportError::CoverFileUnreadable)
}

/// The file's own bytes decide its type; the extension in the path does not.
fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else {
        None
    }
}

fn extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/webp" => "webp",
        "image/jpeg" => "jpg",
        _ => "png",
    }
}

fn count_type(figures: &mut CoverFigures, mime_type: &str) {
    match mime_type {
        "image/webp" => figures.webp += 1,
        "image/jpeg" => figures.jpeg += 1,
        _ => figures.png += 1,
    }
}

fn plan_hash(plan: &[PlannedCover]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(OPS_COVER_RULE_VERSION.as_bytes());
    for cover in plan {
        hasher.update(
            format!(
                "\n{}|{}|{}|{}",
                cover.publication_ref, cover.sha256, cover.mime_type, cover.byte_size
            )
            .as_bytes(),
        );
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Copies one cover into `<media root>/ops-covers/` unless an identical file is already there.
/// Returns whether a file was written.
fn store_file(request: &CoverImportRequest, cover: &PlannedCover) -> Result<bool, OpsImportError> {
    let target = request.media_root.join(cover.storage_key());
    if let Ok(existing) = std::fs::read(&target) {
        let same = Sha256::digest(&existing)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            == cover.sha256;
        return if same {
            Ok(false)
        } else {
            Err(OpsImportError::CoverStorageConflict)
        };
    }
    let bytes = read_cover(&request.covers_dir, &cover.source_path)?
        .ok_or(OpsImportError::CoverFileUnreadable)?;
    let directory = target
        .parent()
        .ok_or(OpsImportError::CoverStorageConflict)?;
    std::fs::create_dir_all(directory).map_err(|_| OpsImportError::CoverStorageUnavailable)?;
    let temporary = directory.join(format!(".{}.{}.partial", cover.sha256, Uuid::new_v4()));
    std::fs::write(&temporary, &bytes).map_err(|_| OpsImportError::CoverStorageUnavailable)?;
    std::fs::rename(&temporary, &target).map_err(|_| {
        let _ = std::fs::remove_file(&temporary);
        OpsImportError::CoverStorageUnavailable
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{image_type, relative_media_path};

    #[test]
    fn only_workbench_media_paths_inside_the_folder_are_accepted() {
        assert_eq!(
            relative_media_path("/api/local-media/workspaces/w1/media/covers/a%20b.webp")
                .as_deref(),
            Some("workspaces/w1/media/covers/a b.webp")
        );
        for refused in [
            "/api/local-media/../etc/passwd",
            "/api/local-media/workspaces/../../x.webp",
            "/api/local-media/workspaces//x.webp",
            "/api/local-media/%2E%2E/x.webp",
            "/api/local-media/a%5Cb.webp",
            "https://example.com/x.webp",
            "/api/local-media/",
            "/api/local-media/%ZZ.webp",
        ] {
            assert!(relative_media_path(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn the_bytes_decide_the_image_type() {
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(
            image_type(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]),
            Some("image/png")
        );
        assert_eq!(image_type(b"<svg onload=alert(1)>"), None);
        assert_eq!(image_type(b"RIFF\0\0\0\0WAVE"), None);
    }
}
