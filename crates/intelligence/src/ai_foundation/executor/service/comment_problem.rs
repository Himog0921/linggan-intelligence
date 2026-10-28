//! Creator work analysis uses frozen dossier inputs and the shared execution ledger.
use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        admission, batch_store::InputRef, comment_problem as packet, comment_research as owner,
        store::FoundationError,
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
    stage: u8,
    context: Value,
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::CommentProblemStage {
        request_ref,
        stage_number,
    } = item.input
    else {
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
    let linked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_stage s JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) WHERE s.item_ref=$1 AND s.request_ref=$2 AND s.stage_number=$3 AND b.task_key=$4 AND b.task_config_ref=$5)")
        .bind(item.item_ref).bind(request_ref).bind(stage_number as i32).bind(item.task_key.as_str()).bind(item.task_config_ref).fetch_one(&mut *tx).await?;
    if !linked || item.task_key.comment_stage() != Some(stage_number) {
        return Err(ExecutorError::Invalid);
    }
    let (_, input) = owner::frozen_in(&mut tx, request_ref).await?;
    let packet = packet::prepare(&mut tx, request_ref, stage_number).await?;
    let super::research_call::Call {
        invocation,
        request,
        prompt_version_ref,
    } = super::research_call::prepare_in(
        &mut tx,
        item,
        secrets,
        super::research_call::Input {
            contract: packet::CONTRACT,
            reference: request_ref,
            fingerprint: &input.input_fingerprint,
            payload: json!({"packet":packet.payload}),
            response_schema: crate::ai_foundation::transport::ResponseSchema {
                name: "comment_problem_stage".into(),
                schema: packet.schema,
            },
            material_class: "comment_material",
        },
    )
    .await?;
    sqlx::query("INSERT INTO linggan_comment_problem_dispatch(item_ref,invocation_ref,context) VALUES($1,$2,$3)").bind(item.item_ref).bind(invocation).bind(&packet.context).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        request_ref,
        prompt_version_ref,
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
                "comment_dependency_failed",
                "comment_no_problem_signal",
                "comment_problem_support_pending",
            ]
            .contains(&reason)
            {
                item.skip_unattempted(
                    db,
                    match reason {
                        "ai_source_unavailable" => "ai_source_unavailable",
                        "comment_no_problem_signal" => "comment_no_problem_signal",
                        "comment_problem_support_pending" => "comment_problem_support_pending",
                        _ => "comment_dependency_failed",
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
    let sources = owner::frozen_in(&mut tx, prepared.request_ref).await;
    let mut parsed = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|text| packet::validate(text, prepared.stage, &prepared.context));
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
    if allowed && parsed.is_some() && prepared.stage == 2 {
        sqlx::query("SAVEPOINT comment_acceptance")
            .execute(&mut *tx)
            .await?;
        if let Err(error) = packet::publish_in(
            &mut tx,
            prepared.request_ref,
            prepared.invocation,
            &prepared.context,
        )
        .await
        {
            sqlx::query("ROLLBACK TO SAVEPOINT comment_acceptance")
                .execute(&mut *tx)
                .await?;
            match error {
                FoundationError::Database(e) => return Err(e.into()),
                _ => parsed = None,
            }
        }
    }
    let success = allowed && parsed.is_some();
    let failure =
        super::research_call::failure(response, allowed, success, "comment_publication_withheld");
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["requestRef"] = json!(prepared.request_ref);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    sqlx::query("INSERT INTO linggan_comment_problem_stage_result(item_ref,invocation_ref,output,failure_code) VALUES($1,$2,$3,$4)").bind(item.item_ref).bind(prepared.invocation).bind(if success {parsed.as_ref()}else{None}).bind(failure).execute(&mut *tx).await?;
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
    let safe_output = json!({"requestRef":prepared.request_ref,"methodVersion":packet::CONTRACT});
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
