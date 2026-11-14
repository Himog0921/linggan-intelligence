//! Assemble protocol-separated demand and creator groups from the existing metric rows.
use super::{CardError, model};
use crate::opportunity::{
    CreatorDensity, CreatorDensityGroup, OpportunityMetricParams, rate_per_1k,
    sort_creator_density_group,
};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

fn rate(k: i32, n: i32) -> Result<Option<f64>, CardError> {
    let k = u64::try_from(k).map_err(|_| CardError::InvalidMetrics)?;
    let n = u64::try_from(n).map_err(|_| CardError::InvalidMetrics)?;
    Ok(rate_per_1k(k, n))
}

pub(super) async fn demand(
    tx: &mut Transaction<'_, Postgres>,
    problem: Uuid,
    taxonomy: i64,
    version: i32,
    as_of: &str,
    params: &OpportunityMetricParams,
) -> Result<Vec<model::DemandGroup>, CardError> {
    let rows = sqlx::query(
        "SELECT protocol_version,period_start::text,k,n,ci_low,ci_high,baseline_k,baseline_n,\
         no_baseline,mentioners_total,evidence_tier FROM linggan_opportunity_problem_metric_period \
         WHERE problem_ref=$1 AND taxonomy_version=$2 AND metric_def_version=$3 \
           AND computed_as_of=$4::timestamptz ORDER BY protocol_version NULLS LAST,period_start DESC",
    )
    .bind(problem)
    .bind(taxonomy)
    .bind(version)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    let count = usize::try_from(params.series_periods).map_err(|_| CardError::InvalidMetrics)?;
    let window_count = params
        .series_periods
        .checked_add(params.baseline_periods)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or(CardError::InvalidMetrics)?;
    let mut grouped: BTreeMap<Option<i32>, Vec<sqlx::postgres::PgRow>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry(row.try_get("protocol_version")?)
            .or_default()
            .push(row);
    }
    if grouped.is_empty() {
        return Err(CardError::MissingMetrics);
    }
    let mut result = Vec::with_capacity(grouped.len());
    for (protocol_version, rows) in grouped {
        if rows.len() < window_count {
            return Err(CardError::MissingMetrics);
        }
        let first = &rows[0];
        let k: i32 = first.try_get("k")?;
        let n: i32 = first.try_get("n")?;
        let low: Option<f64> = first.try_get("ci_low")?;
        let high: Option<f64> = first.try_get("ci_high")?;
        let interval = match (low, high) {
            (Some(low), Some(high)) => Some(model::Interval {
                low: low * 1000.0,
                high: high * 1000.0,
            }),
            (None, None) if n == 0 => None,
            _ => return Err(CardError::InvalidMetrics),
        };
        let baseline_k: i32 = first.try_get("baseline_k")?;
        let baseline_n: i32 = first.try_get("baseline_n")?;
        let mut flags = Vec::new();
        if u32::try_from(n).map_err(|_| CardError::InvalidMetrics)? < params.min_n_period {
            flags.push("insufficient_sample");
        }
        if first.try_get::<bool, _>("no_baseline")? {
            flags.push("no_baseline");
        }
        let mut series = Vec::with_capacity(count);
        for row in rows.iter().take(count).rev() {
            let k: i32 = row.try_get("k")?;
            let n: i32 = row.try_get("n")?;
            series.push(model::Period {
                period_start: row.try_get("period_start")?,
                k,
                n,
                rate_per_1k: rate(k, n)?,
            });
        }
        let window_has_samples =
            rows.iter()
                .take(window_count)
                .try_fold(false, |present, row| {
                    let n: i32 = row.try_get("n")?;
                    Ok::<_, sqlx::Error>(present || n > 0)
                })?;
        result.push(model::DemandGroup {
            protocol_version,
            mentioners_total: first.try_get("mentioners_total")?,
            evidence_tier: first.try_get("evidence_tier")?,
            current: model::CurrentPeriod {
                period_start: first.try_get("period_start")?,
                k,
                n,
                rate_per_1k: rate(k, n)?,
                ci_per_1k: interval,
            },
            baseline: model::Baseline {
                k: baseline_k,
                n: baseline_n,
                rate_per_1k: rate(baseline_k, baseline_n)?,
            },
            series,
            flags,
            window_has_samples,
        });
    }
    Ok(result)
}

pub(super) async fn creators(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    problem: Uuid,
    taxonomy: i64,
    version: i32,
    as_of: &str,
    params: &OpportunityMetricParams,
) -> Result<Vec<model::CreatorGroup>, CardError> {
    let rows = sqlx::query(
        "SELECT density.protocol_version,density.creator_id,density.k,density.n,density.ci_low,\
         density.ci_high,density.sufficient,profile.display_name,profile.follower_count,\
         EXISTS(SELECT 1 FROM collection_observation_target target JOIN observation_domain_target relation \
           ON relation.target_ref=target.target_ref WHERE relation.domain_ref=$1 AND relation.created_at<=$5::timestamptz \
           AND target.first_stored_at<=$5::timestamptz AND target.target_kind='creator' \
           AND target.purpose_kind='long_term' AND target.lifecycle_state<>'dismissed' \
           AND target.platform='xhs' AND target.identity_key=density.creator_id) AS observed \
         FROM linggan_opportunity_creator_problem_density density \
         LEFT JOIN LATERAL (SELECT CASE WHEN p.display_name_state='KNOWN' THEN p.display_name END AS display_name, \
           CASE WHEN p.follower_count_state='KNOWN' THEN p.follower_count END AS follower_count \
           FROM linggan_material_author_profile p JOIN linggan_runtime_capture_package package USING(package_ref) \
           WHERE p.platform='xhs' AND p.author_external_id=density.creator_id AND p.created_at<=$5::timestamptz \
             AND package.accepted_at<=$5::timestamptz \
           ORDER BY p.observed_at::timestamptz DESC,p.created_at DESC,p.material_ref DESC LIMIT 1) profile ON true \
         WHERE density.problem_ref=$2 AND density.taxonomy_version=$3 AND density.metric_def_version=$4 \
           AND density.computed_as_of=$5::timestamptz AND density.window_days=$6 \
         ORDER BY density.protocol_version NULLS LAST,density.creator_id",
    )
    .bind(domain).bind(problem).bind(taxonomy).bind(version).bind(as_of)
    .bind(i32::try_from(params.creator_window_days).map_err(|_| CardError::InvalidMetrics)?)
    .fetch_all(&mut **tx).await?;
    let mut grouped: BTreeMap<Option<i32>, CreatorDensityGroup> = BTreeMap::new();
    let mut details = BTreeMap::new();
    for row in rows {
        let protocol_version: Option<i32> = row.try_get("protocol_version")?;
        let k: i32 = row.try_get("k")?;
        let n: i32 = row.try_get("n")?;
        let low: f64 = row.try_get("ci_low")?;
        let high: f64 = row.try_get("ci_high")?;
        let creator_id: String = row.try_get("creator_id")?;
        let sufficient: bool = row.try_get("sufficient")?;
        let creator = model::Creator {
            creator_id: creator_id.clone(),
            name: row.try_get("display_name")?,
            followers: row.try_get("follower_count")?,
            k,
            n,
            rate_per_1k: rate(k, n)?.ok_or(CardError::InvalidMetrics)?,
            ci_per_1k: model::Interval {
                low: low * 1000.0,
                high: high * 1000.0,
            },
            observed: row.try_get("observed")?,
        };
        let density = CreatorDensity {
            creator_id: creator_id.clone(),
            k: u64::try_from(k).map_err(|_| CardError::InvalidMetrics)?,
            n: u64::try_from(n).map_err(|_| CardError::InvalidMetrics)?,
            ci_low: low,
            ci_high: high,
            sufficient,
        };
        if details
            .insert((protocol_version, creator_id), creator)
            .is_some()
        {
            return Err(CardError::InvalidMetrics);
        }
        let group = grouped
            .entry(protocol_version)
            .or_insert_with(|| CreatorDensityGroup {
                problem_ref: problem,
                taxonomy_version: taxonomy,
                metric_def_version: version,
                window_days: params.creator_window_days,
                protocol_version,
                ranked: Vec::new(),
                insufficient: Vec::new(),
            });
        if sufficient {
            group.ranked.push(density);
        } else {
            group.insufficient.push(density);
        }
    }
    let mut result = Vec::with_capacity(grouped.len());
    for (_, mut group) in grouped {
        sort_creator_density_group(&mut group);
        let protocol_version = group.protocol_version;
        let mut take_creators =
            |items: Vec<CreatorDensity>| -> Result<Vec<model::Creator>, CardError> {
                items
                    .into_iter()
                    .map(|density| {
                        details
                            .remove(&(protocol_version, density.creator_id))
                            .ok_or(CardError::InvalidMetrics)
                    })
                    .collect()
            };
        result.push(model::CreatorGroup {
            protocol_version,
            ranked: take_creators(group.ranked)?,
            insufficient: take_creators(group.insufficient)?,
        });
    }
    Ok(result)
}
