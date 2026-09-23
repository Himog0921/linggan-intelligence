//! User-confirmed StudyRun preparation over a frozen, domain-qualified source set.
//!
//! A caller chooses works; the database rechecks policy, material qualification and the comment
//! budget in one repeatable-read transaction. This creates no provider invocation and does not
//! silently add an unselected work.

use crate::comment_study_source::{StudySource, StudySourceError, eligible_sources_in_transaction};
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

const MAX_SELECTED_WORKS: usize = 100;

#[derive(Debug, Clone)]
pub struct PrepareStudyRunRequest {
    pub content_public_refs: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedStudyRun {
    pub run_ref: Uuid,
    pub as_of: String,
    pub selected_work_count: usize,
    pub covered_work_count: usize,
    pub target_count: usize,
    pub needs_context_count: usize,
    pub comment_budget: i32,
}

#[derive(Debug, Error)]
pub enum PrepareStudyRunError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Source(#[from] StudySourceError),
    #[error("at least one and at most 100 works must be selected")]
    InvalidSelection,
    #[error("no active Study policy is configured")]
    PolicyMissing,
    #[error("the selected works have no currently eligible comments")]
    NoEligibleSources,
}

struct ActivePolicy {
    policy_ref: Uuid,
    domain_ref: Uuid,
    comment_budget: i32,
    context_character_budget: i32,
}

/// Revalidates a user selection and freezes only its currently eligible source comments.
pub async fn prepare_study_run(
    database: &Database,
    request: PrepareStudyRunRequest,
) -> Result<PreparedStudyRun, PrepareStudyRunError> {
    let selected = normalized_work_selection(request.content_public_refs)?;
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *transaction)
        .await?;
    let policy = read_active_policy(&mut transaction).await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *transaction)
        .await?;
    let sources = eligible_sources_in_transaction(
        &mut transaction,
        policy.domain_ref,
        &as_of,
        &selected,
        i64::from(policy.comment_budget),
    )
    .await?;
    if sources.is_empty() {
        return Err(PrepareStudyRunError::NoEligibleSources);
    }
    let run_ref = Uuid::new_v4();
    let grouped = group_sources(sources);
    insert_run(
        &mut transaction,
        run_ref,
        &policy,
        &as_of,
        &selected,
        &grouped,
    )
    .await?;
    let needs_context_count =
        insert_work_and_targets(&mut transaction, run_ref, &policy, grouped).await?;
    transaction.commit().await?;
    Ok(PreparedStudyRun {
        run_ref,
        as_of,
        selected_work_count: selected.len(),
        covered_work_count: count_covered_works(database, run_ref).await?,
        target_count: count_targets(database, run_ref).await?,
        needs_context_count,
        comment_budget: policy.comment_budget,
    })
}

fn normalized_work_selection(values: Vec<Uuid>) -> Result<Vec<Uuid>, PrepareStudyRunError> {
    let selected: Vec<Uuid> = values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if selected.is_empty() || selected.len() > MAX_SELECTED_WORKS {
        return Err(PrepareStudyRunError::InvalidSelection);
    }
    Ok(selected)
}

async fn read_active_policy(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<ActivePolicy, PrepareStudyRunError> {
    let row = sqlx::query(
        "SELECT policy.policy_ref,policy.domain_ref,policy.comment_budget,policy.context_character_budget \
         FROM linggan_comment_study_active_policy active \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE active.singleton FOR SHARE OF active",
    )
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PrepareStudyRunError::PolicyMissing)?;
    Ok(ActivePolicy {
        policy_ref: row.get("policy_ref"),
        domain_ref: row.get("domain_ref"),
        comment_budget: row.get("comment_budget"),
        context_character_budget: row.get("context_character_budget"),
    })
}

fn group_sources(sources: Vec<StudySource>) -> BTreeMap<Uuid, Vec<StudySource>> {
    let mut grouped = BTreeMap::new();
    for source in sources {
        grouped
            .entry(source.content_public_ref)
            .or_insert_with(Vec::new)
            .push(source);
    }
    grouped
}

async fn insert_run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    policy: &ActivePolicy,
    as_of: &str,
    selected: &[Uuid],
    grouped: &BTreeMap<Uuid, Vec<StudySource>>,
) -> Result<(), sqlx::Error> {
    let selected_sources: Vec<Uuid> = grouped
        .values()
        .flat_map(|sources| sources.iter().map(|source| source.source_ref))
        .collect();
    let manifest = json!({
        "contract":"comment-study.run-selection.v1",
        "requestedWorkRefs":selected,
        "coveredWorkRefs":grouped.keys().collect::<Vec<_>>(),
        "targetSourceRefs":selected_sources,
        "commentBudget":policy.comment_budget
    });
    sqlx::query(
        "INSERT INTO linggan_comment_study_run( \
           run_ref,policy_ref,as_of,state,selection_manifest,selection_hash \
         ) VALUES($1,$2,$3::timestamptz,'prepared',$4,$5)",
    )
    .bind(run_ref)
    .bind(policy.policy_ref)
    .bind(as_of)
    .bind(&manifest)
    .bind(hash_value(&manifest))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_work_and_targets(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    policy: &ActivePolicy,
    grouped: BTreeMap<Uuid, Vec<StudySource>>,
) -> Result<usize, sqlx::Error> {
    let mut needs_context_count = 0;
    for (work_ref, sources) in grouped {
        let context_manifest = bounded_context_manifest(
            &sources[0].context_manifest,
            usize::try_from(policy.context_character_budget).expect("policy budget is positive"),
        );
        let context_state = context_state(&context_manifest);
        insert_work(
            transaction,
            run_ref,
            work_ref,
            policy.domain_ref,
            context_state,
            &context_manifest,
        )
        .await?;
        for source in sources {
            needs_context_count += usize::from(
                insert_target(transaction, run_ref, work_ref, source, &context_manifest).await?,
            );
        }
    }
    Ok(needs_context_count)
}

async fn insert_work(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    work_ref: Uuid,
    domain_ref: Uuid,
    context_state: &str,
    context_manifest: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO linggan_comment_study_work( \
           run_ref,content_public_ref,domain_ref,selection_reason,context_state,context_manifest,context_hash \
         ) VALUES($1,$2,$3,'user_selected',$4,$5,$6)",
    )
    .bind(run_ref)
    .bind(work_ref)
    .bind(domain_ref)
    .bind(context_state)
    .bind(context_manifest)
    .bind(hash_value(context_manifest))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Closes a run once none of its frozen targets can still advance on their own.
///
/// Nothing in the codebase ever wrote `completed`, `completed_with_failures` or `cancelled`,
/// though the schema has defined all three from the start: the only statement that set a run's
/// state set it to `running`. Every run that ever started therefore stayed `running` for good,
/// including runs whose every target had long since reached a terminal state.
///
/// Which terminal state a run takes describes its material, not how hard the system tried. A run
/// whose targets were all excluded never had anything left to study and is `cancelled`; one that
/// lost some targets to failure or exclusion is `completed_with_failures`; one that resolved every
/// target is `completed` — `needs_context` counts as resolved, because a named material gap is an
/// honest research outcome rather than a failure, and re-studying it is a later run's job.
///
/// `ready` has no producer today, but it is counted as unsettled anyway: for a state the schema
/// allows, erring towards leaving a run open is recoverable, while closing one that still holds
/// work is not.
pub(crate) async fn close_run_if_settled(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<Option<&'static str>, sqlx::Error> {
    let open: Option<String> = sqlx::query_scalar(
        "SELECT state FROM linggan_comment_study_run \
         WHERE run_ref=$1 AND finished_at IS NULL FOR UPDATE",
    )
    .bind(run_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    if open.is_none() {
        return Ok(None);
    }
    let counts = sqlx::query(
        "SELECT count(*) FILTER (WHERE state IN ('ready','queued','running')) AS unsettled, \
                count(*) FILTER (WHERE state='failed') AS failed, \
                count(*) FILTER (WHERE state='excluded') AS excluded, \
                count(*) AS total \
         FROM linggan_comment_study_target WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(&mut **transaction)
    .await?;
    let unsettled: i64 = counts.get("unsettled");
    let total: i64 = counts.get("total");
    if unsettled > 0 || total == 0 {
        return Ok(None);
    }
    let failed: i64 = counts.get("failed");
    let excluded: i64 = counts.get("excluded");
    let state = if excluded == total {
        "cancelled"
    } else if failed > 0 || excluded > 0 {
        "completed_with_failures"
    } else {
        "completed"
    };
    sqlx::query(
        "UPDATE linggan_comment_study_run SET state=$2,finished_at=scope_001_now() \
         WHERE run_ref=$1 AND finished_at IS NULL",
    )
    .bind(run_ref)
    .bind(state)
    .execute(&mut **transaction)
    .await?;
    Ok(Some(state))
}

async fn insert_target(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    work_ref: Uuid,
    source: StudySource,
    work_context: &Value,
) -> Result<bool, sqlx::Error> {
    let (dependency_state, state, parent_context) = target_dependency(&source);
    let input_manifest = json!({
        "contract":"comment-study.target-input.v1",
        "targetSourceRef":source.source_ref,
        "workContext":work_context,
        "parentContext":parent_context
    });
    sqlx::query(
        "INSERT INTO linggan_comment_study_target( \
           target_ref,run_ref,content_public_ref,source_ref,parent_source_ref,research_text, \
           research_sha256,dependency_state,state,input_manifest,input_hash \
         ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(Uuid::new_v4())
    .bind(run_ref)
    .bind(work_ref)
    .bind(source.source_ref)
    .bind(source.parent_source_ref)
    .bind(&source.research_text)
    .bind(hash_text(&source.research_text))
    .bind(dependency_state)
    .bind(state)
    .bind(&input_manifest)
    .bind(hash_value(&input_manifest))
    .execute(&mut **transaction)
    .await?;
    Ok(state == "needs_context")
}

fn target_dependency(source: &StudySource) -> (&'static str, &'static str, Value) {
    if source.clean_state == "direct" {
        return ("self_contained", "queued", Value::Null);
    }
    match (&source.parent_source_ref, &source.parent_research_text) {
        (Some(source_ref), Some(text)) => (
            "parent_available",
            "queued",
            json!({"sourceRef":source_ref,"researchText":text}),
        ),
        _ => (
            "parent_required_missing",
            "needs_context",
            json!({"state":"missing"}),
        ),
    }
}

fn context_state(manifest: &Value) -> &'static str {
    let has_sources = manifest["sources"]
        .as_array()
        .is_some_and(|sources| !sources.is_empty());
    if manifest["truncated"].as_bool().unwrap_or(false) {
        "partial"
    } else if has_sources {
        "ready"
    } else {
        "missing"
    }
}

/// Makes the configured context-character cap part of the frozen work contract.  Fragments are
/// retained whole (never mid-quote) in a stable relevance order, while omitted material is
/// explicitly recorded so a model cannot mistake partial context for a complete work record.
pub(crate) fn bounded_context_manifest(manifest: &Value, budget: usize) -> Value {
    let mut fragments: Vec<Value> = manifest["sources"].as_array().cloned().unwrap_or_default();
    fragments.sort_by_key(sort_key);
    let mut used = 0_usize;
    let mut retained = Vec::new();
    // A caller may apply a smaller budget to an already bounded source projection.
    // Earlier omissions remain auditable; re-bounding must never declare that input complete.
    let mut omitted = manifest["omitted"].as_array().cloned().unwrap_or_default();
    for fragment in fragments {
        let length = fragment["characterCount"].as_u64().map(|n| n as usize)
            .unwrap_or_else(|| fragment["text"].as_str().map_or(0, |text| text.chars().count()));
        if fragment["text"].is_string() && used.saturating_add(length) <= budget {
            used += length;
            retained.push(fragment);
        } else {
            // What was cut has to be nameable. A bare count cannot tell a caller whether the
            // budget dropped a stray caption or the whole second half of a work.
            omitted.push(json!({
                "kind":fragment["kind"],
                "sourceRef":fragment["sourceRef"],
                "slotOrdinal":fragment["slotOrdinal"],
                "characterCount":length
            }));
        }
    }
    json!({
        "contract":"comment-study.context.v1",
        "workRef":manifest["workRef"],
        "cleanerVersion":manifest["cleanerVersion"],
        "characterBudget":budget,
        "includedCharacterCount":used,
        "orderBasis":ORDER_BASIS,
        "omittedFragmentCount":omitted.len(),
        "omitted":omitted,
        "truncated":!omitted.is_empty(),
        "sources":retained
    })
}

/// Media fragments carry `slotOrdinal`, which is the work's own display order for its images and
/// video. Nothing in `source_location` records a position *inside* a slot — it holds only a blob
/// digest and a processor version — so two texts from one slot have no knowable reading order and
/// are merely kept deterministic by their source reference. The basis travels with the manifest so
/// a reader never mistakes that tiebreak for the order the work is actually read in.
const ORDER_BASIS: &str = "native_title,body,slot_ordinal,stable_source_ref";

fn sort_key(fragment: &Value) -> (u8, i64, String) {
    let priority = match fragment["kind"].as_str().unwrap_or("") {
        "native_title" => 0,
        "body" => 1,
        _ => 2,
    };
    // A slot whose ordinal the producer never established sorts after every known one rather than
    // ahead of slot 1, which is where a missing value would otherwise land.
    let slot_ordinal = fragment["slotOrdinal"].as_i64().unwrap_or(i64::MAX);
    (priority, slot_ordinal, fragment["sourceRef"].to_string())
}

fn hash_text(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hash_value(value: &Value) -> String {
    hash_text(&serde_json::to_string(value).expect("JSON values are serializable"))
}

async fn count_covered_works(database: &Database, run_ref: Uuid) -> Result<usize, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_work WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(database.pool())
            .await?;
    Ok(usize::try_from(count).unwrap_or(0))
}

async fn count_targets(database: &Database, run_ref: Uuid) -> Result<usize, sqlx::Error> {
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1")
            .bind(run_ref)
            .fetch_one(database.pool())
            .await?;
    Ok(usize::try_from(count).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_cap_keeps_whole_high_priority_fragments_and_marks_the_omission() {
        let manifest = json!({
            "contract":"comment-study.context.v1",
            "workRef":"00000000-0000-4000-8000-000000000010",
            "cleanerVersion":"test",
            "sources":[
                {"kind":"transcript","sourceRef":"t1","text":"123456"},
                {"kind":"body","sourceRef":"b1","text":"1234"},
                {"kind":"native_title","sourceRef":"n1","text":"12"}
            ]
        });
        let bounded = bounded_context_manifest(&manifest, 6);
        assert_eq!(bounded["includedCharacterCount"], 6);
        assert_eq!(bounded["omittedFragmentCount"], 1);
        assert_eq!(bounded["truncated"], true);
        assert_eq!(bounded["sources"][0]["kind"], "native_title");
        assert_eq!(bounded["sources"][1]["kind"], "body");
        assert_eq!(context_state(&bounded), "partial");
    }

    #[test]
    fn rebounding_preserves_prior_omission_metadata() {
        let manifest=json!({"sources":[{"kind":"body","sourceRef":"body","text":null,"characterCount":21000},
            {"kind":"native_title","sourceRef":"title","text":"ABC","characterCount":3}]});
        let first=bounded_context_manifest(&manifest,20000);
        let second=bounded_context_manifest(&first,2);
        assert_eq!(second["omittedFragmentCount"],2);
        assert_eq!(second["truncated"],true);
        assert_eq!(second["includedCharacterCount"],0);
    }

    #[test]
    fn media_fragments_follow_the_works_slot_order_not_their_reference_order() {
        // The references sort in the exact reverse of the slot order, so ordering by reference —
        // as the manifest used to — hands the model the work's images backwards. References that
        // happened to sort the same way as the slots would let this assertion pass either way.
        let manifest = json!({
            "contract":"comment-study.context.v1",
            "workRef":"00000000-0000-4000-8000-000000000010",
            "cleanerVersion":"test",
            "sources":[
                {"kind":"ocr_text","sourceRef":"a-gamma","text":"三","slotOrdinal":3},
                {"kind":"ocr_text","sourceRef":"z-alpha","text":"一","slotOrdinal":1},
                {"kind":"ocr_text","sourceRef":"m-beta","text":"二","slotOrdinal":2}
            ]
        });
        let bounded = bounded_context_manifest(&manifest, 100);
        let order: Vec<&str> = bounded["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|fragment| fragment["text"].as_str().unwrap())
            .collect();
        assert_eq!(order, vec!["一", "二", "三"]);
        assert_eq!(bounded["orderBasis"], ORDER_BASIS);
    }

    #[test]
    fn a_slot_without_a_known_ordinal_sorts_after_the_known_ones() {
        let manifest = json!({
            "contract":"comment-study.context.v1",
            "workRef":"00000000-0000-4000-8000-000000000010",
            "cleanerVersion":"test",
            "sources":[
                {"kind":"ocr_text","sourceRef":"a-unknown","text":"未知"},
                {"kind":"ocr_text","sourceRef":"z-second","text":"二","slotOrdinal":2}
            ]
        });
        let bounded = bounded_context_manifest(&manifest, 100);
        let order: Vec<&str> = bounded["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|fragment| fragment["text"].as_str().unwrap())
            .collect();
        assert_eq!(order, vec!["二", "未知"]);
    }

    #[test]
    fn an_omitted_fragment_is_named_rather_than_only_counted() {
        let manifest = json!({
            "contract":"comment-study.context.v1",
            "workRef":"00000000-0000-4000-8000-000000000010",
            "cleanerVersion":"test",
            "sources":[
                {"kind":"native_title","sourceRef":"n1","text":"12"},
                {"kind":"ocr_text","sourceRef":"late","text":"3456","slotOrdinal":9}
            ]
        });
        let bounded = bounded_context_manifest(&manifest, 2);
        assert_eq!(bounded["omittedFragmentCount"], 1);
        assert_eq!(
            bounded["omitted"],
            json!([{"kind":"ocr_text","sourceRef":"late","slotOrdinal":9,"characterCount":4}])
        );
    }
}
