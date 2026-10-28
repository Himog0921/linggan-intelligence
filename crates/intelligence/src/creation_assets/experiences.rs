//! Local, erasable experience storage. Not yet connected to draft or HTTP operations.
//!
//! Confirmation is a record of the subject's actual confirmation, not authentication of the
//! person entering the record. Never put the content into immutable receipts or ordinary logs.
use super::experience_policy::{ExperienceState, use_in_draft};
use super::requests::{command_hash, lock};
use super::{AssetsError, types::*};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

async fn snapshot(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<Experience, AssetsError> {
    snapshot_revision(tx, id, None).await
}
async fn snapshot_revision(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    revision: Option<i32>,
) -> Result<Experience, AssetsError> {
    let row = sqlx::query("SELECT e.experience_ref,e.code,e.account_ref,e.state,COALESCE($2,e.current_revision) AS current_revision,e.created_at::text AS created_at,e.withdrawn_at::text AS withdrawn_at,r.title,r.body,r.tags,r.origin,c.send_to_model,c.confirmed_at::text AS confirmed_at FROM linggan_creation_experience e LEFT JOIN linggan_creation_experience_revision r ON r.experience_ref=e.experience_ref AND r.revision=COALESCE($2,e.current_revision) LEFT JOIN linggan_creation_experience_consent c ON c.experience_ref=r.experience_ref AND c.revision=r.revision WHERE e.experience_ref=$1")
        .bind(id).bind(revision).fetch_optional(&mut **tx).await?.ok_or(AssetsError::NotFound)?;
    Ok(Experience {
        id: row.try_get("experience_ref")?,
        code: row.try_get("code")?,
        account_ref: row.try_get("account_ref")?,
        state: row.try_get("state")?,
        revision: row.try_get("current_revision")?,
        title: row.try_get("title")?,
        body: row.try_get("body")?,
        tags: row.try_get("tags")?,
        origin: row.try_get("origin")?,
        send_to_model: row.try_get("send_to_model")?,
        confirmed_at: row.try_get("confirmed_at")?,
        created_at: row.try_get("created_at")?,
        withdrawn_at: row.try_get("withdrawn_at")?,
    })
}

struct PriorResult {
    id: Uuid,
    revision: i32,
    state: String,
}

async fn previous(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    kind: &str,
    hash: &str,
) -> Result<Option<PriorResult>, AssetsError> {
    lock(tx, &format!("experience-request:{request}")).await?;
    let row = sqlx::query("SELECT experience_ref,kind,command_sha256,result_revision,result_state FROM linggan_creation_experience_request WHERE request_ref=$1")
        .bind(request).fetch_optional(&mut **tx).await?;
    match row {
        None => Ok(None),
        Some(row)
            if row.try_get::<String, _>("kind")? == kind
                && row
                    .try_get::<Option<String>, _>("command_sha256")?
                    .as_deref()
                    == Some(hash) =>
        {
            Ok(Some(PriorResult {
                id: row.try_get("experience_ref")?,
                revision: row.try_get("result_revision")?,
                state: row.try_get("result_state")?,
            }))
        }
        Some(row)
            if row
                .try_get::<Option<String>, _>("command_sha256")?
                .is_none() =>
        {
            Err(AssetsError::NotFound)
        }
        Some(_) => Err(AssetsError::RequestConflict),
    }
}

async fn replay_result(
    tx: &mut Transaction<'_, Postgres>,
    prior: PriorResult,
) -> Result<ExperienceReceipt, AssetsError> {
    let mut experience = snapshot_revision(tx, prior.id, Some(prior.revision)).await?;
    if experience.state == "withdrawn" && prior.state != "withdrawn" {
        return Err(AssetsError::NotFound);
    }
    experience.state = prior.state;
    if experience.state == "awaiting_subject" {
        experience.confirmed_at = None;
        experience.send_to_model = None;
    }
    Ok(ExperienceReceipt {
        experience,
        replayed: true,
    })
}

async fn record(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    kind: &str,
    experience: &Experience,
    hash: &str,
) -> Result<(), AssetsError> {
    sqlx::query("INSERT INTO linggan_creation_experience_request(request_ref,kind,experience_ref,command_sha256,result_revision,result_state) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(request).bind(kind).bind(experience.id).bind(hash).bind(experience.revision).bind(&experience.state).execute(&mut **tx).await?;
    Ok(())
}

pub async fn read_experience(db: &Database, id: Uuid) -> Result<Experience, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let value = snapshot(&mut tx, id).await?;
    tx.commit().await?;
    Ok(value)
}

/// Qualification uses the stored owner and the consent attached to the exact revision. This is
/// still a library read; draft insertion must requalify within its own write transaction.
pub async fn read_usable_experience(
    db: &Database,
    id: Uuid,
    revision: i32,
    account: Option<Uuid>,
) -> Result<UsableExperience, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let row = sqlx::query("SELECT e.code,e.account_ref,e.state,r.body,c.draft_use FROM linggan_creation_experience e LEFT JOIN linggan_creation_experience_revision r ON r.experience_ref=e.experience_ref AND r.revision=$2 LEFT JOIN linggan_creation_experience_consent c ON c.experience_ref=r.experience_ref AND c.revision=r.revision WHERE e.experience_ref=$1")
        .bind(id).bind(revision).fetch_optional(&mut *tx).await?.ok_or(AssetsError::NotFound)?;
    let state: String = row.try_get("state")?;
    let draft_use: Option<bool> = row.try_get("draft_use")?;
    let state = match state.as_str() {
        "disabled" => ExperienceState::Disabled,
        "withdrawn" => ExperienceState::Withdrawn,
        "awaiting_subject" | "confirmed" if draft_use == Some(true) => ExperienceState::Confirmed,
        "awaiting_subject" | "confirmed" => ExperienceState::AwaitingSubject,
        _ => return Err(AssetsError::Invalid("experience_state")),
    };
    use_in_draft(
        row.try_get("account_ref")?,
        account,
        state,
        draft_use.is_some(),
        draft_use == Some(true),
    )
    .map_err(|_| AssetsError::SourceUnavailable)?;
    let body: Option<String> = row.try_get("body")?;
    let result = UsableExperience {
        id,
        code: row.try_get("code")?,
        revision,
        body: body.ok_or(AssetsError::SourceUnavailable)?,
    };
    tx.commit().await?;
    Ok(result)
}

fn validate_payload(
    title: &str,
    body: &str,
    tags: &[String],
    origin: &str,
) -> Result<(), AssetsError> {
    if title.trim().is_empty()
        || title.trim().chars().count() > 200
        || body.trim().is_empty()
        || body.trim().chars().count() > 10000
        || tags.len() > 20
        || tags
            .iter()
            .any(|t| t.trim().is_empty() || t.chars().count() > 60)
        || !matches!(origin, "oral" | "manuscript" | "anonymized_coaching")
    {
        return Err(AssetsError::Invalid("experience"));
    }
    Ok(())
}

pub async fn create_experience(
    db: &Database,
    cmd: &ExperienceCreate,
) -> Result<ExperienceReceipt, AssetsError> {
    let title = cmd.title.trim();
    let body = cmd.body.trim();
    validate_payload(title, body, &cmd.tags, &cmd.origin)?;
    let hash = command_hash(
        "experience_create",
        &(cmd.account_ref, title, body, &cmd.tags, &cmd.origin),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Some(prior) = previous(&mut tx, cmd.request_id, "create", &hash).await? {
        let receipt = replay_result(&mut tx, prior).await?;
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, &format!("identity:{}", cmd.account_ref)).await?;
    let identity = super::identity::latest_in(&mut tx, cmd.account_ref).await?;
    if identity.is_none_or(|value| value.has_real_person != Some(true)) {
        return Err(AssetsError::Invalid("account_has_no_confirmed_person"));
    }
    let id = Uuid::new_v4();
    let sequence: i64 =
        sqlx::query_scalar("SELECT nextval('linggan_creation_experience_code_seq')")
            .fetch_one(&mut *tx)
            .await?;
    let code = format!("JL-{sequence:04}");
    sqlx::query("INSERT INTO linggan_creation_experience(experience_ref,code,account_ref,state,current_revision,created_by) VALUES($1,$2,$3,'awaiting_subject',1,'local_machine')")
        .bind(id).bind(code).bind(cmd.account_ref).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_creation_experience_revision(experience_ref,revision,title,body,tags,origin,saved_by) VALUES($1,1,$2,$3,$4,$5,'local_machine')")
        .bind(id).bind(title).bind(body).bind(&cmd.tags).bind(&cmd.origin).execute(&mut *tx).await?;
    let experience = snapshot(&mut tx, id).await?;
    record(&mut tx, cmd.request_id, "create", &experience, &hash).await?;
    tx.commit().await?;
    Ok(ExperienceReceipt {
        experience,
        replayed: false,
    })
}

/// Editing leaves old revisions in place for existing references, but the new revision requires
/// a new confirmation. No draft use is exposed until all withdrawal paths have been implemented.
pub async fn edit_experience(
    db: &Database,
    id: Uuid,
    cmd: &ExperienceEdit,
) -> Result<ExperienceReceipt, AssetsError> {
    let title = cmd.title.trim();
    let body = cmd.body.trim();
    validate_payload(title, body, &cmd.tags, &cmd.origin)?;
    let hash = command_hash(
        "experience_edit",
        &(
            id,
            cmd.expected_revision,
            title,
            body,
            &cmd.tags,
            &cmd.origin,
        ),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Some(prior) = previous(&mut tx, cmd.request_id, "edit", &hash).await? {
        let receipt = replay_result(&mut tx, prior).await?;
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, &format!("experience:{id}")).await?;
    let current = snapshot(&mut tx, id).await?;
    if current.revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    if !matches!(current.state.as_str(), "awaiting_subject" | "confirmed") {
        return Err(AssetsError::Invalid("experience_state"));
    }
    let next = current
        .revision
        .checked_add(1)
        .ok_or(AssetsError::Invalid("experience_revision"))?;
    sqlx::query("INSERT INTO linggan_creation_experience_revision(experience_ref,revision,title,body,tags,origin,saved_by) VALUES($1,$2,$3,$4,$5,$6,'local_machine')")
        .bind(id).bind(next).bind(title).bind(body).bind(&cmd.tags).bind(&cmd.origin).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_creation_experience SET state='awaiting_subject',current_revision=$2 WHERE experience_ref=$1").bind(id).bind(next).execute(&mut *tx).await?;
    let experience = snapshot(&mut tx, id).await?;
    record(&mut tx, cmd.request_id, "edit", &experience, &hash).await?;
    tx.commit().await?;
    Ok(ExperienceReceipt {
        experience,
        replayed: false,
    })
}

pub async fn confirm_experience(
    db: &Database,
    id: Uuid,
    cmd: &ExperienceConfirm,
) -> Result<ExperienceReceipt, AssetsError> {
    if !cmd.subject_confirmed || !cmd.draft_use {
        return Err(AssetsError::Invalid("subject_consent"));
    }
    let hash = command_hash(
        "experience_confirm",
        &(
            id,
            cmd.expected_revision,
            cmd.subject_confirmed,
            cmd.draft_use,
            cmd.send_to_model,
        ),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Some(prior) = previous(&mut tx, cmd.request_id, "confirm", &hash).await? {
        let receipt = replay_result(&mut tx, prior).await?;
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, &format!("experience:{id}")).await?;
    let current = snapshot(&mut tx, id).await?;
    if current.revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    if current.state != "awaiting_subject" {
        return Err(AssetsError::Invalid("experience_state"));
    }
    sqlx::query("INSERT INTO linggan_creation_experience_consent(experience_ref,revision,draft_use,send_to_model,recorded_by) VALUES($1,$2,true,$3,'local_machine')")
        .bind(id).bind(cmd.expected_revision).bind(cmd.send_to_model).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_creation_experience SET state='confirmed' WHERE experience_ref=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let experience = snapshot(&mut tx, id).await?;
    record(&mut tx, cmd.request_id, "confirm", &experience, &hash).await?;
    tx.commit().await?;
    Ok(ExperienceReceipt {
        experience,
        replayed: false,
    })
}

/// Erases the library payload. This is not yet the S1b withdrawal operation: draft, history,
/// export, AI and template references must be integrated before exposing this function to users.
pub async fn withdraw_experience(
    db: &Database,
    id: Uuid,
    request_id: Uuid,
) -> Result<ExperienceReceipt, AssetsError> {
    let hash = command_hash("experience_withdraw", &id)?;
    let mut tx = db.pool().begin().await?;
    if let Some(prior) = previous(&mut tx, request_id, "withdraw", &hash).await? {
        let receipt = replay_result(&mut tx, prior).await?;
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, &format!("experience:{id}")).await?;
    let current = snapshot(&mut tx, id).await?;
    if current.state == "withdrawn" {
        return Err(AssetsError::Invalid("experience_state"));
    }
    sqlx::query("DELETE FROM linggan_creation_experience_fragment WHERE experience_ref=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM linggan_creation_experience_revision WHERE experience_ref=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE linggan_creation_experience_request SET command_sha256=NULL WHERE experience_ref=$1").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_creation_experience SET state='withdrawn',withdrawn_at=scope_001_now() WHERE experience_ref=$1").bind(id).execute(&mut *tx).await?;
    let experience = snapshot(&mut tx, id).await?;
    record(&mut tx, request_id, "withdraw", &experience, &hash).await?;
    tx.commit().await?;
    Ok(ExperienceReceipt {
        experience,
        replayed: false,
    })
}
