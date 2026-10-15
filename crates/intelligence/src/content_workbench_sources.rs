//! One qualified source/version supplies reading, highlighting and exact citations.
use crate::content_workbench::types::*;
use crate::ops_ledger::read::read_creation_publications;
use linggan_evidence::research_dossier;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

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
        fragment_id: format!(
            "{source}.{}",
            serde_json::to_value(&field)
                .unwrap_or(Value::Null)
                .as_str()
                .unwrap_or("unknown")
        ),
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
pub async fn read_source_in(
    tx: &mut Transaction<'_, Postgres>,
    kind: &SourceKind,
    source: Uuid,
    work: Option<Uuid>,
    field: &SourceField,
) -> Result<(SourceItem, Value), SourceError> {
    if *kind == SourceKind::Comment {
        if *field != SourceField::CommentBody {
            return Err(SourceError::Invalid);
        }
        let c = linggan_evidence::read_creation_comment_in(tx, source)
            .await?
            .ok_or(SourceError::Unavailable)?;
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
        let version = text_hash(
            &json!([source, c["packageRef"], c["observedAt"], text_hash(&text)]).to_string(),
        );
        let mut i = item(
            kind.clone(),
            source,
            Some(work_id),
            field.clone(),
            version,
            "匿名原声".into(),
            text,
        );
        i.author_label = "匿名评论来源".into();
        i.title = c["parentWork"]["fragments"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|f| f["field"] == "title")
            .and_then(|f| f["text"].as_str())
            .map(|s| s.to_owned())
            .unwrap_or_else(|| "匿名原声 · 父笔记未知".into());
        return Ok((i, c));
    }
    if *kind == SourceKind::OpsPublication {
        if !matches!(field, SourceField::Title | SourceField::Body) {
            return Err(SourceError::Invalid);
        }
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
        return Ok((i, json!({"publication":p})));
    }
    let work_id = work.ok_or(SourceError::Invalid)?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let mut d = research_dossier::read_in(tx, &[work_id], &as_of)
        .await?
        .remove(&work_id)
        .ok_or(SourceError::Unavailable)?;
    let name = serde_json::to_value(field).map_err(|_| SourceError::Invalid)?;
    let f = d
        .fragments
        .iter()
        .find(|f| f.source_ref == source && name.as_str() == Some(f.field.as_str()))
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
    if (*kind == SourceKind::Note && !matches!(field, SourceField::Title | SourceField::Body))
        || (*kind == SourceKind::Ocr
            && !matches!(field, SourceField::Ocr | SourceField::CoverHeadline))
    {
        return Err(SourceError::Invalid);
    }
    let title = d
        .fragments
        .iter()
        .find(|f| f.field == "title")
        .map(|f| f.text.clone())
        .unwrap_or_default();
    let mut i = item(
        kind.clone(),
        source,
        Some(work_id),
        field.clone(),
        f.source_version,
        title,
        f.text,
    );
    i.author_label = d.metadata["authorDisplayName"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if i.title.is_empty() {
                "作者未知 · 来源编号分组".into()
            } else {
                "作者未知 · 原篇标题分组".into()
            }
        });
    if *kind == SourceKind::Ocr {
        let m = d
            .media
            .iter_mut()
            .find(|m| m["derivativeRef"] == json!(source))
            .ok_or(SourceError::Unavailable)?;
        research_dossier::enrich_layout(tx, m).await?;
        i.location = Some(ocr_location(m, &i.text)?);
    }
    Ok((i, json!({"dossier":d})))
}
fn ocr_location(media: &Value, text: &str) -> Result<OcrLocation, SourceError> {
    let layer = &media["ocrLayering"];
    let parse =
        |v: &Value| serde_json::from_value::<Uuid>(v.clone()).map_err(|_| SourceError::Unavailable);
    let retained = layer["retainedLines"]
        .as_array()
        .ok_or(SourceError::Unavailable)?;
    let mut spans = Vec::new();
    let mut offset = 0;
    // The semantic layer joins retained lines verbatim; if a processor changed text, no
    // approximate contains match is promoted into an exact image location.
    for line in layer["lines"].as_array().ok_or(SourceError::Unavailable)? {
        let line_ref = parse(&line["lineRef"])?;
        if !retained.iter().any(|r| r["lineRef"] == json!(line_ref)) {
            continue;
        }
        let s = line["text"]
            .as_str()
            .ok_or(SourceError::Unavailable)?
            .trim();
        let remaining: String = text.chars().skip(offset).collect();
        let leading = remaining.chars().take_while(|c| c.is_whitespace()).count();
        let tail: String = remaining.chars().skip(leading).collect();
        if !tail.starts_with(s) {
            continue;
        }
        let start = offset + leading;
        let end = start + s.chars().count();
        offset = end;
        let b = line["bbox"].as_array().ok_or(SourceError::Unavailable)?;
        if b.len() != 4 {
            return Err(SourceError::Unavailable);
        }
        let coords: Vec<f64> = b
            .iter()
            .map(|v| v.as_f64().ok_or(SourceError::Unavailable))
            .collect::<Result<_, _>>()?;
        let bbox = [
            coords[0],
            coords[1],
            coords[2] - coords[0],
            coords[3] - coords[1],
        ];
        if bbox.iter().any(|v| !v.is_finite() || *v < 0.0)
            || bbox[0] + bbox[2] > 1.0
            || bbox[1] + bbox[3] > 1.0
        {
            return Err(SourceError::Unavailable);
        }
        spans.push(OcrLineSpan {
            line_ref,
            start,
            end,
            bbox,
        });
    }
    if spans.is_empty() {
        return Err(SourceError::Unavailable);
    }
    Ok(OcrLocation {
        derivative_ref: parse(&media["derivativeRef"])?,
        layout_ref: parse(&layer["layoutRef"])?,
        layering_ref: parse(&layer["layeringRef"])?,
        blob_sha256: media["semanticMediaSource"]["blobSha256"]
            .as_str()
            .ok_or(SourceError::Unavailable)?
            .into(),
        slot_key: media["slotKey"]
            .as_str()
            .ok_or(SourceError::Unavailable)?
            .into(),
        line_spans: spans,
    })
}
fn exact_slice(text: &str, start: usize, end: usize) -> Result<String, SourceError> {
    if start >= end || end > text.chars().count() {
        return Err(SourceError::Invalid);
    }
    Ok(text.chars().skip(start).take(end - start).collect())
}
fn selection_location(
    location: Option<OcrLocation>,
    start: usize,
    end: usize,
) -> Option<OcrLocation> {
    location.map(|mut l| {
        l.line_spans.retain(|s| s.start < end && s.end > start);
        l
    })
}
pub async fn validate_citation(
    tx: &mut Transaction<'_, Postgres>,
    c: &Citation,
) -> Result<ValidatedCitation, SourceError> {
    let (i, _) = read_source_in(tx, &c.source_kind, c.source_ref, c.work_ref, &c.field).await?;
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
        read_source_in(&mut tx, &a.source_kind, a.source_ref, a.work_ref, &a.field).await?;
    if i.source_version != a.source_version {
        return Err(SourceError::Changed);
    }
    let text = exact_slice(&i.text, a.start, a.end)?;
    let hash = text_hash(&text);
    let citation = Citation {
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
) -> Result<Value, SourceError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    ensure_source_domain_in(&mut tx, kind, source, work, domain).await?;
    let (i, context) = read_source_in(&mut tx, kind, source, work, field).await?;
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
                match read_source_in(&mut tx, &kind, reference, None, &field).await {
                    Ok((i, _)) => items.push(i),
                    Err(SourceError::Unavailable | SourceError::Restricted) => {}
                    Err(e) => return Err(e),
                }
            }
        }
        _ => {
            let dossiers = research_dossier::read_in(&mut tx, &refs, &as_of).await?;
            for d in dossiers.into_values() {
                let title = d
                    .fragments
                    .iter()
                    .find(|f| f.field == "title")
                    .map(|f| f.text.clone())
                    .unwrap_or_default();
                for f in d.fragments {
                    let field = match (kind.clone(), f.field.as_str()) {
                        (SourceKind::Note, "title") => SourceField::Title,
                        (SourceKind::Note, "body") => SourceField::Body,
                        (SourceKind::Ocr, "ocr") => SourceField::Ocr,
                        (SourceKind::Ocr, "cover_headline") => SourceField::CoverHeadline,
                        _ => continue,
                    };
                    let mut entry = item(
                        kind.clone(),
                        f.source_ref,
                        Some(d.work_ref),
                        field,
                        f.source_version,
                        title.clone(),
                        f.text,
                    );
                    entry.author_label = d.metadata["authorDisplayName"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| "作者未知 · 原篇标题分组".into());
                    items.push(entry);
                }
            }
        }
    }
    for i in &mut items {
        if i.source_kind == SourceKind::Ocr {
            match read_source_in(&mut tx, &i.source_kind, i.source_ref, i.work_ref, &i.field).await
            {
                Ok((full, _)) => *i = full,
                Err(SourceError::Unavailable) => i.qualification = "location_unavailable".into(),
                Err(e) => return Err(e),
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
    fn nonorigin_ocr_box_and_unicode_line_mapping() {
        let line = Uuid::new_v4();
        let next = Uuid::new_v4();
        let layout = Uuid::new_v4();
        let derivative = Uuid::new_v4();
        let media = json!({"derivativeRef":derivative,"slotKey":"xhs:synthetic:image:2","semanticMediaSource":{"blobSha256":"a".repeat(64)},"ocrLayering":{"layoutRef":layout,"layeringRef":Uuid::new_v4(),"retainedLines":[{"lineRef":line},{"lineRef":next}],"lines":[{"lineRef":line,"text":" 甲😀 ","bbox":[0.12,0.28,0.88,0.305]},{"lineRef":next,"text":"乙","bbox":[0.2,0.4,0.9,0.5]}]}});
        let location = ocr_location(&media, "甲😀\n乙").unwrap();
        assert_eq!(location.layout_ref, layout);
        assert_eq!(location.line_spans[0].start, 0);
        assert_eq!(location.line_spans[0].end, 2);
        assert_eq!(location.line_spans[1].start, 3);
        assert!((location.line_spans[0].bbox[2] - 0.76).abs() < 1e-10);
        assert!((location.line_spans[0].bbox[3] - 0.025).abs() < 1e-10);
        let selected = selection_location(Some(location), 1, 4).unwrap();
        assert_eq!(selected.line_spans.len(), 2);
        assert!(ocr_location(&media, "模型改写过的文字").is_err());
    }
}
