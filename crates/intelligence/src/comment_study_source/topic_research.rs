//! Full qualified comment pagination for Topic Map; Study preview APIs stay bounded.
use super::*;

/// Complete acquired-comment input for Topic Map, using the same qualification and
/// cleaning gates as Study. Reads are paged; the Study preview/run limits are unchanged.
pub(crate) async fn eligible_sources_for_topic_research(
    database: &Database,
    domain: Uuid,
    as_of: &str,
    selections: &[(Uuid, StudySourceRole)],
) -> Result<Vec<StudySource>, StudySourceError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='15s'")
        .execute(&mut *tx)
        .await?;
    let sources =
        eligible_sources_for_topic_research_in_transaction(&mut tx, domain, as_of, selections)
            .await?;
    tx.commit().await?;
    Ok(sources)
}

pub(crate) async fn eligible_sources_for_topic_research_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    selections: &[(Uuid, StudySourceRole)],
) -> Result<Vec<StudySource>, StudySourceError> {
    let mut selected_roles = BTreeMap::new();
    for &(work, role) in selections {
        selected_roles
            .entry(work)
            .and_modify(|current: &mut StudySourceRole| *current = (*current).min(role))
            .or_insert(role);
    }
    let selections: Vec<_> = selected_roles.into_iter().collect();
    if selections.is_empty() {
        return Ok(Vec::new());
    }
    let mut sources = Vec::new();
    for selections in selections.chunks(100) {
        let mut after = (0, Uuid::nil());
        loop {
            let page = candidate_page(tx, domain, as_of, Some(selections), None, after).await?;
            let mut candidates = Vec::new();
            for row in &page {
                after = (row.try_get("rank")?, row.try_get("content_public_ref")?);
                if let Ok(candidate) = classify(row)? {
                    candidates.push(candidate);
                }
            }
            sources.extend(attach_context(tx, domain, as_of, candidates).await?);
            if page.len() < SOURCE_PAGE_SIZE as usize {
                break;
            }
        }
    }
    Ok(sources)
}
