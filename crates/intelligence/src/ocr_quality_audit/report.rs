//! Reports separate cloud opinions, human-confirmed points, failed calls and unknown usage.
use super::{review::HumanMetrics, source};
use crate::ai_foundation::store::FoundationError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub async fn detail(db: &Database, sample: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let row=sqlx::query("SELECT s.*,o.result,o.failure_code,o.vision_qualified,r.model_id,r.prompt_version,r.connection_version_ref FROM linggan_ocr_audit_sample s JOIN linggan_ocr_audit_run r USING(run_ref) LEFT JOIN linggan_ocr_audit_observation o USING(sample_ref) WHERE s.sample_ref=$1").bind(sample).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let mut result = json!({"sampleRef":sample,"runRef":row.get::<Uuid,_>("run_ref"),"pool":row.get::<String,_>("pool"),"modelId":row.get::<String,_>("model_id"),"promptVersion":row.get::<String,_>("prompt_version"),"visionQualified":row.get::<Option<bool>,_>("vision_qualified"),"failureCode":row.get::<Option<String>,_>("failure_code"),"modelOpinion":row.get::<Option<Value>,_>("result"),"localSnapshot":row.get::<Value,_>("local_snapshot")});
    if let Some(reading) = row.get::<Option<Uuid>, _>("reading_ref") {
        let s = source::load_in(&mut tx, reading)
            .await?
            .ok_or(FoundationError::NotFound)?;
        s.validate_snapshot(&row.get::<Value, _>("local_snapshot"))?;
        result["readingRef"] = json!(reading);
        result["workRef"] = json!(s.work_ref);
        result["imageRef"] = json!(s.slot_key);
        result["blobSha256"] = json!(s.image.blob_sha256);
        result["unitRef"] = json!(s.unit_ref);
        result["blocks"] = json!(s.blocks);
        result["readingIssues"] = json!(s.issues);
        result["originalAssetUrl"] = json!(s.original_url);
        let reviews:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY revision),'[]') FROM linggan_ocr_audit_review v WHERE sample_ref=$1").bind(sample).fetch_one(&mut *tx).await?;
        result["reviews"] = reviews;
    }
    let issues:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(v) ORDER BY position),'[]') FROM linggan_ocr_audit_issue v WHERE sample_ref=$1").bind(sample).fetch_one(&mut *tx).await?;
    result["issues"] = issues;
    Ok(result)
}
pub async fn latest_for_reading(db: &Database, id: Uuid) -> Result<Value, FoundationError> {
    let sample:Option<Uuid>=sqlx::query_scalar("SELECT sample_ref FROM linggan_ocr_audit_sample s WHERE reading_ref=$1 ORDER BY EXISTS(SELECT 1 FROM linggan_ocr_audit_observation o WHERE o.sample_ref=s.sample_ref AND o.result IS NOT NULL) DESC,created_at DESC LIMIT 1").bind(id).fetch_optional(db.pool()).await?;
    match sample {
        Some(s) => detail(db, s).await,
        None => Ok(json!({"sampleRef":null,"state":"not_sampled"})),
    }
}
pub async fn report(db: &Database, run: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let r = sqlx::query(
        "SELECT *,created_at::text AS created_text FROM linggan_ocr_audit_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(FoundationError::NotFound)?;
    let samples=sqlx::query("SELECT s.*,o.result IS NOT NULL AS observed,o.vision_qualified,COALESCE(o.failure_code,ctl.failure_code) AS failure_code,i.state AS invocation_state,i.input_tokens,i.output_tokens,i.cost_pico_usd::text AS cost_pico_usd,d.reserved_tokens,d.reserved_cost_micro_usd FROM linggan_ocr_audit_sample s LEFT JOIN linggan_ocr_audit_dispatch d USING(sample_ref) LEFT JOIN linggan_model_invocation i USING(invocation_ref) LEFT JOIN linggan_ocr_audit_observation o ON o.sample_ref=s.sample_ref LEFT JOIN linggan_ai_batch batch ON batch.command_ref=s.run_ref LEFT JOIN linggan_ai_batch_item bi ON bi.batch_ref=batch.batch_ref AND bi.item_key=s.sample_ref::text LEFT JOIN linggan_ai_batch_item_control ctl ON ctl.item_ref=bi.item_ref WHERE s.run_ref=$1 ORDER BY s.created_at,s.sample_ref").bind(run).fetch_all(&mut *tx).await?;
    let mut pools = vec![];
    let mut visible = vec![];
    for pool in ["random", "risk", "regression", "probe"] {
        let mut checked = 0;
        let mut differences = 0;
        let mut pending = 0;
        let mut reviewed_agreements = 0;
        let mut reviewed_disagreements = 0;
        let mut failures = 0;
        let mut withheld = 0;
        let mut actual = 0;
        let mut input = Some(0_i64);
        let mut output = Some(0_i64);
        let mut cost = Some(0_u128);
        let mut reserved = 0_i64;
        let mut metrics = serde_json::Map::new();
        for key in [
            "titleFalseAdoption",
            "titleMissing",
            "bodyMissing",
            "hallucinatedText",
            "disputedRecovery",
        ] {
            metrics.insert(
                key.into(),
                json!({"numerator":0,"denominator":0,"rate":null}),
            );
        }
        let selected: Vec<_> = samples
            .iter()
            .filter(|s| s.get::<String, _>("pool") == pool)
            .collect();
        for s in &selected {
            let id: Uuid = s.get("sample_ref");
            if s.get::<Option<String>, _>("invocation_state").is_some() {
                actual += 1;
                input = input
                    .zip(s.get::<Option<i64>, _>("input_tokens"))
                    .map(|(a, b)| a + b);
                output = output
                    .zip(s.get::<Option<i64>, _>("output_tokens"))
                    .map(|(a, b)| a + b);
                cost = cost
                    .zip(
                        s.get::<Option<String>, _>("cost_pico_usd")
                            .and_then(|s| s.parse::<u128>().ok()),
                    )
                    .map(|(a, b)| a + b);
                reserved += s.get::<Option<i64>, _>("reserved_tokens").unwrap_or(0);
            }
            let mut work_ref = None;
            if let Some(reading) = s.get::<Option<Uuid>, _>("reading_ref") {
                match source::load_in(&mut tx, reading).await? {
                    Some(source) => work_ref = Some(source.work_ref),
                    None => {
                        withheld += 1;
                        continue;
                    }
                }
            }
            let issues: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM linggan_ocr_audit_issue WHERE sample_ref=$1",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            differences += issues;
            let reviews:Vec<Value>=sqlx::query_scalar("SELECT metrics FROM (SELECT decision,metrics FROM linggan_ocr_audit_review WHERE sample_ref=$1 ORDER BY revision DESC LIMIT 1) current_review WHERE decision<>'pending'").bind(id).fetch_all(&mut *tx).await?;
            if !reviews.is_empty() {
                checked += 1;
                if issues == 0 {
                    reviewed_agreements += 1
                } else {
                    reviewed_disagreements += 1
                }
            }
            let pending_issues:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_ocr_audit_issue issue WHERE issue.sample_ref=$1 AND COALESCE((SELECT decision FROM linggan_ocr_audit_review review WHERE review.sample_ref=issue.sample_ref AND (review.issue_ref=issue.issue_ref OR review.issue_ref IS NULL) ORDER BY revision DESC LIMIT 1),'pending')='pending'").bind(id).fetch_one(&mut *tx).await?;
            pending += pending_issues;
            for value in reviews {
                let m: HumanMetrics =
                    serde_json::from_value(value).map_err(|_| FoundationError::Invalid)?;
                for (key, c) in m.pairs() {
                    let v = &mut metrics[key];
                    v["numerator"] =
                        json!(v["numerator"].as_u64().unwrap() + u64::from(c.numerator));
                    v["denominator"] =
                        json!(v["denominator"].as_u64().unwrap() + u64::from(c.denominator));
                }
            }
            if s.get::<Option<String>, _>("failure_code").is_some() {
                failures += 1;
            }
            visible.push(json!({"sampleRef":id,"workRef":work_ref,"pool":pool,"readingRef":s.get::<Option<Uuid>,_>("reading_ref"),"observed":s.get::<bool,_>("observed"),"visionQualified":s.get::<Option<bool>,_>("vision_qualified"),"differenceCount":issues,"failureCode":s.get::<Option<String>,_>("failure_code")}));
        }
        for v in metrics.values_mut() {
            let d = v["denominator"].as_u64().unwrap();
            v["rate"] = if d == 0 {
                Value::Null
            } else {
                json!((100.0 * v["numerator"].as_u64().unwrap() as f64 / d as f64).round())
            };
        }
        pools.push(json!({"pool":pool,"selected":selected.len(),"withheld":withheld,"humanReviewedSamples":checked,"pendingDifferences":pending,"modelDifferences":differences,"humanReviewedAgreements":reviewed_agreements,"humanReviewedDisagreements":reviewed_disagreements,"modelCalls":actual,"failures":failures,"inputTokens":input,"outputTokens":output,"reservedTokens":reserved,"estimatedCostPicoUsd":cost.map(|v|v.to_string()),"metrics":metrics}));
    }
    let trend:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(jsonb_build_object('runRef',run_ref,'createdAt',created_at,'kind',kind) ORDER BY created_at DESC),'[]') FROM(SELECT run_ref,created_at,kind FROM linggan_ocr_audit_run WHERE kind='weekly' AND created_at>$1::text::timestamptz-interval '28 days' AND created_at<=$1::text::timestamptz ORDER BY created_at DESC LIMIT 4)v").bind(r.get::<String,_>("created_text")).fetch_one(&mut *tx).await?;
    Ok(
        json!({"runRef":run,"kind":r.get::<String,_>("kind"),"runKey":r.get::<String,_>("run_key"),"createdAt":r.get::<String,_>("created_text"),"inventory":r.get::<Value,_>("inventory"),"domainRef":r.get::<Value,_>("settings")["domainRef"],"modelId":r.get::<String,_>("model_id"),"promptVersion":r.get::<String,_>("prompt_version"),"settings":r.get::<Value,_>("settings"),"reviewVersions":sqlx::query_scalar::<_,Value>("SELECT COALESCE(jsonb_agg(jsonb_build_object('reviewRef',review_ref,'revision',revision) ORDER BY created_at),'[]') FROM linggan_ocr_audit_review WHERE sample_ref IN(SELECT sample_ref FROM linggan_ocr_audit_sample WHERE run_ref=$1)").bind(run).fetch_one(&mut *tx).await?,"pools":pools,"samples":visible,"fourWeekRuns":trend,"truthBasis":"human_confirmed_points_only","costBasis":"frozen_price_estimate_not_provider_bill"}),
    )
}
pub async fn recent(db: &Database) -> Result<Value, FoundationError> {
    let rows: Vec<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM(SELECT run_ref,created_at,row_number() OVER(PARTITION BY (kind='weekly') ORDER BY created_at DESC,run_ref) AS n FROM linggan_ocr_audit_run)v WHERE n<=4 ORDER BY created_at DESC",
    )
    .fetch_all(db.pool())
    .await?;
    let mut reports = vec![];
    for id in rows {
        reports.push(report(db, id).await?);
    }
    Ok(json!({"runs":reports}))
}
