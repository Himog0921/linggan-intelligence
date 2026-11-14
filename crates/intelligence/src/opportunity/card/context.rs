//! Domain-level quality flags and transparent collection protocol descriptions.
use super::{CardError, model};
use crate::opportunity::{MetricExclusion, OpportunityMetricParams, PeriodBasis};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) fn basis(params: &OpportunityMetricParams) -> &'static str {
    match params.period_basis {
        PeriodBasis::WorkPublishedCohort => "work_published_cohort",
        PeriodBasis::CommentPublished => "comment_published",
    }
}

fn volume_shift(counts: impl IntoIterator<Item = (i64, i64)>, threshold: f64) -> bool {
    let (current, previous) = counts
        .into_iter()
        .fold((0_i64, 0_i64), |(cur, prev), (c, p)| (cur + c, prev + p));
    previous > 0 && ((current - previous).abs() as f64 / previous as f64) > threshold
}

pub(super) async fn exclusions_and_flags(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    version: i32,
    params: &OpportunityMetricParams,
    metrics_exclusions: &[MetricExclusion],
    groups: &[model::DemandGroup],
) -> Result<(Vec<model::Exclusion>, Vec<&'static str>), CardError> {
    let exclusions: Vec<_> = metrics_exclusions
        .iter()
        .filter(|item| item.metric_def_version == version)
        .map(|item| model::Exclusion {
            reason: item.reason.clone(),
            comments: item.comments,
        })
        .collect();
    let eligible: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_eligible_comments($1,$2::timestamptz,$3,$4)",
    )
    .bind(domain)
    .bind(as_of)
    .bind(&params.period_tz)
    .bind(basis(params))
    .fetch_one(&mut **tx)
    .await?;
    let total = eligible + exclusions.iter().map(|item| item.comments).sum::<i64>();
    let share = |reason: &str| -> f64 {
        if total <= 0 {
            return 0.0;
        }
        exclusions
            .iter()
            .find(|item| item.reason == reason)
            .map_or(0.0, |item| item.comments as f64 / total as f64)
    };
    let mut flags = vec!["not_calibrated"];
    if share("period_anchor_missing") > params.max_unanchored_share {
        flags.push("period_anchor_missing");
    }
    if share("work_author_unknown") > params.max_author_unknown_share {
        flags.push("author_unknown_share_high");
    }
    let current_total: i64 = groups.iter().map(|group| i64::from(group.current.n)).sum();
    if current_total < i64::from(params.collection_target_per_period) {
        flags.push("collection_below_target");
    }
    if groups
        .iter()
        .filter(|group| group.window_has_samples && group.protocol_version.is_some())
        .count()
        > 1
    {
        flags.push("collection_protocol_changed");
    }
    let counts = groups
        .iter()
        .map(|group| (i64::from(group.current.n), i64::from(group.previous_n)));
    if volume_shift(counts, params.collection_volume_shift_alert) {
        flags.push("collection_volume_shift");
    }
    if !has_engagement_reference(tx, domain, as_of, params).await? {
        flags.push("engagement_reference_missing");
    }
    Ok((exclusions, flags))
}

async fn has_engagement_reference(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    params: &OpportunityMetricParams,
) -> Result<bool, CardError> {
    let days: i32 = params
        .supply_engagement_ref
        .strip_prefix("domain_median_likes_")
        .and_then(|value| value.strip_suffix('d'))
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .ok_or(CardError::InvalidEngagementReference)?;
    Ok(sqlx::query_scalar(
        "WITH latest AS (SELECT DISTINCT ON(detail.content_public_ref) detail.content_public_ref,\
         detail.published_at FROM linggan_material_content_detail detail \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         WHERE detail.created_at<=$2::timestamptz AND package.accepted_at<=$2::timestamptz \
         ORDER BY detail.content_public_ref,linggan_opportunity_observed_at(detail.observed_at) DESC NULLS LAST,\
                  detail.created_at DESC,detail.package_ref DESC,detail.material_ref DESC) \
         SELECT EXISTS(SELECT 1 FROM latest JOIN linggan_material_domain_usage usage \
         ON usage.content_public_ref=latest.content_public_ref AND usage.domain_ref=$1 \
         AND usage.use_kind='domain_research' AND usage.role='primary' \
         AND (usage.basis_kind='legacy_domain_migration' OR usage.created_at<=$2::timestamptz) \
         JOIN linggan_material_engagement_observation engagement ON engagement.content_public_ref=latest.content_public_ref \
         JOIN linggan_runtime_capture_package package ON package.package_ref=engagement.package_ref \
         WHERE engagement.like_count_state='KNOWN' AND engagement.created_at<=$2::timestamptz \
           AND package.accepted_at<=$2::timestamptz \
           AND latest.published_at >= $2::timestamptz-make_interval(days=>$3) \
           AND latest.published_at < $2::timestamptz)",
    ).bind(domain).bind(as_of).bind(days).fetch_one(&mut **tx).await?)
}

pub(super) async fn limitations(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    params: &OpportunityMetricParams,
) -> Result<Vec<String>, CardError> {
    let rows = sqlx::query(
        "SELECT DISTINCT package.payload->'collectionProtocol' AS protocol \
         FROM linggan_opportunity_eligible_comments($1,$2::timestamptz,$3,$4) eligible \
         LEFT JOIN linggan_material_comment_first_admission admission ON admission.content_public_ref=eligible.content_ref \
           AND admission.comment_external_id=eligible.comment_external_id \
         LEFT JOIN linggan_runtime_capture_package package ON package.package_ref=admission.first_package_ref \
           AND package.accepted_at<=$2::timestamptz \
           AND EXISTS(SELECT 1 FROM linggan_material_comment observation WHERE observation.content_public_ref=eligible.content_ref \
             AND observation.comment_external_id=eligible.comment_external_id AND observation.package_ref=package.package_ref \
             AND observation.created_at<=$2::timestamptz) \
         WHERE eligible.period_start >= (linggan_opportunity_period_start($2::timestamptz,$3)::timestamp \
           - make_interval(weeks=>$5-1))::date \
           AND eligible.period_start <= linggan_opportunity_period_start($2::timestamptz,$3)",
    ).bind(domain).bind(as_of).bind(&params.period_tz).bind(basis(params))
        .bind(i32::try_from(params.series_periods.checked_add(params.baseline_periods)
            .ok_or(CardError::InvalidMetrics)?).map_err(|_| CardError::InvalidMetrics)?)
        .fetch_all(&mut **tx).await?;
    let mut protocols: BTreeMap<Option<i64>, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        let value: Option<serde_json::Value> = row.try_get("protocol")?;
        let Some(value) = value.filter(|value| value.is_object()) else {
            protocols
                .entry(None)
                .or_default()
                .insert("排序和单次上限未记录".to_owned());
            continue;
        };
        let version = value.get("version").and_then(serde_json::Value::as_i64);
        let sort = match value.get("sortMode").and_then(serde_json::Value::as_str) {
            Some("platform_default") => "平台默认顺序",
            _ => "排序未记录",
        };
        let limit = match value.get("singleRunLimit") {
            Some(serde_json::Value::Number(value)) => format!("单次上限 {value}"),
            Some(serde_json::Value::Null) => "单次上限未设置（全量遍历）".to_owned(),
            _ => "单次上限未记录".to_owned(),
        };
        protocols
            .entry(version)
            .or_default()
            .insert(format!("{sort}，{limit}"));
    }
    if protocols.is_empty() {
        protocols
            .entry(None)
            .or_default()
            .insert("排序和单次上限未记录".to_owned());
    }
    let mut result = vec![match params.period_basis {
        PeriodBasis::WorkPublishedCohort => "提及率按作品发布周统计，表示该周发布的作品下的评论中，提及该问题的比例；近期作品积累评论的时间较短，当周数字偏少。".to_owned(),
        PeriodBasis::CommentPublished => "提及率按评论发布周统计。".to_owned(),
    }];
    for (version, descriptions) in protocols {
        let label = version.map_or_else(
            || "未知协议".to_owned(),
            |value| format!("采集协议 {value}"),
        );
        result.push(format!(
            "{label}：{}。",
            descriptions.into_iter().collect::<Vec<_>>().join("；")
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::volume_shift;

    #[test]
    fn volume_shift_uses_domain_totals_even_when_protocol_mix_changes() {
        assert!(!volume_shift([(200, 100), (0, 100)], 0.5));
        assert!(volume_shift([(100, 100), (100, 0)], 0.5));
        assert!(!volume_shift([(100, 0)], 0.5));
    }
}
