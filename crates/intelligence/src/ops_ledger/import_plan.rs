//! Turns three validated exports into the rows the ledger should hold (`ops-import.v1`).
//! Pure: no database access. The store compares this plan with what is already recorded.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::business_time::{CivilDate, ExportInstant};
use super::dedupe::{DedupeInput, dedupe};
use super::export_rows::{
    ACCOUNT_KEYS, AccountExportRow, AccountStatus, DAILY_KEYS, DailyExportRow, DataSyncStatus,
    EntrySource, ExportFile, PUBLICATION_ABANDONED_KEYS, PUBLICATION_KEYS, ParsedRow, Platform,
    PublicationExportRow, ReviewStatus, parse_rows,
};
use super::links::{ContentIdState, ProfileLinkState};
use super::plan_rows::{plan_accounts, plan_daily, plan_publications};
use super::{ExportFiles, OpsImportError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum LedgerState {
    InLedger,
    AbsentFromExport,
}

impl LedgerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InLedger => "in_ledger",
            Self::AbsentFromExport => "absent_from_export",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        [Self::InLedger, Self::AbsentFromExport]
            .into_iter()
            .find(|state| state.as_str() == value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum PublishedDateState {
    AsRecorded,
    SuspectYear,
}

impl PublishedDateState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AsRecorded => "as_recorded",
            Self::SuspectYear => "suspect_year",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        [Self::AsRecorded, Self::SuspectYear]
            .into_iter()
            .find(|state| state.as_str() == value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedAccount {
    pub source_record_id: String,
    pub ledger_state: LedgerState,
    pub platform: Platform,
    pub account_name: String,
    pub handle: Option<String>,
    pub status: Option<AccountStatus>,
    pub is_retained: Option<bool>,
    pub operator_name: Option<String>,
    pub responsible_user_source_id: Option<String>,
    pub follower_count: Option<i64>,
    pub profile_url: Option<String>,
    pub profile_link_state: ProfileLinkState,
    pub platform_user_id: Option<String>,
    pub device_code: Option<String>,
    pub ip_location: Option<String>,
    pub notes: Option<String>,
    pub review_status: Option<ReviewStatus>,
    pub created_by_user_source_id: Option<String>,
    pub source_created_at: Option<String>,
    pub source_updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedPublication {
    pub source_record_id: String,
    pub account_source_id: Option<String>,
    pub platform: Platform,
    pub publication_url: String,
    pub canonical_url: String,
    pub content_id_state: ContentIdState,
    pub platform_content_id: Option<String>,
    pub published_at: String,
    pub published_date_state: PublishedDateState,
    pub title: Option<String>,
    pub body: Option<String>,
    pub publisher_name: Option<String>,
    pub responsible_user_source_id: Option<String>,
    pub created_by_user_source_id: Option<String>,
    pub like_count: Option<i64>,
    pub collect_count: Option<i64>,
    pub comment_count: Option<i64>,
    pub cover_click_rate: Option<f64>,
    pub entry_source: EntrySource,
    pub review_status: ReviewStatus,
    pub legacy_effective_publication: bool,
    pub legacy_data_synced_at: Option<String>,
    pub legacy_data_sync_status: DataSyncStatus,
    pub merged_into_source_id: Option<String>,
    pub missing: bool,
    pub source_created_at: String,
    pub source_updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlannedDaily {
    pub source_record_id: String,
    pub operator_name: String,
    pub business_date: CivilDate,
    pub links_opened_count: i32,
    pub followers_gained: i32,
    pub responsible_user_source_id: Option<String>,
    pub created_by_user_source_id: Option<String>,
    pub review_status: ReviewStatus,
    pub notes: Option<String>,
    pub missing: bool,
    pub source_created_at: String,
    pub source_updated_at: String,
}

/// Old daily figures that are compared but not migrated.
#[derive(Debug, Clone, Copy)]
pub(super) struct LegacyDailyFigures {
    pub published_count: i64,
    pub viral_count: i64,
}

/// Everything the import would write, plus the inputs the reconciliation needs.
pub(super) struct ImportPlan {
    pub workspace_id: String,
    pub export_rows: [usize; 3],
    pub latest_record_update: String,
    pub accounts: Vec<PlannedAccount>,
    pub publications: Vec<PlannedPublication>,
    pub view_counts: HashMap<String, Option<i64>>,
    pub daily: Vec<PlannedDaily>,
    pub legacy_daily: HashMap<String, LegacyDailyFigures>,
    pub dedupe_groups: usize,
    pub groups_with_conflicting_views: usize,
    pub groups_with_different_dates: usize,
    pub abandoned_non_null: BTreeMap<&'static str, usize>,
    pub media_covers: usize,
}

pub(super) fn build_plan(files: &ExportFiles) -> Result<ImportPlan, OpsImportError> {
    let accounts: Vec<ParsedRow<AccountExportRow>> =
        parse_rows(ExportFile::Accounts, &files.accounts, &ACCOUNT_KEYS)?;
    let publications: Vec<ParsedRow<PublicationExportRow>> = parse_rows(
        ExportFile::Publications,
        &files.publications,
        &PUBLICATION_KEYS,
    )?;
    let daily: Vec<ParsedRow<DailyExportRow>> =
        parse_rows(ExportFile::DailyMetrics, &files.daily, &DAILY_KEYS)?;
    let workspace_id = single_workspace(&accounts, &publications, &daily)?;
    let mut planned_accounts = plan_accounts(&accounts)?;
    let known_accounts: HashMap<&str, Platform> = planned_accounts
        .iter()
        .map(|account| (account.source_record_id.as_str(), account.platform))
        .collect();
    let (mut planned_publications, view_counts, rebuilt) =
        plan_publications(&publications, &known_accounts)?;
    planned_accounts.extend(rebuilt);
    let groups = apply_dedupe(&mut planned_publications, &view_counts);
    let (planned_daily, legacy_daily) = plan_daily(&daily)?;
    let latest_record_update =
        latest_update(&planned_accounts, &planned_publications, &planned_daily);
    Ok(ImportPlan {
        workspace_id,
        export_rows: [accounts.len(), publications.len(), daily.len()],
        latest_record_update,
        accounts: planned_accounts,
        publications: planned_publications,
        view_counts,
        daily: planned_daily,
        legacy_daily,
        dedupe_groups: groups.0,
        groups_with_conflicting_views: groups.1,
        groups_with_different_dates: groups.2,
        abandoned_non_null: abandoned_counts(&publications),
        media_covers: publications
            .iter()
            .filter(|row| has_cover(&row.raw))
            .count(),
    })
}

fn single_workspace(
    accounts: &[ParsedRow<AccountExportRow>],
    publications: &[ParsedRow<PublicationExportRow>],
    daily: &[ParsedRow<DailyExportRow>],
) -> Result<String, OpsImportError> {
    let workspaces: HashSet<&str> = accounts
        .iter()
        .map(|row| row.row.workspace_id.as_str())
        .chain(publications.iter().map(|row| row.row.workspace_id.as_str()))
        .chain(daily.iter().map(|row| row.row.workspace_id.as_str()))
        .collect();
    let mut workspaces = workspaces.into_iter();
    match (workspaces.next(), workspaces.next()) {
        (Some(only), None) if !only.trim().is_empty() && only.len() <= 128 => Ok(only.to_owned()),
        (None, _) => Err(OpsImportError::invalid(
            ExportFile::Accounts,
            None,
            "整批",
            "三份导出都是空的",
        )),
        _ => Err(OpsImportError::invalid(
            ExportFile::Accounts,
            None,
            "workspaceId",
            "三份导出不属于同一个工作区",
        )),
    }
}

fn apply_dedupe(
    publications: &mut [PlannedPublication],
    views: &HashMap<String, Option<i64>>,
) -> (usize, usize, usize) {
    let outcome = {
        let inputs: Vec<DedupeInput<'_>> = publications
            .iter()
            .map(|publication| {
                let view_count = views.get(&publication.source_record_id).copied().flatten();
                DedupeInput {
                    source_record_id: &publication.source_record_id,
                    platform: publication.platform,
                    content_id: publication.platform_content_id.as_deref(),
                    canonical_url: &publication.canonical_url,
                    has_view_count: view_count.is_some(),
                    filled_fields: filled_fields(publication, view_count),
                    published_at: &publication.published_at,
                    created_at: &publication.source_created_at,
                    business_date: business_date(&publication.published_at),
                    view_count,
                }
            })
            .collect();
        dedupe(&inputs)
    };
    for (merged, kept) in &outcome.merged_into {
        publications[*merged].merged_into_source_id =
            Some(publications[*kept].source_record_id.clone());
    }
    (
        outcome.groups,
        outcome.groups_with_conflicting_views,
        outcome.groups_with_different_dates,
    )
}

fn filled_fields(publication: &PlannedPublication, view_count: Option<i64>) -> usize {
    [
        view_count.is_some(),
        publication.like_count.is_some(),
        publication.collect_count.is_some(),
        publication.comment_count.is_some(),
        publication.cover_click_rate.is_some(),
        publication.title.is_some(),
        publication.body.is_some(),
    ]
    .into_iter()
    .filter(|filled| *filled)
    .count()
}

/// Beijing business day of a stored export instant (already validated).
pub(super) fn business_date(instant: &str) -> CivilDate {
    ExportInstant::parse(instant)
        .expect("planned instants are validated")
        .shanghai_date()
}

fn latest_update(
    accounts: &[PlannedAccount],
    publications: &[PlannedPublication],
    daily: &[PlannedDaily],
) -> String {
    accounts
        .iter()
        .filter_map(|account| account.source_updated_at.as_deref())
        .chain(
            publications
                .iter()
                .map(|publication| publication.source_updated_at.as_str()),
        )
        .chain(daily.iter().map(|row| row.source_updated_at.as_str()))
        .max()
        .unwrap_or_default()
        .to_owned()
}

fn abandoned_counts(rows: &[ParsedRow<PublicationExportRow>]) -> BTreeMap<&'static str, usize> {
    PUBLICATION_ABANDONED_KEYS
        .iter()
        .map(|key| {
            (
                *key,
                rows.iter()
                    .filter(|row| row.raw.get(*key).is_some_and(|value| !value.is_null()))
                    .count(),
            )
        })
        .collect()
}

fn has_cover(raw: &serde_json::Map<String, serde_json::Value>) -> bool {
    raw.get("media")
        .and_then(|media| media.get("cover"))
        .is_some_and(|cover| !cover.is_null())
}
