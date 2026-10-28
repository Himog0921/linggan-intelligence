//! Creator work analysis uses frozen dossier inputs and the shared execution ledger.
use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        TaskKey, admission, batch_store::InputRef, creator_account_input,
        creator_account_research as owner, store::FoundationError,
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
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::CreatorAccountResearch { request_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(
        &mut tx,
        TaskKey::CreatorAccountResearch,
        item.task_config_ref,
        true,
    )
    .await?;
    admission::check_queue_input_scope(
        &mut tx,
        TaskKey::CreatorAccountResearch,
        item.task_config_ref,
        &item.input,
    )
    .await?;
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creator_account_research_request WHERE request_ref=$1 AND task_config_ref=$2 AND batch_ref=$3 AND method_version=$4)")
        .bind(request_ref).bind(item.task_config_ref).bind(item.batch_ref).bind(owner::CONTRACT).fetch_one(&mut *tx).await?;
    if !linked {
        return Err(ExecutorError::Invalid);
    }
    let data = owner::frozen_in(&mut tx, request_ref).await?;
    let input = creator_account_input::prepare(&data);
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
            fingerprint: &linggan_evidence::creator_discovery::account_research_fingerprint(
                &data.works.iter().collect::<Vec<_>>(),
            ),
            payload: json!({"input":input.packet}),
            response_schema: crate::ai_foundation::transport::ResponseSchema {
                name: "creator_account_research".into(),
                schema: creator_account_input::schema(),
            },
            material_class: "note_material",
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        request_ref,
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
    let parsed = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| {
            sources.as_ref().ok().and_then(|d| {
                creator_account_input::validate(&v, &creator_account_input::prepare(d))
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
    let failure =
        super::research_call::failure(response, allowed, success, "creator_publication_withheld");
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["requestRef"] = json!(prepared.request_ref);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    sqlx::query("INSERT INTO linggan_creator_account_research_result(request_ref,invocation_ref,result,failure_code) VALUES($1,$2,$3,$4)")
        .bind(prepared.request_ref).bind(prepared.invocation).bind(if success {parsed.as_ref()} else {None}).bind(failure).execute(&mut *tx).await?;
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
