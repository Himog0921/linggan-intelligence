//! Field supplements are visible in the original run's request inspector. Content reads
//! enforce the same source, domain, retention and recording policy as the actual dispatch.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(crate) async fn calls(db: &Database, batch: Uuid) -> Result<Vec<Value>, ModelError> {
    Ok(sqlx::query_scalar(r#"SELECT jsonb_build_object(
      'invocationRef',v.invocation_ref,'repairRef',r.repair_ref,'purpose','field_repair',
      'state',CASE r.state WHEN 'succeeded_partial' THEN 'partial' ELSE r.state END,'failureCode',r.failure_code,
      'inputTokens',v.input_tokens,'outputTokens',v.output_tokens,'reservedTokens',v.reserved_tokens,'chargedTokens',v.charged_tokens,
      'usageUnknown',v.input_tokens IS NULL OR v.output_tokens IS NULL,'callStarted',v.result->'callStarted',
      'startedAt',v.created_at,'finishedAt',v.finished_at,'elapsedMs',v.elapsed_ms,
      'requestedComments',1,'acceptedComments',CASE WHEN r.supplement_analysis_ref IS NULL THEN 0 ELSE 1 END,
      'requestedFields',jsonb_array_length(r.requested_fields),'remainingRejectedFields',r.result_manifest->'remainingRejectedFields',
      'sourceRefs',jsonb_build_array(r.source_ref),'workRef',s.work_ref,'workTitle',s.work_title,
      'modelId',model.model_id,'diagnostic',COALESCE(v.result#>'{safe,diagnostic}',v.result->'diagnostic'),
      'inputLimit',cfg.input_token_limit,'outputLimit',cfg.output_token_limit)
      FROM linggan_comment_field_repair r JOIN linggan_model_invocation v USING(invocation_ref)
      JOIN linggan_model_config cfg USING(config_ref) JOIN linggan_model_entry model ON model.model_ref=cfg.model_ref
      LEFT JOIN linggan_ci_source s ON s.source_ref=r.source_ref
      WHERE EXISTS(SELECT 1 FROM linggan_comment_daily_item i WHERE i.batch_ref=$1 AND i.semantic_ref=r.semantic_ref)
      ORDER BY v.created_at DESC,v.invocation_ref LIMIT 100"#).bind(batch).fetch_all(db.pool()).await?)
}

pub(crate) async fn count(db: &Database, batch: Uuid) -> Result<i64, ModelError> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM linggan_comment_field_repair r WHERE invocation_ref IS NOT NULL AND EXISTS(SELECT 1 FROM linggan_comment_daily_item i WHERE i.batch_ref=$1 AND i.semantic_ref=r.semantic_ref)")
        .bind(batch).fetch_one(db.pool()).await?)
}

pub(crate) async fn detail(
    db: &Database,
    batch: Uuid,
    invocation: Uuid,
    domain: Uuid,
) -> Result<Value, ModelError> {
    let row=sqlx::query(r#"SELECT t.*,t.expires_at>scope_001_now() AS fresh,
      linggan_ci_analysis_context_readable(t.context_guard) AS readable,r.state,
      (t.purged_at IS NULL) AS unpurged,r.started_at,r.finished_at,r.requested_fields,r.base_analysis_ref,r.supplement_analysis_ref
      FROM linggan_comment_field_repair r JOIN linggan_comment_field_repair_trace t USING(repair_ref)
      JOIN linggan_material_comment source ON source.material_ref=r.source_ref
      JOIN linggan_material_content work ON work.public_ref=source.content_public_ref
      WHERE r.invocation_ref=$2 AND work.domain_ref=$3
        AND EXISTS(SELECT 1 FROM linggan_comment_daily_item i WHERE i.batch_ref=$1 AND i.semantic_ref=r.semantic_ref)
    "#).bind(batch).bind(invocation).bind(domain).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    let retained: Option<Value> = row.get("input_content");
    let available = row.get::<bool, _>("fresh")
        && row.get::<bool, _>("readable")
        && row.get::<bool, _>("unpurged");
    let availability = if !row.get::<bool, _>("readable") {
        "RESTRICTED"
    } else if !row.get::<bool, _>("fresh") {
        "EXPIRED"
    } else if retained.is_none() {
        "NOT_RECORDED"
    } else {
        "AVAILABLE"
    };
    let input=retained.filter(|_|available).map(|retained|{
        let prompt:Value=retained["prompt"].as_str().and_then(|p|serde_json::from_str(p).ok()).unwrap_or(Value::Null);
        json!({"contract":"comment-field-repair.v1","system":retained["system"],"policy":retained["policy"],
          "selectedFields":retained["selectedFields"],"comments":[prompt["untrustedMaterial"]["comment"]],"outputSchema":prompt["outputSchema"]})
    });
    let output: Option<String> = row.get("output_content");
    Ok(
        json!({"purpose":"field_repair","availability":availability,"input":input,
      "output":output.filter(|_|available).map(|text|json!({"text":text,"truncated":false})),
      "validation":if available{row.get::<Value,_>("validation")}else{json!([])},
      "repairReceipt":{"baseAnalysisRef":row.get::<Uuid,_>("base_analysis_ref"),"supplementAnalysisRef":row.get::<Option<Uuid>,_>("supplement_analysis_ref"),"state":row.get::<String,_>("state"),"fields":row.get::<Value,_>("requested_fields")},
      "events":[],"inputHash":row.get::<String,_>("input_hash"),"outputHash":row.get::<Option<String>,_>("output_hash")}),
    )
}
