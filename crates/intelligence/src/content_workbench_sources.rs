//! One qualified source/version supplies reading, highlighting and exact citations.
use crate::content_workbench::types::*;
use crate::ops_ledger::read::read_creation_publications;
use linggan_evidence::research_dossier;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[path = "content_workbench_sources/materials.rs"]
pub mod materials;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source_invalid")]
    Invalid,
    #[error("source_changed")]
    Changed,
    #[error("source_restricted")]
    Restricted,
    #[error("source_unavailable")]
    Unavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}
impl SourceError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "source_invalid",
            Self::Changed => "source_changed",
            Self::Restricted => "source_restricted",
            Self::Unavailable => "source_unavailable",
            Self::Database(_) => "source_read_unavailable",
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceItem {
    pub as_of: String,
    pub block_ref: String,
    pub speaker: String,
    pub structure: Value,
    pub issues: Vec<Value>,
    pub fragment_id: String,
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub field: SourceField,
    pub source_version: String,
    pub title: String,
    pub text: String,
    pub body_state: String,
    pub author_label: String,
    pub qualification: String,
    pub location: Option<OcrLocation>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceQuery {
    pub domain_ref: Option<Uuid>,
    pub account_ref: Option<Uuid>,
    pub kind: Option<SourceKind>,
    pub query: Option<String>,
    pub cursor: Option<usize>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnchorRequest {
    pub as_of: String,
    pub block_ref: String,
    pub domain_ref: Uuid,
    pub source_kind: SourceKind,
    pub source_ref: Uuid,
    pub work_ref: Option<Uuid>,
    pub field: SourceField,
    pub source_version: String,
    pub start: usize,
    pub end: usize,
}
pub struct ValidatedCitation {
    pub selected_text: String,
    pub text_hash: String,
    pub speaker: String,
    pub content: Value,
    pub issues: Vec<Value>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationQualification {
    pub source_ref: Uuid,
    pub state: String,
    pub code: String,
}
pub fn text_hash(text: &str) -> String {
    crate::research_text::content_hash(text)
}
fn item(
    kind: SourceKind,
    source: Uuid,
    work: Option<Uuid>,
    field: SourceField,
    version: String,
    title: String,
    text: String,
) -> SourceItem {
    SourceItem {
        as_of: String::new(),
        fragment_id: source_block_ref(source, &field),
        block_ref: source_block_ref(source, &field),
        speaker: "unconfirmed".into(), structure: Value::Null, issues: Vec::new(),
        source_kind: kind,
        source_ref: source,
        work_ref: work,
        field,
        source_version: version,
        title,
        text,
        body_state: "KNOWN".into(),
        author_label: "来源材料".into(),
        qualification: "readable".into(),
        location: None,
    }
}
fn comment_item(
    source: Uuid,
    work: Option<Uuid>,
    field: &SourceField,
    c: &Value,
) -> Result<SourceItem, SourceError> {
    if c["qualification"] == "restricted" {
        return Err(SourceError::Restricted);
    }
    let work_id: Uuid =
        serde_json::from_value(c["workRef"].clone()).map_err(|_| SourceError::Invalid)?;
    if work.is_some_and(|w| w != work_id) {
        return Err(SourceError::Invalid);
    }
    let text = c["body"]
        .as_str()
        .filter(|_| c["bodyState"] == "KNOWN")
        .ok_or(SourceError::Unavailable)?
        .to_owned();
    let version =
        text_hash(&json!([source, c["packageRef"], c["observedAt"], text_hash(&text)]).to_string());
    let mut i = item(
        SourceKind::Comment,
        source,
        Some(work_id),
        field.clone(),
        version,
        "匿名原声".into(),
        text,
    );
    i.author_label = "匿名评论来源".into();
    i.speaker = "other".into();
    i.title = c["parentWork"]["dossier"]["note"]["displayTitle"].as_str()
        .unwrap_or("匿名原声 · 父笔记未知").to_owned();
    Ok(i)
}

pub async fn read_source_in(
    tx: &mut Transaction<'_, Postgres>,
    kind: &SourceKind,
    source: Uuid,
    work: Option<Uuid>,
    field: &SourceField,
    block_ref: &str,
    as_of: &str,
) -> Result<(SourceItem, Value), SourceError> {
    let valid: bool = sqlx::query_scalar("SELECT CASE WHEN pg_input_is_valid($1,'timestamp with time zone') THEN $1::timestamptz<=scope_001_now() ELSE false END")
        .bind(as_of).fetch_one(&mut **tx).await?;
    if !valid { return Err(SourceError::Invalid); }
    if *kind == SourceKind::Comment {
        if *field != SourceField::CommentBody {
            return Err(SourceError::Invalid);
        }
        let c = linggan_evidence::read_creation_comment_at_in(tx, source, as_of)
            .await?
            .ok_or(SourceError::Unavailable)?;
        let mut i = comment_item(source, work, field, &c)?;
        i.as_of = as_of.into();
        if i.block_ref != block_ref { return Err(SourceError::Unavailable); }
        return Ok((i, c));
    }
    if *kind == SourceKind::OpsPublication {
        if !matches!(field, SourceField::Title | SourceField::Body) {
            return Err(SourceError::Invalid);
        }
        let visible: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ops_publication p JOIN linggan_ops_import_batch b ON b.batch_ref=p.last_changed_batch_ref WHERE p.publication_ref=$1 AND b.applied_at<=$2::timestamptz)")
            .bind(source).bind(as_of).fetch_one(&mut **tx).await?;
        if !visible { return Err(SourceError::Unavailable); }
        let p = read_creation_publications(tx, None, None, Some(source))
            .await?
            .into_iter()
            .next()
            .ok_or(SourceError::Unavailable)?;
        let text = if *field == SourceField::Title {
            p.title.clone()
        } else {
            p.body.clone()
        }
        .ok_or(SourceError::Unavailable)?;
        let mut i = item(
            kind.clone(),
            source,
            None,
            field.clone(),
            p.source_version.clone(),
            p.title.clone().unwrap_or_default(),
            text,
        );
        i.author_label = p
            .account_name
            .clone()
            .unwrap_or_else(|| "我方历史内容".into());
        i.as_of = as_of.into();
        if i.block_ref != block_ref { return Err(SourceError::Unavailable); }
        return Ok((i, json!({"publication":p})));
    }
    let work_id = work.ok_or(SourceError::Invalid)?;
    let d = research_dossier::read_in(tx, &[work_id], as_of)
        .await?
        .remove(&work_id)
        .ok_or(SourceError::Unavailable)?;
    let name = serde_json::to_value(field).map_err(|_| SourceError::Invalid)?;
    let f = d
        .fragments
        .iter()
        .find(|f| f.fragment_id == block_ref && f.source_ref == source && name.as_str() == Some(f.field.as_str()))
        .cloned()
        .ok_or_else(|| {
            if *kind == SourceKind::Note
                && d.fragments
                    .iter()
                    .any(|f| name.as_str() == Some(f.field.as_str()))
            {
                SourceError::Changed
            } else {
                SourceError::Unavailable
            }
        })?;
    let expected = fragment_kind(&f.field).ok_or(SourceError::Invalid)?;
    if expected.0 != *kind || expected.1 != *field { return Err(SourceError::Invalid); }
    let i = dossier_item(&d, &f)?;
    Ok((i, json!({"dossier":d})))
}
fn source_block_ref(source: Uuid, field: &SourceField) -> String {
    format!("{source}.{}", serde_json::to_value(field).expect("field is serializable").as_str().expect("field is a string"))
}

fn fragment_kind(field: &str) -> Option<(SourceKind, SourceField)> {
    Some(match field {
        "title" => (SourceKind::Note, SourceField::Title),
        "body" => (SourceKind::Note, SourceField::Body),
        "tags" => (SourceKind::Note, SourceField::Tags),
        "ocr" => (SourceKind::Ocr, SourceField::Ocr),
        "transcript" => (SourceKind::Transcript, SourceField::Transcript),
        "frame_text" => (SourceKind::FrameText, SourceField::FrameText),
        _ => return None,
    })
}

fn dossier_item(d: &research_dossier::ResearchDossier, f: &linggan_evidence::creator_discovery::Fragment) -> Result<SourceItem, SourceError> {
    let (kind, field) = fragment_kind(&f.field).ok_or(SourceError::Invalid)?;
    let mut item = item(kind, f.source_ref, Some(d.work_ref), field, f.source_version.clone(),
        d.note["displayTitle"].as_str().unwrap_or("").to_owned(), f.text.clone());
    item.as_of = d.as_of.clone();
    item.fragment_id = f.fragment_id.clone();
    item.block_ref = f.fragment_id.clone();
    item.speaker = f.speaker.clone();
    item.structure = f.structure.clone();
    item.issues = f.issues.clone();
    item.author_label = d.metadata["author"]["displayName"].as_str().unwrap_or("作者未知").to_owned();
    item.location = block_location(d, f);
    Ok(item)
}

fn block_location(d: &research_dossier::ResearchDossier, f: &linggan_evidence::creator_discovery::Fragment) -> Option<OcrLocation> {
    let bbox: [f64;4] = serde_json::from_value(f.structure.get("bbox")?.clone()).ok()?;
    let unit = f.unit_ref?;
    let material = d.materials.iter().find(|m| m["ref"] == json!(unit))?;
    Some(OcrLocation { unit_ref:f.unit_ref?, reading_ref:f.reading_ref?,
        block_ref:f.fragment_id.clone(), slot_key:material["slotKey"].as_str()?.into(), bbox })
}

fn exact_slice(text: &str, start: usize, end: usize) -> Result<String, SourceError> {
    if start >= end || end > text.chars().count() {
        return Err(SourceError::Invalid);
    }
    Ok(text.chars().skip(start).take(end - start).collect())
}
fn selection_location(location: Option<OcrLocation>, _start: usize, _end: usize) -> Option<OcrLocation> { location }
pub async fn validate_citation(
    tx: &mut Transaction<'_, Postgres>,
    c: &Citation,
) -> Result<ValidatedCitation, SourceError> {
    let (i, _) = read_source_in(tx, &c.source_kind, c.source_ref, c.work_ref, &c.field, &c.block_ref, &c.as_of).await?;
    if i.source_version != c.source_version {
        return Err(SourceError::Changed);
    }
    let text = exact_slice(&i.text, c.start, c.end)?;
    let hash = text_hash(&text);
    if hash != c.text_hash || selection_location(i.location, c.start, c.end) != c.location {
        return Err(SourceError::Invalid);
    }
    Ok(ValidatedCitation {
        selected_text: text,
        text_hash: hash,
        speaker: i.speaker,
        content: if i.structure.is_object() { i.structure } else { json!({"kind":"paragraph","text":i.text}) },
        issues: i.issues,
    })
}
pub async fn qualify_citations(
    tx: &mut Transaction<'_, Postgres>,
    citations: &[Citation],
) -> Result<Vec<CitationQualification>, SourceError> {
    let mut statuses = Vec::with_capacity(citations.len());
    for c in citations {
        let result = validate_citation(tx, c).await;
        let (state, code) = match result {
            Ok(_) => ("readable", "source_readable"),
            Err(SourceError::Database(e)) => return Err(SourceError::Database(e)),
            Err(e) => (
                match e {
                    SourceError::Changed => "changed",
                    SourceError::Restricted => "restricted",
                    _ => "unavailable",
                },
                e.code(),
            ),
        };
        statuses.push(CitationQualification {
            source_ref: c.source_ref,
            state: state.into(),
            code: code.into(),
        });
    }
    Ok(statuses)
}
async fn ensure_source_domain_in(
    tx: &mut Transaction<'_, Postgres>,
    kind: &SourceKind,
    source: Uuid,
    work: Option<Uuid>,
    domain: Uuid,
) -> Result<(), SourceError> {
    let allowed = if *kind == SourceKind::OpsPublication {
        !read_creation_publications(tx, Some(domain), None, Some(source))
            .await?
            .is_empty()
    } else {
        let work = if *kind == SourceKind::Comment {
            let c = linggan_evidence::read_creation_comment_in(tx, source)
                .await?
                .ok_or(SourceError::Unavailable)?;
            if c["qualification"] == "restricted" {
                return Err(SourceError::Restricted);
            }
            serde_json::from_value(c["workRef"].clone()).map_err(|_| SourceError::Invalid)?
        } else {
            work.ok_or(SourceError::Invalid)?
        };
        research_dossier::creation_work_in_domain(tx, work, domain).await?
    };
    if allowed {
        Ok(())
    } else {
        Err(SourceError::Unavailable)
    }
}
pub async fn validate_citation_in_domain(
    tx: &mut Transaction<'_, Postgres>,
    c: &Citation,
    domain: Uuid,
) -> Result<ValidatedCitation, SourceError> {
    ensure_source_domain_in(tx, &c.source_kind, c.source_ref, c.work_ref, domain).await?;
    validate_citation(tx, c).await
}
pub async fn qualify_citations_in_domain(
    tx: &mut Transaction<'_, Postgres>,
    citations: &[Citation],
    domain: Uuid,
) -> Result<Vec<CitationQualification>, SourceError> {
    let mut out = Vec::new();
    for c in citations {
        let result = validate_citation_in_domain(tx, c, domain).await;
        let (state, code) = match result {
            Ok(_) => ("readable", "source_readable"),
            Err(SourceError::Database(e)) => return Err(SourceError::Database(e)),
            Err(e) => (
                match e {
                    SourceError::Changed => "changed",
                    SourceError::Restricted => "restricted",
                    _ => "unavailable",
                },
                e.code(),
            ),
        };
        out.push(CitationQualification {
            source_ref: c.source_ref,
            state: state.into(),
            code: code.into(),
        });
    }
    Ok(out)
}
pub async fn validate_anchor(db: &Database, a: &AnchorRequest) -> Result<Value, SourceError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    ensure_source_domain_in(
        &mut tx,
        &a.source_kind,
        a.source_ref,
        a.work_ref,
        a.domain_ref,
    )
    .await?;
    let (i, _) =
        read_source_in(&mut tx, &a.source_kind, a.source_ref, a.work_ref, &a.field, &a.block_ref, &a.as_of).await?;
    if i.source_version != a.source_version {
        return Err(SourceError::Changed);
    }
    let text = exact_slice(&i.text, a.start, a.end)?;
    let hash = text_hash(&text);
    let citation = Citation {
        block_ref: a.block_ref.clone(),
        as_of: a.as_of.clone(),
        source_kind: a.source_kind.clone(),
        source_ref: a.source_ref,
        work_ref: a.work_ref,
        field: a.field.clone(),
        source_version: a.source_version.clone(),
        start: a.start,
        end: a.end,
        text_hash: hash.clone(),
        location: selection_location(i.location, a.start, a.end),
    };
    tx.commit().await?;
    Ok(json!({"citation":citation,"selectedText":text,"textHash":hash}))
}
pub async fn read_source(
    db: &Database,
    domain: Uuid,
    kind: &SourceKind,
    source: Uuid,
    work: Option<Uuid>,
    field: &SourceField,
    block_ref: &str,
    source_version: &str,
    as_of: &str,
) -> Result<Value, SourceError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    ensure_source_domain_in(&mut tx, kind, source, work, domain).await?;
    let (i, context) = read_source_in(&mut tx, kind, source, work, field, block_ref, as_of).await?;
    if i.source_version != source_version { return Err(SourceError::Changed); }
    tx.commit().await?;
    Ok(json!({"item":i,"context":context}))
}

pub async fn list_sources(db: &Database, q: &SourceQuery) -> Result<Value, SourceError> {
    if q.query.as_ref().is_some_and(|s| s.chars().count() > 200)
        || q.cursor.unwrap_or(0) > 1_000_000
    {
        return Err(SourceError::Invalid);
    }
    let domain = q.domain_ref.ok_or(SourceError::Invalid)?;
    let kind = q.kind.clone().unwrap_or(SourceKind::Note);
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let offset = q.cursor.unwrap_or(0) as i64;
    let (mut refs, total) = match kind {
        SourceKind::Comment => {
            linggan_evidence::creation_comment_directory_in(
                &mut tx,
                domain,
                q.query.as_deref(),
                offset,
            )
            .await?
        }
        SourceKind::OpsPublication => {
            crate::ops_ledger::read::creation_publication_directory_in(
                &mut tx,
                domain,
                q.account_ref,
                q.query.as_deref(),
                offset,
            )
            .await?
        }
        _ => {
            research_dossier::creation_work_directory_in(
                &mut tx,
                domain,
                q.query.as_deref(),
                offset,
            )
            .await?
        }
    };
    let more = refs.len() > 30;
    if more {
        refs.truncate(30)
    }
    let returned_source_count = refs.len();
    let mut items = Vec::new();
    match kind {
        SourceKind::Comment | SourceKind::OpsPublication => {
            for reference in refs {
                let field = if kind == SourceKind::Comment {
                    SourceField::CommentBody
                } else {
                    SourceField::Body
                };
                match read_source_in(&mut tx, &kind, reference, None, &field, &source_block_ref(reference, &field), &as_of).await {
                    Ok((i, _)) => items.push(i),
                    Err(SourceError::Unavailable | SourceError::Restricted) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        _ => {
            let dossiers = research_dossier::read_in(&mut tx, &refs, &as_of).await?;
            for d in dossiers.into_values() {
                for f in &d.fragments {
                    if fragment_kind(&f.field).is_some_and(|(source_kind,_)| source_kind == kind) {
                        items.push(dossier_item(&d, f)?);
                    }
                }
            }
        }
    }
    tx.commit().await?;
    let fragment_count = items.len();
    Ok(
        json!({"contract":"content-workbench-sources.v1","asOf":as_of,"items":items,"distribution":[{"sourceKind":kind,"count":total,"basis":"source_directory"}],"total":total,"totalBasis":"source_directory","returnedSourceCount":returned_source_count,"returnedFragmentCount":fragment_count,"nextCursor":if more{Some(offset+30)}else{None},"searchScope":if kind==SourceKind::Comment{"current_anonymous_comment_text"}else if kind==SourceKind::OpsPublication{"current_export_title_body"}else{"admitted_native_title_body_history_directory"},"scope":"collected_material_only"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codepoints_preserve_emoji_and_newlines() {
        assert_eq!(exact_slice("甲😀\n乙", 1, 3).unwrap(), "😀\n");
        assert!(exact_slice("😀", 0, 2).is_err());
    }
    #[test]
    fn selections_keep_the_real_block_box_without_fabricating_character_geometry() {
        let location = OcrLocation {unit_ref:Uuid::new_v4(),reading_ref:Uuid::new_v4(),
            block_ref:"canonical-block".into(),slot_key:"image:1".into(),bbox:[0.12,0.28,0.76,0.025]};
        assert_eq!(selection_location(Some(location.clone()),1,4),Some(location));
        assert!(selection_location(None,1,4).is_none());
    }
}
