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
    if task.comment_stage().is_none()
        && task.topic_stage().is_none()
        && task != TaskKey::WorkComparison
        && task != TaskKey::CreatorResearch
        && task != TaskKey::CreatorAccountResearch
        && task != TaskKey::CommentStudy
    {
        return base::check_queue_input_scope(tx, task, config, input).await;
    }
    if !matches!(input, InputRef::TopicResearch { stage_number, .. } if task.topic_stage()==Some(*stage_number))
        && !matches!(input, InputRef::CommentProblemStage {stage_number,..} if task.comment_stage()==Some(*stage_number) && *stage_number>0)
        && !matches!(
            (task, input),
            (TaskKey::WorkComparison, InputRef::WorkComparison { .. })
                | (
                    TaskKey::CreatorResearch,
                    InputRef::CreatorWorkResearch { .. }
                )
                | (TaskKey::CommentStudy, InputRef::CommentResearch { .. })
                | (
                    TaskKey::CreatorAccountResearch,
                    InputRef::CreatorAccountResearch { .. }
                )
        )
    {
        return Err(FoundationError::Invalid);
    }
    let allowed:bool=sqlx::query_scalar(
        "SELECT COALESCE((SELECT permission.allowed FROM linggan_ai_task_config task \
         JOIN linggan_model_config model ON model.config_ref=task.model_config_ref \
         JOIN linggan_model_capability capability ON capability.capability_ref=model.capability_ref \
         JOIN linggan_ai_current_egress permission ON permission.endpoint_ref=capability.endpoint_ref \
           AND permission.task_key=task.task_key AND permission.material_class='note_material' \
         WHERE task.task_config_ref=$1 AND task.task_key=$2),false)",
    ).bind(config).bind(task.as_str()).fetch_one(&mut **tx).await?;
    if !allowed {
        return Err(FoundationError::AdmissionDenied(vec![
            "egress_not_approved".into(),
        ]));
    }
    if task.comment_stage().is_some() || task.topic_stage().is_some() {
        let comments:bool=sqlx::query_scalar("SELECT COALESCE((SELECT permission.allowed FROM linggan_ai_task_config task JOIN linggan_model_config model ON model.config_ref=task.model_config_ref JOIN linggan_model_capability capability ON capability.capability_ref=model.capability_ref JOIN linggan_ai_current_egress permission ON permission.endpoint_ref=capability.endpoint_ref AND permission.task_key=task.task_key AND permission.material_class='comment_material' WHERE task.task_config_ref=$1),false)")
            .bind(config).fetch_one(&mut **tx).await?;
        if !comments {
            return Err(FoundationError::AdmissionDenied(vec![
                "comment_egress_not_approved".into(),
            ]));
        }
    }
    if task == TaskKey::CreatorAccountResearch {
        let approved:bool=sqlx::query_scalar("SELECT COALESCE((SELECT permission.allowed FROM linggan_ai_task_config task JOIN linggan_model_config model ON model.config_ref=task.model_config_ref JOIN linggan_model_capability capability ON capability.capability_ref=model.capability_ref JOIN linggan_ai_current_egress permission ON permission.endpoint_ref=capability.endpoint_ref AND permission.task_key=task.task_key AND permission.material_class='public_author_bio' WHERE task.task_config_ref=$1),false)")
            .bind(config).fetch_one(&mut **tx).await?;
        if !approved {
            return Err(FoundationError::AdmissionDenied(vec![
                "author_bio_egress_not_approved".into(),
            ]));
        }
    }
    Ok(())
}
