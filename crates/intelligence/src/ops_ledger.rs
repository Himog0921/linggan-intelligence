//! OPS-MIGRATION-001 D1 (Issue #402): the operations ledger mirrored from content-workbench
//! exports — own accounts, publication registrations, view-count reports and operator daily
//! reports. Contract and rule versions: `docs/data-contracts/ops-ledger-001.md`.
//!
//! An import is previewed first (read-only) and then applied with the preview hash. Imported
//! values state what the workbench recorded at export time; they are not Linggan observations.

mod business_time;
pub mod covers;
mod dedupe;
mod export_rows;
mod import_batch;
mod import_diff;
mod import_plan;
mod import_store;
mod ledger_read;
mod ledger_totals;
mod links;
mod plan_rows;
pub mod read;
mod reconciliation;

use linggan_storage_postgres::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

pub use export_rows::ExportFile;
pub use import_diff::ChangeCounts;
pub use ledger_totals::{ConsistencyCheck, LedgerTotals, compare_with_ledger, read_ledger_totals};
pub use reconciliation::Reconciliation;

pub const OPS_IMPORT_RULE_VERSION: &str = "ops-import.v1";
/// DEC-0009: an own Xiaohongshu post with at least this many views is a hit. Douyin is not judged.
pub const XHS_HIT_VIEW_THRESHOLD: i64 = 5000;

/// True for a valid `YYYY-MM-DD` calendar date (Beijing business days use this shape).
pub(crate) fn is_business_date(text: &str) -> bool {
    business_time::CivilDate::parse(text).is_some()
}

/// The three export files exactly as downloaded.
pub struct ExportFiles {
    pub accounts: Vec<u8>,
    pub publications: Vec<u8>,
    pub daily: Vec<u8>,
}

pub struct ImportRequest {
    pub files: ExportFiles,
    /// When the files were exported, with an explicit UTC offset.
    pub exported_at: String,
    /// Business domain every newly seen account is attributed to.
    pub domain_ref: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum OpsImportError {
    #[error("{}{}字段「{field}」：{reason}", .file.label(), row_label(*.row))]
    InvalidExport {
        file: ExportFile,
        row: Option<usize>,
        field: String,
        reason: &'static str,
    },
    #[error("运营台账的表还没有建（迁移 0123 未执行），未写入")]
    SchemaUnavailable,
    #[error("指定的业务领域不存在，未写入")]
    UnknownDomain,
    #[error("导出时间必须带时区，且不能早于导出内容的最后修改时间或晚于现在，未写入")]
    InvalidExportedAt,
    #[error("预览指纹不一致：文件、导出时间、业务领域或规则版本与预览时不同，未写入")]
    PreviewHashMismatch,
    #[error("台账里出现了规则之外的取值，未写入")]
    UnexpectedStoredValue,
    #[error("拷来的封面文件读不出来，未写入")]
    CoverFileUnreadable,
    #[error("本机媒体目录里已有同名但内容不同的封面文件，未写入")]
    CoverStorageConflict,
    #[error("本机媒体目录写不进去，未登记")]
    CoverStorageUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl OpsImportError {
    pub(crate) fn invalid(
        file: ExportFile,
        row: Option<usize>,
        field: &str,
        reason: &'static str,
    ) -> Self {
        Self::InvalidExport {
            file,
            row,
            field: field.to_owned(),
            reason,
        }
    }
}

fn row_label(row: Option<usize>) -> String {
    row.map_or_else(|| " ".to_owned(), |row| format!("第 {row} 行 "))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputFingerprint {
    pub accounts_sha256: String,
    pub publications_sha256: String,
    pub daily_sha256: String,
    pub account_rows: usize,
    pub publication_rows: usize,
    pub daily_rows: usize,
    pub latest_record_update: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub preview_hash: String,
    pub rule_version: &'static str,
    pub exported_at: String,
    pub domain_ref: Uuid,
    pub input: InputFingerprint,
    pub reconciliation: Reconciliation,
    pub changes: ChangeCounts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyOutcome {
    Applied,
    /// The same preview was applied before; nothing was written this time.
    AlreadyApplied,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReceipt {
    pub outcome: ApplyOutcome,
    pub batch_ref: Uuid,
    pub preview_hash: String,
    pub exported_at: String,
    pub changes: ChangeCounts,
}

/// Everything derived from the request before touching the ledger.
struct Prepared {
    plan: import_plan::ImportPlan,
    input: InputFingerprint,
    exported_at: String,
    preview_hash: String,
}

/// Validates the exports and shows what an apply would change. Never writes.
pub async fn preview_ops_import(
    database: &Database,
    request: &ImportRequest,
) -> Result<ImportPreview, OpsImportError> {
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *transaction)
        .await?;
    let prepared = prepare(database, &mut transaction, request).await?;
    let existing = ledger_read::read_existing(&mut transaction).await?;
    let changes = import_diff::diff(&prepared.plan, &existing).counts;
    transaction.rollback().await?;
    Ok(ImportPreview {
        reconciliation: reconciliation::reconcile(&prepared.plan),
        preview_hash: prepared.preview_hash,
        rule_version: OPS_IMPORT_RULE_VERSION,
        exported_at: prepared.exported_at,
        domain_ref: request.domain_ref,
        input: prepared.input,
        changes,
    })
}

/// Applies a previewed import in one transaction. A different preview hash writes nothing; an
/// already applied preview returns its original receipt.
pub async fn apply_ops_import(
    database: &Database,
    request: &ImportRequest,
    expected_preview_hash: &str,
) -> Result<ImportReceipt, OpsImportError> {
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('linggan_ops_import', 0))")
        .execute(&mut *transaction)
        .await?;
    let prepared = prepare(database, &mut transaction, request).await?;
    if prepared.preview_hash != expected_preview_hash {
        return Err(OpsImportError::PreviewHashMismatch);
    }
    if let Some(receipt) =
        import_batch::applied_receipt(&mut transaction, &prepared.preview_hash).await?
    {
        return Ok(receipt);
    }
    let existing = ledger_read::read_existing(&mut transaction).await?;
    let changes = import_diff::diff(&prepared.plan, &existing);
    let batch_ref = Uuid::new_v4();
    import_store::write_changes(
        &mut transaction,
        &changes,
        batch_ref,
        request.domain_ref,
        &prepared.exported_at,
    )
    .await?;
    import_batch::insert_batch(
        &mut transaction,
        &prepared,
        batch_ref,
        request.domain_ref,
        &changes.counts,
    )
    .await?;
    transaction.commit().await?;
    Ok(ImportReceipt {
        outcome: ApplyOutcome::Applied,
        batch_ref,
        preview_hash: prepared.preview_hash,
        exported_at: prepared.exported_at,
        changes: changes.counts,
    })
}

async fn prepare(
    database: &Database,
    connection: &mut PgConnection,
    request: &ImportRequest,
) -> Result<Prepared, OpsImportError> {
    if !ledger_read::ledger_schema_is_ready(database).await? {
        return Err(OpsImportError::SchemaUnavailable);
    }
    let plan = import_plan::build_plan(&request.files)?;
    let domain_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM observation_domain WHERE domain_ref = $1)",
    )
    .bind(request.domain_ref)
    .fetch_one(&mut *connection)
    .await?;
    if !domain_exists {
        return Err(OpsImportError::UnknownDomain);
    }
    let exported_at =
        normalized_exported_at(connection, &request.exported_at, &plan.latest_record_update)
            .await?;
    let input = InputFingerprint {
        accounts_sha256: sha256_hex(&request.files.accounts),
        publications_sha256: sha256_hex(&request.files.publications),
        daily_sha256: sha256_hex(&request.files.daily),
        account_rows: plan.export_rows[0],
        publication_rows: plan.export_rows[1],
        daily_rows: plan.export_rows[2],
        latest_record_update: plan.latest_record_update.clone(),
    };
    let preview_hash = sha256_hex(
        format!(
            "{OPS_IMPORT_RULE_VERSION}\n{exported_at}\n{}\n{}\n{}\n{}\n",
            request.domain_ref,
            input.accounts_sha256,
            input.publications_sha256,
            input.daily_sha256
        )
        .as_bytes(),
    );
    Ok(Prepared {
        plan,
        input,
        exported_at,
        preview_hash,
    })
}

/// The export time in the export's own UTC shape. It needs an explicit offset, cannot precede the
/// newest record in the files and cannot lie in the future.
async fn normalized_exported_at(
    connection: &mut PgConnection,
    stated: &str,
    latest_record_update: &str,
) -> Result<String, OpsImportError> {
    let stated = stated.trim();
    let bytes = stated.as_bytes();
    let has_offset = stated.ends_with('Z')
        || (bytes.len() > 6
            && matches!(bytes[bytes.len() - 6], b'+' | b'-')
            && bytes[bytes.len() - 3] == b':');
    if !has_offset {
        return Err(OpsImportError::InvalidExportedAt);
    }
    let normalized: Option<String> = sqlx::query_scalar(
        "SELECT to_char(t AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') \
           FROM (SELECT $1::timestamptz AS t) stated \
          WHERE t <= now() + interval '5 minutes' \
            AND CASE WHEN $2 = '' THEN true ELSE t >= $2::timestamptz END",
    )
    .bind(stated)
    .bind(latest_record_update)
    .fetch_optional(&mut *connection)
    .await
    .map_err(|_| OpsImportError::InvalidExportedAt)?;
    normalized.ok_or(OpsImportError::InvalidExportedAt)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
