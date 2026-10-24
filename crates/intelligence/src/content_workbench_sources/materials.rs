//! Bounded local reading collections. Store identities, re-read permission and exact fields.
use super::*;
use std::collections::HashSet;

pub const METHOD: &str = "workbench.lexical-context.v1";
const LIMIT: usize = 24;
const CANDIDATE_LIMIT: usize = 120;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareRequest {
    pub domain_ref: Uuid,
    pub title: String,
    pub angle: String,
    #[serde(default)]
    pub outline: Vec<String>,
    pub topic_ref: Option<Uuid>,
    pub terms: Option<Vec<String>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchQuery {
    pub domain_ref: Uuid,
    pub kind: Option<SourceKind>,
    pub query: String,
    pub cursor: Option<usize>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveRequest {
    pub domain_ref: Uuid,
    pub terms: Option<Vec<String>>,
    pub selections: Vec<MaterialSelection>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDocument {
    pub document_id: String,
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub source_version: String,
    pub title: String,
    pub author_label: String,
    pub qualification: String,
    pub fragments: Vec<SourceItem>,
    pub context: Value,
    pub match_reasons: Vec<String>,
}
fn kind_name(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::Note => "note",
        SourceKind::Comment => "comment",
        SourceKind::OpsPublication => "ops_publication",
        SourceKind::Ocr => "ocr",
        SourceKind::Transcript => "transcript",
        SourceKind::FrameText => "frame_text",
    }
}
pub fn document_id(kind: &SourceKind, source: Uuid) -> String {
    format!("{}:{source}", kind_name(kind))
}

#[path = "material_collection.rs"]
mod collection;
#[path = "material_documents.rs"]
mod documents;
#[path = "material_matching.rs"]
mod matching;
use collection::{begin, response};
pub use collection::{prepare, search};
use documents::{read_comment_documents_in, read_document_in, read_with_comments_in};
use matching::*;

pub async fn resolve(db: &Database, request: &ResolveRequest) -> Result<Value, SourceError> {
    if request.selections.len() > 60 {
        return Err(SourceError::Invalid);
    }
    let terms = permitted_terms(request.terms.as_deref().unwrap_or_default())?;
    let mut tx = begin(db).await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let refs: Vec<_> = request
        .selections
        .iter()
        .filter(|s| s.source_kind == SourceKind::Comment)
        .map(|s| s.source_ref)
        .collect();
    let comments = read_comment_documents_in(&mut tx, request.domain_ref, &refs).await?;
    let mut documents = Vec::new();
    let mut seen = HashSet::new();
    for selection in &request.selections {
        validate_selection_shape(selection)?;
        if !seen.insert(selection.document_id.clone()) {
            return Err(SourceError::Invalid);
        }
        let mut d =
            match read_with_comments_in(&mut tx, request.domain_ref, selection, &comments).await {
                Ok(mut d) => {
                    if d.source_version != selection.source_version {
                        d.qualification = "changed".into();
                    }
                    d
                }
                Err(SourceError::Database(e)) => return Err(SourceError::Database(e)),
                Err(e) => MaterialDocument {
                    document_id: selection.document_id.clone(),
                    source_kind: selection.source_kind.clone(),
                    source_ref: selection.source_ref,
                    work_ref: selection.work_ref,
                    source_version: selection.source_version.clone(),
                    title: String::new(),
                    author_label: String::new(),
                    qualification: if matches!(e, SourceError::Restricted) {
                        "restricted"
                    } else {
                        "unavailable"
                    }
                    .into(),
                    fragments: vec![],
                    context: Value::Null,
                    match_reasons: vec![],
                },
            };
        d.match_reasons = current_reasons(&d, selection, &terms);
        documents.push(d);
    }
    tx.commit().await?;
    let gaps = if documents.iter().any(|d| d.qualification != "readable") {
        vec!["部分材料已变化或不可用，请重新核对".into()]
    } else {
        vec![]
    };
    Ok(response(&as_of, "", &terms, documents, false, 0, gaps))
}
fn current_reasons(
    document: &MaterialDocument,
    selection: &MaterialSelection,
    terms: &[String],
) -> Vec<String> {
    let mut result = vec![
        if selection.added_by == "manual" {
            "作者手动加入"
        } else {
            "此前为本篇准备，当前资格已重新核对"
        }
        .into(),
    ];
    if document.fragments.is_empty() || terms.is_empty() {
        return result;
    }
    let text = document
        .fragments
        .iter()
        .map(|f| f.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let hits = matches(&text, terms);
    if hits.is_empty() {
        result.push("当前原文未命中这些检索词，请核对相关性".into());
    } else {
        result.extend(reasons(&hits, false));
    }
    if document.source_kind == SourceKind::Comment
        && document.context["parentWork"]["state"] == "known"
    {
        let dossier = &document.context["parentWork"]["dossier"];
        let parent = dossier["note"]["platformTitle"]
            .as_str()
            .map(str::to_owned)
            .into_iter()
            .chain(
                dossier["materials"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|unit| unit["blocks"].as_array().into_iter().flatten())
                    .map(linggan_evidence::research_dossier::content::block_text),
            )
            .collect::<Vec<_>>()
            .join("\n");
        let hits = matches(&parent, terms);
        if !hits.is_empty() {
            result.push(format!("父笔记当前原文命中：{}", hits.join("、")));
        }
    }
    result
}
pub fn validate_selection_shape(s: &MaterialSelection) -> Result<(), SourceError> {
    if s.source_ref.is_nil()
        || s.document_id != document_id(&s.source_kind, s.source_ref)
        || s.source_kind == SourceKind::Ocr
        || (s.source_kind == SourceKind::Note && s.work_ref != Some(s.source_ref))
        || (s.source_kind == SourceKind::OpsPublication && s.work_ref.is_some())
        || !matches!(s.added_by.as_str(), "prepared" | "manual")
        || s.source_version.len() != 64
        || !s.source_version.bytes().all(|b| b.is_ascii_hexdigit())
        || s.match_reasons.len() > 12
        || s.match_reasons.iter().any(|s| s.chars().count() > 200)
    {
        return Err(SourceError::Invalid);
    }
    Ok(())
}
pub async fn validate_selection_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    s: &MaterialSelection,
) -> Result<(), SourceError> {
    validate_selection_shape(s)?;
    let current = read_document_in(tx, domain, &s.source_kind, s.source_ref, s.work_ref).await?;
    if current.source_version != s.source_version {
        return Err(SourceError::Changed);
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentQuery {
    pub domain_ref: Uuid,
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub as_of: Option<String>,
    pub block_ref: Option<String>,
    pub source_version: Option<String>,
    pub citation_source_ref: Option<Uuid>,
    pub field: Option<SourceField>,
}
pub async fn read_document(db: &Database, q: &DocumentQuery) -> Result<Value, SourceError> {
    let mut tx = begin(db).await?;
    let document = match (
        &q.as_of,
        &q.block_ref,
        &q.source_version,
        q.citation_source_ref,
        &q.field,
    ) {
        (None, None, None, None, None) => {
            read_document_in(
                &mut tx,
                q.domain_ref,
                &q.source_kind,
                q.source_ref,
                q.work_ref,
            )
            .await?
        }
        (Some(as_of), Some(block_ref), Some(version), Some(source), Some(field)) => {
            let kind = if q.source_kind == SourceKind::Note {
                if q.work_ref != Some(q.source_ref) {
                    return Err(SourceError::Invalid);
                }
                let field_name = serde_json::to_value(field).map_err(|_| SourceError::Invalid)?;
                fragment_kind(field_name.as_str().ok_or(SourceError::Invalid)?)
                    .ok_or(SourceError::Invalid)?
                    .0
            } else {
                if source != q.source_ref {
                    return Err(SourceError::Invalid);
                }
                q.source_kind.clone()
            };
            ensure_source_domain_in(&mut tx, &kind, source, q.work_ref, q.domain_ref).await?;
            let (item, context) =
                read_source_in(&mut tx, &kind, source, q.work_ref, field, block_ref, as_of).await?;
            if &item.source_version != version {
                return Err(SourceError::Changed);
            }
            if q.source_kind == SourceKind::Note {
                let dossier = research_dossier::read_in(&mut tx, &[q.source_ref], as_of)
                    .await?
                    .remove(&q.source_ref)
                    .ok_or(SourceError::Unavailable)?;
                documents::note_document_in(&mut tx, dossier).await?
            } else {
                MaterialDocument {
                    document_id: document_id(&q.source_kind, q.source_ref),
                    source_kind: q.source_kind.clone(),
                    source_ref: q.source_ref,
                    work_ref: item.work_ref,
                    source_version: item.source_version.clone(),
                    title: item.title.clone(),
                    author_label: item.author_label.clone(),
                    qualification: "readable".into(),
                    fragments: vec![item],
                    context,
                    match_reasons: vec![],
                }
            }
        }
        _ => return Err(SourceError::Invalid),
    };
    tx.commit().await?;
    Ok(json!(document))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refreshed_reasons_use_current_words_and_never_replay_restricted_or_saved_prose() {
        let source = Uuid::new_v4();
        let selection = MaterialSelection {
            document_id: document_id(&SourceKind::Comment, source),
            source_kind: SourceKind::Comment,
            source_ref: source,
            work_ref: Some(Uuid::new_v4()),
            source_version: "a".repeat(64),
            added_by: "prepared".into(),
            match_reasons: vec!["伪造旧来源原文".into()],
        };
        let mut document = MaterialDocument {
            document_id: selection.document_id.clone(),
            source_kind: SourceKind::Comment,
            source_ref: source,
            work_ref: selection.work_ref,
            source_version: selection.source_version.clone(),
            title: String::new(),
            author_label: String::new(),
            qualification: "readable".into(),
            fragments: vec![item(
                SourceKind::Comment,
                source,
                selection.work_ref,
                SourceField::CommentBody,
                "a".repeat(64),
                String::new(),
                "写作业时很磨蹭".into(),
            )],
            context: json!({"parentWork":{"state":"known","dossier":{"note":{"platformTitle":"作业"},"materials":[{"blocks":[{"kind":"paragraph","text":"作业启动的第一步"}]}]}}}),
            match_reasons: vec![],
        };
        let terms = vec!["作业".into(), "启动".into(), "磨蹭".into()];
        let reasons = current_reasons(&document, &selection, &terms).join("\n");
        assert!(reasons.contains("当前原文命中：作业、磨蹭"));
        assert!(reasons.contains("父笔记当前原文命中：作业、启动"));
        assert!(!reasons.contains("伪造旧来源原文"));
        document.qualification = "restricted".into();
        document.fragments.clear();
        document.context = Value::Null;
        let reasons = current_reasons(&document, &selection, &terms).join("\n");
        assert!(!reasons.contains("当前原文命中"));
        assert!(!reasons.contains("父笔记"));
    }
}
