//! Bounded execution of already granted comment work; no model-owned scheduling or database tools.
use crate::{
    comment_analysis::*, model_invocation::*, model_plans::*, model_secrets::*, model_settings::*,
    pi_adapter::*,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::sync::Mutex;
use uuid::Uuid;

struct ReservedCall {
    invocation: Uuid,
    work: Uuid,
    config: Uuid,
    version: Uuid,
    model_id: String,
    input_limit: i32,
    output_limit: i32,
    timeout: i32,
}
async fn reserve_call(db: &Database) -> Result<Option<ReservedCall>, ModelError> {
    let mut tx = db.pool().begin().await?;
    // Serialize only short reservation transactions in this local workspace. The next
    // statements obtain fresh snapshots after the lock, so concurrent workers cannot
    // reserve against an older budget total. Provider I/O is outside this lock.
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    // A crashed caller may have consumed tokens. Keep the reservation as unknown usage.
    sqlx::query("UPDATE linggan_model_invocation i SET state=CASE WHEN EXISTS(SELECT 1 FROM linggan_comment_analysis_work w WHERE w.work_ref=i.work_ref AND w.state IN ('succeeded','no_signal')) THEN 'succeeded' ELSE 'failed' END,failure_code=CASE WHEN EXISTS(SELECT 1 FROM linggan_comment_analysis_work w WHERE w.work_ref=i.work_ref AND w.state IN ('succeeded','no_signal')) THEN NULL ELSE 'worker_interrupted' END,finished_at=scope_001_now(),result=(COALESCE(i.result,'{}'::jsonb)-'validationPending')||jsonb_build_object('recovered',true,'commentQualified',EXISTS(SELECT 1 FROM linggan_comment_analysis_work w WHERE w.work_ref=i.work_ref AND w.state IN ('succeeded','no_signal'))) WHERE i.state='running' AND i.created_at<scope_001_now()-interval '120 seconds'").execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_analysis_work w SET state='pending',failure_code=NULL WHERE state='failed' AND failure_code IN ('provider_unavailable','provider_timeout','lease_expired') AND updated_at<scope_001_now()-interval '30 seconds' AND EXISTS(SELECT 1 FROM linggan_comment_model_work b JOIN linggan_model_config c ON c.config_ref=b.config_ref JOIN linggan_model_plan p ON p.plan_ref=b.plan_ref WHERE b.work_ref=w.work_ref AND p.enabled AND w.attempts<c.max_attempts)").execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_analysis_work w SET state=CASE WHEN w.attempts<c.max_attempts THEN 'pending' ELSE 'failed' END,lease_ref=NULL,lease_until=NULL,failure_code='lease_expired',updated_at=scope_001_now() FROM linggan_comment_model_work b JOIN linggan_model_config c ON c.config_ref=b.config_ref WHERE w.work_ref=b.work_ref AND w.state='running' AND w.lease_until<=scope_001_now()").execute(&mut *tx).await?;
    // Also settle pending rows left by an older recovery or the generic three-attempt
    // retry endpoint. Frozen model configuration remains the stricter execution limit.
    sqlx::query("UPDATE linggan_comment_analysis_work w SET state='failed',failure_code=COALESCE(w.failure_code,'lease_expired'),updated_at=scope_001_now() FROM linggan_comment_model_work b JOIN linggan_model_config c ON c.config_ref=b.config_ref WHERE w.work_ref=b.work_ref AND w.state='pending' AND w.attempts>=c.max_attempts").execute(&mut *tx).await?;
    let row=sqlx::query("SELECT b.work_ref,b.config_ref,b.plan_ref,model.model_ref,model.model_id,v.version_ref,config.input_token_limit,config.output_token_limit,config.timeout_seconds FROM linggan_comment_model_work b JOIN linggan_comment_analysis_work work USING(work_ref) JOIN linggan_model_plan plan ON plan.plan_ref=b.plan_ref JOIN linggan_model_config config ON config.config_ref=b.config_ref JOIN linggan_model_entry model ON model.model_ref=config.model_ref JOIN linggan_model_connection_version v ON v.version_ref=model.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE plan.enabled AND c.enabled AND work.state='pending' AND work.attempts<config.max_attempts AND EXISTS(SELECT 1 FROM linggan_comment_research_readable source WHERE source.material_ref=work.source_ref) AND NOT EXISTS(SELECT 1 FROM linggan_model_invocation call WHERE call.work_ref=b.work_ref AND call.state='running') AND (SELECT COALESCE(sum(charged_tokens),0) FROM linggan_model_invocation call WHERE call.plan_ref=plan.plan_ref)+config.input_token_limit+config.output_token_limit<=plan.token_limit ORDER BY b.created_at,b.work_ref FOR UPDATE OF plan,c SKIP LOCKED LIMIT 1")
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let reserved = ReservedCall {
        invocation: Uuid::new_v4(),
        work: row.get("work_ref"),
        config: row.get("config_ref"),
        version: row.get("version_ref"),
        model_id: row.get("model_id"),
        input_limit: row.get("input_token_limit"),
        output_limit: row.get("output_token_limit"),
        timeout: row.get("timeout_seconds"),
    };
    let total = i64::from(reserved.input_limit + reserved.output_limit);
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,work_ref,plan_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens) VALUES($1,$2,$3,$4,$5,$6,'analyze',$7,'running',$8,$8)")
        .bind(reserved.invocation).bind(reserved.version).bind(row.get::<Uuid,_>("model_ref")).bind(reserved.work).bind(row.get::<Uuid,_>("plan_ref"))
        .bind(reserved.config).bind(reserved.work.to_string()).bind(total).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(reserved))
}
async fn no_call(db: &Database, reserved: &ReservedCall, code: &str) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code=$2,input_tokens=0,output_tokens=0,charged_tokens=0,result=$3,finished_at=scope_001_now() WHERE invocation_ref=$1 AND state='running'")
        .bind(reserved.invocation).bind(code).bind(json!({"callStarted":false,"failureCode":code})).execute(db.pool()).await?;
    Ok(())
}
pub async fn run_model_work_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    sync_automatic_model_work(db).await?;
    let Some(reserved) = reserve_call(db).await? else {
        return Ok(false);
    };
    let request = connection_request(db, store, reserved.version).await;
    let mut request = match request {
        Ok(r) => r,
        Err(e) => {
            no_call(db, &reserved, e.code()).await?;
            return Err(e);
        }
    };
    let claimed =
        claim_selected_comment_analysis(db, &model_version(reserved.config), Some(reserved.work))
            .await;
    let input = match claimed {
        Ok(Some(i)) => i,
        Ok(None) => {
            no_call(db, &reserved, "claim_conflict").await?;
            return Ok(false);
        }
        Err(e) => {
            no_call(db, &reserved, "model_source_unavailable").await?;
            return Err(e.into());
        }
    };
    request.model_id = reserved.model_id.clone();
    request.max_output_tokens = reserved.output_limit;
    request.timeout_ms = reserved.timeout as u64 * 1000;
    let prompt = analysis_prompt(&input)?;
    if prompt.len() + input.instruction.len() + 512 > reserved.input_limit as usize {
        no_call(db, &reserved, "model_input_limit").await?;
        fail_comment_analysis(
            db,
            input.work_ref,
            input.lease_ref,
            CommentAnalysisFailure::InvalidOutput,
        )
        .await?;
        return Ok(true);
    }
    // Read dispatch permission again immediately before sending material; in-flight completion
    // remains auditable if a person subsequently pauses the connection or plan.
    let allowed:bool=sqlx::query_scalar("SELECT p.enabled AND c.enabled FROM linggan_comment_model_work b JOIN linggan_model_plan p ON p.plan_ref=b.plan_ref JOIN linggan_model_connection_version v ON v.version_ref=$2 JOIN linggan_model_connection c USING(connection_ref) WHERE b.work_ref=$1")
        .bind(reserved.work).bind(reserved.version).fetch_one(db.pool()).await?;
    if !allowed {
        no_call(db, &reserved, "model_disabled").await?;
        fail_comment_analysis(
            db,
            input.work_ref,
            input.lease_ref,
            CommentAnalysisFailure::ProviderUnavailable,
        )
        .await?;
        return Ok(true);
    }
    let provider = PiCommentModel {
        adapter,
        request,
        input_limit: reserved.input_limit,
        receipt: Mutex::new(None),
    };
    let output = provider.analyze(&input).await;
    let receipt = provider
        .receipt
        .into_inner()
        .map_err(|_| ModelError::AdapterUnavailable)?;
    finish_comment_call(db, &reserved, &input, output, receipt).await?;
    Ok(true)
}
async fn finish_comment_call(
    db: &Database,
    reserved: &ReservedCall,
    input: &CommentAnalysisInput,
    output: Result<CommentAnalysisOutput, CommentAnalysisFailure>,
    receipt: Option<Result<PiResponse, &'static str>>,
) -> Result<(), ModelError> {
    let response = receipt.as_ref().and_then(|r| r.as_ref().ok());
    let overrun = response.is_some_and(|r| {
        r.usage
            .input_tokens
            .is_some_and(|v| v > i64::from(reserved.input_limit))
            || r.usage
                .output_tokens
                .is_some_and(|v| v > i64::from(reserved.output_limit))
    });
    // Store SDK usage before business validation. Invalid candidates may still have cost.
    checkpoint_invocation_usage(db, reserved.invocation, response).await?;
    let outcome = match output {
        Ok(output) if !overrun => complete_comment_analysis(db, input, &output)
            .await
            .map_err(|_| "model_invalid_output"),
        Ok(_) => Err("model_budget_overrun"),
        Err(CommentAnalysisFailure::ProviderTimeout) => Err("provider_timeout"),
        Err(CommentAnalysisFailure::InvalidOutput) => Err("model_invalid_output"),
        Err(_) => Err("provider_unavailable"),
    };
    let failure = outcome.as_ref().err().copied();
    if let Some(code) = failure {
        let reason = if code == "provider_timeout" {
            CommentAnalysisFailure::ProviderTimeout
        } else if code == "provider_unavailable" {
            CommentAnalysisFailure::ProviderUnavailable
        } else {
            CommentAnalysisFailure::InvalidOutput
        };
        let _ = fail_comment_analysis(db, input.work_ref, input.lease_ref, reason).await;
    }
    sqlx::query("UPDATE linggan_model_invocation SET state=$2,failure_code=$3,result=$4,finished_at=scope_001_now() WHERE invocation_ref=$1 AND state='running'")
        .bind(reserved.invocation).bind(if outcome.is_ok(){"succeeded"}else{"failed"}).bind(failure)
        .bind(json!({"callStarted":true,"commentQualified":outcome.is_ok(),"workRef":reserved.work,"sourceRef":input.source_ref,"configRef":reserved.config})).execute(db.pool()).await?;
    Ok(())
}
pub async fn model_schema_ready(db: &Database) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT to_regclass('linggan_model_workspace') IS NOT NULL")
        .fetch_one(db.pool())
        .await
        .unwrap_or(false)
}
pub async fn run_model_worker(db: Database) {
    let store = model_secret_store();
    let adapter = PiAdapter::configured();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
    loop {
        interval.tick().await;
        if !model_schema_ready(&db).await {
            continue;
        }
        let _ = model_worker_heartbeat(&db, "running", None).await;
        match run_model_work_once(&db, store.as_ref(), &adapter).await {
            Ok(_) => {
                let _ = model_worker_heartbeat(&db, "idle", None).await;
            }
            Err(error) => {
                eprintln!("comment model worker: {}", error.code());
                let _ = model_worker_heartbeat(&db, "error", Some(error.code())).await;
            }
        }
    }
}

pub async fn model_worker_heartbeat(
    db: &Database,
    state: &str,
    error: Option<&str>,
) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_model_workspace SET worker_last_seen_at=scope_001_now(),worker_state=$1,worker_last_error=$2 WHERE singleton")
        .bind(state).bind(error).execute(db.pool()).await?;
    Ok(())
}
