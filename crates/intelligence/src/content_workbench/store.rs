use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use super::requests::{RequestOwner, ensure_request_owner_in, lock, receipt, replay};
use super::source_usage::{prepare, raw_present, redact, redact_title_in, target_mut};

fn stored_document(value: serde_json::Value) -> Result<WorkDocument, WorkbenchError> {
    match serde_json::from_value(value.clone()) {
        Ok(document) => Ok(document),
        // Pre-dossier workbench revisions did not persist the immutable block and
        // as-of identity required by the current citation contract.  They cannot
        // be safely requalified as current source material, so keep the original
        // record intact and distinguish it from a transient database failure.
        Err(_)
            if value
                .get("excerpts")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|excerpts| {
                    excerpts.iter().any(|excerpt| {
                        excerpt
                            .get("citation")
                            .and_then(serde_json::Value::as_object)
                            .is_some_and(|citation| {
                                !citation.contains_key("blockRef") || !citation.contains_key("asOf")
                            })
                    })
                }) =>
        {
            Err(WorkbenchError::LegacyReferenceUnavailable)
        }
        Err(error) => Err(error.into()),
    }
}

pub async fn read_revision_in(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    id: Option<Uuid>,
) -> Result<(Uuid, Revision), WorkbenchError> {
    let row=sqlx::query("SELECT w.domain_ref,r.revision_ref,r.revision_number,r.saved_at::text,r.document FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.work_ref=w.work_ref AND r.revision_ref=COALESCE($2,w.current_revision_ref) WHERE w.work_ref=$1").bind(work).bind(id).fetch_optional(&mut **tx).await?.ok_or(WorkbenchError::NotFound)?;
    let domain = row.try_get("domain_ref")?;
    let mut revision = Revision {
        id: row.try_get("revision_ref")?,
        number: row.try_get("revision_number")?,
        saved_at: row.try_get("saved_at")?,
        document: stored_document(row.try_get("document")?)?,
    };
    super::experience_fragments::project(tx, revision.id, &mut revision.document).await?;
    Ok((domain, revision))
}
pub async fn list_works(
    db: &Database,
    domain: Option<Uuid>,
) -> Result<Vec<WorkSummary>, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let rows = sqlx::query("SELECT w.work_ref,w.domain_ref,r.revision_ref,r.saved_at::text,r.document, (SELECT cr.revision_ref FROM linggan_creation_copy_receipt cr JOIN linggan_creation_copy_confirmation cc ON cc.receipt_ref=cr.receipt_ref WHERE cr.work_ref=w.work_ref AND cr.revision_ref=r.revision_ref LIMIT 1) AS copied_revision_id FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.revision_ref=w.current_revision_ref WHERE ($1::uuid IS NULL OR w.domain_ref=$1) ORDER BY r.saved_at DESC,w.work_ref LIMIT 200")
        .bind(domain).fetch_all(&mut *tx).await?;
    let mut summaries = Vec::new();
    for row in rows {
        let work_id = row.try_get("work_ref")?;
        let domain_ref = row.try_get("domain_ref")?;
        let revision_id = row.try_get("revision_ref")?;
        let saved_at = row.try_get("saved_at")?;
        let (title, idea_ref) = match stored_document(row.try_get("document")?) {
            Ok(document) => (
                redact_title_in(&mut tx, domain_ref, &document).await?,
                document.idea_ref,
            ),
            Err(WorkbenchError::LegacyReferenceUnavailable) => {
                ("[历史作品：引用版本无法安全读取]".into(), None)
            }
            Err(error) => return Err(error),
        };
        summaries.push(WorkSummary {
            work_id,
            domain_ref,
            title,
            revision_id,
            saved_at,
            copied_revision_id: row.try_get("copied_revision_id")?,
            idea_ref,
        });
    }
    summaries.sort_by(|a, b| {
        b.saved_at
            .cmp(&a.saved_at)
            .then_with(|| a.work_id.cmp(&b.work_id))
    });
    tx.commit().await?;
    Ok(summaries)
}

/// Idea names and intent are read through the anchor work's ordinary source projection.
pub async fn list_idea_boxes(
    db: &Database,
    domain: Uuid,
) -> Result<Vec<IdeaBoxSummary>, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let rows = sqlx::query("SELECT idea_ref,anchor_work_ref FROM linggan_creation_idea_box WHERE domain_ref=$1 ORDER BY created_at DESC,idea_ref")
        .bind(domain).fetch_all(&mut *tx).await?;
    let mut result = Vec::new();
    for row in rows {
        let idea_ref: Uuid = row.try_get("idea_ref")?;
        let anchor_work_id: Uuid = row.try_get("anchor_work_ref")?;
        let (_, mut revision) = read_revision_in(&mut tx, anchor_work_id, None).await?;
        redact(&mut tx, domain, &mut revision.document).await?;
        if revision.document.idea_ref != Some(idea_ref) {
            return Err(WorkbenchError::Invalid("idea_anchor"));
        }
        let name = revision
            .document
            .idea_name
            .ok_or(WorkbenchError::Invalid("idea_anchor"))?;
        result.push(IdeaBoxSummary {
            idea_ref,
            anchor_work_id,
            anchor_revision_id: revision.id,
            saved_at: revision.saved_at,
            name,
            intent: revision.document.angle,
        });
    }
    tx.commit().await?;
    Ok(result)
}

async fn ensure_idea_box(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    doc: &WorkDocument,
) -> Result<(), WorkbenchError> {
    let existing_anchor: Option<Uuid> = sqlx::query_scalar(
        "SELECT idea_ref FROM linggan_creation_idea_box WHERE anchor_work_ref=$1",
    )
    .bind(work)
    .fetch_optional(&mut **tx)
    .await?;
    if existing_anchor.is_some() && existing_anchor != doc.idea_ref {
        return Err(WorkbenchError::Invalid("idea_anchor"));
    }
    let Some(idea) = doc.idea_ref else {
        return Ok(());
    };
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation_idea:{idea}"))
        .execute(&mut **tx)
        .await?;
    let domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM linggan_creation_work WHERE work_ref=$1")
            .bind(work)
            .fetch_one(&mut **tx)
            .await?;
    if doc.idea_name.is_some() {
        sqlx::query("INSERT INTO linggan_creation_idea_box(idea_ref,domain_ref,anchor_work_ref) VALUES($1,$2,$3) ON CONFLICT (idea_ref) DO NOTHING")
            .bind(idea).bind(domain).bind(work).execute(&mut **tx).await?;
    }
    let row = sqlx::query(
        "SELECT domain_ref,anchor_work_ref FROM linggan_creation_idea_box WHERE idea_ref=$1",
    )
    .bind(idea)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(WorkbenchError::Invalid("idea_not_found"))?;
    let anchor: Uuid = row.try_get("anchor_work_ref")?;
    if row.try_get::<Uuid, _>("domain_ref")? != domain
        || (anchor == work) != doc.idea_name.is_some()
    {
        return Err(WorkbenchError::Invalid("idea_anchor"));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(DISTINCT account_ref) FROM (SELECT r.document->>'accountRef' AS account_ref FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.revision_ref=w.current_revision_ref WHERE w.domain_ref=$1 AND w.work_ref<>$2 AND r.document->>'ideaRef'=$3 UNION ALL SELECT $4::text) accounts WHERE account_ref IS NOT NULL")
        .bind(domain).bind(work).bind(idea.to_string()).bind(doc.account_ref.map(|id| id.to_string())).fetch_one(&mut **tx).await?;
    if count > 3 {
        return Err(WorkbenchError::Invalid("idea_account_limit"));
    }
    Ok(())
}

pub async fn read_work(db: &Database, work: Uuid) -> Result<WorkEnvelope, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let (domain, mut revision) = read_revision_in(&mut tx, work, None).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *tx)
        .await?;
    let experience_reset_required = !super::experience_fragments::withdrawn_history(&mut tx, work)
        .await?
        .is_empty();
    let statuses = redact(&mut tx, domain, &mut revision.document).await?;
    let exports = super::exports::exports_in(&mut tx, work).await?;
    tx.commit().await?;
    Ok(WorkEnvelope {
        work_id: work,
        domain_ref: domain,
        saved_by: "local_machine".into(),
        revision,
        experience_reset_required,
        source_statuses: statuses,
        exports,
    })
}
pub async fn revisions(db: &Database, work: Uuid) -> Result<Vec<Revision>, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT revision_ref FROM linggan_creation_revision WHERE work_ref=$1 ORDER BY revision_number DESC LIMIT 200").bind(work).fetch_all(&mut *tx).await?;
    let mut out = Vec::new();
    for id in ids {
        let (domain, mut r) = read_revision_in(&mut tx, work, Some(id)).await?;
        redact(&mut tx, domain, &mut r.document).await?;
        out.push(r);
    }
    tx.commit().await?;
    Ok(out)
}

async fn append(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    previous: Option<&Revision>,
    doc: WorkDocument,
) -> Result<Revision, WorkbenchError> {
    let id = Uuid::new_v4();
    let number = previous.map_or(1, |r| r.number + 1);
    let (stored, fragments) =
        super::experience_fragments::prepare(tx, work, previous.map(|r| &r.document), &doc).await?;
    // Experience reads and writes take the shared gate first; keep that order for idea links too.
    ensure_idea_box(tx, work, &doc).await?;
    let bytes = serde_json::to_vec(&stored)?;
    let saved_at:String=sqlx::query_scalar("INSERT INTO linggan_creation_revision(revision_ref,work_ref,revision_number,previous_revision_ref,document,document_sha256) VALUES($1,$2,$3,$4,$5,$6) RETURNING saved_at::text").bind(id).bind(work).bind(number).bind(previous.map(|r|r.id)).bind(serde_json::to_value(&stored)?).bind(hash(bytes)).fetch_one(&mut **tx).await?;
    super::experience_fragments::insert(tx, id, &fragments).await?;
    sqlx::query("UPDATE linggan_creation_work SET current_revision_ref=$2 WHERE work_ref=$1")
        .bind(work)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(Revision {
        id,
        number,
        saved_at,
        document: doc,
    })
}
pub async fn create_work(db: &Database, r: &CreateRequest) -> Result<SaveReceipt, WorkbenchError> {
    let command = hash(serde_json::to_vec(&json!(["create", r]))?);
    let mut tx = db.pool().begin().await?;
    ensure_request_owner_in(&mut tx, r.request_id, RequestOwner::Work).await?;
    let mut result = if let Some(r) = replay(&mut tx, r.request_id, &command).await? {
        r
    } else {
        let domain_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)",
        )
        .bind(r.domain_ref)
        .fetch_one(&mut *tx)
        .await?;
        if !domain_exists {
            return Err(WorkbenchError::Invalid("domainRef"));
        }
        let work = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_creation_work(work_ref,domain_ref,saved_by) VALUES($1,$2,'local_machine')").bind(work).bind(r.domain_ref).execute(&mut *tx).await?;
        let mut doc = r.document.clone();
        prepare(&mut tx, work, r.domain_ref, None, &mut doc).await?;
        let revision = append(&mut tx, work, None, doc).await?;
        receipt(
            &mut tx,
            work,
            r.request_id,
            &command,
            SaveReceipt {
                request_id: r.request_id,
                work_id: work,
                outcome: "accepted".into(),
                replayed: false,
                revision,
            },
        )
        .await?
    };
    redact(&mut tx, r.domain_ref, &mut result.revision.document).await?;
    tx.commit().await?;
    Ok(result)
}
pub async fn save(
    db: &Database,
    work: Uuid,
    r: &SaveRequest,
) -> Result<SaveReceipt, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let command = hash(serde_json::to_vec(&json!(["save", work, r]))?);
    let mut out = save_in(
        &mut tx,
        work,
        r.request_id,
        r.expected_revision,
        r.document.clone(),
        &command,
        None,
        false,
        false,
    )
    .await?;
    let (domain, _) = read_revision_in(&mut tx, work, None).await?;
    redact(&mut tx, domain, &mut out.revision.document).await?;
    tx.commit().await?;
    Ok(out)
}

#[cfg(test)]
mod stored_document_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_citation_without_immutable_source_identity_is_distinguished() {
        let mut value = serde_json::to_value(WorkDocument::default()).unwrap();
        value["excerpts"] = json!([{
            "id": Uuid::new_v4(),
            "citation": {
                "sourceKind": "note",
                "sourceRef": Uuid::new_v4(),
                "workRef": Uuid::new_v4(),
                "field": "body",
                "sourceVersion": "legacy",
                "start": 0,
                "end": 1,
                "textHash": "hash",
                "location": null
            },
            "selectedText": "x",
            "textHash": "hash"
        }]);

        assert!(matches!(
            stored_document(value),
            Err(WorkbenchError::LegacyReferenceUnavailable)
        ));
    }

    #[test]
    fn current_document_still_decodes_strictly() {
        assert!(stored_document(serde_json::to_value(WorkDocument::default()).unwrap()).is_ok());
    }
}
async fn save_in(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    request: Uuid,
    expected: Uuid,
    mut doc: WorkDocument,
    command: &str,
    adoption: Option<&AdoptionLink>,
    force_revision: bool,
    allow_epoch_rotation: bool,
) -> Result<SaveReceipt, WorkbenchError> {
    ensure_request_owner_in(tx, request, RequestOwner::Work).await?;
    lock(tx, work.to_string()).await?;
    if let Some(r) = replay(tx, request, command).await? {
        return Ok(r);
    }
    let (domain, current) = read_revision_in(tx, work, None).await?;
    let (outcome, revision) = if current.id != expected {
        ("stale", current)
    } else {
        if !allow_epoch_rotation && doc.experience_epoch != current.document.experience_epoch {
            return Err(WorkbenchError::Invalid("experience_editor_reset_required"));
        }
        prepare(tx, work, domain, Some(&current.document), &mut doc).await?;
        if doc == current.document && adoption.is_none() && !force_revision {
            ("unchanged", current)
        } else {
            let revision = append(tx, work, Some(&current), doc).await?;
            if let Some(l) = adoption {
                sqlx::query("INSERT INTO linggan_creation_adoption(revision_ref,suggestion_ref,action,input_revision_ref,model_invocation_ref,method_version,work_ref) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(revision.id).bind(l.suggestion_id).bind(&l.action).bind(l.input_revision).bind(l.model_invocation_ref).bind(&l.method_version).bind(work).execute(&mut **tx).await?;
            }
            ("accepted", revision)
        }
    };
    receipt(
        tx,
        work,
        request,
        command,
        SaveReceipt {
            request_id: request,
            work_id: work,
            outcome: outcome.into(),
            replayed: false,
            revision,
        },
    )
    .await
}
pub async fn adopt_in(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    request: Uuid,
    expected: Uuid,
    doc: WorkDocument,
    link: &AdoptionLink,
) -> Result<SaveReceipt, WorkbenchError> {
    if link.input_revision != expected {
        return Err(WorkbenchError::Invalid("inputRevision"));
    }
    let command = hash(serde_json::to_vec(&json!([
        "adopt", work, request, expected, &doc, link
    ]))?);
    let mut receipt = save_in(
        tx,
        work,
        request,
        expected,
        doc,
        &command,
        Some(link),
        true,
        false,
    )
    .await?;
    let (domain, _) = read_revision_in(tx, work, None).await?;
    redact(tx, domain, &mut receipt.revision.document).await?;
    Ok(receipt)
}
pub async fn restore(
    db: &Database,
    work: Uuid,
    r: &RestoreRequest,
) -> Result<SaveReceipt, WorkbenchError> {
    let mut tx = db.pool().begin().await?;
    let command = hash(serde_json::to_vec(&json!(["restore", work, r]))?);
    ensure_request_owner_in(&mut tx, r.request_id, RequestOwner::Work).await?;
    lock(&mut tx, work.to_string()).await?;
    let (_, mut revision) = read_revision_in(&mut tx, work, Some(r.restore_revision)).await?;
    let (_, current) = read_revision_in(&mut tx, work, None).await?;
    // Restoring manuscript content must not silently sever or change its current idea box.
    revision.document.idea_ref = current.document.idea_ref;
    revision.document.idea_name = current.document.idea_name.clone();
    if current.document.idea_name.is_some() {
        revision.document.angle = current.document.angle.clone();
    }
    let mut out = save_in(
        &mut tx,
        work,
        r.request_id,
        r.expected_revision,
        revision.document,
        &command,
        None,
        true,
        false,
    )
    .await?;
    let (domain, _) = read_revision_in(&mut tx, work, None).await?;
    redact(&mut tx, domain, &mut out.revision.document).await?;
    tx.commit().await?;
    Ok(out)
}

/// Resume after withdrawal from the stored server revision, never from the editor's stale body.
pub async fn reset_withdrawn(
    db: &Database,
    work: Uuid,
    r: &ResetWithdrawnRequest,
) -> Result<SaveReceipt, WorkbenchError> {
    let command = hash(serde_json::to_vec(&json!(["reset_withdrawn", work, r]))?);
    let mut tx = db.pool().begin().await?;
    ensure_request_owner_in(&mut tx, r.request_id, RequestOwner::Work).await?;
    lock(&mut tx, work.to_string()).await?;
    if let Some(mut result) = replay(&mut tx, r.request_id, &command).await? {
        let (domain, _) = read_revision_in(&mut tx, work, None).await?;
        redact(&mut tx, domain, &mut result.revision.document).await?;
        tx.commit().await?;
        return Ok(result);
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *tx)
        .await?;
    let (domain, current) = read_revision_in(&mut tx, work, None).await?;
    let mut doc = current.document.clone();
    if current.id == r.expected_revision {
        let withdrawn = super::experience_fragments::withdrawn_history(&mut tx, work).await?;
        if withdrawn.is_empty() {
            return Err(WorkbenchError::Invalid("experience_reset_not_required"));
        }
        doc.experience_epoch = Some(Uuid::new_v4());
        let removed: Vec<Uuid> = doc
            .experience_slots
            .iter()
            .filter(|slot| withdrawn.contains(&slot.experience_ref))
            .map(|slot| slot.paragraph_id)
            .collect();
        doc.paragraphs
            .retain(|paragraph| !removed.contains(&paragraph.id));
        doc.experience_slots
            .retain(|slot| !withdrawn.contains(&slot.experience_ref));
        doc.experience_gaps.retain(|id| !removed.contains(id));
        doc.uses.retain(|usage| !matches!(usage.target, UseTarget::Paragraph { paragraph_id } if removed.contains(&paragraph_id)));
        for id in withdrawn {
            sqlx::query("INSERT INTO linggan_creation_experience_reset(request_ref,work_ref,experience_ref) VALUES($1,$2,$3)")
                .bind(r.request_id).bind(work).bind(id).execute(&mut *tx).await?;
        }
    }
    let mut out = save_in(
        &mut tx,
        work,
        r.request_id,
        r.expected_revision,
        doc,
        &command,
        None,
        true,
        true,
    )
    .await?;
    redact(&mut tx, domain, &mut out.revision.document).await?;
    tx.commit().await?;
    Ok(out)
}
pub async fn place(
    db: &Database,
    work: Uuid,
    r: &PlaceRequest,
) -> Result<SaveReceipt, WorkbenchError> {
    let command = hash(serde_json::to_vec(&json!(["place", work, r]))?);
    let mut tx = db.pool().begin().await?;
    ensure_request_owner_in(&mut tx, r.request_id, RequestOwner::Work).await?;
    lock(&mut tx, work.to_string()).await?;
    let mut out = if let Some(out) = replay(&mut tx, r.request_id, &command).await? {
        out
    } else {
        let (domain, current) = read_revision_in(&mut tx, work, None).await?;
        if current.id != r.expected_revision {
            receipt(
                &mut tx,
                work,
                r.request_id,
                &command,
                SaveReceipt {
                    request_id: r.request_id,
                    work_id: work,
                    outcome: "stale".into(),
                    replayed: false,
                    revision: current,
                },
            )
            .await?
        } else {
            let mut doc = current.document.clone();
            let c = doc
                .canvas
                .iter()
                .find(|c| c.id == r.canvas_id)
                .cloned()
                .ok_or(WorkbenchError::Invalid("canvasId"))?;
            if r.targets.is_empty() || r.targets.len() > 30 {
                return Err(WorkbenchError::Invalid("targets"));
            }
            let text = if c.expression.is_empty() {
                let e = doc
                    .excerpts
                    .iter()
                    .find(|e| Some(e.id) == c.excerpt_id)
                    .ok_or(WorkbenchError::Invalid("excerptId"))?;
                let v = crate::content_workbench_sources::validate_citation_in_domain(
                    &mut tx,
                    &e.citation,
                    domain,
                )
                .await
                .map_err(|_| WorkbenchError::Source)?;
                v.selected_text
            } else {
                if let Some(id) = c.excerpt_id {
                    let e = doc
                        .excerpts
                        .iter()
                        .find(|e| e.id == id)
                        .ok_or(WorkbenchError::Invalid("excerptId"))?;
                    crate::content_workbench_sources::validate_citation_in_domain(
                        &mut tx,
                        &e.citation,
                        domain,
                    )
                    .await
                    .map_err(|_| WorkbenchError::Source)?;
                }
                c.expression.clone()
            };
            for target in &r.targets {
                if validation::target_text(&doc, target).is_none() {
                    return Err(WorkbenchError::Invalid("target"));
                }
            }
            for target in &r.targets {
                if doc.uses.iter().any(|u| {
                    u.canvas_id == c.id
                        && u.expression_revision == c.expression_revision
                        && u.target == *target
                }) {
                    continue;
                }
                let dest = target_mut(&mut doc, target).ok_or(WorkbenchError::Invalid("target"))?;
                if !dest.is_empty() {
                    dest.push_str("\n\n");
                }
                dest.push_str(&text);
                let placed = hash(dest.as_bytes());
                doc.uses.push(MaterialUse {
                    source_span: None,
                    id: Uuid::new_v4(),
                    canvas_id: c.id,
                    expression_revision: c.expression_revision,
                    excerpt_id: c.excerpt_id,
                    target: target.clone(),
                    placed_text_hash: placed,
                    state: UseState::Current,
                    content_origin: if c.expression.is_empty()
                        || doc
                            .excerpts
                            .iter()
                            .find(|e| Some(e.id) == c.excerpt_id)
                            .and_then(|e| e.selected_text.as_deref())
                            .is_some_and(|raw| raw_present(&text, raw))
                    {
                        ContentOrigin::Excerpt
                    } else {
                        ContentOrigin::Expression
                    },
                });
            }
            prepare(&mut tx, work, domain, Some(&current.document), &mut doc).await?;
            let revision = if doc == current.document {
                current
            } else {
                append(&mut tx, work, Some(&current), doc).await?
            };
            receipt(
                &mut tx,
                work,
                r.request_id,
                &command,
                SaveReceipt {
                    request_id: r.request_id,
                    work_id: work,
                    outcome: "accepted".into(),
                    replayed: false,
                    revision,
                },
            )
            .await?
        }
    };
    let (domain, _) = read_revision_in(&mut tx, work, None).await?;
    redact(&mut tx, domain, &mut out.revision.document).await?;
    tx.commit().await?;
    Ok(out)
}
