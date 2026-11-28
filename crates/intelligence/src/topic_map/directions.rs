//! A reviewed operating catalog maps stable research topics to editorial directions.
//! A published version remains in effect while its successor is edited.
use super::{TopicMapError, TopicMapSnapshot, TopicMapTopic};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Direction {
    pub direction_ref: Uuid,
    pub label: String,
    pub definition: String,
    pub topic_refs: Vec<Uuid>,
    #[serde(default)]
    pub text_source: Option<(Uuid, Uuid)>,
    #[serde(default)]
    pub text_sources: Vec<(Uuid, Uuid)>,
    #[serde(default)]
    pub evidence_work_refs: Vec<Uuid>,
    #[serde(default)]
    pub candidate_source: Option<(Uuid, Uuid)>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub directions: Vec<Direction>,
    /// Explicitly selected parent-facing benchmark creators on Xiaohongshu.
    pub benchmark_authors: BTreeSet<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Save {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub expected_revision: i64,
    pub preview_hash: Option<String>,
    pub publish: bool,
    pub catalog: Catalog,
}

fn available(topic: &TopicMapTopic, domain: Uuid) -> bool {
    topic.core["sourceState"] != "source_unavailable"
        && topic.domain_ref == Some(domain)
        && topic.lifecycle_state != "superseded"
        && !topic.work_refs.is_empty()
}

fn propose(topics: &[&TopicMapTopic]) -> Catalog {
    let ids: HashSet<_> = topics.iter().map(|t| t.topic_ref).collect();
    let mut roots: Vec<_> = topics
        .iter()
        .copied()
        .filter(|t| !t.parent_topic_ref.is_some_and(|p| ids.contains(&p)))
        .collect();
    roots.sort_by_key(|t| (std::cmp::Reverse(t.work_refs.len()), t.topic_ref));
    Catalog {
        directions: roots
            .into_iter()
            .take(40)
            .map(|t| Direction {
                direction_ref: t.topic_ref,
                label: t.display_name.clone(),
                definition: t.definition_text.clone(),
                topic_refs: vec![t.topic_ref],
                text_source: Some((t.topic_ref, t.definition_ref)),
                text_sources: Vec::new(),
                evidence_work_refs: Vec::new(),
                candidate_source: None,
            })
            .collect(),
        benchmark_authors: BTreeSet::new(),
    }
}

async fn qualified_sources<'a>(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    domain: Uuid,
    catalogs: impl IntoIterator<Item = &'a Catalog>,
    snapshot: &TopicMapSnapshot,
) -> Result<BTreeSet<(Uuid, Uuid)>, TopicMapError> {
    let requested: BTreeSet<_> = catalogs
        .into_iter()
        .flat_map(|c| c.directions.iter().filter_map(|d| d.candidate_source))
        .collect();
    if requested.is_empty() {
        return Ok(requested);
    }
    crate::ai_foundation::operating_direction::readable_candidates_in(
        tx, domain, &requested, snapshot,
    )
    .await
    .map_err(|e| TopicMapError::Source(e.to_string()))
}

async fn validate_sources(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    s: &Save,
    snap: &TopicMapSnapshot,
) -> Result<(), TopicMapError> {
    let current = qualified_sources(tx, s.domain_ref, [&s.catalog], snap).await?;
    if s.catalog
        .directions
        .iter()
        .filter_map(|d| d.candidate_source)
        .any(|id| !current.contains(&id))
    {
        return Err(TopicMapError::Invalid("direction_source_changed"));
    }
    Ok(())
}

pub async fn save(
    db: &Database,
    s: Save,
    snapshot: &TopicMapSnapshot,
) -> Result<Value, TopicMapError> {
    let state = if s.publish { "published" } else { "draft" };
    let catalog = serde_json::to_value(&s.catalog).expect("catalog serializes");
    let mut tx = db.pool().begin().await?;
    // Serialize only edits to this domain, including the first version.
    sqlx::query_scalar::<_, Uuid>(
        "SELECT domain_ref FROM observation_domain WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(s.domain_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(TopicMapError::NotFound)?;
    if let Some(row) = sqlx::query("SELECT domain_ref, revision, state, catalog FROM linggan_operating_direction_revision WHERE command_ref=$1")
        .bind(s.command_ref).fetch_optional(&mut *tx).await? {
        let revision:i64=row.get("revision");
        if row.get::<Uuid,_>("domain_ref")!=s.domain_ref || revision-1!=s.expected_revision || row.get::<String,_>("state")!=state || row.get::<Value,_>("catalog")!=catalog {return Err(TopicMapError::Conflict);}
        return Ok(json!({"revision":revision,"state":state}));
    }
    let revision:i64=sqlx::query_scalar("SELECT coalesce(max(revision),0) FROM linggan_operating_direction_revision WHERE domain_ref=$1").bind(s.domain_ref).fetch_one(&mut *tx).await?;
    if revision != s.expected_revision {
        return Err(TopicMapError::Conflict);
    }
    validate(&s.catalog, s.publish, s.domain_ref, snapshot)?;
    validate_sources(&mut tx, &s, snapshot).await?;
    if s.publish
        && impact_in(&mut tx, &s, snapshot).await?["previewHash"].as_str()
            != s.preview_hash.as_deref()
    {
        return Err(TopicMapError::Conflict);
    }
    sqlx::query("INSERT INTO linggan_operating_direction_revision(domain_ref, revision, command_ref, state, catalog) VALUES ($1,$2,$3,$4,$5)")
        .bind(s.domain_ref).bind(revision+1).bind(s.command_ref).bind(state).bind(catalog).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(json!({"revision":revision+1,"state":state}))
}

pub(crate) fn evidence_current(direction: &Direction, snapshot: &TopicMapSnapshot, domain: Uuid) -> bool {
    direction.evidence_work_refs.iter().all(|work| {
        snapshot.topics.iter().any(|topic| {
            available(topic, domain)
                && direction
                    .text_sources
                    .iter()
                    .any(|(id, _)| *id == topic.topic_ref)
                && topic.work_refs.contains(work)
        })
    })
}

fn validate(
    catalog: &Catalog,
    publish: bool,
    domain: Uuid,
    snapshot: &TopicMapSnapshot,
) -> Result<(), TopicMapError> {
    let known: HashSet<_> = snapshot
        .topics
        .iter()
        .filter(|t| available(t, domain))
        .map(|t| t.topic_ref)
        .collect();
    let external: HashSet<_> = snapshot
        .works
        .iter()
        .filter(|w| w.platform == "xhs" && !w.own)
        .filter_map(|w| w.author_external_id.as_ref())
        .collect();
    let mut ids = HashSet::new();
    let mut mapped = HashSet::new();
    if (publish && catalog.directions.is_empty())
        || catalog
            .benchmark_authors
            .iter()
            .any(|author| !external.contains(author))
    {
        return Err(TopicMapError::Invalid("invalid_direction_catalog"));
    }
    for d in &catalog.directions {
        if !ids.insert(d.direction_ref)
            || d.label.trim().is_empty()
            || d.definition.trim().is_empty()
            || d.text_source
                .into_iter()
                .chain(d.text_sources.iter().copied())
                .any(|(id, version)| {
                    !snapshot.topics.iter().any(|t| {
                        available(t, domain) && t.topic_ref == id && t.definition_ref == version
                    })
                })
            || !evidence_current(d, snapshot, domain)
            || d.topic_refs.is_empty()
            || d.topic_refs
                .iter()
                .any(|t| !known.contains(t) || !mapped.insert(*t))
        {
            return Err(TopicMapError::Invalid("invalid_direction_mapping"));
        }
    }
    Ok(())
}

pub async fn preview(
    db: &Database,
    s: Save,
    snap: &TopicMapSnapshot,
) -> Result<Value, TopicMapError> {
    validate(&s.catalog, s.publish, s.domain_ref, snap)?;
    let mut tx = db.pool().begin().await?;
    validate_sources(&mut tx, &s, snap).await?;
    let impact = impact_in(&mut tx, &s, snap).await?;
    tx.commit().await?;
    Ok(impact)
}
async fn impact_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    s: &Save,
    snap: &TopicMapSnapshot,
) -> Result<Value, TopicMapError> {
    let topics: Vec<_> = snap
        .topics
        .iter()
        .filter(|t| available(t, s.domain_ref))
        .collect();
    let current=sqlx::query_scalar::<_,Value>("SELECT catalog FROM linggan_operating_direction_revision WHERE domain_ref=$1 AND state='published' ORDER BY revision DESC LIMIT 1")
        .bind(s.domain_ref).fetch_optional(&mut **tx).await?;
    let prior: Catalog = current
        .map(serde_json::from_value)
        .transpose()
        .map_err(|_| TopicMapError::Source("invalid saved catalog".into()))?
        .unwrap_or_default();
    let before = project(&prior, &topics, snap);
    let after = project(&s.catalog, &topics, snap);
    fn placements(view: &Value) -> BTreeMap<String, BTreeSet<String>> {
        let mut placements = BTreeMap::<String, BTreeSet<String>>::new();
        for d in view["directions"].as_array().into_iter().flatten() {
            for w in d["works"].as_array().into_iter().flatten() {
                if let (Some(work), Some(direction)) = (w["workRef"].as_str(), d["id"].as_str()) {
                    placements
                        .entry(work.into())
                        .or_default()
                        .insert(direction.into());
                }
            }
        }
        placements
    }
    let old = placements(&before);
    let new = placements(&after);
    let changed: BTreeSet<_> = old
        .keys()
        .chain(new.keys())
        .filter(|k| old.get(*k) != new.get(*k))
        .collect();
    let transitions: Vec<_> = changed
        .iter()
        .map(|k| (*k, old.get(*k), new.get(*k)))
        .collect();
    let fingerprint = crate::research_text::content_hash(
        &json!([s.domain_ref, s.expected_revision, s.catalog, transitions]).to_string(),
    );
    Ok(
        json!({"previewHash":fingerprint,"before":before["assignedWorks"],"after":after["assignedWorks"],"changedWorks":changed.len(),
        "priorDirections":prior.directions.len(),"nextDirections":s.catalog.directions.len()}),
    )
}
pub async fn read(
    db: &Database,
    domain: Uuid,
    snap: &TopicMapSnapshot,
) -> Result<Value, TopicMapError> {
    let topics: Vec<_> = snap
        .topics
        .iter()
        .filter(|t| available(t, domain))
        .collect();
    let mut tx = db.pool().begin().await?;
    let rows=sqlx::query("SELECT DISTINCT ON (state) state, revision, catalog FROM linggan_operating_direction_revision WHERE domain_ref=$1 ORDER BY state, revision DESC")
        .bind(domain).fetch_all(&mut *tx).await?;
    let mut latest = 0;
    let mut draft = None;
    let mut published = None;
    let mut published_revision = None;
    for row in rows {
        let revision: i64 = row.get("revision");
        let catalog: Catalog = serde_json::from_value(row.get("catalog"))
            .map_err(|_| TopicMapError::Source("invalid saved catalog".into()))?;
        if row.get::<String, _>("state") == "published" {
            published_revision = Some(revision);
            published = Some(catalog.clone());
        }
        if revision > latest {
            latest = revision;
            draft = Some(catalog);
        }
    }
    let current: BTreeMap<_, _> = topics
        .iter()
        .map(|t| (t.topic_ref, t.definition_ref))
        .collect();
    let sources =
        qualified_sources(&mut tx, domain, draft.iter().chain(published.iter()), snap).await?;
    tx.commit().await?;
    fn redact(
        sources: &BTreeSet<(Uuid, Uuid)>,
        c: &mut Catalog,
        current: &BTreeMap<Uuid, Uuid>,
        snapshot: &TopicMapSnapshot,
        domain: Uuid,
    ) {
        for d in &mut c.directions {
            if d.candidate_source.is_some_and(|id| !sources.contains(&id))
                || !evidence_current(d, snapshot, domain)
                || d.text_source
                    .into_iter()
                    .chain(d.text_sources.iter().copied())
                    .any(|(t, v)| current.get(&t) != Some(&v))
            {
                d.label = "来源已变化的方向".into();
                d.definition = "重新选择当前依据并确认方向说明。".into();
            }
        }
    }
    let mut suggested = draft.unwrap_or_else(|| propose(&topics));
    redact(&sources, &mut suggested, &current, snap, domain);
    if let Some(c) = published.as_mut() {
        redact(&sources, c, &current, snap, domain);
    }
    let titles: BTreeMap<_, _> = snap
        .works
        .iter()
        .map(|w| (w.work_ref, w.title.as_ref()))
        .collect();
    let mut authors = BTreeMap::new();
    for w in snap.works.iter().filter(|w| !w.own && w.platform == "xhs") {
        if let Some(id) = w.author_external_id.as_ref() {
            authors
                .entry(id)
                .or_insert_with(|| w.creator_display_name.as_deref().unwrap_or(id));
        }
    }
    Ok(
        json!({"revision":latest,"publishedRevision":published_revision,"draft":suggested,
      "topics":topics.iter().map(|t| json!({"id":t.topic_ref,"label":t.display_name,"definition":t.definition_text,"definitionRef":t.definition_ref,"parent":t.parent_topic_ref,"samples":t.work_refs.iter().take(3).map(|w|json!({"id":w,"title":titles.get(w)})).collect::<Vec<_>>()})).collect::<Vec<_>>(),
      "authors":authors.iter().map(|(id,name)|json!({"id":id,"name":name})).collect::<Vec<_>>(),
      "published":published.as_ref().map(|p| project(p,&topics,snap)), "totalWorks":snap.works.len()}),
    )
}

fn project(catalog: &Catalog, topics: &[&TopicMapTopic], snap: &TopicMapSnapshot) -> Value {
    // Inherit from the nearest confirmed research topic. A more specific mapping
    // overrides its parent; separate direct discussions may still map to several directions.
    let parents: BTreeMap<_, _> = topics
        .iter()
        .map(|t| (t.topic_ref, t.parent_topic_ref))
        .collect();
    let explicit: BTreeMap<_, _> = catalog
        .directions
        .iter()
        .flat_map(|d| d.topic_refs.iter().map(move |t| (*t, d.direction_ref)))
        .collect();
    let mut members: BTreeMap<Uuid, HashSet<Uuid>> = BTreeMap::new();
    for topic in topics {
        let mut current = Some(topic.topic_ref);
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                break;
            }
            if let Some(direction) = explicit.get(&id) {
                members
                    .entry(*direction)
                    .or_default()
                    .extend(topic.direct_work_refs.iter().copied());
                break;
            }
            current = parents.get(&id).copied().flatten();
        }
    }
    let publications: BTreeMap<_, _> = snap.scope["operations"]["works"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some((
                Uuid::parse_str(r["workRef"].as_str()?).ok()?,
                r["publications"].as_array()?,
            ))
        })
        .collect();
    let mut assigned = HashSet::new();
    let mut directions = Vec::new();
    for d in &catalog.directions {
        let works = members.remove(&d.direction_ref).unwrap_or_default();
        assigned.extend(works.iter().copied());
        let mut own_total = 0;
        let mut own_known = 0;
        let mut own_hits = 0;
        let mut external_total = 0;
        let mut external_known = 0;
        let mut external_hits = 0;
        let mut sources = Vec::new();
        for w in snap.works.iter().filter(|w| works.contains(&w.work_ref)) {
            let xhs: Vec<_> = publications
                .get(&w.work_ref)
                .into_iter()
                .flat_map(|p| p.iter())
                .filter(|p| p["platform"] == "xhs")
                .collect();
            let own_hit = xhs.iter().any(|p| p["hit"] == true);
            if !xhs.is_empty() {
                own_total += 1;
                if own_hit || xhs.iter().all(|p| p["views"].is_number()) {
                    own_known += 1;
                }
                if own_hit {
                    own_hits += 1;
                }
            }
            let benchmark = !w.own
                && xhs.is_empty()
                && w.platform == "xhs"
                && w.author_external_id
                    .as_ref()
                    .is_some_and(|a| catalog.benchmark_authors.contains(a));
            if benchmark {
                external_total += 1;
                if w.likes.is_some() {
                    external_known += 1;
                }
                if w.likes.is_some_and(|v| v >= 200) {
                    external_hits += 1;
                }
            }
            sources.push(json!({"workRef":w.work_ref,"title":w.title,"ownHit":own_hit,"publications":xhs,"benchmark":benchmark,"likes":w.likes}));
        }
        directions.push(json!({"id":d.direction_ref,"label":d.label,"definition":d.definition,"works":sources,
            "ownTotal":own_total,"ownKnown":own_known,"ownHits":own_hits,"externalTotal":external_total,"externalKnown":external_known,"externalHits":external_hits}));
    }
    json!({"assignedWorks":assigned.len(),"directions":directions})
}

#[cfg(test)]
#[path = "directions_tests.rs"]
mod tests;
