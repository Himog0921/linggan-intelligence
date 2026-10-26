//! One frozen image, one provider invocation.  A failed/ambiguous receipt is terminal for
//! the run so a later image can never be sent after a previous one failed.
use super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{TaskKey, admission, batch_store::InputRef, transport_store},
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    ocr_recognition_schema as visual,
    pi_adapter::{PiAdapter, PiImage, PiRequest, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_evidence::ocr_source::ImageRole;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    item: Uuid,
    run: Uuid,
    blob: String,
    content: Uuid,
    slot_key: String,
    materialization_ref: Uuid,
    width: u32,
    height: u32,
    expected_reading_ref: Option<Uuid>,
}
fn image(
    storage: &str,
    mime: &str,
    size: i64,
    hash: &str,
) -> Result<(PiImage, u32, u32), ExecutorError> {
    use base64::Engine;
    use std::path::{Path, PathBuf};
    if Path::new(storage).is_absolute()
        || storage
            .split('/')
            .any(|p| p.is_empty() || matches!(p, "." | ".."))
    {
        return Err(ExecutorError::Invalid);
    }
    let root = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
        .map(PathBuf::from)
        .ok_or(ExecutorError::Invalid)?;
    let root = std::fs::canonicalize(root).map_err(|_| ExecutorError::Invalid)?;
    let file = std::fs::canonicalize(root.join(storage)).map_err(|_| ExecutorError::Invalid)?;
    if !file.starts_with(&root) {
        return Err(ExecutorError::Invalid);
    }
    let bytes = std::fs::read(file).map_err(|_| ExecutorError::Invalid)?;
    if bytes.len() as i64 != size || linggan_evidence::ocr_source::content_hash(&bytes) != hash {
        return Err(ExecutorError::Invalid);
    }
    let fmt = image::guess_format(&bytes).map_err(|_| ExecutorError::Invalid)?;
    let actual = match fmt {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(ExecutorError::Invalid),
    };
    if actual != mime {
        return Err(ExecutorError::Invalid);
    }
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(&bytes), fmt)
        .into_dimensions()
        .map_err(|_| ExecutorError::Invalid)?;
    Ok((
        PiImage {
            mime_type: actual.into(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        },
        w,
        h,
    ))
}
fn write_raw(item: Uuid, raw: &str) -> Result<(String, i64), ExecutorError> {
    use std::path::PathBuf;
    let root = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
        .map(PathBuf::from)
        .ok_or(ExecutorError::Invalid)?;
    let root = std::fs::canonicalize(root).map_err(|_| ExecutorError::Invalid)?;
    let relative = format!("ocr/manual/{item}.json");
    let target = root.join(&relative);
    let parent = target.parent().ok_or(ExecutorError::Invalid)?;
    std::fs::create_dir_all(parent).map_err(|_| ExecutorError::Invalid)?;
    let parent = std::fs::canonicalize(parent).map_err(|_| ExecutorError::Invalid)?;
    if !parent.starts_with(&root) {
        return Err(ExecutorError::Invalid);
    }
    let temporary = parent.join(format!(".{item}.tmp"));
    std::fs::write(&temporary, raw.as_bytes()).map_err(|_| ExecutorError::Invalid)?;
    std::fs::rename(&temporary, &target).map_err(|_| ExecutorError::Invalid)?;
    Ok((relative, raw.len() as i64))
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::OcrRecognitionItem { item_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(ExecutorError::Invalid);
    };
    admission::check_queue_configuration(
        &mut tx,
        TaskKey::OcrRecognition,
        item.task_config_ref,
        true,
    )
    .await?;
    admission::check_queue_input_scope(
        &mut tx,
        TaskKey::OcrRecognition,
        item.task_config_ref,
        &item.input,
    )
    .await?;
    let r=sqlx::query("SELECT i.*,run.content_public_ref,run.task_config_ref FROM linggan_ocr_recognition_item i JOIN linggan_ocr_recognition_run run USING(run_ref) WHERE i.item_ref=$1 FOR UPDATE OF i,run").bind(item_ref).fetch_one(&mut *tx).await?;
    if r.get::<Uuid, _>("task_config_ref") != item.task_config_ref {
        return Err(ExecutorError::Invalid);
    };
    let cfg: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(item.task_config_ref)
    .fetch_one(&mut *tx)
    .await?;
    let model = transport_store::load_frozen_in(&mut tx, cfg).await?;
    if model.limits.attempts != 1 {
        return Err(ExecutorError::Invalid);
    };
    let (pic, w, h) = image(
        &r.get::<String, _>("storage_key"),
        &r.get::<String, _>("mime_type"),
        r.get("byte_size"),
        &r.get::<String, _>("blob_sha256"),
    )?;
    let workspace: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    if secrets.is_synthetic()
        && !(model.local_endpoint && model.base_url.starts_with("http://127.0.0.1:"))
    {
        return Err(ExecutorError::Invalid);
    };
    let role: String = r.get("role");
    let ordinal: Option<i32> = r.get("ordinal");
    let image_role = if role == "cover" {
        ImageRole::ExplicitCover
    } else if role == "page" {
        ImageRole::Interior
    } else {
        ImageRole::Unknown
    };
    let (system, prompt) = visual::prompt_for_image(Some((image_role, ordinal.map(|n| n as u32))));
    let mut request = model.request(
        secrets.get(workspace, model.secret_ref)?,
        system.into(),
        prompt,
        Some(visual::response_schema()),
    );
    request.operation = "analyze_image".into();
    request.images = vec![pic];
    request.wire_bytes()?;
    let reserved = model
        .limits
        .input_tokens
        .checked_add(model.limits.output_tokens)
        .ok_or(ExecutorError::Invalid)?;
    let admitted =
        super::budget::admit_in(&mut tx, item.task_key, item.task_config_ref, reserved).await?;
    let price =
        super::budget::latest_price_in(&mut tx, model.current_model_ref, model.endpoint_ref)
            .await?;
    let egress:Uuid=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key='ocr_recognition' AND material_class='manual_ocr_image' AND allowed").bind(model.endpoint_ref).fetch_one(&mut *tx).await?;
    let invocation = Uuid::new_v4();
    let req_hash = content_hash(
        &json!([
            "ocr-recognition.v1",
            visual::PROMPT_VERSION,
            item.item_ref,
            item.task_config_ref,
            model.connection_version_ref,
            r.get::<String, _>("blob_sha256")
        ])
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,task_config_ref,batch_ref,round_no,quota_bucket,classification_source,dispatch_admitted_at,deadline_at,egress_event_ref,result,created_at,price_ref) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,'ocr_recognition',$7,$8,$9,'task_day','producer_v1',$10::text::timestamptz,$10::text::timestamptz+make_interval(secs=>$11),$12,$13,$10::text::timestamptz,$14)").bind(invocation).bind(model.connection_version_ref).bind(model.current_model_ref).bind(cfg).bind(req_hash).bind(reserved as i64).bind(item.task_config_ref).bind(item.batch_ref).bind((item.position/linggan_contracts::ai_limits::AI_LIMITS.round_size+1)as i32).bind(admitted).bind(request.timeout_ms as f64/1000.0+10.0).bind(egress).bind(json!({"producer":"ai-gen1-executor.v1","purpose":"ocr_recognition","promptVersion":visual::PROMPT_VERSION})).bind(price).execute(&mut *tx).await?;
    let changed=sqlx::query("UPDATE linggan_ai_batch_item_control SET invocation_ref=$2,attempt_count=attempt_count+1,executed_config_ref=$3,execution_contract='ocr-recognition.v1' WHERE item_ref=$1 AND attempt_count=0").bind(item.item_ref).bind(invocation).bind(item.task_config_ref).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ExecutorError::FenceLost);
    }
    sqlx::query("UPDATE linggan_ocr_recognition_item SET state='running',invocation_ref=$2 WHERE item_ref=$1").bind(item_ref).bind(invocation).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_ocr_recognition_run SET state='running',updated_at=scope_001_now() WHERE run_ref=$1").bind(r.get::<Uuid,_>("run_ref")).execute(&mut *tx).await?;
    let expected_reading_ref:Option<Uuid>=sqlx::query_scalar("SELECT current_reading_ref FROM linggan_material_unit WHERE content_public_ref=$1 AND unit_key=$2").bind(r.get::<Uuid,_>("content_public_ref")).bind(format!("image:{}",r.get::<String,_>("slot_key"))).fetch_optional(&mut *tx).await?.flatten();
    tx.commit().await?;
    Ok(Prepared {
        expected_reading_ref,
        invocation,
        request,
        item: item_ref,
        run: r.get("run_ref"),
        blob: r.get("blob_sha256"),
        content: r.get("content_public_ref"),
        slot_key: r.get("slot_key"),
        materialization_ref: r.get("materialization_ref"),
        width: w,
        height: h,
    })
}
pub async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let p = match prepare(db, item, secrets).await {
        Ok(v) => v,
        Err(ExecutorError::Configuration(
            crate::ai_foundation::store::FoundationError::AdmissionDenied(_),
        )) => {
            item.skip_unattempted(db, "ocr_admission_denied").await?;
            return Ok(());
        }
        Err(e) => return super::preparation::handle(db, item, e).await,
    };
    let response = super::dispatch::call(db, item, adapter, &p.request).await;
    receipt::finalize(db, item, p.invocation, response.as_ref(), || {
        settle(db, item, &p, response.as_ref())
    })
    .await
}
async fn settle(
    db: &Database,
    item: &ClaimedItem,
    p: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let (mut tx, cancelled) = receipt::lock_in(db, item, p.invocation, response).await?;
    let raw = response.and_then(|r| r.text.as_deref());
    // Preserve exactly what arrived, including a response that fails the versioned visual
    // schema.  Such a receipt remains inspectable but never becomes a candidate reading.
    let raw_asset = raw.map(|body| write_raw(p.item, body)).transpose()?;
    let parsed = response.filter(|r| r.ok).map(|r| {
        visual::parse_visual_diagnostic(
            r.text.as_deref().unwrap_or_default(),
            r.diagnostic
                .as_ref()
                .and_then(|d| d.finish_reason.as_deref()),
        )
    });
    let success = matches!(parsed, Some(Ok(_))) && !cancelled;
    let failure = if success {
        None
    } else if cancelled {
        Some("cancel_requested")
    } else if response
        .and_then(|r| r.diagnostic.as_ref())
        .and_then(|d| d.finish_reason.as_deref())
        == Some("length")
    {
        Some("ocr_output_truncated")
    } else if let Some(Err(code)) = &parsed {
        Some(*code)
    } else {
        Some("ocr_provider_failed")
    };
    let parsed = parsed.and_then(Result::ok);
    let raw_hash = raw.map(content_hash);
    sqlx::query("UPDATE linggan_ocr_recognition_item SET state=$2,raw_result_ref=$3,raw_result_sha256=$4,raw_json=$5,finish_reason=$6 WHERE item_ref=$1")
        .bind(p.item)
        .bind(if success { "succeeded" } else { "failed" })
        .bind(raw_asset.as_ref().map(|(key, _)| key))
        .bind(raw_hash.as_ref())
        .bind(raw)
        .bind(response.and_then(|r| r.diagnostic.as_ref()).and_then(|d| d.finish_reason.as_deref()))
        .execute(&mut *tx)
        .await?;
    if let Some(v) = parsed.filter(|_| !cancelled) {
        let (raw_storage_key, raw_size) = raw_asset.ok_or(ExecutorError::Invalid)?;
        // A run is an immutable producer generation.  Replaying this receipt selects the same
        // version, while starting another confirmed run cannot overwrite this derivative.
        let processor_version = format!(
            "{}:{}",
            linggan_evidence::cloud_ocr_adoption::PROCESSOR_VERSION,
            p.run
        );
        let job = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'image_ocr',$4,'full') ON CONFLICT(blob_sha256,slot_key,processor_kind,processor_version,input_scope) DO NOTHING")
            .bind(job).bind(&p.blob).bind(&p.slot_key).bind(&processor_version).execute(&mut *tx).await?;
        let job:Uuid=sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE blob_sha256=$1 AND slot_key=$2 AND processor_kind='image_ocr' AND processor_version=$3 AND input_scope='full'")
            .bind(&p.blob).bind(&p.slot_key).bind(&processor_version).fetch_one(&mut *tx).await?;
        let derivative = Uuid::new_v4();
        let hash = raw_hash.as_deref().ok_or(ExecutorError::Invalid)?;
        sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,$4,$5)").bind(derivative).bind(job).bind(hash).bind(raw_size).bind(&raw_storage_key).execute(&mut *tx).await?;
        let layout = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'cloud_manual',$5,$6,$7,$8,$9,$10)")
            .bind(layout).bind(derivative).bind(p.content).bind(&p.blob).bind(&processor_version).bind(p.width as i32).bind(p.height as i32).bind(hash).bind(raw_size).bind(&raw_storage_key).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) VALUES($1,$2,'succeeded','cloud_manual_ocr_saved')").bind(Uuid::new_v4()).bind(job).execute(&mut *tx).await?;
        let visual_json = serde_json::to_value(&v).map_err(|_| ExecutorError::Invalid)?;
        let blocks = linggan_evidence::cloud_ocr_adoption::blocks_from_visual(&visual_json)?;
        let reading = linggan_evidence::material_reading::producers::publish_cloud_in(
            &mut tx,
            p.content,
            job,
            p.materialization_ref,
            derivative,
            blocks,
            p.expected_reading_ref,
        )
        .await?;
        sqlx::query("UPDATE linggan_ocr_recognition_item SET reading_ref=$2 WHERE item_ref=$1")
            .bind(p.item)
            .bind(reading.reading_ref)
            .execute(&mut *tx)
            .await?;
    }
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["promptVersion"] = json!(visual::PROMPT_VERSION);
    result["executorReceipt"] = json!(item.lease_token);
    finish_invocation_in(&mut tx, p.invocation, response, success, failure, &result).await?;
    if success {
        sqlx::query("UPDATE linggan_ocr_recognition_run SET completed_images=completed_images+1,state=CASE WHEN completed_images+1=total_images THEN 'succeeded' ELSE 'running' END,updated_at=scope_001_now() WHERE run_ref=$1").bind(p.run).execute(&mut *tx).await?;
    } else {
        sqlx::query("UPDATE linggan_ocr_recognition_run SET state='stopped',failure_code=$2,updated_at=scope_001_now() WHERE run_ref=$1").bind(p.run).bind(failure).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_ocr_recognition_item SET state='unattempted' WHERE run_ref=$1 AND state='pending'").bind(p.run).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_ai_batch_item_control c SET state='skipped',usable=false,failure_code='ocr_prior_item_failed',finished_at=scope_001_now() FROM linggan_ai_batch_item i WHERE i.item_ref=c.item_ref AND i.batch_ref=$1 AND c.state='pending'").bind(item.batch_ref).execute(&mut *tx).await?;
    }
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: Some(&json!({"itemRef":p.item})),
            failure,
            counts_as_failure: !success && !cancelled,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
