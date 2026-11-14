//! Current Core coverage over an already domain-qualified finite work scope.
use super::*;
use std::collections::BTreeMap;

pub(crate) async fn scope_coverage(
    db: &Database,
    works: &[Uuid],
    cutoff: &str,
) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let rows = sqlx::query("SELECT work_ref,material_snapshot_identity FROM linggan_content_understanding_saved_core WHERE work_ref=ANY($1) AND method_version=$2 AND result IS NOT NULL AND created_at<=$3::timestamptz")
        .bind(works).bind(METHOD).bind(cutoff).fetch_all(&mut *tx).await?;
    let mut identities = BTreeMap::<Uuid, Vec<String>>::new();
    for row in rows {
        identities
            .entry(row.get("work_ref"))
            .or_default()
            .push(row.get("material_snapshot_identity"));
    }
    let existing: Vec<_> = identities.keys().copied().collect();
    let mut dossiers =
        linggan_evidence::research_dossier::read_in(&mut tx, &existing, cutoff).await?;
    let mut counts =
        json!({"available":0,"missing":works.len()-existing.len(),"stale":0,"unknown":0});
    for (work, saved) in identities {
        let state = match dossiers.remove(&work) {
            None => "unknown",
            Some(dossier) => {
                let packet = source::from_dossier(dossier, cutoff)?;
                classify(
                    &saved,
                    &packet.snapshot.material_snapshot_identity,
                    packet.coverage["materialReadiness"]["state"] == "ready",
                )
            }
        };
        counts[state] = json!(counts[state].as_u64().unwrap_or(0) + 1);
    }
    tx.commit().await?;
    Ok(counts)
}

fn classify(saved: &[String], current: &str, ready: bool) -> &'static str {
    if ready && saved.iter().any(|identity| identity == current) {
        "available"
    } else {
        "stale"
    }
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
