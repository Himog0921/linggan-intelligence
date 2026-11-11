use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        admission, batch_store::InputRef, content_understanding as owner, store::FoundationError,
        transport::ResponseSchema,
    },
    content_understanding::{self as contract, Generation, METHOD},
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
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
    let InputRef::ContentUnderstanding { request_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
        .await?;
    admission::check_queue_input_scope(&mut tx, item.task_key, item.task_config_ref, &item.input)
        .await?;
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_content_understanding_request WHERE request_ref=$1 AND task_config_ref=$2 AND batch_ref=$3 AND item_ref=$4 AND method_version=$5)")
        .bind(request_ref).bind(item.task_config_ref).bind(item.batch_ref).bind(item.item_ref).bind(METHOD).fetch_one(&mut *tx).await?;
    if !linked {
        return Err(ExecutorError::Invalid);
    }
    let packet = owner::frozen_in(&mut tx, request_ref).await?;
    let images = owner::images_in(&mut tx, &packet).await?;
    let call = super::research_call::prepare_with_images_in(
        &mut tx,
        item,
        secrets,
        super::research_call::Input {
            contract: METHOD,
            reference: request_ref,
            fingerprint: &packet.hash(),
            payload: owner::payload(&packet),
            response_schema: ResponseSchema {
                name: "content_understanding".into(),
                schema: contract::response_schema(),
            },
            material_class: "note_material",
        },
        images,
    )
    .await?;
    tx.commit().await?;
    Ok(Prepared {
        invocation: call.invocation,
        request: call.request,
        request_ref,
        prompt_version_ref: call.prompt_version_ref,
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
            if reason == "ai_source_unavailable" {
                item.skip_unattempted(db, "ai_source_unavailable").await?;
            } else {
                item.defer(db, reason).await?;
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
    let versions = sqlx::query("SELECT model_config_ref,skill_version_ref,to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at FROM linggan_ai_task_config WHERE task_config_ref=$1")
        .bind(item.task_config_ref).fetch_one(&mut *tx).await?;
    let provider_model = response
        .and_then(|r| r.diagnostic.as_ref())
        .and_then(|d| d.provider_returned_model.clone());
    let generation = Generation {
        analysis_ref: prepared.request_ref,
        skill_version_ref: versions
            .get::<Option<Uuid>, _>("skill_version_ref")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".into()),
        prompt_version_ref: prepared.prompt_version_ref.to_string(),
        model_config_ref: versions.get::<Uuid, _>("model_config_ref").to_string(),
        provider_returned_model: provider_model.unwrap_or_default(),
        created_at: versions.get("created_at"),
    };
    let mut validation = None;
    let parsed = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|draft| {
            sources.as_ref().ok().and_then(|packet| {
                match contract::accept(draft, &packet.snapshot, &generation) {
                    Ok(result) => Some(result),
                    Err(code) => {
                        validation = Some(code);
                        None
                    }
                }
            })
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
    let failure = super::research_call::failure(
        response,
        allowed,
        success,
        "content_understanding_publication_withheld",
    );
    let mut metadata = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    if let Some(code) = validation {
        metadata["validationCode"] = json!(code);
    }
    metadata["executorReceipt"] = json!(item.lease_token);
    metadata["requestRef"] = json!(prepared.request_ref);
    metadata["promptVersion"] = json!(prepared.prompt_version_ref);
    sqlx::query("INSERT INTO linggan_content_understanding_result(request_ref,work_ref,material_snapshot_identity,method_version,invocation_ref,result,raw_output,failure_code) SELECT request_ref,work_ref,material_snapshot_identity,method_version,$2,$3,$4,$5 FROM linggan_content_understanding_request WHERE request_ref=$1")
        .bind(prepared.request_ref).bind(prepared.invocation).bind(if success { parsed.as_ref() } else { None })
        .bind(if allowed { response.and_then(|r| r.text.as_deref()) } else { None }).bind(failure).execute(&mut *tx).await?;
    receipt::observe_in(&mut tx, prepared.invocation, response).await?;
    finish_invocation_in(
        &mut tx,
        prepared.invocation,
        response,
        success,
        failure,
        &metadata,
    )
    .await?;
    let output = json!({"requestRef":prepared.request_ref,"methodVersion":METHOD});
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: success.then_some(&output),
            failure,
            counts_as_failure: !success && allowed && !receipt::not_dispatched(response),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
