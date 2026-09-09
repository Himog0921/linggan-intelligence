//! Durable work packets share the model invocation ledger. No remote call occurs in a transaction.
use crate::{
    comment_cleaning::{CLEANER_VERSION, clean_pending},
    comment_daily::{self},
    comment_execution_budget::{BudgetPurpose, check_budget_in},
    comment_packet::{ResearchPacket, build_packet_with_rule},
    comment_semantic_reservation::{SemanticReservations, frozen_rule, reserve_semantics},
    model_invocation::{checkpoint_invocation_usage, connection_request},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::{PiAdapter, PiResponse},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
struct Reserved {
    packet: Uuid,
    batch: Uuid,
    invocation: Uuid,
    config: Uuid,
    connection: Uuid,
    model: String,
    input_limit: i32,
    output_limit: i32,
    timeout: i32,
    research: ResearchPacket,
    policy: crate::comment_runtime::ContextPolicy,
    semantics: Vec<Uuid>,
    fingerprints: Vec<String>,
}
async fn recover(db: &Database) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    let rows=sqlx::query("SELECT packet_ref,invocation_ref,batch_ref,source_refs FROM linggan_comment_daily_packet WHERE purpose='extraction' AND state='running' AND lease_until<=scope_001_now() FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for row in rows {
        sqlx::query("UPDATE linggan_comment_daily_item SET state='failed',failure_code='worker_interrupted' WHERE batch_ref=$1 AND source_ref=ANY($2) AND state='running'").bind(row.get::<Uuid,_>("batch_ref")).bind(row.get::<Vec<Uuid>,_>("source_refs")).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code='worker_interrupted',finished_at=scope_001_now(),result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('callStarted',true,'usageUnknown',input_tokens IS NULL OR output_tokens IS NULL) WHERE invocation_ref=$1 AND state='running'").bind(row.get::<Uuid,_>("invocation_ref")).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_comment_daily_packet SET state='failed',finished_at=scope_001_now() WHERE packet_ref=$1").bind(row.get::<Uuid,_>("packet_ref")).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE linggan_comment_semantic_work s SET state='failed',failure_code='worker_interrupted',updated_at=scope_001_now() FROM linggan_model_invocation v WHERE s.invocation_ref=v.invocation_ref AND s.state='running' AND v.state='failed'").execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item i SET state=s.state,failure_code=s.failure_code,analysis_ref=s.analysis_ref FROM linggan_comment_semantic_work s WHERE i.semantic_ref=s.semantic_ref AND i.state='running' AND s.state<>'running'").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
async fn recover_context(db: &Database) -> Result<(), ModelError> {
    let rows=sqlx::query("SELECT b.request,b.context_policy,i.batch_ref,i.source_ref,i.context_candidate_hash,(i.context_candidate_at<=scope_001_now()-interval '60 seconds') AS settled,a.result FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) LEFT JOIN linggan_comment_analysis_work a ON a.work_ref=i.analysis_ref WHERE b.enabled AND (b.request ? 'ruleRevisionRef' OR b.request->>'ruleVersion'='comment-research.v4') AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)) AND (i.state='context_missing' OR (i.state IN ('succeeded','no_signal') AND jsonb_array_length(COALESCE(a.result#>'{semantic,contextMissing}','[]'::jsonb))>0)) ORDER BY i.context_candidate_at NULLS FIRST,b.window_end,i.source_ref LIMIT 100").fetch_all(db.pool()).await?;
    for row in rows {
        let source: Uuid = row.get("source_ref");
        let policy = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
        let rule = frozen_rule(db, &row.get::<Value, _>("request")).await?;
        let Ok(packet) =
            build_packet_with_rule(db, &[source], "context-check", &policy, &rule).await
        else {
            continue;
        };
        let context = &packet.inputs[0].context;
        let available = crate::comment_packet::semantic_context_for_comment(
            context,
            &packet.cleaned[0].text,
        )["fragments"]
            .as_array()
            .is_some_and(|fragments| !fragments.is_empty());
        let old: Option<Value> = row.get("result");
        let fingerprint = crate::comment_research::comment_source_hash(
            &crate::comment_packet::semantic_context_for_comment(context, &packet.cleaned[0].text)
                .to_string(),
        );
        let changed = old
            .as_ref()
            .is_none_or(|r| r["contextFingerprint"].as_str() != Some(fingerprint.as_str()));
        if available && changed {
            if row
                .get::<Option<String>, _>("context_candidate_hash")
                .as_deref()
                != Some(fingerprint.as_str())
            {
                sqlx::query("UPDATE linggan_comment_daily_item SET context_candidate_hash=$3,context_candidate_at=scope_001_now() WHERE batch_ref=$1 AND source_ref=$2").bind(row.get::<Uuid,_>("batch_ref")).bind(source).bind(&fingerprint).execute(db.pool()).await?;
            } else if row.get::<Option<bool>, _>("settled") == Some(true) {
                // Keep accepted history; only the changed dependency receives a new semantic version.
                sqlx::query("UPDATE linggan_comment_daily_item SET state='pending',failure_code=NULL,semantic_ref=NULL,attempts=0,context_candidate_hash=NULL,context_candidate_at=NULL WHERE batch_ref=$1 AND source_ref=$2 AND state IN ('context_missing','succeeded','no_signal')").bind(row.get::<Uuid,_>("batch_ref")).bind(source).execute(db.pool()).await?;
            }
        } else {
            sqlx::query("UPDATE linggan_comment_daily_item SET context_candidate_at=scope_001_now(),context_candidate_hash=NULL WHERE batch_ref=$1 AND source_ref=$2").bind(row.get::<Uuid,_>("batch_ref")).bind(source).execute(db.pool()).await?;
        }
    }
    Ok(())
}
pub(crate) fn missing_context(research: &ResearchPacket) -> Vec<Uuid> {
    research
        .inputs
        .iter()
        .zip(&research.cleaned)
        .filter(|(input, cleaned)| {
            cleaned.state == "context"
                && crate::comment_packet::semantic_context_for_comment(
                    &input.context,
                    &cleaned.text,
                )["fragments"]
                    .as_array()
                    .is_none_or(Vec::is_empty)
        })
        .map(|(input, _)| input.source_ref)
        .collect()
}
fn admitted_source_refs(candidates: &[sqlx::postgres::PgRow], remaining_sources: i64) -> Vec<Uuid> {
    // A retry reuses an already admitted source slot; source caps only limit first calls.
    let mut remaining = remaining_sources.max(0);
    candidates
        .iter()
        .filter_map(|item| {
            if item.get::<i32, _>("attempts") > 0 {
                Some(item.get("source_ref"))
            } else if remaining > 0 {
                remaining -= 1;
                Some(item.get("source_ref"))
            } else {
                None
            }
        })
        .collect()
}
async fn settle_unavailable_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), ModelError> {
    // Source cap limits analysis, never the frozen manifest or intake counts.
    sqlx::query("UPDATE linggan_comment_daily_item i SET state='restricted',failure_code='source_unavailable' WHERE i.state='pending' AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_readable s WHERE s.material_ref=i.source_ref)").execute(&mut **tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item i SET state=c.state FROM linggan_comment_clean c,linggan_comment_daily_batch b WHERE b.batch_ref=i.batch_ref AND (b.request ? 'ruleRevisionRef' OR b.request->>'ruleVersion'='comment-research.v4') AND c.source_ref=i.source_ref AND c.cleaner_version=$1 AND c.state IN ('low_information','anomaly','dropped') AND i.state='pending'").bind(CLEANER_VERSION).execute(&mut **tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item i SET state='source_limit',failure_code='source_limit' FROM linggan_comment_daily_batch b,linggan_comment_clean c WHERE i.batch_ref=b.batch_ref AND (b.request ? 'ruleRevisionRef' OR b.request->>'ruleVersion'='comment-research.v4') AND c.source_ref=i.source_ref AND c.cleaner_version=$1 AND c.state IN ('direct','context') AND i.state='pending' AND i.attempts=0 AND (SELECT count(*) FROM linggan_comment_daily_item used WHERE used.batch_ref=b.batch_ref AND used.attempts>0)>=COALESCE((SELECT (a.request->>'sourceLimit')::integer FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.source_limit)").bind(CLEANER_VERSION).execute(&mut **tx).await?;
    Ok(())
}
async fn select_packet_sources(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    row: &sqlx::postgres::PgRow,
    rule: &crate::comment_research_rules::RuleSnapshot,
    policy: &crate::comment_runtime::ContextPolicy,
) -> Result<Vec<Uuid>, ModelError> {
    let batch: Uuid = row.get("batch_ref");
    let output_limit: i32 = row.get("output_token_limit");
    // V5 may emit several independently evidenced atoms for each comment. Reserve a
    // larger output allowance instead of inheriting the compact V4 label estimate.
    let per_comment_output = if rule.rule_version.as_str() == "comment-research.v5" {
        1024
    } else {
        256
    };
    let max_comments =
        i64::from((output_limit / per_comment_output).clamp(1, 30)).min(policy.max_comments as i64);
    let candidates=sqlx::query("SELECT i.source_ref,i.attempts FROM linggan_comment_daily_item i JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref JOIN linggan_ci_comment_identity identity ON identity.work_ref=s.content_public_ref AND identity.comment_external_id=s.comment_external_id JOIN linggan_comment_clean c ON c.source_ref=i.source_ref AND c.cleaner_version=$3 WHERE i.batch_ref=$1 AND s.content_public_ref=$2 AND i.state='pending' AND i.attempts<$5 AND COALESCE(i.queue_class,CASE WHEN i.attempts>0 THEN 'recovery' ELSE 'new_intake' END)=$6 AND c.state IN ('direct','context') ORDER BY CASE COALESCE(i.queue_class,CASE WHEN i.attempts>0 THEN 'recovery' ELSE 'new_intake' END) WHEN 'new_intake' THEN 0 WHEN 'recovery' THEN 1 ELSE 2 END,identity.first_observed_at,i.source_ref LIMIT $4")
        .bind(batch).bind(row.get::<Uuid,_>("content_public_ref")).bind(CLEANER_VERSION).bind(max_comments).bind(row.get::<i32,_>("max_attempts")).bind(row.get::<String,_>("selected_queue_class")).fetch_all(&mut **tx).await?;
    let mut refs = admitted_source_refs(&candidates, row.get("remaining_sources"));
    if row.get::<String, _>("kind") != "selected" {
        refs = crate::comment_auto_backlog::admit_day_sources(tx, &refs).await?;
    }
    Ok(refs)
}
async fn build_admitted_packet(
    db: &Database,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    refs: &mut Vec<Uuid>,
    row: &sqlx::postgres::PgRow,
    policy: &crate::comment_runtime::ContextPolicy,
    rule: &crate::comment_research_rules::RuleSnapshot,
) -> Result<Option<ResearchPacket>, ModelError> {
    let config: Uuid = row.get("config_ref");
    let batch: Uuid = row.get("batch_ref");
    let input_limit: i32 = row.get("input_token_limit");
    let research = loop {
        let packet =
            build_packet_with_rule(db, &refs, &comment_daily::version(config), &policy, &rule)
                .await?;
        if packet.prompt.len() + packet.system.len() + 512 <= input_limit as usize {
            break packet;
        }
        if refs.len() == 1 {
            sqlx::query("UPDATE linggan_comment_daily_item SET state='failed',failure_code='model_input_limit' WHERE batch_ref=$1 AND source_ref=$2").bind(batch).bind(refs[0]).execute(&mut **tx).await?;
            return Ok(None);
        }
        refs.pop();
    };
    // A short reply without either parent or work text must remain context-insufficient.
    let missing = missing_context(&research);
    if !missing.is_empty() {
        sqlx::query("UPDATE linggan_comment_daily_item SET state='context_missing',failure_code='context_missing' WHERE batch_ref=$1 AND source_ref=ANY($2)").bind(batch).bind(missing).execute(&mut **tx).await?;
        return Ok(None);
    }
    Ok(Some(research))
}
async fn reserve(
    db: &Database,
    drain: Option<&crate::model_worker_drain::ModelWorkerDrain>,
    include_history: bool,
) -> Result<Option<Reserved>, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    settle_unavailable_items(&mut tx).await?;
    let row=sqlx::query(crate::model_settings::with_model_callability("SELECT b.kind,b.context_policy,b.batch_ref,b.config_ref,b.token_limit,b.request,(COALESCE((SELECT (a.request->>'sourceLimit')::integer FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.source_limit)-(SELECT count(*) FROM linggan_comment_daily_item used WHERE used.batch_ref=b.batch_ref AND used.attempts>0)) AS remaining_sources,cfg.input_token_limit,cfg.output_token_limit,cfg.timeout_seconds,cfg.max_attempts,m.model_ref,m.model_id,v.version_ref,s.content_public_ref,COALESCE(i.queue_class,CASE WHEN i.attempts>0 THEN 'recovery' ELSE 'new_intake' END) AS selected_queue_class FROM linggan_comment_daily_batch b JOIN linggan_model_config cfg USING(config_ref) JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) JOIN linggan_comment_daily_item i USING(batch_ref) JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref JOIN linggan_ci_comment_identity identity ON identity.work_ref=s.content_public_ref AND identity.comment_external_id=s.comment_external_id JOIN linggan_comment_clean c ON c.source_ref=i.source_ref AND c.cleaner_version=$1 WHERE ($2::boolean OR COALESCE(i.queue_class,CASE WHEN i.attempts>0 THEN 'recovery' ELSE 'new_intake' END)<>'historical') AND b.enabled AND (b.request ? 'ruleRevisionRef' OR b.request->>'ruleVersion'='comment-research.v4') AND __MODEL_CALLABLE__ AND (b.kind<>'supplement' OR EXISTS(SELECT 1 FROM linggan_comment_daily_batch origin WHERE origin.batch_ref::text=b.request->>'originBatchRef' AND origin.enabled)) AND conn.enabled AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)) AND (b.kind='selected' OR linggan_comment_day_source_allowed(s.content_public_ref,s.comment_external_id)) AND i.state='pending' AND i.attempts<cfg.max_attempts AND c.state IN ('direct','context') AND (b.kind IN('daily','backlog') OR COALESCE((SELECT sum(v.charged_tokens) FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) JOIN linggan_comment_daily_batch family ON family.batch_ref=p.batch_ref WHERE COALESCE(family.request->>'originBatchRef',family.batch_ref::text)=COALESCE(b.request->>'originBatchRef',b.batch_ref::text)),0)+cfg.input_token_limit+cfg.output_token_limit<=COALESCE((SELECT (a.request->>'tokenLimit')::bigint FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.token_limit)) ORDER BY CASE COALESCE(i.queue_class,CASE WHEN i.attempts>0 THEN 'recovery' ELSE 'new_intake' END) WHEN 'new_intake' THEN 0 WHEN 'recovery' THEN 1 ELSE 2 END,identity.first_observed_at,b.window_end,b.batch_ref,s.content_public_ref,i.source_ref LIMIT 1"))
        .bind(CLEANER_VERSION).bind(include_history).fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let batch: Uuid = row.get("batch_ref");
    let config: Uuid = row.get("config_ref");
    let input_limit: i32 = row.get("input_token_limit");
    let output_limit: i32 = row.get("output_token_limit");
    let policy = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
    let rule = frozen_rule(db, &row.get::<Value, _>("request")).await?;
    let mut refs = select_packet_sources(&mut tx, &row, &rule, &policy).await?;
    if refs.is_empty() {
        tx.commit().await?;
        return Ok(None);
    }
    let Some(mut research) =
        build_admitted_packet(db, &mut tx, &mut refs, &row, &policy, &rule).await?
    else {
        tx.commit().await?;
        return Ok(None);
    };
    let SemanticReservations {
        execute_refs,
        semantics,
        fingerprints,
        unknown_recovery,
    } = reserve_semantics(
        &mut tx,
        &research,
        config,
        batch,
        &row.get::<Value, _>("request"),
        row.get("max_attempts"),
    )
    .await?;
    if execute_refs.is_empty() {
        tx.commit().await?;
        return Ok(None);
    }
    refs = execute_refs;
    research =
        build_packet_with_rule(db, &refs, &comment_daily::version(config), &policy, &rule).await?;
    let budget = check_budget_in(
        &mut tx,
        BudgetPurpose::Extraction,
        i64::from(input_limit + output_limit),
        unknown_recovery,
    )
    .await?;
    if !budget.allowed {
        tx.rollback().await?;
        return Ok(None);
    }
    let invocation = Uuid::new_v4();
    let packet = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)")
        .bind(invocation).bind(row.get::<Uuid,_>("version_ref")).bind(row.get::<Uuid,_>("model_ref")).bind(config).bind(crate::comment_research::comment_source_hash(&research.prompt)).bind(i64::from(input_limit+output_limit)).bind(&budget.ledger_metadata).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE linggan_comment_semantic_work SET invocation_ref=$2 WHERE semantic_ref=ANY($1)",
    )
    .bind(&semantics)
    .bind(invocation)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO linggan_comment_daily_packet(packet_ref,batch_ref,source_refs,context_refs,context_hash,invocation_ref,lease_until,state) VALUES($1,$2,$3,$4,$5,$6,scope_001_now()+interval '120 seconds','running')")
        .bind(packet).bind(batch).bind(&refs).bind(&research.context_refs).bind(&research.context_hash).bind(invocation).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item SET state='running',attempts=attempts+1,failure_code=NULL,execution_day=$3::date,queue_class=COALESCE(queue_class,CASE WHEN attempts>0 THEN 'recovery' ELSE 'new_intake' END) WHERE batch_ref=$1 AND source_ref=ANY($2) AND state='pending'").bind(batch).bind(&refs).bind(&budget.execution_day).execute(&mut *tx).await?;
    if drain.is_some_and(|d| d.is_requested()) {
        tx.rollback().await?;
        return Ok(None);
    }
    tx.commit().await?;
    Ok(Some(Reserved {
        packet,
        batch,
        invocation,
        config,
        connection: row.get("version_ref"),
        model: row.get("model_id"),
        input_limit,
        output_limit,
        timeout: row.get("timeout_seconds"),
        research,
        policy,
        semantics,
        fingerprints,
    }))
}
async fn permitted(db: &Database, r: &Reserved, dispatch: bool) -> Result<bool, ModelError> {
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_batch b JOIN linggan_model_config cfg USING(config_ref) JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE b.batch_ref=$1 AND b.enabled AND (b.kind<>'supplement' OR EXISTS(SELECT 1 FROM linggan_comment_daily_batch origin WHERE origin.batch_ref::text=b.request->>'originBatchRef' AND origin.enabled)) AND c.enabled AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)))").bind(r.batch).fetch_one(db.pool()).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_readable WHERE material_ref=ANY($1)",
    )
    .bind(&r.research.context_refs)
    .fetch_one(db.pool())
    .await?;
    Ok((!dispatch || active) && count == r.research.context_refs.len() as i64)
}
pub async fn run_daily_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    run_daily_once_with_drain(db, store, adapter, None).await
}
async fn prepare_pending_research(db: &Database) -> Result<(), ModelError> {
    crate::comment_runtime::expire_content(db).await?;
    recover(db).await?;
    comment_daily::seal_all_due(db).await?;

    clean_pending(db).await?;
    crate::comment_local_recovery::recover(db).await?;
    recover_context(db).await?;
    crate::comment_legacy_reuse::reconcile(db).await?;
    crate::comment_research_reconcile::reconcile(db).await?;
    crate::comment_eligibility_projection::refresh(db, None, 100).await?;
    crate::comment_auto_backlog::seal(db).await?;
    Ok(())
}

pub async fn run_daily_once_with_drain(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&crate::model_worker_drain::ModelWorkerDrain>,
) -> Result<bool, ModelError> {
    if drain.is_some_and(|d| d.is_requested()) {
        return Ok(false);
    }
    if !comment_daily::schema_ready(db).await? {
        return Ok(false);
    }
    prepare_pending_research(db).await?;
    let selected = reserve(db, drain, false).await?;
    let selected = if selected.is_some() {
        selected
    } else {
        if crate::comment_field_repair::schema_ready(db).await?
            && crate::comment_field_repair::run_next_field_repair(db, store, adapter, drain).await?
        {
            return Ok(true);
        }
        reserve(db, drain, true).await?
    };
    let Some(r) = selected else {
        return Ok(false);
    };
    let mut request = match connection_request(db, store, r.connection).await {
        Ok(v) => v,
        Err(e) => {
            finish(db, &r, None, Some(e.code()), false).await?;
            return Ok(true);
        }
    };
    if !permitted(db, &r, true).await? {
        finish(db, &r, None, Some("model_disabled"), false).await?;
        return Ok(true);
    }
    request.operation = "analyze".into();
    request.model_id = r.model.clone();
    request.prompt = r.research.prompt.clone();
    request.system = r.research.system.to_string();
    request.max_output_tokens = r.output_limit;
    request.timeout_ms = r.timeout as u64 * 1000;
    if drain.is_some_and(|d| d.is_requested()) {
        finish(db, &r, None, Some("worker_draining"), false).await?;
        return Ok(false);
    }
    crate::comment_runtime::begin_trace(db, r.invocation, r.packet, &r.research, &r.policy).await?;
    if drain.is_some_and(|d| d.is_requested()) {
        finish(db, &r, None, Some("worker_draining"), false).await?;
        return Ok(false);
    }
    let outcome = adapter.call(&request).await;
    let response = outcome.as_ref().ok();
    crate::comment_runtime::record_response(db, r.invocation, response, &r.policy).await?;
    checkpoint_invocation_usage(db, r.invocation, response).await?;
    if matches!(outcome, Err(ModelError::Timeout)) {
        crate::comment_runtime::record_process_timeout(db, r.invocation, request.timeout_ms + 1000)
            .await?;
    }
    let mut failure = provider_failure(&outcome, &r);
    if !permitted(db, &r, false).await? {
        failure = Some("source_or_plan_unavailable");
    }
    let refs: Vec<_> = r.research.inputs.iter().map(|i| i.source_ref).collect();
    match build_packet_with_rule(
        db,
        &refs,
        &comment_daily::version(r.config),
        &r.policy,
        &r.research.rule_snapshot,
    )
    .await
    {
        Ok(current) if current.context_hash == r.research.context_hash => {}
        _ => failure = Some("context_changed"),
    }
    finish(db, &r, response, failure, true).await?;
    crate::comment_eligibility_projection::refresh(db, Some(&refs), refs.len() as i64).await?;
    Ok(true)
}
fn provider_failure(
    outcome: &Result<PiResponse, ModelError>,
    r: &Reserved,
) -> Option<&'static str> {
    let mut failure = outcome.as_ref().err().map(ModelError::code);
    if let Some(p) = outcome.as_ref().ok() {
        if !p.ok {
            failure = Some(match p.failure_code.as_deref() {
                Some("output_limit") => "output_limit",
                Some("provider_timeout") => "provider_timeout",
                Some("provider_stream_interrupted") => "provider_stream_interrupted",
                Some("provider_terminal_missing") => "provider_terminal_missing",
                Some("provider_network_error") => "provider_network_error",
                Some("provider_content_filtered") => "provider_content_filtered",
                Some("authentication_failed") => "authentication_failed",
                Some("provider_rate_limited") => "provider_rate_limited",
                Some("unexpected_content") => "unexpected_content",
                Some("model_input_limit") => "model_input_limit",
                Some("provider_refused") => "provider_refused",
                Some("provider_request_rejected") => "provider_request_rejected",
                Some("provider_unavailable") => "provider_unavailable",
                _ => "provider_failed",
            });
        }
        if p.usage
            .input_tokens
            .is_some_and(|v| v > i64::from(r.input_limit))
            || p.usage
                .output_tokens
                .is_some_and(|v| v > i64::from(r.output_limit))
        {
            failure = Some("model_budget_overrun");
        }
    }
    failure
}

async fn finish(
    db: &Database,
    r: &Reserved,
    response: Option<&PiResponse>,
    failure: Option<&str>,
    started: bool,
) -> Result<(), ModelError> {
    let parsed = if let Some(code) = failure {
        Err(code)
    } else {
        r.research
            .parse_for_rule(response.and_then(|v| v.text.as_deref()).unwrap_or(""))
    };
    let global_failure = parsed.as_ref().err().copied();
    let mut tx = db.pool().begin().await?;
    let valid:bool=sqlx::query_scalar("SELECT state='running' AND lease_until>scope_001_now() FROM linggan_comment_daily_packet WHERE packet_ref=$1 FOR UPDATE").bind(r.packet).fetch_one(&mut *tx).await?;
    if !valid {
        return Err(ModelError::Conflict);
    }
    let outcomes =
        parsed.unwrap_or_else(|code| r.research.inputs.iter().map(|_| Err(code)).collect());
    let diagnostics = if failure.is_none() {
        r.research
            .validation_diagnostics_for_rule(response.and_then(|p| p.text.as_deref()).unwrap_or(""))
    } else {
        vec![
            json!({"commentRef":null,"code":failure,"path":"request","expected":"供应商完整返回","actual":"请求未完成或未获接纳"}),
        ]
    };
    let mut outcome_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut success = 0;
    for (index, (input, result)) in r.research.inputs.iter().zip(outcomes).enumerate() {
        let (state, code, analysis_ref) = match result {
            Ok(mut value) => {
                let state = if value["semantic"]["outcome"] == "no_signal" {
                    "no_signal"
                } else {
                    "succeeded"
                };
                let reference = Uuid::new_v4();
                // A batch is a frozen run version; retries never replace previous accepted analyses.
                let version = format!(
                    "{}:{}",
                    comment_daily::version(r.config),
                    r.semantics[index]
                );
                value["semanticFingerprint"] = json!(r.fingerprints[index]);
                value["modelVersion"] = json!(version);
                value["ruleRevisionRef"] = json!(r.research.rule_snapshot.rule_revision_ref);
                value["ruleHash"] = json!(r.research.rule_snapshot.canonical_hash);
                let actual:Uuid=sqlx::query_scalar("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result,attempts) VALUES($1,$2,$3,$4,$5,$6,1) RETURNING work_ref")
                    .bind(reference).bind(input.source_ref).bind(r.research.rule_snapshot.rule_version.as_str()).bind(version).bind(state).bind(value).fetch_one(&mut *tx).await?;
                success += 1;
                (state, None, Some(actual))
            }
            Err(code) => ("failed", Some(code), None),
        };
        *outcome_counts.entry(state.into()).or_default() += 1;
        sqlx::query("UPDATE linggan_comment_semantic_work SET state=$2,failure_code=$3,analysis_ref=$4,updated_at=scope_001_now() WHERE semantic_ref=$1 AND invocation_ref=$5 AND state='running'").bind(r.semantics[index]).bind(state).bind(code).bind(analysis_ref).bind(r.invocation).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_comment_daily_item SET state=$2,failure_code=$3,analysis_ref=$4 WHERE semantic_ref=$1 AND state='running'").bind(r.semantics[index]).bind(state).bind(code).bind(analysis_ref).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE linggan_comment_daily_packet SET state=$2,finished_at=scope_001_now() WHERE packet_ref=$1").bind(r.packet).bind(if success==r.research.inputs.len(){"succeeded"}else if success>0{"partial"}else{"failed"}).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_model_invocation SET state=$2,failure_code=$3,result=COALESCE(result,'{}'::jsonb)||$4,finished_at=scope_001_now(),charged_tokens=CASE WHEN $5 THEN charged_tokens ELSE 0 END,input_tokens=CASE WHEN $5 THEN input_tokens ELSE 0 END,output_tokens=CASE WHEN $5 THEN output_tokens ELSE 0 END WHERE invocation_ref=$1 AND state='running'")
        .bind(r.invocation).bind(if success==r.research.inputs.len(){"succeeded"}else{"failed"}).bind(global_failure.or(if success<r.research.inputs.len(){Some("partial_output")}else{None}))
        .bind(json!({"callStarted":started,"batchRef":r.batch,"packetRef":r.packet,"acceptedComments":success,"requestedComments":r.research.inputs.len(),"failureCode":global_failure,"normalization":response.and_then(|p|p.text.as_deref()).and_then(ResearchPacket::normalization_kind)})).bind(started).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_request_trace SET validation=$2,outcomes=$3,events=events||jsonb_build_array(jsonb_build_object('kind','validation_finished','at',scope_001_now()),jsonb_build_object('kind','results_saved','at',scope_001_now())) WHERE invocation_ref=$1")
      .bind(r.invocation).bind(json!(diagnostics)).bind(json!(outcome_counts)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
