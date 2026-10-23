//! Frozen scalar ranges are restored from current-qualified canonical sources.
use super::{TopicMapError, TopicMapSavedFragment, TopicMapSavedRange};
use crate::comment_study_source::{
    StudySourceRole, eligible_sources_for_topic_research_in_transaction,
};
use linggan_evidence::FrozenWorkSource;
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
    let mut native = HashMap::<Uuid, Vec<FrozenWorkSource>>::new();
    for manifest in manifests {
        let Some(work) = reference(manifest, "workRef") else {
            return Ok(None);
        };
        let Some(_source) = reference(manifest, "sourceRef") else {
            return Ok(None);
        };
        let Some(field) = manifest["field"].as_str() else {
            return Ok(None);
        };
        if !is_comment_field(field) {
            let Some(frozen) = frozen_source(manifest) else {
                return Ok(None);
            };
            native.entry(work).or_default().push(frozen);
        }
    }
    let mut texts = HashMap::new();
    for (work, sources) in native {
        let restored = linggan_evidence::read_frozen_work_texts_in_transaction(tx, work, &sources)
            .await
            .map_err(|e| TopicMapError::Source(e.to_string()))?;
        for (source, values) in restored {
            texts.insert((work, source), values);
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
        let context_only = field == "parent_comment_context" || field.starts_with("work_context:");
        let text = if is_comment_field(field) {
            let Some(cutoff) = manifest["asOf"].as_str() else {
                return Ok(None);
            };
            comments
                .get(&(work, source, context_only, cutoff.to_owned()))
                .and_then(|text| frozen_slice(text, manifest))
        } else {
            let Some(frozen) = frozen_source(manifest) else {
                return Ok(None);
            };
            texts
                .get(&(work, frozen))
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
                .expect("source tuple was qualified")
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

fn frozen_source(manifest: &Value) -> Option<FrozenWorkSource> {
    let field = manifest["field"].as_str()?;
    Some(FrozenWorkSource {
        source_ref: reference(manifest, "sourceRef")?,
        block_ref: manifest["blockRef"].as_str()?.to_owned(),
        field: field
            .strip_prefix("work_context:")
            .unwrap_or(field)
            .to_owned(),
        source_version: manifest["sourceVersion"].as_str()?.to_owned(),
        as_of: manifest["asOf"].as_str()?.to_owned(),
    })
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

type CommentScope = (Uuid, Uuid, String);
type QualifiedComments = HashMap<CommentScope, Option<Uuid>>;
type CommentTexts = HashMap<(Uuid, Uuid, bool, String), String>;

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
) -> Result<CommentTexts, TopicMapError> {
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
    let refs: Vec<_> = comment_refs
        .iter()
        .filter_map(|manifest| reference(manifest, "sourceRef"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let records = comment_records(tx, &refs).await?;
    let mut qualified = QualifiedComments::new();
    for cutoff in comment_refs
        .iter()
        .filter_map(|m| m["asOf"].as_str())
        .collect::<BTreeSet<_>>()
    {
        let cutoff_value = linggan_evidence::research_dossier::cutoff_in(tx, Some(cutoff)).await?;
        // The same gate retains current domain/restriction checks, while both
        // child and parent observations come from this manifest's cutoff.
        let sources = eligible_sources_for_topic_research_in_transaction(
            tx,
            domain,
            &cutoff_value,
            &selections,
        )
        .await
        .map_err(|e| TopicMapError::Source(e.to_string()))?;
        for source in sources {
            qualified.insert(
                (
                    source.content_public_ref,
                    source.source_ref,
                    cutoff.to_owned(),
                ),
                source
                    .parent_source_ref
                    .filter(|_| source.parent_research_text.is_some()),
            );
        }
    }
    Ok(select_comment_texts(&comment_refs, &records, &qualified))
}

fn select_comment_texts(
    manifests: &[&Value],
    records: &HashMap<Uuid, CommentRecord>,
    qualified: &QualifiedComments,
) -> CommentTexts {
    let mut texts = HashMap::new();
    let mut parents = HashSet::new();
    for manifest in manifests
        .iter()
        .filter(|m| m["field"] != "parent_comment_context")
    {
        let Some(scope) = comment_scope(manifest) else {
            continue;
        };
        let Some(parent) = qualified.get(&scope) else {
            continue;
        };
        let Some(record) = records
            .get(&scope.1)
            .filter(|r| comment_tuple_matches(manifest, scope.1, r))
        else {
            continue;
        };
        if let Some(text) = &record.text {
            texts.insert((scope.0, scope.1, false, scope.2.clone()), text.clone());
        }
        if let Some(parent) = parent
            && records
                .get(parent)
                .is_some_and(|r| r.work == record.work && record.parent.as_ref() == Some(&r.id))
        {
            parents.insert((scope.0, *parent, scope.2));
        }
    }
    for manifest in manifests
        .iter()
        .filter(|m| m["field"] == "parent_comment_context")
    {
        let Some(scope) = comment_scope(manifest) else {
            continue;
        };
        let Some(record) = records
            .get(&scope.1)
            .filter(|r| comment_tuple_matches(manifest, scope.1, r))
        else {
            continue;
        };
        if parents.contains(&scope)
            && let Some(text) = &record.text
        {
            texts.insert((scope.0, scope.1, true, scope.2), text.clone());
        }
    }
    texts
}

fn comment_scope(manifest: &Value) -> Option<CommentScope> {
    Some((
        reference(manifest, "workRef")?,
        reference(manifest, "sourceRef")?,
        manifest["asOf"].as_str()?.to_owned(),
    ))
}

fn comment_tuple_matches(manifest: &Value, source: Uuid, record: &CommentRecord) -> bool {
    let Some(text) = &record.text else {
        return false;
    };
    let kind = if manifest["field"] == "parent_comment_context" {
        "comment-context"
    } else {
        "comment"
    };
    let block = format!(
        "{}.{}.{}",
        record.work,
        kind,
        linggan_evidence::creator_discovery::hash(&record.id)
    );
    Some(record.work) == reference(manifest, "workRef")
        && reference(manifest, "sourceRef") == Some(source)
        && manifest["blockRef"] == block
        && manifest["sourceVersion"] == linggan_evidence::creator_discovery::hash(text)
}

#[cfg(test)]
mod tuple_tests {
    use super::*;
    #[test]
    fn historical_comment_requires_exact_source_version_block_and_qualified_cutoff() {
        let work = Uuid::from_u128(1);
        let source = Uuid::from_u128(2);
        let text = "完整评论原声🙂";
        let record = CommentRecord {
            work,
            id: "reply".into(),
            parent: None,
            text: Some(text.into()),
        };
        let cutoff = "2026-10-15T00:00:00Z";
        let records = HashMap::from([(source, record)]);
        let qualified = HashMap::from([((work, source, cutoff.to_owned()), None)]);
        let manifest = serde_json::json!({"workRef":work,"sourceRef":source,"field":"unresearched_comment",
            "blockRef":format!("{work}.comment.{}",linggan_evidence::creator_discovery::hash("reply")),
            "sourceVersion":linggan_evidence::creator_discovery::hash(text),"asOf":cutoff});
        assert_eq!(
            select_comment_texts(&[&manifest], &records, &qualified)
                .get(&(work, source, false, cutoff.to_owned()))
                .map(String::as_str),
            Some(text)
        );
        for key in ["workRef", "sourceRef", "sourceVersion", "blockRef", "asOf"] {
            let mut forged = manifest.clone();
            forged[key] = Value::String("unqualified".into());
            assert!(
                select_comment_texts(&[&forged], &records, &qualified).is_empty(),
                "must bind {key}"
            );
        }
        assert!(select_comment_texts(&[&manifest], &records, &HashMap::new()).is_empty());
    }

    #[test]
    fn child_qualification_cannot_authorize_other_parent_version_or_cutoff() {
        let (work, child, parent, later_parent) = (
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
        );
        let cutoff = "2026-10-15T00:00:00Z";
        let later = "2026-10-16T00:00:00Z";
        let child_text = "这一步该怎样开始练习？";
        let parent_text = "从一次很短的练习开始。";
        let records = HashMap::from([
            (
                child,
                CommentRecord {
                    work,
                    id: "reply".into(),
                    parent: Some("parent".into()),
                    text: Some(child_text.into()),
                },
            ),
            (
                parent,
                CommentRecord {
                    work,
                    id: "parent".into(),
                    parent: None,
                    text: Some(parent_text.into()),
                },
            ),
            (
                later_parent,
                CommentRecord {
                    work,
                    id: "parent".into(),
                    parent: None,
                    text: Some(parent_text.into()),
                },
            ),
        ]);
        let qualified = HashMap::from([((work, child, cutoff.to_owned()), Some(parent))]);
        let child_manifest = serde_json::json!({"workRef":work,"sourceRef":child,"field":"unresearched_comment",
            "blockRef":format!("{work}.comment.{}",linggan_evidence::creator_discovery::hash("reply")),
            "sourceVersion":linggan_evidence::creator_discovery::hash(child_text),"asOf":cutoff});
        let parent_manifest = serde_json::json!({"workRef":work,"sourceRef":parent,"field":"parent_comment_context",
            "blockRef":format!("{work}.comment-context.{}",linggan_evidence::creator_discovery::hash("parent")),
            "sourceVersion":linggan_evidence::creator_discovery::hash(parent_text),"asOf":cutoff});
        let original =
            select_comment_texts(&[&child_manifest, &parent_manifest], &records, &qualified);
        assert_eq!(original.len(), 2);
        for (key, value) in [
            ("sourceRef", serde_json::json!(later_parent)),
            ("asOf", serde_json::json!(later)),
        ] {
            let mut forged = parent_manifest.clone();
            forged[key] = value;
            let selected = select_comment_texts(&[&child_manifest, &forged], &records, &qualified);
            assert_eq!(selected.len(), 1, "parent authority must bind {key}");
            assert!(selected.contains_key(&(work, child, false, cutoff.to_owned())));
        }
        let only_later = HashMap::from([((work, child, later.to_owned()), Some(parent))]);
        assert!(
            select_comment_texts(&[&child_manifest, &parent_manifest], &records, &only_later)
                .is_empty()
        );
    }
}
