//! Research views are recomputed from receipts and current source qualification, never model counts.
use crate::{comment_cleaning::CLEANER_VERSION, model_settings::ModelError};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub async fn overview(db: &Database) -> Result<Value, ModelError> {
    let schedule: Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) FROM linggan_comment_daily_schedule s WHERE singleton",
    )
    .fetch_one(db.pool())
    .await?;
    let rows=sqlx::query("SELECT b.*,COALESCE((SELECT (a.request->>'sourceLimit')::integer FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.source_limit) AS effective_source_limit,COALESCE((SELECT (a.request->>'tokenLimit')::bigint FROM linggan_comment_daily_adjustment a WHERE a.batch_ref=COALESCE((b.request->>'originBatchRef')::uuid,b.batch_ref) AND a.kind='continue' ORDER BY a.created_at DESC,a.command_ref DESC LIMIT 1),b.token_limit) AS effective_token_limit,(SELECT input_token_limit+output_token_limit FROM linggan_model_config WHERE config_ref=b.config_ref) AS next_reservation,b.window_start::text AS start_text,b.window_end::text AS end_text,(SELECT count(*) FROM linggan_comment_daily_item i WHERE i.batch_ref=b.batch_ref) AS total,(SELECT count(DISTINCT s.content_public_ref) FROM linggan_comment_daily_item i JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref WHERE i.batch_ref=b.batch_ref) AS works,COALESCE((SELECT jsonb_object_agg(state,n) FROM (SELECT state,count(*) n FROM linggan_comment_daily_item WHERE batch_ref=b.batch_ref GROUP BY state) counts),'{}'::jsonb) AS counts,COALESCE((SELECT sum(v.charged_tokens) FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref) JOIN linggan_comment_daily_batch family ON family.batch_ref=p.batch_ref WHERE COALESCE(family.request->>'originBatchRef',family.batch_ref::text)=COALESCE(b.request->>'originBatchRef',b.batch_ref::text)),0)::bigint AS charged,COALESCE((SELECT jsonb_object_agg(state,n) FROM (SELECT COALESCE(c.state,'pending') AS state,count(*) n FROM linggan_comment_daily_item i LEFT JOIN linggan_comment_clean c ON c.source_ref=i.source_ref AND c.cleaner_version=COALESCE(b.request->>'cleanerVersion','comment-clean.v1') WHERE i.batch_ref=b.batch_ref GROUP BY COALESCE(c.state,'pending')) clean_counts),'{}'::jsonb) AS cleaning FROM linggan_comment_daily_batch b ORDER BY b.window_end DESC,b.created_at DESC LIMIT 60").fetch_all(db.pool()).await?;
    let items:Vec<_>=rows.iter().map(|r|json!({"batchRef":r.get::<Uuid,_>("batch_ref"),"kind":r.get::<String,_>("kind"),"originBatchRef":r.get::<Value,_>("request")["originBatchRef"],"newIntakeCount":r.get::<Value,_>("request")["newIntakeCount"],"backlogCount":r.get::<Value,_>("request")["backlogCount"],"enabled":r.get::<bool,_>("enabled"),"start":r.get::<String,_>("start_text"),"end":r.get::<String,_>("end_text"),"total":r.get::<i64,_>("total"),"works":r.get::<i64,_>("works"),"counts":r.get::<Value,_>("counts"),"cleaning":r.get::<Value,_>("cleaning"),"nextReservation":r.get::<i32,_>("next_reservation"),"chargedTokens":r.get::<i64,_>("charged"),"tokenLimit":r.get::<i64,_>("effective_token_limit"),"sourceLimit":r.get::<i32,_>("effective_source_limit")})).collect();
    Ok(json!({"schedule":schedule,"items":items,"limit":60,"timezone":"Asia/Shanghai"}))
}
pub async fn batch_detail(
    db: &Database,
    batch: Uuid,
    after: Option<Uuid>,
) -> Result<Value, ModelError> {
    let cleaner: String = sqlx::query_scalar("SELECT COALESCE(request->>'cleanerVersion','comment-clean.v1') FROM linggan_comment_daily_batch WHERE batch_ref=$1").bind(batch).fetch_one(db.pool()).await?;
    let rows=sqlx::query("SELECT i.*,s.content_public_ref,s.body_text,c.state AS clean_state,c.result AS cleaning,a.result AS analysis FROM linggan_comment_daily_item i LEFT JOIN linggan_comment_research_readable s ON s.material_ref=i.source_ref LEFT JOIN linggan_comment_clean c ON c.source_ref=s.material_ref AND c.cleaner_version=$3 LEFT JOIN linggan_comment_analysis_work a ON a.work_ref=i.analysis_ref WHERE i.batch_ref=$1 AND ($2::uuid IS NULL OR i.source_ref>$2) ORDER BY i.source_ref LIMIT 51").bind(batch).bind(after).bind(cleaner).fetch_all(db.pool()).await?;
    let mut items = vec![];
    for r in rows.iter().take(50) {
        let readable = r.get::<Option<Uuid>, _>("content_public_ref").is_some();
        // Source restrictions in any jointly supplied comment invalidate the derived packet output.
        let mut analysis = r.get::<Option<Value>, _>("analysis");
        if let Some(value) = &analysis {
            if !context_readable(db, value).await? {
                analysis = None;
            }
        }
        items.push(json!({"sourceRef":r.get::<Uuid,_>("source_ref"),"originBatchRef":r.get::<Option<Uuid>,_>("origin_batch_ref"),"workRef":r.get::<Option<Uuid>,_>("content_public_ref"),"body":r.get::<Option<String>,_>("body_text").map(|s|s.chars().take(160).collect::<String>()),"state":if readable{r.get::<String,_>("state")}else{"restricted".into()},"failureCode":r.get::<Option<String>,_>("failure_code"),"cleanState":r.get::<Option<String>,_>("clean_state"),"cleaningReasons":r.get::<Option<Value>,_>("cleaning").and_then(|v|v.get("reasons").cloned()),"analysis":if readable{analysis}else{None}}));
    }
    let mut calls:Vec<Value>=sqlx::query_scalar(r#"SELECT jsonb_build_object(
      'invocationRef',v.invocation_ref,'packetRef',p.packet_ref,'purpose',p.purpose,'state',p.state,'failureCode',v.failure_code,
      'inputTokens',v.input_tokens,'outputTokens',v.output_tokens,'reservedTokens',v.reserved_tokens,'chargedTokens',v.charged_tokens,
      'usageUnknown',v.input_tokens IS NULL OR v.output_tokens IS NULL,'callStarted',v.result->'callStarted','review',v.result->'usageReviewRef',
      'startedAt',v.created_at,'finishedAt',v.finished_at,'elapsedMs',COALESCE(v.elapsed_ms,(extract(epoch FROM v.finished_at-v.created_at)*1000)::bigint),
      'requestedComments',cardinality(p.source_refs),'acceptedComments',v.result->'acceptedComments',
      'succeededComments',CASE WHEN t.outcomes<>'{}'::jsonb THEN COALESCE(t.outcomes->'succeeded','0'::jsonb) END,
      'noSignalComments',CASE WHEN t.outcomes<>'{}'::jsonb THEN COALESCE(t.outcomes->'no_signal','0'::jsonb) END,
      'failedComments',CASE WHEN t.outcomes<>'{}'::jsonb THEN COALESCE(t.outcomes->'failed','0'::jsonb) END,
      'sourceRefs',p.source_refs,'workRef',s.work_ref,'workTitle',s.work_title,'modelId',m.model_id,'contextPolicy',b.context_policy,
      'inputLimit',cfg.input_token_limit,'outputLimit',cfg.output_token_limit)
      FROM linggan_comment_daily_packet p JOIN linggan_model_invocation v USING(invocation_ref)
      JOIN linggan_comment_daily_batch b USING(batch_ref) JOIN linggan_model_config cfg ON cfg.config_ref=b.config_ref
      JOIN linggan_model_entry m ON m.model_ref=cfg.model_ref LEFT JOIN linggan_comment_request_trace t USING(invocation_ref)
      LEFT JOIN linggan_ci_source s ON s.source_ref=p.source_refs[1]
      WHERE p.batch_ref=$1 ORDER BY v.created_at DESC,v.invocation_ref LIMIT 100"#).bind(batch).fetch_all(db.pool()).await?;
    let work_refs: Vec<Uuid> = calls
        .iter()
        .filter_map(|c| c["workRef"].as_str().and_then(|v| Uuid::parse_str(v).ok()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    for chunk in work_refs.chunks(20) {
        let works =
            linggan_evidence::comment_research_read::read_comment_work_contexts(db, chunk).await?;
        for w in works.as_array().into_iter().flatten() {
            for call in &mut calls {
                if call["workRef"] == w["workRef"] {
                    call["workTitle"] = w["title"].clone();
                }
            }
        }
    }
    let call_total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_daily_packet WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(db.pool())
            .await?;
    let adjustments:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('commandRef',command_ref,'kind',kind,'request',request,'createdAt',created_at) FROM linggan_comment_daily_adjustment WHERE batch_ref=$1 ORDER BY created_at DESC LIMIT 100").bind(batch).fetch_all(db.pool()).await?;
    Ok(
        json!({"items":items,"calls":calls,"callTotal":call_total,"adjustments":adjustments,"nextCursor":if rows.len()>50{rows.get(49).map(|r|r.get::<Uuid,_>("source_ref"))}else{None}}),
    )
}
pub async fn context_readable(db: &Database, result: &Value) -> Result<bool, ModelError> {
    let refs = match result.pointer("/contextRefs/researchSourceRefs") {
        None => Vec::new(), // Historical v1 results use their separate parent source check.
        Some(Value::Array(values)) => {
            let parsed: Option<Vec<Uuid>> = values
                .iter()
                .map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
                .collect();
            let Some(refs) = parsed else {
                return Ok(false);
            };
            if refs.is_empty() {
                return Ok(false);
            }
            refs
        }
        Some(_) => return Ok(false),
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_research_readable WHERE material_ref=ANY($1)",
    )
    .bind(&refs)
    .fetch_one(db.pool())
    .await?;
    if count != refs.len() as i64 {
        return Ok(false);
    }
    let jobs = result
        .pointer("/contextRefs/mediaJobs")
        .and_then(Value::as_array);
    if let Some(jobs) = jobs.filter(|j| !j.is_empty()) {
        let work = result
            .pointer("/contextRefs/workRef")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or(ModelError::Source)?;
        let current = linggan_evidence::read_work_resource(db, work)
            .await
            .map_err(|_| ModelError::Source)?;
        let available = current
            .as_ref()
            .and_then(|w| w.inspector.get("derivatives"))
            .and_then(Value::as_array);
        if !jobs.iter().all(|job| {
            available.is_some_and(|a| {
                a.iter().any(|d| {
                    d["jobRef"] == *job && d["state"] == "ACQUIRED" && d["displayText"].is_string()
                })
            })
        }) {
            return Ok(false);
        }
    }
    Ok(true)
}
pub async fn source_states(db: &Database, refs: &[Uuid]) -> Result<Value, ModelError> {
    let rows=sqlx::query("SELECT s.material_ref,c.state,c.result,COALESCE((SELECT i.state FROM linggan_comment_daily_item i JOIN linggan_comment_daily_batch b USING(batch_ref) WHERE i.source_ref=s.material_ref ORDER BY b.created_at DESC,b.batch_ref DESC LIMIT 1),(SELECT state FROM linggan_comment_analysis_work a WHERE a.source_ref=s.material_ref ORDER BY created_at DESC,work_ref DESC LIMIT 1)) AS analysis_state FROM linggan_comment_research_readable s LEFT JOIN linggan_comment_clean c ON c.source_ref=s.material_ref AND c.cleaner_version=$2 WHERE s.material_ref=ANY($1)").bind(refs).bind(CLEANER_VERSION).fetch_all(db.pool()).await?;
    Ok(json!(rows.iter().map(|r|json!({"sourceRef":r.get::<Uuid,_>("material_ref"),"cleanState":r.get::<Option<String>,_>("state"),"cleaning":r.get::<Option<Value>,_>("result").map(|v|json!({"text":v["text"].as_str().map(|s|s.chars().take(4000).collect::<String>()),"reasons":v["reasons"]})),"analysisState":r.get::<Option<String>,_>("analysis_state")})).collect::<Vec<_>>()))
}

pub async fn cleaning_members(db: &Database, state: &str) -> Result<Vec<Uuid>, ModelError> {
    if ![
        "pending",
        "direct",
        "context",
        "low_information",
        "anomaly",
        "dropped",
    ]
    .contains(&state)
    {
        return Err(ModelError::Invalid);
    }
    Ok(sqlx::query_scalar("SELECT s.material_ref FROM linggan_comment_research_readable s LEFT JOIN linggan_comment_clean c ON c.source_ref=s.material_ref AND c.cleaner_version=$2 WHERE COALESCE(c.state,'pending')=$1").bind(state).bind(CLEANER_VERSION).fetch_all(db.pool()).await?)
}
