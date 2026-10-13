//! Rule `ops-dedupe.v1`: registrations of the same post are grouped and one record is kept.
//! A group is linked by an equal parsed content id or an exactly equal canonical link on the same
//! platform. Merged records stay in the ledger unchanged except for their merge target.

use std::cmp::Reverse;
use std::collections::HashMap;

use super::export_rows::Platform;

/// What the rule needs to know about one registration.
pub(super) struct DedupeInput<'a> {
    pub source_record_id: &'a str,
    pub platform: Platform,
    pub content_id: Option<&'a str>,
    pub canonical_url: &'a str,
    pub has_view_count: bool,
    pub filled_fields: usize,
    pub published_at: &'a str,
    pub created_at: &'a str,
    pub business_date: super::business_time::CivilDate,
    pub view_count: Option<i64>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct DedupeOutcome {
    /// Index of the kept record for every merged index.
    pub merged_into: HashMap<usize, usize>,
    pub groups: usize,
    pub groups_with_conflicting_views: usize,
    pub groups_with_different_dates: usize,
}

pub(super) fn dedupe(records: &[DedupeInput<'_>]) -> DedupeOutcome {
    let mut sets = DisjointSets::new(records.len());
    let mut first_by_key: HashMap<(Platform, &str, &str), usize> = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        let keys = [
            record.content_id.map(|id| (record.platform, "content", id)),
            Some((record.platform, "canonical", record.canonical_url)),
        ];
        for key in keys.into_iter().flatten() {
            match first_by_key.get(&key) {
                Some(&first) => sets.union(first, index),
                None => {
                    first_by_key.insert(key, index);
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..records.len() {
        groups.entry(sets.find(index)).or_default().push(index);
    }
    let mut outcome = DedupeOutcome::default();
    for members in groups.into_values().filter(|members| members.len() > 1) {
        let kept = *members
            .iter()
            .min_by_key(|&&index| survivor_key(&records[index]))
            .expect("a group has members");
        outcome.groups += 1;
        outcome.groups_with_conflicting_views +=
            usize::from(differs(&members, |index| records[index].view_count));
        outcome.groups_with_different_dates += usize::from(differs(&members, |index| {
            Some(records[index].business_date)
        }));
        for index in members.into_iter().filter(|&index| index != kept) {
            outcome.merged_into.insert(index, kept);
        }
    }
    outcome
}

/// Smaller is kept: has a view count, more filled fields, earlier publish time, earlier creation,
/// smaller original record id.
fn survivor_key<'a>(record: &DedupeInput<'a>) -> (bool, Reverse<usize>, &'a str, &'a str, &'a str) {
    (
        !record.has_view_count,
        Reverse(record.filled_fields),
        record.published_at,
        record.created_at,
        record.source_record_id,
    )
}

/// True when the group has at least two different known values.
fn differs<T: PartialEq>(members: &[usize], value: impl Fn(usize) -> Option<T>) -> bool {
    let mut known = members.iter().filter_map(|&index| value(index));
    let Some(first) = known.next() else {
        return false;
    };
    known.any(|other| other != first)
}

struct DisjointSets {
    parent: Vec<usize>,
}

impl DisjointSets {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, index: usize) -> usize {
        let mut root = index;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = index;
        while self.parent[current] != root {
            let next = self.parent[current];
            self.parent[current] = root;
            current = next;
        }
        root
    }

    fn union(&mut self, left: usize, right: usize) {
        let (left, right) = (self.find(left), self.find(right));
        if left != right {
            self.parent[right.max(left)] = right.min(left);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DedupeInput, dedupe};
    use crate::ops_ledger::business_time::CivilDate;
    use crate::ops_ledger::export_rows::Platform;

    fn record<'a>(id: &'a str, content: Option<&'a str>, canonical: &'a str) -> DedupeInput<'a> {
        DedupeInput {
            source_record_id: id,
            platform: Platform::Xhs,
            content_id: content,
            canonical_url: canonical,
            has_view_count: false,
            filled_fields: 0,
            published_at: "2026-09-01T02:00:00.000Z",
            created_at: "2026-09-01T03:00:00.000Z",
            business_date: CivilDate::parse("2026-09-01").unwrap(),
            view_count: None,
        }
    }

    #[test]
    fn same_note_or_same_canonical_link_is_one_post() {
        let mut records = vec![
            record("a", Some("n1"), "https://x/1"),
            record("b", Some("n1"), "https://x/1?other"),
            record("c", None, "https://short/9"),
            record("d", None, "https://short/9"),
            record("e", Some("n2"), "https://x/2"),
        ];
        records[1].has_view_count = true;
        records[1].view_count = Some(10);
        let outcome = dedupe(&records);
        assert_eq!(outcome.groups, 2);
        assert_eq!(
            outcome.merged_into.get(&0),
            Some(&1),
            "the record with a view count is kept"
        );
        assert_eq!(
            outcome.merged_into.get(&3),
            Some(&2),
            "ties keep the smaller original id"
        );
        assert!(!outcome.merged_into.contains_key(&1) && !outcome.merged_into.contains_key(&4));
    }

    #[test]
    fn platforms_never_merge_and_conflicts_are_counted() {
        let mut records = vec![
            record("a", Some("n1"), "https://x/1"),
            record("b", Some("n1"), "https://x/1"),
            record("c", None, "https://x/1"),
        ];
        records[2].platform = Platform::Douyin;
        records[0].view_count = Some(100);
        records[0].has_view_count = true;
        records[1].view_count = Some(200);
        records[1].has_view_count = true;
        records[1].filled_fields = 3;
        records[1].business_date = CivilDate::parse("2026-09-02").unwrap();
        let outcome = dedupe(&records);
        assert_eq!(outcome.groups, 1);
        assert_eq!(
            outcome.merged_into.get(&0),
            Some(&1),
            "more filled fields win after views"
        );
        assert!(
            !outcome.merged_into.contains_key(&2),
            "a Douyin record never joins an XHS group"
        );
        assert_eq!(outcome.groups_with_conflicting_views, 1);
        assert_eq!(outcome.groups_with_different_dates, 1);
    }
}
