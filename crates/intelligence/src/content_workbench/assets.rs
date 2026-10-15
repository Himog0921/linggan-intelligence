use super::*;
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
pub(crate) fn root() -> Result<PathBuf, WorkbenchError> {
    let root = std::env::var("LINGGAN_CREATION_ASSET_ROOT")
        .map(PathBuf::from)
        .map_err(|_| WorkbenchError::StorageNotConfigured)?;
    if !root.is_absolute() {
        return Err(WorkbenchError::StorageNotConfigured);
    }
    Ok(root)
}
pub(crate) fn checked_path(key: &str) -> Result<PathBuf, WorkbenchError> {
    if key.is_empty()
        || Path::new(key).is_absolute()
        || key
            .split('/')
            .any(|s| s.is_empty() || matches!(s, "." | ".."))
    {
        return Err(WorkbenchError::Invalid("storage_key"));
    }
    let root = fs::canonicalize(root()?)?;
    let file = fs::canonicalize(root.join(key))?;
    if !file.starts_with(root) {
        return Err(WorkbenchError::Invalid("storage_path"));
    }
    Ok(file)
}
pub(crate) fn verified_file(key: &str, sha: &str, size: i64) -> Result<PathBuf, WorkbenchError> {
    let path = checked_path(key)?;
    let meta = fs::metadata(&path)?;
    if !meta.is_file()
        || i64::try_from(meta.len()).ok() != Some(size)
        || size < 1
        || size > 150 * 1024 * 1024
    {
        return Err(WorkbenchError::Invalid("file_size"));
    }
    if hash(fs::read(&path)?) != sha {
        return Err(WorkbenchError::Invalid("file_hash"));
    }
    Ok(path)
}
pub(crate) fn persist(key: &str, bytes: &[u8]) -> Result<(), WorkbenchError> {
    if key
        .split('/')
        .any(|s| s.is_empty() || matches!(s, "." | ".."))
        || Path::new(key).is_absolute()
    {
        return Err(WorkbenchError::Invalid("storage_key"));
    }
    fs::create_dir_all(root()?)?;
    let base = fs::canonicalize(root()?)?;
    let dest = base.join(key);
    let parent = dest
        .parent()
        .ok_or(WorkbenchError::Invalid("storage_key"))?;
    fs::create_dir_all(parent)?;
    if !fs::canonicalize(parent)?.starts_with(&base) {
        return Err(WorkbenchError::Invalid("storage_path"));
    }
    if dest.exists() {
        let existing = checked_path(key)?;
        if hash(fs::read(existing)?) == hash(bytes) {
            return Ok(());
        }
        return Err(WorkbenchError::ExportConflict);
    }
    let temp = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    match fs::hard_link(&temp, &dest) {
        Ok(()) => {
            fs::remove_file(temp)?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(temp);
            if hash(fs::read(checked_path(key)?)?) == hash(bytes) {
                Ok(())
            } else {
                Err(WorkbenchError::ExportConflict)
            }
        }
        Err(e) => {
            let _ = fs::remove_file(temp);
            Err(e.into())
        }
    }
}
pub(crate) fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), WorkbenchError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(WorkbenchError::Invalid("image_size"));
    }
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|_| WorkbenchError::Invalid("png"))?;
    let size = reader
        .output_buffer_size()
        .ok_or(WorkbenchError::Invalid("png_size"))?;
    if size > 64 * 1024 * 1024 {
        return Err(WorkbenchError::Invalid("png_size"));
    }
    let mut buffer = vec![0; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|_| WorkbenchError::Invalid("png_integrity"))?;
    reader
        .finish()
        .map_err(|_| WorkbenchError::Invalid("png_integrity"))?;
    Ok((frame.width, frame.height))
}
fn inspect(bytes: &[u8]) -> Result<(&'static str, u32, u32), WorkbenchError> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(WorkbenchError::Invalid("image_size"));
    }
    let format = image::guess_format(bytes).map_err(|_| WorkbenchError::Invalid("image"))?;
    let mime = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(WorkbenchError::Invalid("image_type")),
    };
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(12000);
    limits.max_image_height = Some(12000);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| WorkbenchError::Invalid("image_decode"))?;
    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > 16_000_000 {
        return Err(WorkbenchError::Invalid("image_dimensions"));
    }
    if format == image::ImageFormat::Png {
        png_dimensions(bytes)?;
    }
    Ok((mime, w, h))
}
pub async fn upload_asset(
    db: &Database,
    work: Uuid,
    request: Uuid,
    bytes: &[u8],
) -> Result<AssetRecord, WorkbenchError> {
    let (mime, w, h) = inspect(bytes)?;
    let sha = hash(bytes);
    let mut tx = db.pool().begin().await?;
    super::requests::ensure_request_owner_in(
        &mut tx,
        request,
        super::requests::RequestOwner::Asset,
    )
    .await?;
    super::store::read_revision_in(&mut tx, work, None).await?;
    if let Some(row) = sqlx::query("SELECT * FROM linggan_creation_asset WHERE request_ref=$1")
        .bind(request)
        .fetch_optional(&mut *tx)
        .await?
    {
        let asset = from_row(&row)?;
        if asset.work_id != work || asset.sha256 != sha {
            return Err(WorkbenchError::RequestConflict);
        }
        verified_file(
            &row.try_get::<String, _>("storage_key")?,
            &sha,
            asset.byte_size,
        )?;
        tx.commit().await?;
        return Ok(asset);
    }
    let id = Uuid::new_v4();
    let key = format!("works/{work}/assets/{id}/{sha}");
    persist(&key, bytes)?;
    sqlx::query("INSERT INTO linggan_creation_asset(asset_ref,work_ref,request_ref,storage_key,sha256,byte_size,mime_type,width,height) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(id).bind(work).bind(request).bind(key).bind(&sha).bind(bytes.len() as i64).bind(mime).bind(w as i32).bind(h as i32).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(AssetRecord {
        id,
        work_id: work,
        sha256: sha,
        byte_size: bytes.len() as i64,
        mime_type: mime.into(),
        width: w,
        height: h,
    })
}
fn from_row(row: &sqlx::postgres::PgRow) -> Result<AssetRecord, WorkbenchError> {
    Ok(AssetRecord {
        id: row.try_get("asset_ref")?,
        work_id: row.try_get("work_ref")?,
        sha256: row.try_get("sha256")?,
        byte_size: row.try_get("byte_size")?,
        mime_type: row.try_get("mime_type")?,
        width: row.try_get::<i32, _>("width")? as u32,
        height: row.try_get::<i32, _>("height")? as u32,
    })
}
pub async fn asset_file(
    db: &Database,
    work: Uuid,
    id: Uuid,
) -> Result<(PathBuf, AssetRecord), WorkbenchError> {
    let row =
        sqlx::query("SELECT * FROM linggan_creation_asset WHERE asset_ref=$1 AND work_ref=$2")
            .bind(id)
            .bind(work)
            .fetch_optional(db.pool())
            .await?
            .ok_or(WorkbenchError::NotFound)?;
    let record = from_row(&row)?;
    let path = verified_file(
        &row.try_get::<String, _>("storage_key")?,
        &record.sha256,
        record.byte_size,
    )?;
    Ok((path, record))
}
pub(crate) async fn asset_in(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    id: Uuid,
) -> Result<AssetRecord, WorkbenchError> {
    let row =
        sqlx::query("SELECT * FROM linggan_creation_asset WHERE asset_ref=$1 AND work_ref=$2")
            .bind(id)
            .bind(work)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(WorkbenchError::NotFound)?;
    let r = from_row(&row)?;
    verified_file(
        &row.try_get::<String, _>("storage_key")?,
        &r.sha256,
        r.byte_size,
    )?;
    Ok(r)
}
