//! Saved decisions resolve their immutable source versions through current permission gates.
use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::collections::HashSet;
#[path = "saved/definitions.rs"]
mod definitions;
#[path = "saved/sources.rs"]
mod sources;
pub(crate) use definitions::available as definitions_available;
#[cfg(test)]
#[path = "saved/tests.rs"]
mod tests;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapSavedRange {
    pub start: usize,
    pub end: usize,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapSavedFragment {
    pub work_ref: Uuid,
    pub fragment_id: String,
    pub source_ref: Uuid,
    pub field: String,
    pub source_version: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub cited_ranges: Vec<TopicMapSavedRange>,
    pub context_only: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicMapSavedAlternative {
    pub alternative_ref: Uuid,
    pub domain_ref: Uuid,
    pub topic_ref: Uuid,
    pub definition_ref: Uuid,
    pub kind: String,
    pub source_state: String,
    pub title: Option<String>,
    pub angle: Option<String>,
    pub rationale: Option<String>,
    pub method_version: String,
    pub original_definition: Option<Value>,
    pub research_manifest: Value,
    pub source_manifests: Vec<Value>,
    pub fragments: Vec<TopicMapSavedFragment>,
}

pub async fn read_saved_alternative(
    db: &Database,
    domain: Uuid,
    alternative: Uuid,
) -> Result<TopicMapSavedAlternative, TopicMapError> {
    // The shared definition-source reader acquires pooled canonical inputs; run
    // it before this transaction so a one-connection pool remains supported.
    let unavailable = crate::topic_map_core::unavailable_definitions(db, Some(domain))
        .await
        .map_err(|e| TopicMapError::Source(e.to_string()))?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT * FROM linggan_topic_map_alternative WHERE alternative_ref=$1 AND domain_ref=$2",
    )
    .bind(alternative)
    .bind(domain)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(TopicMapError::NotFound)?;
    let research_manifest: Value = row.get("research_manifest");
    let source_manifests:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('workRef',work_public_ref,'sourceManifest',source_manifest) FROM linggan_topic_map_alternative_evidence WHERE alternative_ref=$1 ORDER BY work_public_ref").bind(alternative).fetch_all(&mut *tx).await?;
    let (manifests, refs) = source_manifests_for_read(&research_manifest, &source_manifests);
    let domain_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active')",
    )
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    let mut available = domain_active && !manifests.is_empty() && refs.len() <= 10;
    available &= definitions::available(
        &mut tx,
        domain,
        row.get("definition_ref"),
        &research_manifest,
        &unavailable,
    )
    .await?;
    let mut version_changed = false;
    for work in &refs {
        let (qualified, changed) = qualify_work(&mut tx, domain, *work, &source_manifests).await?;
        available &= qualified;
        version_changed |= changed;
    }
    let fragments = if available {
        sources::restore_saved_fragments(&mut tx, domain, &manifests, &research_manifest).await?
    } else {
        None
    };
    available &= fragments.is_some();
    let fragments = fragments.unwrap_or_default();
    let definition_ref: Uuid = row.get("definition_ref");
    let original_definition: Option<Value> = if available {
        sqlx::query_scalar("SELECT jsonb_build_object('definitionRef',definition_ref,'version',version,'displayName',display_name,'definitionText',definition_text) FROM linggan_topic_definition WHERE definition_ref=$1").bind(definition_ref).fetch_optional(&mut *tx).await?
    } else {
        None
    };
    let result = TopicMapSavedAlternative {
        alternative_ref: alternative,
        domain_ref: domain,
        topic_ref: row.get("topic_ref"),
        definition_ref,
        kind: row.get("kind"),
        source_state: if !available {
            "source_unavailable"
        } else if version_changed {
            "version_changed_available"
        } else {
            "available"
        }
        .into(),
        title: available.then(|| row.get("title")),
        angle: available.then(|| row.get("angle")),
        rationale: available.then(|| row.get("rationale")),
        method_version: row.get("method_version"),
        original_definition,
        research_manifest: safe_research_manifest(&research_manifest),
        source_manifests,
        fragments,
    };
    tx.commit().await?;
    Ok(result)
}

fn source_manifests_for_read(research: &Value, saved: &[Value]) -> (Vec<Value>, HashSet<Uuid>) {
    let input = research["inputRefs"]
        .get("source")
        .unwrap_or(&research["inputRefs"]);
    let mut manifests: Vec<Value> = input["fragments"].as_array().cloned().unwrap_or_else(|| {
        saved
            .iter()
            .flat_map(|m| {
                m["sourceManifest"]["frozenFragments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned()
            })
            .collect()
    });
    let mut refs: HashSet<Uuid> = saved
        .iter()
        .filter_map(|m| m["workRef"].as_str()?.parse().ok())
        .collect();
    for manifest in &mut manifests {
        let work = manifest["workRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .or_else(|| {
                manifest["fragmentId"]
                    .as_str()?
                    .split('.')
                    .next()?
                    .parse()
                    .ok()
            });
        if let Some(work) = work {
            manifest["workRef"] = json!(work);
            refs.insert(work);
        }
    }
    (manifests, refs)
}

async fn qualify_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    work: Uuid,
    saved: &[Value],
) -> Result<(bool, bool), TopicMapError> {
    let scoped: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=$2)")
        .bind(domain).bind(work).fetch_one(&mut **tx).await?;
    if !scoped {
        return Ok((false, false));
    }
    let current = linggan_evidence::read_work_resource_in_transaction(tx, work)
        .await
        .map_err(|e| TopicMapError::Source(e.to_string()))?;
    let Some(current) = current else {
        return Ok((false, false));
    };
    if current.summary.restriction_state == "WITHDRAWN_OR_RESTRICTED" {
        return Ok((false, false));
    }
    let changed = saved
        .iter()
        .find(|m| m["workRef"] == json!(work))
        .is_some_and(|saved| {
            let mut manifest = saved["sourceManifest"].clone();
            if let Some(object) = manifest.as_object_mut() {
                object.remove("frozenFragments");
            }
            manifest != canonical_source_manifest(&current)
        });
    Ok((true, changed))
}
