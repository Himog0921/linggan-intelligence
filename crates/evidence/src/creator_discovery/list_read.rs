//! Exact creator statistics first; only visible unjudged works expand full dossiers.
//! Research/manual support and full-text predicates always use canonical full reads.
use super::*;

pub async fn load_list(database: &Database, q: &CreatorScope) -> Result<Value, DiscoveryError> {
    q.validate().map_err(DiscoveryError::Invalid)?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let value = read_in(&mut tx, q).await?;
    tx.commit().await?;
    Ok(value)
}

async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    q: &CreatorScope,
) -> Result<Value, DiscoveryError> {
    let mut deferred = HashSet::new();
    let mut data = load_materials_scoped_in(tx, q, None, None, Some(&mut deferred)).await?;
    let page = aggregate(&data, q);
    let needed: Vec<Uuid> = page["items"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|item| item["representatives"].as_array().into_iter().flatten())
        .filter_map(|work| work["workRef"].as_str()?.parse().ok())
        .filter(|reference| deferred.contains(reference))
        .collect();
    if needed.is_empty() {
        return Ok(page);
    }
    // The exact same transaction/watermark owns both statistics and displayed text.
    // There is no new material cache and no weaker source eligibility path.
    let hydrated = load_materials_in(tx, q, Some(&needed), Some(&data.as_of)).await?;
    let mut hydrated: HashMap<_, _> = hydrated
        .works
        .into_iter()
        .map(|work| (work.work_ref, work))
        .collect();
    for work in &mut data.works {
        if needed.contains(&work.work_ref) {
            *work = hydrated
                .remove(&work.work_ref)
                .ok_or(DiscoveryError::Invalid("dossier_unavailable"))?;
        }
    }
    Ok(aggregate(&data, q))
}
