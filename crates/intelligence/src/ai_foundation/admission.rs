//! One prequeue/dispatch admission owner; the content-comparison scope adds no egress grants.
use super::{TaskKey, batch_store, store, transport_store};
use batch_store::InputRef;
use sqlx::{Postgres, Transaction};
use store::FoundationError;
use uuid::Uuid;
mod base;
pub(crate) use base::check_queue_configuration;
pub(crate) async fn check_queue_input_scope(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    config: Uuid,
    input: &InputRef,
) -> Result<(), FoundationError> {
    if task != TaskKey::WorkComparison {
        return base::check_queue_input_scope(tx, task, config, input).await;
    }
    if !matches!(input, InputRef::WorkComparison { .. }) {
        return Err(FoundationError::Invalid);
    }
    let allowed:bool=sqlx::query_scalar(
        "SELECT COALESCE((SELECT permission.allowed FROM linggan_ai_task_config task \
         JOIN linggan_model_config model ON model.config_ref=task.model_config_ref \
         JOIN linggan_model_capability capability ON capability.capability_ref=model.capability_ref \
         JOIN linggan_ai_current_egress permission ON permission.endpoint_ref=capability.endpoint_ref \
           AND permission.task_key=task.task_key AND permission.material_class='note_material' \
         WHERE task.task_config_ref=$1 AND task.task_key='work_comparison'),false)",
    ).bind(config).fetch_one(&mut **tx).await?;
    if !allowed {
        return Err(FoundationError::AdmissionDenied(vec![
            "egress_not_approved".into(),
        ]));
    }
    Ok(())
}
