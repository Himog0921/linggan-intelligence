//! OCR opinions use the same fenced executor, ledger and transport as other AI tasks.
use super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        TaskKey, admission, batch_store::InputRef, store::FoundationError, transport_store,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    ocr_quality_audit::{self as audit, Settings, source},
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;
struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    sample: Uuid,
    probe: bool,
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::OcrAuditSample { sample_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(
        &mut tx,
        TaskKey::OcrQualityAudit,
        item.task_config_ref,
        true,
    )
    .await?;
    admission::check_queue_input_scope(
        &mut tx,
        TaskKey::OcrQualityAudit,
        item.task_config_ref,
        &item.input,
    )
    .await?;
    let sample=sqlx::query("SELECT s.*,r.settings,r.task_config_ref,r.endpoint_ref,r.connection_version_ref,r.price_ref,r.model_id FROM linggan_ocr_audit_sample s JOIN linggan_ocr_audit_run r USING(run_ref) WHERE s.sample_ref=$1 FOR UPDATE OF r").bind(sample_ref).fetch_one(&mut *tx).await?;
    if sample.get::<Uuid, _>("task_config_ref") != item.task_config_ref {
        return Err(ExecutorError::Invalid);
    }
    let settings: Settings =
        serde_json::from_value(sample.get("settings")).map_err(|_| ExecutorError::Invalid)?;
    let cfg: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(item.task_config_ref)
    .fetch_one(&mut *tx)
    .await?;
    let model = transport_store::load_frozen_in(&mut tx, cfg).await?;
    if model.endpoint_ref != sample.get::<Uuid, _>("endpoint_ref")
        || model.connection_version_ref != sample.get::<Uuid, _>("connection_version_ref")
        || model.model_id != sample.get::<String, _>("model_id")
        || model.limits.attempts != 1
        || model.limits.timeout_seconds > u64::from(settings.timeout_seconds)
    {
        return Err(FoundationError::AdmissionDenied(vec!["audit_model_changed".into()]).into());
    }
    let probe = sample.get::<String, _>("pool") == "probe";
    let (image, image_context) = if probe {
        (source::probe_image(), None)
    } else {
        let source = source::load_in(&mut tx, sample.get::<Uuid, _>("reading_ref"))
            .await?
            .ok_or_else(|| {
                FoundationError::AdmissionDenied(vec!["audit_source_unavailable".into()])
            })?;
        if source.image.blob_sha256 != sample.get::<String, _>("blob_sha256") {
            return Err(ExecutorError::Invalid);
        }
        source.validate_snapshot(&sample.get::<serde_json::Value, _>("local_snapshot"))?;
        if source.rule_version != sample.get::<String, _>("local_rule_version") {
            return Err(ExecutorError::Invalid);
        }
        let image = source::read_image(&source).map_err(|_| {
            FoundationError::AdmissionDenied(vec!["audit_source_unavailable".into()])
        })?;
        (image, Some((source.image_role, source.sequence_ordinal)))
    };
    let workspace: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    if secrets.is_synthetic()
        && !(model.local_endpoint && model.base_url.starts_with("http://127.0.0.1:"))
    {
        return Err(
            FoundationError::AdmissionDenied(vec!["synthetic_endpoint_required".into()]).into(),
        );
    }
    let (system, prompt) = audit::prompt_for_image(image_context);
    let mut request = model.request(
        secrets.get(workspace, model.secret_ref)?,
        system.into(),
        prompt,
        Some(audit::response_schema()),
    );
    request.operation = "analyze_image".into();
    request.images = vec![image];
    request.wire_bytes()?;
    let reservation = model
        .limits
        .input_tokens
        .checked_add(model.limits.output_tokens)
        .ok_or(ExecutorError::Invalid)?;
    let price: Uuid = sample
        .get::<Option<Uuid>, _>("price_ref")
        .ok_or(ExecutorError::Invalid)?;
    let rates=sqlx::query("SELECT input_micro_usd_per_million,output_micro_usd_per_million FROM linggan_model_price WHERE price_ref=$1 AND endpoint_ref=$2 AND identity_ref=$3").bind(price).bind(model.endpoint_ref).bind(model.identity_ref).fetch_one(&mut *tx).await?;
    let input: u128 = rates
        .get::<Option<i64>, _>("input_micro_usd_per_million")
        .ok_or_else(|| FoundationError::AdmissionDenied(vec!["audit_price_unknown".into()]))?
        as u128;
    let output: u128 = rates
        .get::<Option<i64>, _>("output_micro_usd_per_million")
        .ok_or_else(|| FoundationError::AdmissionDenied(vec!["audit_price_unknown".into()]))?
        as u128;
    let cost = (u128::from(model.limits.input_tokens) * input
        + u128::from(model.limits.output_tokens) * output)
        .div_ceil(1_000_000);
    let held=sqlx::query("SELECT count(*)::bigint AS calls,COALESCE(sum(d.reserved_tokens),0)::text AS tokens,COALESCE(sum(d.reserved_cost_micro_usd),0)::text AS cost,count(*) FILTER(WHERE i.state='running')::bigint AS running FROM linggan_ocr_audit_dispatch d JOIN linggan_ocr_audit_sample s USING(sample_ref) JOIN linggan_model_invocation i USING(invocation_ref) WHERE s.run_ref=$1").bind(sample.get::<Uuid,_>("run_ref")).fetch_one(&mut *tx).await?;
    if held.get::<i64, _>("running") >= i64::from(settings.concurrency) {
        return Err(FoundationError::AdmissionDenied(vec!["audit_concurrency_full".into()]).into());
    }
    let material = if probe {
        "synthetic_image_probe"
    } else {
        "ocr_quality_image"
    };
    let egress:Uuid=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key='ocr_quality_audit' AND material_class=$2 AND allowed").bind(model.endpoint_ref).bind(material).fetch_one(&mut *tx).await?;
    let admitted =
        super::budget::admit_in(&mut tx, item.task_key, item.task_config_ref, reservation).await?;
    let invocation = Uuid::new_v4();
    let hash = content_hash(
        &json!([
            audit::CONTRACT,
            audit::PROMPT_VERSION,
            item.item_ref,
            sample_ref,
            item.task_config_ref,
            model.connection_version_ref,
            sample.get::<String, _>("blob_sha256")
        ])
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,task_config_ref,batch_ref,round_no,quota_bucket,classification_source,dispatch_admitted_at,deadline_at,egress_event_ref,result,created_at,price_ref) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,'ocr_quality_audit',$7,$8,$9,'task_day','producer_v1',$10::text::timestamptz,$10::text::timestamptz+make_interval(secs=>$11),$12,$13,$10::text::timestamptz,$14)")
 .bind(invocation).bind(model.connection_version_ref).bind(model.current_model_ref).bind(cfg).bind(hash).bind(reservation as i64).bind(item.task_config_ref).bind(item.batch_ref).bind((item.position/linggan_contracts::ai_limits::AI_LIMITS.round_size+1) as i32).bind(admitted).bind(request.timeout_ms as f64/1000.0+10.0).bind(egress).bind(json!({"producer":"ai-gen1-executor.v1","purpose":"ocr_quality_audit","promptVersion":audit::PROMPT_VERSION,"sampleRef":sample_ref,"visualProbe":probe})).bind(price).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_ocr_audit_dispatch(sample_ref,invocation_ref,reserved_tokens,reserved_cost_micro_usd) VALUES($1,$2,$3,$4)").bind(sample_ref).bind(invocation).bind(reservation as i64).bind(i64::try_from(cost).map_err(|_|ExecutorError::Invalid)?).execute(&mut *tx).await?;
    let changed=sqlx::query("UPDATE linggan_ai_batch_item_control SET invocation_ref=$2,attempt_count=attempt_count+1,executed_config_ref=$3,execution_contract=$4 WHERE item_ref=$1 AND attempt_count=0").bind(item.item_ref).bind(invocation).bind(item.task_config_ref).bind(audit::CONTRACT).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ExecutorError::FenceLost);
    }
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        sample: sample_ref,
        probe,
    })
}
pub async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let p = match prepare(db, item, secrets).await {
        Ok(p) => p,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            let reason = reasons
                .first()
                .map(String::as_str)
                .unwrap_or("admission_denied");
            let terminal = match reason {
                "audit_run_budget_exhausted" => Some("audit_run_budget_exhausted"),
                "audit_source_unavailable" => Some("audit_source_unavailable"),
                "audit_privacy_unreviewed" => Some("audit_privacy_unreviewed"),
                _ => None,
            };
            if let Some(code) = terminal {
                item.skip_unattempted(db, code).await?;
            } else {
                item.defer(db, reason).await?;
            }
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
    let parsed = response.filter(|r| r.ok).and_then(|r| {
        audit::parse_visual(
            r.text.as_deref()?,
            r.diagnostic.as_ref()?.finish_reason.as_deref(),
        )
        .ok()
    });
    let source = if p.probe {
        None
    } else {
        let id: Uuid = sqlx::query_scalar(
            "SELECT reading_ref FROM linggan_ocr_audit_sample WHERE sample_ref=$1",
        )
        .bind(p.sample)
        .fetch_one(&mut *tx)
        .await?;
        source::load_in(&mut tx, id).await?
    };
    let permission = admission::check_queue_input_scope(
        &mut tx,
        item.task_key,
        item.task_config_ref,
        &item.input,
    )
    .await;
    let controls =
        admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
            .await;
    let allowed =
        !cancelled && permission.is_ok() && controls.is_ok() && (p.probe || source.is_some());
    let qualified = p.probe && parsed.as_ref().is_some_and(audit::qualifies_probe);
    let success = parsed.is_some() && allowed && (!p.probe || qualified);
    let failure = if success {
        None
    } else {
        Some(if !allowed {
            "audit_publication_withheld"
        } else if p.probe && parsed.is_some() && !qualified {
            "audit_visual_probe_failed"
        } else if response.is_some_and(|r| r.ok) {
            "audit_visual_schema_invalid"
        } else {
            "audit_provider_failed"
        })
    };
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["visionQualified"] = json!(success && qualified);
    result["sampleRef"] = json!(p.sample);
    // Provider text remains only in this protected opinion row, never ordinary invocation logs.
    sqlx::query("INSERT INTO linggan_ocr_audit_observation(sample_ref,invocation_ref,result,vision_qualified,failure_code) VALUES($1,$2,$3,$4,$5)").bind(p.sample).bind(p.invocation).bind(if success{parsed.as_ref().map(|v|json!(v))}else{None}).bind(success&&qualified).bind(failure).execute(&mut *tx).await?;
    if let (true, Some(source), Some(v)) = (success, source, parsed.as_ref()) {
        let snapshot: serde_json::Value = sqlx::query_scalar(
            "SELECT local_snapshot FROM linggan_ocr_audit_sample WHERE sample_ref=$1",
        )
        .bind(p.sample)
        .fetch_one(&mut *tx)
        .await?;
        for (n, d) in audit::difference::compare(&source, v, &snapshot)?
            .iter()
            .enumerate()
        {
            sqlx::query("INSERT INTO linggan_ocr_audit_issue(issue_ref,sample_ref,position,suggestion) VALUES($1,$2,$3,$4)").bind(Uuid::new_v4()).bind(p.sample).bind(n as i32).bind(json!(d)).execute(&mut *tx).await?;
        }
    }
    receipt::observe_in(&mut tx, p.invocation, response).await?;
    finish_invocation_in(&mut tx, p.invocation, response, success, failure, &result).await?;
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: Some(&json!({"sampleRef":p.sample,"visionQualified":success&&qualified})),
            failure,
            counts_as_failure: !success && !cancelled,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
