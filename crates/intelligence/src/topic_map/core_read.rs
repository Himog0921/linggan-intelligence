//! Current-source projection of completed windows and independently accepted partial decisions.
use super::*;
use crate::topic_map_research::{self, ResearchInput};
use linggan_evidence::creator_discovery::Fragment;
use linggan_storage_postgres::Database;
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};

#[path = "core_read/legacy.rs"]
mod legacy;
#[path = "core_read/load.rs"]
mod load;
#[path = "core_read/partial.rs"]
mod partial;
#[path = "core_read/projection.rs"]
mod projection;
#[path = "core_read/units.rs"]
mod units;
#[path = "core_read/work_summary.rs"]
mod work_summary;

#[derive(Clone)]
struct Window {
    result: Option<Uuid>,
    task: Uuid,
    method: String,
    time: String,
    input: ResearchInput,
    manifest: Value,
    output: Value,
    core: Value,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ComparisonScope {
    scope_work_refs: Vec<Uuid>,
    selected_work_refs: Vec<Uuid>,
    state: String,
    boundary: String,
}

impl ComparisonScope {
    fn from_manifest(manifest: &Value, primary: Uuid) -> Option<Self> {
        let coverage = &manifest["coverage"];
        let scope: Self = serde_json::from_value(coverage.clone()).ok()?;
        let requested: BTreeSet<_> = scope.scope_work_refs.iter().copied().collect();
        let selected: BTreeSet<_> = scope.selected_work_refs.iter().copied().collect();
        let discussions: BTreeSet<Uuid> = coverage["selectedDiscussions"]
            .as_array()?
            .iter()
            .map(|discussion| discussion["workRef"].as_str()?.parse().ok())
            .collect::<Option<_>>()?;
        if !(1..=10).contains(&requested.len())
            || requested.len() != scope.scope_work_refs.len()
            || selected.len() != scope.selected_work_refs.len()
            || !selected.contains(&primary)
            || !selected.is_subset(&requested)
            || selected != discussions
        {
            return None;
        }
        Some(scope)
    }

    fn value(&self, result: Option<Uuid>) -> Value {
        let mut value = json!(self);
        value["resultRef"] = json!(result);
        value
    }
}

impl Window {
    fn is_comparison(&self) -> bool {
        self.manifest["coverage"]["kind"] == "comparison"
    }
    fn is_partial(&self) -> bool {
        self.result.is_none()
    }
}

pub(super) async fn attach_windows(
    db: &Database,
    q: &TopicMapQuery,
    works: &mut [TopicMapWork],
    topics: &mut [TopicMapTopic],
) -> Result<Vec<Value>, TopicMapError> {
    let ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_concept_rule')IS NOT NULL")
            .fetch_one(db.pool())
            .await?;
    if !ready {
        return Ok(vec![]);
    }
    let unavailable = crate::topic_map_core::unavailable_definitions(db, q.domain_ref)
        .await
        .map_err(|e| TopicMapError::Source(e.to_string()))?;
    units::attach_rules(db, q, topics, &unavailable).await?;
    let mut windows = load::load_windows(db, q, works, topics, &unavailable).await?;
    let mut candidates = Vec::new();
    for work in works {
        units::invalidate_annotation(work, &unavailable);
        if let Some(parts) = windows.remove(&work.work_ref) {
            candidates.extend(projection::project_work(work, parts, topics, &unavailable));
        }
    }
    Ok(candidates)
}

/// Counters and relations describe exactly the work set returned by this read request.
pub(super) fn summarize_scope(works: &[TopicMapWork], topics: &mut [TopicMapTopic]) {
    units::summarize_scope(works, topics);
}

#[cfg(test)]
#[path = "core_read/tests.rs"]
mod tests;
