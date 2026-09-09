//! Local-only projection and recovery. Reuse is independent of connection and token availability.
use crate::{
    comment_daily::AutoPolicy,
    comment_packet::build_packet_with_rule,
    comment_research_fingerprint::{
        self as fingerprint, CleanState, EligibilityCategory, EligibilityRequest, RecoveryPolicy,
        ResultState, RuleFingerprint, SourceIdentity, SourceQualification,
    },
    comment_semantic_reservation::frozen_rule,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn reconcile(db: &Database) -> Result<(), ModelError> {
    let policy: Option<Value> = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_optional(db.pool())
    .await?;
    let policy = policy
        .map(AutoPolicy::parse)
        .transpose()?
        .unwrap_or_default();
    let workspace =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await?;
    let rows = sqlx::query(r#"
        SELECT i.batch_ref,i.source_ref,b.request,b.context_policy,cfg.max_attempts,
               s.content_public_ref,s.comment_external_id
        FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref)
        JOIN linggan_model_config cfg USING(config_ref)
        JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref
        WHERE b.enabled AND (b.request ? 'ruleRevisionRef' OR b.request->>'ruleVersion'='comment-research.v4')
          AND (b.kind='selected' OR EXISTS(SELECT 1 FROM linggan_comment_daily_schedule WHERE singleton AND enabled))
          AND i.state IN ('pending','source_limit','failed')
        ORDER BY i.eligibility_checked_at NULLS FIRST,b.window_end,i.source_ref LIMIT 100
    "#).fetch_all(db.pool()).await?;
    for row in rows {
        let source: Uuid = row.get("source_ref");
        let batch: Uuid = row.get("batch_ref");
        let request: Value = row.get("request");
        sqlx::query("UPDATE linggan_comment_daily_item SET eligibility_checked_at=scope_001_now() WHERE batch_ref=$1 AND source_ref=$2")
            .bind(batch).bind(source).execute(db.pool()).await?;
        let rule = frozen_rule(db, &request).await?;
        let context = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
        let packet =
            build_packet_with_rule(db, &[source], "local-eligibility", &context, &rule).await?;
        let Some(input) = packet.inputs.first() else {
            continue;
        };
        let clean_state = match packet.cleaned[0].state.as_str() {
            "direct" => CleanState::Direct,
            "context" => CleanState::Context,
            "dropped" => CleanState::Dropped,
            "anomaly" => CleanState::Anomaly,
            _ => CleanState::LowInformation,
        };
        let mut qualification = SourceQualification::eligible(clean_state);
        qualification.context_missing =
            !crate::comment_daily_runner::missing_context(&packet).is_empty();
        let eligibility = fingerprint::evaluate_eligibility(
            db,
            &EligibilityRequest {
                workspace_ref: workspace,
                source_ref: source,
                input: fingerprint::build_semantic_input(
                    input,
                    &SourceIdentity {
                        work_ref: row.get("content_public_ref"),
                        comment_external_id: row.get("comment_external_id"),
                    },
                    &RuleFingerprint::from(&rule),
                ),
                qualification,
                explicit_generation: (request["reanalyze"] == true).then_some(batch),
                recovery: RecoveryPolicy {
                    retry_max_attempts: row.get("max_attempts"),
                    unknown_retry_max_attempts: policy.unknown_retry_max_attempts,
                    retry_cooldown_seconds: 60,
                },
            },
        )
        .await?;
        match eligibility.category {
            EligibilityCategory::Reusable => {
                sqlx::query("UPDATE linggan_comment_daily_item SET state=$3,analysis_ref=$4,semantic_ref=$5,failure_code=NULL WHERE batch_ref=$1 AND source_ref=$2 AND state IN('pending','source_limit','failed')")
                    .bind(batch).bind(source).bind(if eligibility.result_state==ResultState::NoSignal {"no_signal"}else{"succeeded"})
                    .bind(eligibility.last_accepted_analysis_ref).bind(eligibility.semantic_ref).execute(db.pool()).await?;
            }
            EligibilityCategory::FailedRecovery if eligibility.eligible_to_dispatch => {
                sqlx::query("UPDATE linggan_comment_daily_item SET state='pending',failure_code=NULL,queue_class='recovery',execution_day=(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date WHERE batch_ref=$1 AND source_ref=$2 AND state='failed'")
                    .bind(batch).bind(source).execute(db.pool()).await?;
            }
            _ => {}
        }
    }
    Ok(())
}
