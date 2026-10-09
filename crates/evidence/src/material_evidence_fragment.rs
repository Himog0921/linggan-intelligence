//! The single bounded original-voice excerpt a work carries into the library list.
//!
//! `media-lifecycle-contract.md` already draws the line this module implements: restricted raw
//! material stays inside the controlled Inspector, while ordinary lists carry a bounded excerpt.
//! Nothing here returns a whole body, a whole comment or a whole OCR page — every branch goes
//! through [`window`], which caps the returned text and reports whether it was cut.
//!
//! The excerpt is evidence, never a summary. It is copied verbatim out of one named source and
//! always travels with the source kind, so the reader can tell an author's own sentence from
//! text a machine read off an image. Reader comments remain restricted source material and are
//! only returned from the explicitly authorised comment-research channel.

use crate::material_projection_types::{MaterialEvidenceFragment, MaterialLibraryItem};
use sqlx::{AssertSqlSafe, Postgres, Row, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

/// Total characters an excerpt may carry. Chosen against the V9 research row, where the quote
/// occupies one line of a two-line clamp at the narrowest supported column.
const EXCERPT_CHARS: usize = 88;

/// Characters of run-up kept before a search hit so the match lands inside a readable phrase
/// rather than at the very start of the excerpt.
const LEAD_IN_CHARS: usize = 20;

/// Reads the strongest available excerpt for one work.
///
/// With a query, "strongest" means the match the reader is looking for, so machine-read text
/// is considered after the work body. Someone searching a phrase that appears inside an image
/// can still locate it by the slot key carried back with the excerpt.
///
/// Without a query the order flips to favour legibility. Real production OCR of a stylised
/// cover is frequently noise — `"oy 六 字 =] Li"` is a verbatim example from this library — and
/// a row quoting that reads as a broken page rather than as evidence. So an unqueried row
/// prefers the author's own body text and only falls back to machine-read text, preferring a
/// body image over a cover. A reader comment is never a list excerpt: a bounded length does not
/// make restricted comment source material safe for this ordinary list read.
///
/// Returns `None` when the work genuinely has no readable text yet. That is a real state — an
/// accepted discovery whose detail, media and comments were never acquired — and the caller
/// must render it as such instead of inventing a placeholder line.
pub(crate) async fn read_batch(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    query: Option<&str>,
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let query = query.map(str::trim).filter(|value| !value.is_empty());
    if let Some(query) = query {
        for item in items.iter_mut() {
            item.evidence_fragment = item
                .body_text
                .as_deref()
                .and_then(|body| match_in(body, query))
                .map(|hit| fragment_from(hit, "detail_body", "SEARCH_MATCH", None, None));
        }
    }
    // Probe once in this transaction, only when a source-text read is actually needed.
    let ready = if items.iter().any(|item| item.evidence_fragment.is_none()) {
        ocr_retirement_schema_ready(tx).await?
    } else {
        false
    };
    if let Some(query) = query {
        fill_text_candidates(tx, items, Some(query), as_of, ready, true).await?;
        fill_text_candidates(tx, items, Some(query), as_of, ready, false).await?;
    }
    for item in items
        .iter_mut()
        .filter(|item| item.evidence_fragment.is_none())
    {
        item.evidence_fragment = item
            .body_text
            .as_deref()
            .map(str::trim)
            .filter(|body| !body.is_empty())
            .map(|body| {
                fragment_from(
                    window(body, None),
                    "detail_body",
                    "FIRST_AVAILABLE",
                    None,
                    None,
                )
            });
    }
    fill_text_candidates(tx, items, None, as_of, ready, true).await?;
    fill_text_candidates(tx, items, None, as_of, ready, false).await?;
    for item in items {
        item.body_text = None;
    }
    Ok(())
}

async fn fill_text_candidates(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [MaterialLibraryItem],
    query: Option<&str>,
    as_of: &str,
    retirement_ready: bool,
    image_layer: bool,
) -> Result<(), sqlx::Error> {
    let refs: Vec<Uuid> = items
        .iter()
        .filter(|item| item.evidence_fragment.is_none())
        .map(|item| item.identity.public_ref)
        .collect();
    if refs.is_empty() || (image_layer && !retirement_ready) {
        return Ok(());
    }
    let mut fragments =
        read_text_candidates(tx, &refs, query, as_of, retirement_ready, image_layer).await?;
    for item in items
        .iter_mut()
        .filter(|item| item.evidence_fragment.is_none())
    {
        item.evidence_fragment = fragments.remove(&item.identity.public_ref);
    }
    Ok(())
}

/// The excerpt plus where the query landed inside it, in characters.
struct Excerpt {
    text: String,
    truncated: bool,
    match_offset: Option<usize>,
    match_length: Option<usize>,
}

fn fragment_from(
    excerpt: Excerpt,
    source_kind: &'static str,
    selection_basis: &'static str,
    source_ref: Option<Uuid>,
    slot_key: Option<String>,
) -> MaterialEvidenceFragment {
    MaterialEvidenceFragment {
        text: excerpt.text,
        source_kind,
        selection_basis,
        truncated: excerpt.truncated,
        source_ref,
        slot_key,
        match_offset: excerpt.match_offset,
        match_length: excerpt.match_length,
    }
}

/// Case-insensitive containment, returning the excerpt window around the hit.
///
/// The search runs against the collapsed text rather than the raw source, because that is the
/// text the excerpt will actually carry. Searching the raw string first and collapsing after
/// would shift every offset by however much whitespace sat in the run-up — which for OCR output,
/// where hard line breaks are the norm, is enough to highlight the wrong words.
fn match_in(haystack: &str, needle: &str) -> Option<Excerpt> {
    let text = collapse_whitespace(haystack);
    if text.is_empty() {
        return None;
    }
    let lower_text = text.to_lowercase();
    let lower_needle = needle.to_lowercase();
    if lower_needle.is_empty() {
        return None;
    }
    let byte_hit = lower_text.find(&lower_needle)?;
    // Map the byte position in the lowercased copy back to a character position. This holds for
    // the scripts this product reads, where lowercasing is either identity (CJK) or 1:1 (Latin).
    let char_hit = lower_text[..byte_hit].chars().count();
    let needle_chars = lower_needle.chars().count();
    Some(window_collapsed(&text, Some((char_hit, needle_chars))))
}

/// Cuts `text` down to [`EXCERPT_CHARS`], collapsing its whitespace first.
///
/// OCR output and note bodies carry hard line breaks that would otherwise eat most of the
/// character budget and break the single-line quote treatment.
fn window(text: &str, hit: Option<(usize, usize)>) -> Excerpt {
    window_collapsed(&collapse_whitespace(text), hit)
}

/// The windowing itself, on text whose whitespace is already collapsed. Every offset in and out
/// of this function is a character offset, never a byte offset — the front end highlights with
/// `Array.from`, and a byte offset would split a CJK codepoint.
fn window_collapsed(flat: &str, hit: Option<(usize, usize)>) -> Excerpt {
    let chars: Vec<char> = flat.chars().collect();
    let total = chars.len();

    let (start, hit_len) = match hit {
        None => (0, None),
        Some((offset, len)) => (offset.min(total).saturating_sub(LEAD_IN_CHARS), Some(len)),
    };
    let start = start.min(total.saturating_sub(EXCERPT_CHARS.min(total)));
    let end = (start + EXCERPT_CHARS).min(total);

    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < total {
        out.push('…');
    }

    let lead_ellipsis = usize::from(start > 0);
    Excerpt {
        text: out,
        truncated: start > 0 || end < total,
        match_offset: hit.map(|(offset, _)| offset.saturating_sub(start) + lead_ellipsis),
        match_length: hit_len,
    }
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.trim().chars() {
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(character);
    }
    out
}

/// Same selectors for one or many works: ACCEPTED substantive OCR precedes derived text;
/// unqueried derived text excludes covers, and no restricted comment becomes a list excerpt.
async fn read_text_candidates(
    tx: &mut Transaction<'_, Postgres>,
    refs: &[Uuid],
    query: Option<&str>,
    as_of: &str,
    retirement_ready: bool,
    image_layer: bool,
) -> Result<HashMap<Uuid, MaterialEvidenceFragment>, sqlx::Error> {
    let retirement_filter = if retirement_ready {
        " AND NOT EXISTS (SELECT 1 FROM linggan_media_ocr_retirement retired WHERE retired.retired_job_ref=job.job_ref)"
    } else {
        ""
    };
    // All SQL fragments are private compile-time literals; search and identity stay bound.
    let sql = if image_layer {
        "SELECT DISTINCT ON (layout.content_public_ref) layout.content_public_ref,derivative.derivative_ref,layer.image_substantive_text AS display_text,'image_substantive_text'::text AS kind,job.slot_key \
         FROM linggan_media_ocr_layering_result layer JOIN linggan_media_ocr_layout layout USING(layout_ref) \
         JOIN linggan_media_derivative derivative ON derivative.derivative_ref=layout.ocr_derivative_ref \
         JOIN linggan_media_processing_job job ON job.job_ref=derivative.job_ref \
         WHERE layout.content_public_ref=ANY($1) AND layer.state='ACCEPTED' AND layer.created_at <= $2::timestamptz \
           AND nullif(btrim(layer.image_substantive_text),'') IS NOT NULL \
           AND ($3::text IS NULL OR lower(layer.image_substantive_text) LIKE '%' || lower($3) || '%') \
           AND ($3::text IS NOT NULL OR job.slot_key NOT LIKE '%:cover:%') \
           AND NOT EXISTS (SELECT 1 FROM linggan_media_ocr_retirement retired WHERE retired.retired_job_ref=job.job_ref) \
         ORDER BY layout.content_public_ref,layer.created_at DESC".to_owned()
    } else {
        format!(
            "SELECT DISTINCT ON (derived.content_public_ref) derived.content_public_ref,derived.derivative_ref,derived.kind,derived.display_text,job.slot_key \
         FROM linggan_material_derived_text derived JOIN linggan_media_derivative derivative USING(derivative_ref) \
         JOIN linggan_media_processing_job job USING(job_ref) \
         WHERE derived.content_public_ref=ANY($1) AND derived.created_at <= $2::timestamptz \
           AND ($3::text IS NULL OR lower(derived.display_text) LIKE '%' || lower($3) || '%') \
           AND ($3::text IS NOT NULL OR job.slot_key NOT LIKE '%:cover:%'){retirement_filter} \
         ORDER BY derived.content_public_ref,job.slot_key,derived.created_at DESC"
        )
    };
    let rows = sqlx::query(AssertSqlSafe(sql))
        .bind(refs)
        .bind(as_of)
        .bind(query)
        .fetch_all(&mut **tx)
        .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let text: String = row.get("display_text");
            let excerpt = match query {
                Some(query) => match_in(&text, query),
                None => {
                    let trimmed = text.trim();
                    (!trimmed.is_empty()).then(|| window(trimmed, None))
                }
            }?;
            let kind: String = row.get("kind");
            let fragment = fragment_from(
                excerpt,
                if image_layer {
                    "image_substantive_text"
                } else {
                    derived_source_kind(&kind)
                },
                if query.is_some() {
                    "SEARCH_MATCH"
                } else {
                    "FIRST_AVAILABLE"
                },
                row.get("derivative_ref"),
                row.get("slot_key"),
            );
            Some((row.get("content_public_ref"), fragment))
        })
        .collect())
}

async fn ocr_retirement_schema_ready(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT to_regclass('linggan_media_ocr_retirement') IS NOT NULL")
        .fetch_one(&mut **tx)
        .await
}

fn derived_source_kind(kind: &str) -> &'static str {
    match kind {
        "asr_text" => "asr_text",
        "frame_ocr_text" => "frame_ocr_text",
        _ => "ocr_text",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_returned_whole_and_not_marked_truncated() {
        let excerpt = window("休息时也在等待下一件事", None);
        assert_eq!(excerpt.text, "休息时也在等待下一件事");
        assert!(!excerpt.truncated);
        assert_eq!(excerpt.match_offset, None);
    }

    #[test]
    fn long_text_is_cut_to_the_budget_and_marked() {
        let source = "字".repeat(300);
        let excerpt = window(&source, None);
        // One trailing ellipsis is added on top of the character budget.
        assert_eq!(excerpt.text.chars().count(), EXCERPT_CHARS + 1);
        assert!(excerpt.truncated);
    }

    #[test]
    fn a_hit_deep_in_the_text_keeps_its_run_up() {
        let source = format!("{}成瘾{}", "前".repeat(120), "后".repeat(120));
        let excerpt = match_in(&source, "成瘾").expect("the needle is present");
        let chars: Vec<char> = excerpt.text.chars().collect();
        let offset = excerpt
            .match_offset
            .expect("a search hit reports an offset");
        assert_eq!(chars[offset], '成');
        assert_eq!(chars[offset + 1], '瘾');
        assert_eq!(excerpt.match_length, Some(2));
        assert!(excerpt.truncated);
    }

    #[test]
    fn a_hit_near_the_start_is_not_pushed_off_the_front() {
        let excerpt = match_in("成瘾问题很少被人看见", "成瘾").expect("the needle is present");
        assert_eq!(excerpt.match_offset, Some(0));
        assert!(!excerpt.truncated);
    }

    #[test]
    fn line_breaks_collapse_so_ocr_text_stays_on_one_line() {
        let excerpt = window("ALVA\n\n@alvatheadhder\n2026年8月30日", None);
        assert_eq!(excerpt.text, "ALVA @alvatheadhder 2026年8月30日");
    }

    #[test]
    fn matching_ignores_case_for_latin_queries() {
        let excerpt = match_in("ADHD 人群常常被忽视", "adhd").expect("the needle is present");
        assert_eq!(excerpt.match_offset, Some(0));
        assert_eq!(excerpt.match_length, Some(4));
    }

    #[test]
    fn a_missing_needle_reports_no_excerpt() {
        assert!(match_in("完全不相干的一句话", "成瘾").is_none());
    }

    #[test]
    fn empty_source_text_never_becomes_an_excerpt() {
        assert!(match_in("   ", "成瘾").is_none());
    }
}
