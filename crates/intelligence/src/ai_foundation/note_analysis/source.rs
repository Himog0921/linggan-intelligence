use super::*;
use crate::note_analysis::{Packet, Scope, Source};
use linggan_evidence::research_dossier;
/// Shared dossier owns content eligibility. This owner only selects configured text columns.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    scope: Scope,
    cutoff: &str,
    columns: &[String],
) -> Result<Packet, FoundationError> {
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=$2 AND created_at<=$3::timestamptz)").bind(domain).bind(work).bind(cutoff).fetch_one(&mut **tx).await?;
    if !allowed {
        return Err(FoundationError::NotFound);
    }
    let d = research_dossier::read_in(tx, &[work], cutoff)
        .await?
        .remove(&work)
        .ok_or(FoundationError::NotFound)?;
    let mut sources = Vec::new();
    for f in &d.fragments {
        let field = match f.field.as_str() {
            "native_title" => "title",
            "image_substantive_text" => "ocr",
            "asr_text" => "transcript",
            "frame_ocr_text" => "frame_text",
            v => v,
        };
        if !columns.iter().any(|x| x == field) || f.text.trim().is_empty() {
            continue;
        }
        sources.push(Source {
            citation_id: f.fragment_id.clone(),
            kind: "material".into(),
            source_ref: f.source_ref,
            source_version: f.source_version.clone(),
            start: f.start,
            text: f.text.clone(),
            speaker: f.speaker.clone(),
            parent_citation_id: None,
        });
    }
    let material_snapshot_identity = crate::research_text::content_hash(
        &serde_json::to_string(&sources).map_err(|_| FoundationError::Invalid)?,
    );
    let mut authors = 0;
    let mut unknown = 0;
    if scope == Scope::NoteWithComments {
        // The selected historical latest version includes author replies; restrictions apply now.
        let rows=sqlx::query("WITH selected AS (SELECT DISTINCT ON(c.comment_external_id)c.* FROM linggan_material_comment c JOIN linggan_runtime_capture_package p USING(package_ref) WHERE c.content_public_ref=$1 AND c.created_at<=$2::timestamptz AND p.accepted_at<=$2::timestamptz ORDER BY c.comment_external_id,c.observed_at::timestamptz DESC,c.created_at DESC,c.material_ref DESC) SELECT c.material_ref,c.comment_external_id,c.parent_comment_external_id,c.body_text,c.author_external_id,a.author_external_id AS work_author FROM selected c LEFT JOIN LATERAL linggan_material_content_author_at(c.content_public_ref,$2::timestamptz) a ON true WHERE c.body_state='KNOWN' AND c.body_text IS NOT NULL AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id) ORDER BY c.comment_external_id")
   .bind(work).bind(cutoff).fetch_all(&mut **tx).await?;
        let ids: std::collections::BTreeMap<String, String> = rows
            .iter()
            .map(|r| {
                (
                    r.get("comment_external_id"),
                    format!("comment:{}", r.get::<Uuid, _>("material_ref")),
                )
            })
            .collect();
        for r in rows {
            let body: String = r.get("body_text");
            if body.trim().is_empty() {
                continue;
            }
            let who: Option<String> = r.get("author_external_id");
            let owner: Option<String> = r.get("work_author");
            let speaker = match (who, owner) {
                (Some(a), Some(b)) if a == b => {
                    authors += 1;
                    "author"
                }
                (Some(_), Some(_)) => "other",
                _ => {
                    unknown += 1;
                    "unconfirmed"
                }
            };
            let id: Uuid = r.get("material_ref");
            let parent: Option<String> = r.get("parent_comment_external_id");
            sources.push(Source {
                citation_id: format!("comment:{id}"),
                kind: "comment".into(),
                source_ref: id,
                source_version: crate::research_text::content_hash(&body),
                start: 0,
                text: body,
                speaker: speaker.into(),
                parent_citation_id: parent.and_then(|p| ids.get(&p).cloned()),
            });
        }
    }
    let count = sources.iter().filter(|s| s.kind == "comment").count();
    Ok(Packet {
        domain_ref: domain,
        work_ref: work,
        scope,
        as_of: cutoff.into(),
        material_snapshot_identity,
        coverage: json!({"materialColumns":columns,"includedCommentCount":count,"authorCommentCount":authors,"unknownAuthorCommentCount":unknown,"platformCommentCount":d.metadata["interaction"]["comments"]["value"],"issues":d.coverage["issues"].as_array().into_iter().flatten().filter_map(|v|v.as_str().or_else(||v["message"].as_str()).or_else(||v["code"].as_str())).collect::<Vec<_>>(),"dossierCoverage":d.coverage}),
        sources,
    })
}
pub(super) async fn columns(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
) -> Result<Vec<String>, FoundationError> {
    let v: Value = sqlx::query_scalar(
        "SELECT material_columns FROM linggan_ai_task_config WHERE task_config_ref=$1",
    )
    .bind(config)
    .fetch_one(&mut **tx)
    .await?;
    serde_json::from_value(v).map_err(|_| FoundationError::Invalid)
}
pub(crate) async fn frozen_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Packet, FoundationError> {
    let r=sqlx::query("SELECT input_manifest,input_hash,task_config_ref FROM linggan_note_analysis_request WHERE request_ref=$1").bind(request).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let saved: Packet =
        serde_json::from_value(r.get("input_manifest")).map_err(|_| FoundationError::Invalid)?;
    let cols = columns(tx, r.get("task_config_ref")).await?;
    let current = load(
        tx,
        saved.domain_ref,
        saved.work_ref,
        saved.scope.clone(),
        &saved.as_of,
        &cols,
    )
    .await?;
    if current.fingerprint() != saved.fingerprint()
        || saved.hash() != r.get::<String, _>("input_hash")
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(saved)
}
