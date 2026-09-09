//! Admit real pre-enablement history and changed semantic inputs under explicit automatic
//! policy. Manifests share the existing queue, semantic owner and execution-day budget.
use crate::{
    comment_daily::{AutoPolicy, OutdatedPolicy},
    comment_research_rules::read_active_rule_in,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub(crate) async fn seal(db: &Database) -> Result<bool, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let Some(schedule)=sqlx::query("SELECT *,auto_enabled_at::text AS enabled_text FROM linggan_comment_daily_schedule WHERE singleton AND enabled AND auto_enabled_at IS NOT NULL").fetch_optional(&mut *tx).await?else{return Ok(false)};
    let auto: Value = schedule.get("auto_policy");
    let policy = AutoPolicy::parse(auto.clone())?;
    if !policy.historical_enabled && policy.outdated_policy == OutdatedPolicy::Disabled {
        return Ok(false);
    }
    let rule = read_active_rule_in(&mut tx).await?;
    let historical_updates = policy.outdated_policy == OutdatedPolicy::Historical;
    let update_enabled = policy.outdated_policy != OutdatedPolicy::Disabled;
    let rows=sqlx::query(r#"SELECT s.source_ref,e.fingerprint,
       CASE WHEN e.result_state='outdated' THEN 'recovery' ELSE 'historical' END AS queue_class
      FROM linggan_ci_source s JOIN observation_domain d USING(domain_ref)
      JOIN linggan_comment_research_eligibility_current e ON e.source_ref=s.source_ref
      WHERE d.is_own_domain AND e.rule_hash=$1 AND e.eligible_to_dispatch
       AND e.source_context_revision=linggan_comment_research_context_revision(s.source_ref)
       AND ((e.result_state='unstudied' AND $2 AND s.first_observed_at>=$3::timestamptz AND s.first_observed_at<$4::timestamptz)
         OR (e.result_state='outdated' AND $5 AND ($6 OR s.first_observed_at>=$4::timestamptz)))
       AND NOT EXISTS(SELECT 1 FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref)
          WHERE i.source_ref=s.source_ref AND (i.state IN('pending','running','source_limit')
            OR b.request->'eligibilityFingerprints'->>s.source_ref::text=e.fingerprint))
      ORDER BY CASE WHEN e.result_state='outdated' THEN 0 ELSE 1 END,s.first_observed_at,s.canonical_ref LIMIT $7
    "#).bind(&rule.canonical_hash).bind(policy.historical_enabled).bind(&policy.history_start).bind(schedule.get::<String,_>("enabled_text"))
      .bind(update_enabled).bind(historical_updates).bind(schedule.get::<i32,_>("source_limit")).fetch_all(&mut *tx).await?;
    if rows.is_empty() {
        return Ok(false);
    }
    let refs: Vec<Uuid> = rows.iter().map(|r| r.get("source_ref")).collect();
    let fingerprints: serde_json::Map<String, Value> = rows
        .iter()
        .map(|r| {
            (
                r.get::<Uuid, _>("source_ref").to_string(),
                json!(r.get::<String, _>("fingerprint")),
            )
        })
        .collect();
    let batch = Uuid::new_v4();
    let request = json!({"ruleRevisionRef":rule.rule_revision_ref,"ruleHash":rule.canonical_hash,"ruleVersion":rule.rule_version.as_str(),"schemaVersion":rule.schema_version,"selectorVersion":rule.selector_version,
        "autoPolicy":auto,"automatic":true,"eligibilityFingerprints":fingerprints,"reason":"authorized_history_or_changed_input","scheduleRevision":schedule.get::<i32,_>("revision"),"newIntake":false});
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request,context_policy) VALUES($1,'backlog',$2,scope_001_now(),scope_001_now(),$3,$4,$5,(SELECT policy FROM linggan_comment_context_settings WHERE singleton))")
        .bind(batch).bind(schedule.get::<Uuid,_>("config_ref")).bind(refs.len() as i32).bind(policy.day_token_limit).bind(request).execute(&mut *tx).await?;
    for row in rows {
        sqlx::query("INSERT INTO linggan_comment_daily_item(batch_ref,source_ref,queue_class,execution_day) VALUES($1,$2,$3,(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date)")
            .bind(batch).bind(row.get::<Uuid,_>("source_ref")).bind(row.get::<String,_>("queue_class")).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Called under the workspace lock before semantic ownership or invocation reservation.
pub(crate) async fn admit_day_sources(
    conn: &mut sqlx::PgConnection,
    refs: &[Uuid],
) -> Result<Vec<Uuid>, ModelError> {
    let mut remaining:i64=sqlx::query_scalar("SELECT greatest(source_limit::bigint-(SELECT count(*) FROM linggan_comment_execution_day_sources()),0) FROM linggan_comment_daily_schedule WHERE singleton").fetch_one(&mut *conn).await?;
    let known:Vec<Uuid>=sqlx::query_scalar("SELECT source.material_ref FROM linggan_material_comment source JOIN linggan_comment_execution_day_sources() used ON used.work_ref=source.content_public_ref AND used.comment_external_id=source.comment_external_id WHERE source.material_ref=ANY($1)").bind(refs).fetch_all(&mut *conn).await?;
    Ok(refs
        .iter()
        .filter(|reference| {
            if known.contains(reference) {
                true
            } else if remaining > 0 {
                remaining -= 1;
                true
            } else {
                false
            }
        })
        .copied()
        .collect())
}
