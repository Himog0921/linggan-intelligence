//! Existing worker lane: bounded canonical queue -> atomic budget -> adapter -> typed acceptance.
use crate::{
    model_invocation::{connection_request, finish_invocation_in},
    model_secrets::ModelSecretStore,
    model_settings::{ModelError, reserve_research_model_semantic_dispatch_permit},
    pi_adapter::{PiAdapter, PiResponse},
    topic_map_core as core,
    topic_map_research::{self, ResearchError, ResearchInput},
    topic_map_research_analysis::{self as analysis, Discussion, ResearchOutput, ResolutionOutput},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
#[path = "topic_map_research_worker/dispatch.rs"]
mod dispatch;
#[path = "topic_map_research_worker/lifecycle.rs"]
mod lifecycle;
#[path = "topic_map_research_worker/preparation.rs"]
mod preparation;
#[path = "topic_map_research_worker/settlement.rs"]
mod settlement;
use preparation::prepare;
use settlement::settle;
fn err(e: ResearchError) -> ModelError {
    match e {
        ResearchError::Database(e) => ModelError::Database(e),
        _ => ModelError::Source,
    }
}

/// Recovery cannot turn an unknown transmitted call into permission to send it twice.
async fn recover(db: &Database) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    let rows=sqlx::query("SELECT q.invocation_ref,q.task_ref,q.dispatch_started_at IS NOT NULL AS sent FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) JOIN linggan_topic_map_research_task t USING(task_ref) WHERE i.state='running' AND q.deadline_at<scope_001_now() AND t.state='running' FOR UPDATE OF i,t SKIP LOCKED LIMIT 32").fetch_all(&mut *tx).await?;
    for r in rows {
        let sent: bool = r.get("sent");
        let invocation: Uuid = r.get("invocation_ref");
        sqlx::query("UPDATE linggan_model_invocation SET state='failed',finished_at=scope_001_now(),failure_code=$2,charged_tokens=CASE WHEN $3 THEN charged_tokens ELSE 0 END,result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('dispatchUnknown',$3::bool) WHERE invocation_ref=$1").bind(invocation).bind(if sent{"unknown_dispatch"}else{"request_not_started"}).bind(sent).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_topic_map_research_task SET state=CASE WHEN $2 THEN 'unknown_dispatch' WHEN phase_attempt_count>=(SELECT c.max_attempts FROM linggan_topic_map_research_run r JOIN linggan_model_config c ON c.config_ref=r.config_ref WHERE r.run_ref=linggan_topic_map_research_task.run_ref) THEN 'failed' ELSE 'queued' END,last_reason=CASE WHEN $2 THEN 'unknown_dispatch' ELSE 'request_not_started' END,lease_token=NULL,lease_expires_at=NULL WHERE task_ref=$1").bind(r.get::<Uuid,_>("task_ref")).bind(sent).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE linggan_topic_map_research_run r SET state='queued',last_reason=NULL,updated_at=scope_001_now() FROM linggan_topic_map_research_policy p WHERE r.domain_ref=p.domain_ref AND r.state='daily_budget_paused' AND r.updated_at < date_trunc('day',scope_001_now()AT TIME ZONE 'Asia/Shanghai')AT TIME ZONE 'Asia/Shanghai' AND p.status='active' AND EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=r.domain_ref AND d.status='active')").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn run_once(
    db: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    let ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_concept_rule')IS NOT NULL")
            .fetch_one(db.pool())
            .await?;
    if !ready {
        return Ok(false);
    }
    recover(db).await?;
    crate::topic_map_research_collection::advance_comment_rounds_once(db)
        .await
        .map_err(err)?;
    let search_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_search_target')IS NOT NULL")
            .fetch_one(db.pool())
            .await?;
    if search_ready {
        crate::topic_map_collection_search::advance_search_rounds_once(db)
            .await
            .map_err(err)?;
    }
    // One bounded scan per tick, cyclic domain order; no work means no provider call.
    let p=sqlx::query("SELECT p.domain_ref FROM linggan_topic_map_research_policy p JOIN observation_domain d USING(domain_ref) WHERE p.automatic_enabled AND p.status='active' AND d.status='active' ORDER BY p.updated_at,p.domain_ref LIMIT 1").fetch_optional(db.pool()).await?;
    if let Some(p) = p {
        let domain: Uuid = p.get("domain_ref");
        topic_map_research::queue_research_run(
            db,
            domain,
            Uuid::new_v4(),
            "incremental",
            &[],
            None,
        )
        .await
        .map_err(err)?;
        sqlx::query("UPDATE linggan_topic_map_research_policy SET updated_at=scope_001_now()WHERE domain_ref=$1").bind(domain).execute(db.pool()).await?;
    }
    let maintained = core::backfill::advance_once(db).await?;
    let maintained = core::comparison::queue_once(db).await? || maintained;
    let maintained = lifecycle::complete_ready_runs(db).await? || maintained;
    // Deterministic round-robin: smallest previous dispatch count serves history as well as live work.
    let row = next_task(db).await?;
    let Some(row) = row else {
        return Ok(maintained);
    };
    dispatch::execute(db, secrets, adapter, row).await
}

async fn next_task(db: &Database) -> Result<Option<sqlx::postgres::PgRow>, ModelError> {
    Ok(sqlx::query(r#"
      SELECT t.*,r.config_ref,r.method_version
      FROM linggan_topic_map_research_task t
      JOIN linggan_topic_map_research_run r USING(run_ref)
      JOIN linggan_topic_map_research_policy p ON p.domain_ref=r.domain_ref
      JOIN observation_domain d ON d.domain_ref=r.domain_ref
      JOIN linggan_model_config c ON c.config_ref=r.config_ref
      WHERE t.state='queued' AND t.phase_attempt_count<c.max_attempts
        AND r.state IN ('queued','running') AND p.status='active' AND d.status='active'
        AND (p.automatic_enabled
          OR (r.trigger='on_demand' AND COALESCE(r.input_scope->>'backfillReopened','false')<>'true')
          OR EXISTS(
            SELECT 1 FROM jsonb_array_elements(CASE WHEN jsonb_typeof(r.input_scope->'backfillAuthorizations')='array' THEN r.input_scope->'backfillAuthorizations' ELSE '[]'::jsonb END) permission
            WHERE permission->'requestRef'=(t.recall_manifest#>'{backfill,authorizationRequestRef}')
              AND permission->'workRefs' @> jsonb_build_array(t.work_public_ref)
              AND jsonb_array_length(CASE WHEN jsonb_typeof(t.recall_manifest#>'{backfill,forcedDefinitionRefs}')='array' THEN t.recall_manifest#>'{backfill,forcedDefinitionRefs}' ELSE '[]'::jsonb END)>0
              AND permission->'definitionRefs' @> (t.recall_manifest#>'{backfill,forcedDefinitionRefs}')
              AND (NOT (t.recall_manifest->'backfill' ? 'comparisonScopeWorkRefs') OR (
                jsonb_array_length(CASE WHEN jsonb_typeof(t.recall_manifest#>'{backfill,comparisonScopeWorkRefs}')='array' THEN t.recall_manifest#>'{backfill,comparisonScopeWorkRefs}' ELSE '[]'::jsonb END) BETWEEN 1 AND 10
                AND permission->'workRefs' @> (t.recall_manifest#>'{backfill,comparisonScopeWorkRefs}')
              ))
          ))
      ORDER BY (SELECT count(*) FROM linggan_topic_map_research_request q WHERE q.run_ref=r.run_ref),
        CASE r.trigger WHEN 'on_demand' THEN 0 WHEN 'incremental' THEN 1 ELSE 2 END,t.created_at
      LIMIT 1
    "#).fetch_optional(db.pool()).await?)
}

struct Prepared {
    system: String,
    prompt: String,
    draft: Option<ResearchOutput>,
    units: Vec<(String, Discussion)>,
    candidates: Value,
    recall: Value,
    cursor: usize,
    total_units: usize,
    definition_unavailable: bool,
    eligible_definitions: Vec<Uuid>,
    previous: Vec<Value>,
}
