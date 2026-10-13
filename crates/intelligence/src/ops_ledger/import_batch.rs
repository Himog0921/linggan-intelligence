//! The applied-import batch row: written once at the end of an apply, read back for replays.

use sqlx::{PgConnection, Row};
use uuid::Uuid;

use super::import_diff::ChangeCounts;
use super::{ApplyOutcome, ImportReceipt, OPS_IMPORT_RULE_VERSION, OpsImportError, Prepared};

const COUNT_COLUMNS: [&str; 12] = [
    "accounts_inserted",
    "accounts_updated",
    "accounts_unchanged",
    "publications_inserted",
    "publications_updated",
    "publications_unchanged",
    "publications_marked_missing",
    "daily_inserted",
    "daily_updated",
    "daily_unchanged",
    "daily_marked_missing",
    "view_reports_appended",
];

pub(super) async fn applied_receipt(
    connection: &mut PgConnection,
    preview_hash: &str,
) -> Result<Option<ImportReceipt>, OpsImportError> {
    let Some(row) = sqlx::query(
        "SELECT batch_ref, to_char(exported_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS exported, \
                accounts_inserted, accounts_updated, accounts_unchanged, publications_inserted, \
                publications_updated, publications_unchanged, publications_marked_missing, \
                daily_inserted, daily_updated, daily_unchanged, daily_marked_missing, view_reports_appended \
           FROM linggan_ops_import_batch WHERE preview_hash = $1",
    )
    .bind(preview_hash)
    .fetch_optional(&mut *connection)
    .await?
    else {
        return Ok(None);
    };
    let mut values = [0_i32; 12];
    for (value, column) in values.iter_mut().zip(COUNT_COLUMNS) {
        *value = row.try_get(column)?;
    }
    let [ai, au, ac, pi, pu, pc, pm, di, du, dc, dm, vr] = values;
    Ok(Some(ImportReceipt {
        outcome: ApplyOutcome::AlreadyApplied,
        batch_ref: row.try_get("batch_ref")?,
        preview_hash: preview_hash.to_owned(),
        exported_at: row.try_get("exported")?,
        changes: ChangeCounts {
            accounts_inserted: ai,
            accounts_updated: au,
            accounts_unchanged: ac,
            publications_inserted: pi,
            publications_updated: pu,
            publications_unchanged: pc,
            publications_marked_missing: pm,
            daily_inserted: di,
            daily_updated: du,
            daily_unchanged: dc,
            daily_marked_missing: dm,
            view_reports_appended: vr,
        },
    }))
}

pub(super) async fn insert_batch(
    connection: &mut PgConnection,
    prepared: &Prepared,
    batch_ref: Uuid,
    domain_ref: Uuid,
    counts: &ChangeCounts,
) -> Result<(), OpsImportError> {
    let rows =
        |count: usize| i32::try_from(count).map_err(|_| OpsImportError::UnexpectedStoredValue);
    sqlx::query(
        "INSERT INTO linggan_ops_import_batch ( \
            batch_ref, preview_hash, rule_version, source_system, source_workspace_id, exported_at, \
            domain_ref, accounts_sha256, publications_sha256, daily_sha256, account_rows, \
            publication_rows, daily_rows, accounts_inserted, accounts_updated, accounts_unchanged, \
            publications_inserted, publications_updated, publications_unchanged, \
            publications_marked_missing, daily_inserted, daily_updated, daily_unchanged, \
            daily_marked_missing, view_reports_appended) \
         VALUES ($1,$2,$3,'content_workbench',$4,$5::timestamptz,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15, \
                 $16,$17,$18,$19,$20,$21,$22,$23,$24)",
    )
    .bind(batch_ref)
    .bind(&prepared.preview_hash)
    .bind(OPS_IMPORT_RULE_VERSION)
    .bind(&prepared.plan.workspace_id)
    .bind(&prepared.exported_at)
    .bind(domain_ref)
    .bind(&prepared.input.accounts_sha256)
    .bind(&prepared.input.publications_sha256)
    .bind(&prepared.input.daily_sha256)
    .bind(rows(prepared.input.account_rows)?)
    .bind(rows(prepared.input.publication_rows)?)
    .bind(rows(prepared.input.daily_rows)?)
    .bind(counts.accounts_inserted)
    .bind(counts.accounts_updated)
    .bind(counts.accounts_unchanged)
    .bind(counts.publications_inserted)
    .bind(counts.publications_updated)
    .bind(counts.publications_unchanged)
    .bind(counts.publications_marked_missing)
    .bind(counts.daily_inserted)
    .bind(counts.daily_updated)
    .bind(counts.daily_unchanged)
    .bind(counts.daily_marked_missing)
    .bind(counts.view_reports_appended)
    .execute(&mut *connection)
    .await?;
    Ok(())
}
