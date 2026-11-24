//! CONTENT-WORKBENCH-001 一题多号: open one draft per account under an existing idea, all or none.
//!
//! The command makes no rules of its own. Each new draft goes through `create_work_as_in`, so the account
//! check, the source checks and the idea's account limit are the ones every other draft already meets.
use super::source_usage::redact;
use super::store::{IDEA_ACCOUNT_LIMIT, create_work_as_in, read_revision_in};
use super::*;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// `open_idea_drafts_as_in` in its own transaction, committed only when every draft was made.
pub async fn open_idea_drafts(
    db: &linggan_storage_postgres::Database,
    idea: Uuid,
    command: &OpenIdeaDraftsRequest,
) -> Result<OpenedIdeaDrafts, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let opened = open_idea_drafts_as_in(&mut tx, idea, command, None).await?;
    tx.commit().await?;
    Ok(opened)
}

/// Opens a draft for each requested account that has none under `idea`, in one transaction: if any of
/// them cannot be made, none is. An account that already writes under the idea keeps its draft and is
/// reported with `created: false`, so asking twice never opens a second one. Sending the same command
/// again (same `requestId`) after a lost response reports the drafts it made as `created: true`, with
/// `replayed: true`, so the client can tell its own drafts from ones that were already there.
///
/// Admin-only by its route. The anchor is read as an admin reads it, comment quotes included; offering
/// this to members would first need the member projection that strips them.
///
/// A new draft takes the idea's working material from its anchor draft: the angle, the research topic link,
/// and the material cards whose source can be read now. It takes none of what the anchor wrote or of
/// what belongs to its account: no paragraphs, pages, experiences, bridge sentence or tags.
pub async fn open_idea_drafts_as_in(
    tx: &mut Transaction<'_, Postgres>,
    idea: Uuid,
    command: &OpenIdeaDraftsRequest,
    member: Option<Uuid>,
) -> Result<OpenedIdeaDrafts, WorkbenchError> {
    let mut accounts = Vec::new();
    for account in &command.account_refs {
        if !accounts.contains(account) {
            accounts.push(*account);
        }
    }
    if accounts.is_empty() || accounts.len() as i64 > IDEA_ACCOUNT_LIMIT {
        return Err(WorkbenchError::Invalid("accounts"));
    }
    // Match append's experience → idea lock order before opening any draft.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut **tx)
        .await?;
    // The same lock `ensure_idea_box` takes, so two openings of one idea cannot interleave.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation_idea:{idea}"))
        .execute(&mut **tx)
        .await?;
    let row = sqlx::query(
        "SELECT domain_ref,anchor_work_ref FROM linggan_creation_idea_box WHERE idea_ref=$1",
    )
    .bind(idea)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(WorkbenchError::Invalid("idea_not_found"))?;
    let domain: Uuid = row.try_get("domain_ref")?;
    let anchor: Uuid = row.try_get("anchor_work_ref")?;

    // The idea's drafts as they stand: one per account, the one most recently worked on.
    let existing: HashMap<Uuid, (Uuid, Uuid)> = sqlx::query(
        "SELECT DISTINCT ON (account) (r.document->>'accountRef')::uuid AS account,w.work_ref,r.revision_ref \
         FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.revision_ref=w.current_revision_ref \
         WHERE w.domain_ref=$1 AND r.document->>'ideaRef'=$2 AND r.document->>'accountRef' IS NOT NULL \
         ORDER BY account,r.saved_at DESC,w.work_ref",
    )
    .bind(domain)
    .bind(idea.to_string())
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|row| {
        Ok((
            row.try_get("account")?,
            (row.try_get("work_ref")?, row.try_get("revision_ref")?),
        ))
    })
    .collect::<Result<_, sqlx::Error>>()?;
    // Drafts that an earlier run of this same command made, found by the identities it gave them.
    let identities: Vec<Uuid> = accounts
        .iter()
        .map(|account| draft_request(command.request_id, *account))
        .collect();
    let made_before: HashMap<Uuid, Uuid> = sqlx::query(
        "SELECT request_ref,work_ref FROM linggan_creation_request WHERE request_ref=ANY($1)",
    )
    .bind(&identities)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|row| Ok((row.try_get("request_ref")?, row.try_get("work_ref")?)))
    .collect::<Result<_, sqlx::Error>>()?;
    let wanted: HashSet<Uuid> = existing.keys().chain(accounts.iter()).copied().collect();
    if wanted.len() as i64 > IDEA_ACCOUNT_LIMIT {
        return Err(WorkbenchError::Invalid("idea_account_limit"));
    }

    // What a new draft starts from is read once, and only when some account needs one.
    let starting = if accounts
        .iter()
        .any(|account| !existing.contains_key(account))
    {
        Some(Starting::from_anchor(tx, domain, anchor, idea).await?)
    } else {
        None
    };
    let skipped_excerpts = starting
        .as_ref()
        .map_or(0, |starting| starting.skipped_excerpts);
    let mut drafts = Vec::new();
    let mut replayed = false;
    for account in accounts {
        if let Some(&work_id) = made_before.get(&draft_request(command.request_id, account)) {
            let revision_id: Uuid = sqlx::query_scalar(
                "SELECT current_revision_ref FROM linggan_creation_work WHERE work_ref=$1",
            )
            .bind(work_id)
            .fetch_one(&mut **tx)
            .await?;
            replayed = true;
            drafts.push(OpenedIdeaDraft {
                work_id,
                account_ref: account,
                created: true,
                revision_id,
            });
            continue;
        }
        if let Some(&(work_id, revision_id)) = existing.get(&account) {
            drafts.push(OpenedIdeaDraft {
                work_id,
                account_ref: account,
                created: false,
                revision_id,
            });
            continue;
        }
        let document = starting
            .as_ref()
            .ok_or(WorkbenchError::Invalid("idea_anchor"))?
            .document_for(account, idea);
        let made = create_work_as_in(
            tx,
            &CreateRequest {
                request_id: draft_request(command.request_id, account),
                domain_ref: domain,
                document,
            },
            member,
        )
        .await?;
        drafts.push(OpenedIdeaDraft {
            work_id: made.work_id,
            account_ref: account,
            created: true,
            revision_id: made.revision.id,
        });
    }
    Ok(OpenedIdeaDrafts {
        request_id: command.request_id,
        idea_ref: idea,
        drafts,
        skipped_excerpts,
        replayed,
    })
}

/// What a new draft under the idea starts from, read once from the idea's anchor draft.
struct Starting {
    name: String,
    angle: String,
    topic_ref: Option<Uuid>,
    topic_angle_refs: Vec<Uuid>,
    excerpts: Vec<Excerpt>,
    canvas: Vec<CanvasBlock>,
    writing_template: Option<WritingTemplateRef>,
    skipped_excerpts: usize,
}
impl Starting {
    async fn from_anchor(
        tx: &mut Transaction<'_, Postgres>,
        domain: Uuid,
        anchor: Uuid,
        idea: Uuid,
    ) -> Result<Self, WorkbenchError> {
        let (_, revision) = read_revision_in(tx, anchor, None).await?;
        let mut source = revision.document;
        if source.idea_ref != Some(idea) {
            return Err(WorkbenchError::Invalid("idea_anchor"));
        }
        // Qualification is read the way every projection reads it; only a source that can be read
        // right now is carried over, never a changed, restricted or unavailable one. The name and the
        // intent are taken from the redacted copy too: an idea may quote a source that is restricted now,
        // and a new draft that dropped that source would no longer know to hide the quote.
        let statuses = redact(tx, domain, &mut source).await?;
        let name = source
            .idea_name
            .take()
            .ok_or(WorkbenchError::Invalid("idea_anchor"))?;
        let readable: HashSet<Uuid> = statuses
            .iter()
            .filter(|status| status.state == "readable")
            .map(|status| status.excerpt_id)
            .collect();
        let excerpts: Vec<Excerpt> = source
            .excerpts
            .iter()
            .filter(|excerpt| readable.contains(&excerpt.id))
            .cloned()
            .collect();
        // A card that quotes an excerpt that stays behind stays behind with it; a note card has no source.
        let canvas = source
            .canvas
            .into_iter()
            .filter(|card| card.excerpt_id.is_none_or(|id| readable.contains(&id)))
            .collect();
        Ok(Self {
            name,
            angle: source.angle,
            topic_ref: source.topic_ref,
            topic_angle_refs: source.topic_angle_refs,
            skipped_excerpts: source.excerpts.len() - excerpts.len(),
            writing_template: source.writing_template,
            excerpts,
            canvas,
        })
    }

    /// The draft for one account. Every excerpt and card gets a new identity: they belong to this draft.
    fn document_for(&self, account: Uuid, idea: Uuid) -> WorkDocument {
        let new_excerpt: HashMap<Uuid, Uuid> = self
            .excerpts
            .iter()
            .map(|excerpt| (excerpt.id, Uuid::new_v4()))
            .collect();
        WorkDocument {
            title: self.name.clone(),
            account_ref: Some(account),
            topic_ref: self.topic_ref,
            topic_angle_refs: self.topic_angle_refs.clone(),
            idea_ref: Some(idea),
            idea_name: None,
            writing_template: self.writing_template.clone(),
            angle: self.angle.clone(),
            excerpts: self
                .excerpts
                .iter()
                .map(|excerpt| Excerpt {
                    id: new_excerpt[&excerpt.id],
                    ..excerpt.clone()
                })
                .collect(),
            canvas: self
                .canvas
                .iter()
                .map(|card| CanvasBlock {
                    id: Uuid::new_v4(),
                    excerpt_id: card.excerpt_id.map(|id| new_excerpt[&id]),
                    ..card.clone()
                })
                .collect(),
            ..WorkDocument::default()
        }
    }
}

/// A request identity for one account's draft that follows from the command's, so every draft the command
/// makes is traceable to it and a repeated command with the same identity makes the same ones.
fn draft_request(command: Uuid, account: Uuid) -> Uuid {
    let digest = Sha256::digest(format!("open_idea_draft:{command}:{account}"));
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drafts_request_identity_follows_from_the_command_and_the_account() {
        let (command, a, b) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        assert_eq!(draft_request(command, a), draft_request(command, a));
        assert_ne!(draft_request(command, a), draft_request(command, b));
        assert_ne!(draft_request(command, a), draft_request(Uuid::new_v4(), a));
    }
}
