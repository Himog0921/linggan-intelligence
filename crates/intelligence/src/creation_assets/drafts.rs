//! Checking a draft and preparing its text for the clipboard. A copy is two facts: a receipt that
//! says which revision was checked under which ruleset, and a confirmation that the text reached
//! the clipboard. Neither is publication, and the text itself is never stored.
use super::checks::check_manuscript;
use super::copy_format::{CopyText, format_copy};
use super::fact_cards::confirmed_terms_in;
use super::identity::latest_in as latest_identity_in;
use super::requests::{Claim, claim, command_hash, lock};
use super::subject::{Context, Subject};
use super::{AssetsError, types::*, words::RULESET_VERSION};
use crate::content_workbench::{
    self as workbench, ContentOrigin, UseTarget, WorkDocument, WorkbenchError,
};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

fn workbench_error(error: WorkbenchError) -> AssetsError {
    match error {
        WorkbenchError::NotFound => AssetsError::NotFound,
        WorkbenchError::Invalid(_) => AssetsError::Invalid("document"),
        other => AssetsError::Workbench(other),
    }
}

async fn load_context(
    tx: &mut Transaction<'_, Postgres>,
    account: Option<Uuid>,
) -> Result<Context, AssetsError> {
    let identity = match account {
        Some(account) => latest_identity_in(tx, account).await?,
        None => None,
    };
    Ok(Context {
        identity,
        cards: confirmed_terms_in(tx).await?,
    })
}

/// Runs every manuscript rule against a draft as the editor currently holds it, with the account
/// identity and the confirmed fact cards as they are now.
pub async fn check_draft(db: &Database, doc: &WorkDocument) -> Result<CheckReport, AssetsError> {
    // Unsaved editor input may contain duplicate topics, which this check must report.
    // Keep the normal size limit and validate the remaining document on a deduplicated copy.
    if doc.tags.len() > 30 {
        return Err(AssetsError::Invalid("document"));
    }
    let mut valid_shape = doc.clone();
    let mut seen = std::collections::HashSet::new();
    valid_shape.tags.retain(|tag| seen.insert(tag.clone()));
    workbench::validate_document(&valid_shape).map_err(workbench_error)?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let ctx = load_context(&mut tx, doc.account_ref).await?;
    tx.commit().await?;
    Ok(check_manuscript(&Subject::from_document(doc), &ctx))
}

fn text_sha(copy: &CopyText) -> String {
    crate::content_workbench::hash(format!("{}\n{}", copy.title, copy.text))
}

/// A saved revision contains its original source text. Requalify it before both the first copy
/// and a replay, and refuse if the copy would include text hidden by the workbench projection.
async fn copy_subject(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    mut revision: workbench::Revision,
    ctx: &Context,
) -> Result<Subject, AssetsError> {
    if !workbench::experience_fragments_available_in(tx, revision.id, &revision.document)
        .await
        .map_err(workbench_error)?
    {
        return Err(AssetsError::SourceUnavailable);
    }
    let original_document = revision.document.clone();
    let original = Subject::from_document(&original_document);
    let statuses = workbench::redact_revision_with_statuses_in(tx, domain, &mut revision)
        .await
        .map_err(workbench_error)?;
    let public = Subject::from_document(&revision.document);
    let restricted_use = original_document.uses.iter().any(|usage| {
        usage.content_origin == ContentOrigin::Excerpt
            && statuses.iter().any(|status| {
                usage.excerpt_id == Some(status.excerpt_id) && status.state != "readable"
            })
            && match usage.target {
                UseTarget::Title => true,
                UseTarget::Paragraph { paragraph_id } => original_document
                    .paragraphs
                    .iter()
                    .any(|p| p.id == paragraph_id),
                UseTarget::Tag { index } => index < original_document.tags.len(),
                _ => false,
            }
    });
    let restricted_quote = original_document.excerpts.iter().any(|excerpt| {
        let unavailable = statuses
            .iter()
            .any(|status| status.excerpt_id == excerpt.id && status.state != "readable");
        unavailable
            && excerpt.selected_text.as_deref().is_some_and(|raw| {
                !raw.is_empty()
                    && (original.title.contains(raw)
                        || original
                            .paragraphs
                            .iter()
                            .any(|(_, text)| text.contains(raw))
                        || original.tags.iter().any(|tag| tag.contains(raw))
                        || ctx
                            .bridge_output(&original)
                            .is_some_and(|(text, _)| text.contains(raw)))
            })
    });
    if original.title != public.title
        || original.paragraphs != public.paragraphs
        || original.tags != public.tags
        || restricted_use
        || restricted_quote
    {
        return Err(AssetsError::SourceUnavailable);
    }
    Ok(public)
}

fn rule_counts(report: &CheckReport, level: Level) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for finding in report.findings.iter().filter(|f| f.level == level) {
        *counts.entry(finding.rule.clone()).or_default() += 1;
    }
    counts
}

fn ready(
    receipt_ref: Uuid,
    work: Uuid,
    revision: Uuid,
    copy: CopyText,
    warn: usize,
    hint: usize,
    replayed: bool,
) -> CopyReady {
    CopyReady {
        receipt_ref,
        work_id: work,
        revision_id: revision,
        ruleset_version: RULESET_VERSION.into(),
        title: copy.title,
        body: copy.body,
        tags: copy.tags,
        text: copy.text,
        warn_count: warn,
        hint_count: hint,
        replayed,
    }
}

fn total(counts: serde_json::Value) -> usize {
    counts
        .as_object()
        .map(|map| {
            map.values()
                .filter_map(serde_json::Value::as_u64)
                .map(|n| n as usize)
                .sum()
        })
        .unwrap_or(0)
}

/// The same command again hands back the same text, or reports the draft as stale when the
/// assets it was checked against have changed the text since.
async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    request: Uuid,
) -> Result<CopyReady, AssetsError> {
    let row = sqlx::query(
        "SELECT receipt_ref,revision_ref,warn_rules,hint_rules,text_sha256 FROM linggan_creation_copy_receipt WHERE request_ref=$1 AND work_ref=$2",
    )
    .bind(request)
    .bind(work)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AssetsError::NotFound)?;
    let revision_ref: Uuid = row.try_get("revision_ref")?;
    let (domain, revision) = workbench::read_revision_in(tx, work, Some(revision_ref))
        .await
        .map_err(workbench_error)?;
    let ctx = load_context(tx, revision.document.account_ref).await?;
    let subject = copy_subject(tx, domain, revision, &ctx).await?;
    let copy = format_copy(&subject, ctx.bridge_output(&subject)).ok_or(AssetsError::Stale)?;
    if text_sha(&copy) != row.try_get::<String, _>("text_sha256")? {
        return Err(AssetsError::Stale);
    }
    Ok(ready(
        row.try_get("receipt_ref")?,
        work,
        revision_ref,
        copy,
        total(row.try_get("warn_rules")?),
        total(row.try_get("hint_rules")?),
        true,
    ))
}

/// Checks the saved current revision and, when nothing must still be fixed, returns the text for
/// the clipboard and writes the receipt. `expected_revision` must be the current revision.
pub async fn prepare_copy(
    db: &Database,
    work: Uuid,
    cmd: &CopyCommand,
) -> Result<CopyReady, AssetsError> {
    let sha = command_hash("copy_prepare", &(work, cmd.expected_revision))?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "copy_prepare", &sha).await? {
        let done = replay(&mut tx, work, cmd.request_id).await?;
        tx.commit().await?;
        return Ok(done);
    }
    lock(&mut tx, &format!("copy:{work}")).await?;
    let (domain, revision) = workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(workbench_error)?;
    if revision.id != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let ctx = load_context(&mut tx, revision.document.account_ref).await?;
    let revision_id = revision.id;
    let subject = copy_subject(&mut tx, domain, revision, &ctx).await?;
    let report = check_manuscript(&subject, &ctx);
    if report.count(Level::Must) > 0 {
        return Err(AssetsError::Blocked(report.findings));
    }
    let copy = format_copy(&subject, ctx.bridge_output(&subject))
        .ok_or(AssetsError::Invalid("body_empty"))?;
    let receipt = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_creation_copy_receipt(receipt_ref,work_ref,revision_ref,request_ref,ruleset_version,fact_cards,warn_rules,hint_rules,text_sha256,saved_by)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'local_machine')",
    )
    .bind(receipt)
    .bind(work)
    .bind(revision_id)
    .bind(cmd.request_id)
    .bind(&report.ruleset_version)
    .bind(serde_json::to_value(&report.fact_cards)?)
    .bind(serde_json::to_value(rule_counts(&report, Level::Warn))?)
    .bind(serde_json::to_value(rule_counts(&report, Level::Hint))?)
    .bind(text_sha(&copy))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ready(
        receipt,
        work,
        revision_id,
        copy,
        report.count(Level::Warn),
        report.count(Level::Hint),
        false,
    ))
}

async fn mark_by_request(
    tx: &mut Transaction<'_, Postgres>,
    receipt: Uuid,
) -> Result<CopiedMark, AssetsError> {
    let row = sqlx::query(
        "SELECT r.receipt_ref,r.revision_ref,c.copied_at::text AS copied_at FROM linggan_creation_copy_confirmation c JOIN linggan_creation_copy_receipt r ON r.receipt_ref=c.receipt_ref WHERE c.receipt_ref=$1",
    )
    .bind(receipt)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AssetsError::NotFound)?;
    Ok(CopiedMark {
        receipt_ref: row.try_get("receipt_ref")?,
        revision_id: row.try_get("revision_ref")?,
        copied_at: row.try_get("copied_at")?,
    })
}

/// Records that the prepared text reached the clipboard. A second confirmation of the same
/// receipt returns the first one.
pub async fn confirm_copy(
    db: &Database,
    work: Uuid,
    receipt: Uuid,
    cmd: &CopyConfirm,
) -> Result<CopiedMark, AssetsError> {
    let sha = command_hash("copy_confirm", &(work, receipt))?;
    let mut tx = db.pool().begin().await?;
    let claimed = claim(&mut tx, cmd.request_id, "copy_confirm", &sha).await?;
    lock(&mut tx, &format!("copy:{work}")).await?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_copy_receipt WHERE receipt_ref=$1 AND work_ref=$2)",
    )
    .bind(receipt)
    .bind(work)
    .fetch_one(&mut *tx)
    .await?;
    if !known {
        return Err(AssetsError::NotFound);
    }
    if matches!(claimed, Claim::Fresh) {
        sqlx::query(
            "INSERT INTO linggan_creation_copy_confirmation(receipt_ref,request_ref,copied_by) VALUES($1,$2,'local_machine') ON CONFLICT (receipt_ref) DO NOTHING",
        )
        .bind(receipt)
        .bind(cmd.request_id)
        .execute(&mut *tx)
        .await?;
    }
    let mark = mark_by_request(&mut tx, receipt).await?;
    tx.commit().await?;
    Ok(mark)
}

/// The latest confirmed copy of a draft, if any. The editor compares its revision with the
/// current one to tell "已复制" from "草稿中".
pub async fn copy_status(db: &Database, work: Uuid) -> Result<CopyStatus, AssetsError> {
    let row = sqlx::query(
        "SELECT r.receipt_ref,r.revision_ref,c.copied_at::text AS copied_at FROM linggan_creation_copy_confirmation c JOIN linggan_creation_copy_receipt r ON r.receipt_ref=c.receipt_ref WHERE r.work_ref=$1 ORDER BY c.copied_at DESC,r.created_at DESC LIMIT 1",
    )
    .bind(work)
    .fetch_optional(db.pool())
    .await?;
    let copied = row
        .map(|row| -> Result<CopiedMark, sqlx::Error> {
            Ok(CopiedMark {
                receipt_ref: row.try_get("receipt_ref")?,
                revision_id: row.try_get("revision_ref")?,
                copied_at: row.try_get("copied_at")?,
            })
        })
        .transpose()?;
    Ok(CopyStatus { copied })
}
