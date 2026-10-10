//! Budget reservation and last-moment dispatch guards in the existing worker lane.
use super::*;
use crate::model_settings::ResearchModelSemanticDispatchPermit;
use crate::pi_adapter::PiRequest;

pub(super) async fn execute(
    db: &Database,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    row: sqlx::postgres::PgRow,
) -> Result<bool, ModelError> {
    let domain: Uuid = row.get("domain_ref");
    let work: Uuid = row.get("work_public_ref");
    let task: Uuid = row.get("task_ref");
    let run: Uuid = row.get("run_ref");
    let config: Uuid = row.get("config_ref");
    let all = topic_map_research::load_inputs(db, domain, config)
        .await
        .map_err(err)?;
    let manifest: Value = row.get("input_refs");
    let input = topic_map_research::restore_scoped_window(&all, work, &manifest);
    let Some(input) = input else {
        sqlx::query("UPDATE linggan_topic_map_research_task SET state='stale',last_reason='source_or_definition_changed' WHERE task_ref=$1 AND state='queued'").bind(task).execute(db.pool()).await?;
        return Ok(true);
    };
    let model=sqlx::query("SELECT c.*,m.model_id,m.connection_version_ref FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref)WHERE c.config_ref=$1").bind(config).fetch_one(db.pool()).await?;
    let input_limit = i64::from(model.get::<i32, _>("input_token_limit"));
    let mut prepared = prepare(db, adapter, &input, &row, input_limit).await?;
    if prepared.definition_unavailable {
        sqlx::query("UPDATE linggan_topic_map_research_task SET state='stale',last_reason='definition_source_unavailable' WHERE task_ref=$1 AND state='queued'").bind(task).execute(db.pool()).await?;
        return Ok(true);
    }
    let estimate = analysis::estimate_input_tokens(&prepared.system, &prepared.prompt);
    if estimate > input_limit {
        sqlx::query("UPDATE linggan_topic_map_research_task SET state='failed',last_reason='model_input_limit' WHERE task_ref=$1 AND state='queued'").bind(task).execute(db.pool()).await?;
        return Ok(true);
    }
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
    if !fresh_preparation(db, &row, &prepared).await? {
        permit.release().await?;
        return Ok(true);
    }
    let reserved = estimate + i64::from(model.get::<i32, _>("output_token_limit"));
    let mut provider = connection_request(db, secrets, model.get("connection_version_ref")).await?;
    provider.operation = "analyze".into();
    provider.model_id = model.get("model_id");
    provider.timeout_ms = model.get::<i32, _>("timeout_seconds") as u64 * 1000;
    provider.max_output_tokens = model.get("output_token_limit");
    provider.system = prepared.system.clone();
    provider.prompt = std::mem::take(&mut prepared.prompt);
    let invocation = Uuid::new_v4();
    let lease = Uuid::new_v4();
    let pending = Pending {
        row: &row,
        model: &model,
        input: &input,
        prepared: &prepared,
        provider: &provider,
        invocation,
        lease,
        reserved,
    };
    match reserve(&mut permit, &pending).await? {
        Gate::Idle => {
            permit.release().await?;
            return Ok(false);
        }
        Gate::Paused => {
            permit.release().await?;
            return Ok(true);
        }
        Gate::Ready => {}
    }
    if !authorize_send(db, &mut permit, &pending).await? {
        permit.release().await?;
        return Ok(true);
    }
    let response = adapter.call(&provider).await;
    accept_response(db, &pending, response.as_ref().ok()).await?;
    permit.release().await?;
    Ok(true)
}

async fn fresh_preparation(
    db: &Database,
    row: &sqlx::postgres::PgRow,
    prepared: &Prepared,
) -> Result<bool, ModelError> {
    let (available, unavailable) = source_snapshot(db, row).await?;
    let candidates_current = candidate_sources_current(prepared, &unavailable);
    if available && candidates_current {
        return Ok(true);
    }
    let (state, reason) = if available {
        ("queued", "definition_changed_recalling")
    } else {
        ("stale", "pre_dispatch_source_changed")
    };
    sqlx::query("UPDATE linggan_topic_map_research_task SET state=$2,last_reason=$3,phase_attempt_count=0 WHERE task_ref=$1 AND state='queued' AND phase=$4 AND attempt_count=$5")
        .bind(row.get::<Uuid,_>("task_ref")).bind(state).bind(reason)
        .bind(row.get::<String,_>("phase")).bind(row.get::<i32,_>("attempt_count"))
        .execute(db.pool()).await?;
    Ok(false)
}
pub(super) async fn source_snapshot(
    db: &Database,
    row: &sqlx::postgres::PgRow,
) -> Result<(bool, std::collections::HashSet<Uuid>), ModelError> {
    let domain: Uuid = row.get("domain_ref");
    let all = topic_map_research::load_inputs(db, domain, row.get("config_ref"))
        .await
        .map_err(err)?;
    let manifest: Value = row.get("input_refs");
    let unavailable = core::unavailable_definitions(db, Some(domain)).await?;
    let available =
        topic_map_research::restore_scoped_window(&all, row.get("work_public_ref"), &manifest)
            .is_some()
            && core::definition_dependencies(&manifest).is_disjoint(&unavailable);
    Ok((available, unavailable))
}
pub(super) fn candidate_sources_current(
    prepared: &Prepared,
    unavailable: &std::collections::HashSet<Uuid>,
) -> bool {
    prepared
        .candidates
        .as_array()
        .into_iter()
        .flatten()
        .all(|c| {
            c["definitionRef"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|id| !unavailable.contains(&id))
        })
}
async fn accept_response(
    db: &Database,
    pending: &Pending<'_>,
    response: Option<&PiResponse>,
) -> Result<(), ModelError> {
    let row = pending.row;
    let prepared = pending.prepared;
    let (current, unavailable) = source_snapshot(db, row).await?;
    let (extracted, resolved) = validated_response(
        response,
        &row.get::<String, _>("phase"),
        pending.input,
        prepared,
    );
    let eligible_definitions = prepared
        .eligible_definitions
        .iter()
        .copied()
        .filter(|id| !unavailable.contains(id))
        .collect();
    settle(
        db,
        super::settlement::Completion {
            invocation: pending.invocation,
            task: row.get("task_ref"),
            run: row.get("run_ref"),
            lease: pending.lease,
            input: pending.input,
            response,
            extracted,
            resolved,
            prepared,
            source_current: current,
            candidate_sources_current: candidate_sources_current(prepared, &unavailable),
            eligible_definitions,
        },
    )
    .await
}

enum Gate {
    Ready,
    Idle,
    Paused,
}
struct Pending<'a> {
    row: &'a sqlx::postgres::PgRow,
    model: &'a sqlx::postgres::PgRow,
    input: &'a ResearchInput,
    prepared: &'a Prepared,
    provider: &'a PiRequest,
    invocation: Uuid,
    lease: Uuid,
    reserved: i64,
}
async fn reserve(
    permit: &mut ResearchModelSemanticDispatchPermit,
    p: &Pending<'_>,
) -> Result<Gate, ModelError> {
    let domain: Uuid = p.row.get("domain_ref");
    let task: Uuid = p.row.get("task_ref");
    let run: Uuid = p.row.get("run_ref");
    let config: Uuid = p.row.get("config_ref");
    let hash: String = p.row.get("input_hash");
    let phase: String = p.row.get("phase");
    let Pending {
        model,
        input,
        prepared,
        provider,
        invocation,
        lease,
        reserved,
        ..
    } = p;
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
        || !dispatch_authorized(
            policy.get("automatic_enabled"),
            &run_row.get::<String, _>("trigger"),
            &run_row.get::<Value, _>("input_scope"),
            &task_row.get::<Value, _>("recall_manifest"),
            task_row.get("work_public_ref"),
        )
        || task_row.get::<i32, _>("phase_attempt_count") >= model.get::<i32, _>("max_attempts")
        || !same_task_snapshot(&task_row, p.row)
    {
        tx.rollback().await?;
        return Ok(Gate::Idle);
    }
    let run_used:i64=sqlx::query_scalar("SELECT COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref)WHERE q.run_ref=$1").bind(run).fetch_one(&mut *tx).await?;
    let daily_used:i64=sqlx::query_scalar("SELECT COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) JOIN linggan_topic_map_research_run r USING(run_ref) WHERE r.domain_ref=$1 AND q.budget_day=(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date").bind(domain).fetch_one(&mut *tx).await?;
    let pause = if run_used + *reserved > run_row.get::<i64, _>("token_limit") {
        Some("run_budget_exhausted")
    } else if daily_used + *reserved > policy.get::<i64, _>("daily_token_limit") {
        Some("daily_budget_paused")
    } else {
        None
    };
    if let Some(reason) = pause {
        sqlx::query("UPDATE linggan_topic_map_research_run SET state=$2,last_reason=$2,updated_at=scope_001_now()WHERE run_ref=$1").bind(run).bind(reason).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(Gate::Paused);
    }
    let ordinal = task_row.get::<i32, _>("attempt_count") + 1;
    let request_hash=linggan_evidence::creator_discovery::hash(&json!({"input":hash,"model":config,"method":analysis::METHOD_VERSION,"phase":phase,"system":provider.system,"prompt":provider.prompt}).to_string());
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)").bind(invocation).bind(model.get::<Uuid,_>("connection_version_ref")).bind(model.get::<Uuid,_>("model_ref")).bind(config).bind(&request_hash).bind(reserved).bind(json!({"purpose":"topic_map","domainRef":domain,"runRef":run,"taskRef":task,"methodVersion":analysis::METHOD_VERSION})).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_topic_map_research_request(invocation_ref,task_ref,run_ref,attempt_ordinal,request_hash,request_manifest,budget_day,deadline_at,phase)VALUES($1,$2,$3,$4,$5,$6,(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date,scope_001_now()+make_interval(secs=>$7),$8)").bind(invocation).bind(task).bind(run).bind(ordinal).bind(&request_hash).bind(json!({"source":topic_map_research::reference_manifest(&input),"phase":phase,"unitIds":prepared.units.iter().map(|(id,_)|id).collect::<Vec<_>>(),"topics":prepared.candidates,"recall":prepared.recall})).bind(model.get::<i32,_>("timeout_seconds")+30).bind(&phase).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_topic_map_research_task SET state='running',attempt_count=$2,phase_attempt_count=phase_attempt_count+1,lease_token=$3,lease_expires_at=scope_001_now()+make_interval(secs=>$4)WHERE task_ref=$1").bind(task).bind(ordinal).bind(lease).bind(model.get::<i32,_>("timeout_seconds")+30).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_topic_map_research_run SET state='running',updated_at=scope_001_now()WHERE run_ref=$1").bind(run).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Gate::Ready)
}

async fn authorize_send(
    db: &Database,
    permit: &mut ResearchModelSemanticDispatchPermit,
    pending: &Pending<'_>,
) -> Result<bool, ModelError> {
    let invocation = pending.invocation;
    let task: Uuid = pending.row.get("task_ref");
    let lease = pending.lease;
    // Just before transmission, serialize with stop/pause and a new budget day.
    let mut tx = permit.begin().await?;
    let current = sqlx::query("SELECT r.state='running' AND p.status='active' AND d.status='active' AND q.budget_day=(scope_001_now()AT TIME ZONE 'Asia/Shanghai')::date AS enabled,p.automatic_enabled,r.trigger,r.input_scope,t.recall_manifest,t.work_public_ref FROM linggan_topic_map_research_request q JOIN linggan_topic_map_research_run r ON r.run_ref=q.run_ref JOIN linggan_topic_map_research_policy p ON p.domain_ref=r.domain_ref JOIN observation_domain d ON d.domain_ref=r.domain_ref JOIN linggan_topic_map_research_task t ON t.task_ref=q.task_ref WHERE q.invocation_ref=$1 AND t.task_ref=$2 AND t.lease_token=$3 AND t.state='running' FOR UPDATE OF p,r,t")
        .bind(invocation).bind(task).bind(lease).fetch_optional(&mut *tx).await?;
    let enabled = current.is_some_and(|row| {
        row.get::<bool, _>("enabled")
            && dispatch_authorized(
                row.get("automatic_enabled"),
                &row.get::<String, _>("trigger"),
                &row.get::<Value, _>("input_scope"),
                &row.get::<Value, _>("recall_manifest"),
                row.get("work_public_ref"),
            )
    });
    // All possibly blocking policy/run/task locks have been acquired. Recheck
    // source and definition dependencies at the actual transmission boundary.
    let denial = if !enabled {
        Some(("queued", "pre_dispatch_paused"))
    } else {
        let (available, unavailable) = source_snapshot(db, pending.row).await?;
        if !available {
            Some(("stale", "pre_dispatch_source_changed"))
        } else if !candidate_sources_current(pending.prepared, &unavailable) {
            Some(("queued", "definition_changed_recalling"))
        } else {
            // Another configuration may have been queued before the preceding
            // call became unknown while this task waited for the shared model.
            let all = topic_map_research::load_inputs(
                db,
                pending.row.get("domain_ref"),
                pending.row.get("config_ref"),
            )
            .await
            .map_err(err)?;
            if core::comparison::blocks_unknown_dispatch(
                &mut tx,
                pending.row.get("domain_ref"),
                pending.input,
                &all,
            )
            .await?
            {
                Some(("stale", "prior_unknown_dispatch"))
            } else {
                None
            }
        }
    };
    if let Some((state, reason)) = denial {
        sqlx::query("UPDATE linggan_model_invocation SET state='failed',charged_tokens=0,failure_code=$2,finished_at=scope_001_now() WHERE invocation_ref=$1")
            .bind(invocation).bind(reason).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_topic_map_research_task SET state=$3,last_reason=$4,phase_attempt_count=GREATEST(0,phase_attempt_count-1),lease_token=NULL,lease_expires_at=NULL WHERE task_ref=$1 AND lease_token=$2")
            .bind(task).bind(lease).bind(state).bind(reason).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(false);
    }
    sqlx::query("UPDATE linggan_topic_map_research_request SET dispatch_started_at=scope_001_now()WHERE invocation_ref=$1").bind(invocation).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}

fn dispatch_authorized(
    automatic: bool,
    trigger: &str,
    scope: &Value,
    recall: &Value,
    work: Uuid,
) -> bool {
    automatic
        || (trigger == "on_demand" && scope.get("backfillReopened").is_none_or(|v| *v == false))
        || core::backfill::task_is_explicitly_authorized(scope, recall, work)
}

fn validated_response(
    response: Option<&PiResponse>,
    phase: &str,
    input: &ResearchInput,
    prepared: &Prepared,
) -> (Option<ResearchOutput>, Option<ResolutionOutput>) {
    let text = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_ref())
        .filter(|s| s.len() <= 65536);
    let extracted = if phase == "extract" || phase == "compare" {
        text.and_then(|s| serde_json::from_str::<ResearchOutput>(s).ok())
            .filter(|o| {
                o.contract == analysis::EXTRACT_CONTRACT
                    && analysis::validate_output(o, &input.fragments, &[]).is_ok()
            })
    } else {
        None
    };
    let resolved = if phase == "resolve" {
        text.and_then(|s| serde_json::from_str::<ResolutionOutput>(s).ok())
            .filter(|o| {
                analysis::validate_resolution(o, &prepared.units, &prepared.candidates).is_ok()
            })
    } else {
        None
    };
    (extracted, resolved)
}

/// A model permit may wait while another worker advances this same queued task.
/// Recheck the frozen preparation before reserving tokens or transmitting anything.
fn same_task_snapshot(current: &sqlx::postgres::PgRow, frozen: &sqlx::postgres::PgRow) -> bool {
    ["attempt_count", "resolution_cursor"]
        .iter()
        .all(|key| current.get::<i32, _>(*key) == frozen.get::<i32, _>(*key))
        && ["phase", "input_hash"]
            .iter()
            .all(|key| current.get::<String, _>(*key) == frozen.get::<String, _>(*key))
        && ["input_refs", "recall_manifest", "resolutions_json"]
            .iter()
            .all(|key| current.get::<Value, _>(*key) == frozen.get::<Value, _>(*key))
        && current.get::<Option<Value>, _>("distilled_json")
            == frozen.get::<Option<Value>, _>("distilled_json")
}
