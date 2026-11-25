use super::{AssetsError, StructuralStep, TemplateReceipt, get_in, next_code};
use crate::content_workbench as workbench;
use linggan_evidence::research_dossier;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateEvidenceTemplate {
    pub request_id: Uuid,
    pub work_ref: Uuid,
    pub domain_ref: Uuid,
    pub material_revision: String,
    pub steps: Vec<StructuralStep>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceTemplateSource {
    pub work_ref: Uuid,
    pub material_revision: String,
}

pub(super) async fn available(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    domain: Uuid,
    material_revision: &str,
) -> Result<bool, AssetsError> {
    if !research_dossier::creation_work_in_domain(tx, work, domain).await? {
        return Ok(false);
    }
    let cutoff = research_dossier::cutoff_in(tx, None).await?;
    let mut dossiers = research_dossier::read_in(tx, &[work], &cutoff).await?;
    Ok(dossiers.remove(&work).is_some_and(|dossier| {
        dossier.material_revision == material_revision
            && dossier.materials.iter().any(|material| {
                matches!(material["kind"].as_str(), Some("body" | "image" | "audio"))
                    && material["sourceState"] == "present"
                    && material["blocks"]
                        .as_array()
                        .is_some_and(|blocks| !blocks.is_empty())
            })
    }))
}

pub async fn create_evidence_template_in(
    tx: &mut Transaction<'_, Postgres>,
    command: &CreateEvidenceTemplate,
    member: Option<Uuid>,
) -> Result<TemplateReceipt, AssetsError> {
    if command.steps.is_empty() || command.steps.len() > 12 {
        return Err(AssetsError::Invalid("template_steps"));
    }
    if command.material_revision.len() != 64
        || !command
            .material_revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AssetsError::Invalid("template_material_revision"));
    }
    let hash = super::command_hash("structural_template_from_evidence", command)?;
    workbench::ensure_request_owner_in(tx, command.request_id, workbench::RequestOwner::Template)
        .await
        .map_err(|error| match error {
            workbench::WorkbenchError::RequestConflict => AssetsError::RequestConflict,
            other => AssetsError::Workbench(other),
        })?;
    if let Some((code, previous_hash)) = sqlx::query_as::<_, (String, String)>(
        "SELECT code,request_hash FROM linggan_creation_template WHERE request_ref=$1",
    )
    .bind(command.request_id)
    .fetch_optional(&mut **tx)
    .await?
    {
        if previous_hash != hash {
            return Err(AssetsError::RequestConflict);
        }
        return Ok(TemplateReceipt {
            template: get_in(tx, &code).await?,
            replayed: true,
        });
    }
    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_template_status WHERE request_ref=$1
         UNION ALL SELECT 1 FROM linggan_creation_template_revision WHERE request_ref=$1)",
    )
    .bind(command.request_id)
    .fetch_one(&mut **tx)
    .await?;
    if used {
        return Err(AssetsError::RequestConflict);
    }
    super::lock(
        tx,
        &format!("structural-template:evidence:{}", command.work_ref),
    )
    .await?;
    if !available(
        tx,
        command.work_ref,
        command.domain_ref,
        &command.material_revision,
    )
    .await?
    {
        return Err(AssetsError::SourceUnavailable);
    }
    let duplicate: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_template WHERE evidence_work_ref=$1)",
    )
    .bind(command.work_ref)
    .fetch_one(&mut **tx)
    .await?;
    if duplicate {
        return Err(AssetsError::TemplateAlreadyImported);
    }
    let code = next_code(tx).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_creation_template(template_ref,code,evidence_work_ref,
         evidence_domain_ref,evidence_material_revision,request_ref,request_hash,created_by_member_ref)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    ).bind(id).bind(&code).bind(command.work_ref).bind(command.domain_ref)
     .bind(&command.material_revision).bind(command.request_id).bind(hash).bind(member)
     .execute(&mut **tx).await?;
    sqlx::query("INSERT INTO linggan_creation_template_revision(template_ref,revision,steps) VALUES($1,1,$2)")
        .bind(id).bind(serde_json::to_value(&command.steps)?).execute(&mut **tx).await?;
    Ok(TemplateReceipt {
        template: get_in(tx, &code).await?,
        replayed: false,
    })
}
