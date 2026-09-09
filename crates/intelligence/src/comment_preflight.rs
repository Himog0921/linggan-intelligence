//! Read-only planning uses the same semantic eligibility as the reservation transaction.
use crate::{
    comment_cleaning::CLEANER_VERSION,
    comment_packet::build_packet_with_rule,
    comment_research_fingerprint::{
        self as eligibility, CleanState, EligibilityCategory, EligibilityRequest, RecoveryPolicy,
        RuleFingerprint, SourceIdentity, SourceQualification,
    },
    comment_research_rules::{self, RuleSnapshot},
    comment_runtime::ContextPolicy,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Row, postgres::PgRow};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const MAX_RESEARCH_SOURCES: usize = 3000;

struct PlanningContext<'a> {
    config: Option<PgRow>,
    workspace: Uuid,
    rule: RuleSnapshot,
    policy: &'a ContextPolicy,
    recovery: RecoveryPolicy,
    reanalyze: bool,
}
struct PlannedSource {
    reference: Uuid,
    work: Uuid,
    legacy_category: &'static str,
    category: EligibilityCategory,
    fingerprint: String,
    eligibility: Value,
    dispatch: bool,
    tokens: usize,
}

pub async fn inspect(
    db: &Database,
    refs: &[Uuid],
    policy: &ContextPolicy,
    reanalyze: bool,
) -> Result<Value, ModelError> {
    let context = planning_context(db, policy, reanalyze).await?;
    let rows=sqlx::query("SELECT material_ref,content_public_ref,comment_external_id,body_text FROM linggan_comment_research_readable WHERE material_ref=ANY($1) ORDER BY content_public_ref,material_ref")
        .bind(refs).fetch_all(db.pool()).await?;
    if rows.len() != refs.len() {
        return Err(ModelError::Source);
    }
    let mut plans = Vec::new();
    for row in rows {
        plans.push(plan_source(db, &row, &context).await?);
    }
    Ok(summarize(&plans, &context))
}
async fn planning_context<'a>(
    db: &Database,
    policy: &'a ContextPolicy,
    reanalyze: bool,
) -> Result<PlanningContext<'a>, ModelError> {
    let workspace =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await?;
    let config=sqlx::query(crate::model_settings::with_model_callability("SELECT c.config_ref,c.input_token_limit,c.output_token_limit,c.max_attempts FROM linggan_model_workspace w JOIN linggan_model_config c ON c.config_ref=w.default_config_ref JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE w.singleton AND conn.enabled AND __MODEL_CALLABLE__")).fetch_optional(db.pool()).await?;
    let auto: Value = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_one(db.pool())
    .await?;
    let auto: crate::comment_daily::AutoPolicy =
        serde_json::from_value(auto).map_err(|_| ModelError::Invalid)?;
    auto.validate()?;
    let recovery = RecoveryPolicy {
        retry_max_attempts: config.as_ref().map(|c| c.get("max_attempts")).unwrap_or(0),
        unknown_retry_max_attempts: auto.unknown_retry_max_attempts,
        retry_cooldown_seconds: 60,
    };
    Ok(PlanningContext {
        config,
        workspace,
        rule: comment_research_rules::read_active_rule(db).await?,
        policy,
        recovery,
        reanalyze,
    })
}
async fn plan_source(
    db: &Database,
    row: &PgRow,
    context: &PlanningContext<'_>,
) -> Result<PlannedSource, ModelError> {
    let reference: Uuid = row.get("material_ref");
    let work: Uuid = row.get("content_public_ref");
    if row.get::<Option<String>, _>("body_text").is_none() {
        return Ok(PlannedSource {
            reference,
            work,
            legacy_category: "anomaly",
            category: EligibilityCategory::Anomaly,
            fingerprint: crate::comment_research::comment_source_hash(""),
            eligibility: json!({"sourceRef":reference,"category":"anomaly","resultState":"unstudied",
                "executionState":"idle","reasonCode":"comment_body_missing","eligibleToDispatch":false}),
            dispatch: false,
            tokens: 0,
        });
    }
    let packet = build_packet_with_rule(
        db,
        &[reference],
        "preflight-no-call",
        context.policy,
        &context.rule,
    )
    .await?;
    let input = &packet.inputs[0];
    let state = match packet.cleaned[0].state.as_str() {
        "direct" => CleanState::Direct,
        "context" => CleanState::Context,
        "dropped" => CleanState::Dropped,
        "anomaly" => CleanState::Anomaly,
        _ => CleanState::Unknown,
    };
    let mut qualification = SourceQualification::eligible(state);
    qualification.context_missing =
        !crate::comment_daily_runner::missing_context(&packet).is_empty();
    let rule = RuleFingerprint::from(&context.rule);
    let semantic = eligibility::build_semantic_input(
        input,
        &SourceIdentity {
            work_ref: work,
            comment_external_id: row.get("comment_external_id"),
        },
        &rule,
    );
    let result = eligibility::evaluate_eligibility(
        db,
        &EligibilityRequest {
            workspace_ref: context.workspace,
            source_ref: reference,
            input: semantic,
            qualification,
            // A read-only preview generation is deterministic and never used for dispatch.
            explicit_generation: context.reanalyze.then_some(Uuid::nil()),
            recovery: context.recovery.clone(),
        },
    )
    .await?;
    let mut legacy_category = match result.category {
        EligibilityCategory::Reusable => "reusable",
        EligibilityCategory::InProgress => "inProgress",
        EligibilityCategory::FailedRecovery => "retryRequired",
        EligibilityCategory::ContextMissing => "contextMissing",
        EligibilityCategory::Anomaly => "anomaly",
        EligibilityCategory::Excluded => "dropped",
        _ => "newAnalysis",
    };
    let input_bound = packet.prompt.len() + packet.system.len() + 512;
    let mut dispatch = result.eligible_to_dispatch && context.config.is_some();
    let mut diagnostic = json!(result);
    if dispatch
        && context
            .config
            .as_ref()
            .is_some_and(|c| input_bound > c.get::<i32, _>("input_token_limit") as usize)
    {
        dispatch = false;
        legacy_category = "inputTooLarge";
        diagnostic["eligibleToDispatch"] = json!(false);
        diagnostic["reasonCode"] = json!("model_input_limit");
    }
    let tokens = if dispatch {
        input_bound
            + context
                .config
                .as_ref()
                .unwrap()
                .get::<i32, _>("output_token_limit") as usize
    } else {
        0
    };
    Ok(PlannedSource {
        reference,
        work,
        legacy_category,
        category: result.category,
        fingerprint: result.base_fingerprint,
        eligibility: diagnostic,
        dispatch,
        tokens,
    })
}
fn summarize(plans: &[PlannedSource], context: &PlanningContext<'_>) -> Value {
    let mut counts = BTreeMap::from([
        ("reusable", 0usize),
        ("newAnalysis", 0),
        ("dropped", 0),
        ("contextMissing", 0),
        ("anomaly", 0),
        ("retryRequired", 0),
        ("inProgress", 0),
        ("inputTooLarge", 0),
    ]);
    let mut categories: BTreeMap<String, usize> = BTreeMap::new();
    let mut fingerprints = serde_json::Map::new();
    let mut by_work: BTreeMap<Uuid, usize> = BTreeMap::new();
    let mut tokens = 0;
    for plan in plans {
        *counts.entry(plan.legacy_category).or_default() += 1;
        let key = serde_json::to_value(plan.category)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        *categories.entry(key).or_default() += 1;
        fingerprints.insert(plan.reference.to_string(), json!(plan.fingerprint));
        if plan.dispatch {
            *by_work.entry(plan.work).or_default() += 1;
            tokens += plan.tokens;
        }
    }
    let max_packet = context
        .config
        .as_ref()
        .map(|c| (c.get::<i32, _>("output_token_limit") / 256).clamp(1, 30) as usize)
        .unwrap_or(1)
        .min(context.policy.max_comments);
    let minimum_calls: usize = by_work.values().map(|n| n.div_ceil(max_packet)).sum();
    let dispatch_count: usize = by_work.values().sum();
    json!({"total":plans.len(),"counts":counts,"categories":categories,
        "items":plans.iter().map(|p|&p.eligibility).collect::<Vec<_>>(),
        "configRef":context.config.as_ref().map(|c|c.get::<Uuid,_>("config_ref")),"fingerprints":fingerprints,
        "cleanerVersion":CLEANER_VERSION,"contractVersion":context.rule.rule_version,
        "ruleRevisionRef":context.rule.rule_revision_ref,"ruleHash":context.rule.canonical_hash,
        "plannedExternalComments":dispatch_count,"plannedWorks":by_work.len(),
        "estimatedCalls":{"min":minimum_calls,"max":dispatch_count,"available":context.config.is_some()},
        "tokenUpperBound":if context.config.is_some(){Some(tokens)}else{None},
        "estimateMethod":"单条输入UTF8字节保守上界加输出预算；实际按作品分包，用量以供应商回执为准"})
}
