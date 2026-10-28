//! One real writing invocation per claimed item; the domain owns input qualification and adoption.
use super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{TaskKey, admission, batch_store::InputRef, transport::ResponseSchema},
    content_workbench_ai::{self as writing, AiError, execution},
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PI_PROTOCOL, PiAdapter, PiRequest, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

struct Prepared {
    invocation: Uuid,
    prompt_version_ref: Uuid,
    request: PiRequest,
    request_ref: Option<Uuid>,
    input: execution::Input,
}
fn schema() -> ResponseSchema {
    ResponseSchema {
        name: "content_workbench_writing".into(),
        schema: json!({"type":"object","additionalProperties":false,
        "properties":{"title":{"type":["string","null"]},"paragraphs":{"type":"array","maxItems":200,"items":{"type":"object","additionalProperties":false,
            "properties":{"paragraphId":{"type":["string","null"]},"text":{"type":"string"}},"required":["paragraphId","text"]}},
            "pages":{"type":"array","maxItems":30,"items":{"type":"object","additionalProperties":false,"properties":{"pageId":{"type":["string","null"]},"heading":{"type":"string"},"body":{"type":"string"}},"required":["pageId","heading","body"]}}},
        "required":["title","paragraphs","pages"]}),
    }
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    if item.task_key != TaskKey::WritingLegacy {
        return Err(ExecutorError::Invalid);
    }
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(AiError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
        .await?;
    admission::check_queue_input_scope(&mut tx, item.task_key, item.task_config_ref, &item.input)
        .await?;
    let (preview, request_ref, approval) = match item.input {
        InputRef::WritingRequest { request_ref } if item.kind == "interactive" => {
            let row=sqlx::query("SELECT preview_ref,state,task_config_ref,approved_budget_tokens FROM linggan_creation_ai_request
                WHERE request_ref=$1 AND batch_ref=$2 AND item_ref=$3 FOR UPDATE")
                .bind(request_ref).bind(item.batch_ref).bind(item.item_ref).fetch_optional(&mut *tx).await?.ok_or(AiError::NotFound)?;
            if row.get::<String, _>("state") == "cancel_requested" {
                return Err(AiError::AdmissionDenied(vec!["writing_cancelled".into()]).into());
            }
            if row.get::<String, _>("state") != "queued"
                || row.get::<Option<Uuid>, _>("task_config_ref") != Some(item.task_config_ref)
            {
                return Err(AiError::Stale.into());
            }
            (
                row.get::<Uuid, _>("preview_ref"),
                Some(request_ref),
                row.get::<Option<i64>, _>("approved_budget_tokens"),
            )
        }
        InputRef::WritingPreview { preview_ref } if item.kind == "trial" => {
            (preview_ref, None, None)
        }
        _ => return Err(ExecutorError::Invalid),
    };
    let input = execution::load_in(&mut tx, preview).await?;
    if request_ref.is_some() && input.preview_config != item.task_config_ref {
        return Err(AiError::Stale.into());
    }
    let model = execution::model_in(&mut tx, item.task_config_ref).await?;
    let current_model = model.current_model_ref.ok_or(AiError::Model)?;
    let system = execution::system_in(&mut tx, item.task_config_ref).await?;
    let prompt_version_ref: Uuid = sqlx::query_scalar(
        "SELECT prompt_version_ref FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(item.task_config_ref)
    .fetch_one(&mut *tx)
    .await?;
    let prompt = input.payload.to_string();
    let reservation = model
        .limits
        .input_tokens
        .checked_add(model.limits.output_tokens)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(ExecutorError::Invalid)?;
    if request_ref.is_some() && approval.is_none_or(|approved| approved < reservation) {
        return Err(AiError::Invalid.into());
    }
    let workspace: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    if secrets.is_synthetic()
        && !(model.local_endpoint && model.base_url.starts_with("http://127.0.0.1:"))
    {
        return Err(AiError::AdmissionDenied(vec!["synthetic_endpoint_required".into()]).into());
    }
    let request = model.request(
        secrets.get(workspace, model.secret_ref)?,
        system,
        prompt,
        Some(schema()),
    );
    request.wire_bytes()?;
    // Connection share lock from load_frozen_in serializes this final permission read with revoke.
    let egress:Option<Uuid>=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key='writing_legacy' AND material_class='selected_writing_material' AND allowed")
        .bind(model.endpoint_ref).fetch_optional(&mut *tx).await?;
    let egress =
        egress.ok_or_else(|| AiError::AdmissionDenied(vec!["egress_not_approved".into()]))?;
    let admitted = super::budget::admit_in(
        &mut tx,
        item.task_key,
        item.task_config_ref,
        reservation as u64,
    )
    .await?;
    let price =
        super::budget::latest_price_in(&mut tx, Some(current_model), model.endpoint_ref).await?;
    let invocation = Uuid::new_v4();
    let hash = content_hash(
        &json!([
            writing::METHOD,
            item.item_ref,
            item.task_config_ref,
            model.connection_version_ref,
            &input.hash
        ])
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,
        task_key,task_config_ref,batch_ref,round_no,quota_bucket,classification_source,dispatch_admitted_at,deadline_at,egress_event_ref,result,created_at,price_ref)
        VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,'writing_legacy',$7,$8,$9,'task_day','producer_v1',$10::text::timestamptz,
         $10::text::timestamptz+make_interval(secs=>$11),$12,$13,$10::text::timestamptz,$14)")
        .bind(invocation).bind(model.connection_version_ref).bind(current_model).bind(model.config_ref).bind(hash).bind(reservation)
        .bind(item.task_config_ref).bind(item.batch_ref).bind((item.position/AI_LIMITS.round_size+1) as i32).bind(admitted)
        .bind(request.timeout_ms as f64/1000.0+AI_LIMITS.adapter_deadline_allowance_seconds as f64).bind(egress)
        .bind(json!({"producer":"ai-gen1-executor.v1","purpose":"content_workbench","methodVersion":writing::METHOD})).bind(price).execute(&mut *tx).await?;
    let changed=sqlx::query("UPDATE linggan_ai_batch_item_control c SET invocation_ref=$2,attempt_count=attempt_count+1,
        executed_config_ref=cfg.task_config_ref,executed_skill_version_ref=cfg.skill_version_ref,execution_contract=cfg.execution_contract
        FROM linggan_ai_task_config cfg WHERE item_ref=$1 AND cfg.task_config_ref=$3 AND attempt_count=0")
        .bind(item.item_ref).bind(invocation).bind(item.task_config_ref).execute(&mut *tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ExecutorError::FenceLost);
    }
    if let Some(reference) = request_ref {
        let changed=sqlx::query("UPDATE linggan_creation_ai_request SET state='running',invocation_ref=$2,dispatch_started=true WHERE request_ref=$1 AND state='queued'")
            .bind(reference).bind(invocation).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
    }
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        prompt_version_ref,
        request,
        request_ref,
        input,
    })
}
pub async fn execute_writing(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let prepared = match prepare(db, item, secrets).await {
        Ok(prepared) => prepared,
        Err(ExecutorError::Writing(AiError::AdmissionDenied(reasons)))
        | Err(ExecutorError::Configuration(
            crate::ai_foundation::store::FoundationError::AdmissionDenied(reasons),
        )) => {
            if reasons.iter().any(|r| r == "writing_cancelled")
                && let InputRef::WritingRequest { request_ref } = item.input
            {
                writing::cancel(db, request_ref).await?;
            }
            item.defer(
                db,
                reasons
                    .first()
                    .map(String::as_str)
                    .unwrap_or("admission_denied"),
            )
            .await?;
            return Ok(());
        }
        Err(ExecutorError::Writing(AiError::Source | AiError::Stale)) => {
            item.skip_unattempted(db, "ai_source_unavailable").await?;
            return Ok(());
        }
        Err(error) => return super::preparation::handle(db, item, error).await,
    };
    let response = super::dispatch::call(db, item, adapter, &prepared.request).await;
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
    let (mut tx, mut cancelled) = receipt::lock_in(db, item, prepared.invocation, response).await?;
    if let Some(reference) = prepared.request_ref {
        let state: String = sqlx::query_scalar(
            "SELECT state FROM linggan_creation_ai_request WHERE request_ref=$1 FOR UPDATE",
        )
        .bind(reference)
        .fetch_one(&mut *tx)
        .await?;
        cancelled |= state == "cancel_requested";
    }
    let readable = prepared.input.readable_in(&mut tx).await?;
    let suggestion = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|text| prepared.input.validate(text).ok());
    let suggestion = suggestion
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| ExecutorError::Invalid)?
        .filter(|value| value.to_string().len() <= AI_LIMITS.max_text_bytes as usize);
    let provider_ok = response.is_some_and(|r| r.ok);
    let no_send = receipt::not_dispatched(response);
    let failure = if no_send {
        Some("ai_adapter_rejected_before_dispatch")
    } else if response.is_none() {
        Some("ai_dispatch_unknown")
    } else if !provider_ok {
        Some("ai_provider_failed")
    } else if suggestion.is_none() {
        Some("ai_output_invalid")
    } else {
        None
    };
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["adapterVersion"] = json!(PI_PROTOCOL);
    result["executorReceipt"] = json!(item.lease_token);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    result["writingQualified"] = json!(suggestion.is_some());
    result["sourceQualifiedAtSettlement"] = json!(readable);
    receipt::observe_in(&mut tx, prepared.invocation, response).await?;
    finish_invocation_in(
        &mut tx,
        prepared.invocation,
        response,
        failure.is_none(),
        failure,
        &result,
    )
    .await?;
    let usable = suggestion.is_some() && readable && !cancelled;
    let item_failure = if cancelled {
        Some("ai_cancelled_after_dispatch")
    } else if !readable {
        Some("ai_source_unavailable")
    } else {
        failure
    };
    let output = if let Some(reference) = prepared.request_ref {
        sqlx::query("UPDATE linggan_creation_ai_request SET state=$2,suggestion=$3,failure_code=$4,finished_at=scope_001_now() WHERE request_ref=$1")
            .bind(reference).bind(if cancelled {"cancelled"} else if usable {"succeeded"} else {"failed"})
            .bind(if usable {suggestion.as_ref()} else {None}).bind(item_failure).execute(&mut *tx).await?;
        if usable {
            sqlx::query("INSERT INTO linggan_creation_ai_suggestion(suggestion_ref,work_ref,request_ref) VALUES($1,$2,$1)")
                .bind(reference).bind(prepared.input.manifest.work_ref).execute(&mut *tx).await?;
        }
        Some(
            json!({"requestRef":reference,"suggestionRef":if usable {Some(reference)} else {None}}),
        )
    } else {
        suggestion
    };
    if provider_ok {
        sqlx::query("INSERT INTO linggan_creation_writing_qualification(model_ref,connection_version_ref,method_version,invocation_ref,qualified)
            SELECT model_ref,connection_version_ref,$2,invocation_ref,$3 FROM linggan_model_invocation WHERE invocation_ref=$1")
            .bind(prepared.invocation).bind(writing::METHOD).bind(output.is_some()&&failure.is_none()).execute(&mut *tx).await?;
    }
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success: usable,
            cancelled,
            output: output.as_ref(),
            failure: item_failure,
            counts_as_failure: failure.is_some() && !no_send && readable,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
