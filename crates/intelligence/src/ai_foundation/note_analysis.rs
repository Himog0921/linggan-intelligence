//! Manual single-note analysis on the existing AI queue.
mod read;
mod source;
use super::{
    TaskKey, admission,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use crate::note_analysis::{METHOD, Packet, Scope};
use linggan_storage_postgres::Database;
pub use read::{latest, read, read_context};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub(crate) use source::frozen_in;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
pub const CONTRACT: &str = METHOD;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub scope: Scope,
    pub knowledge_cutoff: String,
    pub input_hash: String,
    pub task_config_ref: Uuid,
    pub task_revision: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub request_ref: Uuid,
    pub batch_ref: Uuid,
    pub item_ref: Uuid,
}
async fn configuration(tx: &mut Transaction<'_, Postgres>) -> Result<(Uuid, i64), FoundationError> {
    Ok(sqlx::query_as("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='note_interpretation'").fetch_one(&mut **tx).await?)
}
async fn reasons(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
    p: &Packet,
) -> Result<Vec<String>, FoundationError> {
    let mut reasons = Vec::new();
    for outcome in [
        admission::check_queue_configuration(tx, TaskKey::NoteInterpretation, config, false).await,
        admission::check_queue_input_scope(
            tx,
            TaskKey::NoteInterpretation,
            config,
            &InputRef::NoteAnalysis {
                request_ref: Uuid::nil(),
                include_comments: p.scope == Scope::NoteWithComments,
            },
        )
        .await,
    ] {
        match outcome {
            Ok(()) => {}
            Err(FoundationError::AdmissionDenied(c)) => reasons.extend(c),
            Err(e) => return Err(e),
        }
    }
    if !p.sources.iter().any(|s| s.kind == "material") {
        reasons.push("no_readable_material".into());
    }
    let unresolved: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_note_analysis_request r JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND r.work_ref=$2 AND r.content_fingerprint=$3 AND c.failure_code='ai_dispatch_unknown')")
        .bind(p.domain_ref).bind(p.work_ref).bind(p.fingerprint()).fetch_one(&mut **tx).await?;
    if unresolved {
        reasons.push("ai_prior_dispatch_unresolved".into());
    }
    // Reuse the transport estimator and count the actual instructions/schema too.
    let limit: Option<i64> = sqlx::query_scalar("SELECT m.input_token_limit::bigint FROM linggan_ai_task_config c JOIN linggan_model_config m ON m.config_ref=c.model_config_ref WHERE c.task_config_ref=$1").bind(config).fetch_optional(&mut **tx).await?;
    if let Some(limit) = limit {
        let (_, mut system, instruction, skill) =
            super::prompt::frozen_in(tx, config, TaskKey::NoteInterpretation).await?;
        if let Some(skill) = skill {
            system.push_str("\n\n");
            system.push_str(&skill);
        }
        let prompt = json!({"material":p.model_material(),"instruction":instruction}).to_string();
        let schema = crate::note_analysis::response_schema().to_string();
        if super::transport::estimate_input_tokens(&system, &format!("{prompt}{schema}"))
            > limit as u64
        {
            reasons.push("input_too_large".into());
        }
    }
    Ok(reasons)
}
pub async fn preview(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    scope: Scope,
) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    let (config, revision) = configuration(&mut tx).await?;
    let cols = source::columns(&mut tx, config).await?;
    let p = source::load(&mut tx, domain, work, scope.clone(), &cutoff, &cols).await?;
    let reasons = reasons(&mut tx, config, &p).await?;
    let result = json!({"contract":"note-analysis.preview.v1","domainRef":domain,"workRef":work,"scope":scope,"knowledgeCutoff":cutoff,"inputHash":p.hash(),"taskConfigRef":config,"taskRevision":revision,"methodVersion":METHOD,"coverage":p.coverage,"canStart":reasons.is_empty(),"reasons":reasons});
    tx.commit().await?;
    Ok(result)
}
pub async fn start(db: &Database, r: &Start) -> Result<Receipt, FoundationError> {
    if r.domain_ref.is_nil() || r.work_ref.is_nil() || r.task_revision < 0 {
        return Err(FoundationError::Invalid);
    }
    let mut c = Command::begin(db, r.command_ref, "note_analysis", r).await?;
    if let Some(v) = c.replay()? {
        return Ok(v);
    }
    let config = lock_setting(&mut c.tx, TaskKey::NoteInterpretation, r.task_revision).await?;
    if config != r.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let cutoff =
        linggan_evidence::research_dossier::cutoff_in(&mut c.tx, Some(&r.knowledge_cutoff)).await?;
    let cols = source::columns(&mut c.tx, config).await?;
    let p = source::load(
        &mut c.tx,
        r.domain_ref,
        r.work_ref,
        r.scope.clone(),
        &cutoff,
        &cols,
    )
    .await?;
    if p.hash() != r.input_hash {
        return Err(FoundationError::Conflict);
    }
    let current_cutoff = linggan_evidence::research_dossier::cutoff_in(&mut c.tx, None).await?;
    let current = source::load(
        &mut c.tx,
        r.domain_ref,
        r.work_ref,
        r.scope.clone(),
        &current_cutoff,
        &cols,
    )
    .await?;
    if current.fingerprint() != p.fingerprint() {
        return Err(FoundationError::Conflict);
    }
    let denied = reasons(&mut c.tx, config, &p).await?;
    if !denied.is_empty() {
        return Err(FoundationError::AdmissionDenied(denied));
    }
    let request = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request.to_string(),
        input: InputRef::NoteAnalysis {
            request_ref: request,
            include_comments: r.scope == Scope::NoteWithComments,
        },
    }];
    let hash = batch_store::input_hash(TaskKey::NoteInterpretation, &inputs, false)?;
    let batch = batch_store::insert_batch(
        &mut c.tx,
        r.command_ref,
        TaskKey::NoteInterpretation,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(&mut *c.tx)
            .await?;
    sqlx::query("INSERT INTO linggan_note_analysis_request(request_ref,command_ref,domain_ref,work_ref,scope,knowledge_cutoff,input_hash,input_manifest,content_fingerprint,task_config_ref,task_revision,method_version,batch_ref,item_ref) VALUES($1,$2,$3,$4,$5,$6::timestamptz,$7,$8,$9,$10,$11,$12,$13,$14)")
 .bind(request).bind(r.command_ref).bind(r.domain_ref).bind(r.work_ref).bind(r.scope.as_str()).bind(cutoff).bind(p.hash()).bind(json!(p)).bind(p.fingerprint()).bind(config).bind(r.task_revision).bind(METHOD).bind(batch).bind(item).execute(&mut *c.tx).await?;
    c.finish(Receipt {
        request_ref: request,
        batch_ref: batch,
        item_ref: item,
    })
    .await
}
