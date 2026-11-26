use super::{FoundationError, ResearchInput, WorkEligibility, source, source_error};
use linggan_evidence::research_dossier;
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub async fn read(
    db: &Database,
    domain: Uuid,
    works: &[Uuid],
) -> Result<Vec<WorkEligibility>, FoundationError> {
    if domain.is_nil() || works.is_empty() || works.len() > 20 || works.iter().any(Uuid::is_nil) {
        return Err(FoundationError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let config = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='topic_research'",
    )
    .fetch_one(&mut *tx)
    .await?;
    let cutoff = research_dossier::cutoff_in(&mut tx, None).await?;
    let (_, result) = qualify(&mut tx, domain, config, works, &cutoff, None).await?;
    tx.commit().await?;
    Ok(result)
}

pub(super) async fn qualify(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    config: Uuid,
    works: &[Uuid],
    cutoff: &str,
    expected_core: Option<Uuid>,
) -> Result<(Vec<ResearchInput>, Vec<WorkEligibility>), FoundationError> {
    let input =
        source::load_core_inputs_for_works_at_in(tx, domain, config, works, cutoff, expected_core)
            .await
            .map_err(source_error)?;
    let unresolved: Vec<Value> = sqlx::query_scalar(
        "SELECT DISTINCT w.input_refs FROM linggan_topic_research_window w
         JOIN linggan_topic_research_run r USING(run_ref)
         JOIN linggan_topic_research_stage s USING(window_ref)
         JOIN linggan_ai_batch_item_control c USING(item_ref)
         WHERE r.domain_ref=$1 AND w.work_ref=ANY($2) AND c.failure_code='ai_dispatch_unknown'",
    )
    .bind(domain)
    .bind(works)
    .fetch_all(&mut **tx)
    .await?;
    let refs: Vec<Value> = unresolved
        .iter()
        .flat_map(|manifest| {
            manifest["fragments"]
                .as_array()
                .into_iter()
                .flatten()
                .cloned()
        })
        .collect();
    let mut windows = Vec::new();
    let mut results = BTreeMap::new();
    for full in input {
        let work_windows = source::research_windows(&full);
        let reason = if work_windows.is_empty() {
            Some("ai_source_unavailable".to_owned())
        } else if work_windows
            .iter()
            .any(|window| source::overlaps_unknown_dispatch(window, &full, &refs))
        {
            Some("ai_prior_dispatch_unresolved".to_owned())
        } else {
            windows.extend(work_windows);
            None
        };
        results.insert(full.work.work_ref, reason);
    }
    let missing: Vec<_> = works
        .iter()
        .filter(|work| !results.contains_key(*work))
        .copied()
        .collect();
    if !missing.is_empty() {
        let scoped: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT content_public_ref FROM linggan_material_domain_usage
             WHERE domain_ref=$1 AND use_kind='domain_research' AND content_public_ref=ANY($2)",
        )
        .bind(domain)
        .bind(&missing)
        .fetch_all(&mut **tx)
        .await?;
        let scoped_set: BTreeSet<_> = scoped.iter().copied().collect();
        let dossiers = research_dossier::read_in(tx, &scoped, cutoff).await?;
        for work in missing {
            let reason = if scoped_set.contains(&work) {
                dossiers
                    .get(&work)
                    .and_then(|dossier| {
                        let readiness = research_dossier::l1_material_readiness(dossier);
                        readiness
                            .gaps
                            .into_iter()
                            .filter(|gap| {
                                matches!(
                                    gap.code.as_str(),
                                    "video_transcript_required" | "video_transcript_unavailable"
                                )
                            })
                            .max_by_key(|gap| gap.code == "video_transcript_unavailable")
                            .map(|gap| gap.code)
                    })
                    .unwrap_or_else(|| "content_understanding_required".into())
            } else {
                "ai_source_unavailable".into()
            };
            results.insert(work, Some(reason));
        }
    }
    Ok((
        windows,
        works
            .iter()
            .map(|&work_ref| WorkEligibility {
                work_ref,
                reason: results.get(&work_ref).cloned().flatten(),
            })
            .collect(),
    ))
}
