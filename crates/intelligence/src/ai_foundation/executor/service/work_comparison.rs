//! Content comparison is an ordinary fenced AI item, with source checks before and after dispatch.
use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        TaskKey, admission, batch_store::InputRef, prompt, store::FoundationError, transport_store,
        work_comparison as owner,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    request_ref: Uuid,
    prompt_version_ref: Uuid,
}

async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::WorkComparison { request_ref } = &item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(
        &mut tx,
        TaskKey::WorkComparison,
        item.task_config_ref,
        true,
    )
    .await?;
    admission::check_queue_input_scope(
        &mut tx,
        TaskKey::WorkComparison,
        item.task_config_ref,
        &item.input,
    )
    .await?;
    let linked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_work_comparison_request WHERE request_ref=$1 AND task_config_ref=$2 AND batch_ref=$3 AND method_version=$4)")
        .bind(request_ref).bind(item.task_config_ref).bind(item.batch_ref).bind(owner::CONTRACT).fetch_one(&mut *tx).await?;
    if !linked {
        return Err(ExecutorError::Invalid);
    }
    let materials = owner::frozen_in(&mut tx, *request_ref).await?;
    let cfg: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(item.task_config_ref)
    .fetch_one(&mut *tx)
    .await?;
    let model = transport_store::load_frozen_in(&mut tx, cfg).await?;
    if model.limits.attempts != 1 {
        return Err(FoundationError::AdmissionDenied(vec![
            "comparison_requires_single_attempt".into(),
        ])
        .into());
    }
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
    let (prompt_version_ref, mut system, instruction, skill) =
        prompt::frozen_in(&mut tx, item.task_config_ref, TaskKey::WorkComparison).await?;
    if let Some(skill) = skill {
        system.push_str("\n\n");
        system.push_str(&skill);
    }
    let prompt = owner::prompt(&materials, &instruction);
    let schema = crate::ai_foundation::transport::ResponseSchema {
        name: "work_content_comparison".into(),
        schema: owner::response_schema(),
    };
    let request = model.request(
        secrets.get(workspace, model.secret_ref)?,
        system,
        prompt,
        Some(schema),
    );
    request.wire_bytes()?;
    let reservation = model
        .limits
        .input_tokens
        .checked_add(model.limits.output_tokens)
        .ok_or(ExecutorError::Invalid)?;
    let egress:Uuid=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key='work_comparison' AND material_class='note_material' AND allowed")
        .bind(model.endpoint_ref).fetch_one(&mut *tx).await?;
    let admitted =
        super::super::budget::admit_in(&mut tx, item.task_key, item.task_config_ref, reservation)
            .await?;
    let invocation = Uuid::new_v4();
    let hash = content_hash(
        &json!([
            owner::CONTRACT,
            item.item_ref,
            request_ref,
            item.task_config_ref,
            model.connection_version_ref,
            owner::fingerprint(&materials)
        ])
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,task_config_ref,batch_ref,round_no,quota_bucket,classification_source,dispatch_admitted_at,deadline_at,egress_event_ref,result,created_at) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,'work_comparison',$7,$8,$9,'task_day','producer_v1',$10::text::timestamptz,$10::text::timestamptz+make_interval(secs=>$11),$12,$13,$10::text::timestamptz)")
        .bind(invocation).bind(model.connection_version_ref).bind(model.current_model_ref).bind(cfg).bind(hash).bind(reservation as i64).bind(item.task_config_ref).bind(item.batch_ref)
        .bind((item.position/linggan_contracts::ai_limits::AI_LIMITS.round_size+1) as i32).bind(admitted).bind(request.timeout_ms as f64/1000.0+10.0)
        .bind(egress).bind(json!({"producer":"ai-gen1-executor.v1","purpose":"work_comparison","requestRef":request_ref,"methodVersion":owner::CONTRACT,"promptVersion":prompt_version_ref})).execute(&mut *tx).await?;
    let changed=sqlx::query("UPDATE linggan_ai_batch_item_control SET invocation_ref=$2,attempt_count=attempt_count+1,executed_config_ref=$3,executed_skill_version_ref=(SELECT skill_version_ref FROM linggan_ai_task_config WHERE task_config_ref=$3),execution_contract=$4 WHERE item_ref=$1 AND attempt_count=0")
        .bind(item.item_ref).bind(invocation).bind(item.task_config_ref).bind(owner::CONTRACT).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ExecutorError::FenceLost);
    }
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        request_ref: *request_ref,
        prompt_version_ref,
    })
}

pub(super) async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let prepared = match prepare(db, item, secrets).await {
        Ok(value) => value,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            let reason = reasons
                .first()
                .map(String::as_str)
                .unwrap_or("admission_denied");
            match reason {
                "comparison_source_unavailable" => {
                    item.skip_unattempted(db, "comparison_source_unavailable")
                        .await?
                }
                "comparison_source_changed" => {
                    item.skip_unattempted(db, "comparison_source_changed")
                        .await?
                }
                _ => item.defer(db, reason).await?,
            }
            return Ok(());
        }
        Err(error) => return super::super::preparation::handle(db, item, error).await,
    };
    let response = super::super::dispatch::call(db, item, adapter, &prepared.request).await;
    receipt::finalize(db, item, prepared.invocation, response.as_ref(), || {
        settle(db, item, &prepared, response.as_ref())
    })
    .await
}

async fn settle(
    db: &Database,
    item: &ClaimedItem,
    prepared: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let (mut tx, cancelled) = receipt::lock_in(db, item, prepared.invocation, response).await?;
    let sources = owner::frozen_in(&mut tx, prepared.request_ref).await;
    let parsed = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|text| {
            sources
                .as_ref()
                .ok()
                .and_then(|materials| owner::validate_analysis(text, materials).ok())
        });
    let allowed = !cancelled
        && sources.is_ok()
        && admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
            .await
            .is_ok()
        && admission::check_queue_input_scope(
            &mut tx,
            item.task_key,
            item.task_config_ref,
            &item.input,
        )
        .await
        .is_ok();
    let success = allowed && parsed.is_some();
    let failure = if success {
        None
    } else {
        Some(if !allowed {
            "comparison_publication_withheld"
        } else if response.is_some_and(|r| r.ok) {
            "comparison_schema_invalid"
        } else {
            "comparison_provider_failed"
        })
    };
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["requestRef"] = json!(prepared.request_ref);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    sqlx::query("INSERT INTO linggan_work_comparison_result(request_ref,invocation_ref,result,failure_code) VALUES($1,$2,$3,$4)")
        .bind(prepared.request_ref).bind(prepared.invocation).bind(if success{parsed.as_ref().map(|a|json!(a))}else{None}).bind(failure).execute(&mut *tx).await?;
    receipt::observe_in(&mut tx, prepared.invocation, response).await?;
    finish_invocation_in(
        &mut tx,
        prepared.invocation,
        response,
        success,
        failure,
        &result,
    )
    .await?;
    let safe_output = json!({"requestRef":prepared.request_ref,"methodVersion":owner::CONTRACT});
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: success.then_some(&safe_output),
            failure,
            counts_as_failure: !success && !cancelled && !receipt::not_dispatched(response),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
