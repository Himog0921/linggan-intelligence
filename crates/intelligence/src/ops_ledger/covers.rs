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

/// v2: a link anywhere on the way to a file is refused, and every file is re-checked against the
/// preview right before it is stored.
pub const OPS_COVER_RULE_VERSION: &str = "ops-cover.v2";
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
    /// A link anywhere on the path, not a regular file, empty, or over 20 MiB.
    pub refused_files: usize,
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
        let file = match read_cover(&request.covers_dir, &source_path)? {
            CoverSource::Bytes(file) => file,
            CoverSource::Missing => {
                figures.missing_files += 1;
                continue;
            }
            CoverSource::Refused => {
                figures.refused_files += 1;
                continue;
            }
        };
        let Some(mime_type) = image_type(&file) else {
            figures.not_an_image += 1;
            continue;
        };
        let sha256 = sha256_hex(&file);
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

#[derive(Debug, PartialEq, Eq)]
enum CoverSource {
    Missing,
    Refused,
    Bytes(Vec<u8>),
}

/// Reads a regular file under `covers_dir`. A link on any step of the way — a folder or the file
/// itself — is refused, so a copied tree cannot reach outside the folder the operator chose; so
/// are empty and oversize files.
fn read_cover(covers_dir: &Path, relative: &str) -> Result<CoverSource, OpsImportError> {
    let mut path = covers_dir.to_path_buf();
    let mut metadata = None;
    for segment in relative.split('/') {
        path.push(segment);
        let step = match std::fs::symlink_metadata(&path) {
            Ok(step) => step,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CoverSource::Missing);
            }
            Err(_) => return Err(OpsImportError::CoverFileUnreadable),
        };
        if step.file_type().is_symlink() {
            return Ok(CoverSource::Refused);
        }
        metadata = Some(step);
    }
    let Some(metadata) = metadata else {
        return Ok(CoverSource::Missing);
    };
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_COVER_BYTES {
        return Ok(CoverSource::Refused);
    }
    std::fs::read(&path)
        .map(CoverSource::Bytes)
        .map_err(|_| OpsImportError::CoverFileUnreadable)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
        return if sha256_hex(&existing) == cover.sha256 {
            Ok(false)
        } else {
            Err(OpsImportError::CoverStorageConflict)
        };
    }
    // The file was hashed while planning and is read again here; only the same bytes may be
    // stored under the planned name and registered with the planned size and type.
    let CoverSource::Bytes(bytes) = read_cover(&request.covers_dir, &cover.source_path)? else {
        return Err(OpsImportError::CoverFileChanged);
    };
    if i64::try_from(bytes.len()).ok() != Some(cover.byte_size)
        || image_type(&bytes) != Some(cover.mime_type)
        || sha256_hex(&bytes) != cover.sha256
    {
        return Err(OpsImportError::CoverFileChanged);
    }
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
mod tests;
