//! Current Core coverage over an already domain-qualified finite work scope.
use super::*;
use std::collections::BTreeMap;

#[cfg(test)]
pub(crate) async fn scope_coverage(
    db: &Database,
    works: &[Uuid],
    cutoff: &str,
) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let dossiers = linggan_evidence::research_dossier::read_in(&mut tx, works, cutoff).await?;
    let states = coverage_from_dossiers(&mut tx, works, cutoff, dossiers).await?;
    tx.commit().await?;
    Ok(count_coverage(works.iter(), &states))
}

pub(crate) fn count_coverage<'a>(
    works: impl Iterator<Item = &'a Uuid>,
    states: &BTreeMap<Uuid, &'static str>,
) -> Value {
    let mut counts = json!({"available":0,"missing":0,"stale":0,"unknown":0});
    for work in works {
        let state = states.get(work).copied().unwrap_or("missing");
        counts[state] = json!(counts[state].as_u64().unwrap_or(0) + 1);
    }
    counts
}

/// Qualify saved results against the exact page snapshot that supplied material readiness.
pub(crate) async fn coverage_from_dossiers(
    tx: &mut Transaction<'_, Postgres>,
    works: &[Uuid],
    cutoff: &str,
    mut dossiers: BTreeMap<Uuid, linggan_evidence::research_dossier::ResearchDossier>,
) -> Result<BTreeMap<Uuid, &'static str>, FoundationError> {
    let rows = sqlx::query("SELECT core.work_ref,core.material_snapshot_identity,request.input_manifest FROM linggan_content_understanding_saved_core core JOIN linggan_content_understanding_request request USING(request_ref) WHERE core.work_ref=ANY($1) AND core.method_version=$2 AND core.result IS NOT NULL AND core.created_at<=$3::timestamptz")
        .bind(works).bind(METHOD).bind(cutoff).fetch_all(&mut **tx).await?;
    let mut identities = BTreeMap::<Uuid, Vec<(String, Option<source::Packet>)>>::new();
    for row in rows {
        identities.entry(row.get("work_ref")).or_default().push((
            row.get("material_snapshot_identity"),
            serde_json::from_value(row.get("input_manifest")).ok(),
        ));
    }
    let mut states = BTreeMap::new();
    for (work, saved) in identities {
        let state = match dossiers.remove(&work) {
            None => "unknown",
            Some(dossier) => {
                let packet = source::from_dossier(dossier, cutoff)?;
                let mut available = false;
                for (identity, previous) in &saved {
                    if qualified(
                        identity,
                        &packet.snapshot.material_snapshot_identity,
                        packet.coverage["materialReadiness"]["state"] == "ready",
                    ) {
                        available = true;
                        break;
                    }
                    if let Some(prior) = previous {
                        if source::saved_result_available(tx, work, cutoff, prior, &packet).await? {
                            available = true;
                            break;
                        }
                    }
                }
                if available { "available" } else { "stale" }
            }
        };
        states.insert(work, state);
    }
    Ok(states)
}

#[cfg(test)]
fn classify(saved: &[String], current: &str, ready: bool) -> &'static str {
    if saved
        .iter()
        .any(|identity| qualified(identity, current, ready))
    {
        "available"
    } else {
        "stale"
    }
}

pub(super) fn qualified(saved: &str, current: &str, ready: bool) -> bool {
    ready && saved == current
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_core_requires_same_material_and_current_readiness() {
        let saved = vec!["older".into(), "current".into()];
        assert_eq!(classify(&saved, "current", true), "available");
        assert_eq!(classify(&saved, "new", true), "stale");
        assert_eq!(classify(&saved, "current", false), "stale");
    }
}
