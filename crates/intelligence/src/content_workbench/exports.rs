use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
};
use uuid::Uuid;
use zip::{ZipWriter, write::SimpleFileOptions};
pub(crate) async fn exports_in(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
) -> Result<Vec<ExportRecord>, WorkbenchError> {
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT export_ref FROM linggan_creation_export WHERE work_ref=$1 ORDER BY created_at DESC LIMIT 50").bind(work).fetch_all(&mut **tx).await?;
    let mut exports = Vec::new();
    for id in ids {
        exports.push(read_in(tx, id).await?);
    }
    Ok(exports)
}
pub(super) async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<ExportRecord, WorkbenchError> {
    let row = sqlx::query("SELECT * FROM linggan_creation_export WHERE export_ref=$1")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(WorkbenchError::NotFound)?;
    let rows=sqlx::query("SELECT page_ref,ordinal,sha256,byte_size FROM linggan_creation_export_page WHERE export_ref=$1 ORDER BY ordinal").bind(id).fetch_all(&mut **tx).await?;
    Ok(ExportRecord {
        id,
        work_id: row.try_get("work_ref")?,
        revision_id: row.try_get("revision_ref")?,
        state: row.try_get("state")?,
        failure_code: row.try_get("failure_code")?,
        renderer_version: row.try_get("renderer_version")?,
        font_version: row.try_get("font_version")?,
        pages: rows
            .into_iter()
            .map(|r| {
                Ok(ExportPage {
                    page_id: r.try_get("page_ref")?,
                    ordinal: r.try_get::<i32, _>("ordinal")? as u32,
                    sha256: r.try_get("sha256")?,
                    byte_size: r.try_get("byte_size")?,
                })
            })
            .collect::<Result<_, sqlx::Error>>()?,
        package_sha256: row.try_get("package_sha256")?,
        package_byte_size: row.try_get("package_byte_size")?,
    })
}
pub async fn read_export(db: &Database, id: Uuid) -> Result<ExportRecord, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let r = read_in(&mut tx, id).await?;
    tx.commit().await?;
    Ok(r)
}
async fn command_replay(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    command: &str,
) -> Result<Option<ExportRecord>, WorkbenchError> {
    let row = sqlx::query(
        "SELECT command_sha256,receipt FROM linggan_creation_export_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else { return Ok(None) };
    if row.try_get::<String, _>("command_sha256")? != command {
        return Err(WorkbenchError::RequestConflict);
    }
    Ok(Some(serde_json::from_value(row.try_get("receipt")?)?))
}
async fn command_receipt(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    command: &str,
    out: &ExportRecord,
) -> Result<(), WorkbenchError> {
    sqlx::query("INSERT INTO linggan_creation_export_request(request_ref,export_ref,command_sha256,receipt) VALUES($1,$2,$3,$4)").bind(request).bind(out.id).bind(command).bind(serde_json::to_value(out)?).execute(&mut **tx).await?;
    Ok(())
}
pub async fn mark_export_failed(
    db: &Database,
    id: Uuid,
    r: &ExportFailureRequest,
) -> Result<ExportRecord, WorkbenchError> {
    if !matches!(
        r.code.as_str(),
        "render_failure" | "font_unavailable" | "asset_unavailable" | "overflow" | "upload_failure"
    ) {
        return Err(WorkbenchError::Invalid("failureCode"));
    }
    let command = hash(serde_json::to_vec(&json!(["fail", id, r]))?);
    let mut tx = db.pool().begin().await?;
    super::requests::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        super::requests::RequestOwner::ExportCommand,
    )
    .await?;
    if let Some(out) = command_replay(&mut tx, r.request_id, &command).await? {
        tx.commit().await?;
        return Ok(out);
    }
    super::requests::lock(&mut tx, id.to_string()).await?;
    let out = read_in(&mut tx, id).await?;
    if out.state == "complete" {
        return Err(WorkbenchError::ExportConflict);
    }
    sqlx::query(
        "UPDATE linggan_creation_export SET state='failed',failure_code=$2 WHERE export_ref=$1",
    )
    .bind(id)
    .bind(&r.code)
    .execute(&mut *tx)
    .await?;
    let out = read_in(&mut tx, id).await?;
    command_receipt(&mut tx, r.request_id, &command, &out).await?;
    tx.commit().await?;
    Ok(out)
}
pub async fn retry_export(
    db: &Database,
    id: Uuid,
    r: &FinalizeRequest,
) -> Result<(ExportRecord, Revision), WorkbenchError> {
    let command = hash(serde_json::to_vec(&json!(["retry", id, r]))?);
    let mut tx = db.pool().begin().await?;
    super::requests::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        super::requests::RequestOwner::ExportCommand,
    )
    .await?;
    if let Some(out) = command_replay(&mut tx, r.request_id, &command).await? {
        let revision = qualify(&mut tx, out.work_id, out.revision_id).await?;
        tx.commit().await?;
        return Ok((out, revision));
    }
    super::requests::lock(&mut tx, id.to_string()).await?;
    let out = read_in(&mut tx, id).await?;
    let revision = qualify(&mut tx, out.work_id, out.revision_id).await?;
    if out.state == "failed" {
        sqlx::query("UPDATE linggan_creation_export SET state='rendering',failure_code=NULL WHERE export_ref=$1").bind(id).execute(&mut *tx).await?;
    }
    let out = read_in(&mut tx, id).await?;
    command_receipt(&mut tx, r.request_id, &command, &out).await?;
    tx.commit().await?;
    Ok((out, revision))
}
pub(crate) async fn qualify(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    revision: Uuid,
) -> Result<Revision, WorkbenchError> {
    let (domain, r) = super::store::read_revision_in(tx, work, Some(revision)).await?;
    if r.document.account_ref.is_none() {
        return Err(WorkbenchError::Invalid("accountRef"));
    }
    let mut doc = r.document.clone();
    super::source_usage::prepare(tx, work, domain, Some(&r.document), &mut doc).await?;
    let qualification = crate::content_workbench_sources::qualify_citations_in_domain(
        tx,
        &r.document
            .excerpts
            .iter()
            .map(|e| e.citation.clone())
            .collect::<Vec<_>>(),
        domain,
    )
    .await
    .map_err(|_| WorkbenchError::Source)?;
    for (e, q) in r.document.excerpts.iter().zip(qualification) {
        if q.state != "readable" {
            let active = r.document.uses.iter().any(|u| {
                u.excerpt_id == Some(e.id)
                    && u.content_origin == ContentOrigin::Excerpt
                    && super::validation::target_text(&r.document, &u.target).is_some()
            });
            let raw = e.selected_text.as_deref().unwrap_or("");
            let exposed = r
                .document
                .tags
                .iter()
                .any(|tag| super::source_usage::raw_present(tag, raw))
                || super::source_usage::raw_present(&r.document.title, raw)
                || r.document
                    .paragraphs
                    .iter()
                    .any(|p| super::source_usage::raw_present(&p.text, raw))
                || r.document.pages.iter().any(|p| {
                    super::source_usage::raw_present(&p.title, raw)
                        || super::source_usage::raw_present(&p.subtitle, raw)
                        || p.blocks
                            .iter()
                            .any(|b| super::source_usage::raw_present(&b.text, raw))
                });
            if active || exposed {
                return Err(WorkbenchError::Source);
            }
        }
    }
    for p in &r.document.pages {
        if let Some(a) = p.asset_id {
            super::assets::asset_in(tx, work, a).await?;
        }
    }
    let mut public = r;
    super::source_usage::redact(tx, domain, &mut public.document).await?;
    Ok(public)
}
pub async fn freeze_export(
    db: &Database,
    work: Uuid,
    r: &FreezeRequest,
) -> Result<(ExportRecord, Revision), WorkbenchError> {
    if r.renderer_version != "creation-renderer.v1"
        || r.font_version.is_empty()
        || r.font_version.len() > 200
    {
        return Err(WorkbenchError::Invalid("renderer"));
    }
    let command = hash(serde_json::to_vec(&json!([work, r]))?);
    let mut tx = db.pool().begin().await?;
    super::requests::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        super::requests::RequestOwner::ExportFreeze,
    )
    .await?;
    super::requests::lock(&mut tx, work.to_string()).await?;
    if let Some(row) = sqlx::query(
        "SELECT export_ref,command_sha256 FROM linggan_creation_export WHERE request_ref=$1",
    )
    .bind(r.request_id)
    .fetch_optional(&mut *tx)
    .await?
    {
        if row.try_get::<String, _>("command_sha256")? != command {
            return Err(WorkbenchError::RequestConflict);
        }
        let id = row.try_get("export_ref")?;
        let out = read_in(&mut tx, id).await?;
        let rev = qualify(&mut tx, work, out.revision_id).await?;
        tx.commit().await?;
        return Ok((out, rev));
    }
    let (_, current) = super::store::read_revision_in(&mut tx, work, None).await?;
    if current.id != r.expected_revision {
        return Err(WorkbenchError::ExportConflict);
    }
    let revision = qualify(&mut tx, work, current.id).await?;
    if revision.document.pages.is_empty()
        || revision
            .document
            .pages
            .first()
            .is_none_or(|p| p.kind != PageKind::Cover)
    {
        return Err(WorkbenchError::Invalid("export_pages"));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_creation_export(export_ref,work_ref,revision_ref,request_ref,command_sha256,renderer_version,font_version,state) VALUES($1,$2,$3,$4,$5,$6,$7,'rendering')").bind(id).bind(work).bind(revision.id).bind(r.request_id).bind(command).bind(&r.renderer_version).bind(&r.font_version).execute(&mut *tx).await?;
    for (index, p) in revision.document.pages.iter().enumerate() {
        sqlx::query("INSERT INTO linggan_creation_export_page(export_ref,page_ref,ordinal) VALUES($1,$2,$3)").bind(id).bind(p.id).bind(index as i32+1).execute(&mut *tx).await?;
    }
    let out = read_in(&mut tx, id).await?;
    let manifest = json!({"contract":"creation-export.v1","export":&out,"revisionId":revision.id,"documentHash":hash(serde_json::to_vec(&revision.document)?),"assetHashes":asset_manifest(&mut tx,work,&revision.document).await?});
    super::assets::persist(
        &format!("works/{work}/exports/{id}/manifest.json"),
        &serde_json::to_vec(&manifest)?,
    )?;
    tx.commit().await?;
    Ok((out, revision))
}
async fn asset_manifest(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    doc: &WorkDocument,
) -> Result<Vec<AssetRecord>, WorkbenchError> {
    let mut assets = Vec::new();
    for p in &doc.pages {
        if let Some(id) = p.asset_id {
            assets.push(super::assets::asset_in(tx, work, id).await?);
        }
    }
    Ok(assets)
}
pub async fn upload_export_page(
    db: &Database,
    id: Uuid,
    page: Uuid,
    bytes: &[u8],
) -> Result<ExportRecord, WorkbenchError> {
    if super::assets::png_dimensions(bytes)? != (1080, 1440) {
        return Err(WorkbenchError::Invalid("png_dimensions"));
    }
    let sha = hash(bytes);
    let mut tx = db.pool().begin().await?;
    super::requests::lock(&mut tx, id.to_string()).await?;
    let out = read_in(&mut tx, id).await?;
    qualify(&mut tx, out.work_id, out.revision_id).await?;
    let row=sqlx::query("SELECT sha256,storage_key,byte_size FROM linggan_creation_export_page WHERE export_ref=$1 AND page_ref=$2").bind(id).bind(page).fetch_optional(&mut *tx).await?.ok_or(WorkbenchError::Invalid("pageId"))?;
    if let Some(existing) = row.try_get::<Option<String>, _>("sha256")? {
        if sha != existing {
            return Err(WorkbenchError::ExportConflict);
        }
        super::assets::verified_file(
            &row.try_get::<String, _>("storage_key")?,
            &sha,
            row.try_get("byte_size")?,
        )?;
        tx.commit().await?;
        return Ok(out);
    }
    if out.state != "rendering" {
        return Err(WorkbenchError::ExportConflict);
    }
    let key = format!("works/{}/exports/{id}/pages/{page}/{sha}.png", out.work_id);
    super::assets::persist(&key, bytes)?;
    sqlx::query("UPDATE linggan_creation_export_page SET sha256=$3,storage_key=$4,byte_size=$5 WHERE export_ref=$1 AND page_ref=$2 AND sha256 IS NULL").bind(id).bind(page).bind(sha).bind(key).bind(bytes.len() as i64).execute(&mut *tx).await?;
    let out = read_in(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}
pub async fn finalize_export(
    db: &Database,
    id: Uuid,
    r: &FinalizeRequest,
) -> Result<ExportRecord, WorkbenchError> {
    let command = hash(serde_json::to_vec(&json!(["finalize", id, r]))?);
    let mut tx = db.pool().begin().await?;
    super::requests::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        super::requests::RequestOwner::ExportCommand,
    )
    .await?;
    if let Some(out) = command_replay(&mut tx, r.request_id, &command).await? {
        qualify(&mut tx, out.work_id, out.revision_id).await?;
        package_in(&mut tx, &out).await?;
        tx.commit().await?;
        return Ok(out);
    }
    super::requests::lock(&mut tx, id.to_string()).await?;
    let out = read_in(&mut tx, id).await?;
    let revision = qualify(&mut tx, out.work_id, out.revision_id).await?;
    if out.state == "complete" {
        package_in(&mut tx, &out).await?;
        command_receipt(&mut tx, r.request_id, &command, &out).await?;
        tx.commit().await?;
        return Ok(out);
    }
    if out.state == "failed" {
        return Err(WorkbenchError::ExportConflict);
    }
    if out.pages.is_empty() || out.pages.iter().any(|p| p.sha256.is_none()) {
        return Err(WorkbenchError::Incomplete);
    }
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("正文.txt", options)
        .map_err(|_| WorkbenchError::Invalid("zip"))?;
    let text = format!(
        "{}\n\n{}",
        revision.document.title,
        revision
            .document
            .paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    let text = if revision.document.tags.is_empty() {
        text
    } else {
        format!(
            "{text}\n\n{}",
            revision
                .document
                .tags
                .iter()
                .map(|tag| {
                    if tag.starts_with('#') {
                        tag.clone()
                    } else {
                        format!("#{tag}")
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    zip.write_all(text.as_bytes())?;
    for p in &out.pages {
        let path = page_in(&mut tx, &out, p.page_id).await?;
        let bytes = fs::read(path)?;
        if super::assets::png_dimensions(&bytes)? != (1080, 1440) {
            return Err(WorkbenchError::Invalid("png_dimensions"));
        }
        zip.start_file(
            format!(
                "{:02}-{}.png",
                p.ordinal,
                if p.ordinal == 1 { "封面" } else { "内页" }
            ),
            options,
        )
        .map_err(|_| WorkbenchError::Invalid("zip"))?;
        zip.write_all(&bytes)?;
    }
    let bytes = zip
        .finish()
        .map_err(|_| WorkbenchError::Invalid("zip"))?
        .into_inner();
    if bytes.len() > 150 * 1024 * 1024 {
        return Err(WorkbenchError::Invalid("zip_size"));
    }
    let sha = hash(&bytes);
    let key = format!("works/{}/exports/{id}/package/{sha}.zip", out.work_id);
    super::assets::persist(&key, &bytes)?;
    sqlx::query("UPDATE linggan_creation_export SET state='complete',package_storage_key=$2,package_sha256=$3,package_byte_size=$4 WHERE export_ref=$1 AND state='rendering'").bind(id).bind(key).bind(sha).bind(bytes.len() as i64).execute(&mut *tx).await?;
    let out = read_in(&mut tx, id).await?;
    command_receipt(&mut tx, r.request_id, &command, &out).await?;
    tx.commit().await?;
    Ok(out)
}
async fn package_in(
    tx: &mut Transaction<'_, Postgres>,
    out: &ExportRecord,
) -> Result<PathBuf, WorkbenchError> {
    if out.state != "complete" {
        return Err(WorkbenchError::Incomplete);
    }
    let key: String = sqlx::query_scalar(
        "SELECT package_storage_key FROM linggan_creation_export WHERE export_ref=$1",
    )
    .bind(out.id)
    .fetch_one(&mut **tx)
    .await?;
    super::assets::verified_file(
        &key,
        out.package_sha256
            .as_deref()
            .ok_or(WorkbenchError::Incomplete)?,
        out.package_byte_size.ok_or(WorkbenchError::Incomplete)?,
    )
}
async fn page_in(
    tx: &mut Transaction<'_, Postgres>,
    out: &ExportRecord,
    page: Uuid,
) -> Result<PathBuf, WorkbenchError> {
    let row=sqlx::query("SELECT storage_key,sha256,byte_size FROM linggan_creation_export_page WHERE export_ref=$1 AND page_ref=$2 AND sha256 IS NOT NULL").bind(out.id).bind(page).fetch_optional(&mut **tx).await?.ok_or(WorkbenchError::Incomplete)?;
    super::assets::verified_file(
        &row.try_get::<String, _>("storage_key")?,
        &row.try_get::<String, _>("sha256")?,
        row.try_get("byte_size")?,
    )
}
pub async fn export_file(db: &Database, id: Uuid) -> Result<PathBuf, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let out = read_in(&mut tx, id).await?;
    qualify(&mut tx, out.work_id, out.revision_id).await?;
    let path = package_in(&mut tx, &out).await?;
    tx.commit().await?;
    Ok(path)
}
pub async fn export_page_file(
    db: &Database,
    id: Uuid,
    page: Uuid,
) -> Result<PathBuf, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let out = read_in(&mut tx, id).await?;
    qualify(&mut tx, out.work_id, out.revision_id).await?;
    let path = page_in(&mut tx, &out, page).await?;
    tx.commit().await?;
    Ok(path)
}
