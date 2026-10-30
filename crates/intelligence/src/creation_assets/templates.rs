//! Draft-based structural templates. The persisted steps are a closed vocabulary: no manuscript,
//! quote, account name or free-form example is copied into a template or command receipt.
use super::{
    AssetsError,
    requests::{command_hash, lock},
};
use crate::content_workbench as workbench;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralStep {
    Opening,
    Scene,
    Experience,
    Feeling,
    Reflection,
    Action,
    Ending,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTemplate {
    pub request_id: Uuid,
    pub work_id: Uuid,
    pub expected_revision: Uuid,
    pub steps: Vec<StructuralStep>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateAction {
    Approve,
    Disable,
    Enable,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewTemplate {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub action: TemplateAction,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviseTemplate {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub steps: Vec<StructuralStep>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuralTemplate {
    pub code: String,
    pub revision: i32,
    pub source_work_id: Uuid,
    pub source_revision_id: Uuid,
    pub source_available: bool,
    pub status: String,
    pub steps: Vec<StructuralStep>,
    pub created_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateReceipt {
    pub template: StructuralTemplate,
    pub replayed: bool,
}

async fn qualified(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    revision_id: Uuid,
) -> Result<bool, AssetsError> {
    // A revision may have detached all experience slots while earlier revisions still
    // require withdrawal propagation. Serialize that history check with withdrawal.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut **tx)
        .await?;
    let (domain, mut revision) =
        match workbench::read_revision_in(tx, work, Some(revision_id)).await {
            Ok(value) => value,
            Err(workbench::WorkbenchError::Database(error)) => {
                return Err(AssetsError::Database(error));
            }
            Err(_) => return Ok(false),
        };
    if !workbench::experience_fragments_available_in(tx, revision_id, &revision.document)
        .await
        .map_err(AssetsError::Workbench)?
    {
        return Ok(false);
    }
    // A structural template stays tied to the history from which it was created.
    // A later reset must not retroactively make an older source eligible again.
    let withdrawn: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_revision r CROSS JOIN LATERAL jsonb_array_elements(COALESCE(r.document->'experienceSlots','[]'::jsonb)) slot(value) JOIN linggan_creation_experience e ON e.experience_ref::text=slot.value->>'experienceRef' JOIN linggan_creation_revision src ON src.revision_ref=$2 AND src.work_ref=r.work_ref WHERE r.work_ref=$1 AND r.revision_number<=src.revision_number AND e.state='withdrawn' AND NOT EXISTS (SELECT 1 FROM linggan_creation_experience_reset reset JOIN linggan_creation_request request ON request.request_ref=reset.request_ref JOIN linggan_creation_revision reset_revision ON reset_revision.revision_ref::text=request.receipt->'revision'->>'id' WHERE reset.work_ref=r.work_ref AND reset.experience_ref=e.experience_ref AND reset_revision.revision_number<=src.revision_number))")
        .bind(work).bind(revision_id).fetch_one(&mut **tx).await?;
    if withdrawn {
        return Ok(false);
    }
    let before_redaction = revision.document.clone();
    let statuses = workbench::redact_revision_with_statuses_in(tx, domain, &mut revision)
        .await
        .map_err(AssetsError::Workbench)?;
    Ok(statuses.iter().all(|status| status.state == "readable")
        && revision.document == before_redaction
        && !revision
            .document
            .uses
            .iter()
            .any(|usage| usage.state == workbench::UseState::NeedsReview))
}
async fn get_in(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<StructuralTemplate, AssetsError> {
    get_revision_in(tx, code, None).await
}
async fn get_revision_in(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
    selected: Option<i32>,
) -> Result<StructuralTemplate, AssetsError> {
    let row = sqlx::query("SELECT t.code,t.work_ref,t.revision_ref,r.created_at::text,r.revision,r.steps,COALESCE((SELECT s.status FROM linggan_creation_template_status s WHERE s.template_ref=t.template_ref AND s.revision=r.revision ORDER BY s.sequence DESC LIMIT 1),'review') AS status FROM linggan_creation_template t JOIN linggan_creation_template_revision r USING(template_ref) WHERE t.code=$1 AND ($2::integer IS NULL OR r.revision=$2) ORDER BY r.revision DESC LIMIT 1")
        .bind(code).bind(selected).fetch_optional(&mut **tx).await?.ok_or(AssetsError::NotFound)?;
    let work = row.try_get("work_ref")?;
    let revision = row.try_get("revision_ref")?;
    Ok(StructuralTemplate {
        code: row.try_get("code")?,
        revision: row.try_get("revision")?,
        source_work_id: work,
        source_revision_id: revision,
        source_available: qualified(tx, work, revision).await?,
        status: row.try_get("status")?,
        steps: serde_json::from_value(row.try_get("steps")?)?,
        created_at: row.try_get("created_at")?,
    })
}
pub async fn structural_template_history(
    db: &Database,
    code: &str,
) -> Result<Vec<StructuralTemplate>, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let revisions: Vec<i32> = sqlx::query_scalar("SELECT r.revision FROM linggan_creation_template_revision r JOIN linggan_creation_template t USING(template_ref) WHERE t.code=$1 ORDER BY r.revision DESC")
        .bind(code).fetch_all(&mut *tx).await?;
    if revisions.is_empty() {
        return Err(AssetsError::NotFound);
    }
    let mut result = Vec::with_capacity(revisions.len());
    for revision in revisions {
        result.push(get_revision_in(&mut tx, code, Some(revision)).await?);
    }
    tx.commit().await?;
    Ok(result)
}

pub async fn revise_structural_template(
    db: &Database,
    code: &str,
    cmd: &ReviseTemplate,
) -> Result<TemplateReceipt, AssetsError> {
    if cmd.steps.is_empty() || cmd.steps.len() > 12 {
        return Err(AssetsError::Invalid("template_steps"));
    }
    let sha = command_hash(
        "structural_template_revise",
        &(code, cmd.expected_revision, &cmd.steps),
    )?;
    let mut tx = db.pool().begin().await?;
    workbench::ensure_request_owner_in(&mut tx, cmd.request_id, workbench::RequestOwner::Template)
        .await
        .map_err(|error| match error {
            workbench::WorkbenchError::RequestConflict => AssetsError::RequestConflict,
            other => AssetsError::Workbench(other),
        })?;
    if let Some(row) = sqlx::query("SELECT t.code,r.revision,r.request_hash FROM linggan_creation_template_revision r JOIN linggan_creation_template t USING(template_ref) WHERE r.request_ref=$1").bind(cmd.request_id).fetch_optional(&mut *tx).await? {
        if row.get::<String,_>("request_hash") != sha { return Err(AssetsError::RequestConflict); }
        let template = get_revision_in(&mut tx, &row.get::<String,_>("code"), Some(row.get("revision"))).await?;
        tx.commit().await?;
        return Ok(TemplateReceipt { template, replayed: true });
    }
    let used: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_template WHERE request_ref=$1 UNION ALL SELECT 1 FROM linggan_creation_template_status WHERE request_ref=$1)").bind(cmd.request_id).fetch_one(&mut *tx).await?;
    if used {
        return Err(AssetsError::RequestConflict);
    }
    lock(&mut tx, &format!("structural-template:{code}")).await?;
    let current = get_in(&mut tx, code).await?;
    if current.revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    if !current.source_available {
        return Err(AssetsError::SourceUnavailable);
    }
    if current.status == "off" {
        return Err(AssetsError::Invalid("template_transition"));
    }
    let next = current
        .revision
        .checked_add(1)
        .ok_or(AssetsError::Invalid("template_revision"))?;
    sqlx::query("INSERT INTO linggan_creation_template_revision(template_ref,revision,steps,request_ref,request_hash) SELECT template_ref,$2,$3,$4,$5 FROM linggan_creation_template WHERE code=$1")
        .bind(code).bind(next).bind(serde_json::to_value(&cmd.steps)?).bind(cmd.request_id).bind(sha).execute(&mut *tx).await?;
    let template = get_revision_in(&mut tx, code, Some(next)).await?;
    tx.commit().await?;
    Ok(TemplateReceipt {
        template,
        replayed: false,
    })
}
pub async fn list_structural_templates(
    db: &Database,
    domain: Uuid,
) -> Result<Vec<StructuralTemplate>, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let codes: Vec<String> = sqlx::query_scalar("SELECT t.code FROM linggan_creation_template t JOIN linggan_creation_work w ON w.work_ref=t.work_ref WHERE w.domain_ref=$1 ORDER BY t.created_at DESC,t.code DESC LIMIT 200")
        .bind(domain).fetch_all(&mut *tx).await?;
    let mut items = Vec::with_capacity(codes.len());
    for code in codes {
        items.push(get_in(&mut tx, &code).await?);
    }
    tx.commit().await?;
    Ok(items)
}
pub async fn create_structural_template(
    db: &Database,
    cmd: &CreateTemplate,
) -> Result<TemplateReceipt, AssetsError> {
    if cmd.steps.is_empty() || cmd.steps.len() > 12 {
        return Err(AssetsError::Invalid("template_steps"));
    }
    let sha = command_hash(
        "structural_template_create",
        &(cmd.work_id, cmd.expected_revision, &cmd.steps),
    )?;
    let mut tx = db.pool().begin().await?;
    workbench::ensure_request_owner_in(&mut tx, cmd.request_id, workbench::RequestOwner::Template)
        .await
        .map_err(|error| match error {
            workbench::WorkbenchError::RequestConflict => AssetsError::RequestConflict,
            other => AssetsError::Workbench(other),
        })?;
    if let Some(row) =
        sqlx::query("SELECT code,request_hash FROM linggan_creation_template WHERE request_ref=$1")
            .bind(cmd.request_id)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<String, _>("request_hash") != sha {
            return Err(AssetsError::RequestConflict);
        }
        let template = get_in(&mut tx, &row.get::<String, _>("code")).await?;
        tx.commit().await?;
        return Ok(TemplateReceipt {
            template,
            replayed: true,
        });
    }
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_template_status WHERE request_ref=$1 UNION ALL SELECT 1 FROM linggan_creation_template_revision WHERE request_ref=$1)",
    )
    .bind(cmd.request_id)
    .fetch_one(&mut *tx)
    .await?
    {
        return Err(AssetsError::RequestConflict);
    }
    workbench::work_lock(&mut tx, cmd.work_id.to_string())
        .await
        .map_err(AssetsError::Workbench)?;
    let (_, current) = workbench::read_revision_in(&mut tx, cmd.work_id, None)
        .await
        .map_err(AssetsError::Workbench)?;
    if current.id != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    if !current
        .document
        .paragraphs
        .iter()
        .any(|p| !p.text.trim().is_empty())
    {
        return Err(AssetsError::Invalid("template_empty_draft"));
    }
    if !qualified(&mut tx, cmd.work_id, cmd.expected_revision).await? {
        return Err(AssetsError::SourceUnavailable);
    }
    let code: String = sqlx::query_scalar(
        "SELECT 'YB-' || CASE WHEN length(n)<4 THEN lpad(n,4,'0') ELSE n END FROM (SELECT nextval('linggan_creation_template_code_seq')::text n) seq",
    )
    .fetch_one(&mut *tx)
    .await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_creation_template(template_ref,code,work_ref,revision_ref,request_ref,request_hash) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(id).bind(&code).bind(cmd.work_id).bind(cmd.expected_revision).bind(cmd.request_id).bind(sha).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_creation_template_revision(template_ref,revision,steps) VALUES($1,1,$2)")
        .bind(id).bind(serde_json::to_value(&cmd.steps)?).execute(&mut *tx).await?;
    let template = get_in(&mut tx, &code).await?;
    tx.commit().await?;
    Ok(TemplateReceipt {
        template,
        replayed: false,
    })
}
pub async fn review_structural_template(
    db: &Database,
    code: &str,
    cmd: &ReviewTemplate,
) -> Result<TemplateReceipt, AssetsError> {
    let sha = command_hash(
        "structural_template_review",
        &(code, cmd.expected_revision, &cmd.action),
    )?;
    let mut tx = db.pool().begin().await?;
    workbench::ensure_request_owner_in(&mut tx, cmd.request_id, workbench::RequestOwner::Template)
        .await
        .map_err(|error| match error {
            workbench::WorkbenchError::RequestConflict => AssetsError::RequestConflict,
            other => AssetsError::Workbench(other),
        })?;
    if let Some(row) = sqlx::query("SELECT t.code,s.request_hash FROM linggan_creation_template_status s JOIN linggan_creation_template t USING(template_ref) WHERE s.request_ref=$1").bind(cmd.request_id).fetch_optional(&mut *tx).await? {
        if row.get::<String,_>("request_hash") != sha { return Err(AssetsError::RequestConflict); }
        let template = get_in(&mut tx,&row.get::<String,_>("code")).await?;
        tx.commit().await?; return Ok(TemplateReceipt { template, replayed:true });
    }
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_template WHERE request_ref=$1 UNION ALL SELECT 1 FROM linggan_creation_template_revision WHERE request_ref=$1)",
    )
    .bind(cmd.request_id)
    .fetch_one(&mut *tx)
    .await?
    {
        return Err(AssetsError::RequestConflict);
    }
    lock(&mut tx, &format!("structural-template:{code}")).await?;
    let current = get_in(&mut tx, code).await?;
    if current.revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let next = match (current.status.as_str(), &cmd.action) {
        ("review", TemplateAction::Approve) if current.source_available => "ok",
        ("review" | "ok", TemplateAction::Disable) => "off",
        ("off", TemplateAction::Enable) => "review",
        ("review", TemplateAction::Approve) => return Err(AssetsError::SourceUnavailable),
        _ => return Err(AssetsError::Invalid("template_transition")),
    };
    sqlx::query("INSERT INTO linggan_creation_template_status(event_ref,template_ref,revision,sequence,request_ref,request_hash,status) SELECT $1,t.template_ref,$2,COALESCE((SELECT max(sequence) FROM linggan_creation_template_status WHERE template_ref=t.template_ref AND revision=$2),0)+1,$3,$4,$5 FROM linggan_creation_template t WHERE code=$6")
        .bind(Uuid::new_v4()).bind(current.revision).bind(cmd.request_id).bind(sha).bind(next).bind(code).execute(&mut *tx).await?;
    let template = get_in(&mut tx, code).await?;
    tx.commit().await?;
    Ok(TemplateReceipt {
        template,
        replayed: false,
    })
}
