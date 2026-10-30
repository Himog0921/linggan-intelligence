//! Checking a draft and preparing its text for the clipboard. A copy is two facts: a receipt that
//! says which revision was checked under which ruleset, and a confirmation that the text reached
//! the clipboard. Neither is publication, and the text itself is never stored.
use super::checks::check_manuscript;
use super::copy_format::{CopyText, format_copy};
use super::fact_cards::confirmed_terms_in;
use super::identity::latest_in as latest_identity_in;
use super::requests::{Claim, claim, command_hash, lock};
use super::subject::{Context, Subject};
use super::{AssetsError, types::*};
use crate::content_workbench::{
    self as workbench, ContentOrigin, SourceKind, UseTarget, WorkDocument, WorkbenchError,
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
    check_draft_for_work(db, doc, None).await
}

/// When the current work is known, distinguish this draft's own reference from other drafts.
pub async fn check_draft_for_work(
    db: &Database,
    doc: &WorkDocument,
    work: Option<Uuid>,
) -> Result<CheckReport, AssetsError> {
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
    let mut report = check_manuscript(&Subject::from_document(doc), &ctx);
    add_grade_warnings(&mut tx, doc, &mut report).await?;
    add_repeated_experience_warnings(&mut tx, doc, work, &mut report).await?;
    tx.commit().await?;
    Ok(report)
}

async fn add_grade_warnings(
    tx: &mut Transaction<'_, Postgres>,
    doc: &WorkDocument,
    report: &mut CheckReport,
) -> Result<(), AssetsError> {
    const GRADES: [&str; 6] = ["一年级", "二年级", "三年级", "四年级", "五年级", "六年级"];
    let mut mentions = Vec::new();
    for paragraph in &doc.paragraphs {
        for prefix in ["现在", "今年", "已经"] {
            for (start, _) in paragraph.text.match_indices(prefix) {
                let after_prefix = start + prefix.len();
                let tail = &paragraph.text[after_prefix..];
                let trimmed = tail.trim_start_matches(char::is_whitespace);
                let whitespace_bytes = tail.len() - trimmed.len();
                for grade in GRADES {
                    if trimmed.starts_with(grade) {
                        let end = after_prefix + whitespace_bytes + grade.len();
                        let scalar_start = paragraph.text[..start].chars().count();
                        let scalar_end = paragraph.text[..end].chars().count();
                        let quoted_comment = doc.uses.iter().any(|usage| {
                            let UseTarget::Paragraph { paragraph_id } = usage.target else {
                                return false;
                            };
                            if paragraph_id != paragraph.id
                                || usage.content_origin != ContentOrigin::Excerpt
                            {
                                return false;
                            }
                            let Some(span) = &usage.source_span else {
                                return false;
                            };
                            if span.start > scalar_start || span.end < scalar_end {
                                return false;
                            }
                            let Some(excerpt) = doc.excerpts.iter().find(|item| {
                                Some(item.id) == usage.excerpt_id
                                    && item.citation.source_kind == SourceKind::Comment
                            }) else {
                                return false;
                            };
                            let placed: String = paragraph
                                .text
                                .chars()
                                .skip(span.start)
                                .take(span.end.saturating_sub(span.start))
                                .collect();
                            excerpt.selected_text.as_deref() == Some(placed.as_str())
                        });
                        if quoted_comment {
                            continue;
                        }
                        mentions.push((paragraph, start, end, grade));
                    }
                }
            }
        }
    }
    if mentions.is_empty() {
        return Ok(());
    }
    let grade: Option<String> = sqlx::query_scalar("SELECT r.text FROM linggan_creation_persona_fact f JOIN linggan_creation_persona_fact_revision r ON r.fact_ref=f.fact_ref AND r.revision=f.current_revision JOIN linggan_creation_account_person p ON p.account_ref=f.account_ref AND p.subject_ref=f.subject_ref WHERE f.account_ref=$1 AND f.fact_key='年级' AND f.state='active' AND r.status='confirmed' AND (SELECT i.has_real_person FROM linggan_creation_account_identity i WHERE i.account_ref=f.account_ref ORDER BY i.revision_number DESC LIMIT 1) IS TRUE")
        .bind(doc.account_ref).fetch_optional(&mut **tx).await?;
    let Some(grade) = grade.filter(|value| GRADES.contains(&value.trim())) else {
        report.not_run.push(NotRun {
            rule: "R-W4".into(),
            reason: "账号的年级事实尚未确认，无法检查年级是否一致".into(),
        });
        return Ok(());
    };
    let mut emitted = 0;
    for (paragraph, start, end, mentioned) in mentions {
        if mentioned == grade.trim() {
            continue;
        }
        if emitted >= super::words::MAX_FINDINGS_PER_RULE {
            break;
        }
        let finding = Finding {
            rule: "R-W4".into(),
            level: Level::Warn,
            message: "这里的年级与账号已确认的年级不同，请结合引用或叙述对象核对。".into(),
            location: Location::Paragraph {
                id: paragraph.id,
                start: paragraph.text[..start].encode_utf16().count(),
                end: paragraph.text[..end].encode_utf16().count(),
            },
            fixes: vec![Fix {
                label: "找到这段".into(),
                action: FixAction::EditByHand,
            }],
        };
        let position = report
            .findings
            .iter()
            .position(|item| {
                matches!(
                    item.rule.as_str(),
                    "R-W5" | "R-W6" | "R-W7" | "R-W8" | "R-W9" | "R-I1" | "R-I2"
                )
            })
            .unwrap_or(report.findings.len());
        report.findings.insert(position, finding);
        emitted += 1;
    }
    Ok(())
}

async fn add_repeated_experience_warnings(
    tx: &mut Transaction<'_, Postgres>,
    doc: &WorkDocument,
    work: Option<Uuid>,
    report: &mut CheckReport,
) -> Result<(), AssetsError> {
    if doc.experience_slots.is_empty() {
        return Ok(());
    }
    let Some(work) = work else {
        report.not_run.push(NotRun {
            rule: "R-W5".into(),
            reason: "未指定当前稿件，无法检查经历是否在其他稿件中引用".into(),
        });
        return Ok(());
    };
    let refs: Vec<String> = doc
        .experience_slots
        .iter()
        .map(|slot| slot.experience_ref.to_string())
        .collect();
    let account = doc.account_ref.map(|id| id.to_string());
    let repeated: Vec<String> = sqlx::query_scalar("SELECT DISTINCT slot->>'experienceRef' FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.revision_ref=w.current_revision_ref CROSS JOIN LATERAL jsonb_array_elements(COALESCE(r.document->'experienceSlots','[]'::jsonb)) slot WHERE w.work_ref<>$1 AND r.document->>'accountRef'=$2 AND slot->>'experienceRef'=ANY($3)")
        .bind(work).bind(account).bind(&refs).fetch_all(&mut **tx).await?;
    let mut emitted = 0;
    for slot in &doc.experience_slots {
        if emitted >= super::words::MAX_FINDINGS_PER_RULE {
            break;
        }
        if !repeated.contains(&slot.experience_ref.to_string()) {
            continue;
        }
        if let Some(paragraph) = doc.paragraphs.iter().find(|p| p.id == slot.paragraph_id) {
            let finding = Finding {
                rule: "R-W5".into(),
                level: Level::Warn,
                message: "这段经历已在其他已保存稿件中引用；可以换一段，或换角度和说法。".into(),
                location: Location::Paragraph {
                    id: paragraph.id,
                    start: 0,
                    end: paragraph.text.encode_utf16().count(),
                },
                fixes: vec![Fix {
                    label: "找到这段".into(),
                    action: FixAction::EditByHand,
                }],
            };
            let position = report
                .findings
                .iter()
                .position(|item| {
                    matches!(
                        item.rule.as_str(),
                        "R-W6" | "R-W7" | "R-W8" | "R-W9" | "R-I1" | "R-I2"
                    )
                })
                .unwrap_or(report.findings.len());
            report.findings.insert(position, finding);
            emitted += 1;
        }
    }
    Ok(())
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
    ruleset_version: String,
    warn: usize,
    hint: usize,
    replayed: bool,
) -> CopyReady {
    CopyReady {
        receipt_ref,
        work_id: work,
        revision_id: revision,
        ruleset_version,
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
        "SELECT receipt_ref,revision_ref,ruleset_version,warn_rules,hint_rules,text_sha256 FROM linggan_creation_copy_receipt WHERE request_ref=$1 AND work_ref=$2",
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
        row.try_get("ruleset_version")?,
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
    let report_doc = revision.document.clone();
    let subject = copy_subject(&mut tx, domain, revision, &ctx).await?;
    let mut report = check_manuscript(&subject, &ctx);
    add_grade_warnings(&mut tx, &report_doc, &mut report).await?;
    add_repeated_experience_warnings(&mut tx, &report_doc, Some(work), &mut report).await?;
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
    for slot in &report_doc.experience_slots {
        sqlx::query("INSERT INTO linggan_creation_experience_copy_link(receipt_ref,experience_ref) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(receipt).bind(slot.experience_ref).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(ready(
        receipt,
        work,
        revision_id,
        copy,
        report.ruleset_version.clone(),
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

/// Prefer a confirmed copy of the current revision; otherwise return the latest older copy.
pub async fn copy_status(db: &Database, work: Uuid) -> Result<CopyStatus, AssetsError> {
    let row = sqlx::query(
        "SELECT r.receipt_ref,r.revision_ref,c.copied_at::text AS copied_at FROM linggan_creation_copy_confirmation c JOIN linggan_creation_copy_receipt r ON r.receipt_ref=c.receipt_ref JOIN linggan_creation_work w ON w.work_ref=r.work_ref WHERE r.work_ref=$1 ORDER BY (r.revision_ref=w.current_revision_ref) DESC,c.copied_at DESC,r.created_at DESC LIMIT 1",
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
