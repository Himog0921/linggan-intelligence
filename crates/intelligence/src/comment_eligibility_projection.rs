//! Read model of common eligibility. A broad dependency stamp invalidates cached projections;
//! only the exact production selector decides whether a model call is actually necessary.
use crate::{
    comment_daily::AutoPolicy,
    comment_packet::build_packet_with_rule,
    comment_research_fingerprint::{
        self as fingerprint, CleanState, EligibilityRequest, RecoveryPolicy, ResultState,
        RuleFingerprint, SourceIdentity, SourceQualification,
    },
    comment_research_rules::read_active_rule,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn refresh(
    db: &Database,
    refs: Option<&[Uuid]>,
    limit: i64,
) -> Result<usize, ModelError> {
    let refs: Option<Vec<Uuid>> = refs.map(<[Uuid]>::to_vec);
    let rule = read_active_rule(db).await?;
    let settings = crate::comment_runtime::settings(db).await?;
    let policy = crate::comment_runtime::ContextPolicy::parse(settings["policy"].clone())?;
    let workspace: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(db.pool())
            .await?;
    let auto: Option<Value> = sqlx::query_scalar(
        "SELECT auto_policy FROM linggan_comment_daily_schedule WHERE singleton",
    )
    .fetch_optional(db.pool())
    .await?;
    let auto = auto.map(AutoPolicy::parse).transpose()?.unwrap_or_default();
    let attempts:Option<i32>=sqlx::query_scalar("SELECT cfg.max_attempts FROM linggan_model_workspace ws JOIN linggan_model_config cfg ON cfg.config_ref=ws.default_config_ref WHERE ws.singleton").fetch_optional(db.pool()).await?;
    sqlx::query("DELETE FROM linggan_comment_research_eligibility_current e WHERE NOT EXISTS(SELECT 1 FROM linggan_comment_research_readable r WHERE r.material_ref=e.source_ref)").execute(db.pool()).await?;
    let rows=sqlx::query(r#"
      SELECT s.source_ref,s.work_ref,s.comment_external_id,s.source_sha256,
        linggan_comment_research_context_revision(s.source_ref) AS context_revision
      FROM linggan_ci_source s JOIN observation_domain d USING(domain_ref)
      LEFT JOIN linggan_comment_research_eligibility_current e
        ON e.source_identity=encode(sha256(convert_to(s.work_ref::text||':'||s.comment_external_id,'UTF8')),'hex')
      WHERE d.is_own_domain AND s.body IS NOT NULL
        AND ($1::uuid[] IS NULL OR s.source_ref=ANY($1))
      ORDER BY (e.source_ref IS DISTINCT FROM s.source_ref OR e.source_sha256 IS DISTINCT FROM s.source_sha256
          OR e.rule_hash IS DISTINCT FROM $2 OR e.source_context_revision IS DISTINCT FROM linggan_comment_research_context_revision(s.source_ref)) DESC,
        e.checked_at NULLS FIRST,s.first_observed_at,s.canonical_ref LIMIT $3
    "#).bind(refs).bind(&rule.canonical_hash).bind(limit.clamp(1,500)).fetch_all(db.pool()).await?;
    let mut done = 0;
    for row in rows {
        let source: Uuid = row.get("source_ref");
        let packet = match build_packet_with_rule(
            db,
            &[source],
            "eligibility-no-call",
            &policy,
            &rule,
        )
        .await
        {
            Ok(p) => p,
            Err(ModelError::Source | ModelError::NotFound) => continue,
            Err(e) => return Err(e),
        };
        let input = &packet.inputs[0];
        let qualification = packet_qualification(&packet);
        let eligibility = fingerprint::evaluate_eligibility(
            db,
            &EligibilityRequest {
                workspace_ref: workspace,
                source_ref: source,
                input: fingerprint::build_semantic_input(
                    input,
                    &SourceIdentity {
                        work_ref: row.get("work_ref"),
                        comment_external_id: row.get("comment_external_id"),
                    },
                    &RuleFingerprint::from(&rule),
                ),
                qualification,
                explicit_generation: None,
                recovery: RecoveryPolicy {
                    retry_max_attempts: attempts.unwrap_or(3),
                    unknown_retry_max_attempts: auto.unknown_retry_max_attempts,
                    retry_cooldown_seconds: 60,
                },
            },
        )
        .await?;
        let current = if matches!(
            eligibility.result_state,
            ResultState::Studied | ResultState::NoSignal
        ) {
            eligibility.last_accepted_analysis_ref
        } else {
            None
        };
        // Stamp from before the builder. A concurrent dependency change makes this projection
        // immediately stale instead of blessing a result built against older context.
        sqlx::query(r#"INSERT INTO linggan_comment_research_eligibility_current
          (source_identity,source_ref,source_sha256,fingerprint,rule_revision_ref,rule_hash,schema_version,selector_version,source_context_revision,
           current_analysis_ref,last_accepted_analysis_ref,current_attempt_ref,semantic_ref,result_state,execution_state,reason_code,eligible_to_dispatch,input_manifest)
          VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)
          ON CONFLICT(source_identity) DO UPDATE SET source_ref=EXCLUDED.source_ref,source_sha256=EXCLUDED.source_sha256,
           fingerprint=EXCLUDED.fingerprint,rule_revision_ref=EXCLUDED.rule_revision_ref,rule_hash=EXCLUDED.rule_hash,schema_version=EXCLUDED.schema_version,
           selector_version=EXCLUDED.selector_version,source_context_revision=EXCLUDED.source_context_revision,current_analysis_ref=EXCLUDED.current_analysis_ref,
           last_accepted_analysis_ref=EXCLUDED.last_accepted_analysis_ref,current_attempt_ref=EXCLUDED.current_attempt_ref,semantic_ref=EXCLUDED.semantic_ref,
           result_state=EXCLUDED.result_state,execution_state=EXCLUDED.execution_state,reason_code=EXCLUDED.reason_code,
           eligible_to_dispatch=EXCLUDED.eligible_to_dispatch,input_manifest=EXCLUDED.input_manifest,checked_at=scope_001_now()"#)
          .bind(&eligibility.source_identity).bind(source).bind(&input.source_sha256).bind(&eligibility.fingerprint)
          .bind(rule.rule_revision_ref).bind(&rule.canonical_hash).bind(&rule.schema_version).bind(&rule.selector_version).bind(row.get::<String,_>("context_revision"))
          .bind(current).bind(eligibility.last_accepted_analysis_ref).bind(eligibility.current_attempt_ref).bind(eligibility.semantic_ref)
          .bind(json!(eligibility.result_state).as_str().ok_or(ModelError::Invalid)?.to_owned())
          .bind(json!(eligibility.execution_state).as_str().ok_or(ModelError::Invalid)?.to_owned())
          .bind(&eligibility.reason_code).bind(eligibility.eligible_to_dispatch).bind(eligibility.input_manifest).execute(db.pool()).await?;
        done += 1;
    }
    Ok(done)
}

fn packet_qualification(packet: &crate::comment_packet::ResearchPacket) -> SourceQualification {
    let state = match packet.cleaned[0].state.as_str() {
        "direct" => CleanState::Direct,
        "context" => CleanState::Context,
        "dropped" => CleanState::Dropped,
        "anomaly" => CleanState::Anomaly,
        _ => CleanState::LowInformation,
    };
    let mut qualification = SourceQualification::eligible(state);
    qualification.context_missing =
        !crate::comment_daily_runner::missing_context(&packet).is_empty();
    qualification
}
