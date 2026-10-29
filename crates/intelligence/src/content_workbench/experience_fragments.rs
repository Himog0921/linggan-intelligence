//! Draft experience content is stored separately so withdrawal can erase it from every revision.
use super::*;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const WITHDRAWN: &str = "[本人已撤回这段经历]";
const UNAVAILABLE: &str = "[这段经历当前不可使用]";

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

/// Keep every referenced experience and its account identity stable until this transaction ends.
async fn lock_references(
    tx: &mut Transaction<'_, Postgres>,
    old: Option<&WorkDocument>,
    doc: &WorkDocument,
) -> Result<(), WorkbenchError> {
    let mut experiences: Vec<Uuid> = doc
        .experience_slots
        .iter()
        .map(|s| s.experience_ref)
        .chain(
            old.into_iter()
                .flat_map(|o| o.experience_slots.iter().map(|s| s.experience_ref)),
        )
        .collect();
    experiences.sort_unstable();
    experiences.dedup();
    if experiences.is_empty() {
        return Ok(());
    }
    // One gate spans all source locks in a transaction, including historical revisions.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut **tx)
        .await?;
    for id in experiences {
        lock_experience(tx, id).await?;
    }
    let mut accounts: Vec<Uuid> = doc
        .account_ref
        .into_iter()
        .chain(old.and_then(|o| o.account_ref))
        .collect();
    accounts.sort_unstable();
    accounts.dedup();
    for account in accounts {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("creation_assets:identity:{account}"))
            .execute(&mut **tx)
            .await?;
    }
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

/// A known full experience paragraph may occur only in paragraphs retaining that experience's
/// binding. This covers exact moves/copies, not arbitrary human paraphrases or external paste.
fn metadata(doc: &WorkDocument) -> Vec<&str> {
    let mut out = Vec::new();
    if let Some(name) = &doc.idea_name {
        out.push(name.as_str());
    }
    out.extend(doc.cover_title_candidates.iter().map(String::as_str));
    out.extend(doc.note_title_candidates.iter().map(String::as_str));
    if let Some(p) = &doc.material_preparation {
        out.push(p.query.as_str());
        out.extend(p.terms.iter().map(String::as_str));
        out.extend(p.gaps.iter().map(String::as_str));
    }
    for m in &doc.materials {
        out.extend(m.match_reasons.iter().map(String::as_str));
    }
    for e in &doc.excerpts {
        if let Some(text) = &e.selected_text {
            out.push(text.as_str());
        }
    }
    if let Some(b) = doc.bridge.as_ref().and_then(|b| b.sentence.as_ref()) {
        out.push(b.text.as_str());
    }
    out
}
fn occurrences(text: &str, raw: &str) -> usize {
    // A single character is too common to infer a copy inside an unrelated sentence.
    if raw.chars().count() == 1 {
        usize::from(text == raw)
    } else {
        text.matches(raw).count()
    }
}
fn copied_to_unbound_text(
    doc: &WorkDocument,
    old: Option<&WorkDocument>,
    experience: Uuid,
    raw: &str,
) -> bool {
    if raw.is_empty() || raw == WITHDRAWN || raw == UNAVAILABLE {
        return false;
    }
    let count = |document: &WorkDocument| {
        let fields = super::source_usage::authored_targets(document).iter().filter_map(|target| {
            let bound_to_same = matches!(target, UseTarget::Paragraph { paragraph_id }
                if document.experience_slots.iter().any(|slot| slot.paragraph_id == *paragraph_id && slot.experience_ref == experience));
            (!bound_to_same).then(|| super::validation::target_text(document, target)).flatten()
        }).map(|text| occurrences(text, raw)).sum::<usize>();
        fields
            + metadata(document)
                .iter()
                .map(|text| occurrences(text, raw))
                .sum::<usize>()
    };
    count(doc) > old.map_or(0, count)
}

pub(super) async fn prepare(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    old: Option<&WorkDocument>,
    doc: &WorkDocument,
) -> Result<(WorkDocument, Vec<Fragment>), WorkbenchError> {
    // Even an unbound current document can have sensitive experience in its history.
    // Serialize this check with withdrawal before examining any historical fragments.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut **tx)
        .await?;
    lock_references(tx, old, doc).await?;
    if !withdrawn_history(tx, work).await?.is_empty() {
        // Until editors can prove they discarded stale text, do not persist any new revision.
        return Err(WorkbenchError::Invalid(
            "experience_withdrawal_requires_reset",
        ));
    }
    // Removing a reference cannot silently reclassify its existing text as ordinary authored text.
    if let Some(old) = old {
        for slot in &old.experience_slots {
            if let Some(text) = old
                .paragraphs
                .iter()
                .find(|p| p.id == slot.paragraph_id)
                .map(|p| p.text.as_str())
            {
                if copied_to_unbound_text(doc, Some(old), slot.experience_ref, text) {
                    return Err(WorkbenchError::Invalid("experience_untracked_text"));
                }
            }
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
    let mut ids: Vec<Uuid> = doc
        .experience_slots
        .iter()
        .map(|s| s.experience_ref)
        .chain(
            old.into_iter()
                .flat_map(|old| old.experience_slots.iter().map(|s| s.experience_ref)),
        )
        .collect();
    let historical: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT f.experience_ref FROM linggan_creation_experience_fragment f JOIN linggan_creation_revision r ON r.revision_ref=f.revision_ref WHERE r.work_ref=$1")
        .bind(work).fetch_all(&mut **tx).await?;
    ids.extend(historical);
    ids.sort_unstable();
    ids.dedup();
    if !ids.is_empty() {
        let known = sqlx::query("SELECT f.experience_ref,f.body FROM linggan_creation_experience_fragment f JOIN linggan_creation_revision r ON r.revision_ref=f.revision_ref WHERE r.work_ref=$1 AND f.experience_ref=ANY($2) UNION ALL SELECT experience_ref,body FROM linggan_creation_experience_revision WHERE experience_ref=ANY($2)")
            .bind(work).bind(&ids).fetch_all(&mut **tx).await?;
        for row in known {
            let id: Uuid = row.try_get("experience_ref")?;
            let body: String = row.try_get("body")?;
            if copied_to_unbound_text(doc, old, id, &body) {
                return Err(WorkbenchError::Invalid("experience_untracked_text"));
            }
        }
    }
    let mut fragments = Vec::new();
    for slot in &doc.experience_slots {
        let new_target =
            old.is_some_and(|old| !old.experience_slots.iter().any(|existing| existing == slot));
        // A split or move within this work retains the same already-qualified source revision.
        let continuing = old.is_some_and(|old| {
            old.experience_slots.iter().any(|existing| {
                existing.experience_ref == slot.experience_ref
                    && existing.experience_revision == slot.experience_revision
            })
        });
        if new_target && continuing {
            let contents = |document: &WorkDocument| -> String {
                document
                    .paragraphs
                    .iter()
                    .filter(|paragraph| {
                        document.experience_slots.iter().any(|source| {
                            source.paragraph_id == paragraph.id
                                && source.experience_ref == slot.experience_ref
                                && source.experience_revision == slot.experience_revision
                        })
                    })
                    .map(|paragraph| paragraph.text.as_str())
                    .collect()
            };
            if old.is_none_or(|old| contents(old) != contents(doc)) {
                return Err(WorkbenchError::Invalid("experience_split_changed"));
            }
        }
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_experience e JOIN linggan_creation_experience_revision r ON r.experience_ref=e.experience_ref AND r.revision=$2 JOIN linggan_creation_experience_consent c ON c.experience_ref=r.experience_ref AND c.revision=r.revision WHERE e.experience_ref=$1 AND e.account_ref=$3 AND e.state IN ('confirmed','awaiting_subject') AND c.draft_use AND c.subject_ref=e.subject_ref AND EXISTS(SELECT 1 FROM linggan_creation_account_person p WHERE p.account_ref=e.account_ref AND p.subject_ref=e.subject_ref) AND ($4 OR (e.state='confirmed' AND e.current_revision=$2)) AND (SELECT i.has_real_person FROM linggan_creation_account_identity i WHERE i.account_ref=e.account_ref ORDER BY i.revision_number DESC LIMIT 1) IS TRUE)")
            .bind(slot.experience_ref).bind(slot.experience_revision).bind(doc.account_ref).bind(continuing)
            .fetch_one(&mut **tx).await?;
        if !allowed {
            return Err(WorkbenchError::Source);
        }
        let paragraph = doc
            .paragraphs
            .iter()
            .find(|p| p.id == slot.paragraph_id)
            .ok_or(WorkbenchError::Invalid("experience_slot"))?;
        if copied_to_unbound_text(doc, old, slot.experience_ref, &paragraph.text) {
            return Err(WorkbenchError::Invalid("experience_untracked_text"));
        }
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

pub(super) async fn withdrawn_history(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
) -> Result<Vec<Uuid>, WorkbenchError> {
    Ok(sqlx::query_scalar("SELECT DISTINCT e.experience_ref FROM linggan_creation_revision r CROSS JOIN LATERAL jsonb_array_elements(COALESCE(r.document->'experienceSlots','[]'::jsonb)) AS slot(value) JOIN linggan_creation_experience e ON e.experience_ref::text=slot.value->>'experienceRef' WHERE r.work_ref=$1 AND e.state='withdrawn' AND NOT EXISTS (SELECT 1 FROM linggan_creation_experience_reset s WHERE s.work_ref=r.work_ref AND s.experience_ref=e.experience_ref)")
        .bind(work).fetch_all(&mut **tx).await?)
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
    lock_references(tx, None, doc).await?;
    for slot in &doc.experience_slots {
        let row = sqlx::query("SELECT f.body FROM linggan_creation_experience_fragment f JOIN linggan_creation_experience e ON e.experience_ref=f.experience_ref JOIN linggan_creation_experience_consent c ON c.experience_ref=f.experience_ref AND c.revision=f.experience_revision WHERE f.revision_ref=$1 AND f.paragraph_ref=$2 AND f.experience_ref=$3 AND f.experience_revision=$4 AND e.account_ref=$5 AND e.state IN ('confirmed','awaiting_subject') AND c.draft_use AND c.subject_ref=e.subject_ref AND EXISTS(SELECT 1 FROM linggan_creation_account_person p WHERE p.account_ref=e.account_ref AND p.subject_ref=e.subject_ref) AND (SELECT i.has_real_person FROM linggan_creation_account_identity i WHERE i.account_ref=e.account_ref ORDER BY i.revision_number DESC LIMIT 1) IS TRUE")
            .bind(revision).bind(slot.paragraph_id).bind(slot.experience_ref).bind(slot.experience_revision).bind(doc.account_ref).fetch_optional(&mut **tx).await?;
        let mut text = row.map(|r| r.try_get::<String, _>("body")).transpose()?;
        if text.is_none() {
            let state: Option<String> = sqlx::query_scalar(
                "SELECT state FROM linggan_creation_experience WHERE experience_ref=$1",
            )
            .bind(slot.experience_ref)
            .fetch_optional(&mut **tx)
            .await?;
            text = Some(if state.as_deref() == Some("withdrawn") {
                WITHDRAWN.into()
            } else {
                UNAVAILABLE.into()
            });
        }
        if let Some(p) = doc
            .paragraphs
            .iter_mut()
            .find(|p| p.id == slot.paragraph_id)
        {
            p.text = text.expect("missing content is replaced above");
        }
    }
    Ok(())
}

pub async fn all_available(
    tx: &mut Transaction<'_, Postgres>,
    revision: Uuid,
    doc: &WorkDocument,
) -> Result<bool, WorkbenchError> {
    lock_references(tx, None, doc).await?;
    for slot in &doc.experience_slots {
        let ok: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_experience_fragment f JOIN linggan_creation_experience e ON e.experience_ref=f.experience_ref JOIN linggan_creation_experience_consent c ON c.experience_ref=f.experience_ref AND c.revision=f.experience_revision WHERE f.revision_ref=$1 AND f.paragraph_ref=$2 AND f.experience_ref=$3 AND f.experience_revision=$4 AND e.account_ref=$5 AND e.state IN ('confirmed','awaiting_subject') AND c.draft_use AND c.subject_ref=e.subject_ref AND EXISTS(SELECT 1 FROM linggan_creation_account_person p WHERE p.account_ref=e.account_ref AND p.subject_ref=e.subject_ref) AND (SELECT i.has_real_person FROM linggan_creation_account_identity i WHERE i.account_ref=e.account_ref ORDER BY i.revision_number DESC LIMIT 1) IS TRUE)")
            .bind(revision).bind(slot.paragraph_id).bind(slot.experience_ref).bind(slot.experience_revision).bind(doc.account_ref).fetch_one(&mut **tx).await?;
        if !ok {
            return Ok(false);
        }
    }
    Ok(true)
}
