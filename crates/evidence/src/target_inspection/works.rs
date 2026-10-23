use super::{Error, METHOD_VERSION, Metric, Metrics, Scope, scope, trace};
use crate::WorkResource;
use crate::work_resource_current::{
    WorkResourceCurrent, WorkResourceCurrentSource, read_work_resource_currents,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ComparisonMode {
    #[serde(rename = "current")]
    Current,
    #[serde(rename = "age_24h")]
    Age24h,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSnapshot {
    pub work_ref: Uuid,
    pub status: &'static str,
    pub resource: Option<WorkResource>,
    pub metrics: Metrics,
    pub published_epoch_ms: Option<i64>,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceStatistic {
    pub median: Option<f64>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkComparison {
    pub target_ref: Uuid,
    pub knowledge_cutoff: String,
    pub method_version: &'static str,
    pub mode: ComparisonMode,
    pub age_hours: Option<u32>,
    pub tolerance_minutes: Option<u32>,
    pub items: Vec<WorkSnapshot>,
    pub reference_statistics: BTreeMap<&'static str, ReferenceStatistic>,
}

pub async fn compare_works(
    database: &Database,
    request: &Scope,
    ids: &[Uuid],
    mode: ComparisonMode,
) -> Result<WorkComparison, Error> {
    if !(2..=3).contains(&ids.len())
        || ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(Error::Invalid("select_two_or_three_distinct_works"));
    }
    let (mut tx, cutoff) = scope::begin(database, request).await?;
    let mut readable = Vec::new();
    for id in ids {
        match scope::require_work(&mut tx, request, *id, &cutoff).await {
            Ok(()) => readable.push(*id),
            Err(Error::NotFound) => (),
            Err(e) => return Err(e),
        }
    }
    let mut currents = read_work_resource_currents(&mut tx, &readable, &cutoff)
        .await?
        .into_iter()
        .map(|c| (c.public_ref, c))
        .collect::<BTreeMap<_, _>>();
    let mut items = Vec::new();
    for id in ids {
        if let Some(current) = currents.remove(id) {
            items.push(snapshot(&mut tx, request, &cutoff, current, mode).await?);
        } else {
            items.push(WorkSnapshot {
                work_ref: *id,
                status: "unavailable",
                resource: None,
                metrics: Metrics::unknown(),
                published_epoch_ms: None,
                reasons: vec!["work_not_readable_in_scope"],
            });
        }
    }
    let reference_statistics = statistics(&items);
    tx.commit().await?;
    Ok(WorkComparison {
        target_ref: request.target_ref,
        knowledge_cutoff: cutoff,
        method_version: METHOD_VERSION,
        mode,
        age_hours: matches!(mode, ComparisonMode::Age24h).then_some(24),
        tolerance_minutes: matches!(mode, ComparisonMode::Age24h).then_some(60),
        items,
        reference_statistics,
    })
}

async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    request: &Scope,
    cutoff: &str,
    current: WorkResourceCurrent,
    mode: ComparisonMode,
) -> Result<WorkSnapshot, Error> {
    let mut resource = crate::material_projection::material_item(&current, None);
    crate::material_projection::enrich_discovery_material(
        tx,
        &mut resource,
        cutoff,
        current.author_attribution_source.as_deref(),
    )
    .await
    .map_err(|_| Error::Material)?;
    crate::material_social_read::enrich(tx, &mut resource, None, cutoff)
        .await
        .map_err(|_| Error::Material)?;
    crate::material_projection::enrich_media_material(tx, &mut resource, cutoff)
        .await
        .map_err(|_| Error::Material)?;
    // Research text is read through canonical V2 at this same cutoff, never the list excerpt.
    resource.evidence_fragment = None;
    let mut reasons = Vec::new();
    let metrics = match mode {
        ComparisonMode::Current => current_metrics(tx, &current).await?,
        ComparisonMode::Age24h => match current.published_at_epoch_ms {
            Some(published) => {
                age_metrics(tx, request, current.public_ref, cutoff, published).await?
            }
            None => {
                reasons.push("qualified_publication_time_missing");
                Metrics::unknown()
            }
        },
    };
    if matches!(mode, ComparisonMode::Age24h)
        && [
            &metrics.likes,
            &metrics.comments,
            &metrics.collects,
            &metrics.shares,
        ]
        .iter()
        .all(|m| m.value.is_none())
    {
        reasons.push("no_observation_near_24_hours");
    }
    Ok(WorkSnapshot {
        work_ref: current.public_ref,
        status: "ready",
        resource: Some(resource),
        metrics,
        published_epoch_ms: current.published_at_epoch_ms,
        reasons,
    })
}

async fn current_metrics(
    tx: &mut Transaction<'_, Postgres>,
    c: &WorkResourceCurrent,
) -> Result<Metrics, Error> {
    let fields = [
        (c.like_count, &c.like_count_state, &c.like_source),
        (c.comment_count, &c.comment_count_state, &c.comment_source),
        (c.collect_count, &c.collect_count_state, &c.collect_source),
        (c.share_count, &c.share_count_state, &c.share_source),
    ];
    let times = fields
        .iter()
        .filter_map(|(_, _, s)| s.observed_at.clone())
        .collect::<Vec<_>>();
    let epochs:Vec<(String,i64)>=sqlx::query_as(
        "SELECT value,(extract(epoch FROM value::timestamptz)*1000)::bigint FROM unnest($1::text[]) value",
    ).bind(&times).fetch_all(&mut **tx).await?;
    let epochs = epochs.into_iter().collect::<BTreeMap<_, _>>();
    let build = |(value, state, source): (Option<i64>, &String, &WorkResourceCurrentSource)| {
        let known = state == "KNOWN" && value.is_some_and(|v| v >= 0);
        Metric {
            value: if known { value } else { None },
            state: if known { "known" } else { "unknown" }.into(),
            observation_ref: source.package_ref.map(|p| format!("{p}:{}", c.public_ref)),
            observed_at: source.observed_at.clone(),
            observed_epoch_ms: source
                .observed_at
                .as_ref()
                .and_then(|t| epochs.get(t).copied()),
            source_kind: source.source_lane.clone(),
            precision: "reported".into(),
        }
    };
    Ok(Metrics {
        likes: build(fields[0]),
        comments: build(fields[1]),
        collects: build(fields[2]),
        shares: build(fields[3]),
    })
}

async fn age_metrics(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
    work: Uuid,
    cutoff: &str,
    published: i64,
) -> Result<Metrics, Error> {
    let rows = sqlx::query(include_str!("age_metrics.sql"))
        .bind(work)
        .bind(cutoff)
        .bind(published)
        .bind(scope.domain_ref)
        .fetch_all(&mut **tx)
        .await?;
    let mut result = Metrics::unknown();
    for row in rows {
        let name: String = row.try_get("name")?;
        let reference: Uuid = row.try_get("material_ref")?;
        let raw: Option<Value> = row.try_get("raw")?;
        let precision = trace::precision(raw.as_ref().unwrap_or(&Value::Null), &name);
        let metric = Metric {
            value: row.try_get("value")?,
            state: "known".into(),
            observation_ref: Some(reference.to_string()),
            observed_at: Some(row.try_get("observed_at")?),
            observed_epoch_ms: Some(row.try_get("observed_epoch_ms")?),
            source_kind: Some(row.try_get("source_kind")?),
            precision: precision.into(),
        };
        match name.as_str() {
            "likes" => result.likes = metric,
            "comments" => result.comments = metric,
            "collects" => result.collects = metric,
            "shares" => result.shares = metric,
            _ => return Err(Error::Invalid("unknown_metric")),
        }
    }
    Ok(result)
}

fn statistics(items: &[WorkSnapshot]) -> BTreeMap<&'static str, ReferenceStatistic> {
    let mut result = BTreeMap::new();
    for key in ["likes", "comments", "collects", "shares"] {
        let mut values = items
            .iter()
            .skip(1)
            .filter_map(|item| {
                let metric = match key {
                    "likes" => &item.metrics.likes,
                    "comments" => &item.metrics.comments,
                    "collects" => &item.metrics.collects,
                    _ => &item.metrics.shares,
                };
                (metric.state == "known").then_some(metric.value).flatten()
            })
            .collect::<Vec<_>>();
        values.sort_unstable();
        let n = values.len();
        let median = if n == 0 {
            None
        } else if n % 2 == 1 {
            Some(values[n / 2] as f64)
        } else {
            Some(values[n / 2 - 1] as f64 / 2.0 + values[n / 2] as f64 / 2.0)
        };
        result.insert(key, ReferenceStatistic { median, count: n });
    }
    result
}
