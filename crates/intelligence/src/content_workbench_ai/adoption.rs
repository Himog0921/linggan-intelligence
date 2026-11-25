//! Adopt a confirmed suggestion through the existing revision/source provenance transaction.
use super::{
    AdoptRequest, AiError, PreviewRequest, WritingAction, WritingSuggestion,
    outbound_known_citations, qualify_frozen_inputs, selected_text, sources,
};
use crate::content_workbench::{
    self,
    types::{AdoptionLink, Paragraph, SaveReceipt, UseTarget, WorkDocument},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

fn same_rewrite_target(before: &WorkDocument, current: &WorkDocument, id: Uuid) -> bool {
    let target = UseTarget::Paragraph { paragraph_id: id };
    let paragraph = before.paragraphs.iter().find(|p| p.id == id);
    paragraph.is_some()
        && paragraph == current.paragraphs.iter().find(|p| p.id == id)
        && before.account_ref == current.account_ref
        && before.experience_epoch == current.experience_epoch
        && before.experience_gaps.contains(&id) == current.experience_gaps.contains(&id)
        && before
            .experience_slots
            .iter()
            .filter(|s| s.paragraph_id == id)
            .eq(current
                .experience_slots
                .iter()
                .filter(|s| s.paragraph_id == id))
        && before
            .derived_sources
            .iter()
            .filter(|s| s.target == target)
            .eq(current
                .derived_sources
                .iter()
                .filter(|s| s.target == target))
        && before
            .uses
            .iter()
            .filter(|u| u.target == target)
            .map(|u| {
                (
                    u,
                    before.excerpts.iter().find(|e| Some(e.id) == u.excerpt_id),
                )
            })
            .eq(current.uses.iter().filter(|u| u.target == target).map(|u| {
                (
                    u,
                    current.excerpts.iter().find(|e| Some(e.id) == u.excerpt_id),
                )
            }))
}

pub(super) async fn adopt(db: &Database, id: Uuid, r: &AdoptRequest) -> Result<Value, AiError> {
    let mut tx = db.pool().begin().await?;

    content_workbench::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        content_workbench::RequestOwner::Work,
    )
    .await
    .map_err(|e| match e {
        content_workbench::WorkbenchError::Database(e) => AiError::Database(e),
        _ => AiError::Conflict,
    })?;
    // Author edits to a protected suggestion need the same erasable salt as its preview.
    let protected_salt = if r.rewrite_text.is_some() {
        let salt: Option<Option<Uuid>> = sqlx::query_scalar(
            "SELECT private.hash_salt FROM linggan_creation_ai_request request
             JOIN linggan_creation_ai_preview p USING (preview_ref)
             LEFT JOIN linggan_creation_ai_experience_material private USING (preview_ref)
             WHERE request.request_ref=$1 AND p.input_manifest->>'experienceProtected'='true'",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        salt.map(|value| value.ok_or(AiError::Source)).transpose()?
    } else {
        None
    };
    let command_hash = sources::text_hash(&match protected_salt {
        Some(salt) => json!([id, r, salt]).to_string(),
        None => json!([id, r]).to_string(),
    });
    let prior = sqlx::query(
        "SELECT command_hash,receipt FROM linggan_creation_ai_adoption_request WHERE request_ref=$1",
    )
    .bind(r.request_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = prior {
        if row.get::<String, _>("command_hash") != command_hash {
            return Err(AiError::Conflict);
        }
        let mut receipt: SaveReceipt =
            serde_json::from_value(row.get("receipt")).map_err(|_| AiError::Invalid)?;
        let (domain, revision) = content_workbench::read_revision_in(
            &mut tx,
            receipt.work_id,
            Some(receipt.revision.id),
        )
        .await
        .map_err(|_| AiError::NotFound)?;
        receipt.revision = revision;
        content_workbench::redact_revision_in(&mut tx, domain, &mut receipt.revision)
            .await
            .map_err(|_| AiError::Source)?;
        receipt.replayed = true;
        tx.commit().await?;
        return serde_json::to_value(receipt).map_err(|_| AiError::Invalid);
    }
    let row=sqlx::query("SELECT request.state,request.suggestion,request.invocation_ref,p.work_ref,p.input_revision,p.action,p.method_version,p.input_manifest,private.manifest AS protected_manifest,private.suggestion AS protected_suggestion FROM linggan_creation_ai_request request JOIN linggan_creation_ai_preview p USING(preview_ref) LEFT JOIN linggan_creation_ai_experience_material private USING(preview_ref) WHERE request_ref=$1 FOR UPDATE OF request").bind(id).fetch_optional(&mut *tx).await?.ok_or(AiError::NotFound)?;
    if row.get::<String, _>("state") != "succeeded" {
        return Err(AiError::Invalid);
    }
    let work: Uuid = row.get("work_ref");
    // Saving a revision takes the work lock before the experience gate. Keep adoption
    // in that same order before qualifying or projecting any protected material.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation:{work}"))
        .execute(&mut *tx)
        .await?;
    let input_revision: Uuid = row.get("input_revision");
    let manifest: Value = row.get("input_manifest");
    let protected = manifest.get("experienceProtected").and_then(Value::as_bool) == Some(true);
    let manifest = if protected {
        row.get::<Option<Value>, _>("protected_manifest")
            .ok_or(AiError::Source)?
    } else {
        manifest
    };
    if protected
        && (r.adopt_title
            || r.page_indices
                .as_ref()
                .is_some_and(|items| !items.is_empty()))
    {
        return Err(AiError::Invalid);
    }
    let input: PreviewRequest = serde_json::from_value(manifest).map_err(|_| AiError::Invalid)?;
    if !Box::pin(qualify_frozen_inputs(&mut tx, &input)).await? {
        return Err(AiError::Source);
    }
    let (domain, revision) = content_workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(|_| AiError::NotFound)?;
    if revision.id != r.expected_revision {
        return Err(AiError::Stale);
    }
    let focused = input.action == WritingAction::RewriteSelection
        && input.selection.is_some()
        && !r.adopt_title
        && r.page_indices.as_deref() == Some(&[])
        && r.paragraph_indices.as_deref() == Some(&[0]);
    if r.rewrite_text
        .as_ref()
        .is_some_and(|text| !focused || text.trim().is_empty() || text.chars().count() > 16000)
    {
        return Err(AiError::Invalid);
    }
    let original = if focused && revision.id != input_revision {
        // Keep historical reconstruction off the common legacy adoption future.
        let (_, original) = Box::pin(content_workbench::read_revision_in(
            &mut tx,
            work,
            Some(input_revision),
        ))
        .await
        .map_err(|_| AiError::NotFound)?;
        let target = input
            .selection
            .as_ref()
            .ok_or(AiError::Invalid)?
            .paragraph_id;
        if !same_rewrite_target(&original.document, &revision.document, target) {
            return Err(AiError::TargetChanged);
        }
        super::protected_experience(&revision.document, &input)?;
        Some(Box::new(original))
    } else {
        if revision.id != input_revision {
            return Err(AiError::Stale);
        }
        None
    };
    let suggestion: Value = if protected {
        row.get::<Option<Value>, _>("protected_suggestion")
            .ok_or(AiError::Source)?
    } else {
        row.get("suggestion")
    };
    let output: WritingSuggestion =
        serde_json::from_value(suggestion).map_err(|_| AiError::Invalid)?;
    let action: String = row.get("action");
    let frozen = original.as_deref().unwrap_or(&revision);
    let citations = outbound_known_citations(&frozen.document, &input)?;
    let mut document = revision.document;
    let mut adopted_targets = Vec::new();
    let indices = r
        .paragraph_indices
        .clone()
        .unwrap_or_else(|| (0..output.paragraphs.len()).collect());
    let page_indices = r
        .page_indices
        .clone()
        .unwrap_or_else(|| (0..output.pages.len()).collect());
    if indices.iter().any(|i| *i >= output.paragraphs.len())
        || page_indices.iter().any(|i| *i >= output.pages.len())
    {
        return Err(AiError::Invalid);
    }
    if r.adopt_title {
        if let Some(title) = output.title {
            document.title = title;
            adopted_targets.push(UseTarget::Title);
        }
    }
    if action == "draft" && r.paragraph_indices.is_none() {
        document.paragraphs.clear();
        document.paragraph_steps.clear();
    }
    for i in indices {
        let p = &output.paragraphs[i];
        if action == "rewrite_selection" {
            let target = document
                .paragraphs
                .iter_mut()
                .find(|existing| Some(existing.id) == p.paragraph_id)
                .ok_or(AiError::Invalid)?;
            if let Some(selection) = &input.selection {
                selected_text(target, selection)?;
                let prefix: String = target.text.chars().take(selection.start).collect();
                let suffix: String = target.text.chars().skip(selection.end).collect();
                let replacement = r.rewrite_text.as_deref().unwrap_or(&p.text);
                target.text = format!("{prefix}{replacement}{suffix}");
            } else {
                target.text = p.text.clone();
            }
            adopted_targets.push(UseTarget::Paragraph {
                paragraph_id: target.id,
            });
        } else {
            let id = Uuid::new_v4();
            document.paragraphs.push(Paragraph {
                id,
                text: p.text.clone(),
            });
            adopted_targets.push(UseTarget::Paragraph { paragraph_id: id });
        }
    }
    for i in page_indices {
        let p = &output.pages[i];
        let target = document
            .pages
            .iter_mut()
            .find(|page| Some(page.id) == p.page_id)
            .ok_or(AiError::Invalid)?;
        target.title = p.heading.clone();
        adopted_targets.push(UseTarget::PageTitle { page_id: target.id });
        let block = target.blocks.first_mut().ok_or(AiError::Invalid)?;
        block.text = p.body.clone();
        adopted_targets.push(UseTarget::PageBlock {
            page_id: target.id,
            block_id: block.id,
        });
    }
    content_workbench::propagate_adopted_sources_in(
        &mut tx,
        domain,
        &mut document,
        &citations,
        &adopted_targets,
    )
    .await
    .map_err(|_| AiError::Source)?;
    let link = AdoptionLink {
        suggestion_id: id,
        action,
        input_revision,
        model_invocation_ref: row.get("invocation_ref"),
        method_version: row.get("method_version"),
    };
    let receipt = content_workbench::adopt_in(
        &mut tx,
        work,
        r.request_id,
        r.expected_revision,
        document,
        &link,
    )
    .await
    .map_err(|e| match e {
        content_workbench::WorkbenchError::Database(e) => AiError::Database(e),
        _ => AiError::Conflict,
    })?;
    let mut stored = receipt.clone();
    content_workbench::strip_experience_text(&mut stored.revision.document);
    sqlx::query("INSERT INTO linggan_creation_ai_adoption_request(request_ref,suggestion_ref,command_hash,receipt) VALUES($1,$2,$3,$4)").bind(r.request_id).bind(id).bind(command_hash).bind(serde_json::to_value(&stored).map_err(|_|AiError::Invalid)?).execute(&mut *tx).await?;
    tx.commit().await?;
    serde_json::to_value(receipt).map_err(|_| AiError::Invalid)
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdoptTitleRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
    pub kind: TitleKind,
    pub index: usize,
}
#[derive(Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TitleKind {
    Cover,
    Note,
}

pub(super) async fn adopt_title(
    db: &Database,
    id: Uuid,
    r: &AdoptTitleRequest,
) -> Result<Value, AiError> {
    use crate::content_workbench::types::{
        Alignment, Crop, ImagePage, PageBlock, PageKind, PageStyle, Palette,
    };
    let mut tx = db.pool().begin().await?;
    let command_hash = sources::text_hash(&json!(["title", id, r]).to_string());
    content_workbench::ensure_request_owner_in(
        &mut tx,
        r.request_id,
        content_workbench::RequestOwner::Work,
    )
    .await
    .map_err(|e| match e {
        content_workbench::WorkbenchError::Database(e) => AiError::Database(e),
        _ => AiError::Conflict,
    })?;
    if let Some(row)=sqlx::query("SELECT command_hash,receipt FROM linggan_creation_ai_adoption_request WHERE request_ref=$1").bind(r.request_id).fetch_optional(&mut *tx).await? {
        if row.get::<String,_>("command_hash")!=command_hash {return Err(AiError::Conflict)}
        let mut receipt:SaveReceipt=serde_json::from_value(row.get("receipt")).map_err(|_|AiError::Invalid)?;
        let (domain,revision)=content_workbench::read_revision_in(&mut tx,receipt.work_id,Some(receipt.revision.id)).await.map_err(|_|AiError::NotFound)?;receipt.revision=revision;
        content_workbench::redact_revision_in(&mut tx,domain,&mut receipt.revision).await.map_err(|_|AiError::Source)?;
        receipt.replayed=true;tx.commit().await?;return serde_json::to_value(receipt).map_err(|_|AiError::Invalid)
    }
    let row=sqlx::query("SELECT request.state,request.suggestion,request.invocation_ref,p.work_ref,p.input_revision,p.action,p.method_version,p.input_manifest FROM linggan_creation_ai_request request JOIN linggan_creation_ai_preview p USING(preview_ref) WHERE request_ref=$1 FOR UPDATE OF request").bind(id).fetch_optional(&mut *tx).await?.ok_or(AiError::NotFound)?;
    if row.get::<String, _>("state") != "succeeded"
        || row.get::<String, _>("action") != "title_ideas"
    {
        return Err(AiError::Invalid);
    }
    let work: Uuid = row.get("work_ref");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation:{work}"))
        .execute(&mut *tx)
        .await?;
    let input_revision: Uuid = row.get("input_revision");
    let input: PreviewRequest =
        serde_json::from_value(row.get("input_manifest")).map_err(|_| AiError::Invalid)?;
    if !qualify_frozen_inputs(&mut tx, &input).await? {
        return Err(AiError::Source);
    }
    let (domain, revision) = content_workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(|_| AiError::NotFound)?;
    if revision.id != r.expected_revision {
        return Err(AiError::Stale);
    }
    let (_, original) = content_workbench::read_revision_in(&mut tx, work, Some(input_revision))
        .await
        .map_err(|_| AiError::NotFound)?;
    if revision.id != input_revision {
        let only_this_batch: bool = sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM linggan_creation_revision r WHERE r.work_ref=$1 AND r.revision_number>$2 AND r.revision_number<=$3 AND NOT EXISTS (SELECT 1 FROM linggan_creation_adoption a WHERE a.revision_ref=r.revision_ref AND a.suggestion_ref=$4 AND a.action='title_ideas'))")
            .bind(work).bind(original.number).bind(revision.number).bind(id).fetch_one(&mut *tx).await?;
        if !only_this_batch {
            return Err(AiError::Stale);
        }
    }
    let output: super::title::TitleSuggestion =
        serde_json::from_value(row.get("suggestion")).map_err(|_| AiError::Invalid)?;
    let text = match r.kind {
        TitleKind::Cover => output.cover_titles.get(r.index),
        TitleKind::Note => output.note_titles.get(r.index),
    }
    .ok_or(AiError::Invalid)?
    .clone();
    let citations = outbound_known_citations(&original.document, &input)?;
    let mut document = revision.document;
    let target = match r.kind {
        TitleKind::Note => {
            document.title = text;
            UseTarget::Title
        }
        TitleKind::Cover => {
            if !document
                .pages
                .iter()
                .any(|page| page.kind == PageKind::Cover)
            {
                if document.pages.len() >= 30 {
                    return Err(AiError::Invalid);
                }
                document.pages.insert(
                    0,
                    ImagePage {
                        id: Uuid::new_v4(),
                        kind: PageKind::Cover,
                        template_version: "creation-v3-type".into(),
                        title: String::new(),
                        subtitle: String::new(),
                        blocks: vec![PageBlock {
                            id: Uuid::new_v4(),
                            text: String::new(),
                        }],
                        asset_id: None,
                        crop: Crop {
                            x: 0.0,
                            y: 0.0,
                            width: 1.0,
                            height: 1.0,
                        },
                        style: PageStyle {
                            palette: Palette::Cream,
                            font_family: "system-cjk".into(),
                            width: 1080,
                            height: 1440,
                            alignment: Alignment::Start,
                        },
                    },
                );
            }
            let page = document
                .pages
                .iter_mut()
                .find(|page| page.kind == PageKind::Cover)
                .ok_or(AiError::Invalid)?;
            page.title = text;
            UseTarget::PageTitle { page_id: page.id }
        }
    };
    content_workbench::propagate_adopted_sources_in(
        &mut tx,
        domain,
        &mut document,
        &citations,
        &[target],
    )
    .await
    .map_err(|_| AiError::Source)?;
    let link = AdoptionLink {
        suggestion_id: id,
        action: "title_ideas".into(),
        input_revision: r.expected_revision,
        model_invocation_ref: row.get("invocation_ref"),
        method_version: row.get("method_version"),
    };
    let receipt = content_workbench::adopt_in(
        &mut tx,
        work,
        r.request_id,
        r.expected_revision,
        document,
        &link,
    )
    .await
    .map_err(|e| match e {
        content_workbench::WorkbenchError::Database(e) => AiError::Database(e),
        _ => AiError::Conflict,
    })?;
    let mut stored = receipt.clone();
    content_workbench::strip_experience_text(&mut stored.revision.document);
    sqlx::query("INSERT INTO linggan_creation_ai_adoption_request(request_ref,suggestion_ref,command_hash,receipt) VALUES($1,$2,$3,$4)").bind(r.request_id).bind(id).bind(command_hash).bind(serde_json::to_value(&stored).map_err(|_|AiError::Invalid)?).execute(&mut *tx).await?;
    tx.commit().await?;
    serde_json::to_value(receipt).map_err(|_| AiError::Invalid)
}
