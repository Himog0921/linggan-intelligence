//! Whole document projection built from current qualified source readers.
use super::*;

pub(super) async fn note_document_in(
    _tx: &mut Transaction<'_, Postgres>,
    d: research_dossier::ResearchDossier,
) -> Result<MaterialDocument, SourceError> {
    let source = d.work_ref;
    let title = d.note["displayTitle"].as_str().unwrap_or("").to_owned();
    let author = d.metadata["author"]["displayName"].as_str().unwrap_or("作者未知").to_owned();
    let fields: Vec<_> = d.fragments.iter().filter(|f| fragment_kind(&f.field).is_some())
        .map(|f| dossier_item(&d,f)).collect::<Result<_,_>>()?;
    if fields.is_empty() {
        return Err(SourceError::Unavailable);
    }
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
        let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut **tx).await?;
        let (i, c) = read_source_in(tx, kind, source, work, &field, &source_block_ref(source, &field), &as_of).await?;
        let version = if *kind == SourceKind::Comment {
            comment_document_version(&i.source_version, &c)
        } else {
            i.source_version.clone()
        };
        let parent = i.work_ref;
        let mut fields = vec![i];
        if *kind == SourceKind::OpsPublication {
            match read_source_in(tx, kind, source, None, &SourceField::Title, &source_block_ref(source, &SourceField::Title), &as_of).await {
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

/// Only the comment read is batched; every member still passes the same domain and exact-source checks.
pub(super) async fn read_comment_documents_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    refs: &[Uuid],
) -> Result<std::collections::BTreeMap<Uuid, Result<MaterialDocument, SourceError>>, SourceError> {
    let contexts = linggan_evidence::read_creation_comments_in(tx, refs).await?;
    let mut output = std::collections::BTreeMap::new();
    for (source, context) in contexts {
        let result = async {
            if context["qualification"] == "restricted" {
                return Err(SourceError::Restricted);
            }
            let work: Uuid = serde_json::from_value(context["workRef"].clone())
                .map_err(|_| SourceError::Invalid)?;
            if !research_dossier::creation_work_in_domain(tx, work, domain).await? {
                return Err(SourceError::Unavailable);
            }
            let mut item = comment_item(source, None, &SourceField::CommentBody, &context)?;
            item.as_of = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut **tx).await?;
            Ok(MaterialDocument {
                document_id: document_id(&SourceKind::Comment, source),
                source_kind: SourceKind::Comment,
                source_ref: source,
                work_ref: item.work_ref,
                source_version: comment_document_version(&item.source_version, &context),
                title: item.title.clone(),
                author_label: item.author_label.clone(),
                qualification: "readable".into(),
                fragments: vec![item],
                context,
                match_reasons: vec![],
            })
        }
        .await;
        match result {
            Err(SourceError::Database(error)) => return Err(SourceError::Database(error)),
            result => {
                output.insert(source, result);
            }
        }
    }
    Ok(output)
}

/// Reuse only a successful read from this same frozen transaction. Exceptional sources use
/// the single reader so the existing error/qualification precedence is unchanged.
pub(super) async fn read_with_comments_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    selection: &MaterialSelection,
    comments: &std::collections::BTreeMap<Uuid, Result<MaterialDocument, SourceError>>,
) -> Result<MaterialDocument, SourceError> {
    if selection.source_kind == SourceKind::Comment {
        if let Some(Ok(document)) = comments.get(&selection.source_ref) {
            if selection
                .work_ref
                .is_some_and(|work| Some(work) != document.work_ref)
            {
                return Err(SourceError::Invalid);
            }
            return Ok(document.clone());
        }
    }
    read_document_in(
        tx,
        domain,
        &selection.source_kind,
        selection.source_ref,
        selection.work_ref,
    )
    .await
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
            parent_work["dossier"]["materialRevision"],
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
        let context = json!({"parentWork":{"state":"known","workRef":Uuid::new_v4(),"dossier":{"materialRevision":"parent-v1"}},"parentComment":{"state":"known","sourceRef":Uuid::new_v4(),"bodyState":"KNOWN","body":"父评论😀\n第一步"}});
        let before = comment_document_version(&anchor_version, &context);
        assert_eq!(before, comment_document_version(&anchor_version, &context));
        let mut changed = context.clone();
        changed["parentWork"]["dossier"]["materialRevision"] = json!("parent-v2");
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
