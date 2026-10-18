//! Bounded candidate retrieval and explicit reading-collection preparation.
use super::documents::{note_document_in, read_comment_documents_in};
use super::*;
use sqlx::Row;
use std::collections::BTreeSet;

pub(super) fn response(
    as_of: &str,
    query: &str,
    terms: &[String],
    documents: Vec<MaterialDocument>,
    more: bool,
    cursor: usize,
    gaps: Vec<String>,
) -> Value {
    json!({"contract":"content-workbench-materials.v1","asOf":as_of,"methodVersion":METHOD,"query":query,"terms":terms,"state":if gaps.is_empty(){"ready"}else{"insufficient"},"gaps":gaps,"documents":documents,"nextCursor":if more{Some(cursor+LIMIT)}else{None},"searchScope":"native_title_body_candidates_current_dossier_and_anonymous_comments_bounded_120_no_semantic_search"})
}
pub(super) async fn begin(db: &Database) -> Result<Transaction<'_, Postgres>, SourceError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
async fn candidates_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    terms: &[String],
    cursor: usize,
) -> Result<(Vec<Uuid>, bool), SourceError> {
    let mut refs = BTreeSet::new();
    let mut truncated = false;
    for term in terms {
        let (rows, total) =
            research_dossier::creation_work_directory_in(tx, domain, Some(term), cursor as i64)
                .await?;
        truncated |= total > (cursor + 30) as i64;
        refs.extend(rows.into_iter().take(30));
    }
    truncated |= refs.len() > CANDIDATE_LIMIT;
    Ok((refs.into_iter().take(CANDIDATE_LIMIT).collect(), truncated))
}
pub async fn prepare(db: &Database, request: &PrepareRequest) -> Result<Value, SourceError> {
    let terms = intent_terms(request)?;
    let _ = request.topic_ref; // Topic linkage is retained by the work; lexical results do not assert classification.
    collect(
        db,
        request.domain_ref,
        &request.title,
        &terms,
        true,
        0,
        None,
    )
    .await
}
pub async fn search(db: &Database, q: &SearchQuery) -> Result<Value, SourceError> {
    if q.query.chars().count() > 200
        || q.cursor.unwrap_or(0) > 10_000
        || q.kind == Some(SourceKind::Ocr)
    {
        return Err(SourceError::Invalid);
    }
    let terms = permitted_terms(
        &q.query
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )?;
    collect(
        db,
        q.domain_ref,
        &q.query,
        &terms,
        false,
        q.cursor.unwrap_or(0),
        q.kind.as_ref(),
    )
    .await
}
async fn collect(
    db: &Database,
    domain: Uuid,
    query: &str,
    terms: &[String],
    prepare: bool,
    cursor: usize,
    kind: Option<&SourceKind>,
) -> Result<Value, SourceError> {
    let mut tx = begin(db).await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let (refs, mut limited) = candidates_in(&mut tx, domain, terms, 0).await?;
    let mut documents = Vec::new();
    let mut dossiers = research_dossier::read_in(&mut tx, &refs, &as_of).await?;
    for reference in refs {
        let Some(dossier) = dossiers.remove(&reference) else {
            continue;
        };
        match note_document_in(&mut tx, dossier).await {
            Ok(mut d) => {
                let text = d
                    .fragments
                    .iter()
                    .map(|f| f.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let hits = if prepare {
                    related(&text, terms)
                } else {
                    let hits = matches(&text, terms);
                    (!hits.is_empty()).then_some(hits)
                };
                if let Some(hits) = hits {
                    d.match_reasons = reasons(&hits, false);
                    documents.push(d);
                }
            }
            Err(SourceError::Unavailable | SourceError::Restricted | SourceError::Changed) => {}
            Err(e) => return Err(e),
        }
    }
    if prepare {
        // Freeze the reading parents before reading their comments. An excluded thirteenth
        // work must not consume the comment window belonging to the selected twelve.
        documents.sort_by_key(|d| (related(&d.title, terms).is_none(), d.source_ref));
        documents.truncate(12);
    }
    let related_works: Vec<_> = documents
        .iter()
        .map(|document| document.source_ref)
        .collect();
    // Pick comments only from qualified matching works; own words must also match an action/problem.
    let comment_refs: Vec<Uuid> = if prepare {
        sqlx::query("SELECT material_ref FROM (SELECT c.material_ref,c.content_public_ref,row_number() OVER(PARTITION BY c.content_public_ref ORDER BY c.material_ref) AS parent_order FROM linggan_material_comment_current c JOIN linggan_runtime_capture_package p USING(package_ref) WHERE c.content_public_ref=ANY($1) AND p.accepted_at<=scope_001_now() AND c.body_state='KNOWN' AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id) AND EXISTS(SELECT 1 FROM unnest($2::text[]) term WHERE strpos(lower(c.body_text),term)>0)) candidates ORDER BY parent_order,content_public_ref,material_ref LIMIT 120").bind(&related_works).bind(terms).fetch_all(&mut *tx).await?.iter().map(|r|r.get("material_ref")).collect()
    } else {
        let mut refs = BTreeSet::new();
        for term in terms {
            let (rows, total) =
                linggan_evidence::creation_comment_directory_in(&mut tx, domain, Some(term), 0)
                    .await?;
            limited |= total > 30;
            refs.extend(rows.into_iter().take(30));
        }
        refs.into_iter().take(CANDIDATE_LIMIT).collect()
    };
    let mut comments = read_comment_documents_in(&mut tx, domain, &comment_refs).await?;
    for reference in comment_refs {
        match comments
            .remove(&reference)
            .unwrap_or(Err(SourceError::Unavailable))
        {
            Ok(mut d) => {
                let hits = matches(&d.fragments[0].text, terms);
                let meaningful = if prepare && terms.first().is_some_and(|t| t == "作业") {
                    hits.iter().any(|t| t != "作业" && t != "开始")
                        || (hits.iter().any(|t| t == "作业") && hits.iter().any(|t| t == "开始"))
                } else if prepare {
                    related(&d.fragments[0].text, terms).is_some()
                } else {
                    !hits.is_empty()
                };
                if meaningful {
                    d.match_reasons = reasons(&hits, prepare);
                    documents.push(d);
                }
            }
            Err(SourceError::Unavailable | SourceError::Restricted | SourceError::Changed) => {}
            Err(e) => return Err(e),
        }
    }
    if !prepare {
        let mut refs = BTreeSet::new();
        for term in terms {
            let (rows, total) = crate::ops_ledger::read::creation_publication_directory_in(
                &mut tx,
                domain,
                None,
                Some(term),
                0,
            )
            .await?;
            limited |= total > 30;
            refs.extend(rows.into_iter().take(30));
        }
        for reference in refs.into_iter().take(CANDIDATE_LIMIT) {
            match read_document_in(
                &mut tx,
                domain,
                &SourceKind::OpsPublication,
                reference,
                None,
            )
            .await
            {
                Ok(mut d) => {
                    let hits = matches(
                        &d.fragments
                            .iter()
                            .map(|f| f.text.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                        terms,
                    );
                    if !hits.is_empty() {
                        d.match_reasons = reasons(&hits, false);
                        documents.push(d);
                    }
                }
                Err(SourceError::Unavailable | SourceError::Restricted | SourceError::Changed) => {}
                Err(e) => return Err(e),
            }
        }
    }
    if let Some(kind) = kind {
        documents.retain(|document| document.source_kind == *kind);
    }
    // Preparation reserves room for notes and voices. Search paginates one stable bounded set.
    if !prepare {
        documents.sort_by_key(|d| {
            (
                kind_name(&d.source_kind),
                related(&d.title, terms).is_none(),
                d.source_ref,
            )
        });
    }
    let has_more;
    let selected = if prepare {
        let notes: Vec<_> = documents
            .iter()
            .filter(|d| d.source_kind == SourceKind::Note)
            .take(12)
            .cloned()
            .collect();
        let parents: HashSet<_> = notes.iter().map(|d| d.source_ref).collect();
        let comments: Vec<_> = documents
            .iter()
            .filter(|d| {
                d.source_kind == SourceKind::Comment
                    && d.work_ref.is_some_and(|w| parents.contains(&w))
            })
            .take(12)
            .cloned()
            .collect();
        limited |= documents.len() > LIMIT;
        let mut selected = notes;
        selected.extend(comments);
        has_more = false;
        selected
    } else {
        has_more = documents.len() > cursor + LIMIT;
        documents.into_iter().skip(cursor).take(LIMIT).collect()
    };
    let mut gaps = Vec::new();
    if terms.is_empty() {
        gaps.push("尚无足够具体的检索意图，请补充场景和问题词".into());
    }
    if prepare && !selected.iter().any(|d| d.source_kind == SourceKind::Note) {
        gaps.push("当前有界检索未找到共同匹配场景与问题的可读笔记".into());
    }
    if prepare
        && !selected
            .iter()
            .any(|d| d.source_kind == SourceKind::Comment)
    {
        gaps.push("尚未找到相关笔记下自身匹配问题的可读评论".into());
    }
    if !prepare && selected.is_empty() {
        gaps.push("当前有界检索未找到可读材料，请调整具体词语".into());
    }
    // A bounded candidate pool limits coverage; it is not itself missing reading material.
    if limited && !gaps.is_empty() {
        gaps.push("已到本次有界检索窗口，可检索更多材料补充".into());
    }
    tx.commit().await?;
    Ok(response(
        &as_of, query, terms, selected, has_more, cursor, gaps,
    ))
}
