//! Bounded, shared parent/work context reads. No caller substitutes a newer context for a Run.
use super::{ADHD_DOMAIN_REF, StudySourceError};
use crate::comment_cleaning::{CLEANER_VERSION, clean};
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) async fn read_titles(
    tx: &mut Transaction<'_, Postgres>, works: &[Uuid], as_of: &str,
) -> Result<BTreeMap<Uuid, Value>, sqlx::Error> {
    if works.is_empty() { return Ok(BTreeMap::new()); }
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('linggan_media_ocr_layout') IS NOT NULL AND to_regclass('linggan_media_ocr_layering_result') IS NOT NULL AND to_regclass('linggan_media_ocr_retirement') IS NOT NULL")
        .fetch_one(&mut **tx).await?;
    let ctes = linggan_evidence::work_display_title_ctes(ready)
        .map_err(|_| sqlx::Error::Protocol("shared work title projection unavailable".into()))?;
    let sql = format!("WITH cs_title_scope AS (SELECT unnest($1::uuid[]) AS work_ref), {ctes} SELECT * FROM cs_display_titles");
    let rows = sqlx::query(AssertSqlSafe(sql)).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    rows.into_iter().map(|row| Ok((row.try_get("work_ref")?, json!({
        "displayTitle":row.try_get::<Option<String>,_>("display_title")?,
        "displayTitleSource":row.try_get::<String,_>("display_title_source")?
    })))).collect()
}

pub(crate) async fn read_work_contexts(
    tx: &mut Transaction<'_, Postgres>, domain: Uuid, works: &[Uuid], as_of: &str,
) -> Result<BTreeMap<Uuid, Value>, StudySourceError> {
    if domain.to_string()!=ADHD_DOMAIN_REF { return Err(StudySourceError::InvalidDomain); }
    if works.is_empty() { return Ok(BTreeMap::new()); }
    let allowed: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_content WHERE domain_ref=$1 AND public_ref=ANY($2::uuid[])")
        .bind(domain).bind(works).fetch_one(&mut **tx).await?;
    let unique: std::collections::BTreeSet<_> = works.iter().collect();
    if allowed as usize!=unique.len() { return Err(StudySourceError::InvalidDomain); }
    let titles = read_titles(tx,works,as_of).await?;
    let rows = sqlx::query(include_str!("context.sql")).bind(works).bind(as_of).fetch_all(&mut **tx).await?;
    rows.into_iter().map(|row| {
        let work: Uuid = row.try_get("work_ref")?;
        let fragments: Value = row.try_get("fragments")?;
        let manifest=json!({"contract":"comment-study.context.v1","workRef":work,
            "cleanerVersion":CLEANER_VERSION,"sources":fragments});
        let bounded=crate::comment_study_run::bounded_context_manifest(&manifest,20000);
        Ok((work,json!({"workRef":work,"displayTitle":titles.get(&work).and_then(|t|t.get("displayTitle")),
            "displayTitleSource":titles.get(&work).and_then(|t|t.get("displayTitleSource")),
            "contextManifest":bounded})))
    }).collect()
}

pub(crate) async fn read_parents(
    tx: &mut Transaction<'_, Postgres>, keys: &[(Uuid,String)], as_of: &str,
) -> Result<BTreeMap<(Uuid,String), Value>, sqlx::Error> {
    if keys.is_empty() { return Ok(BTreeMap::new()); }
    let works: Vec<_>=keys.iter().map(|key|key.0).collect();
    let ids: Vec<_>=keys.iter().map(|key|key.1.as_str()).collect();
    let rows=sqlx::query(include_str!("parents.sql")).bind(&works).bind(&ids).bind(as_of)
        .fetch_all(&mut **tx).await?;
    rows.into_iter().map(|row| {
        let work: Uuid=row.try_get("work_ref")?;
        let id: String=row.try_get("external_id")?;
        let raw: Option<String>=row.try_get("raw_prefix")?;
        let cleaned=raw.as_deref().map(clean);
        let visible=cleaned.as_ref().is_some_and(|c|matches!(c.state.as_str(),"direct"|"context"));
        let value=json!({"commentKey":{"workRef":work,"commentExternalId":id},
            "sourceRef":row.try_get::<Option<Uuid>,_>("source_ref")?,
            "sourceState":row.try_get::<String,_>("source_state")?,
            "commentText":raw.as_deref().filter(|_|visible),
            "researchText":cleaned.as_ref().filter(|_|visible).map(|c|c.text.as_str()),
            "cleanState":cleaned.as_ref().map(|c|c.state.as_str()),
            "cleanReasons":cleaned.as_ref().map(|c|&c.reasons),"contextOnly":true});
        Ok(((work,id),value))
    }).collect()
}
