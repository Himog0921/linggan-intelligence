//! Same anonymous creation-comment contract as the single reader, with parent dossiers read once.
//! Callers freeze a read transaction; no context or qualification is cached across requests.
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub async fn read_creation_comments_in(
    tx: &mut Transaction<'_, Postgres>,
    sources: &[Uuid],
) -> Result<BTreeMap<Uuid, Value>, sqlx::Error> {
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    read_creation_comments_at_in(tx, sources, &as_of).await
}

pub async fn read_creation_comments_at_in(
    tx: &mut Transaction<'_, Postgres>,
    sources: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Value>, sqlx::Error> {
    if sources.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows = sqlx::query(
        "SELECT c.material_ref,c.content_public_ref,c.is_reply,c.body_text,c.body_state,c.observed_at,c.package_ref, \
         EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id) AS restricted, \
         parent.material_ref AS parent_ref,parent.body_text AS parent_body,parent.body_state AS parent_body_state, \
         EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=parent.content_public_ref AND r.comment_external_id=parent.comment_external_id) AS parent_restricted \
         FROM linggan_material_comment c JOIN linggan_runtime_capture_package package USING(package_ref) \
         LEFT JOIN LATERAL (SELECT p.* FROM linggan_material_comment p JOIN linggan_runtime_capture_package pp USING(package_ref) \
           WHERE c.is_reply AND p.content_public_ref=c.content_public_ref AND p.comment_external_id=c.parent_comment_external_id \
             AND p.created_at<=$2::timestamptz AND pp.accepted_at<=$2::timestamptz \
           ORDER BY p.observed_at::timestamptz DESC,p.created_at DESC,p.material_ref DESC LIMIT 1) parent ON true \
         WHERE c.material_ref=ANY($1) AND c.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz"
    ).bind(sources).bind(as_of).fetch_all(&mut **tx).await?;
    let works: Vec<Uuid> = rows
        .iter()
        .filter(|row| !row.get::<bool, _>("restricted"))
        .map(|row| row.get("content_public_ref"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let dossiers = crate::research_dossier::read_in(tx, &works, as_of).await?;
    let mut output = BTreeMap::new();
    for row in rows {
        let source: Uuid = row.get("material_ref");
        if row.get::<bool, _>("restricted") {
            output.insert(
                source,
                json!({"sourceRef":source,"qualification":"restricted"}),
            );
            continue;
        }
        let work: Uuid = row.get("content_public_ref");
        let parent = if !row.get::<bool, _>("is_reply") {
            json!({"state":"not_applicable"})
        } else if let Some(parent) = row.get::<Option<Uuid>, _>("parent_ref") {
            if row.get::<bool, _>("parent_restricted") {
                json!({"state":"restricted"})
            } else {
                json!({"state":"known","sourceRef":parent,"body":row.get::<Option<String>,_>("parent_body"),"bodyState":row.get::<String,_>("parent_body_state")})
            }
        } else {
            json!({"state":"unknown"})
        };
        let parent_work = if let Some(dossier) = dossiers.get(&work) {
            json!({"state":"known","workRef":work,"dossier":dossier})
        } else {
            json!({"state":"unavailable","workRef":work})
        };
        output.insert(source,json!({"sourceRef":source,"workRef":work,"body":row.get::<Option<String>,_>("body_text"),"bodyState":row.get::<String,_>("body_state"),"observedAt":row.get::<String,_>("observed_at"),"packageRef":row.get::<Uuid,_>("package_ref"),"qualification":"readable","parentWork":parent_work,"parentComment":parent,"anonymousAuthorContext":{"identity":"WITHHELD","platformUserIdReturned":false}}));
    }
    Ok(output)
}
