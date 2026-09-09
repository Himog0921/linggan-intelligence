//! The only P3 model dispatch path. It reserves through the shared daily ledger, records an
//! independent metadata-only replay trace, and never writes a formal analysis or atom.
use super::{
    FrozenReplayMember, ReplayModelSnapshot, authorization_usage_in, load_member,
    read_model_snapshot,
};
use crate::{
    comment_execution_budget::{BudgetPurpose, check_budget_in},
    comment_packet::{ResearchPacket, build_packet_with_rule},
    comment_replay_metrics::{
        ReplayObservation, ReplayPair, ReplayRepeat, ReplayThresholds, compare_with_schema_mode,
    },
    comment_research::comment_source_hash,
    comment_research_rules::{ActivateRuleRevision, RuleSnapshot, activate_rule_after_policy_in},
    comment_runtime::ContextPolicy,
    model_invocation::{connection_request, finish_invocation_in},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    model_worker_drain::ModelWorkerDrain,
    pi_adapter::{PiAdapter, PiResponse, safe_result},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const RETRYABLE_FAILURES: [&str; 6] = [
    "provider_unavailable",
    "provider_timeout",
    "provider_rate_limited",
    "provider_stream_interrupted",
    "provider_terminal_missing",
    "provider_network_error",
];

pub async fn run_next_replay(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
    drain: Option<&ModelWorkerDrain>,
) -> Result<bool, ModelError> {
    if drain_requested(drain) || production_new_intake_waiting(db).await? {
        return Ok(false);
    }
    let Some(next) = next_queued_item(db).await? else {
        return Ok(false);
    };
    let rule: RuleSnapshot = serde_json::from_value(if next.side == "baseline" {
        next.baseline_snapshot.clone()
    } else {
        next.candidate_snapshot.clone()
    })
    .map_err(|_| ModelError::Invalid)?;
    let frozen_model: ReplayModelSnapshot =
        serde_json::from_value(next.model_snapshot.clone()).map_err(|_| ModelError::Invalid)?;
    let current_model = read_model_snapshot(db, next.config_ref).await?;
    if !same_model_snapshot(&frozen_model, &current_model) {
        fail_run(db, next.run_ref, "model_snapshot_changed").await?;
        return Ok(true);
    }
    // A disabled connection is a no-dispatch condition. The queued run remains inspectable and
    // can resume only after the normal model settings path re-enables a matching version.
    if !current_model.connection_enabled {
        return Ok(false);
    }
    let policy = ContextPolicy::parse(next.context_policy.clone())?;
    let member = load_member(db, next.sample_set_ref, next.member_ordinal).await?;
    let packet = match build_packet_with_rule(
        db,
        &[member.source_ref],
        &frozen_model.model_id,
        &policy,
        &rule,
    )
    .await
    {
        Ok(packet) => packet,
        Err(ModelError::Source) => {
            exclude_member_pair(db, next.run_ref, member.ordinal, "source_unavailable").await?;
            reconcile_run(db, next.run_ref).await?;
            return Ok(true);
        }
        Err(_) if next.side == "baseline" => {
            fail_run(db, next.run_ref, "baseline_unavailable").await?;
            return Ok(true);
        }
        Err(_) => {
            fail_run(db, next.run_ref, "candidate_builder_unavailable").await?;
            return Ok(true);
        }
    };
    if !matches_member(&packet, &member) {
        exclude_member_pair(db, next.run_ref, member.ordinal, "frozen_input_changed").await?;
        reconcile_run(db, next.run_ref).await?;
        return Ok(true);
    }
    if packet.prompt.len() + packet.system.len() + 512 > frozen_model.input_token_limit as usize {
        exclude_member_pair(
            db,
            next.run_ref,
            member.ordinal,
            "frozen_builder_input_limit",
        )
        .await?;
        reconcile_run(db, next.run_ref).await?;
        return Ok(true);
    }
    if drain_requested(drain) {
        return Ok(false);
    }
    let mut request = connection_request(db, store, frozen_model.connection_version_ref).await?;
    request.operation = "analyze".into();
    request.model_id = frozen_model.model_id.clone();
    request.timeout_ms = u64::try_from(frozen_model.timeout_seconds)
        .map_err(|_| ModelError::Invalid)?
        .saturating_mul(1000);
    request.max_output_tokens = frozen_model.output_token_limit;
    request.system = packet.system.into();
    request.prompt = packet.prompt.clone();
    let request_hash = comment_source_hash(&packet.prompt);
    let Some(reserved) = reserve_item(
        db,
        &next,
        &member,
        &rule,
        &policy,
        &frozen_model,
        &request_hash,
        drain,
    )
    .await?
    else {
        return Ok(false);
    };
    let response = adapter.call(&request).await;
    finish_item(db, &reserved, &packet, response).await?;
    reconcile_run(db, reserved.run_ref).await?;
    Ok(true)
}

#[derive(Clone)]
struct QueuedReplayItem {
    item_ref: Uuid,
    run_ref: Uuid,
    sample_set_ref: Uuid,
    member_ordinal: i16,
    side: String,
    repeat_ordinal: i16,
    attempt_ordinal: i16,
    config_ref: Uuid,
    baseline_snapshot: Value,
    candidate_snapshot: Value,
    model_snapshot: Value,
    context_policy: Value,
}

struct ReservedReplayItem {
    item_ref: Uuid,
    run_ref: Uuid,
    member_ordinal: i16,
    side: String,
    repeat_ordinal: i16,
    attempt_ordinal: i16,
    invocation_ref: Uuid,
    input_limit: i32,
    output_limit: i32,
}

struct FinishedReplayItem {
    failure: Option<String>,
    structure_accepted: bool,
    no_signal: bool,
    summary: Value,
    item_state: &'static str,
}

async fn next_queued_item(db: &Database) -> Result<Option<QueuedReplayItem>, ModelError> {
    let row = sqlx::query("SELECT i.item_ref,i.run_ref,r.sample_set_ref,i.member_ordinal,i.side,i.repeat_ordinal,i.attempt_ordinal,r.config_ref,r.baseline_snapshot,r.candidate_snapshot,r.model_snapshot,r.context_policy FROM linggan_comment_replay_item i JOIN linggan_comment_replay_run r USING(run_ref) WHERE i.state='queued' AND r.state IN ('queued','running','waiting_daily_budget') ORDER BY r.created_at,r.run_ref,i.member_ordinal,i.side,i.repeat_ordinal,i.attempt_ordinal LIMIT 1")
        .fetch_optional(db.pool()).await?;
    Ok(row.map(|row| QueuedReplayItem {
        item_ref: row.get("item_ref"),
        run_ref: row.get("run_ref"),
        sample_set_ref: row.get("sample_set_ref"),
        member_ordinal: row.get("member_ordinal"),
        side: row.get("side"),
        repeat_ordinal: row.get("repeat_ordinal"),
        attempt_ordinal: row.get("attempt_ordinal"),
        config_ref: row.get("config_ref"),
        baseline_snapshot: row.get("baseline_snapshot"),
        candidate_snapshot: row.get("candidate_snapshot"),
        model_snapshot: row.get("model_snapshot"),
        context_policy: row.get("context_policy"),
    }))
}

fn same_model_snapshot(left: &ReplayModelSnapshot, right: &ReplayModelSnapshot) -> bool {
    left.config_ref == right.config_ref
        && left.model_ref == right.model_ref
        && left.model_id == right.model_id
        && left.connection_version_ref == right.connection_version_ref
        && left.input_token_limit == right.input_token_limit
        && left.output_token_limit == right.output_token_limit
        && left.timeout_seconds == right.timeout_seconds
}

fn matches_member(packet: &ResearchPacket, member: &FrozenReplayMember) -> bool {
    let Some(input) = packet.inputs.first() else {
        return false;
    };
    input.source_ref == member.source_ref
        && input.source_sha256 == member.source_hash
        && packet.context_hash == member.context_hash
        && packet.context_refs == member.context_refs
}

#[allow(clippy::too_many_arguments)]
async fn reserve_item(
    db: &Database,
    next: &QueuedReplayItem,
    member: &FrozenReplayMember,
    rule: &RuleSnapshot,
    policy: &ContextPolicy,
    model: &ReplayModelSnapshot,
    request_hash: &str,
    drain: Option<&ModelWorkerDrain>,
) -> Result<Option<ReservedReplayItem>, ModelError> {
    let requested = i64::from(model.input_token_limit + model.output_token_limit);
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if drain_requested(drain) {
        tx.rollback().await?;
        return Ok(None);
    }
    let item = sqlx::query("SELECT i.state,r.state AS run_state,r.authorized_token_budget,r.reserved_tokens,COALESCE(r.authorization_root_run_ref,r.run_ref) AS authorization_root FROM linggan_comment_replay_item i JOIN linggan_comment_replay_run r USING(run_ref) WHERE i.item_ref=$1 FOR UPDATE OF i,r")
        .bind(next.item_ref).fetch_optional(&mut *tx).await?;
    let Some(item) = item else {
        tx.commit().await?;
        return Ok(None);
    };
    if item.get::<String, _>("state") != "queued" {
        tx.commit().await?;
        return Ok(None);
    }
    let run_state: String = item.get("run_state");
    if !matches!(
        run_state.as_str(),
        "queued" | "running" | "waiting_daily_budget"
    ) {
        tx.commit().await?;
        return Ok(None);
    }
    let root: Uuid = item.get("authorization_root");
    let run_budget: i64 = sqlx::query_scalar(
        "SELECT authorized_token_budget FROM linggan_comment_replay_run WHERE run_ref=$1",
    )
    .bind(root)
    .fetch_one(&mut *tx)
    .await?;
    let run_reserved = authorization_usage_in(&mut tx, root).await?;
    if requested > run_budget.saturating_sub(run_reserved) {
        wait_for_budget(&mut tx, next.run_ref, "replay_authorized_budget_exhausted").await?;
        tx.commit().await?;
        return Ok(None);
    }
    let daily = check_budget_in(&mut tx, BudgetPurpose::Replay, requested, false).await?;
    if !daily.allowed {
        wait_for_budget(
            &mut tx,
            next.run_ref,
            daily.reason_code.unwrap_or("replay_budget_exhausted"),
        )
        .await?;
        tx.commit().await?;
        return Ok(None);
    }
    if drain_requested(drain) {
        tx.rollback().await?;
        return Ok(None);
    }
    let invocation_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)")
        .bind(invocation_ref).bind(model.connection_version_ref).bind(model.model_ref).bind(model.config_ref)
        .bind(request_hash).bind(requested).bind(&daily.ledger_metadata).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_comment_replay_trace(invocation_ref,item_ref,request_hash,source_hash,context_hash,rule_hash,policy,events) VALUES($1,$2,$3,$4,$5,$6,$7,jsonb_build_array(jsonb_build_object('kind','request_reserved','at',scope_001_now())))")
        .bind(invocation_ref).bind(next.item_ref).bind(request_hash).bind(&member.source_hash).bind(&member.context_hash)
        .bind(&rule.canonical_hash).bind(json!({"contextPolicy":policy,"replayTraceVersion":"comment-replay-trace.v1"}))
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_replay_item SET state='running',invocation_ref=$2,execution_day=$3,started_at=scope_001_now(),failure_code=NULL,exclusion_reason=NULL WHERE item_ref=$1 AND state='queued'")
        .bind(next.item_ref).bind(invocation_ref).bind(&daily.execution_day).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_replay_run SET state='running',reserved_tokens=reserved_tokens+$2,execution_day=COALESCE(execution_day,$3),failure_code=NULL,started_at=COALESCE(started_at,scope_001_now()) WHERE run_ref=$1")
        .bind(next.run_ref).bind(requested).bind(&daily.execution_day).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Some(ReservedReplayItem {
        item_ref: next.item_ref,
        run_ref: next.run_ref,
        member_ordinal: next.member_ordinal,
        side: next.side.clone(),
        repeat_ordinal: next.repeat_ordinal,
        attempt_ordinal: next.attempt_ordinal,
        invocation_ref,
        input_limit: model.input_token_limit,
        output_limit: model.output_token_limit,
    }))
}

async fn wait_for_budget(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    reason: &str,
) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_comment_replay_run SET state='waiting_daily_budget',failure_code=$2 WHERE run_ref=$1")
        .bind(run_ref).bind(reason).execute(&mut **tx).await?;
    Ok(())
}

async fn finish_item(
    db: &Database,
    reserved: &ReservedReplayItem,
    packet: &ResearchPacket,
    response: Result<PiResponse, ModelError>,
) -> Result<(), ModelError> {
    let provider = response.as_ref().ok();
    let finished = prepare_finished_item(packet, &response, reserved);
    let failure = finished.failure.as_deref();
    let structure_accepted = finished.structure_accepted;
    let item_state = finished.item_state;
    let summary = &finished.summary;
    let no_signal = finished.no_signal;
    let mut tx = db.pool().begin().await?;
    finish_invocation_in(
        &mut tx,
        reserved.invocation_ref,
        provider,
        failure.is_none(),
        failure,
        &json!({"callStarted":true,"shadowReplay":true,"itemRef":reserved.item_ref,"safe":safe_result_or_null(provider)}),
    )
    .await?;
    sqlx::query("UPDATE linggan_comment_replay_item SET state=$2,structure_accepted=$3,no_signal=$4,safe_summary=$5,input_tokens=$6,output_tokens=$7,elapsed_ms=$8,failure_code=$9,finished_at=scope_001_now() WHERE item_ref=$1 AND invocation_ref=$10 AND state='running'")
        .bind(reserved.item_ref).bind(item_state).bind(structure_accepted).bind(no_signal).bind(summary)
        .bind(provider.and_then(|value|value.usage.input_tokens)).bind(provider.and_then(|value|value.usage.output_tokens))
        .bind(provider.and_then(|value|value.elapsed_ms)).bind(failure).bind(reserved.invocation_ref)
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_replay_trace SET events=events||jsonb_build_array(jsonb_build_object('kind',$2::text,'at',scope_001_now(),'structureAccepted',$3::boolean)) WHERE invocation_ref=$1")
        .bind(reserved.invocation_ref).bind(if failure.is_none(){"response_validated"}else{"response_failed"}).bind(structure_accepted)
        .execute(&mut *tx).await?;
    if should_retry(failure, provider, reserved.attempt_ordinal) {
        schedule_retry(&mut tx, reserved).await?;
    }
    tx.commit().await?;
    Ok(())
}

fn prepare_finished_item(
    packet: &ResearchPacket,
    response: &Result<PiResponse, ModelError>,
    reserved: &ReservedReplayItem,
) -> FinishedReplayItem {
    let provider = response.as_ref().ok();
    let mut failure = response.as_ref().err().map(|error| error.code().to_owned());
    if let Some(value) = provider {
        if !value.ok {
            failure = Some(
                value
                    .failure_code
                    .clone()
                    .unwrap_or_else(|| "provider_failed".into()),
            );
        }
        if value
            .usage
            .input_tokens
            .is_some_and(|tokens| tokens > i64::from(reserved.input_limit))
            || value
                .usage
                .output_tokens
                .is_some_and(|tokens| tokens > i64::from(reserved.output_limit))
        {
            failure = Some("model_budget_overrun".into());
        }
    }
    let parsed = failure.is_none().then(|| {
        packet.parse_for_rule(
            provider
                .and_then(|value| value.text.as_deref())
                .unwrap_or(""),
        )
    });
    let result = parsed
        .as_ref()
        .and_then(|parsed| parsed.as_ref().ok())
        .and_then(|values| values.first())
        .and_then(|value| value.as_ref().ok());
    let parse_code = parsed.as_ref().and_then(|parsed| {
        parsed.as_ref().err().copied().or_else(|| {
            parsed
                .as_ref()
                .ok()
                .and_then(|values| values.first())
                .and_then(|value| value.as_ref().err().copied())
        })
    });
    let structure_accepted = result.is_some();
    let no_signal =
        result.is_some_and(|value| value.pointer("/semantic/outcome") == Some(&json!("no_signal")));
    if failure.is_none() && !structure_accepted {
        failure = Some(parse_code.unwrap_or("invalid_output").into());
    }
    let diagnostics = if provider.is_some() {
        packet.validation_diagnostics_for_rule(
            provider
                .and_then(|value| value.text.as_deref())
                .unwrap_or(""),
        )
    } else {
        vec![json!({"code":failure.as_deref(),"path":"request","actual":{"type":"not_recorded"}})]
    };
    let atom_evidence_hashes = result
        .map(normalized_atom_evidence_hashes)
        .unwrap_or_default();
    let invariant_violations = if structure_accepted {
        Vec::<&str>::new()
    } else {
        vec!["validator_rejected"]
    };
    // A syntactically complete provider response may still fail the packet validator; it is a
    // completed engineering observation. A provider failure remains a failed item so pairing
    // reports it as an explicit partial execution rather than as a fabricated schema result.
    FinishedReplayItem {
        failure,
        structure_accepted,
        no_signal,
        summary: json!({
        "schemaComparisonVersion":"comment-replay-common-fields.v1",
        "normalization":provider.and_then(|value|value.text.as_deref()).and_then(|raw|packet.normalization_kind_for_rule(raw)),
        "structureAccepted":structure_accepted,
        "noSignal":no_signal,
        "invariantViolations":invariant_violations,
        "validation":diagnostics,
        "atomEvidenceHashes":atom_evidence_hashes,
        }),
        item_state: if provider.is_some_and(|value| value.ok) {
            "succeeded"
        } else {
            "failed"
        },
    }
}

fn safe_result_or_null(response: Option<&PiResponse>) -> Value {
    response.map(safe_result).unwrap_or(Value::Null)
}

fn should_retry(
    failure: Option<&str>,
    response: Option<&PiResponse>,
    attempt_ordinal: i16,
) -> bool {
    attempt_ordinal < 2
        && failure.is_some_and(|code| RETRYABLE_FAILURES.contains(&code))
        && response.is_some_and(|value| {
            value.usage.input_tokens.is_some() && value.usage.output_tokens.is_some()
        })
}

async fn schedule_retry(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    reserved: &ReservedReplayItem,
) -> Result<(), ModelError> {
    let next_attempt = reserved.attempt_ordinal + 1;
    let hash = comment_source_hash(&json!({
        "priorItemRef":reserved.item_ref,"runRef":reserved.run_ref,"memberOrdinal":reserved.member_ordinal,
        "side":reserved.side,"repeatOrdinal":reserved.repeat_ordinal,"attemptOrdinal":next_attempt,
    }).to_string());
    sqlx::query("INSERT INTO linggan_comment_replay_item(item_ref,run_ref,member_ordinal,side,repeat_ordinal,attempt_ordinal,idempotency_hash) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(run_ref,member_ordinal,side,repeat_ordinal,attempt_ordinal) DO NOTHING")
        .bind(Uuid::new_v4()).bind(reserved.run_ref).bind(reserved.member_ordinal).bind(&reserved.side)
        .bind(reserved.repeat_ordinal).bind(next_attempt).bind(hash).execute(&mut **tx).await?;
    Ok(())
}

fn normalized_atom_evidence_hashes(result: &Value) -> Vec<String> {
    let mut hashes = BTreeSet::new();
    for atom in result
        .pointer("/semantic/atoms")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let kind = atom
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        for evidence in atom
            .get("evidence")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            hashes.insert(comment_source_hash(&json!({
                "kind":kind,"sourceRef":evidence.get("sourceRef"),"startChar":evidence.get("startChar"),"endChar":evidence.get("endChar"),
            }).to_string()));
        }
    }
    hashes.into_iter().collect()
}

async fn exclude_member_pair(
    db: &Database,
    run_ref: Uuid,
    member_ordinal: i16,
    reason: &str,
) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_comment_replay_item SET state='excluded',exclusion_reason=$3,failure_code=NULL,finished_at=COALESCE(finished_at,scope_001_now()) WHERE run_ref=$1 AND member_ordinal=$2 AND state IN ('queued','running','succeeded','failed','waiting_daily_budget')")
        .bind(run_ref).bind(member_ordinal).bind(reason).execute(db.pool()).await?;
    Ok(())
}

async fn fail_run(db: &Database, run_ref: Uuid, code: &str) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_comment_replay_run SET state='failed',failure_code=$2,finished_at=scope_001_now() WHERE run_ref=$1 AND state NOT IN ('pass','degraded','insufficient_evidence')")
        .bind(run_ref).bind(code).execute(db.pool()).await?;
    Ok(())
}

async fn reconcile_run(db: &Database, run_ref: Uuid) -> Result<(), ModelError> {
    let pending: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_replay_item WHERE run_ref=$1 AND state IN ('queued','running','waiting_daily_budget'))")
        .bind(run_ref).fetch_one(db.pool()).await?;
    if pending {
        return Ok(());
    }
    let run = sqlx::query("SELECT sample_set_ref,candidate_rule_revision_ref,baseline_snapshot,candidate_snapshot,thresholds,state,run_kind FROM linggan_comment_replay_run WHERE run_ref=$1")
        .bind(run_ref).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    if run.get::<String, _>("state") == "failed" {
        return Ok(());
    }
    let thresholds: ReplayThresholds =
        serde_json::from_value(run.get("thresholds")).map_err(|_| ModelError::Invalid)?;
    let members = sqlx::query("SELECT ordinal,work_ref FROM linggan_comment_replay_member WHERE sample_set_ref=$1 ORDER BY ordinal")
        .bind(run.get::<Uuid,_>("sample_set_ref")).fetch_all(db.pool()).await?;
    let item_rows = sqlx::query("SELECT DISTINCT ON(member_ordinal,side,repeat_ordinal) member_ordinal,side,repeat_ordinal,attempt_ordinal,state,structure_accepted,no_signal,safe_summary,input_tokens,output_tokens,elapsed_ms FROM linggan_comment_replay_item WHERE run_ref=$1 ORDER BY member_ordinal,side,repeat_ordinal,attempt_ordinal DESC")
        .bind(run_ref).fetch_all(db.pool()).await?;
    let mut items = BTreeMap::new();
    for row in item_rows {
        items.insert(
            (
                row.get::<i16, _>("member_ordinal"),
                row.get::<String, _>("side"),
                row.get::<i16, _>("repeat_ordinal"),
            ),
            row,
        );
    }
    let mut pairs = Vec::new();
    let mut repeats = Vec::new();
    for member in members {
        let ordinal: i16 = member.get("ordinal");
        let baseline = items.get(&(ordinal, "baseline".into(), 0));
        let candidate = items.get(&(ordinal, "candidate".into(), 0));
        let excluded = [baseline, candidate]
            .into_iter()
            .flatten()
            .any(|row| row.get::<String, _>("state") == "excluded");
        pairs.push(ReplayPair {
            work_ref: member.get("work_ref"),
            eligible: !excluded,
            baseline: baseline.and_then(row_observation),
            candidate: candidate.and_then(row_observation),
        });
        if let Some(repeated) = items.get(&(ordinal, "candidate".into(), 1)) {
            repeats.push(ReplayRepeat {
                work_ref: member.get("work_ref"),
                eligible: !excluded && repeated.get::<String, _>("state") != "excluded",
                initial: candidate.and_then(row_observation),
                repeated: row_observation(repeated),
                initial_atom_evidence_hashes: candidate.and_then(row_hashes),
                repeated_atom_evidence_hashes: row_hashes(repeated),
            });
        }
    }
    let baseline: RuleSnapshot =
        serde_json::from_value(run.get("baseline_snapshot")).map_err(|_| ModelError::Invalid)?;
    let candidate: RuleSnapshot =
        serde_json::from_value(run.get("candidate_snapshot")).map_err(|_| ModelError::Invalid)?;
    let comparison = compare_with_schema_mode(
        &pairs,
        &repeats,
        thresholds,
        baseline.rule_version != candidate.rule_version,
    );
    let supplier_failure_count = usize::try_from(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_replay_item WHERE run_ref=$1 AND state='failed'",
        )
        .bind(run_ref)
        .fetch_one(db.pool())
        .await?,
    )
    .map_err(|_| ModelError::Invalid)?;
    finalize_run(
        db,
        run_ref,
        run.get("candidate_rule_revision_ref"),
        comparison,
        run.get::<String, _>("run_kind"),
        supplier_failure_count,
    )
    .await
}

fn row_observation(row: &sqlx::postgres::PgRow) -> Option<ReplayObservation> {
    (row.get::<String, _>("state") == "succeeded").then(|| ReplayObservation {
        structure_accepted: row
            .get::<Option<bool>, _>("structure_accepted")
            .unwrap_or(false),
        no_signal: row.get::<Option<bool>, _>("no_signal").unwrap_or(false),
        input_tokens: row
            .get::<Option<i64>, _>("input_tokens")
            .and_then(|value| u64::try_from(value).ok()),
        output_tokens: row
            .get::<Option<i64>, _>("output_tokens")
            .and_then(|value| u64::try_from(value).ok()),
        elapsed_ms: row
            .get::<Option<i64>, _>("elapsed_ms")
            .and_then(|value| u64::try_from(value).ok()),
        invariant_violations: row.get::<Value, _>("safe_summary")["invariantViolations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
    })
}

fn row_hashes(row: &sqlx::postgres::PgRow) -> Option<BTreeSet<String>> {
    (row.get::<String, _>("state") == "succeeded").then(|| {
        row.get::<Value, _>("safe_summary")["atomEvidenceHashes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

async fn finalize_run(
    db: &Database,
    run_ref: Uuid,
    candidate_rule: Uuid,
    comparison: crate::comment_replay_metrics::ReplayComparison,
    run_kind: String,
    supplier_failure_count: usize,
) -> Result<(), ModelError> {
    let mut comparison_value =
        serde_json::to_value(&comparison).map_err(|_| ModelError::Invalid)?;
    let schema_regression = comparison
        .baseline_schema_failure
        .zip(comparison.candidate_schema_failure)
        .is_some_and(|(baseline, candidate)| {
            candidate > comparison.thresholds.maximum_schema_failure
                || candidate > baseline + comparison.thresholds.maximum_schema_increase
        });
    let no_signal_regression = comparison
        .baseline_no_signal
        .zip(comparison.candidate_no_signal)
        .is_some_and(|(baseline, candidate)| {
            (candidate - baseline).abs() > comparison.thresholds.maximum_no_signal_shift
        });
    let health_regression = run_kind == "active_health"
        && supplier_failure_count == 0
        && comparison.completed_pairs >= comparison.thresholds.minimum_pairs
        && comparison.work_count >= comparison.thresholds.minimum_works
        && comparison
            .pair_coverage
            .is_some_and(|coverage| coverage >= comparison.thresholds.minimum_pair_coverage)
        && comparison.unpaired_eligible_count == 0
        && comparison.state == "degraded"
        && (schema_regression || no_signal_regression);
    comparison_value["sameInputSameModel"] = json!(run_kind == "active_health");
    comparison_value["supplierFailureCount"] = json!(supplier_failure_count);
    comparison_value["promptAttributableHealthRegression"] = json!(health_regression);
    comparison_value["stabilityNotAccuracy"] = json!(true);
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let run = sqlx::query("SELECT state,policy_revision,candidate_snapshot,run_kind FROM linggan_comment_replay_run WHERE run_ref=$1 FOR UPDATE")
        .bind(run_ref).fetch_optional(&mut *tx).await?.ok_or(ModelError::NotFound)?;
    if !matches!(run.get::<String, _>("state").as_str(), "queued" | "running") {
        tx.commit().await?;
        return Ok(());
    }
    let policy = sqlx::query("SELECT revision,enabled,selected_candidate_rule_revision_ref,engineering_gate_version FROM linggan_comment_auto_upgrade_policy WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx).await?;
    let active = sqlx::query("SELECT rule_revision_ref,revision FROM linggan_comment_research_rule_active WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx).await?;
    let candidate_snapshot: RuleSnapshot =
        serde_json::from_value(run.get("candidate_snapshot")).map_err(|_| ModelError::Invalid)?;
    let active_rule: Uuid = active.get("rule_revision_ref");
    let policy_allows = run.get::<String, _>("run_kind") == "candidate"
        && comparison.state == "pass"
        && policy.get::<bool, _>("enabled")
        && policy.get::<Option<Uuid>, _>("selected_candidate_rule_revision_ref")
            == Some(candidate_rule)
        && policy.get::<String, _>("engineering_gate_version") == comparison.thresholds.version
        && policy.get::<i64, _>("revision") == run.get::<i64, _>("policy_revision")
        && candidate_snapshot.parent_rule_revision_ref == Some(active_rule);
    let receipt = if policy_allows {
        let active_revision: i32 = active.get("revision");
        activate_rule_after_policy_in(
            &mut tx,
            &ActivateRuleRevision {
                expected_revision: active_revision,
                rule_revision_ref: candidate_rule,
            },
        )
        .await?;
        Some(
            json!({"kind":"automatic_rule_adoption","policyRevision":policy.get::<i64,_>("revision"),"activeRuleRevisionBefore":active_revision,"activeRuleRevisionAfter":active_revision+1,"candidateRuleRevisionRef":candidate_rule,"comparisonState":"pass"}),
        )
    } else {
        None
    };
    sqlx::query("UPDATE linggan_comment_replay_run SET state=$2,comparison=$3,adoption_receipt=$4,failure_code=NULL,finished_at=scope_001_now() WHERE run_ref=$1")
        .bind(run_ref).bind(comparison.state).bind(comparison_value).bind(receipt).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn production_new_intake_waiting(db: &Database) -> Result<bool, ModelError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) WHERE b.enabled AND i.queue_class='new_intake' AND i.state IN ('pending','running'))")
        .fetch_one(db.pool()).await.map_err(ModelError::from)
}

fn drain_requested(drain: Option<&ModelWorkerDrain>) -> bool {
    drain.is_some_and(ModelWorkerDrain::is_requested)
}
