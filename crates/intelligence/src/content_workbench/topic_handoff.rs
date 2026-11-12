//! A saved research angle becomes one existing creation idea box, never a publication.
use super::*;
use crate::ai_foundation::store::{Command, FoundationError};
use crate::content_workbench_sources::materials;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandoffRequest {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub alternative_ref: Uuid,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffReceipt {
    pub alternative_ref: Uuid,
    pub work_id: Uuid,
    pub idea_ref: Uuid,
}
fn command_error(error: FoundationError) -> WorkbenchError {
    match error {
        FoundationError::Database(error) => WorkbenchError::Database(error),
        FoundationError::Conflict => WorkbenchError::RequestConflict,
        _ => WorkbenchError::Invalid("handoff_command"),
    }
}
fn linked(row: &sqlx::postgres::PgRow, alternative: Uuid) -> HandoffReceipt {
    HandoffReceipt {
        alternative_ref: alternative,
        work_id: row.get("anchor_work_ref"),
        idea_ref: row.get("idea_ref"),
    }
}
pub async fn handoff_angle(
    db: &Database,
    request: &HandoffRequest,
) -> Result<HandoffReceipt, WorkbenchError> {
    if request.command_ref.is_nil()
        || request.domain_ref.is_nil()
        || request.alternative_ref.is_nil()
    {
        return Err(WorkbenchError::Invalid("handoff"));
    }
    let mut command = Command::begin(db, request.command_ref, "topic_angle_handoff", request)
        .await
        .map_err(command_error)?;
    if let Some(receipt) = command.replay().map_err(command_error)? {
        return Ok(receipt);
    }
    // The source row is immutable; its row lock serializes different command IDs for this angle.
    sqlx::query("SELECT alternative_ref FROM linggan_topic_map_alternative WHERE alternative_ref=$1 AND domain_ref=$2 FOR UPDATE")
        .bind(request.alternative_ref).bind(request.domain_ref).fetch_optional(&mut *command.tx).await?.ok_or(WorkbenchError::NotFound)?;
    let existing=sqlx::query("SELECT anchor_work_ref,idea_ref FROM linggan_topic_angle_handoff WHERE alternative_ref=$1 AND domain_ref=$2")
        .bind(request.alternative_ref).bind(request.domain_ref).fetch_optional(&mut *command.tx).await?;
    if let Some(row) = existing {
        return command
            .finish(linked(&row, request.alternative_ref))
            .await
            .map_err(command_error);
    }
    let saved = crate::topic_map::read_saved_alternative_in(
        &mut command.tx,
        request.domain_ref,
        request.alternative_ref,
    )
    .await
    .map_err(|error| match error {
        crate::topic_map::TopicMapError::Database(error) => WorkbenchError::Database(error),
        crate::topic_map::TopicMapError::NotFound => WorkbenchError::NotFound,
        _ => WorkbenchError::Source,
    })?;
    if saved.kind != "angle" {
        return Err(WorkbenchError::Invalid("angle_required"));
    }
    if saved.source_state == "source_unavailable" {
        return Err(WorkbenchError::Source);
    }
    let mut selections = Vec::new();
    let works: BTreeSet<Uuid> = saved
        .source_manifests
        .iter()
        .filter_map(|manifest| manifest["workRef"].as_str()?.parse().ok())
        .collect();
    for work in works {
        let mut selection = MaterialSelection {
            document_id: materials::document_id(&SourceKind::Note, work),
            source_kind: SourceKind::Note,
            source_ref: work,
            work_ref: Some(work),
            source_version: "0".repeat(64),
            added_by: "manual".into(),
            match_reasons: vec![],
        };
        let current =
            materials::current_selection_in(&mut command.tx, request.domain_ref, &selection)
                .await
                .map_err(|error| match error {
                    crate::content_workbench_sources::SourceError::Database(error) => {
                        WorkbenchError::Database(error)
                    }
                    _ => WorkbenchError::Source,
                })?;
        selection.source_version = current.source_version;
        selections.push(selection);
    }
    let idea = Uuid::new_v4();
    let title = saved.title.ok_or(WorkbenchError::Source)?;
    let document = WorkDocument {
        topic_angle_refs: vec![request.alternative_ref],
        title: title.clone(),
        idea_ref: Some(idea),
        idea_name: Some(title),
        angle: saved.angle.ok_or(WorkbenchError::Source)?,
        topic_ref: saved.topic_ref,
        materials: selections,
        ..Default::default()
    };
    let created = super::store::create_work_in(
        &mut command.tx,
        &CreateRequest {
            request_id: request.command_ref,
            domain_ref: request.domain_ref,
            document,
        },
    )
    .await?;
    sqlx::query("INSERT INTO linggan_topic_angle_handoff(alternative_ref,domain_ref,idea_ref,anchor_work_ref) VALUES($1,$2,$3,$4)")
        .bind(request.alternative_ref).bind(request.domain_ref).bind(idea).bind(created.work_id).execute(&mut *command.tx).await?;
    command
        .finish(HandoffReceipt {
            alternative_ref: request.alternative_ref,
            work_id: created.work_id,
            idea_ref: idea,
        })
        .await
        .map_err(command_error)
}

/// The saved angle retains its own canonical qualification, including Core images and
/// definition dependencies. A link is not a fabricated text Citation.
pub(super) async fn unavailable_texts_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    doc: &WorkDocument,
) -> Result<Vec<String>, WorkbenchError> {
    let mut texts = Vec::new();
    for alternative in &doc.topic_angle_refs {
        let saved = crate::topic_map::read_saved_alternative_in(tx, domain, *alternative)
            .await
            .map_err(|error| match error {
                crate::topic_map::TopicMapError::Database(error) => WorkbenchError::Database(error),
                _ => WorkbenchError::Source,
            })?;
        if saved.kind != "angle" {
            return Err(WorkbenchError::Source);
        }
        if saved.source_state == "source_unavailable" {
            let row = sqlx::query("SELECT title,angle FROM linggan_topic_map_alternative WHERE alternative_ref=$1 AND domain_ref=$2")
                .bind(alternative).bind(domain).fetch_one(&mut **tx).await?;
            texts.push(row.get::<String, _>("title"));
            texts.push(row.get::<String, _>("angle"));
        }
    }
    texts.sort_by_key(|text| std::cmp::Reverse(text.len()));
    texts.dedup();
    Ok(texts)
}

pub(super) fn redact_origin_texts(doc: &mut WorkDocument, texts: &[String]) {
    // Keep unrelated user-authored edits. Exact original text remains dependent even
    // when copied into another target, matching ordinary excerpt redaction.
    for target in super::source_usage::authored_targets(doc) {
        if let Some(value) = super::source_usage::target_mut(doc, &target) {
            for raw in texts.iter().filter(|raw| !raw.is_empty()) {
                *value = value.replace(raw, "[来源当前不可使用]");
            }
        }
    }
    super::source_usage::redact_source_metadata(doc, texts);
}

pub(crate) async fn qualify_document_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    doc: &WorkDocument,
) -> Result<(), WorkbenchError> {
    qualify_outbound_in(tx, domain, doc, "").await
}

pub(crate) async fn qualify_outbound_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    doc: &WorkDocument,
    instruction: &str,
) -> Result<(), WorkbenchError> {
    let unavailable = unavailable_texts_in(tx, domain, doc).await?;
    if unavailable
        .iter()
        .any(|raw| super::source_usage::raw_present(instruction, raw))
    {
        return Err(WorkbenchError::Source);
    }
    if !unavailable.is_empty() {
        let mut redacted = doc.clone();
        redact_origin_texts(&mut redacted, &unavailable);
        if redacted != *doc {
            return Err(WorkbenchError::Source);
        }
    }
    Ok(())
}
