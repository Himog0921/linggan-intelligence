use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        admission, batch_store::InputRef, billing_pause, content_understanding as owner,
        store::FoundationError, transport::ResponseSchema,
    },
    content_understanding::{self as contract, Generation, METHOD},
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::{Acquire, Row};
use uuid::Uuid;

mod image_readings;

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
    if owner::prior_dispatch_unresolved_in(&mut tx, &packet).await? {
        return Err(
            FoundationError::AdmissionDenied(vec!["ai_prior_dispatch_unresolved".into()]).into(),
        );
    }
    let images = owner::images_in(&mut tx, &packet).await?;
    let output_limit: i32 = sqlx::query_scalar("SELECT model.output_token_limit FROM linggan_ai_task_config task JOIN linggan_model_config model ON model.config_ref=task.model_config_ref WHERE task.task_config_ref=$1")
        .bind(item.task_config_ref).fetch_one(&mut *tx).await?;
    let schema = contract::response_schema_for_images(
        images.len(),
        usize::try_from(output_limit).map_err(|_| ExecutorError::Invalid)?,
    );
    let mut payload = owner::payload(&packet);
    payload["resultSchema"] = schema.clone();
    let call = super::research_call::prepare_with_images_in(
        &mut tx,
        item,
        secrets,
        super::research_call::Input {
            contract: METHOD,
            reference: request_ref,
            fingerprint: &packet.hash(),
            payload,
            response_schema: ResponseSchema {
                name: "content_understanding".into(),
                schema,
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
            if matches!(
                reason,
                "ai_source_unavailable" | "ai_prior_dispatch_unresolved"
            ) {
                item.skip_unattempted(db, reason).await?;
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
    let mut reply_repair = None;
    let parsed = match (response.filter(|r| r.ok), sources.as_ref()) {
        (Some(reply), Ok(packet)) => {
            let complete = reply.diagnostic.as_ref().is_some_and(|d| {
                d.terminal_received == Some(true)
                    && d.finish_reason.as_deref() == Some("stop")
                    && d.limit_kind.is_none()
            });
            let checked =
                contract::reply::parse(reply.text.as_deref().unwrap_or_default(), complete)
                    .and_then(|(draft, repair)| {
                        reply_repair = repair;
                        let readings = image_readings::prepare(&draft, packet);
                        let result = contract::model_draft::accept_model_draft(
                            draft,
                            &packet.snapshot,
                            &generation,
                        )?;
                        Ok((result, readings))
                    });
            match checked {
                Ok(result) => Some(result),
                Err(code) => {
                    validation = Some(code);
                    None
                }
            }
        }
        _ => None,
    };
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
    if let Some(repair) = reply_repair {
        metadata["replyRepair"] = json!(repair);
    }
    metadata["executorReceipt"] = json!(item.lease_token);
    metadata["requestRef"] = json!(prepared.request_ref);
    metadata["promptVersion"] = json!(prepared.prompt_version_ref);
    metadata["draftProtocol"] = json!(contract::model_draft::PROTOCOL);
    metadata["draftGuidance"] = json!(contract::model_draft::GUIDANCE_VERSION);
    sqlx::query("INSERT INTO linggan_content_understanding_result(request_ref,work_ref,material_snapshot_identity,method_version,invocation_ref,result,raw_output,failure_code) SELECT request_ref,work_ref,material_snapshot_identity,method_version,$2,$3,$4,$5 FROM linggan_content_understanding_request WHERE request_ref=$1")
        .bind(prepared.request_ref).bind(prepared.invocation).bind(parsed.as_ref().filter(|_| success).map(|(card, _)| card))
        .bind(if allowed { response.and_then(|r| r.text.as_deref()) } else { None }).bind(failure).execute(&mut *tx).await?;
    if let Some((_, readings)) = parsed.filter(|_| success) {
        match readings {
            Err(code) => metadata["imageReadings"] = json!({"state":"invalid","code":code}),
            Ok(image_batch) if image_batch.readings.is_empty() => {
                let state = if !image_batch.missing.is_empty()
                    || !image_batch.invalid.is_empty()
                    || !image_batch.deferred.is_empty()
                {
                    "incomplete"
                } else if !image_batch.unreadable.is_empty() {
                    "unreadable"
                } else if !image_batch.no_text.is_empty() {
                    "no_text_observed"
                } else {
                    "empty"
                };
                metadata["imageReadings"] = json!({"state":state,"unreadable":image_batch.unreadable,"noTextObserved":image_batch.no_text,"deferred":image_batch.deferred,"missing":image_batch.missing,"invalid":image_batch.invalid});
            }
            Ok(image_batch) => {
                // A failed optional reading must not roll back the paid model reply
                // or its invocation receipt. A savepoint also avoids partial publication.
                let mut image_tx = tx.begin().await?;
                match image_readings::publish(
                    &mut image_tx,
                    prepared.request_ref,
                    image_batch.readings,
                )
                .await
                {
                    Ok(receipts) => {
                        image_tx.commit().await?;
                        let partial = !image_batch.unreadable.is_empty()
                            || !image_batch.no_text.is_empty()
                            || !image_batch.missing.is_empty()
                            || !image_batch.invalid.is_empty()
                            || !image_batch.deferred.is_empty();
                        metadata["imageReadings"] = json!({"state":if partial {"partial"} else {"published"},"receipts":receipts,"unreadable":image_batch.unreadable,"noTextObserved":image_batch.no_text,"deferred":image_batch.deferred,"missing":image_batch.missing,"invalid":image_batch.invalid});
                    }
                    Err(_) => {
                        image_tx.rollback().await?;
                        metadata["imageReadings"] = json!({"state":"failed","code":"image_reading_write_failed","unreadable":image_batch.unreadable,"noTextObserved":image_batch.no_text,"deferred":image_batch.deferred,"missing":image_batch.missing,"invalid":image_batch.invalid});
                    }
                }
            }
        }
    }
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
    if response.is_some_and(|r| {
        !r.ok
            && r.diagnostic.as_ref().is_some_and(|d| {
                d.http_status == Some(402)
                    && d.request_sent == Some(true)
                    && d.response_started == Some(true)
            })
    }) {
        billing_pause::record(&mut tx, prepared.invocation).await?;
    }
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
