//! Ledger-side figures read back from PostgreSQL after an import, and their comparison with the
//! export-side reconciliation. Only rows present in the latest export are counted.

use std::collections::BTreeMap;

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;

use super::reconciliation::{MonthCount, Reconciliation};
use super::{OpsImportError, XHS_HIT_VIEW_THRESHOLD};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMonthTotals {
    pub rows: i64,
    pub links_opened: i64,
    pub followers_gained: i64,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerTotals {
    pub accounts_by_state: BTreeMap<String, i64>,
    pub publications_present: i64,
    pub publications_kept: i64,
    pub publications_by_month: BTreeMap<String, MonthCount>,
    pub publications_suspect_year: i64,
    pub publications_without_account: i64,
    pub current_view_values_by_platform: BTreeMap<String, i64>,
    pub xhs_kept_hits_by_month: BTreeMap<String, i64>,
    pub daily_present: i64,
    pub daily_by_month: BTreeMap<String, DailyMonthTotals>,
}

/// One line of the old/new reconciliation table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyCheck {
    pub item: String,
    pub export: i64,
    pub ledger: i64,
    pub consistent: bool,
}

pub async fn read_ledger_totals(database: &Database) -> Result<LedgerTotals, OpsImportError> {
    let pool = database.pool();
    let mut totals = LedgerTotals::default();
    for row in sqlx::query(
        "SELECT ledger_state || ':' || platform AS key, count(*) AS n FROM linggan_ops_account GROUP BY 1",
    )
    .fetch_all(pool)
    .await?
    {
        totals.accounts_by_state.insert(row.try_get("key")?, row.try_get("n")?);
    }
    for row in sqlx::query(
        "SELECT to_char(published_business_date, 'YYYY-MM') AS month, count(*) AS records, \
                count(*) FILTER (WHERE merged_into_ref IS NULL) AS kept, \
                count(*) FILTER (WHERE published_date_state = 'suspect_year') AS suspect, \
                count(*) FILTER (WHERE account_ref IS NULL) AS without_account \
           FROM linggan_ops_publication WHERE missing_since_batch_ref IS NULL GROUP BY 1",
    )
    .fetch_all(pool)
    .await?
    {
        let month = MonthCount {
            records: row.try_get("records")?,
            kept: row.try_get("kept")?,
        };
        totals.publications_present += month.records;
        totals.publications_kept += month.kept;
        totals.publications_suspect_year += row.try_get::<i64, _>("suspect")?;
        totals.publications_without_account += row.try_get::<i64, _>("without_account")?;
        totals
            .publications_by_month
            .insert(row.try_get("month")?, month);
    }
    read_view_totals(database, &mut totals).await?;
    read_daily_totals(database, &mut totals).await?;
    Ok(totals)
}

async fn read_view_totals(
    database: &Database,
    totals: &mut LedgerTotals,
) -> Result<(), OpsImportError> {
    for row in sqlx::query(
        "WITH latest AS ( \
            SELECT DISTINCT ON (publication_ref) publication_ref, view_count \
              FROM linggan_ops_view_report ORDER BY publication_ref, report_sequence DESC) \
         SELECT p.platform, to_char(p.published_business_date, 'YYYY-MM') AS month, \
                count(*) AS with_value, \
                count(*) FILTER (WHERE p.platform = 'xhs' AND p.merged_into_ref IS NULL \
                                   AND latest.view_count >= $1) AS hits \
           FROM linggan_ops_publication p JOIN latest USING (publication_ref) \
          WHERE p.missing_since_batch_ref IS NULL AND latest.view_count IS NOT NULL \
          GROUP BY 1, 2",
    )
    .bind(XHS_HIT_VIEW_THRESHOLD)
    .fetch_all(database.pool())
    .await?
    {
        let platform: String = row.try_get("platform")?;
        *totals
            .current_view_values_by_platform
            .entry(platform)
            .or_default() += row.try_get::<i64, _>("with_value")?;
        let hits: i64 = row.try_get("hits")?;
        if hits > 0 {
            *totals
                .xhs_kept_hits_by_month
                .entry(row.try_get("month")?)
                .or_default() += hits;
        }
    }
    Ok(())
}

async fn read_daily_totals(
    database: &Database,
    totals: &mut LedgerTotals,
) -> Result<(), OpsImportError> {
    for row in sqlx::query(
        "SELECT to_char(business_date, 'YYYY-MM') AS month, count(*) AS rows, \
                sum(links_opened_count)::bigint AS links, sum(followers_gained)::bigint AS followers \
           FROM linggan_ops_daily_report WHERE missing_since_batch_ref IS NULL GROUP BY 1",
    )
    .fetch_all(database.pool())
    .await?
    {
        let month = DailyMonthTotals {
            rows: row.try_get("rows")?,
            links_opened: row.try_get("links")?,
            followers_gained: row.try_get("followers")?,
        };
        totals.daily_present += month.rows;
        totals.daily_by_month.insert(row.try_get("month")?, month);
    }
    Ok(())
}

/// Compares the export-side figures with what the ledger now holds.
pub fn compare_with_ledger(
    export: &Reconciliation,
    ledger: &LedgerTotals,
) -> Vec<ConsistencyCheck> {
    let mut checks = Vec::new();
    let mut push = |item: String, export: i64, ledger: i64| {
        checks.push(ConsistencyCheck {
            item,
            export,
            ledger,
            consistent: export == ledger,
        });
    };
    for (platform, count) in &export.accounts.in_ledger_by_platform {
        let stored = ledger
            .accounts_by_state
            .get(&format!("in_ledger:{platform}"))
            .copied()
            .unwrap_or(0);
        push(format!("账号（{platform}）"), *count, stored);
    }
    push(
        "发布记录".into(),
        export.publications.export_rows as i64,
        ledger.publications_present,
    );
    push(
        "发布记录（合并后）".into(),
        export.publications.kept_after_merge,
        ledger.publications_kept,
    );
    push(
        "日期存疑".into(),
        export.publications.suspect_year,
        ledger.publications_suspect_year,
    );
    push(
        "账号未知".into(),
        export.publications.without_account,
        ledger.publications_without_account,
    );
    for (month, counts) in &export.publications.by_month {
        let stored = ledger
            .publications_by_month
            .get(month)
            .copied()
            .unwrap_or_default();
        push(format!("{month} 发布记录"), counts.records, stored.records);
        push(
            format!("{month} 发布记录（合并后）"),
            counts.kept,
            stored.kept,
        );
    }
    for (platform, count) in &export.views.with_view_count_by_platform {
        let stored = ledger
            .current_view_values_by_platform
            .get(*platform)
            .copied()
            .unwrap_or(0);
        push(format!("有阅读量（{platform}）"), *count, stored);
    }
    let ledger_hits: i64 = ledger.xhs_kept_hits_by_month.values().sum();
    push(
        format!("小红书阅读 ≥{XHS_HIT_VIEW_THRESHOLD}"),
        export.views.xhs_kept_hits,
        ledger_hits,
    );
    push(
        "每日数据".into(),
        export.daily.export_rows as i64,
        ledger.daily_present,
    );
    for (month, figures) in &export.daily.by_month {
        let stored = ledger
            .daily_by_month
            .get(month)
            .copied()
            .unwrap_or_default();
        push(
            format!("{month} 链接打开"),
            figures.links_opened,
            stored.links_opened,
        );
        push(
            format!("{month} 加微"),
            figures.followers_gained,
            stored.followers_gained,
        );
    }
    checks
}
