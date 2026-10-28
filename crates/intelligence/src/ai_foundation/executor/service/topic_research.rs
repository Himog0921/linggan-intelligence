//! Creator work analysis uses frozen dossier inputs and the shared execution ledger.
use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        admission, batch_store::InputRef, store::FoundationError, topic_research as owner,
        topic_research_packet as packet,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    request_ref: Uuid,
    prompt_version_ref: Uuid,
    window: Uuid,
    stage: u8,
    context: Value,
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::TopicResearch {
        window_ref: request_ref,
        stage_number,
    } = item.input
    else {
        return Err(ExecutorError::Invalid);
    };
    let data = owner::frozen(db, request_ref).await?;
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
        .await?;
    admission::check_queue_input_scope(&mut tx, item.task_key, item.task_config_ref, &item.input)
        .await?;
    let linked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) WHERE s.item_ref=$1 AND s.window_ref=$2 AND s.stage_number=$3 AND b.task_key=$4 AND b.task_config_ref=$5)")
        .bind(item.item_ref).bind(request_ref).bind(stage_number as i32).bind(item.task_key.as_str()).bind(item.task_config_ref).fetch_one(&mut *tx).await?;
    if !linked || item.task_key.topic_stage() != Some(stage_number) {
        return Err(ExecutorError::Invalid);
    }
    if stage_number > 0 {
        owner::previous_in(&mut tx, request_ref, stage_number - 1).await?;
    }
    let packet = packet::prepare(db, &mut tx, request_ref, stage_number, &data).await?;
    let super::research_call::Call {
        invocation,
        request,
        prompt_version_ref,
    } = super::research_call::prepare_in(
        &mut tx,
        item,
        secrets,
        super::research_call::Input {
            contract: owner::CONTRACT,
            reference: request_ref,
            fingerprint: &data.hash,
            payload: json!({"packet":packet.payload}),
            response_schema: crate::ai_foundation::transport::ResponseSchema {
                name: "topic_research_stage".into(),
                schema: packet.schema,
            },
            material_class: "note_material",
        },
    )
    .await?;
    sqlx::query("INSERT INTO linggan_topic_research_dispatch(item_ref,invocation_ref,context) VALUES($1,$2,$3)").bind(item.item_ref).bind(invocation).bind(&packet.context).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        request_ref,
        prompt_version_ref,
        window: request_ref,
        stage: stage_number,
        context: packet.context,
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
            if [
                "ai_source_unavailable",
                "topic_dependency_failed",
                "topic_assignment_not_required",
            ]
            .contains(&reason)
            {
                item.skip_unattempted(
                    db,
                    match reason {
                        "ai_source_unavailable" => "ai_source_unavailable",
                        "topic_assignment_not_required" => "topic_assignment_not_required",
                        _ => "topic_dependency_failed",
                    },
                )
                .await?;
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
    let sources = owner::frozen(db, prepared.window).await;
    if let Err(crate::ai_foundation::store::FoundationError::Database(_)) = &sources {
        return Err(sources.err().unwrap().into());
    }
    let draft = if prepared.stage >= 2 {
        Some(packet::draft_in(&mut tx, prepared.window).await?)
    } else {
        None
    };
    let validation = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|text| {
            sources.as_ref().ok().map(|source| {
                packet::validate(
                    text,
                    prepared.stage,
                    source,
                    &prepared.context,
                    draft.as_ref(),
                )
            })
        });
    let validation_reason = validation
        .as_ref()
        .and_then(|value| value.as_ref().err())
        .copied();
    let mut parsed = validation.and_then(Result::ok);
    let candidate_sources_ok = if prepared.stage >= 2 {
        match sources.as_ref() {
            Ok(input) => {
                match packet::candidate_sources_current(db, input, &prepared.context).await {
                    Ok(()) => true,
                    Err(crate::ai_foundation::store::FoundationError::Database(e)) => {
                        return Err(e.into());
                    }
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    } else {
        true
    };
    let allowed = !cancelled
        && candidate_sources_ok
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
    if allowed && parsed.is_some() && (prepared.stage == 1 || prepared.stage == 3) {
        sqlx::query("SAVEPOINT topic_acceptance")
            .execute(&mut *tx)
            .await?;
        if let Err(error) = packet::publish_in(
            &mut tx,
            prepared.window,
            prepared.stage,
            prepared.invocation,
            sources.as_ref().map_err(|_| ExecutorError::Invalid)?,
            &prepared.context,
        )
        .await
        {
            sqlx::query("ROLLBACK TO SAVEPOINT topic_acceptance")
                .execute(&mut *tx)
                .await?;
            match error {
                crate::model_settings::ModelError::Database(e) => return Err(e.into()),
                _ => parsed = None,
            }
        }
    }
    let success = allowed && parsed.is_some();
    let failure =
        super::research_call::failure(response, allowed, success, "topic_publication_withheld");
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["validationReason"] = json!(validation_reason);
    result["executorReceipt"] = json!(item.lease_token);
    result["requestRef"] = json!(prepared.request_ref);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    sqlx::query("INSERT INTO linggan_topic_research_stage_result(item_ref,invocation_ref,output,failure_code) VALUES($1,$2,$3,$4)")
        .bind(item.item_ref).bind(prepared.invocation).bind(if success {parsed.as_ref()}else{None}).bind(failure).execute(&mut *tx).await?;
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
            counts_as_failure: !success && allowed && !receipt::not_dispatched(response),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
