//! Physical AI call preparation. Business modules own inputs, phases and publication rules.
use super::super::{ExecutorError, budget, queue::ClaimedItem};
use crate::{
    ai_foundation::{prompt, store::FoundationError, transport::ResponseSchema, transport_store},
    model_secrets::ModelSecretStore,
    pi_adapter::PiRequest,
    research_text::content_hash,
};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub(super) struct Input<'a> {
    pub contract: &'static str,
    pub reference: Uuid,
    pub fingerprint: &'a str,
    pub payload: Value,
    pub response_schema: ResponseSchema,
    pub material_class: &'static str,
}
pub(super) struct Call {
    pub invocation: Uuid,
    pub request: PiRequest,
    pub prompt_version_ref: Uuid,
}
pub(super) async fn prepare_in(
    tx: &mut Transaction<'_, Postgres>,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    mut input: Input<'_>,
) -> Result<Call, ExecutorError> {
    let cfg: Uuid = sqlx::query_scalar(
        "SELECT model_config_ref FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(item.task_config_ref)
    .fetch_one(&mut **tx)
    .await?;
    let model = transport_store::load_frozen_in(tx, cfg).await?;
    if model.limits.attempts != 1 {
        return Err(FoundationError::AdmissionDenied(vec![
            "research_requires_single_attempt".into(),
        ])
        .into());
    }
    if secrets.is_synthetic()
        && !(model.local_endpoint && model.base_url.starts_with("http://127.0.0.1:"))
    {
        return Err(
            FoundationError::AdmissionDenied(vec!["synthetic_endpoint_required".into()]).into(),
        );
    }
    let workspace: Uuid =
        sqlx::query_scalar("SELECT workspace_ref FROM linggan_model_workspace WHERE singleton")
            .fetch_one(&mut **tx)
            .await?;
    let (prompt_version_ref, mut system, instruction, skill) =
        prompt::frozen_in(tx, item.task_config_ref, item.task_key).await?;
    if let Some(skill) = skill {
        system.push_str("\n\n");
        system.push_str(&skill);
    }
    input
        .payload
        .as_object_mut()
        .ok_or(ExecutorError::Invalid)?
        .insert("instruction".into(), json!(instruction));
    let request = model.request(
        secrets.get(workspace, model.secret_ref)?,
        system,
        input.payload.to_string(),
        Some(input.response_schema),
    );
    request.wire_bytes()?;
    let reservation = model
        .limits
        .input_tokens
        .checked_add(model.limits.output_tokens)
        .ok_or(ExecutorError::Invalid)?;
    let egress:Uuid=sqlx::query_scalar("SELECT event_ref FROM linggan_ai_current_egress WHERE endpoint_ref=$1 AND task_key=$2 AND material_class=$3 AND allowed").bind(model.endpoint_ref).bind(item.task_key.as_str()).bind(input.material_class).fetch_one(&mut **tx).await?;
    let admitted = budget::admit_in(tx, item.task_key, item.task_config_ref, reservation).await?;
    let invocation = Uuid::new_v4();
    let hash = content_hash(
        &json!([
            input.contract,
            item.item_ref,
            input.reference,
            item.task_config_ref,
            model.connection_version_ref,
            input.fingerprint
        ])
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,task_config_ref,batch_ref,round_no,quota_bucket,classification_source,dispatch_admitted_at,deadline_at,egress_event_ref,result,created_at) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$14,$7,$8,$9,'task_day','producer_v1',$10::text::timestamptz,$10::text::timestamptz+make_interval(secs=>$11),$12,$13,$10::text::timestamptz)")
 .bind(invocation).bind(model.connection_version_ref).bind(model.current_model_ref).bind(cfg).bind(hash).bind(reservation as i64).bind(item.task_config_ref).bind(item.batch_ref)
 .bind((item.position/linggan_contracts::ai_limits::AI_LIMITS.round_size+1) as i32).bind(admitted).bind(request.timeout_ms as f64/1000.0+10.0)
 .bind(egress).bind(json!({"producer":"ai-gen1-executor.v1","purpose":item.task_key.as_str(),"requestRef":input.reference,"methodVersion":input.contract,"promptVersion":prompt_version_ref})).bind(item.task_key.as_str()).execute(&mut **tx).await?;
    let changed=sqlx::query("UPDATE linggan_ai_batch_item_control SET invocation_ref=$2,attempt_count=attempt_count+1,executed_config_ref=$3,executed_skill_version_ref=(SELECT skill_version_ref FROM linggan_ai_task_config WHERE task_config_ref=$3),execution_contract=$4 WHERE item_ref=$1 AND attempt_count=0").bind(item.item_ref).bind(invocation).bind(item.task_config_ref).bind(input.contract).execute(&mut **tx).await?;
    if changed.rows_affected() != 1 {
        return Err(ExecutorError::FenceLost);
    }
    Ok(Call {
        invocation,
        request,
        prompt_version_ref,
    })
}

/// Physical uncertainty survives cancellation and permission/source withholding.
pub(super) fn failure(
    response: Option<&crate::pi_adapter::PiResponse>,
    allowed: bool,
    success: bool,
    withheld: &'static str,
) -> Option<&'static str> {
    if response.is_none() {
        Some("ai_dispatch_unknown")
    } else if success {
        None
    } else if !allowed {
        Some(withheld)
    } else if response.is_some_and(|r| r.ok) {
        Some("ai_output_invalid")
    } else {
        Some("ai_provider_failed")
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn unknown_receipt_is_preserved_even_when_publication_is_withheld() {
        for allowed in [true, false] {
            assert_eq!(
                super::failure(None, allowed, false, "publication_withheld"),
                Some("ai_dispatch_unknown")
            );
        }
    }
}
