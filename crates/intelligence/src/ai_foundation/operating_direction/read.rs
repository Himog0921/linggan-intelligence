//! Published and pending consumers share the same current-source qualification.
use super::*;

pub async fn latest(db: &Database, domain_ref: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let mut rows:Vec<Value>=sqlx::query_scalar(
        "SELECT jsonb_build_object('runRef',r.run_ref,'batchRef',r.batch_ref,'taskKey',b.task_key,'state',c.state,
             'reason',coalesce(c.failure_code,c.wait_reason),'candidates',coalesce(r.candidates,'[]'::jsonb),
             'invocationRef',r.invocation_ref,'createdAt',r.created_at)
         FROM linggan_operating_direction_run r JOIN linggan_ai_batch b USING(batch_ref)
         JOIN linggan_ai_batch_item i USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref)
         WHERE r.domain_ref=$1 ORDER BY r.created_at DESC,r.run_ref DESC LIMIT 20")
        .bind(domain_ref).fetch_all(&mut *tx).await?;
    let ids = rows
        .iter()
        .flat_map(|r| {
            r["candidates"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| key(&r["runRef"], &c["directionRef"]))
        })
        .collect::<BTreeSet<_>>();
    if !ids.is_empty() {
        let snap = current_snapshot(&mut tx, domain_ref).await?;
        let valid = readable_candidates_in(&mut tx, domain_ref, &ids, &snap).await?;
        for r in &mut rows {
            let run = r["runRef"].clone();
            let cs = r["candidates"]
                .as_array_mut()
                .ok_or(FoundationError::Invalid)?;
            let count = cs.len();
            cs.retain(|c| key(&run, &c["directionRef"]).is_some_and(|id| valid.contains(&id)));
            if cs.len() != count {
                r["reason"] = json!("direction_source_changed");
            }
        }
    }
    tx.commit().await?;
    Ok(json!({"runs":rows}))
}

fn key(run: &Value, candidate: &Value) -> Option<(Uuid, Uuid)> {
    Some((
        Uuid::parse_str(run.as_str()?).ok()?,
        Uuid::parse_str(candidate.as_str()?).ok()?,
    ))
}

pub async fn readable_candidates_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    sources: &BTreeSet<(Uuid, Uuid)>,
    snap: &TopicMapSnapshot,
) -> Result<BTreeSet<(Uuid, Uuid)>, FoundationError> {
    if sources.is_empty() {
        return Ok(BTreeSet::new());
    }
    let runs = sources
        .iter()
        .map(|(r, _)| *r)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rows=sqlx::query_as::<_,(Uuid,sqlx::types::Json<Packet>,sqlx::types::Json<Vec<Value>>)>(
        "SELECT run_ref,frozen_input,candidates FROM linggan_operating_direction_run WHERE domain_ref=$1 AND run_ref=ANY($2) AND candidates IS NOT NULL"
    ).bind(domain).bind(runs).fetch_all(&mut **tx).await?;
    let mut refs = BTreeSet::new();
    for (run, _, cs) in &rows {
        for c in &cs.0 {
            if key(&json!(run), &c["directionRef"]).is_some_and(|k| sources.contains(&k)) {
                refs.extend(
                    serde_json::from_value::<Vec<Uuid>>(c["evidenceWorkRefs"].clone())
                        .map_err(|_| FoundationError::Invalid)?,
                );
            }
        }
    }
    let evidence = current_evidence(tx, &refs.into_iter().collect::<Vec<_>>()).await?;
    let support = support_pairs(tx, snap, domain).await?;
    let mut qualified = BTreeSet::new();
    for (run, p, cs) in rows {
        for c in cs.0 {
            if let Some(k) = key(&json!(run), &c["directionRef"])
                && sources.contains(&k)
                && candidate_current(&c, &p, snap, &evidence, &support)
            {
                qualified.insert(k);
            }
        }
    }
    Ok(qualified)
}

fn candidate_current(
    candidate: &Value,
    p: &Packet,
    snap: &TopicMapSnapshot,
    evidence: &BTreeMap<Uuid, Vec<Value>>,
    support: &BTreeSet<(Uuid, Uuid)>,
) -> bool {
    let ids = |k: &str| serde_json::from_value::<BTreeSet<Uuid>>(candidate[k].clone()).ok();
    let (Some(topic_refs), Some(work_refs)) = (ids("topicRefs"), ids("evidenceWorkRefs")) else {
        return false;
    };
    let mut linked = BTreeSet::new();
    for id in topic_refs {
        let Some(t) = snap.topics.iter().find(|t| {
            t.topic_ref == id
                && t.domain_ref == Some(p.domain_ref)
                && t.lifecycle_state != "superseded"
                && t.core["sourceState"] != "source_unavailable"
        }) else {
            return false;
        };
        let Some(saved) = p.topics.get(&id) else {
            return false;
        };
        if t.definition_ref != saved.definition_ref {
            return false;
        }
        let supported: BTreeSet<_> = saved
            .work_refs
            .intersection(&work_refs)
            .copied()
            .filter(|w| t.direct_work_refs.contains(w) && support.contains(&(t.definition_ref, *w)))
            .collect();
        if supported.is_empty() {
            return false;
        }
        linked.extend(supported);
    }
    linked == work_refs
        && work_refs.iter().all(|w| {
            snap.works
                .iter()
                .any(|record| record.work_ref == *w && record.readable)
                && p.evidence
                    .get(w)
                    .zip(evidence.get(w))
                    .is_some_and(|(saved, now)| saved.iter().all(|e| now.contains(e)))
        })
}
