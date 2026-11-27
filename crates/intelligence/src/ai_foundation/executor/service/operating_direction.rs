//! Direction proposals are committed with their receipt and stay outside the published catalog.
use super::{
    super::{
        ExecutorError,
        queue::ClaimedItem,
        receipt::{self, ItemResult},
    },
    research_call,
};
use crate::{
    ai_foundation::operating_direction::{self as direction, Packet},
    ai_foundation::{
        admission, batch_store::InputRef, store::FoundationError, transport::ResponseSchema,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

struct Prepared {
    call: research_call::Call,
    run: Uuid,
    packet: Packet,
}

async fn permitted(
    tx: &mut Transaction<'_, Postgres>,
    item: &ClaimedItem,
) -> Result<(), FoundationError> {
    admission::check_queue_configuration(tx, item.task_key, item.task_config_ref, false).await?;
    admission::check_queue_input_scope(tx, item.task_key, item.task_config_ref, &item.input).await
}

async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Option<Prepared>, ExecutorError> {
    let InputRef::OperatingDirection { run_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Ok(None);
    }
    permitted(&mut tx, item).await?;
    let sqlx::types::Json(packet)=sqlx::query_scalar::<_,sqlx::types::Json<Packet>>(
        "SELECT r.frozen_input FROM linggan_operating_direction_run r JOIN linggan_ai_batch b USING(batch_ref) WHERE r.run_ref=$1 AND r.batch_ref=$2 AND b.task_config_ref=$3"
    ).bind(run_ref).bind(item.batch_ref).bind(item.task_config_ref).fetch_one(&mut *tx).await?;
    direction::check_source(&mut tx, &packet, item.task_config_ref).await?;
    let payload = packet.prompt_input();
    let fingerprint = content_hash(&payload.to_string());
    let call = research_call::prepare_in(
        &mut tx,
        item,
        secrets,
        research_call::Input {
            contract: direction::CONTRACT,
            reference: run_ref,
            fingerprint: &fingerprint,
            payload,
            response_schema: ResponseSchema {
                name: "operating_direction".into(),
                schema: direction::schema(),
            },
            material_class: "note_material",
        },
    )
    .await?;
    tx.commit().await?;
    Ok(Some(Prepared {
        call,
        run: run_ref,
        packet,
    }))
}

pub(super) async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let prepared = match prepare(db, item, secrets).await {
        Ok(Some(p)) => p,
        Ok(None) => return item.skip_unattempted(db, "batch_cancelled").await,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            if reasons.iter().any(|r| r.starts_with("direction_source_")) {
                return item.skip_unattempted(db, "ai_source_unavailable").await;
            }
            return item
                .defer(
                    db,
                    reasons.first().map_or("admission_denied", String::as_str),
                )
                .await;
        }
        Err(e) => return super::super::preparation::handle(db, item, e).await,
    };
    let response = super::super::dispatch::call(db, item, adapter, &prepared.call.request).await;
    receipt::finalize(
        db,
        item,
        prepared.call.invocation,
        response.as_ref(),
        || settle(db, item, &prepared, response.as_ref()),
    )
    .await
}

async fn settle(
    db: &Database,
    item: &ClaimedItem,
    p: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let invocation = p.call.invocation;
    let (mut tx, cancelled) = receipt::lock_in(db, item, invocation, response).await?;
    let allowed = if cancelled {
        false
    } else {
        match async {
            permitted(&mut tx, item).await?;
            direction::check_source(&mut tx, &p.packet, item.task_config_ref).await
        }
        .await
        {
            Ok(()) => true,
            Err(FoundationError::AdmissionDenied(_)) => false,
            Err(e) => return Err(e.into()),
        }
    };
    let accepted = response
        .filter(|r| r.ok && allowed)
        .and_then(|r| r.text.as_deref())
        .and_then(|text| direction::accepted(text, &p.packet));
    sqlx::query("UPDATE linggan_operating_direction_run SET invocation_ref=$2,candidates=$3 WHERE run_ref=$1")
        .bind(p.run).bind(invocation).bind(accepted.as_ref().map(sqlx::types::Json)).execute(&mut *tx).await?;
    let success = accepted.is_some();
    let failure = research_call::failure(response, allowed, success, "ai_source_unavailable");
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    receipt::observe_in(&mut tx, invocation, response).await?;
    finish_invocation_in(&mut tx, invocation, response, success, failure, &result).await?;
    let output = json!({"runRef":p.run,"candidateCount":accepted.as_ref().map(Vec::len)});
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
