use super::{
    super::{
        ExecutorError,
        queue::ClaimedItem,
        receipt::{self, ItemResult},
    },
    research_call::{self, Call},
};
use crate::{
    ai_foundation::{
        admission,
        batch_store::InputRef,
        content_understanding::generate_deep::{self, FrozenDeep, METHOD},
        store::FoundationError,
        transport::ResponseSchema,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    request: Uuid,
) -> Result<(FrozenDeep, Call), ExecutorError> {
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
        .await?;
    admission::check_queue_input_scope(&mut tx, item.task_key, item.task_config_ref, &item.input)
        .await?;
    let frozen = FrozenDeep::load(
        &mut tx,
        request,
        item.task_config_ref,
        item.batch_ref,
        item.item_ref,
    )
    .await?;
    let card = frozen
        .card_in(&mut tx)
        .await?
        .ok_or_else(|| FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]))?;
    let payload = frozen.payload(&card);
    let call = research_call::prepare_in(
        &mut tx,
        item,
        secrets,
        research_call::Input {
            contract: METHOD,
            reference: request,
            fingerprint: &content_hash(&payload.to_string()),
            payload,
            response_schema: ResponseSchema {
                name: "content_understanding_deep".into(),
                schema: generate_deep::response_schema(),
            },
            material_class: "note_material",
        },
    )
    .await?;
    tx.commit().await?;
    Ok((frozen, call))
}

pub(super) async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let InputRef::ContentUnderstandingDeep { request_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let (frozen, call) = match prepare(db, item, secrets, request_ref).await {
        Ok(prepared) => prepared,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            let reason = reasons
                .first()
                .map(String::as_str)
                .unwrap_or("admission_denied");
            return if reason == "ai_source_unavailable" {
                item.skip_unattempted(db, "ai_source_unavailable").await
            } else {
                item.defer(db, reason).await
            };
        }
        Err(error) => return super::super::preparation::handle(db, item, error).await,
    };
    let response = super::super::dispatch::call(db, item, adapter, &call.request).await;
    receipt::finalize(db, item, call.invocation, response.as_ref(), || {
        settle(db, item, request_ref, &frozen, &call, response.as_ref())
    })
    .await
}

async fn settle(
    db: &Database,
    item: &ClaimedItem,
    request: Uuid,
    frozen: &FrozenDeep,
    call: &Call,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let (mut tx, cancelled) = receipt::lock_in(db, item, call.invocation, response).await?;
    let mut allowed = !cancelled
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
    let mut accepted = None;
    if allowed {
        frozen.lock_in(&mut tx).await?;
        let card = frozen.card_in(&mut tx).await?;
        allowed = card.is_some();
        accepted = response
            .filter(|r| r.ok)
            .and_then(|r| r.text.as_deref())
            .and_then(|text| Some(frozen.accept(text, card.as_ref()?)));
    }
    let success = accepted.as_ref().is_some_and(Result::is_ok);
    let failure = accepted
        .as_ref()
        .and_then(|result| result.as_ref().err().copied())
        .or_else(|| {
            research_call::failure(
                response,
                allowed,
                success,
                "content_understanding_publication_withheld",
            )
        });
    if let Some(Ok(findings)) = accepted {
        frozen
            .publish_in(&mut tx, request, call.invocation, findings)
            .await?;
    }
    let mut metadata = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    metadata["executorReceipt"] = json!(item.lease_token);
    metadata["requestRef"] = json!(request);
    metadata["promptVersion"] = json!(call.prompt_version_ref);
    receipt::observe_in(&mut tx, call.invocation, response).await?;
    finish_invocation_in(
        &mut tx,
        call.invocation,
        response,
        success,
        failure,
        &metadata,
    )
    .await?;
    let output = json!({"analysisRef":request,"methodVersion":METHOD});
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: success.then_some(&output),
            failure,
            counts_as_failure: !success
                && allowed
                && !receipt::not_dispatched(response)
                && failure != Some(generate_deep::INSUFFICIENT_EVIDENCE),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
