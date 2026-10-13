//! Per-file planning of the three exports with their value checks (`ops-import.v1`).

use std::collections::{BTreeMap, HashMap, HashSet};

use super::OpsImportError;
use super::business_time::{CivilDate, ExportInstant};
use super::export_rows::{
    AccountExportRow, DailyExportRow, ExportFile, ParsedRow, Platform, PublicationExportRow,
};
use super::import_plan::{
    LedgerState, LegacyDailyFigures, PlannedAccount, PlannedDaily, PlannedPublication,
    PublishedDateState,
};
use super::links::{ProfileLinkState, profile_identity, publication_content_id};

pub(super) fn plan_accounts(
    rows: &[ParsedRow<AccountExportRow>],
) -> Result<Vec<PlannedAccount>, OpsImportError> {
    let file = ExportFile::Accounts;
    let mut seen = HashSet::new();
    let mut planned = Vec::with_capacity(rows.len());
    for (index, parsed) in rows.iter().enumerate() {
        let (row, line) = (&parsed.row, index + 1);
        identity_text(file, line, "id", &row.id)?;
        identity_text(file, line, "accountName", &row.account_name)?;
        if !seen.insert(row.id.as_str()) {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "id",
                "原记录号重复",
            ));
        }
        not_deleted(file, line, row.deleted_at.as_deref())?;
        non_negative(file, line, "followerCount", row.follower_count)?;
        let (profile_link_state, platform_user_id) =
            profile_identity(row.platform, row.profile_url.as_deref());
        planned.push(PlannedAccount {
            source_record_id: row.id.clone(),
            ledger_state: LedgerState::InLedger,
            platform: row.platform,
            account_name: row.account_name.clone(),
            handle: row.handle.clone(),
            status: Some(row.status),
            is_retained: Some(row.is_retained),
            operator_name: row.operator_name.clone(),
            responsible_user_source_id: row.responsible_user_id.clone(),
            follower_count: row.follower_count,
            profile_url: row.profile_url.clone(),
            profile_link_state,
            platform_user_id,
            device_code: row.device_code.clone(),
            ip_location: row.ip_location.clone(),
            notes: row.notes.clone(),
            review_status: Some(row.review_status),
            created_by_user_source_id: row.created_by_user_id.clone(),
            source_created_at: Some(instant(file, line, "createdAt", &row.created_at)?),
            source_updated_at: Some(instant(file, line, "updatedAt", &row.updated_at)?),
        });
    }
    Ok(planned)
}

pub(super) type PublicationPlan = (
    Vec<PlannedPublication>,
    HashMap<String, Option<i64>>,
    Vec<PlannedAccount>,
);

pub(super) fn plan_publications(
    rows: &[ParsedRow<PublicationExportRow>],
    known_accounts: &HashMap<&str, Platform>,
) -> Result<PublicationPlan, OpsImportError> {
    let file = ExportFile::Publications;
    let mut seen = HashSet::new();
    let mut planned = Vec::with_capacity(rows.len());
    let mut views = HashMap::with_capacity(rows.len());
    let mut rebuilt: BTreeMap<String, (Platform, String)> = BTreeMap::new();
    for (index, parsed) in rows.iter().enumerate() {
        let (row, line) = (&parsed.row, index + 1);
        identity_text(file, line, "id", &row.id)?;
        if !seen.insert(row.id.as_str()) {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "id",
                "原记录号重复",
            ));
        }
        let account = resolve_account(row, line, known_accounts, &mut rebuilt)?;
        views.insert(row.id.clone(), row.view_count);
        planned.push(plan_publication(row, line, account)?);
    }
    let rebuilt = rebuilt
        .into_iter()
        .map(|(id, (platform, name))| rebuilt_account(id, platform, name))
        .collect();
    Ok((planned, views, rebuilt))
}

/// The account a publication belongs to. Accounts missing from the account export are rebuilt
/// from their publications, which must agree on platform and name.
fn resolve_account(
    row: &PublicationExportRow,
    line: usize,
    known: &HashMap<&str, Platform>,
    rebuilt: &mut BTreeMap<String, (Platform, String)>,
) -> Result<Option<String>, OpsImportError> {
    let file = ExportFile::Publications;
    let Some(account_id) = row.operation_account_id.as_deref() else {
        return Ok(None);
    };
    identity_text(file, line, "operationAccountId", account_id)?;
    if let Some(platform) = known.get(account_id) {
        if *platform != row.platform {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "platform",
                "与所属账号的平台不一致",
            ));
        }
        return Ok(Some(account_id.to_owned()));
    }
    let Some(name) = row
        .account_name
        .as_deref()
        .filter(|name| !name.trim().is_empty())
    else {
        return Err(OpsImportError::invalid(
            file,
            Some(line),
            "accountName",
            "已删除账号缺少账号名，无法重建",
        ));
    };
    let entry = rebuilt
        .entry(account_id.to_owned())
        .or_insert_with(|| (row.platform, name.to_owned()));
    if entry.0 != row.platform || entry.1 != name {
        return Err(OpsImportError::invalid(
            file,
            Some(line),
            "accountName",
            "同一已删除账号的平台或名称前后不一致",
        ));
    }
    Ok(Some(account_id.to_owned()))
}

fn plan_publication(
    row: &PublicationExportRow,
    line: usize,
    account_source_id: Option<String>,
) -> Result<PlannedPublication, OpsImportError> {
    let file = ExportFile::Publications;
    identity_text(file, line, "publicationUrl", &row.publication_url)?;
    identity_text(file, line, "canonicalUrl", &row.canonical_url)?;
    not_deleted(file, line, row.deleted_at.as_deref())?;
    for (field, value) in [
        ("viewCount", row.view_count),
        ("likeCount", row.like_count),
        ("collectCount", row.collect_count),
        ("commentCount", row.comment_count),
    ] {
        non_negative(file, line, field, value)?;
    }
    if row
        .cover_click_rate
        .is_some_and(|rate| !rate.is_finite() || rate < 0.0)
    {
        return Err(OpsImportError::invalid(
            file,
            Some(line),
            "coverClickRate",
            "不是非负数",
        ));
    }
    let published = ExportInstant::parse(&row.published_at)
        .ok_or_else(|| OpsImportError::invalid(file, Some(line), "publishedAt", "时间格式不符"))?;
    let (content_id_state, platform_content_id) =
        publication_content_id(row.platform, &row.publication_url, &row.canonical_url);
    let published_date_state = if published.shanghai_date().year <= 2024 {
        PublishedDateState::SuspectYear
    } else {
        PublishedDateState::AsRecorded
    };
    Ok(PlannedPublication {
        source_record_id: row.id.clone(),
        account_source_id,
        platform: row.platform,
        publication_url: row.publication_url.clone(),
        canonical_url: row.canonical_url.clone(),
        content_id_state,
        platform_content_id,
        published_at: published.as_str().to_owned(),
        published_date_state,
        title: row.title.clone(),
        body: row.copy.clone(),
        publisher_name: row.publisher_name.clone(),
        responsible_user_source_id: row.responsible_user_id.clone(),
        created_by_user_source_id: row.created_by_user_id.clone(),
        like_count: row.like_count,
        collect_count: row.collect_count,
        comment_count: row.comment_count,
        cover_click_rate: row.cover_click_rate,
        entry_source: row.source,
        review_status: row.review_status,
        legacy_effective_publication: row.effective_publication,
        legacy_data_synced_at: optional_instant(
            file,
            line,
            "dataSyncedAt",
            row.data_synced_at.as_deref(),
        )?,
        legacy_data_sync_status: row.data_sync_status,
        merged_into_source_id: None,
        missing: false,
        source_created_at: instant(file, line, "createdAt", &row.created_at)?,
        source_updated_at: instant(file, line, "updatedAt", &row.updated_at)?,
    })
}

fn rebuilt_account(
    source_record_id: String,
    platform: Platform,
    account_name: String,
) -> PlannedAccount {
    PlannedAccount {
        source_record_id,
        ledger_state: LedgerState::AbsentFromExport,
        platform,
        account_name,
        handle: None,
        status: None,
        is_retained: None,
        operator_name: None,
        responsible_user_source_id: None,
        follower_count: None,
        profile_url: None,
        profile_link_state: ProfileLinkState::Missing,
        platform_user_id: None,
        device_code: None,
        ip_location: None,
        notes: None,
        review_status: None,
        created_by_user_source_id: None,
        source_created_at: None,
        source_updated_at: None,
    }
}

pub(super) type DailyPlan = (Vec<PlannedDaily>, HashMap<String, LegacyDailyFigures>);

pub(super) fn plan_daily(rows: &[ParsedRow<DailyExportRow>]) -> Result<DailyPlan, OpsImportError> {
    let file = ExportFile::DailyMetrics;
    let mut seen_ids = HashSet::new();
    let mut seen_days = HashSet::new();
    let mut planned = Vec::with_capacity(rows.len());
    let mut legacy = HashMap::with_capacity(rows.len());
    for (index, parsed) in rows.iter().enumerate() {
        let (row, line) = (&parsed.row, index + 1);
        identity_text(file, line, "id", &row.id)?;
        identity_text(file, line, "operatorName", &row.operator_name)?;
        if !seen_ids.insert(row.id.as_str()) {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "id",
                "原记录号重复",
            ));
        }
        if row.platform != "all" || !row.account_name.is_empty() {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "platform/accountName",
                "只接受每人每天一行，不接受按平台或账号拆分的行",
            ));
        }
        let date = CivilDate::parse(&row.date_key)
            .ok_or_else(|| OpsImportError::invalid(file, Some(line), "dateKey", "日期格式不符"))?;
        if row.date != format!("{}T00:00:00.000Z", row.date_key) {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "date",
                "与 dateKey 不一致",
            ));
        }
        if !seen_days.insert((row.operator_name.as_str(), date)) {
            return Err(OpsImportError::invalid(
                file,
                Some(line),
                "operatorName/dateKey",
                "同一运营人同一天有两行",
            ));
        }
        not_deleted(file, line, row.deleted_at.as_deref())?;
        for (field, value) in [
            ("linksOpenedCount", i64::from(row.links_opened_count)),
            ("followersGained", i64::from(row.followers_gained)),
            ("publishedCount", row.published_count),
            ("viralCount", row.viral_count),
        ] {
            non_negative(file, line, field, Some(value))?;
        }
        legacy.insert(
            row.id.clone(),
            LegacyDailyFigures {
                published_count: row.published_count,
                viral_count: row.viral_count,
            },
        );
        planned.push(PlannedDaily {
            source_record_id: row.id.clone(),
            operator_name: row.operator_name.clone(),
            business_date: date,
            links_opened_count: row.links_opened_count,
            followers_gained: row.followers_gained,
            responsible_user_source_id: row.responsible_user_id.clone(),
            created_by_user_source_id: row.created_by_user_id.clone(),
            review_status: row.review_status,
            notes: row.notes.clone(),
            missing: false,
            source_created_at: instant(file, line, "createdAt", &row.created_at)?,
            source_updated_at: instant(file, line, "updatedAt", &row.updated_at)?,
        });
    }
    Ok((planned, legacy))
}

fn identity_text(
    file: ExportFile,
    line: usize,
    field: &str,
    value: &str,
) -> Result<(), OpsImportError> {
    let length = value.chars().count();
    if value.trim().is_empty() || length > 4096 {
        return Err(OpsImportError::invalid(
            file,
            Some(line),
            field,
            "为空或过长",
        ));
    }
    Ok(())
}

fn not_deleted(
    file: ExportFile,
    line: usize,
    deleted_at: Option<&str>,
) -> Result<(), OpsImportError> {
    match deleted_at {
        None => Ok(()),
        Some(_) => Err(OpsImportError::invalid(
            file,
            Some(line),
            "deletedAt",
            "导出里不应出现已删除的记录",
        )),
    }
}

fn non_negative(
    file: ExportFile,
    line: usize,
    field: &str,
    value: Option<i64>,
) -> Result<(), OpsImportError> {
    match value {
        Some(number) if number < 0 => Err(OpsImportError::invalid(
            file,
            Some(line),
            field,
            "不是非负整数",
        )),
        _ => Ok(()),
    }
}

fn instant(
    file: ExportFile,
    line: usize,
    field: &str,
    value: &str,
) -> Result<String, OpsImportError> {
    ExportInstant::parse(value)
        .map(|instant| instant.as_str().to_owned())
        .ok_or_else(|| OpsImportError::invalid(file, Some(line), field, "时间格式不符"))
}

fn optional_instant(
    file: ExportFile,
    line: usize,
    field: &str,
    value: Option<&str>,
) -> Result<Option<String>, OpsImportError> {
    value
        .map(|text| instant(file, line, field, text))
        .transpose()
}
