//! Persisted P3 shadow-replay commands. Creation only freezes bounded reference/hash manifests;
//! it does not call a provider. The runner lives in `comment_replay_runner` and is the sole
//! dispatch path, where it rechecks these source/context hashes before every invocation.
use crate::{
    comment_packet::{CONTEXT_SELECTOR_VERSION, build_packet_with_rule},
    comment_replay_metrics::ReplayThresholds,
    comment_research::comment_source_hash,
    comment_research_rules::{RuleCreationSource, RuleSnapshot, RuleVersion, read_rule},
    comment_runtime::ContextPolicy,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

#[path = "comment_replay_runner.rs"]
mod runner;
pub use runner::run_next_replay;

pub const SAMPLE_SELECTOR_VERSION: &str = "comment-replay.sample-selector.v1";
pub const MAX_SAMPLE_MEMBERS: usize = 120;
pub const MAX_REPEAT_MEMBERS: usize = 20;
const DEFAULT_TTL_SECONDS: i64 = 86_400;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCandidateReplay {
    pub run_ref: Uuid,
    pub sample_set_ref: Uuid,
    pub baseline_rule_revision_ref: Uuid,
    pub candidate_rule_revision_ref: Uuid,
    pub config_ref: Uuid,
    pub source_refs: Vec<Uuid>,
    pub seed: i64,
    #[serde(default = "default_ttl_seconds")]
    pub ttl_seconds: i64,
    pub context_policy: ContextPolicy,
    #[serde(default)]
    pub thresholds: ReplayThresholds,
    #[serde(default)]
    pub repeat_member_limit: u8,
    pub authorized_token_budget: i64,
}

const fn default_ttl_seconds() -> i64 {
    DEFAULT_TTL_SECONDS
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigureAutoUpgradePolicy {
    pub expected_revision: i64,
    pub enabled: bool,
    pub selected_candidate_rule_revision_ref: Option<Uuid>,
}

#[derive(Clone)]
pub(crate) struct FrozenReplayMember {
    pub ordinal: i16,
    pub source_ref: Uuid,
    pub work_ref: Uuid,
    pub source_hash: String,
    pub context_refs: Vec<Uuid>,
    pub context_hash: String,
    pub strata: Value,
}

struct PreparedReplay {
    baseline: RuleSnapshot,
    candidate: RuleSnapshot,
    model: ReplayModelSnapshot,
    members: Vec<FrozenReplayMember>,
    selector_hash: String,
    member_hash: String,
    context_policy_hash: String,
    request_hash: String,
    initial_state: &'static str,
}

struct ReplayItemContext<'a> {
    command: &'a CreateCandidateReplay,
    context_policy_hash: &'a str,
    baseline: &'a RuleSnapshot,
    candidate: &'a RuleSnapshot,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReplayModelSnapshot {
    pub config_ref: Uuid,
    pub model_ref: Uuid,
    pub model_id: String,
    pub connection_version_ref: Uuid,
    pub input_token_limit: i32,
    pub output_token_limit: i32,
    pub timeout_seconds: i32,
    #[serde(rename = "connectionEnabledAtFreeze")]
    pub connection_enabled: bool,
}

impl ReplayModelSnapshot {
    pub(crate) fn json(&self) -> Value {
        json!({
            "configRef": self.config_ref,
            "modelRef": self.model_ref,
            "modelId": self.model_id,
            "connectionVersionRef": self.connection_version_ref,
            "inputTokenLimit": self.input_token_limit,
            "outputTokenLimit": self.output_token_limit,
            "timeoutSeconds": self.timeout_seconds,
            "connectionEnabledAtFreeze": self.connection_enabled,
        })
    }
}

pub async fn schema_ready(db: &Database) -> Result<bool, ModelError> {
    sqlx::query_scalar("SELECT to_regclass('linggan_comment_replay_sample_set') IS NOT NULL AND to_regclass('linggan_comment_replay_run') IS NOT NULL AND to_regclass('linggan_comment_replay_item') IS NOT NULL AND to_regclass('linggan_comment_replay_trace') IS NOT NULL")
        .fetch_one(db.pool())
        .await
        .map_err(ModelError::from)
}

pub async fn create_candidate_replay(
    db: &Database,
    command: &CreateCandidateReplay,
) -> Result<Value, ModelError> {
    let prepared = prepare_replay(db, command).await?;
    create_prepared_replay(db, command, prepared, None, None, None, "candidate", true).await
}

/// Creates a continuation in the same transaction that binds it to the first authorization
/// root.  Callers must not create an ordinary run and attach its root afterwards: that would
/// temporarily grant a fresh, independent budget to a successor.
pub(crate) async fn create_replay_successor(
    db: &Database,
    command: &CreateCandidateReplay,
    authorization_root_run_ref: Uuid,
    prior_run_ref: Option<Uuid>,
    follow_up_ref: Option<Uuid>,
    run_kind: &str,
    require_active_baseline: bool,
) -> Result<Value, ModelError> {
    let prepared = prepare_replay(db, command).await?;
    create_prepared_replay(
        db,
        command,
        prepared,
        Some(authorization_root_run_ref),
        prior_run_ref,
        follow_up_ref,
        run_kind,
        require_active_baseline,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn create_prepared_replay(
    db: &Database,
    command: &CreateCandidateReplay,
    prepared: PreparedReplay,
    authorization_root_run_ref: Option<Uuid>,
    prior_run_ref: Option<Uuid>,
    follow_up_ref: Option<Uuid>,
    run_kind: &str,
    require_active_baseline: bool,
) -> Result<Value, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(existing) =
        sqlx::query("SELECT run_ref FROM linggan_comment_replay_run WHERE request_hash=$1")
            .bind(&prepared.request_hash)
            .fetch_optional(&mut *tx)
            .await?
    {
        let run_ref: Uuid = existing.get("run_ref");
        tx.commit().await?;
        return read_replay(db, run_ref).await;
    }
    let active_rule: Uuid = sqlx::query_scalar(
        "SELECT rule_revision_ref FROM linggan_comment_research_rule_active WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;
    if require_active_baseline && active_rule != prepared.baseline.rule_revision_ref {
        return Err(ModelError::Conflict);
    }
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_replay_run WHERE run_ref=$1)",
    )
    .bind(command.run_ref)
    .fetch_one(&mut *tx)
    .await?
    {
        return Err(ModelError::Conflict);
    }
    let authorized_token_budget = if let Some(root) = authorization_root_run_ref {
        let root_row = sqlx::query("SELECT authorized_token_budget,COALESCE(authorization_root_run_ref,run_ref) AS effective_root FROM linggan_comment_replay_run WHERE run_ref=$1 FOR UPDATE")
            .bind(root).fetch_optional(&mut *tx).await?.ok_or(ModelError::NotFound)?;
        if root_row.get::<Uuid, _>("effective_root") != root {
            return Err(ModelError::Conflict);
        }
        let root_budget: i64 = root_row.get("authorized_token_budget");
        let used = authorization_usage_in(&mut tx, root).await?;
        let remaining = root_budget.saturating_sub(used);
        if remaining <= 0 {
            return Err(ModelError::Conflict);
        }
        remaining
    } else {
        command.authorized_token_budget
    };
    persist_sample_set(
        &mut tx,
        command,
        &prepared.members,
        &prepared.selector_hash,
        &prepared.member_hash,
    )
    .await?;
    let policy = sqlx::query(
        "SELECT revision FROM linggan_comment_auto_upgrade_policy WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;
    let policy_revision: i64 = policy.get("revision");
    sqlx::query("INSERT INTO linggan_comment_replay_run(run_ref,request_hash,sample_set_ref,baseline_rule_revision_ref,candidate_rule_revision_ref,baseline_snapshot,candidate_snapshot,config_ref,model_snapshot,context_policy,context_policy_hash,thresholds,seed,repeat_member_limit,authorized_token_budget,policy_revision,state,authorization_root_run_ref,run_kind) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)")
        .bind(command.run_ref).bind(&prepared.request_hash).bind(command.sample_set_ref)
        .bind(prepared.baseline.rule_revision_ref).bind(prepared.candidate.rule_revision_ref)
        .bind(json!(&prepared.baseline)).bind(json!(&prepared.candidate)).bind(command.config_ref).bind(prepared.model.json())
        .bind(json!(command.context_policy)).bind(&prepared.context_policy_hash).bind(json!(command.thresholds))
        .bind(command.seed).bind(i16::from(command.repeat_member_limit)).bind(authorized_token_budget)
        .bind(policy_revision).bind(prepared.initial_state).bind(authorization_root_run_ref).bind(run_kind).execute(&mut *tx).await?;
    persist_initial_items(
        &mut tx,
        command,
        &prepared.members,
        &prepared.context_policy_hash,
        &prepared.baseline,
        &prepared.candidate,
    )
    .await?;
    if let Some(prior) = prior_run_ref {
        reuse_completed_primary_pairs(&mut tx, command.run_ref, prior, &prepared.members).await?;
    }
    if let Some(follow_up) = follow_up_ref {
        let changed = sqlx::query("UPDATE linggan_comment_replay_follow_up SET state='created',successor_run_ref=$2,updated_at=scope_001_now(),failure_code=NULL WHERE follow_up_ref=$1 AND state='queued'")
            .bind(follow_up).bind(command.run_ref).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(ModelError::Conflict);
        }
    }
    tx.commit().await?;
    read_replay(db, command.run_ref).await
}

async fn prepare_replay(
    db: &Database,
    command: &CreateCandidateReplay,
) -> Result<PreparedReplay, ModelError> {
    validate_create(command)?;
    let baseline = read_rule(db, command.baseline_rule_revision_ref).await?;
    let candidate = read_rule(db, command.candidate_rule_revision_ref).await?;
    validate_rule_pair(&baseline, &candidate)?;
    let model = read_model_snapshot(db, command.config_ref).await?;
    let members = freeze_members(db, command, &baseline, &candidate, &model.model_id).await?;
    let selector_hash = selector_hash(command);
    let member_hash = member_hash(&members);
    let context_policy_hash = comment_source_hash(
        &serde_json::to_string(&command.context_policy.semantic_value())
            .map_err(|_| ModelError::Invalid)?,
    );
    let request_hash = command_hash(
        command,
        &selector_hash,
        &member_hash,
        &context_policy_hash,
        &baseline,
        &candidate,
    );
    // An undersized frozen sample still runs its bounded paired receipts. Only after both sides
    // settle can it truthfully conclude `insufficient_evidence`, and those receipts are then
    // eligible for a later old-plus-new continuation without paying for them again.
    let initial_state = "queued";
    Ok(PreparedReplay {
        baseline,
        candidate,
        model,
        members,
        selector_hash,
        member_hash,
        context_policy_hash,
        request_hash,
        initial_state,
    })
}

pub async fn configure_auto_upgrade_policy(
    db: &Database,
    command: &ConfigureAutoUpgradePolicy,
) -> Result<Value, ModelError> {
    let candidate = if command.enabled {
        let candidate_ref = command
            .selected_candidate_rule_revision_ref
            .ok_or(ModelError::Invalid)?;
        let snapshot = read_rule(db, candidate_ref).await?;
        if snapshot.rule_version != RuleVersion::V5
            || snapshot.creation_source != RuleCreationSource::UserCandidate
        {
            return Err(ModelError::Invalid);
        }
        Some(snapshot)
    } else {
        None
    };
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(candidate) = candidate {
        let active_rule: Uuid = sqlx::query_scalar(
            "SELECT rule_revision_ref FROM linggan_comment_research_rule_active WHERE singleton FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        if candidate.parent_rule_revision_ref != Some(active_rule) {
            return Err(ModelError::Conflict);
        }
    }
    let revision: Option<i64> = sqlx::query_scalar("UPDATE linggan_comment_auto_upgrade_policy SET revision=revision+1,enabled=$2,selected_candidate_rule_revision_ref=$3,updated_at=scope_001_now() WHERE singleton AND revision=$1 RETURNING revision")
        .bind(command.expected_revision).bind(command.enabled).bind(command.selected_candidate_rule_revision_ref)
        .fetch_optional(&mut *tx).await?;
    tx.commit().await?;
    Ok(json!({
        "revision": revision.ok_or(ModelError::Conflict)?,
        "enabled": command.enabled,
        "selectedCandidateRuleRevisionRef": command.selected_candidate_rule_revision_ref,
        "state": "SAVED_NO_PROVIDER_CALL",
    }))
}

pub async fn read_replays(db: &Database) -> Result<Value, ModelError> {
    let rows = sqlx::query("SELECT r.run_ref,r.sample_set_ref,r.baseline_rule_revision_ref,r.candidate_rule_revision_ref,r.state,r.failure_code,r.authorized_token_budget,r.reserved_tokens,r.execution_day,r.policy_revision,r.comparison,r.adoption_receipt,r.authorization_root_run_ref,r.run_kind,r.created_at::text AS created_at,r.started_at::text AS started_at,r.finished_at::text AS finished_at,(SELECT count(*) FROM linggan_comment_replay_member m WHERE m.sample_set_ref=r.sample_set_ref) AS frozen_count,(SELECT count(*) FROM linggan_comment_replay_item i WHERE i.run_ref=r.run_ref AND i.state='succeeded') AS succeeded_count,(SELECT count(*) FROM linggan_comment_replay_item i WHERE i.run_ref=r.run_ref AND i.state='excluded') AS excluded_count FROM linggan_comment_replay_run r ORDER BY r.created_at DESC,r.run_ref DESC LIMIT 100")
        .fetch_all(db.pool()).await?;
    Ok(json!({"items": rows.into_iter().map(run_summary).collect::<Vec<_>>(), "limit": 100}))
}

pub async fn read_replay(db: &Database, run_ref: Uuid) -> Result<Value, ModelError> {
    let run = sqlx::query("SELECT r.*,r.created_at::text AS created_text,r.started_at::text AS started_text,r.finished_at::text AS finished_text,s.selector_version,s.selector_hash,s.member_hash,s.frozen_count,s.expires_at::text AS expires_text,s.state AS sample_state FROM linggan_comment_replay_run r JOIN linggan_comment_replay_sample_set s USING(sample_set_ref) WHERE r.run_ref=$1")
        .bind(run_ref).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    let members = sqlx::query("SELECT ordinal,source_ref,work_ref,source_hash,context_refs,context_hash,strata FROM linggan_comment_replay_member WHERE sample_set_ref=$1 ORDER BY ordinal")
        .bind(run.get::<Uuid,_>("sample_set_ref")).fetch_all(db.pool()).await?;
    let items = sqlx::query("SELECT item_ref,member_ordinal,side,repeat_ordinal,attempt_ordinal,invocation_ref,reused_from_item_ref,execution_day,state,structure_accepted,no_signal,safe_summary,input_tokens,output_tokens,elapsed_ms,failure_code,exclusion_reason,created_at::text AS created_at,started_at::text AS started_at,finished_at::text AS finished_at FROM linggan_comment_replay_item WHERE run_ref=$1 ORDER BY member_ordinal,side,repeat_ordinal,attempt_ordinal")
        .bind(run_ref).fetch_all(db.pool()).await?;
    Ok(json!({
        "run": run_detail(&run),
        "sampleSet": {
            "sampleSetRef":run.get::<Uuid,_>("sample_set_ref"),
            "selectorVersion":run.get::<String,_>("selector_version"),
            "selectorHash":run.get::<String,_>("selector_hash"),
            "memberHash":run.get::<String,_>("member_hash"),
            "frozenCount":run.get::<i32,_>("frozen_count"),
            "expiresAt":run.get::<String,_>("expires_text"),
            "state":run.get::<String,_>("sample_state"),
            "members":members.into_iter().map(|member|json!({"ordinal":member.get::<i16,_>("ordinal"),"sourceRef":member.get::<Uuid,_>("source_ref"),"workRef":member.get::<Uuid,_>("work_ref"),"sourceHash":member.get::<String,_>("source_hash"),"contextRefs":member.get::<Vec<Uuid>,_>("context_refs"),"contextHash":member.get::<String,_>("context_hash"),"strata":member.get::<Value,_>("strata")})).collect::<Vec<_>>(),
        },
        "items":items.into_iter().map(item_detail).collect::<Vec<_>>(),
    }))
}

pub(crate) async fn read_model_snapshot(
    db: &Database,
    config_ref: Uuid,
) -> Result<ReplayModelSnapshot, ModelError> {
    let row = sqlx::query("SELECT cfg.config_ref,cfg.input_token_limit,cfg.output_token_limit,cfg.timeout_seconds,m.model_ref,m.model_id,m.connection_version_ref,conn.enabled AS connection_enabled FROM linggan_model_config cfg JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version version ON version.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE cfg.config_ref=$1")
        .bind(config_ref).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    Ok(ReplayModelSnapshot {
        config_ref: row.get("config_ref"),
        model_ref: row.get("model_ref"),
        model_id: row.get("model_id"),
        connection_version_ref: row.get("connection_version_ref"),
        input_token_limit: row.get("input_token_limit"),
        output_token_limit: row.get("output_token_limit"),
        timeout_seconds: row.get("timeout_seconds"),
        connection_enabled: row.get("connection_enabled"),
    })
}

pub(crate) async fn load_member(
    db: &Database,
    sample_set_ref: Uuid,
    ordinal: i16,
) -> Result<FrozenReplayMember, ModelError> {
    let row = sqlx::query("SELECT ordinal,source_ref,work_ref,source_hash,context_refs,context_hash,strata FROM linggan_comment_replay_member WHERE sample_set_ref=$1 AND ordinal=$2")
        .bind(sample_set_ref).bind(ordinal).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    Ok(FrozenReplayMember {
        ordinal: row.get("ordinal"),
        source_ref: row.get("source_ref"),
        work_ref: row.get("work_ref"),
        source_hash: row.get("source_hash"),
        context_refs: row.get("context_refs"),
        context_hash: row.get("context_hash"),
        strata: row.get("strata"),
    })
}

fn validate_create(command: &CreateCandidateReplay) -> Result<(), ModelError> {
    let refs = command.source_refs.iter().collect::<BTreeSet<_>>();
    if command.source_refs.is_empty()
        || command.source_refs.len() > MAX_SAMPLE_MEMBERS
        || refs.len() != command.source_refs.len()
        || !(1..=604_800).contains(&command.ttl_seconds)
        || usize::from(command.repeat_member_limit) > MAX_REPEAT_MEMBERS
        || command.authorized_token_budget < 0
        || !command.thresholds.valid()
    {
        return Err(ModelError::Invalid);
    }
    ContextPolicy::parse(json!(command.context_policy))?;
    Ok(())
}

fn validate_rule_pair(baseline: &RuleSnapshot, candidate: &RuleSnapshot) -> Result<(), ModelError> {
    if !matches!(baseline.rule_version, RuleVersion::V4 | RuleVersion::V5)
        || candidate.rule_version != RuleVersion::V5
        || candidate.creation_source != RuleCreationSource::UserCandidate
        || candidate.parent_rule_revision_ref != Some(baseline.rule_revision_ref)
    {
        return Err(ModelError::Invalid);
    }
    Ok(())
}

async fn freeze_members(
    db: &Database,
    command: &CreateCandidateReplay,
    baseline: &RuleSnapshot,
    candidate: &RuleSnapshot,
    model_id: &str,
) -> Result<Vec<FrozenReplayMember>, ModelError> {
    let mut refs = command.source_refs.clone();
    refs.sort();
    let mut members = Vec::with_capacity(refs.len());
    for (index, source_ref) in refs.into_iter().enumerate() {
        let packet = build_packet_with_rule(
            db,
            &[source_ref],
            model_id,
            &command.context_policy,
            baseline,
        )
        .await?;
        let candidate_packet = build_packet_with_rule(
            db,
            &[source_ref],
            model_id,
            &command.context_policy,
            candidate,
        )
        .await?;
        let input = packet.inputs.first().ok_or(ModelError::Source)?;
        let candidate_input = candidate_packet.inputs.first().ok_or(ModelError::Source)?;
        // Prompt/schema versions can differ. The frozen source identity and selected context
        // must not: a replay is paired only when both builders see the same material snapshot.
        if input.source_ref != candidate_input.source_ref
            || input.source_sha256 != candidate_input.source_sha256
            || packet.context_refs != candidate_packet.context_refs
            || packet.context_hash != candidate_packet.context_hash
        {
            return Err(ModelError::Invalid);
        }
        let work_ref: Uuid = sqlx::query_scalar(
            "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
        )
        .bind(source_ref)
        .fetch_optional(db.pool())
        .await?
        .ok_or(ModelError::Source)?;
        let prior: Option<Value> = sqlx::query_scalar("SELECT result FROM linggan_comment_analysis_work WHERE source_ref=$1 AND state IN('succeeded','no_signal') ORDER BY updated_at DESC,work_ref DESC LIMIT 1")
            .bind(source_ref).fetch_optional(db.pool()).await?.flatten();
        let labels = prior
            .as_ref()
            .and_then(|value| value.pointer("/semantic/labels"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let problems = prior
            .as_ref()
            .and_then(|value| value.pointer("/semantic/problems"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let stances = prior
            .as_ref()
            .and_then(|value| value.pointer("/semantic/stances"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let atom_count = prior
            .as_ref()
            .and_then(|value| value.pointer("/semantic/atoms"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let length = packet
            .cleaned
            .first()
            .map_or(0, |clean| clean.text.chars().count());
        members.push(FrozenReplayMember {
            ordinal: i16::try_from(index + 1).map_err(|_| ModelError::Invalid)?,
            source_ref,
            work_ref,
            source_hash: input.source_sha256.clone(),
            context_refs: packet.context_refs.clone(),
            context_hash: packet.context_hash.clone(),
            strata: json!({
                "lengthClass":if length<80 {"short"} else {"long"},
                "hasParentReply":input.context.pointer("/parent/sourceRef").is_some(),
                "contextFragmentCount":input.context["researchFragments"].as_array().map_or(0,Vec::len),
                "historicalOutcome":prior.as_ref().and_then(|value|value.pointer("/semantic/outcome")).and_then(Value::as_str),
                "historicalFieldRejected":prior.as_ref().is_some_and(|value|value.pointer("/semantic/acceptance").and_then(Value::as_str)==Some("partial")),
                "historicalMultiIntent":labels+problems+stances+atom_count>1,
            }),
        });
    }
    Ok(members)
}

fn selector_hash(command: &CreateCandidateReplay) -> String {
    comment_source_hash(&json!({
        "selectorVersion": SAMPLE_SELECTOR_VERSION,
        "contextSelectorVersion": CONTEXT_SELECTOR_VERSION,
        "contextPolicy": command.context_policy.semantic_value(),
        "seed": command.seed,
        "strata":["lengthClass","hasParentReply","contextFragmentCount","historicalOutcome","historicalFieldRejected","historicalMultiIntent","workRef"],
    }).to_string())
}

fn member_hash(members: &[FrozenReplayMember]) -> String {
    comment_source_hash(&json!(members.iter().map(|member|json!({
        "ordinal":member.ordinal,"sourceRef":member.source_ref,"workRef":member.work_ref,
        "sourceHash":member.source_hash,"contextRefs":member.context_refs,"contextHash":member.context_hash,
        "strata":member.strata,
    })).collect::<Vec<_>>()).to_string())
}

fn command_hash(
    command: &CreateCandidateReplay,
    selector_hash: &str,
    member_hash: &str,
    context_policy_hash: &str,
    baseline: &RuleSnapshot,
    candidate: &RuleSnapshot,
) -> String {
    comment_source_hash(&json!({
        "sampleSetRef":command.sample_set_ref,"selectorHash":selector_hash,"memberHash":member_hash,
        "baselineRuleRevisionRef":baseline.rule_revision_ref,"baselineRuleHash":baseline.canonical_hash,
        "candidateRuleRevisionRef":candidate.rule_revision_ref,"candidateRuleHash":candidate.canonical_hash,
        "configRef":command.config_ref,"contextPolicyHash":context_policy_hash,"thresholds":command.thresholds,
        "seed":command.seed,"repeatMemberLimit":command.repeat_member_limit,"authorizedTokenBudget":command.authorized_token_budget,
    }).to_string())
}

async fn persist_sample_set(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &CreateCandidateReplay,
    members: &[FrozenReplayMember],
    selector_hash: &str,
    member_hash: &str,
) -> Result<(), ModelError> {
    if let Some(row) = sqlx::query("SELECT selector_version,selector_hash,member_hash,seed,frozen_count FROM linggan_comment_replay_sample_set WHERE sample_set_ref=$1 FOR UPDATE")
        .bind(command.sample_set_ref).fetch_optional(&mut **tx).await? {
        if row.get::<String,_>("selector_version") != SAMPLE_SELECTOR_VERSION
            || row.get::<String,_>("selector_hash") != selector_hash
            || row.get::<String,_>("member_hash") != member_hash
            || row.get::<i64,_>("seed") != command.seed
            || row.get::<i32,_>("frozen_count") != members.len() as i32
        {
            return Err(ModelError::Conflict);
        }
        return Ok(());
    }
    sqlx::query("INSERT INTO linggan_comment_replay_sample_set(sample_set_ref,selector_version,selector_hash,member_hash,seed,frozen_count,expires_at) VALUES($1,$2,$3,$4,$5,$6,scope_001_now()+make_interval(secs=>$7))")
        .bind(command.sample_set_ref).bind(SAMPLE_SELECTOR_VERSION).bind(selector_hash).bind(member_hash)
        .bind(command.seed).bind(members.len() as i32).bind(command.ttl_seconds).execute(&mut **tx).await?;
    for member in members {
        sqlx::query("INSERT INTO linggan_comment_replay_member(sample_set_ref,ordinal,source_ref,work_ref,source_hash,context_refs,context_hash,strata) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(command.sample_set_ref).bind(member.ordinal).bind(member.source_ref).bind(member.work_ref)
            .bind(&member.source_hash).bind(&member.context_refs).bind(&member.context_hash).bind(&member.strata)
            .execute(&mut **tx).await?;
    }
    Ok(())
}

async fn persist_initial_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &CreateCandidateReplay,
    members: &[FrozenReplayMember],
    context_policy_hash: &str,
    baseline: &RuleSnapshot,
    candidate: &RuleSnapshot,
) -> Result<(), ModelError> {
    let context = ReplayItemContext {
        command,
        context_policy_hash,
        baseline,
        candidate,
    };
    for member in members {
        for side in ["baseline", "candidate"] {
            insert_item(tx, &context, member, side, 0).await?;
        }
        if member.ordinal > 0 && member.ordinal <= i16::from(command.repeat_member_limit) {
            insert_item(tx, &context, member, "candidate", 1).await?;
        }
    }
    Ok(())
}

/// Copy only completed primary shadow receipts whose frozen input identity is still identical.
/// This makes a continuation's old members inspectable without sending either side to a model
/// again.  The original item remains immutable; the successor points back to it.
async fn reuse_completed_primary_pairs(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    successor_run_ref: Uuid,
    prior_run_ref: Uuid,
    successor_members: &[FrozenReplayMember],
) -> Result<(), ModelError> {
    let prior = sqlx::query(
        "WITH baseline AS (  \
             SELECT DISTINCT ON (member_ordinal) item_ref,member_ordinal,structure_accepted,no_signal,safe_summary,input_tokens,output_tokens,elapsed_ms \
             FROM linggan_comment_replay_item \
             WHERE run_ref=$1 AND side='baseline' AND repeat_ordinal=0 AND state='succeeded' \
             ORDER BY member_ordinal,attempt_ordinal DESC \
         ), candidate AS ( \
             SELECT DISTINCT ON (member_ordinal) item_ref,member_ordinal,structure_accepted,no_signal,safe_summary,input_tokens,output_tokens,elapsed_ms \
             FROM linggan_comment_replay_item \
             WHERE run_ref=$1 AND side='candidate' AND repeat_ordinal=0 AND state='succeeded' \
             ORDER BY member_ordinal,attempt_ordinal DESC \
         ) \
         SELECT member.source_ref,member.source_hash,member.context_refs,member.context_hash, \
                baseline.item_ref AS baseline_item_ref,baseline.structure_accepted AS baseline_structure_accepted,baseline.no_signal AS baseline_no_signal,baseline.safe_summary AS baseline_safe_summary,baseline.input_tokens AS baseline_input_tokens,baseline.output_tokens AS baseline_output_tokens,baseline.elapsed_ms AS baseline_elapsed_ms, \
                candidate.item_ref AS candidate_item_ref,candidate.structure_accepted AS candidate_structure_accepted,candidate.no_signal AS candidate_no_signal,candidate.safe_summary AS candidate_safe_summary,candidate.input_tokens AS candidate_input_tokens,candidate.output_tokens AS candidate_output_tokens,candidate.elapsed_ms AS candidate_elapsed_ms \
         FROM linggan_comment_replay_run run \
         JOIN linggan_comment_replay_member member USING(sample_set_ref) \
         JOIN baseline ON baseline.member_ordinal=member.ordinal \
         JOIN candidate ON candidate.member_ordinal=member.ordinal \
         WHERE run.run_ref=$1",
    )
    .bind(prior_run_ref)
    .fetch_all(&mut **tx)
    .await?;
    for row in prior {
        let Some(member) = successor_members.iter().find(|member| {
            member.source_ref == row.get::<Uuid, _>("source_ref")
                && member.source_hash == row.get::<String, _>("source_hash")
                && member.context_refs == row.get::<Vec<Uuid>, _>("context_refs")
                && member.context_hash == row.get::<String, _>("context_hash")
        }) else {
            continue;
        };
        for side in ["baseline", "candidate"] {
            let prefix = side;
            let (
                prior_item_ref,
                structure_accepted,
                no_signal,
                safe_summary,
                input_tokens,
                output_tokens,
                elapsed_ms,
            ): (
                Uuid,
                Option<bool>,
                Option<bool>,
                Value,
                Option<i64>,
                Option<i64>,
                Option<i64>,
            ) = match prefix {
                "baseline" => (
                    row.get("baseline_item_ref"),
                    row.get("baseline_structure_accepted"),
                    row.get("baseline_no_signal"),
                    row.get("baseline_safe_summary"),
                    row.get("baseline_input_tokens"),
                    row.get("baseline_output_tokens"),
                    row.get("baseline_elapsed_ms"),
                ),
                _ => (
                    row.get("candidate_item_ref"),
                    row.get("candidate_structure_accepted"),
                    row.get("candidate_no_signal"),
                    row.get("candidate_safe_summary"),
                    row.get("candidate_input_tokens"),
                    row.get("candidate_output_tokens"),
                    row.get("candidate_elapsed_ms"),
                ),
            };
            sqlx::query("UPDATE linggan_comment_replay_item SET state='succeeded',structure_accepted=$4,no_signal=$5,safe_summary=$6,input_tokens=$7,output_tokens=$8,elapsed_ms=$9,reused_from_item_ref=$10,finished_at=scope_001_now() WHERE run_ref=$1 AND member_ordinal=$2 AND side=$3 AND repeat_ordinal=0 AND state='queued'")
                .bind(successor_run_ref).bind(member.ordinal).bind(side).bind(structure_accepted).bind(no_signal)
                .bind(safe_summary).bind(input_tokens).bind(output_tokens).bind(elapsed_ms).bind(prior_item_ref)
                .execute(&mut **tx).await?;
        }
    }
    Ok(())
}

/// The workspace row is locked by all replay dispatch/creation paths before this query.  Usage
/// takes the larger of reserved capacity and recorded charge for every run in the root family,
/// so an already-recorded supplier overage can never free capacity for another call.
pub(crate) async fn authorization_usage_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    authorization_root_run_ref: Uuid,
) -> Result<i64, ModelError> {
    sqlx::query_scalar(
        "SELECT COALESCE(sum(GREATEST(r.reserved_tokens,COALESCE(actual.charged_tokens,0))),0)::bigint  \
         FROM linggan_comment_replay_run r  \
         LEFT JOIN LATERAL (  \
           SELECT COALESCE(sum(invocation.charged_tokens),0)::bigint AS charged_tokens  \
           FROM linggan_model_invocation invocation  \
           JOIN (  \
             SELECT item.invocation_ref FROM linggan_comment_replay_item item WHERE item.run_ref=r.run_ref  \
             UNION  \
             SELECT explanation.invocation_ref FROM linggan_comment_replay_explanation explanation WHERE explanation.run_ref=r.run_ref  \
           ) receipts USING(invocation_ref)  \
         ) actual ON true  \
         WHERE COALESCE(r.authorization_root_run_ref,r.run_ref)=$1",
    )
    .bind(authorization_root_run_ref)
    .fetch_one(&mut **tx)
    .await
    .map_err(ModelError::from)
}

async fn insert_item(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: &ReplayItemContext<'_>,
    member: &FrozenReplayMember,
    side: &str,
    repeat_ordinal: i16,
) -> Result<(), ModelError> {
    let idempotency_hash = comment_source_hash(&json!({
        "sampleSetRef":context.command.sample_set_ref,"baseline":context.baseline.canonical_hash,"candidate":context.candidate.canonical_hash,
        "configRef":context.command.config_ref,"contextPolicyHash":context.context_policy_hash,"memberOrdinal":member.ordinal,
        "side":side,"repeatOrdinal":repeat_ordinal,
    }).to_string());
    sqlx::query("INSERT INTO linggan_comment_replay_item(item_ref,run_ref,member_ordinal,side,repeat_ordinal,idempotency_hash) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::new_v4()).bind(context.command.run_ref).bind(member.ordinal).bind(side)
        .bind(repeat_ordinal).bind(idempotency_hash).execute(&mut **tx).await?;
    Ok(())
}

fn run_summary(row: sqlx::postgres::PgRow) -> Value {
    json!({"runRef":row.get::<Uuid,_>("run_ref"),"sampleSetRef":row.get::<Uuid,_>("sample_set_ref"),"baselineRuleRevisionRef":row.get::<Uuid,_>("baseline_rule_revision_ref"),"candidateRuleRevisionRef":row.get::<Uuid,_>("candidate_rule_revision_ref"),"state":row.get::<String,_>("state"),"failureCode":row.get::<Option<String>,_>("failure_code"),"authorizedTokenBudget":row.get::<i64,_>("authorized_token_budget"),"reservedTokens":row.get::<i64,_>("reserved_tokens"),"executionDay":row.get::<Option<String>,_>("execution_day"),"policyRevision":row.get::<i64,_>("policy_revision"),"comparison":row.get::<Option<Value>,_>("comparison"),"adoptionReceipt":row.get::<Option<Value>,_>("adoption_receipt"),"authorizationRootRunRef":row.get::<Option<Uuid>,_>("authorization_root_run_ref"),"runKind":row.get::<String,_>("run_kind"),"frozenCount":row.get::<i64,_>("frozen_count"),"succeededCount":row.get::<i64,_>("succeeded_count"),"excludedCount":row.get::<i64,_>("excluded_count"),"createdAt":row.get::<String,_>("created_at"),"startedAt":row.get::<Option<String>,_>("started_at"),"finishedAt":row.get::<Option<String>,_>("finished_at")})
}

fn run_detail(row: &sqlx::postgres::PgRow) -> Value {
    json!({"runRef":row.get::<Uuid,_>("run_ref"),"requestHash":row.get::<String,_>("request_hash"),"state":row.get::<String,_>("state"),"failureCode":row.get::<Option<String>,_>("failure_code"),"baselineRuleRevisionRef":row.get::<Uuid,_>("baseline_rule_revision_ref"),"candidateRuleRevisionRef":row.get::<Uuid,_>("candidate_rule_revision_ref"),"baselineSnapshot":row.get::<Value,_>("baseline_snapshot"),"candidateSnapshot":row.get::<Value,_>("candidate_snapshot"),"configRef":row.get::<Uuid,_>("config_ref"),"modelSnapshot":row.get::<Value,_>("model_snapshot"),"contextPolicy":row.get::<Value,_>("context_policy"),"contextPolicyHash":row.get::<String,_>("context_policy_hash"),"thresholds":row.get::<Value,_>("thresholds"),"seed":row.get::<i64,_>("seed"),"repeatMemberLimit":row.get::<i16,_>("repeat_member_limit"),"authorizedTokenBudget":row.get::<i64,_>("authorized_token_budget"),"reservedTokens":row.get::<i64,_>("reserved_tokens"),"executionDay":row.get::<Option<String>,_>("execution_day"),"policyRevision":row.get::<i64,_>("policy_revision"),"comparison":row.get::<Option<Value>,_>("comparison"),"adoptionReceipt":row.get::<Option<Value>,_>("adoption_receipt"),"authorizationRootRunRef":row.get::<Option<Uuid>,_>("authorization_root_run_ref"),"runKind":row.get::<String,_>("run_kind"),"createdAt":row.get::<String,_>("created_text"),"startedAt":row.get::<Option<String>,_>("started_text"),"finishedAt":row.get::<Option<String>,_>("finished_text")})
}

fn item_detail(row: sqlx::postgres::PgRow) -> Value {
    json!({
        "itemRef": row.get::<Uuid, _>("item_ref"),
        "memberOrdinal": row.get::<i16, _>("member_ordinal"),
        "side": row.get::<String, _>("side"),
        "repeatOrdinal": row.get::<i16, _>("repeat_ordinal"),
        "attemptOrdinal": row.get::<i16, _>("attempt_ordinal"),
        "invocationRef": row.get::<Option<Uuid>, _>("invocation_ref"),
        "reusedFromItemRef": row.get::<Option<Uuid>, _>("reused_from_item_ref"),
        "executionDay": row.get::<Option<String>, _>("execution_day"),
        "state": row.get::<String, _>("state"),
        "structureAccepted": row.get::<Option<bool>, _>("structure_accepted"),
        "noSignal": row.get::<Option<bool>, _>("no_signal"),
        "summary": row.get::<Value, _>("safe_summary"),
        "usage": {"inputTokens": row.get::<Option<i64>, _>("input_tokens"), "outputTokens": row.get::<Option<i64>, _>("output_tokens")},
        "elapsedMs": row.get::<Option<i64>, _>("elapsed_ms"),
        "failureCode": row.get::<Option<String>, _>("failure_code"),
        "exclusionReason": row.get::<Option<String>, _>("exclusion_reason"),
        "createdAt": row.get::<String, _>("created_at"),
        "startedAt": row.get::<Option<String>, _>("started_at"),
        "finishedAt": row.get::<Option<String>, _>("finished_at"),
    })
}
