//! Batched inserts from already-frozen input. No reread of current raw/parent/work context.
use super::*;
use crate::comment_study_policy::{BUILDER_REVISION, json_hash};
use crate::comment_study_selection::SELECTION_ORDER;
use std::collections::BTreeSet;

pub(super) fn receipt(snapshot: &FrozenSelection, request_ref: Uuid, limits: &StudyRunLimits, run: Option<Uuid>) -> StudyStartReceipt {
    let needs = snapshot.inputs.values().filter(|i|i.dependency_state=="parent_required_missing").count();
    let target_count = snapshot.selection.target_count;
    let covered = snapshot.selection.selected_indices.iter().map(|i|snapshot.rows[*i].comment_key.work_ref)
        .collect::<BTreeSet<_>>().len();
    let pending = snapshot.index_coverage["pendingCount"].as_u64().unwrap_or(0);
    StudyStartReceipt { request_ref, outcome:if target_count>0 {"created"} else if pending>0 {"index_pending"} else {"no_work"}.into(),
        run_ref:run,as_of:snapshot.as_of.clone(),requested_work_count:snapshot.requested_works.len(),
        covered_work_count:covered,target_count,queued_count:target_count-needs,needs_context_count:needs,
        limits:limits.clone(),exclusion_counts:snapshot.selection.exclusion_counts.iter().map(|(k,v)|(k.to_string(),*v)).collect(),
        index_coverage:snapshot.index_coverage.clone(),dispatch_state:run.map(|_|"enabled".into()),idempotent_replay:false }
}

fn hash(value: &Value) -> Result<String, StudyStartError> {
    json_hash(value).map_err(|_| StudySelectionError::InvalidSnapshot.into())
}

fn execution(policy: &Value) -> Result<Value, StudyStartError> {
    let revision = option_env!("COMMENT_STUDY_ENGINE_REVISION").ok_or(StudyStartError::BuildUnrecorded)?;
    if revision.len()!=40 || !revision.bytes().all(|b|b.is_ascii_hexdigit()) {
        return Err(StudyStartError::BuildUnrecorded);
    }
    let mut value = json!({"contract":"comment-study.execution.v1","methodHash":policy["methodHash"],
        "builderRevision":BUILDER_REVISION,"engineRevision":revision,"maxTargetsPerBatch":12,"selectionOrder":SELECTION_ORDER});
    if option_env!("COMMENT_STUDY_ENGINE_DIRTY")==Some("true") { value["dirty"]=json!(true); }
    Ok(value)
}

pub(super) async fn insert_run(tx: &mut Transaction<'_, Postgres>, command: &StartStudyRunCommand,
    run: Uuid, policy: &Value, mut snapshot: FrozenSelection) -> Result<(), StudyStartError>
{
    let covered: BTreeSet<_> = snapshot.selection.selected_indices.iter()
        .map(|i|snapshot.rows[*i].comment_key.work_ref).collect();
    let sources: Vec<_> = snapshot.selection.selected_indices.iter().map(|i|snapshot.rows[*i].source_ref).collect();
    let limits = &command.limits;
    let manifest = json!({"contract":"comment-study.run-selection.v2","requestedWorkRefs":snapshot.requested_works,
        "coveredWorkRefs":covered,"targetSourceRefs":sources,"selectionMode":command.mode,
        "commentBudget":limits.comment_budget,"contextCharacterBudget":limits.context_character_budget,
        "tokenLimit":limits.token_limit,"indexCoverage":snapshot.index_coverage,"exclusionCounts":snapshot.selection.exclusion_counts});
    sqlx::query("INSERT INTO linggan_comment_study_run \
        (run_ref,policy_ref,as_of,state,selection_manifest,selection_hash,comment_budget,context_character_budget, \
         token_limit,execution_manifest,dispatch_state,dispatch_reason) \
        VALUES($1,$2,$3::text::timestamptz,'queued',$4,$5,$6,$7,$8,$9,'enabled',NULL)")
        .bind(run).bind(command.policy_ref).bind(&snapshot.as_of).bind(&manifest).bind(hash(&manifest)?)
        .bind(limits.comment_budget).bind(limits.context_character_budget).bind(limits.token_limit)
        .bind(execution(policy)?).execute(&mut **tx).await?;
    let mut works = Vec::new();
    for work in covered {
        let manifest = snapshot.contexts.remove(&work).ok_or(StudySelectionError::InvalidSnapshot)?;
        works.push(json!({"work_ref":work,"manifest":manifest,"hash":hash(&manifest)?,
            "state":super::super::context_state(&manifest)}));
    }
    sqlx::query("INSERT INTO linggan_comment_study_work \
        (run_ref,content_public_ref,domain_ref,selection_reason,context_state,context_manifest,context_hash) \
        SELECT $1,x.work_ref,$2,'user_selected',x.state,x.manifest,x.hash \
        FROM jsonb_to_recordset($3::jsonb) AS x(work_ref uuid,state text,manifest jsonb,hash text)")
        .bind(run).bind(command.domain_ref).bind(json!(works)).execute(&mut **tx).await?;
    let mut targets = Vec::new();
    for index in &snapshot.selection.selected_indices {
        let input = snapshot.inputs.remove(index).ok_or(StudySelectionError::InvalidSnapshot)?;
        let row = &snapshot.rows[*index];
        targets.push(json!({"ref":Uuid::new_v4(),"work":row.comment_key.work_ref,"id":row.comment_key.comment_external_id,
            "source":row.source_ref,"parent":input.parent_source_ref,"text":input.research_text,"text_hash":input.research_sha256,
            "dependency":input.dependency_state,"state":if input.dependency_state=="parent_required_missing" {"needs_context"} else {"queued"},
            "manifest":input.input_manifest,"hash":input.input_hash,"fingerprint":input.input_fingerprint}));
    }
    sqlx::query("INSERT INTO linggan_comment_study_target \
        (target_ref,run_ref,content_public_ref,comment_external_id,source_ref,parent_source_ref,research_text,research_sha256, \
         dependency_state,state,input_manifest,input_hash,input_fingerprint,finished_at) \
        SELECT x.ref,$1,x.work,x.id,x.source,x.parent,x.text,x.text_hash,x.dependency,x.state,x.manifest,x.hash,x.fingerprint, \
          CASE WHEN x.state='needs_context' THEN scope_001_now() ELSE NULL END \
        FROM jsonb_to_recordset($2::jsonb) AS x(ref uuid,work uuid,id text,source uuid,parent uuid,text text,text_hash text, \
          dependency text,state text,manifest jsonb,hash text,fingerprint text)")
        .bind(run).bind(json!(targets)).execute(&mut **tx).await?;
    super::super::close_run_if_settled(tx,run).await?;
    Ok(())
}
