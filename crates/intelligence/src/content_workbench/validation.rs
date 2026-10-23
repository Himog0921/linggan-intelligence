use super::*;
use std::collections::HashSet;
pub(crate) fn document(doc: &WorkDocument) -> Result<(), WorkbenchError> {
    if doc.contract != "content-workbench.v1"
        || doc.title.chars().count() > 500
        || doc.tags.len() > 30
        || doc.angle.chars().count() > 4000
        || doc.outline.len() > 100
        || doc.paragraphs.len() > 1000
        || doc.excerpts.len() > 200
        || doc.canvas.len() > 500
        || doc.pages.len() > 30
        || doc.uses.len() > 1000
        || doc.materials.len() > 60
    {
        return Err(WorkbenchError::Invalid("document"));
    }
    let mut material_ids = HashSet::new();
    for material in &doc.materials {
        crate::content_workbench_sources::materials::validate_selection_shape(material)
            .map_err(|_| WorkbenchError::Invalid("material_identity"))?;
        if !material_ids.insert(&material.document_id) {
            return Err(WorkbenchError::Invalid("material_duplicate"));
        }
    }
    if let Some(preparation) = &doc.material_preparation {
        if preparation.method_version != crate::content_workbench_sources::materials::METHOD
            || !matches!(preparation.state.as_str(), "ready" | "insufficient")
            || preparation.query.chars().count() > 500
            || preparation.terms.len() > 12
            || preparation
                .terms
                .iter()
                .any(|s| !(2..=40).contains(&s.chars().count()))
            || preparation.gaps.len() > 12
            || preparation.gaps.iter().any(|s| s.chars().count() > 200)
        {
            return Err(WorkbenchError::Invalid("material_preparation"));
        }
    }
    if doc.outline.iter().any(|s| s.chars().count() > 4000) {
        return Err(WorkbenchError::Invalid("outline"));
    }
    let mut tags = HashSet::new();
    if doc.tags.iter().any(|tag| {
        tag.is_empty()
            || tag.trim() != tag
            || tag.chars().count() > 50
            || tag.chars().any(char::is_control)
            || !tags.insert(tag)
    }) {
        return Err(WorkbenchError::Invalid("tags"));
    }
    let mut ids = HashSet::new();
    let mut add = |id: uuid::Uuid| {
        if id.is_nil() || !ids.insert(id) {
            Err(WorkbenchError::Invalid("duplicate_id"))
        } else {
            Ok(())
        }
    };
    for p in &doc.paragraphs {
        add(p.id)?;
        text(&p.text)?;
    }
    for e in &doc.excerpts {
        add(e.id)?;
        citation(&e.citation)?;
        if e.text_hash != e.citation.text_hash {
            return Err(WorkbenchError::Invalid("excerpt_hash"));
        }
        if let Some(t) = &e.selected_text {
            text(t)?;
            if hash(t) != e.text_hash {
                return Err(WorkbenchError::Invalid("excerpt_text"));
            }
        }
    }
    for c in &doc.canvas {
        add(c.id)?;
        text(&c.note)?;
        text(&c.expression)?;
        if c.expression_revision == 0
            || c.excerpt_id
                .is_some_and(|id| !doc.excerpts.iter().any(|e| e.id == id))
        {
            return Err(WorkbenchError::Invalid("canvas"));
        }
    }
    let mut covers = 0;
    for p in &doc.pages {
        add(p.id)?;
        text(&p.title)?;
        text(&p.subtitle)?;
        if p.kind == PageKind::Cover {
            covers += 1;
        }
        if p.blocks.len() > 30
            || !matches!(
                p.template_version.as_str(),
                "creation-v1"
                    | "creation-v1-editorial"
                    | "creation-v3-type"
                    | "creation-v3-split"
                    | "creation-v3-list"
            )
            || p.style.font_family != "system-cjk"
            || p.style.width != 1080
            || p.style.height != 1440
            || !rect([p.crop.x, p.crop.y, p.crop.width, p.crop.height])
        {
            return Err(WorkbenchError::Invalid("page"));
        }
        for b in &p.blocks {
            add(b.id)?;
            text(&b.text)?;
        }
    }
    if covers > 1 {
        return Err(WorkbenchError::Invalid("cover"));
    }
    for u in &doc.uses {
        add(u.id)?;
        if u.excerpt_id
            .is_some_and(|id| !doc.excerpts.iter().any(|e| e.id == id))
            || !sha(&u.placed_text_hash)
            || u.source_span
                .as_ref()
                .is_some_and(|s| s.start >= s.end || !sha(&s.text_hash))
        {
            return Err(WorkbenchError::Invalid("use"));
        }
    }
    if serde_json::to_vec(doc)?.len() > 2 * 1024 * 1024 {
        return Err(WorkbenchError::Invalid("document_size"));
    }
    Ok(())
}
fn text(t: &str) -> Result<(), WorkbenchError> {
    if t.chars().count() > 100_000 {
        Err(WorkbenchError::Invalid("text_size"))
    } else {
        Ok(())
    }
}
fn sha(t: &str) -> bool {
    t.len() == 64
        && t.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn rect(r: [f64; 4]) -> bool {
    r.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
        && r[2] > 0.0
        && r[3] > 0.0
        && r[0] + r[2] <= 1.000001
        && r[1] + r[3] <= 1.000001
}
fn citation(c: &Citation) -> Result<(), WorkbenchError> {
    if c.start >= c.end
        || c.end - c.start > 100_000
        || !sha(&c.text_hash)
        || c.source_version.trim().is_empty()
        || c.source_version.len() > 500
        || c.block_ref.trim().is_empty()
        || c.block_ref.len() > 500
        || c.as_of.trim().is_empty()
        || c.as_of.len() > 100
        || c.source_ref.is_nil()
    {
        return Err(WorkbenchError::Invalid("citation"));
    }
    if let Some(l) = &c.location {
        if l.unit_ref.is_nil()
            || l.reading_ref.is_nil()
            || l.block_ref != c.block_ref
            || l.slot_key.is_empty()
            || l.slot_key.len() > 500
            || !rect(l.bbox)
        {
            return Err(WorkbenchError::Invalid("ocr_location"));
        }
    }
    Ok(())
}
pub(crate) fn target_text<'a>(doc: &'a WorkDocument, target: &UseTarget) -> Option<&'a str> {
    match target {
        UseTarget::Title => Some(doc.title.as_str()),
        UseTarget::Angle => Some(doc.angle.as_str()),
        UseTarget::Outline { index } => doc.outline.get(*index).map(String::as_str),
        UseTarget::Tag { index } => doc.tags.get(*index).map(String::as_str),
        UseTarget::CanvasNote { canvas_id } => doc
            .canvas
            .iter()
            .find(|c| c.id == *canvas_id)
            .map(|c| c.note.as_str()),
        UseTarget::CanvasExpression { canvas_id } => doc
            .canvas
            .iter()
            .find(|c| c.id == *canvas_id)
            .map(|c| c.expression.as_str()),
        UseTarget::PageTitle { page_id } => doc
            .pages
            .iter()
            .find(|p| p.id == *page_id)
            .map(|p| p.title.as_str()),
        UseTarget::PageSubtitle { page_id } => doc
            .pages
            .iter()
            .find(|p| p.id == *page_id)
            .map(|p| p.subtitle.as_str()),
        UseTarget::Paragraph { paragraph_id } => doc
            .paragraphs
            .iter()
            .find(|p| p.id == *paragraph_id)
            .map(|p| p.text.as_str()),
        UseTarget::PageBlock { page_id, block_id } => doc
            .pages
            .iter()
            .find(|p| p.id == *page_id)
            .and_then(|p| p.blocks.iter().find(|b| b.id == *block_id))
            .map(|b| b.text.as_str()),
    }
}
pub(crate) fn refresh_uses(doc: &mut WorkDocument) {
    let copy = doc.clone();
    for u in &mut doc.uses {
        if u.source_span.is_some() && u.content_origin == ContentOrigin::Expression {
            u.state = UseState::NeedsReview;
            continue;
        }
        u.state = match (
            copy.canvas.iter().find(|c| c.id == u.canvas_id),
            target_text(&copy, &u.target),
        ) {
            (Some(c), Some(t))
                if c.expression_revision == u.expression_revision
                    && hash(t) == u.placed_text_hash =>
            {
                UseState::Current
            }
            (Some(_), Some(_)) => UseState::Modified,
            _ => UseState::NeedsReview,
        };
    }
}
