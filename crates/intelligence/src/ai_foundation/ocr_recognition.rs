//! Work-scoped manual OCR persistence. Browser input is an idempotency key and a
//! previously read scope hash; URLs, bytes, provider results and model ids stay server-side.
use super::{
    TaskKey, admission,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    transport_store,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRun {
    pub command_ref: Uuid,
    pub expected_scope_sha256: String,
    pub slot_keys: Vec<String>,
}
#[derive(Clone)]
struct Source {
    slot_key: String,
    blob_sha256: String,
    materialization_ref: Uuid,
    aliases: Vec<String>,
    ordinal: Option<i32>,
    cover: bool,
    original_url: String,
    storage_key: String,
    mime_type: String,
    byte_size: i64,
}

async fn sources_in(
    conn: &mut sqlx::PgConnection,
    work: Uuid,
) -> Result<Vec<Source>, FoundationError> {
    let rows = sqlx::query(
        "WITH current_origin AS MATERIALIZED (
            SELECT DISTINCT ON (o.slot_key) o.* FROM linggan_material_media_origin o
            JOIN linggan_runtime_capture_package p USING(package_ref)
            JOIN linggan_runtime_record_disposition d ON d.package_ref=o.package_ref AND d.record_ordinal=o.record_ordinal
            WHERE d.disposition='accepted_for_media_identity' AND o.content_public_ref=$1 AND linggan_dossier_slot_active($1,o.slot_key,scope_001_now()) AND p.accepted_at<=scope_001_now() AND o.created_at<=scope_001_now()
              AND o.purpose IN ('cover','body_image','live_photo')
            ORDER BY o.slot_key,o.source_generation DESC
        ), candidates AS MATERIALIZED (
            SELECT o.slot_key,o.purpose,m.materialization_ref,m.blob_sha256,
                   CASE WHEN o.display_order_state='KNOWN' THEN o.display_ordinal END ordinal,
                   m.local_asset_path,b.storage_key,b.mime_type,b.byte_size,m.verified_at
            FROM current_origin o
            JOIN LATERAL(SELECT m.* FROM linggan_media_materialization m JOIN linggan_media_download_attempt a USING(download_attempt_ref) JOIN linggan_media_blob image_blob ON image_blob.sha256=m.blob_sha256
                WHERE image_blob.mime_type LIKE 'image/%' AND a.media_observation_ref=o.observation_ref AND m.verified_at<=scope_001_now()
                ORDER BY m.verified_at DESC,m.materialization_ref DESC LIMIT 1) m ON true
            JOIN linggan_media_blob b ON b.sha256=m.blob_sha256
            WHERE b.mime_type LIKE 'image/%'
              AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d
                WHERE d.slot_key=o.slot_key OR d.blob_sha256=m.blob_sha256 OR d.materialization_ref=m.materialization_ref)
        ) SELECT c.*,ARRAY[c.slot_key] || COALESCE((SELECT array_agg(cover.slot_key) FROM candidates cover
                WHERE c.purpose<>'cover' AND c.ordinal=1 AND cover.purpose='cover' AND cover.blob_sha256=c.blob_sha256),'{}') aliases,
            (c.purpose='cover' OR COALESCE(c.ordinal=1,false)) cover
            FROM candidates c
            WHERE c.purpose<>'cover' OR NOT EXISTS(SELECT 1 FROM candidates first_page WHERE first_page.purpose<>'cover' AND first_page.ordinal=1 AND first_page.blob_sha256=c.blob_sha256)
            ORDER BY ordinal NULLS LAST,slot_key",
    )
    .bind(work)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Source {
            slot_key: r.get("slot_key"),
            blob_sha256: r.get("blob_sha256"),
            materialization_ref: r.get("materialization_ref"),
            aliases: r.get("aliases"),
            ordinal: r.get("ordinal"),
            cover: r.get("cover"),
            original_url: format!(
                "/api/local/media/{}/{}",
                r.get::<Uuid, _>("materialization_ref"),
                r.get::<String, _>("blob_sha256")
            ),
            storage_key: r.get("storage_key"),
            mime_type: r.get("mime_type"),
            byte_size: r.get("byte_size"),
        })
        .collect())
}
async fn scope_in(
    conn: &mut sqlx::PgConnection,
    work: Uuid,
    config: Uuid,
) -> Result<(Value, Vec<Source>), FoundationError> {
    let cfg: Option<Uuid> = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(config)
    .fetch_one(&mut *conn)
    .await?;
    let cfg = cfg.ok_or(FoundationError::ConfigurationIncomplete)?;
    let model = transport_store::load_frozen_in(conn, cfg).await?;
    let sources = sources_in(conn, work).await?;
    if sources.is_empty() {
        return Err(FoundationError::NotFound);
    }
    let images:Vec<Value>=sources.iter().map(|s|json!({"blobSha256":s.blob_sha256,"materializationRef":s.materialization_ref,"slotKey":s.slot_key,"aliases":s.aliases,"ordinal":s.ordinal,"cover":s.cover})).collect();
    let sha=crate::research_text::content_hash(&json!({"workRef":work,"taskConfigRef":config,"connectionVersionRef":model.connection_version_ref,"modelId":model.model_id,"endpointUrl":model.base_url,"images":images}).to_string());
    Ok((
        json!({"sha256":sha,"imageCount":sources.len(),"modelLabel":model.model_id,"endpointUrl":model.base_url,"images":sources.iter().map(|s|json!({"slotKey":s.slot_key,"ordinal":s.ordinal,"role":if s.cover{"cover"}else if s.ordinal.is_some(){"page"}else{"unknown"},"originalUrl":s.original_url})).collect::<Vec<_>>()}),
        sources,
    ))
}
async fn ready_scope_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work: Uuid,
    config: Uuid,
) -> Result<Value, FoundationError> {
    let (scope, _) = scope_in(&mut *tx, work, config).await?;
    admission::check_queue_configuration(tx, TaskKey::OcrRecognition, config, false).await?;
    let allowed: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT e.allowed FROM linggan_ai_task_config c
          JOIN linggan_model_config m ON m.config_ref=c.model_config_ref
          JOIN linggan_model_capability cap ON cap.capability_ref=m.capability_ref
          JOIN linggan_ai_current_egress e ON e.endpoint_ref=cap.endpoint_ref
          WHERE c.task_config_ref=$1 AND e.task_key='ocr_recognition'
            AND e.material_class='manual_ocr_image'),false)",
    )
    .bind(config)
    .fetch_one(&mut **tx)
    .await?;
    if !allowed {
        return Err(FoundationError::AdmissionDenied(vec![
            "egress_not_approved".into(),
        ]));
    }
    Ok(scope)
}
pub async fn start_run(
    db: &Database,
    work: Uuid,
    request: &StartRun,
) -> Result<Value, FoundationError> {
    let mut c = Command::begin(
        db,
        request.command_ref,
        "ocr_recognition_start",
        &json!({"workRef":work,"expectedScopeSha256":request.expected_scope_sha256,"slotKeys":request.slot_keys}),
    )
    .await?;
    if let Some(r) = c.replay()? {
        return Ok(r);
    }
    let config:Uuid=sqlx::query_scalar("SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='ocr_recognition' FOR UPDATE").fetch_one(&mut *c.tx).await?;
    let (scope, sources) = scope_in(&mut c.tx, work, config).await?;
    if scope["sha256"].as_str() != Some(request.expected_scope_sha256.as_str()) {
        return Err(FoundationError::Conflict);
    }
    let selected: std::collections::BTreeSet<_> = request.slot_keys.iter().collect();
    if selected.is_empty()
        || selected.len() != request.slot_keys.len()
        || selected
            .iter()
            .any(|key| !sources.iter().any(|s| s.slot_key.as_str() == key.as_str()))
    {
        return Err(FoundationError::Invalid);
    }
    let sources: Vec<_> = sources
        .into_iter()
        .filter(|s| selected.contains(&s.slot_key))
        .collect();
    // Batch cancellation is authoritative. Persist the matching run before looking for
    // active work so a later explicit confirmation creates a new immutable generation.
    sqlx::query(
        "UPDATE linggan_ocr_recognition_run r SET state='stopped',updated_at=scope_001_now()
         FROM linggan_ai_batch_control b
         WHERE r.batch_ref=b.batch_ref AND r.content_public_ref=$1
           AND r.state IN ('queued','running','finalizing')
           AND (b.cancel_requested OR b.state='cancelled')",
    )
    .bind(work)
    .execute(&mut *c.tx)
    .await?;
    if let Some(run)=sqlx::query_scalar::<_,Uuid>("SELECT r.run_ref FROM linggan_ocr_recognition_run r
        LEFT JOIN linggan_ai_batch_control b USING(batch_ref)
        WHERE r.content_public_ref=$1 AND (r.state='unknown' OR (r.state IN('queued','running','finalizing') AND b.state='open' AND NOT b.cancel_requested)) FOR UPDATE OF r").bind(work).fetch_optional(&mut *c.tx).await?{return c.finish(json!({"runRef":run,"replayed":true})).await}
    let run = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ocr_recognition_run(run_ref,command_ref,content_public_ref,task_config_ref,state,model_label,endpoint_url,scope_sha256,total_images) VALUES($1,$2,$3,$4,'queued',$5,$6,$7,$8)").bind(run).bind(request.command_ref).bind(work).bind(config).bind(scope["modelLabel"].as_str()).bind(scope["endpointUrl"].as_str()).bind(scope["sha256"].as_str()).bind(sources.len() as i32).execute(&mut *c.tx).await?;
    let mut inputs = Vec::new();
    for (position, s) in sources.iter().enumerate() {
        let item = Uuid::new_v4();
        let role = if s.cover || s.ordinal == Some(1) {
            "cover"
        } else if s.ordinal.is_some_and(|n| n > 1) {
            "page"
        } else {
            "unknown"
        };
        sqlx::query("INSERT INTO linggan_ocr_recognition_item(item_ref,run_ref,position,slot_key,blob_sha256,materialization_ref,ordinal,role,state,original_url,storage_key,mime_type,byte_size) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'pending',$9,$10,$11,$12)").bind(item).bind(run).bind(position as i32).bind(&s.slot_key).bind(&s.blob_sha256).bind(s.materialization_ref).bind(s.ordinal).bind(role).bind(&s.original_url).bind(&s.storage_key).bind(&s.mime_type).bind(s.byte_size).execute(&mut *c.tx).await?;
        for alias in &s.aliases {
            sqlx::query("INSERT INTO linggan_ocr_recognition_slot_alias(item_ref,item_slot_key) VALUES($1,$2)").bind(item).bind(alias).execute(&mut *c.tx).await?;
        }
        inputs.push(BatchInput {
            item_key: item.to_string(),
            input: InputRef::OcrRecognitionItem { item_ref: item },
        })
    }
    let h = batch_store::input_hash(TaskKey::OcrRecognition, &inputs, false)?;
    let batch = batch_store::insert_batch(
        &mut c.tx,
        request.command_ref,
        TaskKey::OcrRecognition,
        config,
        BatchKind::Interactive,
        &inputs,
        &h,
    )
    .await?;
    sqlx::query("UPDATE linggan_ocr_recognition_run SET batch_ref=$2 WHERE run_ref=$1")
        .bind(run)
        .bind(batch)
        .execute(&mut *c.tx)
        .await?;
    c.finish(json!({"runRef":run,"batchRef":batch,"queued":true}))
        .await
}
pub async fn read_run(db: &Database, work: Uuid) -> Result<Value, FoundationError> {
    let row = sqlx::query("SELECT r.*,b.revision,b.cancel_requested,b.state AS batch_state FROM linggan_ocr_recognition_run r LEFT JOIN linggan_ai_batch_control b USING(batch_ref) WHERE r.content_public_ref=$1 ORDER BY r.created_at DESC LIMIT 1")
        .bind(work)
        .fetch_optional(db.pool())
        .await?;
    let Some(run) = row else {
        let mut tx = db.pool().begin().await?;
        let config: Uuid = sqlx::query_scalar("SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='ocr_recognition'")
            .fetch_one(&mut *tx)
            .await?;
        return match ready_scope_in(&mut tx, work, config).await {
            Ok(scope) => {
                tx.commit().await?;
                Ok(empty(work, "ready", Some(scope), None))
            }
            Err(error) => Ok(empty(
                work,
                "unavailable",
                None,
                Some(unavailable_reason(&error)),
            )),
        };
    };
    let run_ref: Uuid = run.get("run_ref");
    let items = sqlx::query(
        "SELECT i.*,inv.input_tokens,inv.output_tokens,inv.measured_tokens,
            inv.cost_pico_usd::text cost_pico_usd,
            NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d
              WHERE d.slot_key=i.slot_key OR d.blob_sha256=i.blob_sha256 OR d.materialization_ref=i.materialization_ref) visible,
            COALESCE((SELECT jsonb_agg(item_slot_key ORDER BY item_slot_key)
              FROM linggan_ocr_recognition_slot_alias a WHERE a.item_ref=i.item_ref),'[]') aliases,
            ctl.state AS control_state,ctl.failure_code AS control_failure_code,
            COALESCE(u.current_reading_ref=i.reading_ref AND u.source_version=reading.source_version,false) is_current,COALESCE(reading.blocks,'[]'::jsonb) blocks,COALESCE(reading.issues,'[]'::jsonb) issues
         FROM linggan_ocr_recognition_item i
         LEFT JOIN linggan_model_invocation inv ON inv.invocation_ref=i.invocation_ref
         LEFT JOIN linggan_ocr_recognition_run owning_run ON owning_run.run_ref=i.run_ref
         LEFT JOIN linggan_ai_batch_item bi ON bi.batch_ref=owning_run.batch_ref AND bi.item_key=i.item_ref::text
         LEFT JOIN linggan_ai_batch_item_control ctl ON ctl.item_ref=bi.item_ref
         LEFT JOIN linggan_material_reading reading ON reading.reading_ref=i.reading_ref
         LEFT JOIN linggan_material_unit u ON u.unit_ref=reading.unit_ref
         WHERE i.run_ref=$1 ORDER BY i.position",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await?;
    let cancelled = run
        .get::<Option<bool>, _>("cancel_requested")
        .unwrap_or(false)
        || run.get::<Option<String>, _>("batch_state").as_deref() == Some("cancelled");
    let recovery_unknown = items.iter().any(|item| {
        item.get::<Option<String>, _>("control_failure_code")
            .as_deref()
            == Some("ai_dispatch_unknown")
    });
    let read_state = if recovery_unknown {
        "unknown".to_owned()
    } else if cancelled {
        "stopped".to_owned()
    } else {
        run.get::<String, _>("state")
    };
    let read_failure = if recovery_unknown {
        Some("ai_dispatch_unknown".to_owned())
    } else {
        run.get::<Option<String>, _>("failure_code")
    };
    let (ready_scope, unavailable) = if matches!(read_state.as_str(), "succeeded" | "stopped") {
        let mut tx = db.pool().begin().await?;
        let config: Uuid = sqlx::query_scalar("SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='ocr_recognition'").fetch_one(&mut *tx).await?;
        let result = ready_scope_in(&mut tx, work, config).await;
        tx.commit().await?;
        match result {
            Ok(scope) => (Some(scope), None),
            Err(error) => (None, Some(unavailable_reason(&error))),
        }
    } else {
        (None, None)
    };
    for item in &items {
        let blocks: Vec<Value> =
            serde_json::from_value(item.get("blocks")).map_err(|_| FoundationError::Invalid)?;
        linggan_evidence::research_dossier::content::validate_blocks(&blocks)
            .map_err(|_| FoundationError::Invalid)?;
        linggan_evidence::research_dossier::content::validate_issues(
            &item.get::<Value, _>("issues"),
        )
        .map_err(|_| FoundationError::Invalid)?;
    }
    let pending: usize = items
        .iter()
        .filter(|i| i.get::<bool, _>("visible"))
        .map(|i| {
            let blocks: Value = i.get("blocks");
            let issues: Value = i.get("issues");
            issues.as_array().map_or(0, Vec::len)
                + blocks
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|b| b["issues"].as_array().map_or(0, Vec::len))
                    .sum::<usize>()
        })
        .sum();
    // A persisted pointer alone can outlive the source. Reuse the canonical reader's
    // current source/restriction rules when reporting that a result is in the dossier.
    let mut conn=db.pool().acquire().await?;
    let now:String=sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *conn).await?;
    let current=linggan_evidence::material_reading::read_current_in(&mut conn,&[work],&now).await?;
    let current_refs:std::collections::HashSet<Uuid>=current.get(&work).into_iter().flatten().filter_map(|u|u["currentRevisionRef"].as_str().and_then(|s|Uuid::parse_str(s).ok())).collect();
    drop(conn);
    let image_json = items
        .iter()
        .map(|i| {
            let visible = i.get::<bool, _>("visible");
            let has_invocation = i.get::<Option<Uuid>, _>("invocation_ref").is_some();
            let persisted_state = i.get::<String, _>("state");
            let control_state = i.get::<Option<String>, _>("control_state");
            let control_failure = i.get::<Option<String>, _>("control_failure_code");
            let state = if !visible {
                "unknown".to_owned()
            } else if control_failure.as_deref() == Some("ai_dispatch_unknown") {
                "unknown".to_owned()
            } else if matches!(control_state.as_deref(), Some("cancelled" | "skipped"))
                && matches!(persisted_state.as_str(), "pending" | "running")
            {
                "unattempted".to_owned()
            } else {
                persisted_state
            };
            json!({
                "imageRef":i.get::<Uuid,_>("item_ref"),"ordinal":i.get::<Option<i32>,_>("ordinal"),
                "role":i.get::<String,_>("role"),"state":state,
                "slotKey":if visible{json!(i.get::<String,_>("slot_key"))}else{Value::Null},
                "aliases":if visible{i.get::<Value,_>("aliases")}else{json!([])},
                "originalUrl":if visible{json!(i.get::<String,_>("original_url"))}else{Value::Null},
                "rawResultRef":if visible{json!(i.get::<Option<String>,_>("raw_result_ref"))}else{Value::Null},
                "rawResultSha256":if visible{json!(i.get::<Option<String>,_>("raw_result_sha256"))}else{Value::Null},
                "rawJson":if visible{json!(i.get::<Option<String>,_>("raw_json"))}else{Value::Null},
                "invocationRef":if visible{json!(i.get::<Option<Uuid>,_>("invocation_ref"))}else{Value::Null},
                "finishReason":if visible{json!(i.get::<Option<String>,_>("finish_reason"))}else{Value::Null},
                "usage":if visible && has_invocation {json!({"inputTokens":i.get::<Option<i64>,_>("input_tokens"),"outputTokens":i.get::<Option<i64>,_>("output_tokens"),"measuredTokens":i.get::<Option<i64>,_>("measured_tokens")})}else{Value::Null},
                "estimatedCostUsd":if visible { pico_usd(i.get::<Option<String>,_>("cost_pico_usd")) } else { Value::Null },
                "blocks":if visible{i.get::<Value,_>("blocks")}else{json!([])},
                "issues":if visible{i.get::<Value,_>("issues")}else{json!([])},
                "readingRef":if visible{json!(i.get::<Option<Uuid>,_>("reading_ref"))}else{Value::Null},"isCurrent":visible && i.get::<bool,_>("is_current") && i.get::<Option<Uuid>,_>("reading_ref").is_some_and(|r|current_refs.contains(&r)),"generatedBy":"cloud_vision"
            })
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"runRef":run_ref,"workRef":work,"state":read_state,"completedImages":run.get::<i32,_>("completed_images"),"totalImages":run.get::<i32,_>("total_images"),"currentStage":"按冻结顺序处理当前图组。","modelLabel":run.get::<Option<String>,_>("model_label"),"unavailableReason":unavailable,"images":image_json,"pendingCount":pending,"failureCode":read_failure,"batchRef":run.get::<Option<Uuid>,_>("batch_ref"),"batchRevision":run.get::<Option<i64>,_>("revision"),"cancelRequested":run.get::<Option<bool>,_>("cancel_requested").unwrap_or(false),"scope":ready_scope}),
    )
}
fn pico_usd(value: Option<String>) -> Value {
    let Some(pico) = value.and_then(|v| v.parse::<u128>().ok()) else {
        return Value::Null;
    };
    let whole = pico / 1_000_000_000_000;
    let fraction = pico % 1_000_000_000_000;
    let fraction = format!("{fraction:012}").trim_end_matches('0').to_owned();
    json!(if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    })
}
fn unavailable_reason(error: &FoundationError) -> &'static str {
    match error {
        FoundationError::NotFound => "当前作品没有可识别且未撤回的原始图片。",
        FoundationError::ConfigurationIncomplete | FoundationError::NotConnected => {
            "图片文字识别尚未配置可用模型或端点。"
        }
        FoundationError::AdmissionDenied(reasons)
            if reasons.iter().any(|r| r == "egress_not_approved") =>
        {
            "当前模型尚未获准外发本篇原始图片。"
        }
        FoundationError::AdmissionDenied(reasons)
            if reasons.iter().any(|r| r.ends_with("_off")) =>
        {
            "图片文字识别执行开关尚未开启。"
        }
        FoundationError::AdmissionDenied(reasons)
            if reasons.iter().any(|r| r == "daily_budget_unconfigured") =>
        {
            "图片文字识别尚未配置可用额度。"
        }
        FoundationError::AdmissionDenied(_) => "当前模型或图片外发许可尚未开启。",
        _ => "当前无法读取图片文字识别的准备条件。",
    }
}
fn empty(work: Uuid, state: &str, scope: Option<Value>, unavailable_reason: Option<&str>) -> Value {
    json!({"runRef":Value::Null,"workRef":work,"state":state,"completedImages":0,"totalImages":0,"currentStage":if state=="ready"{"确认当前图组后可开始。"}else{"当前图片或模型配置不可用。"},"modelLabel":scope.as_ref().and_then(|s|s["modelLabel"].as_str()),"unavailableReason":unavailable_reason,"images":[],"pendingCount":0,"failureCode":Value::Null,"batchRef":Value::Null,"batchRevision":Value::Null,"cancelRequested":false,"scope":scope})
}
