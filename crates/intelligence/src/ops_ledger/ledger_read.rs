//! Reads what the ledger already holds, in the same shape as the import plan, so a re-import can
//! tell inserted, changed and unchanged rows apart without converting values twice.

use std::collections::HashMap;

use linggan_storage_postgres::Database;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use super::OpsImportError;
use super::business_time::CivilDate;
use super::export_rows::{AccountStatus, DataSyncStatus, EntrySource, Platform, ReviewStatus};
use super::import_plan::{
    LedgerState, PlannedAccount, PlannedDaily, PlannedPublication, PublishedDateState,
};
use super::links::{ContentIdState, ProfileLinkState};

/// Instants are read back in the export's own text shape.
const INSTANT: &str = "'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'";

#[derive(Debug)]
pub(super) struct LatestView {
    pub view_count: Option<i64>,
}

#[derive(Default)]
pub(super) struct ExistingLedger {
    pub accounts: HashMap<String, (Uuid, PlannedAccount)>,
    pub publications: HashMap<String, (Uuid, PlannedPublication)>,
    pub latest_views: HashMap<String, LatestView>,
    pub daily: HashMap<String, (Uuid, PlannedDaily)>,
}

pub(super) async fn ledger_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT to_regclass('linggan_ops_import_batch') IS NOT NULL")
        .fetch_one(database.pool())
        .await
}

pub(super) async fn read_existing(
    connection: &mut PgConnection,
) -> Result<ExistingLedger, OpsImportError> {
    Ok(ExistingLedger {
        accounts: read_accounts(connection).await?,
        publications: read_publications(connection).await?,
        latest_views: read_latest_views(connection).await?,
        daily: read_daily(connection).await?,
    })
}

fn vocabulary<T>(value: &str, parse: impl Fn(&str) -> Option<T>) -> Result<T, OpsImportError> {
    parse(value).ok_or(OpsImportError::UnexpectedStoredValue)
}

fn optional_vocabulary<T>(
    value: Option<String>,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, OpsImportError> {
    value
        .as_deref()
        .map(|text| vocabulary(text, &parse))
        .transpose()
}

async fn read_accounts(
    connection: &mut PgConnection,
) -> Result<HashMap<String, (Uuid, PlannedAccount)>, OpsImportError> {
    let sql = format!(
        "SELECT account_ref, source_record_id, ledger_state, platform, account_name, handle, status, \
                is_retained, operator_name, responsible_user_source_id, follower_count, profile_url, \
                profile_link_state, platform_user_id, device_code, ip_location, notes, review_status, \
                created_by_user_source_id, to_char(source_created_at AT TIME ZONE 'UTC', {INSTANT}) AS created, \
                to_char(source_updated_at AT TIME ZONE 'UTC', {INSTANT}) AS updated \
           FROM linggan_ops_account WHERE source_system = 'content_workbench'"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(&mut *connection)
        .await?;
    let mut accounts = HashMap::with_capacity(rows.len());
    for row in rows {
        let account = PlannedAccount {
            source_record_id: row.try_get("source_record_id")?,
            ledger_state: vocabulary(
                &row.try_get::<String, _>("ledger_state")?,
                LedgerState::from_db,
            )?,
            platform: vocabulary(&row.try_get::<String, _>("platform")?, Platform::from_db)?,
            account_name: row.try_get("account_name")?,
            handle: row.try_get("handle")?,
            status: optional_vocabulary(row.try_get("status")?, AccountStatus::from_db)?,
            is_retained: row.try_get("is_retained")?,
            operator_name: row.try_get("operator_name")?,
            responsible_user_source_id: row.try_get("responsible_user_source_id")?,
            follower_count: row.try_get("follower_count")?,
            profile_url: row.try_get("profile_url")?,
            profile_link_state: vocabulary(
                &row.try_get::<String, _>("profile_link_state")?,
                ProfileLinkState::from_db,
            )?,
            platform_user_id: row.try_get("platform_user_id")?,
            device_code: row.try_get("device_code")?,
            ip_location: row.try_get("ip_location")?,
            notes: row.try_get("notes")?,
            review_status: optional_vocabulary(
                row.try_get("review_status")?,
                ReviewStatus::from_db,
            )?,
            created_by_user_source_id: row.try_get("created_by_user_source_id")?,
            source_created_at: row.try_get("created")?,
            source_updated_at: row.try_get("updated")?,
        };
        accounts.insert(
            account.source_record_id.clone(),
            (row.try_get("account_ref")?, account),
        );
    }
    Ok(accounts)
}

async fn read_publications(
    connection: &mut PgConnection,
) -> Result<HashMap<String, (Uuid, PlannedPublication)>, OpsImportError> {
    let sql = format!(
        "SELECT p.publication_ref, p.source_record_id, account.source_record_id AS account_source_id, \
                p.platform, p.publication_url, p.canonical_url, p.content_id_state, p.platform_content_id, \
                to_char(p.published_at AT TIME ZONE 'UTC', {INSTANT}) AS published, p.published_date_state, \
                p.title, p.body, p.publisher_name, p.responsible_user_source_id, p.created_by_user_source_id, \
                p.like_count, p.collect_count, p.comment_count, p.cover_click_rate, p.entry_source, \
                p.review_status, p.legacy_effective_publication, \
                to_char(p.legacy_data_synced_at AT TIME ZONE 'UTC', {INSTANT}) AS synced, \
                p.legacy_data_sync_status, kept.source_record_id AS merged_into_source_id, \
                p.missing_since_batch_ref IS NOT NULL AS missing, \
                to_char(p.source_created_at AT TIME ZONE 'UTC', {INSTANT}) AS created, \
                to_char(p.source_updated_at AT TIME ZONE 'UTC', {INSTANT}) AS updated \
           FROM linggan_ops_publication p \
           LEFT JOIN linggan_ops_account account ON account.account_ref = p.account_ref \
           LEFT JOIN linggan_ops_publication kept ON kept.publication_ref = p.merged_into_ref \
          WHERE p.source_system = 'content_workbench'"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(&mut *connection)
        .await?;
    let mut publications = HashMap::with_capacity(rows.len());
    for row in rows {
        let publication = stored_publication(&row)?;
        publications.insert(
            publication.source_record_id.clone(),
            (row.try_get("publication_ref")?, publication),
        );
    }
    Ok(publications)
}

fn stored_publication(row: &sqlx::postgres::PgRow) -> Result<PlannedPublication, OpsImportError> {
    Ok(PlannedPublication {
        source_record_id: row.try_get("source_record_id")?,
        account_source_id: row.try_get("account_source_id")?,
        platform: vocabulary(&row.try_get::<String, _>("platform")?, Platform::from_db)?,
        publication_url: row.try_get("publication_url")?,
        canonical_url: row.try_get("canonical_url")?,
        content_id_state: vocabulary(
            &row.try_get::<String, _>("content_id_state")?,
            ContentIdState::from_db,
        )?,
        platform_content_id: row.try_get("platform_content_id")?,
        published_at: row.try_get("published")?,
        published_date_state: vocabulary(
            &row.try_get::<String, _>("published_date_state")?,
            PublishedDateState::from_db,
        )?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        publisher_name: row.try_get("publisher_name")?,
        responsible_user_source_id: row.try_get("responsible_user_source_id")?,
        created_by_user_source_id: row.try_get("created_by_user_source_id")?,
        like_count: row.try_get("like_count")?,
        collect_count: row.try_get("collect_count")?,
        comment_count: row.try_get("comment_count")?,
        cover_click_rate: row.try_get("cover_click_rate")?,
        entry_source: vocabulary(
            &row.try_get::<String, _>("entry_source")?,
            EntrySource::from_db,
        )?,
        review_status: vocabulary(
            &row.try_get::<String, _>("review_status")?,
            ReviewStatus::from_db,
        )?,
        legacy_effective_publication: row.try_get("legacy_effective_publication")?,
        legacy_data_synced_at: row.try_get("synced")?,
        legacy_data_sync_status: vocabulary(
            &row.try_get::<String, _>("legacy_data_sync_status")?,
            DataSyncStatus::from_db,
        )?,
        merged_into_source_id: row.try_get("merged_into_source_id")?,
        missing: row.try_get("missing")?,
        source_created_at: row.try_get("created")?,
        source_updated_at: row.try_get("updated")?,
    })
}

async fn read_latest_views(
    connection: &mut PgConnection,
) -> Result<HashMap<String, LatestView>, OpsImportError> {
    let rows = sqlx::query(
        "SELECT DISTINCT ON (report.publication_ref) p.source_record_id, report.view_count \
           FROM linggan_ops_view_report report \
           JOIN linggan_ops_publication p ON p.publication_ref = report.publication_ref \
          WHERE p.source_system = 'content_workbench' \
          ORDER BY report.publication_ref, report.report_sequence DESC",
    )
    .fetch_all(&mut *connection)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                row.try_get("source_record_id")?,
                LatestView {
                    view_count: row.try_get("view_count")?,
                },
            ))
        })
        .collect()
}

async fn read_daily(
    connection: &mut PgConnection,
) -> Result<HashMap<String, (Uuid, PlannedDaily)>, OpsImportError> {
    let sql = format!(
        "SELECT daily_ref, source_record_id, operator_name, business_date::text AS business_date, \
                links_opened_count, followers_gained, responsible_user_source_id, \
                created_by_user_source_id, review_status, notes, missing_since_batch_ref IS NOT NULL AS missing, \
                to_char(source_created_at AT TIME ZONE 'UTC', {INSTANT}) AS created, \
                to_char(source_updated_at AT TIME ZONE 'UTC', {INSTANT}) AS updated \
           FROM linggan_ops_daily_report WHERE source_system = 'content_workbench'"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .fetch_all(&mut *connection)
        .await?;
    let mut daily = HashMap::with_capacity(rows.len());
    for row in rows {
        let report = PlannedDaily {
            source_record_id: row.try_get("source_record_id")?,
            operator_name: row.try_get("operator_name")?,
            business_date: vocabulary(
                &row.try_get::<String, _>("business_date")?,
                CivilDate::parse,
            )?,
            links_opened_count: row.try_get("links_opened_count")?,
            followers_gained: row.try_get("followers_gained")?,
            responsible_user_source_id: row.try_get("responsible_user_source_id")?,
            created_by_user_source_id: row.try_get("created_by_user_source_id")?,
            review_status: vocabulary(
                &row.try_get::<String, _>("review_status")?,
                ReviewStatus::from_db,
            )?,
            notes: row.try_get("notes")?,
            missing: row.try_get("missing")?,
            source_created_at: row.try_get("created")?,
            source_updated_at: row.try_get("updated")?,
        };
        daily.insert(
            report.source_record_id.clone(),
            (row.try_get("daily_ref")?, report),
        );
    }
    Ok(daily)
}
