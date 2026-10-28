//! Draft experience content is stored separately so withdrawal can erase it from every revision.
use super::*;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const WITHDRAWN: &str = "[本人已撤回这段经历]";

pub(super) struct Fragment {
    paragraph: Uuid,
    experience: Uuid,
    revision: i32,
    text: String,
}

/// Lock the same experience key as confirmation and withdrawal before checking its state.
async fn lock_experience(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<(), WorkbenchError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation_assets:experience:{id}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) fn strip(doc: &mut WorkDocument) {
    for slot in &doc.experience_slots {
        if let Some(paragraph) = doc
            .paragraphs
            .iter_mut()
            .find(|p| p.id == slot.paragraph_id)
        {
            paragraph.text.clear();
        }
    }
}

pub(super) async fn prepare(
    tx: &mut Transaction<'_, Postgres>,
    old: Option<&WorkDocument>,
    doc: &WorkDocument,
) -> Result<(WorkDocument, Vec<Fragment>), WorkbenchError> {
    // Removing a reference cannot silently reclassify its existing text as ordinary authored text.
    if let Some(old) = old {
        for slot in &old.experience_slots {
            if !doc.experience_slots.iter().any(|new| new == slot)
                && doc
                    .paragraphs
                    .iter()
                    .any(|p| p.id == slot.paragraph_id && !p.text.is_empty())
            {
                return Err(WorkbenchError::Invalid("experience_reference_removed"));
            }
        }
    }
    let mut fragments = Vec::new();
    for slot in &doc.experience_slots {
        lock_experience(tx, slot.experience_ref).await?;
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_experience e JOIN linggan_creation_experience_revision r ON r.experience_ref=e.experience_ref AND r.revision=$2 JOIN linggan_creation_experience_consent c ON c.experience_ref=r.experience_ref AND c.revision=r.revision WHERE e.experience_ref=$1 AND e.account_ref=$3 AND e.state IN ('confirmed','awaiting_subject') AND c.draft_use)")
            .bind(slot.experience_ref).bind(slot.experience_revision).bind(doc.account_ref)
            .fetch_one(&mut **tx).await?;
        if !allowed {
            return Err(WorkbenchError::Source);
        }
        let paragraph = doc
            .paragraphs
            .iter()
            .find(|p| p.id == slot.paragraph_id)
            .ok_or(WorkbenchError::Invalid("experience_slot"))?;
        fragments.push(Fragment {
            paragraph: slot.paragraph_id,
            experience: slot.experience_ref,
            revision: slot.experience_revision,
            text: paragraph.text.clone(),
        });
    }
    let mut stored = doc.clone();
    strip(&mut stored);
    Ok((stored, fragments))
}

pub(super) async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    fragments: &[Fragment],
) -> Result<(), WorkbenchError> {
    for f in fragments {
        sqlx::query("INSERT INTO linggan_creation_experience_fragment(revision_ref,paragraph_ref,experience_ref,experience_revision,body) VALUES($1,$2,$3,$4,$5)")
            .bind(revision).bind(f.paragraph).bind(f.experience).bind(f.revision).bind(&f.text).execute(&mut **tx).await?;
    }
    Ok(())
}

/// Missing, unconfirmed, cross-account and withdrawn references do not expose stored text.
pub(super) async fn project(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    doc: &mut WorkDocument,
) -> Result<(), WorkbenchError> {
    for slot in &doc.experience_slots {
        let row = sqlx::query("SELECT f.body FROM linggan_creation_experience_fragment f JOIN linggan_creation_experience e ON e.experience_ref=f.experience_ref JOIN linggan_creation_experience_consent c ON c.experience_ref=f.experience_ref AND c.revision=f.experience_revision WHERE f.revision_ref=$1 AND f.paragraph_ref=$2 AND f.experience_ref=$3 AND f.experience_revision=$4 AND e.account_ref=$5 AND e.state IN ('confirmed','awaiting_subject') AND c.draft_use")
            .bind(revision).bind(slot.paragraph_id).bind(slot.experience_ref).bind(slot.experience_revision).bind(doc.account_ref).fetch_optional(&mut **tx).await?;
        let text = row
            .map(|r| r.try_get::<String, _>("body"))
            .transpose()?
            .unwrap_or_else(|| WITHDRAWN.into());
        if let Some(p) = doc
            .paragraphs
            .iter_mut()
            .find(|p| p.id == slot.paragraph_id)
        {
            p.text = text;
        }
    }
    Ok(())
}

pub async fn all_available(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    doc: &WorkDocument,
) -> Result<bool, WorkbenchError> {
    for slot in &doc.experience_slots {
        let ok: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_experience_fragment f JOIN linggan_creation_experience e ON e.experience_ref=f.experience_ref JOIN linggan_creation_experience_consent c ON c.experience_ref=f.experience_ref AND c.revision=f.experience_revision WHERE f.revision_ref=$1 AND f.paragraph_ref=$2 AND f.experience_ref=$3 AND f.experience_revision=$4 AND e.account_ref=$5 AND e.state IN ('confirmed','awaiting_subject') AND c.draft_use)")
            .bind(revision).bind(slot.paragraph_id).bind(slot.experience_ref).bind(slot.experience_revision).bind(doc.account_ref).fetch_one(&mut **tx).await?;
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}
