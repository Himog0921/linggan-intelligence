//! Freeze eligible dossier evidence with current research topic and version identities.
use super::*;

async fn columns(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
) -> Result<BTreeSet<String>, FoundationError> {
    Ok(
        sqlx::query_scalar::<_, sqlx::types::Json<BTreeSet<String>>>(
            "SELECT material_columns FROM linggan_ai_task_config WHERE task_config_ref=$1",
        )
        .bind(config)
        .fetch_one(&mut **tx)
        .await?
        .0,
    )
}

// Return identities and current, source-qualified topic names. Directory text can itself
// derive from withdrawn material and is never used as provider input.
async fn current_catalog(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    snapshot: &TopicMapSnapshot,
) -> Result<BTreeMap<Uuid, Value>, FoundationError> {
    let catalog:Option<sqlx::types::Json<Catalog>>=sqlx::query_scalar("SELECT catalog FROM linggan_operating_direction_revision WHERE domain_ref=$1 ORDER BY revision DESC LIMIT 1").bind(domain).fetch_optional(&mut **tx).await?;
    let Some(sqlx::types::Json(catalog)) = catalog else {
        return Ok(BTreeMap::new());
    };
    let requested = catalog
        .directions
        .iter()
        .filter_map(|d| d.candidate_source)
        .collect();
    let live = readable_candidates_in(tx, domain, &requested, snapshot).await?;
    let topics: BTreeMap<_, _> = snapshot
        .topics
        .iter()
        .filter(|t| {
            t.domain_ref == Some(domain)
                && t.lifecycle_state != "superseded"
                && t.core["sourceState"] != "source_unavailable"
                && !t.work_refs.is_empty()
        })
        .map(|t| (t.topic_ref, t))
        .collect();
    Ok(catalog
        .directions
        .into_iter()
        .filter(|d| {
            d.candidate_source.is_none_or(|k| live.contains(&k))
                && crate::topic_map::directions::evidence_current(d, snapshot, domain)
                && d.text_source
                    .into_iter()
                    .chain(d.text_sources.iter().copied())
                    .all(|(id, version)| {
                        topics.get(&id).is_some_and(|t| t.definition_ref == version)
                    })
        })
        .filter_map(|d| {
            let current: BTreeMap<_, _> = d
                .topic_refs
                .iter()
                .filter_map(|id| topics.get(id).map(|t| (id, &t.display_name)))
                .collect();
            (!current.is_empty()).then(|| (d.direction_ref, json!({"mappedTopics":current})))
        })
        .collect())
}
pub(super) async fn current_evidence(
    tx: &mut Transaction<'_, Postgres>,
    refs: &[Uuid],
) -> Result<BTreeMap<Uuid, Vec<Value>>, FoundationError> {
    // Project only approved dossier columns, preserving the source of each excerpt.
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let mut evidence = BTreeMap::new();
    for chunk in refs.chunks(100) {
        for (work, dossier) in
            linggan_evidence::research_dossier::read_in(tx, chunk, &as_of).await?
        {
            let mut excerpts = Vec::new();
            let mut fields = BTreeSet::new();
            for f in dossier
                .fragments
                .iter()
                .filter(|f| f.field != "frame_text" && !f.text.trim().is_empty())
            {
                if !fields.insert(&f.field) {
                    continue;
                }
                excerpts.push(json!({"field":f.field,"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"sourceVersion":f.source_version,
                    "start":f.start,"text":f.text.chars().take(180).collect::<String>()}));
            }
            if !excerpts.is_empty() {
                evidence.insert(work, excerpts);
            }
        }
    }
    Ok(evidence)
}
// A boundary or counterexample in a manually reviewed pack is not positive source material
// for induction. A separate accepted supporting discussion on the same work can qualify it.
pub(super) async fn support_pairs(
    tx: &mut Transaction<'_, Postgres>,
    snapshot: &TopicMapSnapshot,
    domain: Uuid,
) -> Result<BTreeSet<(Uuid, Uuid)>, FoundationError> {
    let defs = snapshot
        .topics
        .iter()
        .filter(|t| t.domain_ref == Some(domain))
        .map(|t| t.definition_ref)
        .collect::<Vec<_>>();
    let mut pairs:BTreeSet<_>=sqlx::query_as("SELECT r.definition_ref,m.work_public_ref FROM linggan_topic_classification_run r JOIN linggan_topic_material_member m USING(classification_run_ref) WHERE r.definition_ref=ANY($1) AND r.run_kind='human_adjudicated' AND m.role='support'")
        .bind(defs).fetch_all(&mut **tx).await?.into_iter().collect();
    for work in &snapshot.works {
        let units = work
            .research
            .as_ref()
            .and_then(|r| r.pointer("/core/units"))
            .and_then(Value::as_array);
        for u in units.into_iter().flatten().filter(|u| {
            matches!(u["status"].as_str(), Some("new" | "matched"))
                && u["evidenceRole"] == "support"
        }) {
            for assignment in u["assignments"].as_array().into_iter().flatten() {
                if let Some(def) = assignment["definitionRef"]
                    .as_str()
                    .and_then(|x| Uuid::parse_str(x).ok())
                {
                    pairs.insert((def, work.work_ref));
                }
            }
        }
        if let Some(def) = work
            .annotation
            .as_ref()
            .and_then(|a| a["definition_ref"].as_str())
            .and_then(|x| Uuid::parse_str(x).ok())
        {
            pairs.insert((def, work.work_ref));
        }
    }
    Ok(pairs)
}

pub(super) async fn packet(
    tx: &mut Transaction<'_, Postgres>,
    request: &Generate,
    snapshot: &TopicMapSnapshot,
    config: Uuid,
    frozen: Option<&Packet>,
) -> Result<Packet, FoundationError> {
    let domain = request.domain_ref;
    let cols = columns(tx, config).await?;
    let support = support_pairs(tx, snapshot, domain).await?;
    let existing = current_catalog(tx, domain, snapshot).await?;
    let readable: BTreeSet<_> = snapshot
        .works
        .iter()
        .filter(|w| w.readable)
        .map(|w| w.work_ref)
        .collect();
    let mut topics = BTreeMap::new();
    let mut refs = BTreeSet::new();
    for t in &snapshot.topics {
        if t.domain_ref != Some(domain)
            || t.lifecycle_state == "superseded"
            || t.core["sourceState"] == "source_unavailable"
            || (!request.topic_refs.is_empty() && !request.topic_refs.contains(&t.topic_ref))
        {
            continue;
        }
        let work_refs: BTreeSet<_> = t
            .direct_work_refs
            .iter()
            .copied()
            .filter(|w| {
                support.contains(&(t.definition_ref, *w))
                    && readable.contains(w)
                    && frozen.is_none_or(|p| {
                        p.topics
                            .get(&t.topic_ref)
                            .is_some_and(|x| x.work_refs.contains(w))
                    })
            })
            .collect();
        if work_refs.is_empty() {
            continue;
        }
        refs.extend(work_refs.iter().copied());
        topics.insert(
            t.topic_ref,
            Topic {
                definition_ref: t.definition_ref,
                label: t.display_name.clone(),
                work_refs,
            },
        );
    }
    let mut evidence = current_evidence(tx, &refs.into_iter().collect::<Vec<_>>()).await?;
    evidence.retain(|_, rows| {
        rows.retain(|r| r["field"].as_str().is_some_and(|f| cols.contains(f)));
        !rows.is_empty()
    });
    // The prompt receives representative works per topic. New works arriving later do not alter a frozen run.
    let mut used = BTreeSet::new();
    topics.retain(|_, t| {
        t.work_refs = t
            .work_refs
            .iter()
            .copied()
            .filter(|w| evidence.contains_key(w))
            .take(2)
            .collect();
        used.extend(t.work_refs.iter().copied());
        !t.work_refs.is_empty()
    });
    evidence.retain(|w, _| used.contains(w));
    if topics.is_empty() || request.topic_refs.iter().any(|r| !topics.contains_key(r)) {
        return Err(FoundationError::AdmissionDenied(vec![
            "direction_source_unavailable".into(),
        ]));
    }
    Ok(Packet {
        task: request.task_key,
        domain_ref: domain,
        topics,
        evidence,
        existing,
    })
}
