//! Cross-command request ownership, immutable receipts and read-only reconciliation.
use super::source_usage::redact;
use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(crate) async fn lock(
    tx: &mut Transaction<'_, Postgres>,
    key: impl AsRef<str>,
) -> Result<(), WorkbenchError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation:{}", key.as_ref()))
        .execute(&mut **tx)
        .await?;
    Ok(())
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum RequestOwner {
    Work,
    Asset,
    ExportFreeze,
    ExportCommand,
    AiExecution,
}
impl RequestOwner {
    fn name(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Asset => "asset",
            Self::ExportFreeze => "export_freeze",
            Self::ExportCommand => "export_command",
            Self::AiExecution => "ai_execution",
        }
    }
}
async fn request_owners(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Vec<String>, WorkbenchError> {
    Ok(sqlx::query_scalar("SELECT 'work' FROM linggan_creation_request WHERE request_ref=$1 UNION ALL SELECT 'asset' FROM linggan_creation_asset WHERE request_ref=$1 UNION ALL SELECT 'export_freeze' FROM linggan_creation_export WHERE request_ref=$1 UNION ALL SELECT 'export_command' FROM linggan_creation_export_request WHERE request_ref=$1 UNION ALL SELECT 'ai_execution' FROM linggan_creation_ai_request WHERE request_ref=$1")
        .bind(request).fetch_all(&mut **tx).await?)
}
/// Every creation command uses the same UUID lock, including AI and binary uploads.
pub(crate) async fn ensure_request_owner_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    owner: RequestOwner,
) -> Result<(), WorkbenchError> {
    lock(tx, request.to_string()).await?;
    if request_owners(tx, request)
        .await?
        .iter()
        .any(|previous| previous != owner.name())
    {
        return Err(WorkbenchError::RequestConflict);
    }
    Ok(())
}
pub(crate) async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    command: &str,
) -> Result<Option<SaveReceipt>, WorkbenchError> {
    let row = sqlx::query(
        "SELECT command_sha256,receipt FROM linggan_creation_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(row) = row else { return Ok(None) };
    if row.try_get::<String, _>("command_sha256")? != command {
        return Err(WorkbenchError::RequestConflict);
    }
    let mut r: SaveReceipt = serde_json::from_value(row.try_get("receipt")?)?;
    r.replayed = true;
    Ok(Some(r))
}
pub(super) async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    request: Uuid,
    command: &str,
    r: SaveReceipt,
) -> Result<SaveReceipt, WorkbenchError> {
    sqlx::query("INSERT INTO linggan_creation_request(request_ref,work_ref,command_sha256,receipt) VALUES($1,$2,$3,$4)").bind(request).bind(work).bind(command).bind(serde_json::to_value(&r)?).execute(&mut **tx).await?;
    Ok(r)
}
/// Narrow reconciliation for a command whose response was lost. No execution or retry occurs.
pub async fn read_request(
    db: &Database,
    request: Uuid,
) -> Result<serde_json::Value, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    lock(&mut tx, request.to_string()).await?;
    if request_owners(&mut tx, request).await?.len() > 1 {
        return Err(WorkbenchError::RequestConflict);
    }
    if let Some(value) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT receipt FROM linggan_creation_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        let mut receipt: SaveReceipt = serde_json::from_value(value)?;
        let (domain, _) =
            read_revision_in(&mut tx, receipt.work_id, Some(receipt.revision.id)).await?;
        redact(&mut tx, domain, &mut receipt.revision.document).await?;
        tx.commit().await?;
        return Ok(json!({"requestId":request,"kind":"work","receipt":receipt}));
    }
    if let Some(id) = sqlx::query_scalar::<_, Uuid>(
        "SELECT export_ref FROM linggan_creation_export WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        let export = super::exports::read_in(&mut tx, id).await?;
        let revision = super::exports::qualify(&mut tx, export.work_id, export.revision_id).await?;
        tx.commit().await?;
        return Ok(
            json!({"requestId":request,"kind":"export","receipt":{"export":export,"revision":revision}}),
        );
    }
    if let Some(value) = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT receipt FROM linggan_creation_export_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        let export: ExportRecord = serde_json::from_value(value)?;
        let revision = super::exports::qualify(&mut tx, export.work_id, export.revision_id).await?;
        tx.commit().await?;
        return Ok(
            json!({"requestId":request,"kind":"export","receipt":{"export":export,"revision":revision}}),
        );
    }
    if let Some(row) =
        sqlx::query("SELECT asset_ref,work_ref FROM linggan_creation_asset WHERE request_ref=$1")
            .bind(request)
            .fetch_optional(&mut *tx)
            .await?
    {
        let receipt =
            super::assets::asset_in(&mut tx, row.try_get("work_ref")?, row.try_get("asset_ref")?)
                .await?;
        tx.commit().await?;
        return Ok(json!({"requestId":request,"kind":"asset","receipt":receipt}));
    }
    Err(WorkbenchError::NotFound)
}
