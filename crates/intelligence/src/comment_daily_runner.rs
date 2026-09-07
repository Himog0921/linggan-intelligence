//! Durable work packets share the model invocation ledger. No remote call occurs in a transaction.
use crate::{
    comment_cleaning::{CLEANER_VERSION, clean_pending},
    comment_daily::{self, DAILY_RULE},
    comment_packet::{ResearchPacket, SYSTEM, build_packet},
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
}
async fn recover(db: &Database) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    let rows=sqlx::query("SELECT packet_ref,invocation_ref,batch_ref,source_refs FROM linggan_comment_daily_packet WHERE state='running' AND lease_until<=scope_001_now() FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for row in rows {
        sqlx::query("UPDATE linggan_comment_daily_item SET state='failed',failure_code='worker_interrupted' WHERE batch_ref=$1 AND source_ref=ANY($2) AND state='running'").bind(row.get::<Uuid,_>("batch_ref")).bind(row.get::<Vec<Uuid>,_>("source_refs")).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code='worker_interrupted',finished_at=scope_001_now(),result=jsonb_build_object('callStarted',true,'usageUnknown',input_tokens IS NULL OR output_tokens IS NULL) WHERE invocation_ref=$1 AND state='running'").bind(row.get::<Uuid,_>("invocation_ref")).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_comment_daily_packet SET state='failed',finished_at=scope_001_now() WHERE packet_ref=$1").bind(row.get::<Uuid,_>("packet_ref")).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
async fn reserve(db: &Database) -> Result<Option<Reserved>, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    // Source cap limits analysis, never the frozen manifest or intake counts.
    sqlx::query("UPDATE linggan_comment_daily_item i SET state='restricted',failure_code='source_unavailable' WHERE i.state='pending' AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_readable s WHERE s.material_ref=i.source_ref)").execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item i SET state=c.state FROM linggan_comment_clean c WHERE c.source_ref=i.source_ref AND c.cleaner_version=$1 AND c.state IN ('low_information','anomaly') AND i.state='pending'").bind(CLEANER_VERSION).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item i SET state='source_limit',failure_code='source_limit' FROM linggan_comment_daily_batch b,linggan_comment_clean c WHERE i.batch_ref=b.batch_ref AND c.source_ref=i.source_ref AND c.cleaner_version=$1 AND c.state IN ('direct','context') AND i.state='pending' AND i.attempts=0 AND (SELECT count(*) FROM linggan_comment_daily_item used WHERE used.batch_ref=b.batch_ref AND used.attempts>0)>=b.source_limit").bind(CLEANER_VERSION).execute(&mut *tx).await?;
    let row=sqlx::query("SELECT b.batch_ref,b.config_ref,b.token_limit,(b.source_limit-(SELECT count(*) FROM linggan_comment_daily_item used WHERE used.batch_ref=b.batch_ref AND used.attempts>0)) AS remaining_sources,cfg.input_token_limit,cfg.output_token_limit,cfg.timeout_seconds,cfg.max_attempts,m.model_ref,m.model_id,v.version_ref,s.content_public_ref FROM linggan_comment_daily_batch b JOIN linggan_model_config cfg USING(config_ref) JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) JOIN linggan_comment_daily_item i USING(batch_ref) JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref JOIN linggan_comment_clean c ON c.source_ref=i.source_ref AND c.cleaner_version=$1 WHERE b.enabled AND conn.enabled AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)) AND i.state='pending' AND i.attempts<cfg.max_attempts AND c.state IN ('direct','context') AND COALESCE((SELECT sum(v.charged_tokens) FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=b.batch_ref),0)+cfg.input_token_limit+cfg.output_token_limit<=b.token_limit ORDER BY b.window_end,b.batch_ref,s.content_public_ref,i.source_ref LIMIT 1")
        .bind(CLEANER_VERSION).fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let batch: Uuid = row.get("batch_ref");
    let config: Uuid = row.get("config_ref");
    let input_limit: i32 = row.get("input_token_limit");
    let output_limit: i32 = row.get("output_token_limit");
    let max_comments = i64::from((output_limit / 256).clamp(1, 30))
        .min(row.get::<i64, _>("remaining_sources").max(1));
    let mut refs:Vec<Uuid>=sqlx::query_scalar("SELECT i.source_ref FROM linggan_comment_daily_item i JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref JOIN linggan_comment_clean c ON c.source_ref=i.source_ref AND c.cleaner_version=$3 WHERE i.batch_ref=$1 AND s.content_public_ref=$2 AND i.state='pending' AND i.attempts<$5 AND c.state IN ('direct','context') ORDER BY i.source_ref LIMIT $4")
        .bind(batch).bind(row.get::<Uuid,_>("content_public_ref")).bind(CLEANER_VERSION).bind(max_comments).bind(row.get::<i32,_>("max_attempts")).fetch_all(&mut *tx).await?;
    let research = loop {
        let packet = build_packet(db, &refs, &comment_daily::version(config)).await?;
        if packet.prompt.len() + SYSTEM.len() + 512 <= input_limit as usize {
            break packet;
        }
        if refs.len() == 1 {
            sqlx::query("UPDATE linggan_comment_daily_item SET state='failed',failure_code='model_input_limit' WHERE batch_ref=$1 AND source_ref=$2").bind(batch).bind(refs[0]).execute(&mut *tx).await?;
            tx.commit().await?;
            return Ok(None);
        }
        refs.pop();
    };
    // A short reply without either parent or work text must remain context-insufficient.
    let missing: Vec<Uuid> = research
        .inputs
        .iter()
        .zip(&research.cleaned)
        .filter(|(i, c)| {
            c.state == "context"
                && i.context
                    .pointer("/parent/body")
                    .and_then(Value::as_str)
                    .is_none()
                && i.context
                    .pointer("/work/body/value")
                    .and_then(Value::as_str)
                    .is_none()
        })
        .map(|(i, _)| i.source_ref)
        .collect();
    if !missing.is_empty() {
        sqlx::query("UPDATE linggan_comment_daily_item SET state='context_missing',failure_code='context_missing' WHERE batch_ref=$1 AND source_ref=ANY($2)").bind(batch).bind(missing).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let invocation = Uuid::new_v4();
    let packet = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6)")
        .bind(invocation).bind(row.get::<Uuid,_>("version_ref")).bind(row.get::<Uuid,_>("model_ref")).bind(config).bind(crate::comment_research::comment_source_hash(&research.prompt)).bind(i64::from(input_limit+output_limit)).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_comment_daily_packet(packet_ref,batch_ref,source_refs,context_refs,context_hash,invocation_ref,lease_until,state) VALUES($1,$2,$3,$4,$5,$6,scope_001_now()+interval '120 seconds','running')")
        .bind(packet).bind(batch).bind(&refs).bind(&research.context_refs).bind(&research.context_hash).bind(invocation).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_item SET state='running',attempts=attempts+1,failure_code=NULL WHERE batch_ref=$1 AND source_ref=ANY($2) AND state='pending'").bind(batch).bind(&refs).execute(&mut *tx).await?;
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
    }))
}
async fn permitted(db: &Database, r: &Reserved) -> Result<bool, ModelError> {
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_batch b JOIN linggan_model_config cfg USING(config_ref) JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE b.batch_ref=$1 AND b.enabled AND c.enabled AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)))").bind(r.batch).fetch_one(db.pool()).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_readable WHERE material_ref=ANY($1)",
    )
    .bind(&r.research.context_refs)
    .fetch_one(db.pool())
    .await?;
    Ok(active && count == r.research.context_refs.len() as i64)
}
pub async fn run_daily_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    if !comment_daily::schema_ready(db).await? {
        return Ok(false);
    }
    recover(db).await?;
    comment_daily::seal_due(db).await?;
    clean_pending(db).await?;
    let Some(r) = reserve(db).await? else {
        return Ok(false);
    };
    let mut request = match connection_request(db, store, r.connection).await {
        Ok(v) => v,
        Err(e) => {
            finish(db, &r, None, Some(e.code()), false).await?;
            return Ok(true);
        }
    };
    if !permitted(db, &r).await? {
        finish(db, &r, None, Some("model_disabled"), false).await?;
        return Ok(true);
    }
    request.operation = "analyze".into();
    request.model_id = r.model.clone();
    request.prompt = r.research.prompt.clone();
    request.system = SYSTEM.into();
    request.max_output_tokens = r.output_limit;
    request.timeout_ms = r.timeout as u64 * 1000;
    let outcome = adapter.call(&request).await;
    let response = outcome.as_ref().ok();
    checkpoint_invocation_usage(db, r.invocation, response).await?;
    let mut failure = outcome.as_ref().err().map(ModelError::code);
    if let Some(p) = response {
        if !p.ok {
            failure = Some("provider_failed");
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
    if !permitted(db, &r).await? {
        failure = Some("source_or_plan_unavailable");
    }
    let refs: Vec<_> = r.research.inputs.iter().map(|i| i.source_ref).collect();
    match build_packet(db, &refs, &comment_daily::version(r.config)).await {
        Ok(current) if current.context_hash == r.research.context_hash => {}
        _ => failure = Some("context_changed"),
    }
    finish(db, &r, response, failure, true).await?;
    Ok(true)
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
            .parse(response.and_then(|v| v.text.as_deref()).unwrap_or(""))
    };
    let global_failure = parsed.as_ref().err().copied();
    let mut tx = db.pool().begin().await?;
    let valid:bool=sqlx::query_scalar("SELECT state='running' AND lease_until>scope_001_now() FROM linggan_comment_daily_packet WHERE packet_ref=$1 FOR UPDATE").bind(r.packet).fetch_one(&mut *tx).await?;
    if !valid {
        return Err(ModelError::Conflict);
    }
    let outcomes =
        parsed.unwrap_or_else(|code| r.research.inputs.iter().map(|_| Err(code)).collect());
    let mut success = 0;
    for (input, result) in r.research.inputs.iter().zip(outcomes) {
        let (state, code, analysis_ref) = match result {
            Ok(mut value) => {
                let state = if value["spans"].as_array().is_some_and(Vec::is_empty) {
                    "no_signal"
                } else {
                    "succeeded"
                };
                let reference = Uuid::new_v4();
                // A batch is a frozen run version; retries never replace previous accepted analyses.
                let version = format!("{}:{}", comment_daily::version(r.config), r.batch);
                value["modelVersion"] = json!(version);
                let actual:Uuid=sqlx::query_scalar("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result,attempts) VALUES($1,$2,$3,$4,$5,$6,1) RETURNING work_ref")
                    .bind(reference).bind(input.source_ref).bind(DAILY_RULE).bind(version).bind(state).bind(value).fetch_one(&mut *tx).await?;
                success += 1;
                (state, None, Some(actual))
            }
            Err(code) => ("failed", Some(code), None),
        };
        sqlx::query("UPDATE linggan_comment_daily_item SET state=$3,failure_code=$4,analysis_ref=$5 WHERE batch_ref=$1 AND source_ref=$2 AND state='running'")
            .bind(r.batch).bind(input.source_ref).bind(state).bind(code).bind(analysis_ref).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE linggan_comment_daily_packet SET state=$2,finished_at=scope_001_now() WHERE packet_ref=$1").bind(r.packet).bind(if success==r.research.inputs.len(){"succeeded"}else if success>0{"partial"}else{"failed"}).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_model_invocation SET state=$2,failure_code=$3,result=$4,finished_at=scope_001_now(),charged_tokens=CASE WHEN $5 THEN charged_tokens ELSE 0 END,input_tokens=CASE WHEN $5 THEN input_tokens ELSE 0 END,output_tokens=CASE WHEN $5 THEN output_tokens ELSE 0 END WHERE invocation_ref=$1 AND state='running'")
        .bind(r.invocation).bind(if success==r.research.inputs.len(){"succeeded"}else{"failed"}).bind(global_failure.or(if success<r.research.inputs.len(){Some("partial_output")}else{None}))
        .bind(json!({"callStarted":started,"batchRef":r.batch,"packetRef":r.packet,"acceptedComments":success,"requestedComments":r.research.inputs.len(),"failureCode":global_failure})).bind(started).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
