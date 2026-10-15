//! Current source qualification, exact placements and read-time source redaction.
use super::*;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[path = "source_metadata.rs"]
mod metadata;

pub(super) fn target_mut<'a>(
    doc: &'a mut WorkDocument,
    target: &UseTarget,
) -> Option<&'a mut String> {
    match target {
        UseTarget::Title => Some(&mut doc.title),
        UseTarget::Angle => Some(&mut doc.angle),
        UseTarget::Outline { index } => doc.outline.get_mut(*index),
        UseTarget::Tag { index } => doc.tags.get_mut(*index),
        UseTarget::CanvasNote { canvas_id } => doc
            .canvas
            .iter_mut()
            .find(|c| c.id == *canvas_id)
            .map(|c| &mut c.note),
        UseTarget::CanvasExpression { canvas_id } => doc
            .canvas
            .iter_mut()
            .find(|c| c.id == *canvas_id)
            .map(|c| &mut c.expression),
        UseTarget::PageTitle { page_id } => doc
            .pages
            .iter_mut()
            .find(|p| p.id == *page_id)
            .map(|p| &mut p.title),
        UseTarget::PageSubtitle { page_id } => doc
            .pages
            .iter_mut()
            .find(|p| p.id == *page_id)
            .map(|p| &mut p.subtitle),
        UseTarget::Paragraph { paragraph_id } => doc
            .paragraphs
            .iter_mut()
            .find(|p| p.id == *paragraph_id)
            .map(|p| &mut p.text),
        UseTarget::PageBlock { page_id, block_id } => doc
            .pages
            .iter_mut()
            .find(|p| p.id == *page_id)
            .and_then(|p| p.blocks.iter_mut().find(|b| b.id == *block_id))
            .map(|b| &mut b.text),
    }
}
pub(crate) fn authored_targets(doc: &WorkDocument) -> Vec<UseTarget> {
    let mut targets = vec![UseTarget::Title, UseTarget::Angle];
    targets.extend((0..doc.outline.len()).map(|index| UseTarget::Outline { index }));
    targets.extend((0..doc.tags.len()).map(|index| UseTarget::Tag { index }));
    targets.extend(
        doc.paragraphs
            .iter()
            .map(|p| UseTarget::Paragraph { paragraph_id: p.id }),
    );
    for page in &doc.pages {
        targets.push(UseTarget::PageTitle { page_id: page.id });
        targets.push(UseTarget::PageSubtitle { page_id: page.id });
        targets.extend(page.blocks.iter().map(|b| UseTarget::PageBlock {
            page_id: page.id,
            block_id: b.id,
        }));
    }
    for canvas in &doc.canvas {
        targets.push(UseTarget::CanvasNote {
            canvas_id: canvas.id,
        });
        targets.push(UseTarget::CanvasExpression {
            canvas_id: canvas.id,
        });
    }
    targets
}
fn repair_known_uses(doc: &mut WorkDocument, old: Option<&WorkDocument>) {
    let snapshot = doc.clone();
    let targets = authored_targets(&snapshot);
    for excerpt in &snapshot.excerpts {
        let raw = excerpt.selected_text.as_deref().unwrap_or("");
        let placements: Vec<_> = targets
            .iter()
            .filter_map(|target| {
                let text = validation::target_text(&snapshot, target)?;
                let span = exact_spans(text, raw).into_iter().next()?;
                Some((target.clone(), span, hash(text)))
            })
            .collect();
        if placements.is_empty() {
            continue;
        }
        let canvas_id =
            if let Some(canvas) = doc.canvas.iter().find(|c| c.excerpt_id == Some(excerpt.id)) {
                canvas.id
            } else {
                let prior =
                    old.and_then(|d| d.canvas.iter().find(|c| c.excerpt_id == Some(excerpt.id)));
                let id = prior
                    .filter(|previous| !doc.canvas.iter().any(|canvas| canvas.id == previous.id))
                    .map_or_else(Uuid::new_v4, |canvas| canvas.id);
                doc.canvas.push(CanvasBlock {
                    id,
                    excerpt_id: Some(excerpt.id),
                    group: CanvasGroup::Inbox,
                    note: String::new(),
                    expression: String::new(),
                    expression_revision: prior.map_or(1, |c| c.expression_revision),
                });
                id
            };
        let revision = doc
            .canvas
            .iter()
            .find(|c| c.id == canvas_id)
            .unwrap()
            .expression_revision;
        for (target, span, placed_text_hash) in placements {
            if doc
                .uses
                .iter()
                .any(|u| u.target == target && u.excerpt_id == Some(excerpt.id))
            {
                continue;
            }
            if let Some(previous) = old.and_then(|d| {
                d.uses
                    .iter()
                    .find(|u| u.target == target && u.excerpt_id == Some(excerpt.id))
            }) {
                doc.uses.push(previous.clone());
            } else {
                doc.uses.push(MaterialUse {
                    id: Uuid::new_v4(),
                    canvas_id,
                    expression_revision: revision,
                    excerpt_id: Some(excerpt.id),
                    target,
                    placed_text_hash,
                    state: UseState::Current,
                    content_origin: ContentOrigin::Excerpt,
                    source_span: Some(span),
                });
            }
        }
    }
}

pub(crate) async fn prepare(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    domain: Uuid,
    old: Option<&WorkDocument>,
    doc: &mut WorkDocument,
) -> Result<(), WorkbenchError> {
    if doc.materials.len() > 60 {
        return Err(WorkbenchError::Invalid("materials"));
    }
    for material in &mut doc.materials {
        let unchanged = old.is_some_and(|d| {
            d.materials.iter().any(|previous| {
                previous.document_id == material.document_id
                    && previous.source_kind == material.source_kind
                    && previous.source_ref == material.source_ref
                    && previous.work_ref == material.work_ref
                    && previous.source_version == material.source_version
            })
        });
        if !unchanged {
            crate::content_workbench_sources::materials::validate_selection_in(
                tx, domain, material,
            )
            .await
            .map_err(|_| WorkbenchError::Source)?;
        }
        // Collection metadata stores no caller-provided source prose. The full source is always re-read.
        material.match_reasons = vec![
            if material.added_by == "manual" {
                "作者手动加入"
            } else {
                "规则检索准备，读取时重新核对"
            }
            .into(),
        ];
    }
    // A client cannot erase already adopted source lineage while retaining its exact quote.
    // Identity comes from the old immutable revision; edited placement coordinates are recomputed.
    if let Some(old) = old {
        if doc.excerpts.iter().any(|excerpt| {
            old.excerpts
                .iter()
                .any(|previous| previous.id == excerpt.id && previous.citation != excerpt.citation)
        }) {
            return Err(WorkbenchError::Invalid("excerpt_identity"));
        }
        for excerpt in &old.excerpts {
            let raw = excerpt.selected_text.as_deref().unwrap_or("");
            if (authored_targets(doc).iter().any(|target| {
                validation::target_text(doc, target).is_some_and(|text| raw_present(text, raw))
            }) || metadata::contains_quote(doc, raw))
                && !doc.excerpts.iter().any(|e| e.id == excerpt.id)
            {
                doc.excerpts.push(excerpt.clone());
            }
        }
    }
    repair_known_uses(doc, old);
    let qualifications = crate::content_workbench_sources::qualify_citations_in_domain(
        tx,
        &doc.excerpts
            .iter()
            .map(|e| e.citation.clone())
            .collect::<Vec<_>>(),
        domain,
    )
    .await
    .map_err(|_| WorkbenchError::Source)?;
    for e in &mut doc.excerpts {
        let previous = old.and_then(|d| {
            d.excerpts
                .iter()
                .find(|p| p.id == e.id && p.citation == e.citation)
        });
        match crate::content_workbench_sources::validate_citation_in_domain(tx, &e.citation, domain)
            .await
        {
            Ok(v) => {
                e.selected_text = Some(v.selected_text);
                e.text_hash = v.text_hash;
            }
            Err(_) => {
                let Some(previous) = previous else {
                    return Err(WorkbenchError::Source);
                };
                if e.selected_text
                    .as_ref()
                    .is_some_and(|t| Some(t) != previous.selected_text.as_ref())
                {
                    return Err(WorkbenchError::Source);
                }
                e.selected_text = previous.selected_text.clone();
                e.text_hash = previous.text_hash.clone();
            }
        }
    }
    repair_known_uses(doc, old);
    for (e, q) in doc.excerpts.clone().iter().zip(&qualifications) {
        if q.state != "readable" {
            if metadata::adds_quote(doc, old, e.selected_text.as_deref().unwrap_or("")) {
                return Err(WorkbenchError::Source);
            }
            for u in doc
                .uses
                .clone()
                .iter()
                .filter(|u| u.excerpt_id == Some(e.id))
            {
                let previous = old.and_then(|d| d.uses.iter().find(|p| p == &u));
                let raw = e.selected_text.as_deref().unwrap_or("");
                let added_quote = validation::target_text(doc, &u.target)
                    .is_some_and(|text| raw_present(text, raw))
                    && old
                        .and_then(|d| validation::target_text(d, &u.target))
                        .is_none_or(|text| !raw_present(text, raw));
                if previous.is_none() || added_quote {
                    return Err(WorkbenchError::Source);
                }
                if let Some(dest) = target_mut(doc, &u.target) {
                    if dest == "[来源当前不可使用]" {
                        if let Some(t) = old.and_then(|d| validation::target_text(d, &u.target)) {
                            *dest = t.into();
                        }
                    }
                }
            }
        }
    }
    if let Some(account) = doc.account_ref {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ops_account WHERE account_ref=$1 AND domain_ref=$2 AND ledger_state='in_ledger')").bind(account).bind(domain).fetch_one(&mut **tx).await?;
        if !valid {
            return Err(WorkbenchError::Invalid("accountRef"));
        }
    }
    for page in &doc.pages {
        if let Some(asset) = page.asset_id {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_asset WHERE asset_ref=$1 AND work_ref=$2)").bind(asset).bind(work).fetch_one(&mut **tx).await?;
            if !valid {
                return Err(WorkbenchError::Invalid("assetId"));
            }
        }
    }
    let snapshot = doc.clone();
    for u in &mut doc.uses {
        if let Some(e) = snapshot
            .excerpts
            .iter()
            .find(|e| Some(e.id) == u.excerpt_id)
        {
            let target = validation::target_text(&snapshot, &u.target).unwrap_or("");
            let raw = e.selected_text.as_deref().unwrap_or("");
            let previous = old.and_then(|d| {
                d.uses
                    .iter()
                    .find(|p| p.id == u.id && p.target == u.target && p.excerpt_id == u.excerpt_id)
            });
            let unchanged_direct = previous.is_some_and(|p| {
                p.content_origin == ContentOrigin::Excerpt && hash(target) == p.placed_text_hash
            });
            u.content_origin = if u.source_span.is_some() {
                if !exact_spans(target, raw).is_empty() {
                    ContentOrigin::Excerpt
                } else {
                    ContentOrigin::Expression
                }
            } else if raw_present(target, raw)
                || unchanged_direct
                || previous.is_none()
                    && snapshot
                        .canvas
                        .iter()
                        .find(|c| c.id == u.canvas_id)
                        .is_some_and(|c| {
                            c.expression.is_empty()
                                && c.expression_revision == u.expression_revision
                                && !raw.is_empty()
                        })
            {
                ContentOrigin::Excerpt
            } else {
                ContentOrigin::Expression
            };
            if u.content_origin == ContentOrigin::Excerpt && u.source_span.is_none() {
                u.source_span = exact_spans(target, raw).into_iter().next();
            }
        } else {
            u.content_origin = ContentOrigin::Expression;
        }
    }
    refresh_source_spans(doc);
    validation::refresh_uses(doc);
    validation::document(doc)?;
    Ok(())
}
pub(crate) async fn redact(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    doc: &mut WorkDocument,
) -> Result<Vec<SourceStatus>, WorkbenchError> {
    let qualifications = crate::content_workbench_sources::qualify_citations_in_domain(
        tx,
        &doc.excerpts
            .iter()
            .map(|e| e.citation.clone())
            .collect::<Vec<_>>(),
        domain,
    )
    .await
    .map_err(|_| WorkbenchError::Source)?;
    let mut statuses = Vec::new();
    let mut hidden_quotes = Vec::new();
    for (e, q) in doc.excerpts.iter_mut().zip(qualifications) {
        if matches!(q.state.as_str(), "restricted" | "unavailable") {
            if let Some(raw) = e.selected_text.take() {
                hidden_quotes.push(raw);
            }
        }
        statuses.push(SourceStatus {
            excerpt_id: e.id,
            state: q.state,
            code: q.code,
        });
    }
    // Missing or moved placement metadata must not expose a known restricted full quote.
    // Citation identity comes only from the stored excerpt, never from a text similarity guess.
    let mut targets: Vec<(UseTarget, Option<SourceSpan>)> = Vec::new();
    for target in authored_targets(doc) {
        if let Some(text) = validation::target_text(doc, &target) {
            for raw in &hidden_quotes {
                for span in exact_spans(text, raw) {
                    targets.push((target.clone(), Some(span)));
                }
            }
        }
    }
    let mut handled = Vec::new();
    for (target, _) in &targets {
        if handled.contains(target) {
            continue;
        }
        handled.push(target.clone());
        let related: Vec<_> = targets.iter().filter(|(t, _)| t == target).collect();
        if let Some(text) = target_mut(doc, target) {
            if related.iter().any(|(_, span)| span.is_none()) {
                *text = "[来源当前不可使用]".into();
                continue;
            }
            let mut spans: Vec<_> = related
                .iter()
                .filter_map(|(_, s)| s.as_ref())
                .filter(|span| {
                    scalar_slice(text, span.start, span.end)
                        .is_some_and(|raw| hash(raw) == span.text_hash)
                })
                .map(|s| (s.start, s.end))
                .collect();
            spans.sort_unstable();
            let mut merged: Vec<(usize, usize)> = Vec::new();
            for (start, end) in spans {
                if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
                    last.1 = last.1.max(end);
                } else {
                    merged.push((start, end));
                }
            }
            for (start, end) in merged.into_iter().rev() {
                let prefix: String = text.chars().take(start).collect();
                let suffix: String = text.chars().skip(end).collect();
                *text = format!("{prefix}[来源当前不可使用]{suffix}");
            }
        }
    }
    metadata::redact(doc, &hidden_quotes);
    Ok(statuses)
}

pub(crate) fn raw_present(text: &str, raw: &str) -> bool {
    // This only locates the full text of an already identified Citation; never a fuzzy match.
    !raw.is_empty() && text.contains(raw)
}
pub async fn redact_revision_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    revision: &mut Revision,
) -> Result<(), WorkbenchError> {
    redact(tx, domain, &mut revision.document).await?;
    Ok(())
}

/// Coordinates use Unicode scalars, matching Citation and browser code-point offsets.
pub(crate) fn scalar_slice(text: &str, start: usize, end: usize) -> Option<String> {
    if start >= end || end > text.chars().count() {
        return None;
    }
    Some(text.chars().skip(start).take(end - start).collect())
}
fn exact_spans(text: &str, raw: &str) -> Vec<SourceSpan> {
    let chars: Vec<_> = text.chars().collect();
    let quote: Vec<_> = raw.chars().collect();
    if quote.is_empty() || quote.len() > chars.len() {
        return vec![];
    }
    // KMP preserves overlapping occurrences without quadratic repeated-prefix scans.
    let mut prefix = vec![0usize; quote.len()];
    let mut matched = 0;
    for i in 1..quote.len() {
        while matched > 0 && quote[i] != quote[matched] {
            matched = prefix[matched - 1];
        }
        if quote[i] == quote[matched] {
            matched += 1;
        }
        prefix[i] = matched;
    }
    let mut spans = Vec::new();
    matched = 0;
    let text_hash = hash(raw);
    for (index, character) in chars.iter().enumerate() {
        while matched > 0 && *character != quote[matched] {
            matched = prefix[matched - 1];
        }
        if *character == quote[matched] {
            matched += 1;
        }
        if matched == quote.len() {
            spans.push(SourceSpan {
                start: index + 1 - quote.len(),
                end: index + 1,
                text_hash: text_hash.clone(),
            });
            matched = prefix[matched - 1];
        }
    }
    spans
}
fn refresh_source_spans(doc: &mut WorkDocument) {
    let snapshot = doc.clone();
    let mut keys = Vec::new();
    let mut rebuilt = Vec::new();
    for usage in &snapshot.uses {
        if usage.source_span.is_none() {
            rebuilt.push(usage.clone());
            continue;
        }
        let key = (usage.target.clone(), usage.excerpt_id);
        if keys.contains(&key) {
            continue;
        }
        keys.push(key);
        let raw = snapshot
            .excerpts
            .iter()
            .find(|e| Some(e.id) == usage.excerpt_id)
            .and_then(|e| e.selected_text.as_deref())
            .unwrap_or("");
        let text = validation::target_text(&snapshot, &usage.target).unwrap_or("");
        let spans = exact_spans(text, raw);
        if spans.is_empty() {
            let mut updated = usage.clone();
            updated.state = UseState::NeedsReview;
            updated.content_origin = ContentOrigin::Expression;
            rebuilt.push(updated);
        } else {
            for span in spans {
                let mut updated = snapshot
                    .uses
                    .iter()
                    .find(|previous| {
                        previous.target == usage.target
                            && previous.excerpt_id == usage.excerpt_id
                            && previous.source_span.as_ref() == Some(&span)
                    })
                    .cloned()
                    .unwrap_or_else(|| {
                        let mut new = usage.clone();
                        new.id = Uuid::new_v4();
                        new
                    });
                updated.source_span = Some(span);
                updated.content_origin = ContentOrigin::Excerpt;
                rebuilt.push(updated);
            }
        }
    }
    doc.uses = rebuilt;
}
/// Source identity is the frozen, revalidated Citation, never inferred from matching text.
/// Exact occurrence scanning only locates placements of that already identified source.
pub async fn propagate_adopted_sources_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    doc: &mut WorkDocument,
    citations: &[Citation],
    targets: &[UseTarget],
) -> Result<(), WorkbenchError> {
    for citation in citations {
        let source =
            crate::content_workbench_sources::validate_citation_in_domain(tx, citation, domain)
                .await
                .map_err(|_| WorkbenchError::Source)?;
        for target in targets {
            let Some(text) = validation::target_text(doc, target) else {
                continue;
            };
            let spans = exact_spans(text, &source.selected_text);
            if spans.is_empty() {
                continue;
            }
            let placed_text_hash = hash(text);
            let excerpt_id = if let Some(e) = doc.excerpts.iter().find(|e| e.citation == *citation)
            {
                e.id
            } else {
                let id = Uuid::new_v4();
                doc.excerpts.push(Excerpt {
                    id,
                    citation: citation.clone(),
                    selected_text: Some(source.selected_text.clone()),
                    text_hash: source.text_hash.clone(),
                });
                id
            };
            let canvas_id =
                if let Some(c) = doc.canvas.iter().find(|c| c.excerpt_id == Some(excerpt_id)) {
                    c.id
                } else {
                    let id = Uuid::new_v4();
                    doc.canvas.push(CanvasBlock {
                        id,
                        excerpt_id: Some(excerpt_id),
                        group: CanvasGroup::Inbox,
                        note: String::new(),
                        expression: String::new(),
                        expression_revision: 1,
                    });
                    id
                };
            let expression_revision = doc
                .canvas
                .iter()
                .find(|c| c.id == canvas_id)
                .unwrap()
                .expression_revision;
            doc.uses
                .retain(|u| !(u.target == *target && u.excerpt_id == Some(excerpt_id)));
            for span in spans {
                doc.uses.push(MaterialUse {
                    id: Uuid::new_v4(),
                    canvas_id,
                    expression_revision,
                    excerpt_id: Some(excerpt_id),
                    target: target.clone(),
                    placed_text_hash: placed_text_hash.clone(),
                    state: UseState::Current,
                    content_origin: ContentOrigin::Excerpt,
                    source_span: Some(span),
                });
            }
        }
    }
    Ok(())
}

/// Work summaries expose only a title, so unrelated body/page sources are not re-read.
/// This temporary projection never changes the stored revision or its complete provenance.
pub(crate) async fn redact_title_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    doc: &WorkDocument,
) -> Result<String, WorkbenchError> {
    let uses: Vec<_> = doc
        .uses
        .iter()
        .filter(|usage| usage.target == UseTarget::Title)
        .cloned()
        .collect();
    let excerpts = doc
        .excerpts
        .iter()
        .filter(|excerpt| {
            uses.iter()
                .any(|usage| usage.excerpt_id == Some(excerpt.id))
                || excerpt
                    .selected_text
                    .as_ref()
                    .is_some_and(|raw| raw_present(&doc.title, raw))
        })
        .cloned()
        .collect();
    let mut projection = WorkDocument {
        title: doc.title.clone(),
        uses,
        excerpts,
        ..WorkDocument::default()
    };
    redact(tx, domain, &mut projection).await?;
    Ok(projection.title)
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[test]
    fn exact_spans_keep_overlaps_codepoints_and_long_repeated_patterns() {
        let spans = exact_spans("😀aaaaa尾", "aaa");
        assert_eq!(
            spans.iter().map(|s| (s.start, s.end)).collect::<Vec<_>>(),
            vec![(1, 4), (2, 5), (3, 6)]
        );
        let text = "a".repeat(100_000);
        let quote = "a".repeat(50_000);
        let spans = exact_spans(&text, &quote);
        assert_eq!(spans.len(), 50_001);
        assert_eq!((spans[0].start, spans[0].end), (0, 50_000));
        assert_eq!((spans[50_000].start, spans[50_000].end), (50_000, 100_000));
    }
}
