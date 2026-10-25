//! User-confirmed StudyRun preparation over a frozen, domain-qualified source set.
//!
//! A caller chooses works; the database rechecks policy, material qualification and the comment
//! budget in one repeatable-read transaction. This creates no provider invocation and does not
//! silently add an unselected work.

use crate::comment_study_source::{
    StudySource, StudySourceError, StudySourceRole, eligible_sources_in_transaction_for_selections,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

mod start;
pub use start::{
    StudyRunCancellationReceipt, StudyRunControlAction, StudyRunControlCommand,
    StudyRunControlReceipt, StudyStartError, StudyStartReceipt, TrustedStudyOrigin,
    cancel_study_run, control_study_run, preview_study_selection, recover_study_run,
    start_study_run,
};

const MAX_SELECTED_WORKS: usize = 100;

#[derive(Debug, Clone)]
pub struct PrepareStudyRunRequest {
    pub domain_ref: Uuid,
    pub content_public_refs: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudyWorkSelection {
    pub content_public_ref: Uuid,
    pub observation_role: StudySourceRole,
}

pub struct PrepareStudyRunWithSelectionsRequest {
    pub domain_ref: Uuid,
    pub selections: Vec<StudyWorkSelection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedStudyRun {
    pub run_ref: Uuid,
    pub as_of: String,
    pub selected_work_count: usize,
    pub selected_primary_work_count: usize,
    pub selected_reference_work_count: usize,
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
}

/// Revalidates a user selection and freezes only its currently eligible source comments.
pub async fn prepare_study_run(
    database: &Database,
    request: PrepareStudyRunRequest,
) -> Result<PreparedStudyRun, PrepareStudyRunError> {
    let selections = request
        .content_public_refs
        .into_iter()
        .map(|content_public_ref| StudyWorkSelection {
            content_public_ref,
            observation_role: StudySourceRole::Primary,
        })
        .collect();
    prepare_study_run_with_selections(
        database,
        PrepareStudyRunWithSelectionsRequest {
            domain_ref: request.domain_ref,
            selections,
        },
    )
    .await
}

pub async fn prepare_study_run_with_selections(
    database: &Database,
    request: PrepareStudyRunWithSelectionsRequest,
) -> Result<PreparedStudyRun, PrepareStudyRunError> {
    let selected = normalized_work_selection(request.selections)?;
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *transaction)
        .await?;
    let policy = read_active_policy(&mut transaction, request.domain_ref).await?;
    let domain_active: Option<Uuid> = sqlx::query_scalar(
        "SELECT domain_ref FROM observation_domain \
         WHERE domain_ref=$1 AND status='active' FOR SHARE",
    )
    .bind(policy.domain_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if domain_active.is_none() {
        return Err(PrepareStudyRunError::PolicyMissing);
    }
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *transaction)
        .await?;
    let source_selection: Vec<(Uuid, StudySourceRole)> = selected
        .iter()
        .map(|selection| (selection.content_public_ref, selection.observation_role))
        .collect();
    let sources = eligible_sources_in_transaction_for_selections(
        &mut transaction,
        policy.domain_ref,
        &as_of,
        &source_selection,
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
        selected_primary_work_count: selected
            .iter()
            .filter(|selection| selection.observation_role == StudySourceRole::Primary)
            .count(),
        selected_reference_work_count: selected
            .iter()
            .filter(|selection| selection.observation_role == StudySourceRole::Reference)
            .count(),
        covered_work_count: count_covered_works(database, run_ref).await?,
        target_count: count_targets(database, run_ref).await?,
        needs_context_count,
        comment_budget: policy.comment_budget,
    })
}

fn normalized_work_selection(
    mut selections: Vec<StudyWorkSelection>,
) -> Result<Vec<StudyWorkSelection>, PrepareStudyRunError> {
    selections.sort_by_key(|selection| selection.content_public_ref);
    let distinct: BTreeSet<Uuid> = selections
        .iter()
        .map(|selection| selection.content_public_ref)
        .collect();
    if selections.is_empty()
        || selections.len() > MAX_SELECTED_WORKS
        || distinct.len() != selections.len()
    {
        return Err(PrepareStudyRunError::InvalidSelection);
    }
    Ok(selections)
}

async fn read_active_policy(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain_ref: Uuid,
) -> Result<ActivePolicy, PrepareStudyRunError> {
    let row = sqlx::query(
        "SELECT policy.policy_ref,policy.domain_ref,policy.comment_budget \
         FROM linggan_comment_study_active_policy active \
         JOIN linggan_comment_study_policy policy USING(policy_ref) \
         WHERE active.domain_ref=$1 FOR SHARE OF active",
    )
    .bind(domain_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PrepareStudyRunError::PolicyMissing)?;
    Ok(ActivePolicy {
        policy_ref: row.get("policy_ref"),
        domain_ref: row.get("domain_ref"),
        comment_budget: row.get("comment_budget"),
    })
}

fn group_sources(sources: Vec<StudySource>) -> BTreeMap<(Uuid, StudySourceRole), Vec<StudySource>> {
    let mut grouped = BTreeMap::new();
    for source in sources {
        grouped
            .entry((source.content_public_ref, source.observation_role))
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
    selected: &[StudyWorkSelection],
    grouped: &BTreeMap<(Uuid, StudySourceRole), Vec<StudySource>>,
) -> Result<(), sqlx::Error> {
    let selected_sources: Vec<Uuid> = grouped
        .values()
        .flat_map(|sources| sources.iter().map(|source| source.source_ref))
        .collect();
    let manifest = json!({
        "contract":"comment-study.run-selection.v1",
        "requestedWorkRefs":selected.iter().map(|selection| selection.content_public_ref).collect::<Vec<_>>(),
        "requestedWorkRoles":selected,
        "coveredWorkRoles":grouped.keys().map(|(work_ref, role)| json!({"workRef":work_ref,"observationRole":role})).collect::<Vec<_>>(),
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
    grouped: BTreeMap<(Uuid, StudySourceRole), Vec<StudySource>>,
) -> Result<usize, sqlx::Error> {
    let mut needs_context_count = 0;
    for ((work_ref, observation_role), sources) in grouped {
        let context_manifest = full_context_manifest(&sources[0].context_manifest);
        let context_state = context_state(&context_manifest);
        insert_work(
            transaction,
            run_ref,
            work_ref,
            policy.domain_ref,
            observation_role,
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
    observation_role: StudySourceRole,
    context_state: &str,
    context_manifest: &Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO linggan_comment_study_work( \
           run_ref,content_public_ref,domain_ref,observation_role,selection_reason,context_state,context_manifest,context_hash \
         ) VALUES($1,$2,$3,$4,'user_selected',$5,$6,$7)",
    )
    .bind(run_ref)
    .bind(work_ref)
    .bind(domain_ref)
    .bind(observation_role.as_str())
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
                count(*) FILTER (WHERE state='cancelled') AS cancelled, \
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
    let cancelled: i64 = counts.get("cancelled");
    let state = if excluded + cancelled == total {
        "cancelled"
    } else if failed > 0 || excluded > 0 || cancelled > 0 {
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
    if has_sources { "ready" } else { "missing" }
}

pub fn context_cleaner_version() -> &'static str {
    crate::comment_cleaning::CLEANER_VERSION
}

/// Keep the dossier's canonical order and every selected block. Provider capacity
/// is checked at dispatch; it must never change which source text is being studied.
pub fn full_context_manifest(manifest: &Value) -> Value {
    let sources = manifest["sources"].as_array().cloned().unwrap_or_default();
    let count: usize = sources
        .iter()
        .map(|source| {
            source["text"]
                .as_str()
                .map_or(0, |text| text.chars().count())
        })
        .sum();
    json!({"contract":"comment-study.context.v2","workRef":manifest["workRef"],
        "cleanerVersion":manifest["cleanerVersion"],"dossierRevision":manifest["dossierRevision"],
        "dossierContract":manifest["dossierContract"],"includedCharacterCount":count,
        "readingTitle":manifest["readingTitle"],"orderBasis":"dossier_canonical_block_order","sources":sources})
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
    fn context_retains_all_blocks_in_canonical_order_without_a_budget() {
        let long = format!("{}不得据此归纳成普遍结论。", "完整材料🙂\n".repeat(12000));
        let manifest = json!({"contract":"comment-study.context.v2","workRef":Uuid::from_u128(10),
            "cleanerVersion":context_cleaner_version(),"sources":[
            {"blockRef":"second","sourceRef":Uuid::from_u128(11),"text":long,"speaker":"other","structure":{"type":"quote"},"issues":[{"code":"speaker_unconfirmed"}]},
            {"blockRef":"first","sourceRef":Uuid::from_u128(12),"text":"反例"}]});
        let full = full_context_manifest(&manifest);
        assert_eq!(full["sources"], manifest["sources"]);
        assert_eq!(full["includedCharacterCount"], long.chars().count() + 2);
        for key in ["characterBudget", "truncated", "omitted"] {
            assert!(full.get(key).is_none());
        }
        assert_eq!(context_state(&full), "ready");
    }
}
