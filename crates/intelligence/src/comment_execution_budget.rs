//! One execution-day ledger across extraction, replay, and semantic organization.
//! Callers lock the workspace row and insert their invocation in the same transaction.
use crate::{comment_daily::AutoPolicy, model_settings::ModelError};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};

#[derive(Clone, Copy, Debug)]
pub(crate) enum BudgetPurpose {
    Extraction,
    Replay,
    Semantic,
}
impl BudgetPurpose {
    fn as_str(self) -> &'static str {
        match self {
            Self::Extraction => "extraction",
            Self::Replay => "replay",
            Self::Semantic => "semantic",
        }
    }
}
pub(crate) struct BudgetDecision {
    pub allowed: bool,
    pub execution_day: String,
    pub reason_code: Option<&'static str>,
    pub ledger_metadata: Value,
}

pub(crate) async fn check_budget_in(
    conn: &mut PgConnection,
    purpose: BudgetPurpose,
    requested_tokens: i64,
    unknown_recovery: bool,
) -> Result<BudgetDecision, ModelError> {
    if requested_tokens <= 0 {
        return Err(ModelError::Invalid);
    }
    let configured: Option<Value> = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_optional(&mut *conn)
    .await?;
    let policy = configured
        .map(AutoPolicy::parse)
        .transpose()?
        .unwrap_or_default();
    // Missing legacy purpose tags still count toward the global ceiling. An uncertain call
    // retains its charged reservation; it never becomes free merely because it failed.
    let usage = sqlx::query(r#"
        SELECT (scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date::text AS execution_day,
          COALESCE(sum(charged_tokens),0)::bigint AS total,
          COALESCE(sum(charged_tokens) FILTER(WHERE result->>'budgetPurpose'=$1),0)::bigint AS purpose,
          COALESCE(sum(charged_tokens) FILTER(WHERE result->>'unknownRecovery'='true'),0)::bigint AS unknown
        FROM linggan_model_invocation
        WHERE operation IN ('analyze','embed')
          AND created_at >= date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'
          AND created_at < (date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') + interval '1 day') AT TIME ZONE 'Asia/Shanghai'
    "#).bind(purpose.as_str()).fetch_one(&mut *conn).await?;
    let execution_day: String = usage.get("execution_day");
    let purpose_limit = match purpose {
        BudgetPurpose::Extraction => policy.day_token_limit,
        BudgetPurpose::Replay => policy.replay_token_limit,
        BudgetPurpose::Semantic => policy.semantic_token_limit,
    };
    let exceeds = |used: i64, limit: i64| requested_tokens > limit.saturating_sub(used);
    let reason_code = if exceeds(usage.get("total"), policy.day_token_limit) {
        Some("day_budget_exhausted")
    } else if exceeds(usage.get("purpose"), purpose_limit) {
        Some("purpose_budget_exhausted")
    } else if unknown_recovery
        && (policy.unknown_retry_max_attempts == 0
            || exceeds(usage.get("unknown"), policy.unknown_retry_token_limit))
    {
        Some("unknown_recovery_budget_exhausted")
    } else {
        None
    };
    Ok(BudgetDecision {
        allowed: reason_code.is_none(),
        ledger_metadata: json!({"budgetPurpose":purpose.as_str(),"executionDay":execution_day,
            "unknownRecovery":unknown_recovery,"dayTokenLimit":policy.day_token_limit,
            "purposeTokenLimit":purpose_limit}),
        execution_day,
        reason_code,
    })
}
