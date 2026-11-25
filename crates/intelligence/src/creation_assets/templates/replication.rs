//! Freeze the canonical author-body input and the registered instruction versions for review.
use super::{AssetsError, StructuralStep};
use crate::ai_foundation::{TaskKey, prompt, transport::ResponseSchema};
use linggan_evidence::research_dossier;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

const CONTRACT: &str = "creation-template-replication.v1";
const MAX_BODY_BYTES: usize = 24_000;
const MIN_STEPS: usize = 1;
const MAX_STEPS: usize = 12;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplicationSource {
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub material_revision: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplicationPreview {
    pub source: ReplicationSource,
    pub task_config_ref: Uuid,
    pub input_hash: String,
    pub system: String,
    pub prompt: String,
    pub response_schema: ResponseSchema,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplicationOutput {
    steps: Vec<StructuralStep>,
}

pub fn decode_replication_candidate(response: &str) -> Result<Vec<StructuralStep>, AssetsError> {
    if response.len() > 512 {
        return Err(AssetsError::Invalid("template_replication_output"));
    }
    let output: ReplicationOutput = serde_json::from_str(response)
        .map_err(|_| AssetsError::Invalid("template_replication_output"))?;
    if !(MIN_STEPS..=MAX_STEPS).contains(&output.steps.len()) {
        return Err(AssetsError::Invalid("template_replication_output"));
    }
    Ok(output.steps)
}

fn response_schema() -> ResponseSchema {
    ResponseSchema {
        name: "template_replication".into(),
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["steps"],
            "properties": {
                "steps": {
                    "type": "array",
                    "minItems": MIN_STEPS,
                    "maxItems": MAX_STEPS,
                    "items": {"type": "string", "enum": StructuralStep::ALL}
                }
            }
        }),
    }
}

pub async fn preview_replication(
    db: &Database,
    source: ReplicationSource,
) -> Result<ReplicationPreview, AssetsError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    if !research_dossier::creation_work_in_domain(&mut tx, source.work_ref, source.domain_ref)
        .await?
    {
        return Err(AssetsError::SourceUnavailable);
    }
    let cutoff = research_dossier::cutoff_in(&mut tx, None).await?;
    let dossier = research_dossier::read_in(&mut tx, &[source.work_ref], &cutoff)
        .await?
        .remove(&source.work_ref)
        .ok_or(AssetsError::SourceUnavailable)?;
    if dossier.material_revision != source.material_revision {
        return Err(AssetsError::Stale);
    }
    let body_refs: std::collections::HashSet<&str> = dossier
        .materials
        .iter()
        .filter(|material| material["kind"] == "body" && material["sourceState"] == "present")
        .filter_map(|material| material["blocks"].as_array())
        .flatten()
        .filter_map(|block| block["ref"].as_str())
        .collect();
    let body: Vec<&str> = dossier
        .fragments
        .iter()
        .filter(|fragment| {
            fragment.field == "body"
                && body_refs.contains(fragment.fragment_id.as_str())
                && fragment.speaker == "author"
                && !fragment.text.trim().is_empty()
        })
        .map(|fragment| fragment.text.as_str())
        .collect();
    if body.is_empty() {
        return Err(AssetsError::SourceUnavailable);
    }
    let prompt_text = json!({"authorBody": body}).to_string();
    if prompt_text.len() > MAX_BODY_BYTES {
        return Err(AssetsError::Invalid("template_replication_input_limit"));
    }
    let task = TaskKey::CreationTemplateReplication;
    let task_config_ref: Uuid = sqlx::query_scalar(
        "SELECT cfg.task_config_ref FROM linggan_ai_task_setting setting
         JOIN linggan_ai_task_config cfg ON cfg.task_config_ref=setting.current_config_ref
         WHERE setting.task_key=$1 AND cfg.task_key=setting.task_key
             AND cfg.execution_contract=$2 AND cfg.skill_version_ref IS NOT NULL",
    )
    .bind(task.as_str())
    .bind(CONTRACT)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AssetsError::NotFound)?;
    let (_, mut system, instruction, skill) = prompt::frozen_in(&mut tx, task_config_ref, task)
        .await
        .map_err(|_| AssetsError::Invalid("template_replication_configuration"))?;
    let skill = skill.ok_or(AssetsError::Invalid("template_replication_configuration"))?;
    system.push_str("\n\n");
    system.push_str(&instruction);
    system.push_str("\n\n");
    system.push_str(&skill);
    let response_schema = response_schema();
    let input_hash = super::command_hash(
        CONTRACT,
        &(
            &source,
            task_config_ref,
            &system,
            &prompt_text,
            &response_schema,
        ),
    )?;
    tx.commit().await?;
    Ok(ReplicationPreview {
        source,
        task_config_ref,
        input_hash,
        system,
        prompt: prompt_text,
        response_schema,
    })
}
