//! Human-facing operational summary. Counts are facts from current sources and the shared
//! execution ledger, separate from the user's temporary browse filters.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
pub(crate) async fn read(db: &Database) -> Result<Value, ModelError> {
    let schedule:Value=sqlx::query_scalar("SELECT jsonb_build_object('enabled',enabled,'policy',auto_policy,'firstEnabledAt',auto_enabled_at,'legacyActivationUnknown',legacy_activation_unknown,'nextCutoff',next_end,'revision',revision,'sourceLimit',source_limit) FROM linggan_comment_daily_schedule WHERE singleton").fetch_one(db.pool()).await?;
    let queue:Value=sqlx::query_scalar(r#"SELECT '{"new_intake":0,"recovery":0,"historical":0}'::jsonb || COALESCE(jsonb_object_agg(queue_class,n),'{}') FROM (
      SELECT CASE priority WHEN 0 THEN 'new_intake' WHEN 1 THEN 'recovery' ELSE 'historical' END queue_class,count(*) n FROM (
        SELECT s.canonical_ref,min(CASE COALESCE(i.queue_class,CASE WHEN i.attempts>0 THEN 'recovery' ELSE 'new_intake' END) WHEN 'new_intake' THEN 0 WHEN 'recovery' THEN 1 ELSE 2 END) priority
        FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref)
        JOIN linggan_ci_source s ON s.source_ref=i.source_ref JOIN observation_domain d USING(domain_ref)
        WHERE d.is_own_domain AND b.enabled AND i.state IN('pending','running','source_limit') GROUP BY s.canonical_ref
      ) identities GROUP BY priority) q"#).fetch_one(db.pool()).await?;
    let usage:Value=sqlx::query_scalar(r#"SELECT jsonb_build_object('day',(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date,
      'admittedComments',(SELECT count(*) FROM linggan_comment_execution_day_sources()),'calls',count(*),'chargedTokens',COALESCE(sum(charged_tokens),0),'unknownUsageCalls',count(*) FILTER(WHERE (input_tokens IS NULL OR output_tokens IS NULL) AND result->>'callStarted' IS DISTINCT FROM 'false'),
      'replayTokens',COALESCE(sum(charged_tokens) FILTER(WHERE result->>'budgetPurpose'='replay'),0),
      'semanticTokens',COALESCE(sum(charged_tokens) FILTER(WHERE result->>'budgetPurpose'='semantic'),0),
      'unknownRecoveryTokens',COALESCE(sum(charged_tokens) FILTER(WHERE result->>'unknownRecovery'='true'),0))
      FROM linggan_model_invocation WHERE operation IN('analyze','embed')
      AND created_at>=date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai'
      AND created_at<(date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai')+interval '1 day') AT TIME ZONE 'Asia/Shanghai'"#).fetch_one(db.pool()).await?;
    let historical: i64=sqlx::query_scalar(r#"SELECT count(*) FROM linggan_ci_source s JOIN observation_domain d USING(domain_ref)
      CROSS JOIN linggan_comment_daily_schedule schedule LEFT JOIN linggan_comment_research_eligibility_current e ON e.source_ref=s.source_ref
      WHERE d.is_own_domain AND schedule.singleton AND s.first_observed_at<schedule.auto_enabled_at
        AND e.result_state='unstudied' AND e.eligible_to_dispatch
    "#).fetch_one(db.pool()).await?;
    Ok(
        json!({"schedule":schedule,"queue":queue,"usage":usage,"unresearchedHistory":if schedule["firstEnabledAt"].is_null(){Value::Null}else{json!(historical)},
      "scope":"entire_own_domain","accounting":"实际Token已知时使用实际值；未知用量保留原预留。",
      "historyNote":"历史补齐依剩余日额度推进，不表示今天新出现的用户需求。"}),
    )
}
