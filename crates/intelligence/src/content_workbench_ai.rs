//! Explicit single-call writing suggestions. Reading/saving never enters this executor.
use crate::{
    content_workbench::{self, types::*},
    content_workbench_sources as sources,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub const METHOD: &str = "content-workbench-writing.v2";
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("creation_ai_invalid")]
    Invalid,
    #[error("creation_ai_stale")]
    Stale,
    #[error("creation_ai_not_found")]
    NotFound,
    #[error("creation_ai_conflict")]
    Conflict,
    #[error("creation_ai_source_unavailable")]
    Source,
    #[error("creation_ai_model_unavailable")]
    Model,
    #[error("creation_ai_input_limit")]
    InputLimit,
    #[error("creation_ai_admission_denied")]
    AdmissionDenied(Vec<String>),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}
impl From<crate::ai_foundation::store::FoundationError> for AiError {
    fn from(error: crate::ai_foundation::store::FoundationError) -> Self {
        use crate::ai_foundation::store::FoundationError as F;
        match error {
            F::Database(e) => Self::Database(e),
            F::Conflict => Self::Stale,
            F::Invalid | F::Rule(_) => Self::Invalid,
            F::NotFound => Self::NotFound,
            F::AdmissionDenied(reasons) => Self::AdmissionDenied(reasons),
            _ => Self::Model,
        }
    }
}
impl AiError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "creation_ai_invalid",
            Self::Stale => "creation_ai_stale",
            Self::NotFound => "creation_ai_not_found",
            Self::Conflict => "creation_ai_conflict",
            Self::Source => "creation_ai_source_unavailable",
            Self::Model => "creation_ai_model_unavailable",
            Self::InputLimit => "creation_ai_input_limit",
            Self::AdmissionDenied(_) => "creation_ai_admission_denied",
            Self::Database(_) => "creation_ai_unavailable",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WritingAction {
    Draft,
    RewriteSelection,
    PageCopy,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RewriteSelection {
    pub paragraph_id: Uuid,
    pub start: usize,
    pub end: usize,
    pub text_hash: String,
}
fn selected_text<'a>(
    paragraph: &'a Paragraph,
    selection: &RewriteSelection,
) -> Result<String, AiError> {
    let selected: String = paragraph
        .text
        .chars()
        .skip(selection.start)
        .take(selection.end.saturating_sub(selection.start))
        .collect();
    if paragraph.id != selection.paragraph_id
        || selection.start >= selection.end
        || selection.end > paragraph.text.chars().count()
        || sources::text_hash(&selected) != selection.text_hash
    {
        return Err(AiError::Invalid);
    }
    Ok(selected)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRequest {
    #[serde(default)]
    pub selection: Option<RewriteSelection>,
    pub preview_ref: Uuid,
    pub work_ref: Uuid,
    pub expected_revision: Uuid,
    pub action: WritingAction,
    pub config_ref: Uuid,
    pub citations: Vec<Citation>,
    pub paragraph_ids: Vec<Uuid>,
    pub instruction: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteRequest {
    pub request_id: Uuid,
    pub preview_ref: Uuid,
    pub expected_revision: Uuid,
    pub confirmed_input_hash: String,
    pub budget_tokens: i64,
    pub confirmed: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestedParagraph {
    pub paragraph_id: Option<Uuid>,
    pub text: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SuggestedPage {
    pub page_id: Option<Uuid>,
    pub heading: String,
    pub body: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WritingSuggestion {
    pub title: Option<String>,
    pub paragraphs: Vec<SuggestedParagraph>,
    pub pages: Vec<SuggestedPage>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdoptRequest {
    pub request_id: Uuid,
    pub expected_revision: Uuid,
    pub paragraph_indices: Option<Vec<usize>>,
    pub page_indices: Option<Vec<usize>>,
    pub adopt_title: bool,
}
mod adoption;
pub(crate) mod execution;
mod queue;
mod read;

pub async fn readiness(db: &Database) -> Result<Value, AiError> {
    queue::readiness(db).await
}

fn instruction_valid(r: &PreviewRequest) -> bool {
    r.instruction.chars().count() <= 2000
        && r.citations.len() <= 50
        && r.paragraph_ids.len() <= 200
        && r.selection.as_ref().is_none_or(|selection| {
            r.action == WritingAction::RewriteSelection
                && r.paragraph_ids == vec![selection.paragraph_id]
        })
        && (r.action != WritingAction::RewriteSelection || !r.paragraph_ids.is_empty())
}
/// Inspect only text actually sent by this action, against already known exact Citation text.
fn outbound_known_citations(
    doc: &WorkDocument,
    request: &PreviewRequest,
) -> Result<Vec<Citation>, AiError> {
    let mut texts = vec![
        doc.title.clone(),
        doc.angle.clone(),
        request.instruction.clone(),
    ];
    texts.extend(doc.outline.iter().cloned());
    for paragraph in doc.paragraphs.iter().filter(|p| {
        request.action != WritingAction::RewriteSelection || request.paragraph_ids.contains(&p.id)
    }) {
        texts.push(if let Some(selection) = &request.selection {
            selected_text(paragraph, selection)?
        } else {
            paragraph.text.clone()
        });
    }
    let mut citations = request.citations.clone();
    for excerpt in &doc.excerpts {
        if excerpt
            .selected_text
            .as_ref()
            .is_some_and(|raw| !raw.is_empty() && texts.iter().any(|text| text.contains(raw)))
            && !citations.contains(&excerpt.citation)
        {
            citations.push(excerpt.citation.clone());
        }
    }
    Ok(citations)
}
async fn inputs_in(
    tx: &mut Transaction<'_, Postgres>,
    r: &PreviewRequest,
) -> Result<(Revision, Value, String), AiError> {
    let (domain, revision) = content_workbench::read_revision_in(tx, r.work_ref, None)
        .await
        .map_err(|_| AiError::NotFound)?;
    // Until experience-derived AI inputs and suggestions have a withdrawal path, do not freeze
    // or send them through the existing immutable writing-command store.
    if !revision.document.experience_slots.is_empty()
        || !revision.document.experience_gaps.is_empty()
    {
        return Err(AiError::Source);
    }
    if revision.id != r.expected_revision {
        return Err(AiError::Stale);
    }
    if !instruction_valid(r)
        || r.paragraph_ids
            .iter()
            .any(|id| !revision.document.paragraphs.iter().any(|p| p.id == *id))
    {
        return Err(AiError::Invalid);
    }
    for citation in outbound_known_citations(&revision.document, r)? {
        sources::validate_citation_in_domain(tx, &citation, domain)
            .await
            .map_err(|_| AiError::Source)?;
    }
    let mut materials = Vec::new();
    let mut material_indexes = std::collections::BTreeMap::new();
    for c in &r.citations {
        let v = sources::validate_citation_in_domain(tx, c, domain)
            .await
            .map_err(|_| AiError::Source)?;
        append_canonical_material(&mut materials, &mut material_indexes, c, v)?;
    }
    if let Some(selection) = &r.selection {
        let paragraph = revision
            .document
            .paragraphs
            .iter()
            .find(|p| p.id == selection.paragraph_id)
            .ok_or(AiError::Invalid)?;
        selected_text(paragraph, selection)?;
    }
    let paragraphs: Vec<_> = revision
        .document
        .paragraphs
        .iter()
        .filter(|p| r.action != WritingAction::RewriteSelection || r.paragraph_ids.contains(&p.id))
        .map(|p| {
            let text = if let Some(selection) = &r.selection {
                selected_text(p, selection)?
            } else {
                p.text.clone()
            };
            Ok(json!({"id":p.id,"text":text}))
        })
        .collect::<Result<Vec<_>, AiError>>()?;
    let payload = json!({"action":r.action,"title":revision.document.title,"angle":revision.document.angle,"outline":revision.document.outline,"paragraphs":paragraphs,"selection":r.selection,"pages":revision.document.pages.iter().map(|p|json!({"pageId":p.id,"kind":p.kind})).collect::<Vec<_>>(),"materials":materials,"authorInstruction":r.instruction});
    let hash = sources::text_hash(
        &json!([METHOD, r.expected_revision, r.config_ref, &payload]).to_string(),
    );
    Ok((revision, payload, hash))
}

fn append_canonical_material(
    materials: &mut Vec<Value>,
    indexes: &mut std::collections::BTreeMap<String, usize>,
    citation: &Citation,
    source: sources::ValidatedCitation,
) -> Result<(), AiError> {
    let identity = json!([
        citation.source_kind,
        citation.source_ref,
        citation.work_ref,
        citation.field,
        citation.source_version,
        citation.block_ref,
        citation.as_of
    ])
    .to_string();
    if let Some(index) = indexes.get(&identity) {
        let material = &mut materials[*index];
        if material["content"] != source.content
            || material["speaker"] != source.speaker
            || material["issues"] != json!(source.issues)
        {
            return Err(AiError::Source);
        }
        let citations = material["citations"]
            .as_array_mut()
            .expect("canonical citations");
        let value = json!(citation);
        if !citations.contains(&value) {
            citations.push(value);
        }
    } else {
        indexes.insert(identity, materials.len());
        materials.push(json!({"citations":[citation],"content":source.content,
            "speaker":source.speaker,"issues":source.issues}));
    }
    Ok(())
}
pub async fn preview(db: &Database, r: &PreviewRequest) -> Result<Value, AiError> {
    queue::preview(db, r).await
}
pub async fn execute(db: &Database, r: &ExecuteRequest) -> Result<Value, AiError> {
    queue::enqueue(db, r).await
}
pub async fn status(db: &Database, id: Uuid) -> Result<Value, AiError> {
    read::status(db, id).await
}
async fn qualify_frozen_inputs(
    tx: &mut Transaction<'_, Postgres>,
    input: &PreviewRequest,
) -> Result<bool, AiError> {
    let (domain, revision) =
        content_workbench::read_revision_in(tx, input.work_ref, Some(input.expected_revision))
            .await
            .map_err(|_| AiError::NotFound)?;
    if !revision.document.experience_slots.is_empty()
        || !revision.document.experience_gaps.is_empty()
    {
        return Ok(false);
    }
    let citations = outbound_known_citations(&revision.document, input)?;
    Ok(sources::qualify_citations_in_domain(tx, &citations, domain)
        .await
        .map_err(|_| AiError::Source)?
        .iter()
        .all(|q| q.state == "readable"))
}
pub async fn cancel(db: &Database, id: Uuid) -> Result<Value, AiError> {
    queue::cancel(db, id).await
}

fn validate_suggestion(
    action: &WritingAction,
    value: &str,
    paragraph_ids: &[Uuid],
    page_ids: &[Uuid],
) -> Result<WritingSuggestion, AiError> {
    let output: WritingSuggestion = serde_json::from_str(value).map_err(|_| AiError::Invalid)?;
    if output
        .title
        .as_ref()
        .is_some_and(|s| s.chars().count() > 300)
        || output.paragraphs.len() > 200
        || output.pages.len() > 30
        || output
            .paragraphs
            .iter()
            .any(|p| p.text.is_empty() || p.text.chars().count() > 16000)
        || output
            .pages
            .iter()
            .any(|p| p.heading.chars().count() > 300 || p.body.chars().count() > 16000)
    {
        return Err(AiError::Invalid);
    }
    let mut seen = std::collections::HashSet::new();
    if output
        .paragraphs
        .iter()
        .filter_map(|p| p.paragraph_id)
        .any(|id| !seen.insert(id))
    {
        return Err(AiError::Invalid);
    }
    let mut seen = std::collections::HashSet::new();
    if output
        .pages
        .iter()
        .filter_map(|p| p.page_id)
        .any(|id| !seen.insert(id))
    {
        return Err(AiError::Invalid);
    }
    match action {
        WritingAction::Draft if output.paragraphs.is_empty() || !output.pages.is_empty() => {
            return Err(AiError::Invalid);
        }
        WritingAction::RewriteSelection
            if output.paragraphs.is_empty()
                || output
                    .paragraphs
                    .iter()
                    .any(|p| !p.paragraph_id.is_some_and(|id| paragraph_ids.contains(&id)))
                || !output.pages.is_empty() =>
        {
            return Err(AiError::Invalid);
        }
        WritingAction::PageCopy
            if output.pages.is_empty()
                || output
                    .pages
                    .iter()
                    .any(|p| !p.page_id.is_some_and(|id| page_ids.contains(&id)))
                || !output.paragraphs.is_empty() =>
        {
            return Err(AiError::Invalid);
        }
        _ => {}
    }
    let ids: Vec<_> = output
        .paragraphs
        .iter()
        .filter_map(|p| p.paragraph_id)
        .collect();
    if ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len() {
        return Err(AiError::Invalid);
    }
    Ok(output)
}

pub async fn adopt(db: &Database, id: Uuid, r: &AdoptRequest) -> Result<Value, AiError> {
    adoption::adopt(db, id, r).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn several_quotes_share_one_full_block_without_dropping_tail_roles_or_issues() {
        let text = format!("{}最后才出现的否定🙂", "完整正文。".repeat(4000));
        let content = json!({"kind":"quote","text":text,"speaker":"other"});
        let issue = json!({"key":"unclear","code":"unclear","message":"归属待确认","resolution":"reading_edit"});
        let source = || sources::ValidatedCitation {
            selected_text: "完整正文。".into(),
            text_hash: "selected".into(),
            speaker: "other".into(),
            content: content.clone(),
            issues: vec![issue.clone()],
        };
        let mut first = Citation {
            block_ref: "work.reading.0".into(),
            as_of: "2026-10-23T00:00:00Z".into(),
            source_kind: SourceKind::Ocr,
            source_ref: Uuid::from_u128(1),
            work_ref: Some(Uuid::from_u128(2)),
            field: SourceField::Ocr,
            source_version: Uuid::from_u128(3).to_string(),
            start: 0,
            end: 5,
            text_hash: "quote-1".into(),
            location: None,
        };
        let mut materials = vec![];
        let mut indexes = std::collections::BTreeMap::new();
        append_canonical_material(&mut materials, &mut indexes, &first, source()).unwrap();
        first.start = 10;
        first.end = 15;
        first.text_hash = "quote-2".into();
        append_canonical_material(&mut materials, &mut indexes, &first, source()).unwrap();
        append_canonical_material(&mut materials, &mut indexes, &first, source()).unwrap();
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0]["citations"].as_array().unwrap().len(), 2);
        assert_eq!(materials[0]["content"], content);
        assert_eq!(materials[0]["speaker"], "other");
        assert_eq!(materials[0]["issues"][0], issue);
        assert_eq!(
            serde_json::to_string(&materials)
                .unwrap()
                .matches("最后才出现的否定🙂")
                .count(),
            1
        );
    }
    #[test]
    fn writing_contract_never_accepts_research_json() {
        assert!(
            validate_suggestion(
                &WritingAction::Draft,
                r#"{"outcome":"no_signal"}"#,
                &[],
                &[]
            )
            .is_err()
        );
    }
    #[test]
    fn rewrites_preserve_frozen_paragraph_identity() {
        let id = Uuid::new_v4();
        let text =
            json!({"title":null,"paragraphs":[{"paragraphId":id,"text":"改写😀"}],"pages":[]})
                .to_string();
        assert!(validate_suggestion(&WritingAction::RewriteSelection, &text, &[id], &[]).is_ok());
        assert!(
            validate_suggestion(
                &WritingAction::RewriteSelection,
                &text,
                &[Uuid::new_v4()],
                &[]
            )
            .is_err()
        );
    }
}
