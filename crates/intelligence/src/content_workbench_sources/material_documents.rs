//! Whole document projection built from current qualified source readers.
use super::*;

pub(super) async fn note_document_in(
    tx: &mut Transaction<'_, Postgres>,
    d: research_dossier::ResearchDossier,
) -> Result<MaterialDocument, SourceError> {
    let source = d.work_ref;
    let title = d
        .fragments
        .iter()
        .find(|f| f.field == "title")
        .map(|f| f.text.clone())
        .unwrap_or_default();
    let author = d.metadata["authorDisplayName"]
        .as_str()
        .unwrap_or("作者未知 · 原篇标题分组")
        .to_owned();
    let mut fields = Vec::new();
    for f in &d.fragments {
        let (kind, field) = match f.field.as_str() {
            "title" => (SourceKind::Note, SourceField::Title),
            "body" => (SourceKind::Note, SourceField::Body),
            "ocr" => (SourceKind::Ocr, SourceField::Ocr),
            "cover_headline" => (SourceKind::Ocr, SourceField::CoverHeadline),
            _ => continue,
        };
        let mut i = item(
            kind,
            f.source_ref,
            Some(source),
            field,
            f.source_version.clone(),
            title.clone(),
            f.text.clone(),
        );
        i.author_label = author.clone();
        if i.source_kind == SourceKind::Ocr {
            let media = d
                .media
                .iter()
                .find(|m| m["derivativeRef"] == json!(i.source_ref));
            if let Some(media) = media {
                let mut media = media.clone();
                research_dossier::enrich_layout(tx, &mut media).await?;
                match ocr_location(&media, &i.text) {
                    Ok(location) => i.location = Some(location),
                    Err(SourceError::Unavailable) => {
                        i.qualification = "location_unavailable".into()
                    }
                    Err(e) => return Err(e),
                }
            } else {
                i.qualification = "location_unavailable".into();
            }
        }
        fields.push(i);
    }
    if fields.is_empty() {
        return Err(SourceError::Unavailable);
    }
    fields.sort_by_key(|f| match f.field {
        SourceField::Title => 0,
        SourceField::Body => 1,
        SourceField::CoverHeadline => 2,
        _ => 3,
    });
    Ok(MaterialDocument {
        document_id: document_id(&SourceKind::Note, source),
        source_kind: SourceKind::Note,
        source_ref: source,
        work_ref: Some(source),
        source_version: d.material_revision.clone(),
        title,
        author_label: author,
        qualification: "readable".into(),
        fragments: fields,
        context: json!({"dossier":d}),
        match_reasons: vec![],
    })
}
pub(super) async fn read_document_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    kind: &SourceKind,
    source: Uuid,
    work: Option<Uuid>,
) -> Result<MaterialDocument, SourceError> {
    ensure_source_domain_in(tx, kind, source, work, domain).await?;
    if *kind == SourceKind::Note {
        if work != Some(source) {
            return Err(SourceError::Invalid);
        }
        let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(&mut **tx)
            .await?;
        let d = research_dossier::read_in(tx, &[source], &as_of)
            .await?
            .remove(&source)
            .ok_or(SourceError::Unavailable)?;
        return note_document_in(tx, d).await;
    }
    let (mut fragments, context, version, parent) = {
        let field = match kind {
            SourceKind::Comment => SourceField::CommentBody,
            SourceKind::OpsPublication => SourceField::Body,
            _ => return Err(SourceError::Invalid),
        };
        let (i, c) = read_source_in(tx, kind, source, work, &field).await?;
        let version = if *kind == SourceKind::Comment {
            comment_document_version(&i.source_version, &c)
        } else {
            i.source_version.clone()
        };
        let parent = i.work_ref;
        let mut fields = vec![i];
        if *kind == SourceKind::OpsPublication {
            match read_source_in(tx, kind, source, None, &SourceField::Title).await {
                Ok((i, _)) => fields.insert(0, i),
                Err(SourceError::Unavailable) => {}
                Err(e) => return Err(e),
            }
        }
        (fields, c, version, parent)
    };
    if fragments.is_empty() {
        return Err(SourceError::Unavailable);
    }
    fragments.sort_by_key(|f| match f.field {
        SourceField::Title => 0,
        SourceField::Body => 1,
        SourceField::CoverHeadline => 2,
        _ => 3,
    });
    let first = &fragments[0];
    Ok(MaterialDocument {
        document_id: document_id(kind, source),
        source_kind: kind.clone(),
        source_ref: source,
        work_ref: parent,
        source_version: version,
        title: first.title.clone(),
        author_label: first.author_label.clone(),
        qualification: "readable".into(),
        fragments,
        context,
        match_reasons: vec![],
    })
}

// Reading context has its own version. Exact comment anchors retain the underlying SourceItem version.
fn comment_document_version(comment_version: &str, context: &Value) -> String {
    let parent_work = &context["parentWork"];
    let parent_comment = &context["parentComment"];
    let body_hash = parent_comment["body"].as_str().map(text_hash);
    text_hash(
        &json!([
            "comment-reading-context.v1",
            comment_version,
            parent_work["state"],
            parent_work["workRef"],
            parent_work["materialRevision"],
            parent_comment["state"],
            parent_comment["sourceRef"],
            parent_comment["bodyState"],
            body_hash
        ])
        .to_string(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_context_changes_reading_version_without_mutating_exact_anchor_version() {
        let anchor_version = "a".repeat(64);
        let context = json!({"parentWork":{"state":"known","workRef":Uuid::new_v4(),"materialRevision":"parent-v1"},"parentComment":{"state":"known","sourceRef":Uuid::new_v4(),"bodyState":"KNOWN","body":"父评论😀\n第一步"}});
        let before = comment_document_version(&anchor_version, &context);
        assert_eq!(before, comment_document_version(&anchor_version, &context));
        let mut changed = context.clone();
        changed["parentWork"]["materialRevision"] = json!("parent-v2");
        assert_ne!(before, comment_document_version(&anchor_version, &changed));
        let mut changed = context.clone();
        changed["parentComment"]["body"] = json!("父评论新版本");
        assert_ne!(before, comment_document_version(&anchor_version, &changed));
        let mut changed = context;
        changed["parentComment"] = json!({"state":"restricted"});
        assert_ne!(before, comment_document_version(&anchor_version, &changed));
        assert_eq!(anchor_version, "a".repeat(64));
    }
}
