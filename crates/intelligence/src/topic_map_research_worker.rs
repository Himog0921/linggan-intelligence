//! Existing worker lane: bounded canonical queue -> atomic budget -> adapter -> typed acceptance.
use crate::{
    model_invocation::{connection_request, finish_invocation_in},
    model_secrets::ModelSecretStore,
    model_settings::{ModelError, reserve_research_model_semantic_dispatch_permit},
    pi_adapter::{PiAdapter, PiResponse},
    topic_map_research::{self, ResearchError, ResearchInput},
    topic_map_research_analysis::{self as analysis, ResearchOutput},
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
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
        sqlx::query("UPDATE linggan_topic_map_research_task SET state=CASE WHEN $2 THEN 'unknown_dispatch' WHEN attempt_count>=(SELECT c.max_attempts FROM linggan_topic_map_research_run r JOIN linggan_model_config c ON c.config_ref=r.config_ref WHERE r.run_ref=linggan_topic_map_research_task.run_ref) THEN 'failed' ELSE 'queued' END,last_reason=CASE WHEN $2 THEN 'unknown_dispatch' ELSE 'request_not_started' END,lease_token=NULL,lease_expires_at=NULL WHERE task_ref=$1").bind(r.get::<Uuid,_>("task_ref")).bind(sent).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE linggan_topic_map_research_run r SET state='completed',updated_at=scope_001_now() WHERE r.state IN ('queued','running') AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref AND t.state IN ('queued','running'))").execute(&mut *tx).await?;
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
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_research_policy')IS NOT NULL")
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
    // Deterministic round-robin: smallest previous dispatch count serves history as well as live work.
    let row=sqlx::query("SELECT t.*,r.config_ref,r.method_version FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref) JOIN linggan_topic_map_research_policy p ON p.domain_ref=r.domain_ref JOIN observation_domain d ON d.domain_ref=r.domain_ref JOIN linggan_model_config c ON c.config_ref=r.config_ref WHERE t.state='queued' AND t.attempt_count<c.max_attempts AND r.state IN ('queued','running') AND p.status='active' AND (p.automatic_enabled OR r.trigger='on_demand') AND d.status='active' ORDER BY (SELECT count(*)FROM linggan_topic_map_research_request q WHERE q.run_ref=r.run_ref), CASE r.trigger WHEN 'on_demand' THEN 0 WHEN 'incremental' THEN 1 ELSE 2 END,t.created_at LIMIT 1").fetch_optional(db.pool()).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let domain: Uuid = row.get("domain_ref");
    let work: Uuid = row.get("work_public_ref");
    let task: Uuid = row.get("task_ref");
    let run: Uuid = row.get("run_ref");
    let config: Uuid = row.get("config_ref");
    let hash: String = row.get("input_hash");
    let all = topic_map_research::load_inputs(db, domain, config)
        .await
        .map_err(err)?;
    let context: Vec<Uuid> = row.get::<serde_json::Value, _>("input_refs")["contextWorkRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().and_then(|s| s.parse().ok()))
        .collect();
    let input = all
        .iter()
        .find(|i| i.work.work_ref == work)
        .cloned()
        .map(|i| topic_map_research::with_comparison_context(i, &all, &context))
        .filter(|i| i.hash == hash);
    let Some(mut input) = input else {
        sqlx::query("UPDATE linggan_topic_map_research_task SET state='stale',last_reason='source_or_definition_changed' WHERE task_ref=$1 AND state='queued'").bind(task).execute(db.pool()).await?;
        return Ok(true);
    };
    let mut permit = match reserve_research_model_semantic_dispatch_permit(db, config).await {
        Ok(p) => p,
        Err(e) => {
            sqlx::query(
                "UPDATE linggan_topic_map_research_run SET last_reason=$2 WHERE run_ref=$1",
            )
            .bind(run)
            .bind(e.code())
            .execute(db.pool())
            .await?;
            return Ok(false);
        }
    };
    let model=sqlx::query("SELECT c.*,m.model_id,m.connection_version_ref FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref)WHERE c.config_ref=$1").bind(config).fetch_one(db.pool()).await?;
    let input_limit = i64::from(model.get::<i32, _>("input_token_limit"));
    let prompt = loop {
        let prompt=json!({"contract":"topic-map.research.v1","input":{"domain":input.domain,"workRef":work,"role":"author_work","comparisonWorkRefs":input.context_work_refs,"comparisonBoundary":"at most ten observed canonical works; role/low-performance contrasts unknown unless explicit supplied","fragments":input.fragments,"definitions":input.topics,"commentStudy":input.comment_study,"roleMetadata":input.role_metadata},"outputSchema":analysis::output_schema()}).to_string();
        if (prompt.len() + analysis::SYSTEM.len() + 128) as i64 <= input_limit {
            break prompt;
        }
        if let Some(longest) = input
            .fragments
            .iter_mut()
            .filter(|f| f.text.chars().count() > 16)
            .max_by_key(|f| f.text.len())
        {
            let next = (longest.text.chars().count() / 2).max(16);
            longest.text = longest.text.chars().take(next).collect();
            longest.end = longest.start + next;
        } else {
            sqlx::query("UPDATE linggan_topic_map_research_task SET state='failed',last_reason='model_input_limit' WHERE task_ref=$1 AND state='queued'").bind(task).execute(db.pool()).await?;
            permit.release().await?;
            return Ok(true);
        }
    };
    let estimate = (prompt.len() + analysis::SYSTEM.len() + 128) as i64;
    let reserved = estimate + i64::from(model.get::<i32, _>("output_token_limit"));
    let mut provider = connection_request(db, secrets, model.get("connection_version_ref")).await?;
    provider.operation = "analyze".into();
    provider.model_id = model.get("model_id");
    provider.timeout_ms = model.get::<i32, _>("timeout_seconds") as u64 * 1000;
    provider.max_output_tokens = model.get("output_token_limit");
    provider.system = analysis::SYSTEM.into();
    provider.prompt = prompt;
    let invocation = Uuid::new_v4();
    let lease = Uuid::new_v4();
    let mut tx = permit.begin().await?;
    let policy=sqlx::query("SELECT p.*,d.status AS domain_status FROM linggan_topic_map_research_policy p JOIN observation_domain d USING(domain_ref) WHERE p.domain_ref=$1 FOR UPDATE OF p").bind(domain).fetch_one(&mut *tx).await?;
    let run_row =
        sqlx::query("SELECT * FROM linggan_topic_map_research_run WHERE run_ref=$1 FOR UPDATE")
            .bind(run)
            .fetch_one(&mut *tx)
            .await?;
    let task_row =
        sqlx::query("SELECT * FROM linggan_topic_map_research_task WHERE task_ref=$1 FOR UPDATE")
            .bind(task)
            .fetch_one(&mut *tx)
            .await?;
    if policy.get::<String, _>("status") != "active"
        || policy.get::<String, _>("domain_status") != "active"
        || !["queued", "running"].contains(&run_row.get::<String, _>("state").as_str())
        || task_row.get::<String, _>("state") != "queued"
    {
        tx.rollback().await?;
        permit.release().await?;
        return Ok(false);
    }
    let run_used:i64=sqlx::query_scalar("SELECT COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref)WHERE q.run_ref=$1").bind(run).fetch_one(&mut *tx).await?;
    let daily_used:i64=sqlx::query_scalar("SELECT COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) JOIN linggan_topic_map_research_run r USING(run_ref) WHERE r.domain_ref=$1 AND q.budget_day=(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date").bind(domain).fetch_one(&mut *tx).await?;
    let pause = if run_used + reserved > run_row.get::<i64, _>("token_limit") {
        Some("run_budget_exhausted")
    } else if daily_used + reserved > policy.get::<i64, _>("daily_token_limit") {
        Some("daily_budget_paused")
    } else {
        None
    };
    if let Some(reason) = pause {
        sqlx::query("UPDATE linggan_topic_map_research_run SET state=$2,last_reason=$2,updated_at=scope_001_now()WHERE run_ref=$1").bind(run).bind(reason).execute(&mut *tx).await?;
        tx.commit().await?;
        permit.release().await?;
        return Ok(true);
    }
    let ordinal = task_row.get::<i32, _>("attempt_count") + 1;
    let request_hash=linggan_evidence::creator_discovery::hash(&json!({"input":hash,"model":config,"method":analysis::METHOD_VERSION,"system":analysis::SYSTEM,"prompt":provider.prompt}).to_string());
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)").bind(invocation).bind(model.get::<Uuid,_>("connection_version_ref")).bind(model.get::<Uuid,_>("model_ref")).bind(config).bind(&request_hash).bind(reserved).bind(json!({"purpose":"topic_map","domainRef":domain,"runRef":run,"taskRef":task,"methodVersion":analysis::METHOD_VERSION})).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,deadline_at)VALUES($1,$2,$3,$4,$5,$6,(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now()+make_interval(secs=>$7))").bind(invocation).bind(task).bind(run).bind(ordinal).bind(&request_hash).bind(topic_map_research::reference_manifest(&input)).bind(model.get::<i32,_>("timeout_seconds")+30).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='running',attempt_count=$2,lease_token=$3,lease_expires_at=scope_001_now()+interval '90 seconds'WHERE task_ref=$1").bind(task).bind(ordinal).bind(lease).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_topic_map_research_run SET state='running',updated_at=scope_001_now()WHERE run_ref=$1").bind(run).execute(&mut *tx).await?;
    tx.commit().await?;
    // Just before transmission, serialize with stop/pause and a new budget day.
    let mut tx = permit.begin().await?;
    let enabled:bool=sqlx::query_scalar("SELECT r.state='running' AND p.status='active' AND d.status='active' AND q.budget_day=(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_run r USING(run_ref)JOIN linggan_topic_map_research_policy p ON p.domain_ref=r.domain_ref JOIN observation_domain d ON d.domain_ref=r.domain_ref WHERE q.invocation_ref=$1 FOR UPDATE OF p,r").bind(invocation).fetch_one(&mut *tx).await?;
    if !enabled {
        sqlx::query("UPDATE linggan_model_invocation SET state='failed',charged_tokens=0,failure_code='pre_dispatch_paused',finished_at=scope_001_now()WHERE invocation_ref=$1").bind(invocation).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_topic_map_research_task SET state='queued',lease_token=NULL WHERE task_ref=$1 AND lease_token=$2").bind(task).bind(lease).execute(&mut *tx).await?;
        tx.commit().await?;
        permit.release().await?;
        return Ok(true);
    }
    sqlx::query("UPDATE linggan_topic_map_research_request SET dispatch_started_at=scope_001_now()WHERE invocation_ref=$1").bind(invocation).execute(&mut *tx).await?;
    tx.commit().await?;
    let response = adapter.call(&provider).await;
    let now_inputs = topic_map_research::load_inputs(db, domain, config)
        .await
        .map_err(err)?;
    let current = now_inputs
        .iter()
        .find(|i| i.work.work_ref == work)
        .cloned()
        .map(|i| topic_map_research::with_comparison_context(i, &now_inputs, &context))
        .filter(|i| i.hash == hash);
    let raw = response
        .as_ref()
        .ok()
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_ref())
        .and_then(|s| {
            if s.len() <= 65536 {
                serde_json::from_str::<ResearchOutput>(s).ok()
            } else {
                None
            }
        });
    let topic_refs: Vec<Uuid> = input
        .topics
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["topicRef"].as_str().and_then(|s| s.parse().ok()))
        .collect();
    let accepted = raw.as_ref().is_some_and(|o| {
        analysis::validate_output(o, &input.fragments, &topic_refs).is_ok()
            && o.journey.evidence.iter().all(|c| {
                input
                    .work
                    .fragments
                    .iter()
                    .any(|f| f.fragment_id == c.fragment_id)
            })
            && o.response_matches.iter().all(|r| {
                ["unknown", "not_applicable"].contains(&r.status.as_str())
                    || r.evidence.iter().any(|c| {
                        input.fragments.iter().any(|f| {
                            f.fragment_id == c.fragment_id
                                && ["studied_comment", "unresearched_comment"]
                                    .contains(&f.field.as_str())
                        })
                    })
            })
    }) && current.is_some();
    settle(
        db,
        invocation,
        task,
        run,
        lease,
        &input,
        response.as_ref().ok(),
        raw.filter(|_| accepted),
    )
    .await?;
    permit.release().await?;
    Ok(true)
}

async fn settle(
    db: &Database,
    invocation: Uuid,
    task: Uuid,
    run: Uuid,
    lease: Uuid,
    input: &ResearchInput,
    response: Option<&PiResponse>,
    output: Option<ResearchOutput>,
) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    let owner:Option<Uuid>=sqlx::query_scalar("SELECT task_ref FROM linggan_topic_map_research_task WHERE task_ref=$1 AND lease_token=$2 AND state='running'FOR UPDATE").bind(task).bind(lease).fetch_optional(&mut *tx).await?;
    if owner.is_none() {
        finish_invocation_in(
            &mut tx,
            invocation,
            response,
            false,
            Some("late_result_rejected"),
            &json!({"accepted":false}),
        )
        .await?;
        tx.commit().await?;
        return Ok(());
    }
    let mut state = "failed";
    let mut reason = Some("output_or_source_rejected");
    if let Some(output) = output {
        state = match output.outcome.as_str() {
            "no_signal" => "no_signal",
            "insufficient" => "insufficient",
            _ => "succeeded",
        };
        reason = None;
        sqlx::query("INSERT INTO linggan_topic_map_research_result(result_ref,domain_ref,work_public_ref,input_hash,output_json,method_version,invocation_ref)SELECT $1,domain_ref,work_public_ref,input_hash,$3,$4,$5 FROM linggan_topic_map_research_task WHERE task_ref=$2").bind(Uuid::new_v4()).bind(task).bind(serde_json::to_value(&output).map_err(|_|ModelError::InvalidOutput)?).bind(analysis::METHOD_VERSION).bind(invocation).execute(&mut *tx).await?;
        let candidates: Vec<crate::topic_map::TopicMapCandidateRequest> = output
            .discussions
            .iter()
            .map(|d| crate::topic_map::TopicMapCandidateRequest {
                label: d.label.clone(),
                topic_ref: d.topic_ref,
                evidence_citations: d
                    .evidence
                    .iter()
                    .filter_map(|c| {
                        input
                            .work
                            .fragments
                            .iter()
                            .find(|f| f.fragment_id == c.fragment_id)
                            .map(|f| crate::topic_map::TopicMapCitation {
                                fragment_id: f.fragment_id.clone(),
                                source_ref: f.source_ref,
                                field: f.field.clone(),
                            })
                    })
                    .collect(),
            })
            .filter(|d| !d.evidence_citations.is_empty())
            .collect();
        crate::topic_map::accept_topic_map_candidates_in(
            &mut tx,
            input.domain["domainRef"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .ok_or(ModelError::Source)?,
            input.work.work_ref,
            &candidates,
        )
        .await
        .map_err(|e| match e {
            crate::topic_map::TopicMapError::Database(e) => ModelError::Database(e),
            _ => ModelError::InvalidOutput,
        })?;
        // Model annotation shares the result transaction; references are validated against current input.
        if !output.journey.evidence.is_empty() {
            append_annotation(&mut tx, invocation, input, &output).await?;
        }
    } else if response.is_some_and(|r| {
        !r.ok
            && r.diagnostic
                .as_ref()
                .is_some_and(|d| d.stage == "request_not_started")
    }) {
        let remains:bool=sqlx::query_scalar("SELECT t.attempt_count<c.max_attempts FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref)JOIN linggan_model_config c ON c.config_ref=r.config_ref WHERE task_ref=$1").bind(task).fetch_one(&mut *tx).await?;
        state = if remains { "queued" } else { "failed" };
        reason = Some("request_not_started");
    } else if response.is_none()
        || response.is_some_and(|r| {
            r.diagnostic
                .as_ref()
                .is_none_or(|d| d.response_started.is_none() && d.stage != "request_not_started")
        })
    {
        state = "unknown_dispatch";
        reason = Some("unknown_dispatch");
    }
    finish_invocation_in(
        &mut tx,
        invocation,
        response,
        reason.is_none(),
        reason,
        &json!({"accepted":reason.is_none()}),
    )
    .await?;
    sqlx::query("UPDATE linggan_topic_map_research_task SET state=$3,last_reason=$4,lease_token=NULL,lease_expires_at=NULL,updated_at=scope_001_now()WHERE task_ref=$1 AND lease_token=$2").bind(task).bind(lease).bind(state).bind(reason).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_topic_map_research_run SET state=CASE WHEN EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state IN ('queued','running'))THEN state ELSE 'completed' END,updated_at=scope_001_now()WHERE run_ref=$1 AND state IN ('queued','running')").bind(run).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
async fn append_annotation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    invocation: Uuid,
    input: &ResearchInput,
    output: &ResearchOutput,
) -> Result<(), ModelError> {
    let domain: Uuid = sqlx::query_scalar(
        "SELECT domain_ref FROM linggan_topic_map_research_result WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(&mut **tx)
    .await?;
    let citations:Vec<_>=output.journey.evidence.iter().filter_map(|c|input.fragments.iter().find(|f|f.fragment_id==c.fragment_id).map(|f|json!({"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"field":f.field,"start":c.start,"end":c.end}))).collect();
    let request = crate::topic_map::TopicMapAnnotationRequest {
        idempotency_key: format!("topic-map-invocation:{invocation}"),
        domain_ref: domain,
        work_public_ref: input.work.work_ref,
        topic_ref: None,
        definition_ref: None,
        method_version: analysis::METHOD_VERSION.into(),
        main_stage: output.journey.main_stage.clone(),
        involved_stages: output.journey.involved_stages.clone(),
        overlays: output.journey.overlays.clone(),
        path: output.journey.path.clone(),
        rationale: output.journey.rationale.clone(),
        evidence_citations: citations
            .iter()
            .map(|v| crate::topic_map::TopicMapCitation {
                fragment_id: v["fragmentId"].as_str().unwrap_or("").into(),
                source_ref: v["sourceRef"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(Uuid::nil()),
                field: v["field"].as_str().unwrap_or("").into(),
            })
            .collect(),
    };
    crate::topic_map::append_work_annotation_in(tx, &request)
        .await
        .map_err(|e| match e {
            crate::topic_map::TopicMapError::Database(e) => ModelError::Database(e),
            _ => ModelError::InvalidOutput,
        })?;
    Ok(())
}
