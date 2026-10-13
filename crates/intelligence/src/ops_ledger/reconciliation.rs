//! Export-side figures of the old/new reconciliation table. Aggregates only: no account names,
//! titles, links or operator names ever appear here.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::XHS_HIT_VIEW_THRESHOLD;
use super::business_time::CivilDate;
use super::export_rows::Platform;
use super::import_plan::{ImportPlan, LedgerState, PublishedDateState, business_date};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthCount {
    pub records: i64,
    pub kept: i64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyMonth {
    pub rows: i64,
    pub links_opened: i64,
    pub followers_gained: i64,
    /// Old stored「发布数」(publisher name OR shared responsible id; may double count).
    pub legacy_published_count: i64,
    /// Kept registrations counted per operator-day by publisher name, the Linggan rule.
    pub recomputed_published_count: i64,
    /// Old manual「当日爆款数」, documented but not migrated.
    pub legacy_viral_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reconciliation {
    pub accounts: AccountFigures,
    pub publications: PublicationFigures,
    pub views: ViewFigures,
    pub daily: DailyFigures,
    pub dropped: DroppedFigures,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountFigures {
    pub export_rows: usize,
    pub in_ledger_by_platform: BTreeMap<&'static str, i64>,
    pub in_ledger_by_status: BTreeMap<&'static str, i64>,
    pub rebuilt_from_publications: i64,
    pub publications_on_rebuilt_accounts: i64,
    pub profile_link_states: BTreeMap<&'static str, i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationFigures {
    pub export_rows: usize,
    pub by_platform: BTreeMap<&'static str, i64>,
    pub by_month: BTreeMap<String, MonthCount>,
    pub duplicate_groups: usize,
    pub merged_records: i64,
    pub kept_after_merge: i64,
    pub groups_with_conflicting_views: usize,
    pub groups_with_different_dates: usize,
    pub content_id_states: BTreeMap<String, i64>,
    pub suspect_year: i64,
    pub without_account: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewFigures {
    pub with_view_count_by_platform: BTreeMap<&'static str, i64>,
    pub xhs_kept_with_view_count: i64,
    pub xhs_kept_hits: i64,
    pub xhs_kept_hits_by_month: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyFigures {
    pub export_rows: usize,
    pub by_month: BTreeMap<String, DailyMonth>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DroppedFigures {
    pub abandoned_fields_non_null: BTreeMap<&'static str, usize>,
    pub media_covers_in_workbench: usize,
}

pub(super) fn reconcile(plan: &ImportPlan) -> Reconciliation {
    Reconciliation {
        accounts: account_figures(plan),
        publications: publication_figures(plan),
        views: view_figures(plan),
        daily: daily_figures(plan),
        dropped: DroppedFigures {
            abandoned_fields_non_null: plan.abandoned_non_null.clone(),
            media_covers_in_workbench: plan.media_covers,
        },
    }
}

fn bump<K: Ord>(map: &mut BTreeMap<K, i64>, key: K) {
    *map.entry(key).or_default() += 1;
}

fn account_figures(plan: &ImportPlan) -> AccountFigures {
    let mut figures = AccountFigures {
        export_rows: plan.export_rows[0],
        in_ledger_by_platform: BTreeMap::new(),
        in_ledger_by_status: BTreeMap::new(),
        rebuilt_from_publications: 0,
        publications_on_rebuilt_accounts: 0,
        profile_link_states: BTreeMap::new(),
    };
    let mut rebuilt = std::collections::HashSet::new();
    for account in &plan.accounts {
        bump(
            &mut figures.profile_link_states,
            account.profile_link_state.as_str(),
        );
        match account.ledger_state {
            LedgerState::InLedger => {
                bump(
                    &mut figures.in_ledger_by_platform,
                    account.platform.as_str(),
                );
                if let Some(status) = account.status {
                    bump(&mut figures.in_ledger_by_status, status.as_str());
                }
            }
            LedgerState::AbsentFromExport => {
                figures.rebuilt_from_publications += 1;
                rebuilt.insert(account.source_record_id.as_str());
            }
        }
    }
    figures.publications_on_rebuilt_accounts = plan
        .publications
        .iter()
        .filter(|publication| {
            publication
                .account_source_id
                .as_deref()
                .is_some_and(|id| rebuilt.contains(id))
        })
        .count() as i64;
    figures
}

fn publication_figures(plan: &ImportPlan) -> PublicationFigures {
    let mut figures = PublicationFigures {
        export_rows: plan.export_rows[1],
        by_platform: BTreeMap::new(),
        by_month: BTreeMap::new(),
        duplicate_groups: plan.dedupe_groups,
        merged_records: 0,
        kept_after_merge: 0,
        groups_with_conflicting_views: plan.groups_with_conflicting_views,
        groups_with_different_dates: plan.groups_with_different_dates,
        content_id_states: BTreeMap::new(),
        suspect_year: 0,
        without_account: 0,
    };
    for publication in &plan.publications {
        let kept = publication.merged_into_source_id.is_none();
        bump(&mut figures.by_platform, publication.platform.as_str());
        let month = figures
            .by_month
            .entry(business_date(&publication.published_at).month_key())
            .or_default();
        month.records += 1;
        month.kept += i64::from(kept);
        figures.merged_records += i64::from(!kept);
        figures.kept_after_merge += i64::from(kept);
        let state = format!(
            "{}:{}",
            publication.platform.as_str(),
            publication.content_id_state.as_str()
        );
        bump(&mut figures.content_id_states, state);
        figures.suspect_year +=
            i64::from(publication.published_date_state == PublishedDateState::SuspectYear);
        figures.without_account += i64::from(publication.account_source_id.is_none());
    }
    figures
}

fn view_figures(plan: &ImportPlan) -> ViewFigures {
    let mut figures = ViewFigures {
        with_view_count_by_platform: BTreeMap::new(),
        xhs_kept_with_view_count: 0,
        xhs_kept_hits: 0,
        xhs_kept_hits_by_month: BTreeMap::new(),
    };
    for publication in &plan.publications {
        let Some(views) = plan
            .view_counts
            .get(&publication.source_record_id)
            .copied()
            .flatten()
        else {
            continue;
        };
        bump(
            &mut figures.with_view_count_by_platform,
            publication.platform.as_str(),
        );
        if publication.platform != Platform::Xhs || publication.merged_into_source_id.is_some() {
            continue;
        }
        figures.xhs_kept_with_view_count += 1;
        if views >= XHS_HIT_VIEW_THRESHOLD {
            figures.xhs_kept_hits += 1;
            bump(
                &mut figures.xhs_kept_hits_by_month,
                business_date(&publication.published_at).month_key(),
            );
        }
    }
    figures
}

fn daily_figures(plan: &ImportPlan) -> DailyFigures {
    let mut by_publisher_day: HashMap<(&str, CivilDate), i64> = HashMap::new();
    for publication in plan
        .publications
        .iter()
        .filter(|publication| publication.merged_into_source_id.is_none())
    {
        if let Some(publisher) = publication.publisher_name.as_deref() {
            *by_publisher_day
                .entry((publisher, business_date(&publication.published_at)))
                .or_default() += 1;
        }
    }
    let mut by_month: BTreeMap<String, DailyMonth> = BTreeMap::new();
    for row in &plan.daily {
        let month = by_month.entry(row.business_date.month_key()).or_default();
        let legacy = plan.legacy_daily[&row.source_record_id];
        month.rows += 1;
        month.links_opened += i64::from(row.links_opened_count);
        month.followers_gained += i64::from(row.followers_gained);
        month.legacy_published_count += legacy.published_count;
        month.legacy_viral_count += legacy.viral_count;
        month.recomputed_published_count += by_publisher_day
            .get(&(row.operator_name.as_str(), row.business_date))
            .copied()
            .unwrap_or(0);
    }
    DailyFigures {
        export_rows: plan.export_rows[2],
        by_month,
    }
}
