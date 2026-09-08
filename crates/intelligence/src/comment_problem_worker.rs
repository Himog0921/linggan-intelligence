#[path = "comment_problem_vectors.rs"]
mod vectors;
// Durable Task B queue. Shares the existing batch token ledger; extraction item states
// are never modified. Missing qualified retrieval is explicit and dispatches no request.
use crate::{
    comment_intelligence_problems::{
        PROBLEM_RELATION_VERSION, apply_problem_task_output, prepare_problem_relation,
    },
    comment_research::comment_source_hash,
    model_invocation::{checkpoint_invocation_usage, connection_request, finish_invocation},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

/// An authorized extraction batch can expose pending problem work even before embedding
/// capability is configured. This is a blocked queue, not an invented successful analysis.
pub async fn sync_problem_tasks(db: &Database) -> Result<u64, ModelError> {
    let rows=sqlx::query("SELECT DISTINCT ON(c.candidate_ref) c.candidate_ref,b.batch_ref,b.config_ref FROM linggan_ci_problem_candidate c JOIN linggan_comment_daily_item i ON i.analysis_ref=c.analysis_ref JOIN linggan_comment_daily_batch b USING(batch_ref) WHERE EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=c.domain_ref AND d.is_own_domain) AND c.state='unmerged' AND i.state='succeeded' AND b.enabled AND b.request->>'ruleVersion'='comment-research.v4' AND COALESCE((b.context_policy->>'existingProblems')::boolean,true) AND NOT EXISTS(SELECT 1 FROM linggan_ci_problem_task t WHERE t.candidate_ref=c.candidate_ref AND t.contract_version=$1) ORDER BY c.candidate_ref,b.created_at DESC LIMIT 100")
        .bind(PROBLEM_RELATION_VERSION).fetch_all(db.pool()).await?;
    let mut added = 0;
    for row in rows {
        let candidate: Uuid = row.get("candidate_ref");
        added+=sqlx::query("INSERT INTO linggan_ci_problem_task(task_ref,candidate_ref,batch_ref,config_ref,contract_version,input_fingerprint,state,failure_code) VALUES($1,$2,$3,$4,$5,$6,'blocked_retrieval','problem_retrieval_not_configured') ON CONFLICT(candidate_ref,contract_version,input_fingerprint) DO NOTHING")
            .bind(Uuid::new_v4()).bind(candidate).bind(row.get::<Uuid,_>("batch_ref")).bind(row.get::<Uuid,_>("config_ref"))
            .bind(PROBLEM_RELATION_VERSION).bind(format!("pending:{candidate}")).execute(db.pool()).await?.rows_affected();
    }
    Ok(added)
}

/// Called only by the qualified retrieval pipeline after it has produced target IDs.
/// It reassembles the request from current DB evidence; callers cannot provide arbitrary prompts.
pub async fn queue_problem_task(
    db: &Database,
    batch: Uuid,
    domain: Uuid,
    candidate: Uuid,
    targets: &[Uuid],
    retrieval_version: &str,
) -> Result<Uuid, ModelError> {
    let config:Uuid=sqlx::query_scalar("SELECT b.config_ref FROM linggan_comment_daily_batch b JOIN linggan_comment_daily_item i USING(batch_ref) JOIN linggan_ci_problem_candidate c ON c.analysis_ref=i.analysis_ref WHERE EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=c.domain_ref AND d.is_own_domain) AND b.batch_ref=$1 AND c.candidate_ref=$2 AND c.domain_ref=$3 AND b.enabled AND b.request->>'ruleVersion'='comment-research.v4' AND COALESCE((b.context_policy->>'existingProblems')::boolean,true) AND i.state='succeeded' LIMIT 1")
        .bind(batch).bind(candidate).bind(domain).fetch_optional(db.pool()).await?.ok_or(ModelError::Disabled)?;
    let task = prepare_problem_relation(db, domain, candidate, targets, retrieval_version)
        .await
        .map_err(|_| ModelError::Source)?;
    let fingerprint = comment_source_hash(&json!({"task":task,"configRef":config}).to_string());
    let id=sqlx::query_scalar("INSERT INTO linggan_ci_problem_task(task_ref,candidate_ref,batch_ref,config_ref,contract_version,input_fingerprint,state,task_snapshot,expires_at) VALUES($1,$2,$3,$4,$5,$6,'pending',$7,scope_001_now()+interval '24 hours') ON CONFLICT(candidate_ref,contract_version,input_fingerprint) DO UPDATE SET input_fingerprint=EXCLUDED.input_fingerprint RETURNING task_ref")
        .bind(Uuid::new_v4()).bind(candidate).bind(batch).bind(config).bind(PROBLEM_RELATION_VERSION).bind(fingerprint).bind(task).fetch_one(db.pool()).await?;
    // A configured request supersedes the marker but not any executed request history.
    sqlx::query("UPDATE linggan_ci_problem_task SET state='superseded',failure_code='retrieval_configured',updated_at=scope_001_now() WHERE candidate_ref=$1 AND state='blocked_retrieval'")
        .bind(candidate).execute(db.pool()).await?;
    Ok(id)
}

pub async fn problem_task_status(db: &Database, domain: Uuid) -> Result<Value, ModelError> {
    let rows=sqlx::query("SELECT t.state,t.failure_code,count(*)::bigint AS n FROM linggan_ci_problem_task t JOIN linggan_ci_problem_candidate c USING(candidate_ref) JOIN linggan_ci_source s USING(canonical_ref,domain_ref) WHERE c.domain_ref=$1 AND c.state<>'superseded' GROUP BY t.state,t.failure_code ORDER BY t.state,t.failure_code")
        .bind(domain).fetch_all(db.pool()).await?;
    Ok(
        json!({"contractVersion":PROBLEM_RELATION_VERSION,"states":rows.iter().map(|r|json!({"state":r.get::<String,_>("state"),"reason":r.get::<Option<String>,_>("failure_code"),"count":r.get::<i64,_>("n")})).collect::<Vec<_>>()}),
    )
}

async fn recover(db: &Database) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    let expired=sqlx::query("SELECT task_ref,invocation_ref,packet_ref,applied_receipt FROM linggan_ci_problem_task WHERE state='running' AND lease_until<=scope_001_now() FOR UPDATE SKIP LOCKED").fetch_all(&mut *tx).await?;
    for row in expired {
        if let Some(receipt) = row.get::<Option<Value>, _>("applied_receipt") {
            recover_accepted_task(&mut tx, &row, &receipt).await?;
            continue;
        }
        sqlx::query("UPDATE linggan_model_invocation SET state='failed',failure_code='worker_interrupted',finished_at=scope_001_now(),result=jsonb_build_object('task','problem_relation','usageUnknown',input_tokens IS NULL OR output_tokens IS NULL) WHERE invocation_ref=$1 AND state='running'")
            .bind(row.get::<Option<Uuid>,_>("invocation_ref")).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_comment_daily_packet SET state='failed',finished_at=scope_001_now() WHERE packet_ref=$1 AND purpose IN ('problem_relation','problem_embedding') AND state='running'")
            .bind(row.get::<Option<Uuid>,_>("packet_ref")).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_ci_problem_task SET state='failed',failure_code='worker_interrupted',updated_at=scope_001_now() WHERE task_ref=$1")
            .bind(row.get::<Uuid,_>("task_ref")).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE linggan_ci_problem_task SET task_snapshot=NULL,purged_at=scope_001_now(),state=CASE WHEN state='pending' THEN 'failed' ELSE state END,failure_code=CASE WHEN state='pending' THEN 'context_snapshot_expired' ELSE failure_code END WHERE expires_at<=scope_001_now() AND task_snapshot IS NOT NULL AND state<>'running'").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn run_problem_relation_once(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    recover(db).await?;
    sync_problem_tasks(db).await?;
    let mut tx = db.pool().begin().await?;
    // Same lock as Task A: simultaneous workers cannot over-reserve the batch family.
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let row=sqlx::query("SELECT b.context_policy,t.task_ref,t.task_snapshot,t.batch_ref,t.config_ref,cfg.input_token_limit,cfg.output_token_limit,cfg.timeout_seconds,m.model_ref,m.model_id,m.connection_version_ref FROM linggan_ci_problem_task t JOIN linggan_comment_daily_batch b USING(batch_ref) JOIN linggan_model_config cfg ON cfg.config_ref=t.config_ref JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version cv ON cv.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE EXISTS(SELECT 1 FROM linggan_ci_problem_candidate c JOIN observation_domain d USING(domain_ref) WHERE c.candidate_ref=t.candidate_ref AND c.state='unmerged' AND d.is_own_domain) AND t.applied_receipt IS NULL AND t.state='pending' AND t.expires_at>scope_001_now() AND b.enabled AND conn.enabled AND b.request->>'ruleVersion'='comment-research.v4' AND COALESCE((b.context_policy->>'existingProblems')::boolean,true) AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled)) AND (b.kind<>'supplement' OR EXISTS(SELECT 1 FROM linggan_comment_daily_batch origin WHERE origin.batch_ref::text=b.request->>'originBatchRef' AND origin.enabled)) AND NOT EXISTS(SELECT 1 FROM linggan_comment_daily_item unfinished WHERE unfinished.batch_ref=b.batch_ref AND unfinished.state IN('pending','running')) AND COALESCE((SELECT sum(v.charged_tokens) FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) JOIN linggan_comment_daily_batch family ON family.batch_ref=p.batch_ref WHERE COALESCE(family.request->>'originBatchRef',family.batch_ref::text)=COALESCE(b.request->>'originBatchRef',b.batch_ref::text)),0)+cfg.input_token_limit+cfg.output_token_limit<=COALESCE((SELECT (a.request->>'tokenLimit')::bigint FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.token_limit) ORDER BY t.created_at,t.task_ref LIMIT 1 FOR UPDATE OF t")
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        tx.commit().await?;
        return vectors::advance_problem_retrieval(db, store, adapter).await;
    };
    let task: Value = row.get("task_snapshot");
    let prompt=json!({"contractVersion":task["contractVersion"],"responseSchema":task["responseSchema"],"untrustedMaterial":{"expression":task["expression"],"targets":task["targets"]}}).to_string();
    let system = task["system"].as_str().ok_or(ModelError::Invalid)?;
    let id: Uuid = row.get("task_ref");
    let input: i32 = row.get("input_token_limit");
    let output: i32 = row.get("output_token_limit");
    if prompt.len() + system.len() + 512 > input as usize {
        sqlx::query("UPDATE linggan_ci_problem_task SET state='failed',failure_code='model_input_limit',updated_at=scope_001_now() WHERE task_ref=$1").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(true);
    }
    let invocation = Uuid::new_v4();
    let packet = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6)")
        .bind(invocation).bind(row.get::<Uuid,_>("connection_version_ref")).bind(row.get::<Uuid,_>("model_ref")).bind(row.get::<Uuid,_>("config_ref")).bind(comment_source_hash(&prompt)).bind(i64::from(input+output)).execute(&mut *tx).await?;
    let refs: Vec<Uuid> = serde_json::from_value(task["sourceGuard"]["researchSourceRefs"].clone())
        .map_err(|_| ModelError::Source)?;
    sqlx::query("INSERT INTO linggan_comment_daily_packet(packet_ref,batch_ref,source_refs,context_refs,context_hash,invocation_ref,lease_until,state,purpose) VALUES($1,$2,$3,$3,$4,$5,scope_001_now()+interval '120 seconds','running','problem_relation')")
        .bind(packet).bind(row.get::<Uuid,_>("batch_ref")).bind(refs).bind(comment_source_hash(&task.to_string())).bind(invocation).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_ci_problem_task SET state='running',invocation_ref=$2,packet_ref=$3,lease_until=scope_001_now()+interval '120 seconds',updated_at=scope_001_now() WHERE task_ref=$1")
        .bind(id).bind(invocation).bind(packet).execute(&mut *tx).await?;
    tx.commit().await?;
    let run = RelationRun {
        row,
        task,
        prompt,
        input,
        output,
        invocation,
        packet,
        id,
    };
    finish_relation_call(db, store, adapter, run).await
}
struct RelationRun {
    row: sqlx::postgres::PgRow,
    task: Value,
    prompt: String,
    input: i32,
    output: i32,
    invocation: Uuid,
    packet: Uuid,
    id: Uuid,
}
async fn finish_relation_call(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    run: RelationRun,
) -> Result<bool, ModelError> {
    let (call_started, result) = dispatch_relation(db, store, adapter, &run).await;
    let RelationRun {
        row,
        input,
        output,
        invocation,
        packet,
        id,
        ..
    } = run;
    let response = result.as_ref().ok();
    let policy = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
    crate::comment_runtime::record_response(db, invocation, response, &policy).await?;
    if call_started {
        checkpoint_invocation_usage(db, invocation, response).await?;
    }
    let mut failure = result.as_ref().err().map(|e| e.code().to_owned());
    let mut receipt = Value::Null;
    if let Some(response) = response {
        if !response.ok {
            failure = Some(
                response
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| "provider_failed".into()),
            );
        }
        if response
            .usage
            .input_tokens
            .is_some_and(|n| n > i64::from(input))
            || response
                .usage
                .output_tokens
                .is_some_and(|n| n > i64::from(output))
        {
            failure = Some("model_budget_overrun".into());
        }
        if failure.is_none() {
            match response
                .text
                .as_deref()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
            {
                Some(output) => match apply_problem_task_output(db, id, &output).await {
                    Ok(value) => receipt = value,
                    Err(_) => failure = Some("problem_relation_rejected".into()),
                },
                None => failure = Some("invalid_relation_output".into()),
            }
        }
    }
    sqlx::query("UPDATE linggan_comment_request_trace SET validation=$2,outcomes=$3,events=events||jsonb_build_array(jsonb_build_object('kind','relation_validated','at',scope_001_now())) WHERE invocation_ref=$1")
        .bind(invocation).bind(json!({"accepted":failure.is_none(),"failure":failure})).bind(&receipt).execute(db.pool()).await?;
    let succeeded = failure.is_none();
    finish_invocation(
        db,
        invocation,
        response,
        succeeded,
        failure.as_deref(),
        &json!({"task":"problem_relation","receipt":receipt,"callStarted":call_started,"usageUnknown":call_started && response.is_none()}),
    )
    .await?;
    if !call_started {
        sqlx::query("UPDATE linggan_model_invocation SET charged_tokens=0 WHERE invocation_ref=$1")
            .bind(invocation)
            .execute(db.pool())
            .await?;
    }
    let state = if !succeeded {
        "failed"
    } else if receipt["decisions"].as_array().is_some_and(|d| {
        d.iter().all(|v| v["relation"] != "same") && d.iter().any(|v| v["relation"] == "uncertain")
    }) {
        "needs_judgment"
    } else {
        "succeeded"
    };
    sqlx::query("UPDATE linggan_ci_problem_task SET state=$2,failure_code=$3,updated_at=scope_001_now() WHERE task_ref=$1").bind(id).bind(state).bind(failure).execute(db.pool()).await?;
    sqlx::query("UPDATE linggan_comment_daily_packet SET state=$2,finished_at=scope_001_now() WHERE packet_ref=$1 AND purpose='problem_relation'").bind(packet).bind(if succeeded {"succeeded"} else {"failed"}).execute(db.pool()).await?;
    Ok(true)
}

pub async fn problem_automation_state(db: &Database, domain: Uuid) -> Result<Value, ModelError> {
    let mut value = problem_task_status(db, domain).await?;
    let schedule:bool=sqlx::query_scalar("SELECT COALESCE((SELECT enabled FROM linggan_comment_daily_schedule WHERE singleton),false)").fetch_one(db.pool()).await?;
    let configured = crate::embedding_settings::active_config(db)
        .await?
        .is_some();
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_problem_task t JOIN linggan_ci_problem_candidate c USING(candidate_ref) WHERE c.domain_ref=$1 AND t.state IN('pending','running'))").bind(domain).fetch_one(db.pool()).await?;
    value["status"] = json!(if !configured {
        "retrieval_not_configured"
    } else if active {
        "running_or_queued"
    } else {
        "ready"
    });
    value["dailyEnabled"] = json!(schedule);
    let candidates=sqlx::query("SELECT c.candidate_ref,c.name,c.meaning,s.source_ref,a.result,(SELECT count(*) FROM linggan_ci_problem_boundary_decision h WHERE h.candidate_ref=c.candidate_ref AND h.origin='manual') AS revision FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=c.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 WHERE c.domain_ref=$1 AND c.state='unmerged' AND EXISTS(SELECT 1 FROM linggan_ci_problem_boundary_decision d WHERE d.candidate_ref=c.candidate_ref AND d.relation='uncertain' AND NOT EXISTS(SELECT 1 FROM linggan_ci_problem_boundary_decision h WHERE h.candidate_ref=c.candidate_ref AND h.target_ref=d.target_ref AND h.origin='manual' AND h.created_at>=d.created_at)) ORDER BY c.created_at,c.candidate_ref LIMIT 100")
        .bind(domain).fetch_all(db.pool()).await?;
    let mut judgments = Vec::new();
    for c in candidates {
        if !crate::comment_daily_read::context_readable(db, &c.get::<Value, _>("result")).await? {
            continue;
        }
        let targets:Vec<Value>=sqlx::query_scalar("SELECT DISTINCT ON(d.target_ref) jsonb_build_object('problemRef',p.problem_ref,'name',p.name,'meaning',p.meaning,'definitionRevision',p.definition_revision,'relation',d.relation,'reason',d.reason) FROM linggan_ci_problem_boundary_decision d JOIN linggan_ci_problem p ON p.problem_ref=d.target_ref AND p.definition_revision=d.target_definition_revision AND p.redirect_ref IS NULL WHERE d.candidate_ref=$1 ORDER BY d.target_ref,d.created_at DESC,d.decision_ref DESC")
            .bind(c.get::<Uuid,_>("candidate_ref")).fetch_all(db.pool()).await?;
        judgments.push(json!({"candidateRef":c.get::<Uuid,_>("candidate_ref"),"name":c.get::<String,_>("name"),"meaning":c.get::<String,_>("meaning"),"sourceRefs":[c.get::<Uuid,_>("source_ref")],"revision":c.get::<i64,_>("revision"),"targets":targets}));
    }
    value["needsJudgment"] = json!(judgments);
    Ok(value)
}

async fn begin_problem_trace(
    db: &Database,
    invocation: Uuid,
    packet: Uuid,
    task: &Value,
    policy: &crate::comment_runtime::ContextPolicy,
) -> Result<(), ModelError> {
    let mut refs = std::collections::BTreeSet::new();
    for r in task["sourceGuard"]["researchSourceRefs"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(id) = r.as_str().and_then(|s| Uuid::parse_str(s).ok()) {
            refs.insert(id);
        }
    }
    for t in task["targets"].as_array().into_iter().flatten() {
        for sample in t["evidenceSamples"].as_array().into_iter().flatten() {
            if let Some(id) = sample["sourceRef"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
            {
                refs.insert(id);
            }
        }
    }
    let mut guard = json!({"contextRefs":task["sourceGuard"]});
    guard["contextRefs"]["researchSourceRefs"] = json!(refs);
    let input = json!({"contract":task["contractVersion"],"system":task["system"],"task":"问题关系判断","expression":task["expression"],"existingProblems":task["targets"],"outputSchema":task["responseSchema"],"policy":policy});
    sqlx::query("INSERT INTO linggan_comment_request_trace(invocation_ref,packet_ref,input_content,input_hash,source_hashes,context_guard,policy,events) VALUES($1,$2,$3,$4,'{}',$5,$6,jsonb_build_array(jsonb_build_object('kind','request_started','at',scope_001_now(),'purpose','problem_relation'))) ON CONFLICT DO NOTHING")
        .bind(invocation).bind(packet).bind(if policy.record_content{Some(input)}else{None}).bind(comment_source_hash(&task.to_string())).bind(guard).bind(json!(policy)).execute(db.pool()).await?;
    Ok(())
}

async fn dispatch_relation(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    run: &RelationRun,
) -> (bool, Result<crate::pi_adapter::PiResponse, ModelError>) {
    let RelationRun {
        row,
        task,
        prompt,
        output,
        invocation,
        packet,
        ..
    } = run;
    let (invocation, packet, output) = (*invocation, *packet, *output);
    let system = task["system"].as_str().unwrap_or("");
    let mut call_started = false;
    let result = async {
        let candidate = task["expression"]["candidateRef"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(ModelError::Source)?;
        let domain = task["domain"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(ModelError::Source)?;
        let targets: Vec<Uuid> = task["targets"]
            .as_array()
            .ok_or(ModelError::Source)?
            .iter()
            .map(|t| {
                t["problemRef"]
                    .as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or(ModelError::Source)
            })
            .collect::<Result<_, _>>()?;
        let fresh = prepare_problem_relation(
            db,
            domain,
            candidate,
            &targets,
            task["retrievalVersion"]
                .as_str()
                .ok_or(ModelError::Source)?,
        )
        .await
        .map_err(|_| ModelError::Source)?;
        if fresh != *task {
            return Err(ModelError::Source);
        }
        let mut request = connection_request(db, store, row.get("connection_version_ref")).await?;
        request.operation = "analyze".into();
        request.model_id = row.get("model_id");
        request.system = system.into();
        request.prompt = prompt.clone();
        request.max_output_tokens = output;
        request.timeout_ms = row.get::<i32, _>("timeout_seconds") as u64 * 1000;
        let policy = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
        begin_problem_trace(db, invocation, packet, task, &policy).await?;
        call_started = true;
        adapter.call(&request).await
    }
    .await;
    (call_started, result)
}

fn accepted_task_state(receipt: &Value) -> &'static str {
    if receipt["decisions"].as_array().is_some_and(|d| {
        d.iter().all(|v| v["relation"] != "same") && d.iter().any(|v| v["relation"] == "uncertain")
    }) {
        "needs_judgment"
    } else {
        "succeeded"
    }
}

async fn recover_accepted_task(
    connection: &mut sqlx::PgConnection,
    row: &sqlx::postgres::PgRow,
    receipt: &Value,
) -> Result<(), ModelError> {
    let invocation = row.get::<Uuid, _>("invocation_ref");
    let unknown: bool = sqlx::query_scalar("SELECT input_tokens IS NULL OR output_tokens IS NULL FROM linggan_model_invocation WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(&mut *connection).await?;
    let reason = if unknown {
        Some("usage_review_required")
    } else {
        None
    };
    sqlx::query("UPDATE linggan_model_invocation SET state='succeeded',failure_code=$2,finished_at=scope_001_now(),result=$3 WHERE invocation_ref=$1")
        .bind(invocation).bind(reason).bind(json!({"task":"problem_relation","receipt":receipt,"accepted":true,"recovered":true,"usageUnknown":unknown,"usageReviewRequired":unknown}))
        .execute(&mut *connection).await?;
    sqlx::query("UPDATE linggan_comment_daily_packet SET state='succeeded',finished_at=scope_001_now() WHERE packet_ref=$1 AND purpose='problem_relation'")
        .bind(row.get::<Uuid,_>("packet_ref")).execute(&mut *connection).await?;
    sqlx::query("UPDATE linggan_ci_problem_task SET state=$2,failure_code=$3,updated_at=scope_001_now() WHERE task_ref=$1")
        .bind(row.get::<Uuid,_>("task_ref")).bind(accepted_task_state(receipt)).bind(reason).execute(&mut *connection).await?;
    sqlx::query("UPDATE linggan_comment_request_trace SET validation=$2,outcomes=$3,events=events||jsonb_build_array(jsonb_build_object('kind','accepted_result_recovered','at',scope_001_now(),'usageReviewRequired',$4::boolean)) WHERE invocation_ref=$1")
        .bind(invocation).bind(json!({"accepted":true,"usageReviewRequired":unknown})).bind(receipt).bind(unknown).execute(&mut *connection).await?;
    Ok(())
}
