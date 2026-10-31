//! Verify the original immutable text before proposing a new, current block binding.
use super::*;
use crate::content_workbench_sources as sources;
use legacy::LegacyExcerpt;

pub(super) async fn bind(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    excerpt: &LegacyExcerpt,
    qualified_at: &str,
) -> Result<Citation, WorkbenchError> {
    let old = &excerpt.citation;
    let selected = excerpt
        .selected_text
        .as_deref()
        .ok_or(WorkbenchError::Source)?;
    if selected.is_empty()
        || hash(selected) != excerpt.text_hash
        || excerpt.text_hash != old.text_hash
        || old.location.is_some()
    {
        return Err(WorkbenchError::Source);
    }
    let (query, expected_field) = match old.source_kind {
        SourceKind::Note => (
            "SELECT m.content_public_ref,m.body_text,m.body_state,m.package_ref,m.observed_at,linggan_human_moment(m.created_at) AS recorded_at FROM linggan_material_content_detail m JOIN linggan_runtime_capture_package p USING(package_ref) WHERE m.material_ref=$1",
            SourceField::Body,
        ),
        SourceKind::Comment => (
            "SELECT m.content_public_ref,m.body_text,m.body_state,m.package_ref,m.observed_at,linggan_human_moment(m.created_at) AS recorded_at FROM linggan_material_comment m JOIN linggan_runtime_capture_package p USING(package_ref) WHERE m.material_ref=$1",
            SourceField::CommentBody,
        ),
        _ => return Err(WorkbenchError::Invalid("legacy_source_kind")),
    };
    if old.field != expected_field {
        return Err(WorkbenchError::Source);
    }
    let row = sqlx::query(query)
        .bind(old.source_ref)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(WorkbenchError::Source)?;
    let work: Uuid = row.try_get("content_public_ref")?;
    let text: Option<String> = row.try_get("body_text")?;
    let text = text.ok_or(WorkbenchError::Source)?;
    if old.work_ref != Some(work)
        || row.try_get::<String, _>("body_state")? != "KNOWN"
        || old.start >= old.end
        || old.end > text.chars().count()
        || text
            .chars()
            .skip(old.start)
            .take(old.end - old.start)
            .collect::<String>()
            != selected
    {
        return Err(WorkbenchError::Source);
    }
    let old_version = if old.source_kind == SourceKind::Note {
        hash(serde_json::to_vec(&json!([
            row.try_get::<String, _>("recorded_at")?,
            hash(&text)
        ]))?)
    } else {
        hash(serde_json::to_vec(&json!([
            old.source_ref,
            row.try_get::<Uuid, _>("package_ref")?,
            row.try_get::<String, _>("observed_at")?,
            hash(&text)
        ]))?)
    };
    if old_version != old.source_version {
        return Err(WorkbenchError::Source);
    }
    let current_at: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let (block, version, start, end) = if old.source_kind == SourceKind::Note {
        let dossier = linggan_evidence::research_dossier::read_in(tx, &[work], &current_at)
            .await?
            .remove(&work)
            .ok_or(WorkbenchError::Source)?;
        let mut matches = Vec::new();
        for f in dossier
            .fragments
            .iter()
            .filter(|f| f.source_ref == old.source_ref && f.field == "body")
        {
            for (byte, _) in f.text.match_indices(selected) {
                let start = f.text[..byte].chars().count();
                matches.push((
                    f.fragment_id.clone(),
                    f.source_version.clone(),
                    start,
                    start + selected.chars().count(),
                ));
            }
        }
        if matches.len() != 1 {
            return Err(WorkbenchError::Source);
        }
        matches.remove(0)
    } else {
        let block = format!("{}.comment_body", old.source_ref);
        let (item, _) = sources::read_source_in(
            tx,
            &old.source_kind,
            old.source_ref,
            Some(work),
            &old.field,
            &block,
            &current_at,
        )
        .await
        .map_err(source_error)?;
        (block, item.source_version, old.start, old.end)
    };
    let mut citation = Citation {
        block_ref: block,
        as_of: current_at,
        source_kind: old.source_kind.clone(),
        source_ref: old.source_ref,
        work_ref: Some(work),
        field: old.field.clone(),
        source_version: version,
        start,
        end,
        text_hash: old.text_hash.clone(),
        location: None,
    };
    // Both current use eligibility and the approved preview's new cutoff must still qualify.
    sources::validate_citation_in_domain(tx, &citation, domain)
        .await
        .map_err(source_error)?;
    citation.as_of = qualified_at.into();
    let verified = sources::validate_citation_in_domain(tx, &citation, domain)
        .await
        .map_err(source_error)?;
    if verified.selected_text != selected {
        return Err(WorkbenchError::Source);
    }
    Ok(citation)
}
