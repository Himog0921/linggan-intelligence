//! Splitting the selected comments into research units. Pure: no database, no clock.
//!
//! A unit holds the comments of one work, so the L1 context is shared and never crosses works.
//! Targets of one thread stay adjacent. A comment whose earlier unit was cut off by the output
//! limit goes into smaller units next time, halving on each failure down to one.
use crate::{comment_study_source::StudySourceRole, research_text::content_hash};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Initial per-unit ceiling; P0 measures the real output size and may move it.
pub const UNIT_MAX_TARGETS: usize = 20;
/// Planning estimate of one target's output, in tokens.
pub const OUTPUT_TOKENS_PER_TARGET: usize = 300;
pub const UNIT_MAX_OUTPUT_ESTIMATE: usize = 8_000;

/// How many targets one unit may hold after `truncations` earlier cut-offs.
pub fn capacity(truncations: u32) -> usize {
    let base = UNIT_MAX_TARGETS.min(UNIT_MAX_OUTPUT_ESTIMATE / OUTPUT_TOKENS_PER_TARGET);
    (base >> truncations.min(16)).max(1)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedTarget {
    pub source_ref: Uuid,
    pub role: StudySourceRole,
    pub comment_external_id: String,
    pub thread_key: String,
    pub input_hash: String,
    pub input_fingerprint: String,
    pub truncations: u32,
}

/// The selected, researchable comments of one work and the L1 they will be read against.
#[derive(Debug, Clone)]
pub struct WorkGroup {
    pub work_ref: Uuid,
    pub l1_request_ref: Uuid,
    pub targets: Vec<PlannedTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedUnit {
    pub unit_ref: Uuid,
    pub work_ref: Uuid,
    pub l1_request_ref: Uuid,
    pub targets: Vec<PlannedTarget>,
}

/// Units in execution order: works with more comments first, so the busiest work writes the
/// catalog that smaller works are then read against.
pub fn split(mut groups: Vec<WorkGroup>) -> Vec<PlannedUnit> {
    groups.retain(|group| !group.targets.is_empty());
    groups.sort_by(|a, b| {
        b.targets
            .len()
            .cmp(&a.targets.len())
            .then(a.work_ref.cmp(&b.work_ref))
    });
    let mut units = Vec::new();
    for group in groups {
        let mut by_truncations: BTreeMap<u32, Vec<PlannedTarget>> = BTreeMap::new();
        for target in group.targets {
            by_truncations
                .entry(target.truncations)
                .or_default()
                .push(target);
        }
        for (truncations, mut targets) in by_truncations {
            targets.sort_by(|a, b| {
                (&a.thread_key, &a.comment_external_id)
                    .cmp(&(&b.thread_key, &b.comment_external_id))
            });
            for chunk in targets.chunks(capacity(truncations)) {
                units.push(PlannedUnit {
                    unit_ref: Uuid::new_v4(),
                    work_ref: group.work_ref,
                    l1_request_ref: group.l1_request_ref,
                    targets: chunk.to_vec(),
                });
            }
        }
    }
    units
}

/// Fingerprint of a plan that does not depend on the random unit refs, so a preview and the start
/// that follows it agree exactly when they would create the same units from the same inputs.
pub fn plan_hash(as_of: &str, units: &[PlannedUnit]) -> String {
    let shape: Vec<_> = units
        .iter()
        .map(|unit| {
            json!({
                "workRef": unit.work_ref,
                "l1RequestRef": unit.l1_request_ref,
                "targets": unit.targets.iter().map(|t| json!([t.source_ref, t.role.as_str(), t.input_hash, t.input_fingerprint])).collect::<Vec<_>>(),
            })
        })
        .collect();
    content_hash(
        &json!({"contract":"comment-understanding.plan.v1","asOf":as_of,"units":shape}).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(n: u128, thread: &str, truncations: u32) -> PlannedTarget {
        PlannedTarget {
            source_ref: Uuid::from_u128(n),
            role: StudySourceRole::Primary,
            comment_external_id: format!("c{n:03}"),
            thread_key: thread.into(),
            input_hash: format!("{n:064x}"),
            input_fingerprint: format!("{:064x}", n + 1000),
            truncations,
        }
    }

    fn group(work: u128, targets: Vec<PlannedTarget>) -> WorkGroup {
        WorkGroup {
            work_ref: Uuid::from_u128(work),
            l1_request_ref: Uuid::from_u128(work + 500),
            targets,
        }
    }

    #[test]
    fn a_unit_never_mixes_works_and_respects_the_ceiling() {
        let big = group(1, (0..45).map(|n| target(n, "t", 0)).collect());
        let small = group(2, (100..103).map(|n| target(n, "t", 0)).collect());
        let units = split(vec![small, big]);
        let sizes: Vec<_> = units
            .iter()
            .map(|u| (u.work_ref.as_u128(), u.targets.len()))
            .collect();
        assert_eq!(sizes, [(1, 20), (1, 20), (1, 5), (2, 3)]);
        assert!(
            units
                .iter()
                .all(|u| u.l1_request_ref.as_u128() == u.work_ref.as_u128() + 500)
        );
    }

    #[test]
    fn threads_stay_adjacent_inside_a_work() {
        let mut targets = vec![
            target(3, "b", 0),
            target(1, "a", 0),
            target(4, "b", 0),
            target(2, "a", 0),
        ];
        targets.reverse();
        let units = split(vec![group(1, targets)]);
        let order: Vec<_> = units[0]
            .targets
            .iter()
            .map(|t| t.comment_external_id.as_str())
            .collect();
        assert_eq!(order, ["c001", "c002", "c003", "c004"]);
    }

    #[test]
    fn earlier_truncation_halves_the_unit_down_to_one() {
        assert_eq!(
            [
                capacity(0),
                capacity(1),
                capacity(2),
                capacity(3),
                capacity(4),
                capacity(9)
            ],
            [20, 10, 5, 2, 1, 1]
        );
        let targets = (0..30)
            .map(|n| target(n, "t", if n < 8 { 1 } else { 0 }))
            .collect();
        let sizes: Vec<_> = split(vec![group(1, targets)])
            .iter()
            .map(|u| u.targets.len())
            .collect();
        // 22 untouched comments in 20+2, the 8 that were cut off in 8 (<= 10).
        assert_eq!(sizes, [20, 2, 8]);
    }

    #[test]
    fn plan_hash_ignores_unit_refs_and_changes_with_any_input() {
        let make = || split(vec![group(1, vec![target(1, "t", 0), target(2, "t", 0)])]);
        assert_eq!(
            plan_hash("2026-11-19T00:00:00Z", &make()),
            plan_hash("2026-11-19T00:00:00Z", &make())
        );
        assert_ne!(
            plan_hash("2026-11-19T00:00:00Z", &make()),
            plan_hash("2026-11-19T00:00:01Z", &make())
        );
        let mut changed = make();
        changed[0].targets[0].input_fingerprint = format!("{:064x}", 9);
        assert_ne!(
            plan_hash("2026-11-19T00:00:00Z", &make()),
            plan_hash("2026-11-19T00:00:00Z", &changed)
        );
    }

    #[test]
    fn empty_groups_make_no_units() {
        assert!(split(vec![group(1, vec![])]).is_empty());
    }
}
