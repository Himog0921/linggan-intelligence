//! Complete, shared parent/work context reads. No caller substitutes a newer context for a Run.
use super::StudySourceError;
use crate::comment_cleaning::{CLEANER_VERSION, clean};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) async fn read_titles(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Value>, sqlx::Error> {
    if works.is_empty() {
        return Ok(BTreeMap::new());
    }
    let dossiers = linggan_evidence::research_dossier::read_in(tx, works, as_of).await?;
    Ok(dossiers.into_iter().map(|(work,d)| (work,json!({"displayTitle":d.note["displayTitle"],"displayTitleSource":d.note["displayTitleBasis"]}))).collect())
}

pub(crate) async fn read_work_contexts(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    works: &[Uuid],
    as_of: &str,
) -> Result<BTreeMap<Uuid, Value>, StudySourceError> {
    if works.is_empty() {
        return Ok(BTreeMap::new());
    }
    let allowed: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT content_public_ref) FROM linggan_material_domain_usage \
         WHERE domain_ref=$1 AND content_public_ref=ANY($2::uuid[])",
    )
    .bind(domain)
    .bind(works)
    .fetch_one(&mut **tx)
    .await?;
    let unique: std::collections::BTreeSet<_> = works.iter().collect();
    if allowed as usize != unique.len() {
        return Err(StudySourceError::InvalidDomain);
    }
    let dossiers = linggan_evidence::research_dossier::read_in(tx, works, as_of).await?;
    Ok(dossiers
        .into_iter()
        .map(|(work, dossier)| {
            let full = dossier_manifest(&dossier);
            (
                work,
                json!({"workRef":work,"displayTitle":dossier.note["displayTitle"],
            "displayTitleSource":dossier.note["displayTitleBasis"],
            "contextManifest":full}),
            )
        })
        .collect())
}

pub(crate) fn dossier_manifest(
    dossier: &linggan_evidence::research_dossier::ResearchDossier,
) -> Value {
    full_context_manifest(&json!({
        "contract":"comment-study.context.v2","workRef":dossier.work_ref,
        "readingTitle":linggan_evidence::research_dossier::reading_title(dossier),
        "dossierRevision":dossier.material_revision,"dossierCompleteRevision":dossier.revision,
        "dossierContract":dossier.contract,"coverage":dossier.coverage,
        "cleanerVersion":CLEANER_VERSION,
        "sources":linggan_evidence::research_dossier::context_sources(dossier)
    }))
}

pub(crate) async fn read_parents(
    tx: &mut Transaction<'_, Postgres>,
    keys: &[(Uuid, String)],
    as_of: &str,
) -> Result<BTreeMap<(Uuid, String), Value>, sqlx::Error> {
    if keys.is_empty() {
        return Ok(BTreeMap::new());
    }
    let works: Vec<_> = keys.iter().map(|key| key.0).collect();
    let ids: Vec<_> = keys.iter().map(|key| key.1.as_str()).collect();
    let rows = sqlx::query(include_str!("parents.sql"))
        .bind(&works)
        .bind(&ids)
        .bind(as_of)
        .fetch_all(&mut **tx)
        .await?;
    rows.into_iter()
        .map(|row| {
            let work: Uuid = row.try_get("work_ref")?;
            let id: String = row.try_get("external_id")?;
            let raw: Option<String> = row.try_get("raw_prefix")?;
            let cleaned = raw.as_deref().map(clean);
            let visible = cleaned
                .as_ref()
                .is_some_and(|c| matches!(c.state.as_str(), "direct" | "context"));
            let value = json!({"commentKey":{"workRef":work,"commentExternalId":id},
            "sourceRef":row.try_get::<Option<Uuid>,_>("source_ref")?,
            "sourceState":row.try_get::<String,_>("source_state")?,
            "commentText":raw.as_deref().filter(|_|visible),
            "researchText":cleaned.as_ref().filter(|_|visible).map(|c|c.text.as_str()),
            "cleanState":cleaned.as_ref().map(|c|c.state.as_str()),
            "cleanReasons":cleaned.as_ref().map(|c|&c.reasons),"contextOnly":true});
            Ok(((work, id), value))
        })
        .collect()
}

pub fn full_context_manifest(manifest: &Value) -> Value {
    let sources = manifest["sources"].as_array().cloned().unwrap_or_default();
    let count: usize = sources
        .iter()
        .map(|source| {
            source["text"]
                .as_str()
                .map_or(0, |text| text.chars().count())
        })
        .sum();
    json!({"contract":"comment-study.context.v2","workRef":manifest["workRef"],
        "cleanerVersion":manifest["cleanerVersion"],"dossierRevision":manifest["dossierRevision"],
        "dossierCompleteRevision":manifest["dossierCompleteRevision"],"coverage":manifest["coverage"],
        "dossierContract":manifest["dossierContract"],"includedCharacterCount":count,
        "readingTitle":manifest["readingTitle"],"orderBasis":"dossier_canonical_block_order","sources":sources})
}
