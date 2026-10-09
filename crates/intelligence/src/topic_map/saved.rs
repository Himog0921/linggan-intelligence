//! Saved decisions resolve their immutable source versions through current permission gates.
use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
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
    let mut manifests: Vec<Value> = if research_manifest["inputRefs"]["fragments"].is_array() {
        research_manifest["inputRefs"]["fragments"]
            .as_array()
            .unwrap()
            .clone()
    } else {
        source_manifests
            .iter()
            .flat_map(|m| {
                m["sourceManifest"]["frozenFragments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .cloned()
            })
            .collect()
    };
    let mut refs: HashSet<Uuid> = source_manifests
        .iter()
        .filter_map(|m| m["workRef"].as_str().and_then(|s| s.parse().ok()))
        .collect();
    for m in &mut manifests {
        let work = m["workRef"]
            .as_str()
            .and_then(|s| s.parse::<Uuid>().ok())
            .or_else(|| {
                m["fragmentId"]
                    .as_str()
                    .and_then(|s| s.get(..36))
                    .and_then(|s| s.parse().ok())
            });
        if let Some(work) = work {
            m["workRef"] = json!(work);
            refs.insert(work);
        }
    }
    let domain_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active')",
    )
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    let mut available = domain_active && !manifests.is_empty() && refs.len() <= 10;
    let mut version_changed = false;
    let mut comments = HashMap::new();
    for work in &refs {
        let scoped:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=$2)").bind(domain).bind(work).fetch_one(&mut *tx).await?;
        if !scoped {
            available = false;
            continue;
        }
        let current = linggan_evidence::read_work_resource_in_transaction(&mut tx, *work)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
        let Some(current) = current else {
            available = false;
            continue;
        };
        if current.summary.restriction_state == "WITHDRAWN_OR_RESTRICTED" {
            available = false;
            continue;
        }
        if let Some(saved) = source_manifests
            .iter()
            .find(|m| m["workRef"].as_str() == Some(&work.to_string()))
        {
            let mut saved_manifest = saved["sourceManifest"].clone();
            if let Some(object) = saved_manifest.as_object_mut() {
                object.remove("frozenFragments");
            }
            version_changed |= saved_manifest != canonical_source_manifest(&current);
        }
        if manifests.iter().any(|m| {
            m["workRef"].as_str() == Some(&work.to_string())
                && m["field"].as_str().is_some_and(|s| s.ends_with("comment"))
        }) {
            let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
                .fetch_one(&mut *tx)
                .await?;
            let qualified = crate::comment_study_source::eligible_sources_in_transaction(
                &mut tx,
                domain,
                &now,
                &[*work],
                30,
            )
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
            let source_refs: Vec<Uuid> = qualified.iter().map(|s| s.source_ref).collect();
            let ids:Vec<String>=sqlx::query_scalar("SELECT comment_external_id FROM linggan_material_comment WHERE material_ref=ANY($1)").bind(&source_refs).fetch_all(&mut *tx).await?;
            comments.insert(*work, ids.into_iter().collect::<HashSet<_>>());
        }
    }
    let mut fragments = Vec::new();
    if available {
        for m in &manifests {
            let parse = |key: &str| m[key].as_str().and_then(|s| s.parse::<Uuid>().ok());
            let (Some(work), Some(source)) = (parse("workRef"), parse("sourceRef")) else {
                available = false;
                break;
            };
            let Some(field) = m["field"].as_str() else {
                available = false;
                break;
            };
            let end = m["end"].as_u64().unwrap_or(0) as usize;
            let start = m["start"].as_u64().unwrap_or(0) as usize;
            if end > 10000 || start > end {
                available = false;
                break;
            }
            let candidates = if field.ends_with("comment") {
                let source_row=sqlx::query("SELECT comment_external_id,body_text FROM linggan_material_comment WHERE material_ref=$1 AND content_public_ref=$2").bind(source).bind(work).fetch_optional(&mut *tx).await?;
                match source_row {
                    Some(r)
                        if comments.get(&work).is_some_and(|ids| {
                            ids.contains(&r.get::<String, _>("comment_external_id"))
                        }) =>
                    {
                        r.get::<Option<String>, _>("body_text")
                            .map(|raw| crate::comment_cleaning::clean(&raw))
                            .filter(|clean| matches!(clean.state.as_str(), "direct" | "context"))
                            .map(|clean| vec![clean.text])
                            .unwrap_or_default()
                    }
                    _ => Vec::new(),
                }
            } else {
                linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, source, field)
                    .await
                    .map_err(|e| TopicMapError::Source(e.to_string()))?
            };
            let text = candidates
                .into_iter()
                .filter_map(|text| {
                    let text: String = text.chars().skip(start).take(end - start).collect();
                    (text.chars().count() == end - start
                        && m["textHash"].as_str()
                            == Some(&linggan_evidence::creator_discovery::hash(&text)))
                    .then_some(text)
                })
                .next();
            let Some(text) = text else {
                available = false;
                break;
            };
            let id = m["fragmentId"].as_str().unwrap_or("");
            let ranges: Vec<_> = research_manifest["citations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["fragmentId"].as_str() == Some(id))
                .filter_map(|c| {
                    let start = c["start"].as_u64()? as usize;
                    let end = c["end"].as_u64()? as usize;
                    (start < end && end <= text.chars().count())
                        .then_some(TopicMapSavedRange { start, end })
                })
                .collect();
            fragments.push(TopicMapSavedFragment {
                work_ref: work,
                source_ref: source,
                fragment_id: id.into(),
                field: field.into(),
                source_version: m["sourceVersion"]
                    .as_str()
                    .unwrap_or("immutable-material")
                    .into(),
                start,
                end,
                text,
                cited_ranges: ranges,
            });
        }
    }
    if !available {
        fragments.clear();
    }
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
