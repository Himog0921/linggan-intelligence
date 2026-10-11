//! Qualified incremental source admission before bounded window packing.
use super::ResearchInput;
use super::queue::{attach_legacy_media_aliases, overlaps_unknown_dispatch};
use super::windows::{is_research_evidence, origin_id};
use crate::topic_map_research_analysis::METHOD_VERSION;
use serde_json::Value;
use sqlx::Row;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Reuse frozen source ranges before packing. A newly acquired comment cannot
/// reshape and resend the old batch, and changed required context restores no
/// reusable ranges. Automatic failures retain the explicit-retry boundary.
pub(super) async fn pending_sources(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    scope: &[Uuid],
    inputs: &[ResearchInput],
    trigger: &str,
) -> Result<Vec<ResearchInput>, sqlx::Error> {
    let mut history = BTreeMap::<Uuid, Vec<Value>>::new();
    let mut unknown_refs = Vec::new();
    for works in scope.chunks(500) {
        let rows = sqlx::query("SELECT input_refs FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND state='unknown_dispatch' AND work_public_ref=ANY($2)")
            .bind(domain).bind(works).fetch_all(&mut **tx).await?;
        for row in rows {
            let wrapper: Value = row.get("input_refs");
            unknown_refs.extend(
                wrapper.get("source").unwrap_or(&wrapper)["fragments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        let rows = sqlx::query("SELECT t.work_public_ref,t.input_refs FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref) WHERE t.domain_ref=$1 AND t.work_public_ref=ANY($2) AND r.method_version=$3 AND NOT (t.recall_manifest ? 'backfill') AND COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}','source')<>'comparison' AND (t.state IN ('queued','running','succeeded','no_signal','insufficient','unknown_dispatch') OR ($4<>'on_demand' AND t.state IN ('failed','stopped')))")
            .bind(domain).bind(works).bind(METHOD_VERSION).bind(trigger).fetch_all(&mut **tx).await?;
        for row in rows {
            history
                .entry(row.get("work_public_ref"))
                .or_default()
                .push(row.get("input_refs"));
        }
    }
    attach_legacy_media_aliases(tx, &mut unknown_refs).await?;
    Ok(inputs
        .iter()
        .filter(|i| scope.contains(&i.work.work_ref))
        .map(|full| {
            pending_input(
                full,
                history.get(&full.work.work_ref).map_or(&[], Vec::as_slice),
                &unknown_refs,
            )
        })
        .collect())
}

/// Keep immutable unknown-send exclusions separate from reusable completed
/// ranges: missing parent pages may reopen a source, never an unknown dispatch.
pub(super) fn pending_input(
    full: &ResearchInput,
    history: &[Value],
    unknown_refs: &[Value],
) -> ResearchInput {
    let mut spans = BTreeMap::<String, Vec<(usize, usize)>>::new();
    let mut contexts = BTreeMap::<String, Vec<(usize, usize)>>::new();
    for wrapper in history {
        let manifest = wrapper.get("source").unwrap_or(wrapper);
        if let Some(restored) = super::restore_window(full, manifest) {
            for f in &restored.fragments {
                let map = if is_research_evidence(f) {
                    &mut spans
                } else {
                    &mut contexts
                };
                map.entry(origin_id(&restored, f))
                    .or_default()
                    .push((f.start, f.end));
            }
        }
    }
    let forbidden = forbidden_ranges(full, unknown_refs);
    let mut pending = full.clone();
    pending.fragments = full
        .fragments
        .iter()
        .flat_map(|source| {
            if !is_research_evidence(source) {
                return vec![source.clone()];
            }
            // A repeated child must remain pending until every required parent
            // context page has been processed; child ranges alone are insufficient.
            let parent = full
                .comment_study
                .as_array()
                .into_iter()
                .flatten()
                .find(|c| {
                    c["sourceFragmentId"]
                        .as_str()
                        .or_else(|| c["fragmentId"].as_str())
                        == Some(source.fragment_id.as_str())
                })
                .and_then(|c| c["parentFragmentId"].as_str())
                .and_then(|id| full.fragments.iter().find(|f| f.fragment_id == id));
            let parent_pending = parent.is_some_and(|p| {
                !uncovered_ranges(
                    p.start,
                    p.end,
                    contexts.get(&p.fragment_id).map_or(&[], Vec::as_slice),
                )
                .is_empty()
            });
            let mut excluded = forbidden
                .get(&source.fragment_id)
                .cloned()
                .unwrap_or_default();
            if !parent_pending {
                excluded.extend(
                    spans
                        .get(&source.fragment_id)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            uncovered_ranges(source.start, source.end, &excluded)
                .into_iter()
                .map(|(start, end)| {
                    let mut f = source.clone();
                    f.start = start;
                    f.end = end;
                    f.text = source
                        .text
                        .chars()
                        .skip(start - source.start)
                        .take(end - start)
                        .collect();
                    f
                })
                .collect()
        })
        .collect();
    pending
}

fn forbidden_ranges(
    full: &ResearchInput,
    unknown_refs: &[Value],
) -> BTreeMap<String, Vec<(usize, usize)>> {
    let mut forbidden = BTreeMap::<String, Vec<(usize, usize)>>::new();
    // Remove only hash-proven unknown ranges before packing, so an old
    // uncertain comment cannot block five unrelated new voices in its group.
    for source in full.fragments.iter().filter(|f| is_research_evidence(f)) {
        for reference in unknown_refs {
            let Some((start, end)) = reference["start"].as_u64().zip(reference["end"].as_u64())
            else {
                continue;
            };
            let (start, end) = (start as usize, end as usize);
            if start < source.start || end > source.end || start >= end {
                continue;
            }
            let mut fragment = source.clone();
            fragment.start = start;
            fragment.end = end;
            fragment.text = source
                .text
                .chars()
                .skip(start - source.start)
                .take(end - start)
                .collect();
            let mut single = full.clone();
            single.fragments = vec![fragment];
            if overlaps_unknown_dispatch(&single, full, std::slice::from_ref(reference)) {
                forbidden
                    .entry(source.fragment_id.clone())
                    .or_default()
                    .push((start, end));
            }
        }
    }
    forbidden
}

fn uncovered_ranges(start: usize, end: usize, covered: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut covered = covered.to_vec();
    covered.sort_unstable();
    let mut cursor = start;
    let mut pending = Vec::new();
    for (a, b) in covered {
        if b <= cursor || a >= end {
            continue;
        }
        if a > cursor {
            pending.push((cursor, a.min(end)));
        }
        cursor = cursor.max(b.min(end));
    }
    if cursor < end {
        pending.push((cursor, end));
    }
    pending
}

#[cfg(test)]
mod packing_history_tests {
    use super::uncovered_ranges;
    #[test]
    fn overlapping_prior_ranges_cannot_resend_covered_text_or_hide_tail() {
        assert_eq!(
            uncovered_ranges(0, 20, &[(8, 12), (0, 5), (3, 10), (17, 25)]),
            vec![(12, 17)]
        );
        assert_eq!(
            uncovered_ranges(10, 20, &[(0, 9), (20, 30)]),
            vec![(10, 20)]
        );
        assert!(uncovered_ranges(0, 10, &[(0, 10)]).is_empty());
    }
}
