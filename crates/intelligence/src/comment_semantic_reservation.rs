//! Frozen-rule loading and transactional semantic ownership shared by execution lanes.
use crate::{
    comment_daily::AutoPolicy,
    comment_packet::ResearchPacket,
    comment_research_fingerprint::{
        self as fingerprint, CleanState, EligibilityCategory, EligibilityRequest, RecoveryPolicy,
        ResultState, RuleFingerprint, SourceIdentity, SourceQualification,
    },
    comment_research_rules::{self, RuleSnapshot},
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(crate) async fn frozen_rule(
    db: &Database,
    request: &Value,
) -> Result<RuleSnapshot, ModelError> {
    if let Some(reference) = request["ruleRevisionRef"].as_str() {
        let rule = comment_research_rules::read_rule(
            db,
            Uuid::parse_str(reference).map_err(|_| ModelError::Invalid)?,
        )
        .await?;
        if request["ruleHash"].as_str() != Some(rule.canonical_hash.as_str()) {
            return Err(ModelError::Conflict);
        }
        Ok(rule)
    } else if request["ruleVersion"] == "comment-research.v4" {
        Ok(comment_research_rules::builtin_v4())
    } else {
        Err(ModelError::Invalid)
    }
}
pub(crate) struct SemanticReservations {
    pub execute_refs: Vec<Uuid>,
    pub semantics: Vec<Uuid>,
    pub fingerprints: Vec<String>,
    pub unknown_recovery: bool,
}
pub(crate) async fn reserve_semantics(
    conn: &mut PgConnection,
    research: &ResearchPacket,
    config: Uuid,
    batch: Uuid,
    request: &Value,
    max_attempts: i32,
) -> Result<SemanticReservations, ModelError> {
    let workspace =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(&mut *conn)
            .await?;
    let configured: Option<Value> = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_optional(&mut *conn)
    .await?;
    let policy = configured
        .map(AutoPolicy::parse)
        .transpose()?
        .unwrap_or_default();
    let recovery = RecoveryPolicy {
        retry_max_attempts: max_attempts,
        unknown_retry_max_attempts: policy.unknown_retry_max_attempts,
        retry_cooldown_seconds: 60,
    };
    let mut reserved = SemanticReservations {
        execute_refs: vec![],
        semantics: vec![],
        fingerprints: vec![],
        unknown_recovery: false,
    };
    for (input, cleaned) in research.inputs.iter().zip(&research.cleaned) {
        let source = sqlx::query("SELECT content_public_ref,comment_external_id FROM linggan_material_comment WHERE material_ref=$1")
            .bind(input.source_ref).fetch_one(&mut *conn).await?;
        let semantic_input = fingerprint::build_semantic_input(
            input,
            &SourceIdentity {
                work_ref: source.get("content_public_ref"),
                comment_external_id: source.get("comment_external_id"),
            },
            &RuleFingerprint::from(&research.rule_snapshot),
        );
        let mut eligibility = fingerprint::evaluate_eligibility_in(
            conn,
            &EligibilityRequest {
                workspace_ref: workspace,
                source_ref: input.source_ref,
                input: semantic_input,
                qualification: SourceQualification::eligible(if cleaned.state == "context" {
                    CleanState::Context
                } else {
                    CleanState::Direct
                }),
                explicit_generation: (request["reanalyze"] == true).then_some(batch),
                recovery: recovery.clone(),
            },
        )
        .await?;
        if !eligibility.eligible_to_dispatch {
            let state = match eligibility.category {
                EligibilityCategory::Reusable => {
                    if eligibility.result_state == ResultState::NoSignal {
                        "no_signal"
                    } else {
                        "succeeded"
                    }
                }
                EligibilityCategory::InProgress => "running",
                EligibilityCategory::ContextMissing => "context_missing",
                EligibilityCategory::Anomaly => "anomaly",
                _ => "failed",
            };
            sqlx::query("UPDATE linggan_comment_daily_item SET semantic_ref=$3,state=$4,analysis_ref=COALESCE($5,analysis_ref),failure_code=$6 WHERE batch_ref=$1 AND source_ref=$2 AND state='pending'")
                .bind(batch).bind(input.source_ref).bind(eligibility.semantic_ref).bind(state)
                .bind(eligibility.last_accepted_analysis_ref)
                .bind(if matches!(state,"succeeded"|"no_signal"|"running"){None}else{Some(eligibility.reason_code)})
                .execute(&mut *conn).await?;
            continue;
        }
        eligibility.input_manifest["baseFingerprint"] =
            serde_json::json!(eligibility.base_fingerprint);
        let unknown = eligibility
            .recovery
            .as_ref()
            .is_some_and(|r| r.unknown_charge);
        let semantic = eligibility.semantic_ref.unwrap_or_else(Uuid::new_v4);
        if eligibility.semantic_ref.is_some() {
            sqlx::query("UPDATE linggan_comment_semantic_work SET state='running',attempts=attempts+1,unknown_retry_attempts=unknown_retry_attempts+$2,failure_code=NULL,input_manifest=$3,updated_at=scope_001_now() WHERE semantic_ref=$1")
                .bind(semantic).bind(i32::from(unknown)).bind(&eligibility.input_manifest).execute(&mut *conn).await?;
        } else {
            sqlx::query("INSERT INTO linggan_comment_semantic_work(semantic_ref,workspace_ref,identity_key,fingerprint,source_ref,config_ref,state,attempts,input_manifest) VALUES($1,$2,$3,$4,$5,$6,'running',1,$7)")
                .bind(semantic).bind(workspace).bind(&eligibility.source_identity).bind(&eligibility.fingerprint)
                .bind(input.source_ref).bind(config).bind(&eligibility.input_manifest).execute(&mut *conn).await?;
        }
        sqlx::query("UPDATE linggan_comment_daily_item SET semantic_ref=$3 WHERE batch_ref=$1 AND source_ref=$2")
            .bind(batch).bind(input.source_ref).bind(semantic).execute(&mut *conn).await?;
        reserved.unknown_recovery |= unknown;
        reserved.execute_refs.push(input.source_ref);
        reserved.semantics.push(semantic);
        reserved.fingerprints.push(eligibility.fingerprint);
    }
    Ok(reserved)
}
