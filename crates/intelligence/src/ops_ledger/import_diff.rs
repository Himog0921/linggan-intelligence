//! Compares the import plan with what the ledger holds: inserted, changed, unchanged and newly
//! missing rows, plus the view reports rule `ops-views.v1` appends.

use std::collections::HashMap;

use serde::Serialize;
use uuid::Uuid;

use super::import_plan::{
    ImportPlan, LedgerState, PlannedAccount, PlannedDaily, PlannedPublication,
};
use super::ledger_read::{ExistingLedger, LatestView};

/// Per-table change counts of one import, as previewed or as applied.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeCounts {
    pub accounts_inserted: i32,
    pub accounts_updated: i32,
    pub accounts_unchanged: i32,
    pub publications_inserted: i32,
    pub publications_updated: i32,
    pub publications_unchanged: i32,
    pub publications_marked_missing: i32,
    pub daily_inserted: i32,
    pub daily_updated: i32,
    pub daily_unchanged: i32,
    pub daily_marked_missing: i32,
    pub view_reports_appended: i32,
}

pub(super) struct ChangeSet {
    pub accounts: Vec<(Uuid, PlannedAccount)>,
    pub account_refs: HashMap<String, Uuid>,
    pub publications: Vec<(Uuid, PlannedPublication)>,
    pub publication_refs: HashMap<String, Uuid>,
    pub view_reports: Vec<(Uuid, Option<i64>)>,
    pub daily: Vec<(Uuid, PlannedDaily)>,
    pub counts: ChangeCounts,
}

pub(super) fn diff(plan: &ImportPlan, existing: &ExistingLedger) -> ChangeSet {
    let mut counts = ChangeCounts::default();
    let (accounts, account_refs) = diff_accounts(plan, existing, &mut counts);
    let (publications, publication_refs) = diff_publications(plan, existing, &mut counts);
    let view_reports = diff_views(plan, existing, &publication_refs, &mut counts);
    let daily = diff_daily(plan, existing, &mut counts);
    ChangeSet {
        accounts,
        account_refs,
        publications,
        publication_refs,
        view_reports,
        daily,
        counts,
    }
}

fn diff_accounts(
    plan: &ImportPlan,
    existing: &ExistingLedger,
    counts: &mut ChangeCounts,
) -> (Vec<(Uuid, PlannedAccount)>, HashMap<String, Uuid>) {
    let mut changes = Vec::new();
    let mut refs: HashMap<String, Uuid> = existing
        .accounts
        .iter()
        .map(|(id, (r, _))| (id.clone(), *r))
        .collect();
    for planned in &plan.accounts {
        match existing.accounts.get(&planned.source_record_id) {
            None => {
                let account_ref = Uuid::new_v4();
                refs.insert(planned.source_record_id.clone(), account_ref);
                changes.push((account_ref, planned.clone()));
                counts.accounts_inserted += 1;
            }
            Some((account_ref, stored)) => {
                // A rebuilt account keeps its last known workbench fields; only presence changes.
                let desired = if planned.ledger_state == LedgerState::AbsentFromExport {
                    PlannedAccount {
                        ledger_state: LedgerState::AbsentFromExport,
                        ..stored.clone()
                    }
                } else {
                    planned.clone()
                };
                tally(
                    record(&mut changes, *account_ref, stored, desired),
                    &mut counts.accounts_updated,
                    &mut counts.accounts_unchanged,
                );
            }
        }
    }
    let planned: std::collections::HashSet<&str> = plan
        .accounts
        .iter()
        .map(|account| account.source_record_id.as_str())
        .collect();
    for (id, (account_ref, stored)) in &existing.accounts {
        if !planned.contains(id.as_str()) {
            let desired = PlannedAccount {
                ledger_state: LedgerState::AbsentFromExport,
                ..stored.clone()
            };
            tally(
                record(&mut changes, *account_ref, stored, desired),
                &mut counts.accounts_updated,
                &mut counts.accounts_unchanged,
            );
        }
    }
    (changes, refs)
}

fn diff_publications(
    plan: &ImportPlan,
    existing: &ExistingLedger,
    counts: &mut ChangeCounts,
) -> (Vec<(Uuid, PlannedPublication)>, HashMap<String, Uuid>) {
    let mut changes = Vec::new();
    let mut refs: HashMap<String, Uuid> = existing
        .publications
        .iter()
        .map(|(id, (r, _))| (id.clone(), *r))
        .collect();
    for planned in &plan.publications {
        match existing.publications.get(&planned.source_record_id) {
            None => {
                let publication_ref = Uuid::new_v4();
                refs.insert(planned.source_record_id.clone(), publication_ref);
                changes.push((publication_ref, planned.clone()));
                counts.publications_inserted += 1;
            }
            Some((publication_ref, stored)) => {
                tally(
                    record(&mut changes, *publication_ref, stored, planned.clone()),
                    &mut counts.publications_updated,
                    &mut counts.publications_unchanged,
                );
            }
        }
    }
    let planned: std::collections::HashSet<&str> = plan
        .publications
        .iter()
        .map(|publication| publication.source_record_id.as_str())
        .collect();
    for (id, (publication_ref, stored)) in &existing.publications {
        if !planned.contains(id.as_str()) && !stored.missing {
            let desired = PlannedPublication {
                missing: true,
                merged_into_source_id: None,
                ..stored.clone()
            };
            changes.push((*publication_ref, desired));
            counts.publications_marked_missing += 1;
        } else if !planned.contains(id.as_str()) {
            counts.publications_unchanged += 1;
        }
    }
    (changes, refs)
}

/// Rule `ops-views.v1`: a report is appended only when the export's value differs from the
/// latest report; a value that disappears is reported as cleared.
fn diff_views(
    plan: &ImportPlan,
    existing: &ExistingLedger,
    refs: &HashMap<String, Uuid>,
    counts: &mut ChangeCounts,
) -> Vec<(Uuid, Option<i64>)> {
    let mut reports = Vec::new();
    for publication in &plan.publications {
        let current = plan
            .view_counts
            .get(&publication.source_record_id)
            .copied()
            .flatten();
        let latest = existing.latest_views.get(&publication.source_record_id);
        let append = match (latest, current) {
            (None, Some(_)) => true,
            (
                Some(LatestView {
                    view_count: Some(old),
                }),
                Some(new),
            ) => *old != new,
            (
                Some(LatestView {
                    view_count: Some(_),
                }),
                None,
            ) => true,
            (Some(LatestView { view_count: None }), Some(_)) => true,
            (None | Some(LatestView { view_count: None }), None) => false,
        };
        if append {
            reports.push((refs[&publication.source_record_id], current));
            counts.view_reports_appended += 1;
        }
    }
    reports
}

fn diff_daily(
    plan: &ImportPlan,
    existing: &ExistingLedger,
    counts: &mut ChangeCounts,
) -> Vec<(Uuid, PlannedDaily)> {
    let mut changes = Vec::new();
    for planned in &plan.daily {
        match existing.daily.get(&planned.source_record_id) {
            None => {
                changes.push((Uuid::new_v4(), planned.clone()));
                counts.daily_inserted += 1;
            }
            Some((daily_ref, stored)) => {
                tally(
                    record(&mut changes, *daily_ref, stored, planned.clone()),
                    &mut counts.daily_updated,
                    &mut counts.daily_unchanged,
                );
            }
        }
    }
    let planned: std::collections::HashSet<&str> = plan
        .daily
        .iter()
        .map(|row| row.source_record_id.as_str())
        .collect();
    for (id, (daily_ref, stored)) in &existing.daily {
        if !planned.contains(id.as_str()) && !stored.missing {
            changes.push((
                *daily_ref,
                PlannedDaily {
                    missing: true,
                    ..stored.clone()
                },
            ));
            counts.daily_marked_missing += 1;
        } else if !planned.contains(id.as_str()) {
            counts.daily_unchanged += 1;
        }
    }
    changes
}

/// Queues `desired` when it differs from what is stored; returns whether it did.
fn record<T: PartialEq>(
    changes: &mut Vec<(Uuid, T)>,
    row_ref: Uuid,
    stored: &T,
    desired: T,
) -> bool {
    let changed = *stored != desired;
    if changed {
        changes.push((row_ref, desired));
    }
    changed
}

fn tally(changed: bool, updated: &mut i32, unchanged: &mut i32) {
    *if changed { updated } else { unchanged } += 1;
}
