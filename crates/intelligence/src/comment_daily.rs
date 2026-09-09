//! Explicit daily material grants. SQL time windows use Asia/Shanghai and server corpus intake time.
use crate::{
    comment_research_rules::{RuleSnapshot, read_active_rule_in},
    model_plans::model_version,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub const DAILY_RULE: &str = "comment-research.v4";

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutdatedPolicy {
    Disabled,
    CurrentOnly,
    Historical,
}

/// The runner assigns an item to exactly one current owner/day. Its ordering is independent of
/// the original batch so work/token packing can never move historical work ahead of new intake.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueueClass {
    NewIntake,
    Recovery,
    Historical,
}

impl QueueClass {
    pub const fn priority(self) -> i16 {
        match self {
            Self::NewIntake => 0,
            Self::Recovery => 1,
            Self::Historical => 2,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutoPolicy {
    pub continuous_new: bool,
    pub historical_enabled: bool,
    pub history_start: Option<String>,
    pub outdated_policy: OutdatedPolicy,
    pub unknown_retry_max_attempts: i32,
    pub unknown_retry_token_limit: i64,
    pub day_token_limit: i64,
    pub replay_token_limit: i64,
    pub semantic_token_limit: i64,
}

impl Default for AutoPolicy {
    fn default() -> Self {
        Self {
            continuous_new: false,
            historical_enabled: false,
            history_start: None,
            outdated_policy: OutdatedPolicy::Disabled,
            unknown_retry_max_attempts: 0,
            unknown_retry_token_limit: 0,
            day_token_limit: 100_000,
            replay_token_limit: 0,
            semantic_token_limit: 0,
        }
    }
}

impl AutoPolicy {
    /// Existing explicit schedules authorize only their continuous new-intake lane. This helper
    /// keeps legacy focused tests explicit; a fresh API payload still defaults to no new calls.
    pub fn new_intake_only(day_token_limit: i64) -> Self {
        Self {
            continuous_new: true,
            day_token_limit,
            semantic_token_limit: day_token_limit,
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ModelError> {
        if !(0..=1).contains(&self.unknown_retry_max_attempts)
            || !(1024..=10_000_000).contains(&self.day_token_limit)
            || self.unknown_retry_token_limit < 0
            || self.replay_token_limit < 0
            || self.semantic_token_limit < 0
            || self.unknown_retry_token_limit > self.day_token_limit
            || self.replay_token_limit > self.day_token_limit
            || self.semantic_token_limit > self.day_token_limit
            || self.unknown_retry_token_limit + self.replay_token_limit + self.semantic_token_limit
                > self.day_token_limit
            || self
                .history_start
                .as_ref()
                .is_some_and(|start| start.is_empty() || start.len() > 64)
            || self.historical_enabled && self.history_start.is_none()
        {
            return Err(ModelError::Invalid);
        }
        Ok(())
    }

    pub(crate) fn parse(value: Value) -> Result<Self, ModelError> {
        let policy: Self = serde_json::from_value(value).map_err(|_| ModelError::Invalid)?;
        policy.validate()?;
        Ok(policy)
    }
}
pub async fn schema_ready(db: &Database) -> Result<bool, ModelError> {
    Ok(
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_semantic_work') IS NOT NULL AND to_regclass('linggan_comment_request_trace') IS NOT NULL AND to_regclass('linggan_ci_problem_boundary_decision') IS NOT NULL AND to_regclass('linggan_comment_research_rule_active') IS NOT NULL AND to_regclass('linggan_comment_legacy_fingerprint_alias') IS NOT NULL AND to_regclass('linggan_comment_research_eligibility_current') IS NOT NULL")
            .fetch_one(db.pool())
            .await?,
)
}

fn frozen_rule_request(rule: &RuleSnapshot) -> Value {
    json!({
        "ruleRevisionRef": rule.rule_revision_ref,
        "ruleHash": rule.canonical_hash,
        "ruleVersion": rule.rule_version.as_str(),
        "schemaVersion": rule.schema_version,
        "selectorVersion": rule.selector_version,
    })
}

fn request_has_frozen_rule(request: &Value) -> bool {
    request.get("ruleRevisionRef").is_some()
        || request.get("ruleVersion").and_then(Value::as_str) == Some(DAILY_RULE)
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DailySchedule {
    pub expected_revision: i32,
    pub enabled: bool,
    pub config_ref: Uuid,
    pub source_limit: i32,
    pub token_limit: i64,
    #[serde(default)]
    pub auto_policy: AutoPolicy,
}
async fn validate_config(
    db: &Database,
    config: Uuid,
    sources: i32,
    tokens: i64,
) -> Result<(), ModelError> {
    if !(1..=3000).contains(&sources) || !(1024..=10000000).contains(&tokens) {
        return Err(ModelError::Invalid);
    }
    let allowed:bool=sqlx::query_scalar(crate::model_settings::with_model_callability("SELECT EXISTS(SELECT 1 FROM linggan_model_config cfg JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE cfg.config_ref=$1 AND c.enabled AND cfg.input_token_limit+cfg.output_token_limit<=$2 AND __MODEL_CALLABLE__)"))
        .bind(config).bind(tokens).fetch_one(db.pool()).await?;
    if !allowed {
        return Err(ModelError::NotQualified);
    }
    Ok(())
}
pub async fn save_schedule(db: &Database, r: &DailySchedule) -> Result<Value, ModelError> {
    r.auto_policy.validate()?;
    if let Some(history_start) = &r.auto_policy.history_start {
        let valid: bool =
            sqlx::query_scalar("SELECT pg_input_is_valid($1,'timestamp with time zone')")
                .bind(history_start)
                .fetch_one(db.pool())
                .await?;
        if !valid {
            return Err(ModelError::Invalid);
        }
    }
    if r.enabled {
        validate_config(db, r.config_ref, r.source_limit, r.token_limit).await?;
    } else if !(1..=3000).contains(&r.source_limit) || !(1024..=10_000_000).contains(&r.token_limit)
    {
        return Err(ModelError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let row=sqlx::query("UPDATE linggan_comment_daily_schedule SET revision=revision+1,enabled=$2,config_ref=$3,source_limit=$4,token_limit=$5,auto_policy=$6,auto_enabled_at=CASE WHEN $2 AND auto_enabled_at IS NULL THEN scope_001_now() ELSE auto_enabled_at END,legacy_activation_unknown=CASE WHEN $2 AND auto_enabled_at IS NULL THEN false ELSE legacy_activation_unknown END,next_start=CASE WHEN $2 THEN COALESCE(next_start,scope_001_now()) ELSE next_start END,next_end=CASE WHEN $2 THEN COALESCE(next_end,(date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai')+interval '23 hours'+CASE WHEN (scope_001_now() AT TIME ZONE 'Asia/Shanghai')::time>=time '23:00' THEN interval '1 day' ELSE interval '0' END) AT TIME ZONE 'Asia/Shanghai') ELSE next_end END,updated_at=scope_001_now() WHERE singleton AND revision=$1 RETURNING revision")
        .bind(r.expected_revision).bind(r.enabled).bind(r.config_ref).bind(r.source_limit).bind(r.token_limit).bind(serde_json::to_value(&r.auto_policy).map_err(|_|ModelError::Invalid)?).fetch_optional(&mut *tx).await?.ok_or(ModelError::Conflict)?;
    sqlx::query("UPDATE linggan_comment_daily_schedule SET context_policy=(SELECT policy FROM linggan_comment_context_settings WHERE singleton) WHERE singleton").execute(&mut *tx).await?;
    // Daily is the new scheduling authority; do not leave the previous continuous auto loop active.
    sqlx::query("UPDATE linggan_model_plan SET enabled=false,revision=revision+1 WHERE kind='automatic' AND enabled").execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_model_workspace SET active_auto_plan_ref=NULL WHERE singleton")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(
        json!({"revision":row.get::<i32,_>("revision"),"enabled":r.enabled,"autoPolicy":r.auto_policy}),
    )
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedBatch {
    #[serde(default)]
    pub reanalyze: bool,
    pub batch_ref: Uuid,
    pub config_ref: Uuid,
    pub source_refs: Vec<Uuid>,
    pub token_limit: i64,
}
pub async fn create_selected(db: &Database, r: &SelectedBatch) -> Result<Value, ModelError> {
    if r.source_refs.is_empty()
        || r.source_refs.len() > 3000
        || r.source_refs
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != r.source_refs.len()
    {
        return Err(ModelError::Invalid);
    }
    let mut request = serde_json::to_value(r).map_err(|_| ModelError::Invalid)?;
    request["cleanerVersion"] = json!(crate::comment_cleaning::CLEANER_VERSION);
    validate_config(db, r.config_ref, r.source_refs.len() as i32, r.token_limit).await?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let rule = read_active_rule_in(&mut *tx).await?;
    if let Some(prepared) = sqlx::query_scalar::<_,Value>("SELECT preflight FROM linggan_ci_prepare WHERE prepare_ref=$1 AND expires_at>scope_001_now()")
        .bind(r.batch_ref).fetch_optional(&mut *tx).await? {
        if prepared["ruleHash"].as_str()!=Some(rule.canonical_hash.as_str()) { return Err(ModelError::Conflict); }
    }
    request["rule"] = frozen_rule_request(&rule);
    request["ruleRevisionRef"] = json!(rule.rule_revision_ref);
    request["ruleHash"] = json!(rule.canonical_hash);
    request["ruleVersion"] = json!(rule.rule_version.as_str());
    request["schemaVersion"] = json!(rule.schema_version);
    request["selectorVersion"] = json!(rule.selector_version);
    request["autoPolicy"] = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(old) = sqlx::query_scalar::<_, Value>(
        "SELECT request FROM linggan_comment_daily_batch WHERE batch_ref=$1",
    )
    .bind(r.batch_ref)
    .fetch_optional(&mut *tx)
    .await?
    {
        let original = serde_json::to_value(r).map_err(|_| ModelError::Invalid)?;
        if original.as_object().is_none_or(|fields| {
            fields
                .iter()
                .any(|(key, value)| old.get(key) != Some(value))
        }) {
            return Err(ModelError::Conflict);
        }
        return Ok(json!({"batchRef":r.batch_ref,"replayed":true}));
    }
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_readable s JOIN linggan_material_comment_current c USING(material_ref) WHERE s.material_ref=ANY($1)").bind(&r.source_refs).fetch_one(&mut *tx).await?;
    if count != r.source_refs.len() as i64 {
        return Err(ModelError::Source);
    }
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request,context_policy) VALUES($1,'selected',$2,scope_001_now(),scope_001_now(),$3,$4,$5,COALESCE((SELECT context_policy FROM linggan_ci_prepare WHERE prepare_ref=$1),(SELECT policy FROM linggan_comment_context_settings WHERE singleton)))")
        .bind(r.batch_ref).bind(r.config_ref).bind(r.source_refs.len()as i32).bind(r.token_limit).bind(request).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO linggan_comment_daily_item(batch_ref,source_ref) SELECT $1,unnest($2::uuid[])",
    )
    .bind(r.batch_ref)
    .bind(&r.source_refs)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({"batchRef":r.batch_ref,"replayed":false}))
}
pub async fn seal_due(db: &Database) -> Result<bool, ModelError> {
    let mut tx = db.pool().begin().await?;
    // Serialize manifest ownership transfer with packet reservations.
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let Some(r)=sqlx::query("SELECT *,next_start::text AS start_text,next_end::text AS end_text FROM linggan_comment_daily_schedule WHERE singleton AND enabled AND auto_policy->>'continuousNew'='true' AND next_end<=scope_001_now() FOR UPDATE SKIP LOCKED").fetch_optional(&mut *tx).await? else{return Ok(false)};
    let batch = Uuid::new_v4();
    let start: String = r.get("start_text");
    let end: String = r.get("end_text");
    // Snapshot both intake and transferable ownership before inserting the immutable batch manifest.
    let intake:Vec<Uuid>=sqlx::query_scalar("SELECT c.material_ref FROM linggan_comment_research_current($2::timestamptz-interval '1 microsecond') c JOIN linggan_comment_research_readable s USING(material_ref) JOIN LATERAL (SELECT min(created_at) AS first_at FROM linggan_material_comment a WHERE a.content_public_ref=c.content_public_ref AND a.comment_external_id=c.comment_external_id) first_seen ON true WHERE first_seen.first_at>=$1::timestamptz AND first_seen.first_at<$2::timestamptz")
        .bind(&start).bind(&end).fetch_all(&mut *tx).await?;
    let rule = read_active_rule_in(&mut *tx).await?;
    let transferable=sqlx::query("SELECT i.batch_ref,i.source_ref,COALESCE(i.origin_batch_ref,i.batch_ref) AS origin FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) WHERE b.kind='daily' AND b.enabled AND b.window_end<=$1::timestamptz AND i.state IN('pending','source_limit') AND i.attempts=0 AND i.semantic_ref IS NULL AND EXISTS(SELECT 1 FROM linggan_comment_research_readable allowed WHERE allowed.material_ref=i.source_ref) ORDER BY b.window_end,i.source_ref")
        .bind(&start).fetch_all(&mut *tx).await?;
    let mut backlog = std::collections::BTreeMap::<Uuid, Uuid>::new();
    for item in &transferable {
        let source: Uuid = item.get("source_ref");
        if !intake.contains(&source) {
            backlog.entry(source).or_insert(item.get("origin"));
        }
    }
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request,context_policy) VALUES($1,'daily',$2,$3::timestamptz,$4::timestamptz,$5,$6,$7,(SELECT policy FROM linggan_comment_context_settings WHERE singleton))")
        .bind(batch).bind(r.get::<Uuid,_>("config_ref")).bind(&start).bind(&end).bind(r.get::<i32,_>("source_limit")).bind(r.get::<i64,_>("token_limit"))
        .bind(json!({"scheduleRevision":r.get::<i32,_>("revision"),"timezone":"Asia/Shanghai","cleanerVersion":crate::comment_cleaning::CLEANER_VERSION,"rule":frozen_rule_request(&rule),"ruleRevisionRef":rule.rule_revision_ref,"ruleHash":rule.canonical_hash,"ruleVersion":rule.rule_version.as_str(),"schemaVersion":rule.schema_version,"selectorVersion":rule.selector_version,"autoPolicy":r.get::<Value,_>("auto_policy"),"newIntakeCount":intake.len(),"backlogCount":backlog.len()})).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO linggan_comment_daily_item(batch_ref,source_ref,queue_class,execution_day) SELECT $1,unnest($2::uuid[]),'new_intake',(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date",
    )
    .bind(batch)
    .bind(intake)
    .execute(&mut *tx)
    .await?;
    // A dispatch count of zero and absent semantic claim are both required. Unknown-cost calls
    // and failures retain their old grant; only untouched work receives the next daily budget.
    for item in &transferable {
        sqlx::query("UPDATE linggan_comment_daily_item SET state='carried_forward',failure_code='carried_forward' WHERE batch_ref=$1 AND source_ref=$2 AND state IN('pending','source_limit') AND attempts=0 AND semantic_ref IS NULL")
            .bind(item.get::<Uuid,_>("batch_ref")).bind(item.get::<Uuid,_>("source_ref")).execute(&mut *tx).await?;
    }
    let refs: Vec<Uuid> = backlog.keys().copied().collect();
    let origins: Vec<Uuid> = backlog.values().copied().collect();
    sqlx::query("INSERT INTO linggan_comment_daily_item(batch_ref,source_ref,origin_batch_ref,queue_class,execution_day) SELECT $1,x.source_ref,x.origin,'historical',(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date FROM unnest($2::uuid[],$3::uuid[]) x(source_ref,origin)").bind(batch).bind(refs).bind(origins).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_schedule SET next_start=next_end,next_end=next_end+interval '1 day' WHERE singleton").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}
/// Recovery after downtime must advance every missed half-open [start, 23:00) window rather
/// than resetting the waterline to now. The reservation runner calls this before it chooses a
/// queue class, so each historical batch remains an auditable manifest.
pub async fn seal_all_due(db: &Database) -> Result<u64, ModelError> {
    let mut sealed = 0;
    while seal_due(db).await? {
        sealed += 1;
    }
    Ok(sealed)
}
/// A content revision is follow-up work under the original daily grant, never new intake.
pub(crate) async fn seal_supplements(db: &Database) -> Result<(), ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let rule = read_active_rule_in(&mut *tx).await?;
    let Some(auto_policy): Option<Value> = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton AND enabled",
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        tx.commit().await?;
        return Ok(());
    };
    if AutoPolicy::parse(auto_policy.clone())?.outdated_policy == OutdatedPolicy::Disabled {
        tx.commit().await?;
        return Ok(());
    }
    let rows=sqlx::query("SELECT DISTINCT b.batch_ref,b.config_ref,b.window_start::text AS start_text,b.window_end::text AS end_text,b.source_limit,b.token_limit,current.material_ref FROM linggan_comment_daily_batch b JOIN linggan_comment_daily_item i USING(batch_ref) JOIN linggan_material_comment old ON old.material_ref=i.source_ref JOIN linggan_material_comment_current current ON current.content_public_ref=old.content_public_ref AND current.comment_external_id=old.comment_external_id JOIN linggan_comment_research_readable allowed ON allowed.material_ref=current.material_ref WHERE b.kind='daily' AND b.enabled AND (b.request ? 'ruleRevisionRef' OR b.request->>'ruleVersion'='comment-research.v4') AND old.body_text IS DISTINCT FROM current.body_text AND NOT EXISTS(SELECT 1 FROM linggan_comment_daily_batch prior_batch JOIN linggan_comment_daily_item prior_item USING(batch_ref) JOIN linggan_material_comment prior_source ON prior_source.material_ref=prior_item.source_ref WHERE prior_batch.kind='supplement' AND prior_batch.request->>'originBatchRef'=b.batch_ref::text AND prior_source.content_public_ref=current.content_public_ref AND prior_source.comment_external_id=current.comment_external_id AND prior_source.body_text IS NOT DISTINCT FROM current.body_text) AND NOT EXISTS(SELECT 1 FROM linggan_comment_daily_batch supplement WHERE supplement.kind='supplement' AND supplement.request->>'originBatchRef'=b.batch_ref::text AND supplement.request->>'sourceRef'=current.material_ref::text) ORDER BY b.batch_ref,current.material_ref LIMIT 100").fetch_all(&mut *tx).await?;
    for r in rows {
        let batch = Uuid::new_v4();
        let origin: Uuid = r.get("batch_ref");
        let source: Uuid = r.get("material_ref");
        sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request,context_policy) VALUES($1,'supplement',$2,$3::timestamptz,$4::timestamptz,$5,$6,$7,(SELECT context_policy FROM linggan_comment_daily_batch WHERE batch_ref=($7->>'originBatchRef')::uuid))").bind(batch).bind(r.get::<Uuid,_>("config_ref")).bind(r.get::<String,_>("start_text")).bind(r.get::<String,_>("end_text")).bind(r.get::<i32,_>("source_limit")).bind(r.get::<i64,_>("token_limit")).bind(json!({"originBatchRef":origin,"sourceRef":source,"reason":"source_text_revised","rule":frozen_rule_request(&rule),"ruleRevisionRef":rule.rule_revision_ref,"ruleHash":rule.canonical_hash,"ruleVersion":rule.rule_version.as_str(),"schemaVersion":rule.schema_version,"selectorVersion":rule.selector_version,"autoPolicy":auto_policy,"cleanerVersion":crate::comment_cleaning::CLEANER_VERSION,"newIntake":false})).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO linggan_comment_daily_item(batch_ref,source_ref) VALUES($1,$2)")
            .bind(batch)
            .bind(source)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchAction {
    pub enabled: bool,
}
pub async fn set_batch_enabled(
    db: &Database,
    batch: Uuid,
    enabled: bool,
) -> Result<Value, ModelError> {
    if enabled {
        let compatible:bool=sqlx::query_scalar("SELECT COALESCE((SELECT request ? 'ruleRevisionRef' OR request->>'ruleVersion'=$2 FROM linggan_comment_daily_batch WHERE batch_ref=$1),false)").bind(batch).bind(DAILY_RULE).fetch_one(db.pool()).await?;
        if !compatible {
            return Err(ModelError::NotQualified);
        }
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if sqlx::query("UPDATE linggan_comment_daily_batch SET enabled=$2 WHERE batch_ref=$1")
        .bind(batch)
        .bind(enabled)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        != 1
    {
        return Err(ModelError::NotFound);
    }
    tx.commit().await?;
    Ok(json!({"batchRef":batch,"enabled":enabled}))
}
pub fn version(config: Uuid) -> String {
    model_version(config)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryDaily {
    pub command_ref: Uuid,
}
pub async fn retry_failed(db: &Database, batch: Uuid, command: Uuid) -> Result<Value, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query(
        "SELECT batch_ref,result FROM linggan_comment_daily_command WHERE command_ref=$1",
    )
    .bind(command)
    .fetch_optional(&mut *tx)
    .await?
    {
        if row.get::<Uuid, _>("batch_ref") != batch {
            return Err(ModelError::Conflict);
        }
        return Ok(row.get("result"));
    }
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_batch WHERE batch_ref=$1 AND enabled AND (request ? 'ruleRevisionRef' OR request->>'ruleVersion'='comment-research.v4'))",
    )
    .bind(batch)
    .fetch_one(&mut *tx)
    .await?;
    if !allowed {
        return Err(ModelError::Disabled);
    }
    let queued=sqlx::query("UPDATE linggan_comment_daily_item i SET state='pending',failure_code=NULL FROM linggan_comment_daily_batch b JOIN linggan_model_config c USING(config_ref) WHERE i.batch_ref=b.batch_ref AND b.batch_ref=$1 AND i.state='failed' AND (i.semantic_ref IS NULL OR EXISTS(SELECT 1 FROM linggan_comment_semantic_work sw LEFT JOIN linggan_model_invocation v ON v.invocation_ref=sw.invocation_ref WHERE sw.semantic_ref=i.semantic_ref AND (sw.failure_code=ANY($2) OR v.input_tokens IS NULL OR v.output_tokens IS NULL))) AND i.attempts<c.max_attempts AND NOT EXISTS(SELECT 1 FROM linggan_comment_semantic_work sw WHERE sw.semantic_ref=i.semantic_ref AND sw.attempts>=c.max_attempts) AND NOT EXISTS(SELECT 1 FROM linggan_comment_semantic_work sw JOIN linggan_model_invocation v ON v.invocation_ref=sw.invocation_ref WHERE sw.semantic_ref=i.semantic_ref AND (v.input_tokens IS NULL OR v.output_tokens IS NULL) AND COALESCE((v.result->>'callStarted')::boolean,true) AND NOT EXISTS(SELECT 1 FROM linggan_comment_daily_adjustment a WHERE a.kind='usage_review' AND a.request->>'invocationRef'=v.invocation_ref::text AND (a.request->>'acceptDuplicateCharge'='true' OR (a.request->>'inputTokens' IS NOT NULL AND a.request->>'outputTokens' IS NOT NULL)))) AND EXISTS(SELECT 1 FROM linggan_comment_research_readable s WHERE s.material_ref=i.source_ref)").bind(batch).bind(crate::comment_research_fingerprint::RECOVERABLE_FAILURE_CODES).execute(&mut *tx).await?.rows_affected();
    let result = json!({"batchRef":batch,"queued":queued});
    sqlx::query(
        "INSERT INTO linggan_comment_daily_command(command_ref,batch_ref,result) VALUES($1,$2,$3)",
    )
    .bind(command)
    .bind(batch)
    .bind(&result)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContinueDaily {
    pub command_ref: Uuid,
    pub source_limit: i32,
    pub token_limit: i64,
    pub reason: String,
}
/// Explicit continuation changes only undispatched allowances, retaining the frozen target manifest.
pub async fn continue_batch(
    db: &Database,
    batch: Uuid,
    r: &ContinueDaily,
) -> Result<Value, ModelError> {
    if !(1..=3000).contains(&r.source_limit)
        || !(1024..=10_000_000).contains(&r.token_limit)
        || r.reason.trim().is_empty()
        || r.reason.chars().count() > 500
    {
        return Err(ModelError::Invalid);
    }
    let batch:Uuid=sqlx::query_scalar("SELECT COALESCE((request->>'originBatchRef')::uuid,batch_ref) FROM linggan_comment_daily_batch WHERE batch_ref=$1").bind(batch).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    let request = serde_json::to_value(r).map_err(|_| ModelError::Invalid)?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(old) = sqlx::query(
        "SELECT batch_ref,kind,request FROM linggan_comment_daily_adjustment WHERE command_ref=$1",
    )
    .bind(r.command_ref)
    .fetch_optional(&mut *tx)
    .await?
    {
        if old.get::<Uuid, _>("batch_ref") != batch
            || old.get::<String, _>("kind") != "continue"
            || old.get::<Value, _>("request") != request
        {
            return Err(ModelError::Conflict);
        }
        return Ok(json!({"batchRef":batch,"replayed":true}));
    }
    let old=sqlx::query("SELECT b.source_limit,b.token_limit,b.request,c.input_token_limit+c.output_token_limit AS reservation FROM linggan_comment_daily_batch b JOIN linggan_model_config c USING(config_ref) WHERE batch_ref=$1").bind(batch).fetch_optional(&mut *tx).await?.ok_or(ModelError::NotFound)?;
    let old_request: Value = old.get("request");
    if !request_has_frozen_rule(&old_request) {
        return Err(ModelError::NotQualified);
    }
    if r.source_limit < old.get::<i32, _>("source_limit")
        || r.token_limit < old.get::<i64, _>("token_limit")
        || r.token_limit < i64::from(old.get::<i32, _>("reservation"))
    {
        return Err(ModelError::Invalid);
    }
    sqlx::query("INSERT INTO linggan_comment_daily_adjustment(command_ref,batch_ref,kind,request) VALUES($1,$2,'continue',$3)").bind(r.command_ref).bind(batch).bind(request).execute(&mut *tx).await?;
    let queued=sqlx::query("UPDATE linggan_comment_daily_item SET state='pending',failure_code=NULL WHERE batch_ref=$1 AND state='source_limit'").bind(batch).execute(&mut *tx).await?.rows_affected();
    tx.commit().await?;
    Ok(
        json!({"batchRef":batch,"queued":queued,"sourceLimit":r.source_limit,"tokenLimit":r.token_limit}),
    )
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewUsage {
    pub command_ref: Uuid,
    pub invocation_ref: Uuid,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub accept_duplicate_charge: bool,
    pub reason: String,
}
/// Human reconciliation is audited separately from provider usage; unknown reservations stay reserved.
pub async fn review_usage(
    db: &Database,
    batch: Uuid,
    r: &ReviewUsage,
) -> Result<Value, ModelError> {
    if r.reason.trim().is_empty()
        || r.reason.chars().count() > 500
        || r.input_tokens.is_some() != r.output_tokens.is_some()
        || r.input_tokens
            .into_iter()
            .chain(r.output_tokens)
            .any(|v| !(0..=100_000_000).contains(&v))
        || (!r.accept_duplicate_charge && r.input_tokens.is_none())
    {
        return Err(ModelError::Invalid);
    }
    let request = serde_json::to_value(r).map_err(|_| ModelError::Invalid)?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(old) = sqlx::query(
        "SELECT batch_ref,kind,request FROM linggan_comment_daily_adjustment WHERE command_ref=$1",
    )
    .bind(r.command_ref)
    .fetch_optional(&mut *tx)
    .await?
    {
        if old.get::<Uuid, _>("batch_ref") != batch
            || old.get::<String, _>("kind") != "usage_review"
            || old.get::<Value, _>("request") != request
        {
            return Err(ModelError::Conflict);
        }
        return Ok(json!({"batchRef":batch,"replayed":true}));
    }
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) WHERE p.batch_ref=$1 AND p.invocation_ref=$2 AND v.state<>'running' AND (v.input_tokens IS NULL OR v.output_tokens IS NULL))").bind(batch).bind(r.invocation_ref).fetch_one(&mut *tx).await?;
    if !exists {
        return Err(ModelError::Conflict);
    }
    sqlx::query("INSERT INTO linggan_comment_daily_adjustment(command_ref,batch_ref,kind,request) VALUES($1,$2,'usage_review',$3)").bind(r.command_ref).bind(batch).bind(request).execute(&mut *tx).await?;
    if let Some((input, output)) = r.input_tokens.zip(r.output_tokens) {
        sqlx::query("UPDATE linggan_model_invocation SET charged_tokens=$2,result=COALESCE(result,'{}'::jsonb)||jsonb_build_object('usageReviewRef',$3::text,'usageReviewOrigin','human','reviewedInputTokens',$4::bigint,'reviewedOutputTokens',$5::bigint) WHERE invocation_ref=$1 AND state<>'running'").bind(r.invocation_ref).bind(input+output).bind(r.command_ref.to_string()).bind(input).bind(output).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(
        json!({"batchRef":batch,"invocationRef":r.invocation_ref,"reviewed":true,"duplicateChargeAccepted":r.accept_duplicate_charge}),
    )
}

pub use crate::comment_cleaning::{CLEANER_VERSION, clean_pending};
pub use crate::comment_daily_read::{batch_detail, cleaning_members, overview, source_states};
pub use crate::comment_daily_runner::{run_daily_once, run_daily_once_with_drain};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_schedule_policy_does_not_authorize_automatic_calls_by_default() {
        let policy = AutoPolicy::default();
        assert!(!policy.continuous_new);
        assert!(!policy.historical_enabled);
        assert_eq!(policy.outdated_policy, OutdatedPolicy::Disabled);
        assert_eq!(policy.unknown_retry_max_attempts, 0);
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn historical_and_budget_policies_are_bounded() {
        let mut policy = AutoPolicy::new_intake_only(100_000);
        policy.historical_enabled = true;
        assert!(policy.validate().is_err());
        policy.history_start = Some("2026-09-01T00:00:00+08:00".into());
        policy.semantic_token_limit = 80_000;
        policy.replay_token_limit = 15_000;
        policy.unknown_retry_token_limit = 5_001;
        assert!(policy.validate().is_err());
        policy.unknown_retry_token_limit = 5_000;
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn queue_priority_never_moves_history_ahead_of_recovery_or_new_intake() {
        assert!(QueueClass::NewIntake.priority() < QueueClass::Recovery.priority());
        assert!(QueueClass::Recovery.priority() < QueueClass::Historical.priority());
    }
}
