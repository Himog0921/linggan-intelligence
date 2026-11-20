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
    if task == TaskKey::CommentProblemConsolidation {
        if !matches!(input, InputRef::CommentProblemConsolidation { .. }) {
            return Err(FoundationError::Invalid);
        }
        return require_egress(
            tx,
            task,
            config,
            "comment_material",
            "comment_egress_not_approved",
        )
        .await;
    }
    if task.topic_stage().is_none()
        && task != TaskKey::WorkComparison
        && task != TaskKey::CreatorResearch
        && task != TaskKey::CreatorAccountResearch
        && task != TaskKey::CommentUnderstanding
    {
        return base::check_queue_input_scope(tx, task, config, input).await;
    }
    if !matches!(input, InputRef::TopicResearch { stage_number, .. } if task.topic_stage()==Some(*stage_number))
        && !matches!(
            (task, input),
            (TaskKey::WorkComparison, InputRef::WorkComparison { .. })
                | (
                    TaskKey::CreatorResearch,
                    InputRef::CreatorWorkResearch { .. }
                )
                | (
                    TaskKey::CommentUnderstanding,
                    InputRef::CommentResearchUnit { .. }
                )
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
    let topic_uses_comments = if let InputRef::TopicResearch { window_ref, .. } = input {
        let contract: Option<String> = sqlx::query_scalar("SELECT input_refs->>'inputContract' FROM linggan_topic_research_window WHERE window_ref=$1")
            .bind(window_ref).fetch_optional(&mut **tx).await?.flatten();
        contract.as_deref() != Some(crate::topic_map_research::CORE_INPUT_CONTRACT)
    } else {
        false
    };
    if task == TaskKey::CommentUnderstanding || topic_uses_comments {
        require_egress(
            tx,
            task,
            config,
            "comment_material",
            "comment_egress_not_approved",
        )
        .await?;
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

async fn require_egress(
    tx: &mut Transaction<'_, Postgres>,
    task: TaskKey,
    config: Uuid,
    material_class: &str,
    reason: &str,
) -> Result<(), FoundationError> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ai_task_config task
         JOIN linggan_model_config model ON model.config_ref=task.model_config_ref
         JOIN linggan_model_capability capability ON capability.capability_ref=model.capability_ref
         JOIN linggan_ai_current_egress permission ON permission.endpoint_ref=capability.endpoint_ref
           AND permission.task_key=task.task_key AND permission.material_class=$3 AND permission.allowed
         WHERE task.task_config_ref=$1 AND task.task_key=$2)",
    ).bind(config).bind(task.as_str()).bind(material_class).fetch_one(&mut **tx).await?;
    if !allowed {
        return Err(FoundationError::AdmissionDenied(vec![reason.into()]));
    }
    Ok(())
}
