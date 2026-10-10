//! Frozen scalar ranges are restored from current-qualified canonical sources.
use super::{TopicMapError, TopicMapSavedFragment, TopicMapSavedRange};
use crate::comment_study_source::{
    StudySourceRole, eligible_sources_for_topic_research_in_transaction,
};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeSet, HashMap, HashSet};
use uuid::Uuid;

pub(super) fn is_comment_field(field: &str) -> bool {
    matches!(
        field,
        "studied_comment" | "unresearched_comment" | "parent_comment_context"
    )
}

pub(super) fn scalar_range(manifest: &Value) -> Option<(usize, usize)> {
    let start = usize::try_from(manifest["start"].as_u64()?).ok()?;
    let end = usize::try_from(manifest["end"].as_u64()?).ok()?;
    (start < end).then_some((start, end))
}

pub(super) fn frozen_slice(text: &str, manifest: &Value) -> Option<String> {
    let (start, end) = scalar_range(manifest)?;
    let slice: String = text.chars().skip(start).take(end - start).collect();
    (slice.chars().count() == end - start
        && manifest["textHash"].as_str()? == linggan_evidence::creator_discovery::hash(&slice))
    .then_some(slice)
}

pub(super) fn cited_ranges(manifest: &Value, citations: &Value) -> Vec<TopicMapSavedRange> {
    let Some((fragment_start, fragment_end)) = scalar_range(manifest) else {
        return Vec::new();
    };
    citations
        .as_array()
        .into_iter()
        .flatten()
        .filter(|citation| citation["fragmentId"] == manifest["fragmentId"])
        .filter_map(|citation| {
            let (start, end) = scalar_range(citation)?;
            (fragment_start <= start && end <= fragment_end)
                .then_some(TopicMapSavedRange { start, end })
        })
        .collect()
}

pub(super) async fn restore_saved_fragments(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    manifests: &[Value],
    research: &Value,
) -> Result<Option<Vec<TopicMapSavedFragment>>, TopicMapError> {
    let comments = qualified_comment_texts(tx, domain, manifests).await?;
    let mut native = HashMap::<Uuid, Vec<(Uuid, String)>>::new();
    for manifest in manifests {
        let Some(work) = reference(manifest, "workRef") else {
            return Ok(None);
        };
        let Some(source) = reference(manifest, "sourceRef") else {
            return Ok(None);
        };
        let Some(field) = manifest["field"].as_str() else {
            return Ok(None);
        };
        if !is_comment_field(field) {
            native.entry(work).or_default().push((source, field.into()));
        }
    }
    let mut texts = HashMap::new();
    for (work, sources) in native {
        let restored = linggan_evidence::read_frozen_work_texts_in_transaction(tx, work, &sources)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
        for ((source, field), values) in restored {
            texts.insert((work, source, field), values);
        }
    }
    let mut fragments = Vec::new();
    for manifest in manifests {
        let (Some(work), Some(source), Some(field), Some((start, end)), Some(id)) = (
            reference(manifest, "workRef"),
            reference(manifest, "sourceRef"),
            manifest["field"].as_str(),
            scalar_range(manifest),
            manifest["fragmentId"].as_str(),
        ) else {
            return Ok(None);
        };
        let context_only = field == "parent_comment_context";
        let text = if is_comment_field(field) {
            comments
                .get(&(work, source, context_only))
                .and_then(|text| frozen_slice(text, manifest))
        } else {
            texts
                .get(&(work, source, field.into()))
                .into_iter()
                .flatten()
                .find_map(|text| frozen_slice(text, manifest))
        };
        let Some(text) = text else {
            return Ok(None);
        };
        fragments.push(TopicMapSavedFragment {
            work_ref: work,
            source_ref: source,
            fragment_id: id.into(),
            field: field.into(),
            source_version: manifest["sourceVersion"]
                .as_str()
                .unwrap_or("immutable-material")
                .into(),
            start,
            end,
            text,
            cited_ranges: cited_ranges(manifest, &research["citations"]),
            context_only,
        });
    }
    Ok(Some(fragments))
}

fn reference(manifest: &Value, key: &str) -> Option<Uuid> {
    manifest[key].as_str()?.parse().ok()
}

struct CommentRecord {
    work: Uuid,
    id: String,
    parent: Option<String>,
    text: Option<String>,
}

async fn comment_records(
    tx: &mut Transaction<'_, Postgres>,
    refs: &[Uuid],
) -> Result<HashMap<Uuid, CommentRecord>, sqlx::Error> {
    let mut records = HashMap::new();
    for refs in refs.chunks(500) {
        let rows = sqlx::query("SELECT material_ref,content_public_ref,comment_external_id,parent_comment_external_id,CASE WHEN body_state='KNOWN' THEN body_text ELSE NULL END AS text FROM linggan_material_comment WHERE material_ref=ANY($1)")
            .bind(refs).fetch_all(&mut **tx).await?;
        for row in rows {
            let raw: Option<String> = row.get("text");
            let text = raw
                .map(|raw| crate::comment_cleaning::clean(&raw))
                .filter(|clean| matches!(clean.state.as_str(), "direct" | "context"))
                .map(|clean| clean.text);
            records.insert(
                row.get("material_ref"),
                CommentRecord {
                    work: row.get("content_public_ref"),
                    id: row.get("comment_external_id"),
                    parent: row.get("parent_comment_external_id"),
                    text,
                },
            );
        }
    }
    Ok(records)
}

async fn qualified_comment_texts(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    manifests: &[Value],
) -> Result<HashMap<(Uuid, Uuid, bool), String>, TopicMapError> {
    let comment_refs: Vec<_> = manifests
        .iter()
        .filter(|m| m["field"].as_str().is_some_and(is_comment_field))
        .collect();
    let works: Vec<_> = comment_refs
        .iter()
        .filter_map(|m| reference(m, "workRef"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if works.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query("SELECT content_public_ref,bool_or(role='primary') AS primary_role FROM linggan_material_domain_usage WHERE domain_ref=$1 AND content_public_ref=ANY($2) AND role IN('primary','reference') GROUP BY content_public_ref")
        .bind(domain).bind(&works).fetch_all(&mut **tx).await?;
    let selections: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.get("content_public_ref"),
                if r.get::<bool, _>("primary_role") {
                    StudySourceRole::Primary
                } else {
                    StudySourceRole::Reference
                },
            )
        })
        .collect();
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let qualified =
        eligible_sources_for_topic_research_in_transaction(tx, domain, &now, &selections)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
    let refs: Vec<_> = qualified
        .iter()
        .flat_map(|s| [Some(s.source_ref), s.parent_source_ref])
        .flatten()
        .chain(
            comment_refs
                .iter()
                .filter_map(|m| reference(m, "sourceRef")),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let records = comment_records(tx, &refs).await?;
    let mut eligible = HashSet::new();
    let mut links = HashSet::new();
    for source in qualified {
        if let Some(child) = records.get(&source.source_ref) {
            eligible.insert((child.work, child.id.clone()));
            if source.parent_research_text.is_some()
                && let Some(parent) = source.parent_source_ref.and_then(|r| records.get(&r))
            {
                links.insert((child.work, child.id.clone(), parent.id.clone()));
            }
        }
    }
    Ok(select_comment_texts(
        &comment_refs,
        &records,
        &eligible,
        &links,
    ))
}

fn select_comment_texts(
    manifests: &[&Value],
    records: &HashMap<Uuid, CommentRecord>,
    eligible: &HashSet<(Uuid, String)>,
    links: &HashSet<(Uuid, String, String)>,
) -> HashMap<(Uuid, Uuid, bool), String> {
    let mut texts = HashMap::new();
    let mut parents = HashSet::new();
    for manifest in manifests
        .iter()
        .filter(|m| m["field"] != "parent_comment_context")
    {
        let Some(source) = reference(manifest, "sourceRef") else {
            continue;
        };
        let Some(record) = records
            .get(&source)
            .filter(|r| Some(r.work) == reference(manifest, "workRef"))
        else {
            continue;
        };
        if !eligible.contains(&(record.work, record.id.clone())) {
            continue;
        }
        if let Some(text) = &record.text {
            texts.insert((record.work, source, false), text.clone());
        }
        if let Some(parent) = &record.parent
            && links.contains(&(record.work, record.id.clone(), parent.clone()))
        {
            parents.insert((record.work, parent.clone()));
        }
    }
    for manifest in manifests
        .iter()
        .filter(|m| m["field"] == "parent_comment_context")
    {
        let Some(source) = reference(manifest, "sourceRef") else {
            continue;
        };
        let Some(record) = records
            .get(&source)
            .filter(|r| Some(r.work) == reference(manifest, "workRef"))
        else {
            continue;
        };
        if parents.contains(&(record.work, record.id.clone()))
            && let Some(text) = &record.text
        {
            texts.insert((record.work, source, true), text.clone());
        }
    }
    texts
}
