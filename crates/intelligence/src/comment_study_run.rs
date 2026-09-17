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
fn bounded_context_manifest(manifest: &Value, budget: usize) -> Value {
    let mut fragments: Vec<Value> = manifest["sources"].as_array().cloned().unwrap_or_default();
    fragments.sort_by_key(|fragment| {
        let kind = fragment["kind"].as_str().unwrap_or("");
        let priority = match kind {
            "native_title" => 0,
            "body" => 1,
            _ => 2,
        };
        (priority, kind.to_owned(), fragment["sourceRef"].to_string())
    });
    let mut used = 0_usize;
    let mut omitted = 0_usize;
    let mut retained = Vec::new();
    for fragment in fragments {
        let length = fragment["text"]
            .as_str()
            .map_or(0, |text| text.chars().count());
        if used.saturating_add(length) <= budget {
            used += length;
            retained.push(fragment);
        } else {
            omitted += 1;
        }
    }
    json!({
        "contract":"comment-study.context.v1",
        "workRef":manifest["workRef"],
        "cleanerVersion":manifest["cleanerVersion"],
        "characterBudget":budget,
        "includedCharacterCount":used,
        "omittedFragmentCount":omitted,
        "truncated":omitted > 0,
        "sources":retained
    })
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
}
