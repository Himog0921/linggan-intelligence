//! Target-scoped, reconstructable creator-work lifecycle projection.
//!
//! A lifecycle point is not a second material fact. It is a read-time combination of one stable
//! Work, an author identity that exactly matches the selected creator target, a producer-qualified
//! publication instant and the latest `KNOWN` engagement value at one repeatable-read `as_of`.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

const CREATOR_LIFECYCLE_SCAN_LIMIT: usize = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatorLifecycleWindow {
    Recent90Days,
    All,
}

impl CreatorLifecycleWindow {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "recent_90_days" => Some(Self::Recent90Days),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recent90Days => "recent_90_days",
            Self::All => "all",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreatorLifecycleMetric {
    Likes,
    Comments,
    Collects,
    Shares,
    CompositeV1,
}

impl CreatorLifecycleMetric {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "likes" => Some(Self::Likes),
            "comments" => Some(Self::Comments),
            "collects" => Some(Self::Collects),
            "shares" => Some(Self::Shares),
            "composite_v1" => Some(Self::CompositeV1),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Likes => "likes",
            Self::Comments => "comments",
            Self::Collects => "collects",
            Self::Shares => "shares",
            Self::CompositeV1 => "composite_v1",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatorLifecycleQuery {
    pub window: CreatorLifecycleWindow,
    pub metric: CreatorLifecycleMetric,
}

impl Default for CreatorLifecycleQuery {
    fn default() -> Self {
        Self {
            window: CreatorLifecycleWindow::Recent90Days,
            metric: CreatorLifecycleMetric::Likes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CreatorLifecycleStatus {
    Ready,
    InsufficientObservation,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLifecycleSummary {
    pub linked_work_count: usize,
    pub confirmed_author_work_count: usize,
    pub eligible_point_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLifecycleExclusions {
    pub author_not_verified: usize,
    pub author_mismatch: usize,
    pub published_at_not_qualified: usize,
    pub outside_window: usize,
    pub metric_unknown: usize,
    pub scan_truncated: bool,
}

impl CreatorLifecycleExclusions {
    fn empty() -> Self {
        Self {
            author_not_verified: 0,
            author_mismatch: 0,
            published_at_not_qualified: 0,
            outside_window: 0,
            metric_unknown: 0,
            scan_truncated: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLifecycleReceipt {
    pub scan_limit: usize,
    pub scanned_count: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLifecycleAnalysis {
    pub percentile_version: &'static str,
    pub rolling_median_version: &'static str,
    pub rolling_median_window: usize,
    pub composite_version: &'static str,
}

impl CreatorLifecycleAnalysis {
    fn current() -> Self {
        Self {
            percentile_version: "creator-percentile-v1",
            rolling_median_version: "trailing-5-work-median-v1",
            rolling_median_window: 5,
            composite_version: "composite-v1",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLifecyclePoint {
    pub work_public_ref: Uuid,
    pub title: Option<String>,
    pub title_state: &'static str,
    pub published_at: String,
    pub published_local_date: String,
    pub published_at_epoch_ms: i64,
    pub metric_value: i64,
    pub creator_percentile: f64,
    pub rolling_median: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorLifecycleProjection {
    pub target_ref: Uuid,
    pub target_kind: String,
    pub status: CreatorLifecycleStatus,
    pub as_of: String,
    pub window: CreatorLifecycleWindow,
    pub metric: CreatorLifecycleMetric,
    pub summary: CreatorLifecycleSummary,
    pub exclusions: CreatorLifecycleExclusions,
    pub receipt: CreatorLifecycleReceipt,
    pub analysis: CreatorLifecycleAnalysis,
    pub points: Vec<CreatorLifecyclePoint>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreatorLifecycleReadError {
    #[error("the creator lifecycle projection schema is unavailable")]
    ProjectionUnavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

struct TargetIdentity {
    target_ref: Uuid,
    target_kind: String,
    identity_key: String,
}

struct CandidateWork {
    work_public_ref: Uuid,
    title: Option<String>,
    author_external_id: Option<String>,
    published_at: Option<String>,
    published_local_date: Option<String>,
    published_at_epoch_ms: Option<i64>,
    inside_recent_window: bool,
    like_count: Option<i64>,
    comment_count: Option<i64>,
    collect_count: Option<i64>,
    share_count: Option<i64>,
}

/// Reads one creator's work-lifecycle projection without acquiring data or persisting derived
/// points. `None` means the target reference is not present in this database.
pub async fn read_creator_lifecycle(
    database: &Database,
    target_ref: Uuid,
    query: &CreatorLifecycleQuery,
) -> Result<Option<CreatorLifecycleProjection>, CreatorLifecycleReadError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let target = sqlx::query(
        "SELECT target_ref,target_kind,identity_key \
         FROM collection_observation_target \
         WHERE target_ref=$1 AND first_stored_at <= $2::timestamptz",
    )
    .bind(target_ref)
    .bind(&as_of)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_schema_error)?
    .map(|row| TargetIdentity {
        target_ref: row.get("target_ref"),
        target_kind: row.get("target_kind"),
        identity_key: row.get("identity_key"),
    });
    let Some(target) = target else {
        tx.commit().await?;
        return Ok(None);
    };
    if target.target_kind != "creator" {
        tx.commit().await?;
        return Ok(Some(empty_projection(
            target,
            as_of,
            query,
            CreatorLifecycleStatus::NotApplicable,
        )));
    }

    let rows = sqlx::query(CANDIDATE_WORKS_SQL)
        .bind(target.target_ref)
        .bind(&as_of)
        .bind((CREATOR_LIFECYCLE_SCAN_LIMIT + 1) as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_schema_error)?;
    tx.commit().await?;

    let mut candidates = rows
        .into_iter()
        .map(|row| CandidateWork {
            work_public_ref: row.get("work_public_ref"),
            title: row.get("title"),
            author_external_id: row.get("author_external_id"),
            published_at: row.get("published_at"),
            published_local_date: row.get("published_local_date"),
            published_at_epoch_ms: row.get("published_at_epoch_ms"),
            inside_recent_window: row.get("inside_recent_window"),
            like_count: row.get("like_count"),
            comment_count: row.get("comment_count"),
            collect_count: row.get("collect_count"),
            share_count: row.get("share_count"),
        })
        .collect::<Vec<_>>();
    let truncated = candidates.len() > CREATOR_LIFECYCLE_SCAN_LIMIT;
    candidates.truncate(CREATOR_LIFECYCLE_SCAN_LIMIT);
    let scanned_count = candidates.len();
    let mut confirmed_author_work_count = 0;
    let mut exclusions = CreatorLifecycleExclusions::empty();
    exclusions.scan_truncated = truncated;
    let mut points = Vec::new();
    for candidate in candidates {
        match candidate.author_external_id.as_deref() {
            None => {
                exclusions.author_not_verified += 1;
                continue;
            }
            Some(author) if author != target.identity_key => {
                exclusions.author_mismatch += 1;
                continue;
            }
            Some(_) => confirmed_author_work_count += 1,
        }
        let metric_value = metric_value(&candidate, query.metric);
        let (Some(published_at), Some(published_local_date), Some(published_at_epoch_ms)) = (
            candidate.published_at.as_ref(),
            candidate.published_local_date.as_ref(),
            candidate.published_at_epoch_ms,
        ) else {
            exclusions.published_at_not_qualified += 1;
            continue;
        };
        if query.window == CreatorLifecycleWindow::Recent90Days && !candidate.inside_recent_window {
            exclusions.outside_window += 1;
            continue;
        }
        let Some(metric_value) = metric_value else {
            exclusions.metric_unknown += 1;
            continue;
        };
        points.push(CreatorLifecyclePoint {
            work_public_ref: candidate.work_public_ref,
            title_state: if candidate.title.is_some() {
                "KNOWN"
            } else {
                "UNKNOWN"
            },
            title: candidate.title,
            published_at: published_at.clone(),
            published_local_date: published_local_date.clone(),
            published_at_epoch_ms,
            metric_value,
            creator_percentile: 0.0,
            rolling_median: 0.0,
        });
    }
    points.sort_by_key(|point| (point.published_at_epoch_ms, point.work_public_ref));
    add_analysis_values(
        &mut points,
        CreatorLifecycleAnalysis::current().rolling_median_window,
    );
    let status = if points.is_empty() {
        CreatorLifecycleStatus::InsufficientObservation
    } else {
        CreatorLifecycleStatus::Ready
    };
    Ok(Some(CreatorLifecycleProjection {
        target_ref: target.target_ref,
        target_kind: target.target_kind,
        status,
        as_of,
        window: query.window,
        metric: query.metric,
        summary: CreatorLifecycleSummary {
            linked_work_count: scanned_count,
            confirmed_author_work_count,
            eligible_point_count: points.len(),
        },
        exclusions,
        receipt: CreatorLifecycleReceipt {
            scan_limit: CREATOR_LIFECYCLE_SCAN_LIMIT,
            scanned_count,
            truncated,
        },
        analysis: CreatorLifecycleAnalysis::current(),
        points,
    }))
}

fn empty_projection(
    target: TargetIdentity,
    as_of: String,
    query: &CreatorLifecycleQuery,
    status: CreatorLifecycleStatus,
) -> CreatorLifecycleProjection {
    CreatorLifecycleProjection {
        target_ref: target.target_ref,
        target_kind: target.target_kind,
        status,
        as_of,
        window: query.window,
        metric: query.metric,
        summary: CreatorLifecycleSummary {
            linked_work_count: 0,
            confirmed_author_work_count: 0,
            eligible_point_count: 0,
        },
        exclusions: CreatorLifecycleExclusions::empty(),
        receipt: CreatorLifecycleReceipt {
            scan_limit: CREATOR_LIFECYCLE_SCAN_LIMIT,
            scanned_count: 0,
            truncated: false,
        },
        analysis: CreatorLifecycleAnalysis::current(),
        points: Vec::new(),
    }
}

fn metric_value(candidate: &CandidateWork, metric: CreatorLifecycleMetric) -> Option<i64> {
    match metric {
        CreatorLifecycleMetric::Likes => candidate.like_count,
        CreatorLifecycleMetric::Comments => candidate.comment_count,
        CreatorLifecycleMetric::Collects => candidate.collect_count,
        CreatorLifecycleMetric::Shares => candidate.share_count,
        CreatorLifecycleMetric::CompositeV1 => candidate
            .like_count?
            .checked_add(candidate.collect_count?.checked_mul(2)?)?
            .checked_add(candidate.comment_count?.checked_mul(3)?)?
            .checked_add(candidate.share_count?.checked_mul(4)?),
    }
}

fn add_analysis_values(points: &mut [CreatorLifecyclePoint], rolling_window: usize) {
    let all_values = points
        .iter()
        .map(|point| point.metric_value)
        .collect::<Vec<_>>();
    let point_count = all_values.len();
    for index in 0..points.len() {
        let at_or_below = all_values
            .iter()
            .filter(|value| **value <= points[index].metric_value)
            .count();
        points[index].creator_percentile = if point_count == 0 {
            0.0
        } else {
            at_or_below as f64 * 100.0 / point_count as f64
        };
        let start = (index + 1).saturating_sub(rolling_window);
        let mut trailing = all_values[start..=index].to_vec();
        trailing.sort_unstable();
        let middle = trailing.len() / 2;
        points[index].rolling_median = if trailing.len() % 2 == 0 {
            (trailing[middle - 1] as f64 + trailing[middle] as f64) / 2.0
        } else {
            trailing[middle] as f64
        };
    }
}

fn map_schema_error(error: sqlx::Error) -> CreatorLifecycleReadError {
    match &error {
        sqlx::Error::Database(database_error)
            if matches!(database_error.code().as_deref(), Some("42P01" | "42703")) =>
        {
            CreatorLifecycleReadError::ProjectionUnavailable
        }
        _ => CreatorLifecycleReadError::Database(error),
    }
}

const CANDIDATE_WORKS_SQL: &str = r#"
WITH selected_target AS (
    SELECT target_ref,platform,identity_key
    FROM collection_observation_target
    WHERE target_ref=$1 AND target_kind='creator'
),
surface_candidates AS (
    SELECT DISTINCT finding.content_public_ref
    FROM linggan_material_discovery_finding finding
    JOIN linggan_runtime_capture_package package USING (package_ref)
    JOIN linggan_runtime_task task ON task.task_id=package.task_id
    JOIN selected_target target
      ON target.platform=package.platform
     AND task.task_spec #>> '{target,authorExternalId}'=target.identity_key
    WHERE finding.discovery_kind='profile_discovery'
      AND finding.created_at <= $2::timestamptz
      AND package.accepted_at <= $2::timestamptz
),
author_candidates AS (
    SELECT DISTINCT detail.content_public_ref
    FROM linggan_material_content_detail detail
    JOIN linggan_material_content content ON content.public_ref=detail.content_public_ref
    JOIN selected_target target
      ON target.platform=content.platform
     AND target.identity_key=detail.author_external_id
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE package.accepted_at <= $2::timestamptz
),
candidates AS (
    SELECT content_public_ref FROM surface_candidates
    UNION
    SELECT content_public_ref FROM author_candidates
),
bounded_candidates AS (
    SELECT content.public_ref AS work_public_ref,content.created_at
    FROM candidates
    JOIN linggan_material_content content ON content.public_ref=candidates.content_public_ref
    JOIN selected_target target ON target.platform=content.platform
    ORDER BY content.created_at DESC,content.public_ref DESC
    LIMIT $3
)
SELECT bounded.work_public_ref,
       title.title,
       author.author_external_id,
       published.published_at::text AS published_at,
       (published.published_at AT TIME ZONE 'Asia/Shanghai')::date::text AS published_local_date,
       (extract(epoch FROM published.published_at)*1000)::bigint AS published_at_epoch_ms,
       COALESCE(
         (published.published_at AT TIME ZONE 'Asia/Shanghai')::date
           BETWEEN (($2::timestamptz AT TIME ZONE 'Asia/Shanghai')::date - 89)
               AND ($2::timestamptz AT TIME ZONE 'Asia/Shanghai')::date,
         false
       ) AS inside_recent_window,
       likes.value AS like_count,
       comments.value AS comment_count,
       collects.value AS collect_count,
       shares.value AS share_count
FROM bounded_candidates bounded
LEFT JOIN LATERAL (
    SELECT candidate.title
    FROM (
        SELECT detail.title,detail.observed_at,detail.created_at,1 AS source_priority,detail.package_ref
        FROM linggan_material_content_detail detail
        JOIN linggan_runtime_capture_package package USING (package_ref)
        WHERE detail.content_public_ref=bounded.work_public_ref
          AND package.accepted_at <= $2::timestamptz
          AND detail.title_state='KNOWN'
        UNION ALL
        SELECT finding.title,finding.observed_at,finding.created_at,0 AS source_priority,finding.package_ref
        FROM linggan_material_discovery_finding finding
        JOIN linggan_runtime_capture_package package USING (package_ref)
        WHERE finding.content_public_ref=bounded.work_public_ref
          AND package.accepted_at <= $2::timestamptz
          AND finding.title_state='KNOWN'
    ) candidate
    ORDER BY candidate.observed_at::timestamptz DESC,candidate.created_at DESC,candidate.source_priority DESC,candidate.package_ref DESC
    LIMIT 1
) title ON true
LEFT JOIN LATERAL (
    SELECT detail.author_external_id
    FROM linggan_material_content_detail detail
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE detail.content_public_ref=bounded.work_public_ref
      AND package.accepted_at <= $2::timestamptz
      AND detail.author_external_id IS NOT NULL
    ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC,detail.package_ref DESC
    LIMIT 1
) author ON true
LEFT JOIN LATERAL (
    SELECT detail.published_at
    FROM linggan_material_content_detail detail
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE detail.content_public_ref=bounded.work_public_ref
      AND package.accepted_at <= $2::timestamptz
      AND detail.published_at IS NOT NULL
    ORDER BY detail.observed_at::timestamptz DESC,detail.created_at DESC,detail.package_ref DESC
    LIMIT 1
) published ON true
LEFT JOIN LATERAL (
    SELECT observation.like_count AS value
    FROM linggan_material_engagement_observation observation
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE observation.content_public_ref=bounded.work_public_ref
      AND package.accepted_at <= $2::timestamptz
      AND observation.like_count_state='KNOWN'
    ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
    LIMIT 1
) likes ON true
LEFT JOIN LATERAL (
    SELECT observation.comment_count AS value
    FROM linggan_material_engagement_observation observation
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE observation.content_public_ref=bounded.work_public_ref
      AND package.accepted_at <= $2::timestamptz
      AND observation.comment_count_state='KNOWN'
    ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
    LIMIT 1
) comments ON true
LEFT JOIN LATERAL (
    SELECT observation.collect_count AS value
    FROM linggan_material_engagement_observation observation
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE observation.content_public_ref=bounded.work_public_ref
      AND package.accepted_at <= $2::timestamptz
      AND observation.collect_count_state='KNOWN'
    ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
    LIMIT 1
) collects ON true
LEFT JOIN LATERAL (
    SELECT observation.share_count AS value
    FROM linggan_material_engagement_observation observation
    JOIN linggan_runtime_capture_package package USING (package_ref)
    WHERE observation.content_public_ref=bounded.work_public_ref
      AND package.accepted_at <= $2::timestamptz
      AND observation.share_count_state='KNOWN'
    ORDER BY observation.observed_at::timestamptz DESC,observation.created_at DESC,(observation.source_lane='detail') DESC,observation.package_ref DESC
    LIMIT 1
) shares ON true
ORDER BY bounded.created_at DESC,bounded.work_public_ref DESC
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_query_enums_reject_unknown_values() {
        assert_eq!(
            CreatorLifecycleWindow::parse("recent_90_days"),
            Some(CreatorLifecycleWindow::Recent90Days)
        );
        assert_eq!(
            CreatorLifecycleWindow::parse("all"),
            Some(CreatorLifecycleWindow::All)
        );
        assert_eq!(CreatorLifecycleWindow::parse("last_2160_hours"), None);
        assert_eq!(
            CreatorLifecycleMetric::parse("composite_v1"),
            Some(CreatorLifecycleMetric::CompositeV1)
        );
        assert_eq!(CreatorLifecycleMetric::parse("monitoring_value"), None);
    }

    #[test]
    fn composite_requires_every_known_field_and_checks_overflow() {
        let candidate = candidate_with_counts(Some(10), Some(2), Some(3), Some(4));
        assert_eq!(
            metric_value(&candidate, CreatorLifecycleMetric::CompositeV1),
            Some(38)
        );
        let unknown_share = candidate_with_counts(Some(10), Some(2), Some(3), None);
        assert_eq!(
            metric_value(&unknown_share, CreatorLifecycleMetric::CompositeV1),
            None
        );
        let overflowing = candidate_with_counts(Some(i64::MAX), Some(1), Some(1), Some(1));
        assert_eq!(
            metric_value(&overflowing, CreatorLifecycleMetric::CompositeV1),
            None
        );
    }

    #[test]
    fn server_analysis_owns_the_trailing_five_work_median() {
        let mut points = [1, 100, 2, 99, 3, 98]
            .into_iter()
            .enumerate()
            .map(|(index, metric_value)| CreatorLifecyclePoint {
                work_public_ref: Uuid::from_u128(index as u128 + 1),
                title: None,
                title_state: "UNKNOWN",
                published_at: format!("2026-08-{:02} 00:00:00+00", index + 1),
                published_local_date: format!("2026-08-{:02}", index + 1),
                published_at_epoch_ms: index as i64,
                metric_value,
                creator_percentile: 0.0,
                rolling_median: 0.0,
            })
            .collect::<Vec<_>>();
        let analysis = CreatorLifecycleAnalysis::current();
        assert_eq!(analysis.rolling_median_version, "trailing-5-work-median-v1");
        assert_eq!(analysis.rolling_median_window, 5);
        add_analysis_values(&mut points, analysis.rolling_median_window);
        assert_eq!(
            points
                .iter()
                .map(|point| point.rolling_median)
                .collect::<Vec<_>>(),
            vec![1.0, 50.5, 2.0, 50.5, 3.0, 98.0]
        );
    }

    fn candidate_with_counts(
        like_count: Option<i64>,
        comment_count: Option<i64>,
        collect_count: Option<i64>,
        share_count: Option<i64>,
    ) -> CandidateWork {
        CandidateWork {
            work_public_ref: Uuid::nil(),
            title: None,
            author_external_id: None,
            published_at: None,
            published_local_date: None,
            published_at_epoch_ms: None,
            inside_recent_window: false,
            like_count,
            comment_count,
            collect_count,
            share_count,
        }
    }
}
