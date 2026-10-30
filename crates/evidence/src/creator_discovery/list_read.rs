//! Exact creator statistics first; only visible unjudged works expand full dossiers.
//! Research/manual support and full-text predicates always use canonical full reads.
use super::*;

pub async fn load_list(database: &Database, q: &CreatorScope) -> Result<Value, DiscoveryError> {
    load_list_timed(database, q).await.map(|(value, _)| value)
}

pub async fn load_list_timed(
    database: &Database,
    q: &CreatorScope,
) -> Result<(Value, ReadTimings), DiscoveryError> {
    q.validate().map_err(DiscoveryError::Invalid)?;
    let started = Instant::now();
    let mut timing = ReadTimings::default();
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    timing.pool = started.elapsed();
    let value = read_in(&mut tx, q, &mut timing).await?;
    tx.commit().await?;
    timing.read = started.elapsed();
    Ok((value, timing))
}

async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    q: &CreatorScope,
    timing: &mut ReadTimings,
) -> Result<Value, DiscoveryError> {
    let mut deferred = HashMap::new();
    let mut data = load_materials_scoped_in(tx, q, None, None, Some(&mut deferred), timing).await?;
    let aggregate_started = Instant::now();
    let mut page = aggregate(&data, q);
    timing.aggregate += aggregate_started.elapsed();
    let needed: HashSet<Uuid> = page["items"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|item| item["representatives"].as_array().into_iter().flatten())
        .filter_map(|work| work["workRef"].as_str()?.parse().ok())
        .filter(|reference| deferred.contains_key(reference))
        .collect();
    if needed.is_empty() {
        return Ok(page);
    }
    // The same transaction/as_of owns statistics, Current and dossier eligibility.
    // Reuse Current rather than repeating the whole domain/author metadata load.
    let currents: Vec<_> = needed
        .iter()
        .filter_map(|reference| deferred.get(reference))
        .collect();
    let dossier_started = Instant::now();
    let mut dossiers =
        crate::research_dossier::read_from_currents_in(tx, &currents, &data.as_of).await?;
    timing.dossier += dossier_started.elapsed();
    let representatives_started = Instant::now();
    for work in &mut data.works {
        if needed.contains(&work.work_ref) {
            let dossier = dossiers
                .remove(&work.work_ref)
                .ok_or(DiscoveryError::Invalid("dossier_unavailable"))?;
            material_read::hydrate(work, &data.domain, dossier);
        }
    }
    // No predicate input changes: only publish hydrated representatives. Exact
    // counts, same-work matches, author order and pagination were computed once.
    let hydrated: HashMap<_, _> = data
        .works
        .iter()
        .filter(|work| needed.contains(&work.work_ref))
        .map(|work| (work.work_ref, work))
        .collect();
    for item in page["items"].as_array_mut().into_iter().flatten() {
        for representative in item["representatives"].as_array_mut().into_iter().flatten() {
            let reference = representative["workRef"]
                .as_str()
                .and_then(|value| value.parse::<Uuid>().ok());
            if let Some(work) = reference.and_then(|reference| hydrated.get(&reference)) {
                *representative = json!(work);
            }
        }
    }
    timing.representatives += representatives_started.elapsed();
    Ok(page)
}
