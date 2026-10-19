//! Adopt a confirmed suggestion through the existing revision/source provenance transaction.
use super::{
    AdoptRequest, AiError, PreviewRequest, WritingSuggestion, outbound_known_citations,
    qualify_frozen_inputs, selected_text, sources,
};
use crate::content_workbench::{
    self,
    types::{AdoptionLink, Paragraph, SaveReceipt, UseTarget},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn adopt(db: &Database, id: Uuid, r: &AdoptRequest) -> Result<Value, AiError> {
    let mut tx = db.pool().begin().await?;
    let command_hash = sources::text_hash(&json!([id, r]).to_string());
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
        let (domain, _) = content_workbench::read_revision_in(
            &mut tx,
            receipt.work_id,
            Some(receipt.revision.id),
        )
        .await
        .map_err(|_| AiError::NotFound)?;
        content_workbench::redact_revision_in(&mut tx, domain, &mut receipt.revision)
            .await
            .map_err(|_| AiError::Source)?;
        receipt.replayed = true;
        tx.commit().await?;
        return serde_json::to_value(receipt).map_err(|_| AiError::Invalid);
    }
    let row=sqlx::query("SELECT request.state,request.suggestion,request.invocation_ref,p.work_ref,p.input_revision,p.action,p.method_version,p.input_manifest FROM linggan_creation_ai_request request JOIN linggan_creation_ai_preview p USING(preview_ref) WHERE request_ref=$1 FOR UPDATE OF request").bind(id).fetch_optional(&mut *tx).await?.ok_or(AiError::NotFound)?;
    if row.get::<String, _>("state") != "succeeded" {
        return Err(AiError::Invalid);
    }
    let work: Uuid = row.get("work_ref");
    let input_revision: Uuid = row.get("input_revision");
    if input_revision != r.expected_revision {
        return Err(AiError::Stale);
    }
    let input: PreviewRequest =
        serde_json::from_value(row.get("input_manifest")).map_err(|_| AiError::Invalid)?;
    if !qualify_frozen_inputs(&mut tx, &input).await? {
        return Err(AiError::Source);
    }
    let (domain, revision) = content_workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(|_| AiError::NotFound)?;
    if revision.id != input_revision {
        return Err(AiError::Stale);
    }
    let output: WritingSuggestion =
        serde_json::from_value(row.get("suggestion")).map_err(|_| AiError::Invalid)?;
    let action: String = row.get("action");
    let citations = outbound_known_citations(&revision.document, &input)?;
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
                target.text = format!("{prefix}{}{suffix}", p.text);
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
    sqlx::query("INSERT INTO linggan_creation_ai_adoption_request(request_ref,suggestion_ref,command_hash,receipt) VALUES($1,$2,$3,$4)").bind(r.request_id).bind(id).bind(command_hash).bind(serde_json::to_value(&receipt).map_err(|_|AiError::Invalid)?).execute(&mut *tx).await?;
    tx.commit().await?;
    serde_json::to_value(receipt).map_err(|_| AiError::Invalid)
}
