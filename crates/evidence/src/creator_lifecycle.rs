//! Target-scoped, reconstructable creator-work lifecycle projection.
//!
//! A lifecycle point is not a second material fact. It is a read-time combination of one stable
//! Work, a target-scoped directory or exact-author association, a producer-qualified publication
//! instant and the latest `KNOWN` engagement value at one repeatable-read `as_of`.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

use crate::work_resource_current::read_work_resource_currents;

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
}

impl CreatorLifecycleMetric {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "likes" => Some(Self::Likes),
            "comments" => Some(Self::Comments),
            "collects" => Some(Self::Collects),
            "shares" => Some(Self::Shares),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Likes => "likes",
            Self::Comments => "comments",
            Self::Collects => "collects",
            Self::Shares => "shares",
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
            window: CreatorLifecycleWindow::All,
            metric: CreatorLifecycleMetric::Likes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CreatorLifecycleQueryError {
    #[error("creator lifecycle window is not in the closed query contract")]
    Window,
    #[error("creator lifecycle metric is not in the closed query contract")]
    Metric,
}

impl CreatorLifecycleQuery {
    /// Applies defaults only when a parameter is absent. A present value outside the closed
    /// contract remains an error so HTML and JSON consumers cannot silently claim a different
    /// window or metric than the caller requested.
    pub fn parse_optional(
        window: Option<&str>,
        metric: Option<&str>,
    ) -> Result<Self, CreatorLifecycleQueryError> {
        let window = match window {
            Some(value) => {
                CreatorLifecycleWindow::parse(value).ok_or(CreatorLifecycleQueryError::Window)?
            }
            None => CreatorLifecycleWindow::All,
        };
        let metric = match metric {
            Some(value) => {
                CreatorLifecycleMetric::parse(value).ok_or(CreatorLifecycleQueryError::Metric)?
            }
            None => CreatorLifecycleMetric::Likes,
        };
        Ok(Self { window, metric })
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
    /// Exact only when the 2000 + 1 probe did not observe truncation.
    pub linked_work_count: Option<usize>,
    /// Always known. When truncated this is the conservative `scan_limit + 1` lower bound.
    pub linked_work_count_lower_bound: usize,
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
    pub probed_count: usize,
    pub scanned_count: usize,
    pub returned_count: usize,
    pub truncated: bool,
}

/// Why one Work is allowed to appear in this creator's distribution.
///
/// Directory association is weaker than an exact detail-author match, but it is still a durable
/// target-scoped fact: this Work was found by a profile discovery Work Order owned by this target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CreatorLifecycleAssociation {
    DirectoryLinked,
    AuthorConfirmed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreatorLifecyclePoint {
    pub work_public_ref: Uuid,
    pub title: Option<String>,
    pub title_state: &'static str,
    pub published_at: String,
    pub published_local_date: String,
    pub published_at_epoch_ms: i64,
    pub metric_value: i64,
    pub association_state: CreatorLifecycleAssociation,
    pub new_in_latest_patrol: bool,
}

#[derive(Clone, Debug, PartialEq)]
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

struct LifecycleClock {
    as_of: String,
    recent_start_local_date: String,
    as_of_local_date: String,
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
    let clock = sqlx::query(
        "WITH clock AS (SELECT scope_001_now() AS as_of) \
         SELECT as_of::text AS as_of, \
                ((as_of AT TIME ZONE 'Asia/Shanghai')::date - 89)::text AS recent_start_local_date, \
                (as_of AT TIME ZONE 'Asia/Shanghai')::date::text AS as_of_local_date \
         FROM clock",
    )
        .fetch_one(&mut *tx)
        .await
        .map(|row| LifecycleClock {
            as_of: row.get("as_of"),
            recent_start_local_date: row.get("recent_start_local_date"),
            as_of_local_date: row.get("as_of_local_date"),
        })?;
    let target = sqlx::query(
        "SELECT target_ref,target_kind,identity_key \
         FROM collection_observation_target \
         WHERE target_ref=$1 AND first_stored_at <= $2::timestamptz",
    )
    .bind(target_ref)
    .bind(&clock.as_of)
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
            clock.as_of,
            query,
            CreatorLifecycleStatus::NotApplicable,
        )));
    }

    let rows = sqlx::query(CANDIDATE_WORK_REFS_SQL)
        .bind(target.target_ref)
        .bind(&clock.as_of)
        .bind((CREATOR_LIFECYCLE_SCAN_LIMIT + 1) as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_schema_error)?;
    let mut candidate_membership = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<Uuid, _>("work_public_ref"),
                (
                    row.get::<bool, _>("surface_linked"),
                    row.get::<bool, _>("new_in_latest_patrol"),
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut candidate_refs = candidate_membership
        .iter()
        .map(|(work_public_ref, _)| *work_public_ref)
        .collect::<Vec<_>>();
    let probed_count = candidate_refs.len();
    let truncated = probed_count > CREATOR_LIFECYCLE_SCAN_LIMIT;
    candidate_refs.truncate(CREATOR_LIFECYCLE_SCAN_LIMIT);
    candidate_membership.truncate(CREATOR_LIFECYCLE_SCAN_LIMIT);
    let membership = candidate_membership.into_iter().collect::<HashMap<_, _>>();
    let scanned_count = candidate_refs.len();
    let mut currents = read_work_resource_currents(&mut tx, &candidate_refs, &clock.as_of)
        .await
        .map_err(map_schema_error)?
        .into_iter()
        .map(|current| (current.public_ref, current))
        .collect::<HashMap<_, _>>();
    tx.commit().await?;
    let mut confirmed_author_work_count = 0;
    let mut exclusions = CreatorLifecycleExclusions::empty();
    exclusions.scan_truncated = truncated;
    let mut points = Vec::new();
    for work_public_ref in candidate_refs {
        let Some(candidate) = currents.remove(&work_public_ref) else {
            continue;
        };
        let (surface_linked, new_in_latest_patrol) = membership
            .get(&work_public_ref)
            .copied()
            .unwrap_or((false, false));
        let association_state = match candidate.author_external_id.as_deref() {
            None if surface_linked => CreatorLifecycleAssociation::DirectoryLinked,
            None => {
                exclusions.author_not_verified += 1;
                continue;
            }
            Some(author) if author != target.identity_key => {
                exclusions.author_mismatch += 1;
                continue;
            }
            Some(_) => {
                confirmed_author_work_count += 1;
                CreatorLifecycleAssociation::AuthorConfirmed
            }
        };
        let metric_value = metric_value(
            candidate.like_count,
            candidate.comment_count,
            candidate.collect_count,
            candidate.share_count,
            query.metric,
        );
        let (Some(published_at), Some(published_local_date), Some(published_at_epoch_ms)) = (
            candidate.published_at.as_ref(),
            candidate.published_local_date.as_ref(),
            candidate.published_at_epoch_ms,
        ) else {
            exclusions.published_at_not_qualified += 1;
            continue;
        };
        if query.window == CreatorLifecycleWindow::Recent90Days
            && !(clock.recent_start_local_date.as_str() <= published_local_date.as_str()
                && published_local_date.as_str() <= clock.as_of_local_date.as_str())
        {
            exclusions.outside_window += 1;
            continue;
        }
        let Some(metric_value) = metric_value else {
            exclusions.metric_unknown += 1;
            continue;
        };
        points.push(CreatorLifecyclePoint {
            work_public_ref: candidate.public_ref,
            title_state: if candidate.title_state == "KNOWN" {
                "KNOWN"
            } else {
                "UNKNOWN"
            },
            title: candidate.title,
            published_at: published_at.clone(),
            published_local_date: published_local_date.clone(),
            published_at_epoch_ms,
            metric_value,
            association_state,
            new_in_latest_patrol,
        });
    }
    points.sort_by_key(|point| (point.published_at_epoch_ms, point.work_public_ref));
    let status = if points.is_empty() {
        CreatorLifecycleStatus::InsufficientObservation
    } else {
        CreatorLifecycleStatus::Ready
    };
    Ok(Some(CreatorLifecycleProjection {
        target_ref: target.target_ref,
        target_kind: target.target_kind,
        status,
        as_of: clock.as_of,
        window: query.window,
        metric: query.metric,
        summary: CreatorLifecycleSummary {
            linked_work_count: (!truncated).then_some(probed_count),
            linked_work_count_lower_bound: probed_count,
            confirmed_author_work_count,
            eligible_point_count: points.len(),
        },
        exclusions,
        receipt: CreatorLifecycleReceipt {
            scan_limit: CREATOR_LIFECYCLE_SCAN_LIMIT,
            probed_count,
            scanned_count,
            returned_count: points.len(),
            truncated,
        },
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
            linked_work_count: Some(0),
            linked_work_count_lower_bound: 0,
            confirmed_author_work_count: 0,
            eligible_point_count: 0,
        },
        exclusions: CreatorLifecycleExclusions::empty(),
        receipt: CreatorLifecycleReceipt {
            scan_limit: CREATOR_LIFECYCLE_SCAN_LIMIT,
            probed_count: 0,
            scanned_count: 0,
            returned_count: 0,
            truncated: false,
        },
        points: Vec::new(),
    }
}

fn metric_value(
    like_count: Option<i64>,
    comment_count: Option<i64>,
    collect_count: Option<i64>,
    share_count: Option<i64>,
    metric: CreatorLifecycleMetric,
) -> Option<i64> {
    match metric {
        CreatorLifecycleMetric::Likes => like_count,
        CreatorLifecycleMetric::Comments => comment_count,
        CreatorLifecycleMetric::Collects => collect_count,
        CreatorLifecycleMetric::Shares => share_count,
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

const CANDIDATE_WORK_REFS_SQL: &str = r#"
WITH selected_target AS (
    SELECT target_ref,platform,identity_key
    FROM collection_observation_target
    WHERE target_ref=$1 AND target_kind='creator'
),
qualified_patrols AS (
    SELECT work_order.work_order_ref,package.accepted_at,package.package_ref
    FROM selected_target target
    JOIN collection_work_order work_order ON work_order.target_ref=target.target_ref
    JOIN collection_work_order_lease lease USING(work_order_ref)
    JOIN collection_work_order_lease_task lease_task USING(lease_ref)
    JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
    JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
    JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
    CROSS JOIN LATERAL jsonb_array_elements(
      CASE WHEN jsonb_typeof(package.coverage->'layers')='array'
           THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer
    WHERE work_order.lane='patrol'
      AND package.package_kind='profile_discovery'
      AND package.accepted_at <= $2::timestamptz
      AND package.platform=task.platform
      AND task.task_spec->'capabilitiesRequested' ? package.package_kind
      AND (package.coverage->'target') @> (task.task_spec->'target')
      AND receipt.material_admission='ACCEPTED'
      AND receipt.execution_effect='COMPLETED_LIVE_STEP'
      AND layer->>'capability'='profile_discovery'
      AND COALESCE((layer->>'failed')::integer,0)=0
      AND COALESCE((layer->>'notAttempted')::integer,0)=0
      AND COALESCE((layer->>'unknown')::integer,0)=0
      AND (layer->>'stoppedReason'='surface_ended' OR (
           layer->>'stoppedReason'='maximum_quota'
           AND COALESCE((layer->>'acquired')::integer,-1)=
               COALESCE((task.task_spec->>'maximumQuota')::integer,-2)))
      AND NOT EXISTS (
        SELECT 1 FROM linggan_runtime_record_disposition disposition
        WHERE disposition.package_ref=package.package_ref
          AND disposition.disposition='quarantined'
      )
      AND (SELECT count(*) FROM linggan_runtime_record_disposition disposition
           WHERE disposition.package_ref=package.package_ref
             AND disposition.disposition='accepted_for_library_discovery') =
          COALESCE((layer->>'acquired')::integer,-1)
),
surface_observations AS (
    SELECT finding.content_public_ref,work_order.work_order_ref,package.accepted_at,
           package.package_ref,receipt.execution_effect
    FROM linggan_material_discovery_finding finding
    JOIN linggan_runtime_capture_package package USING (package_ref)
    JOIN linggan_runtime_submission_receipt receipt USING (package_ref)
    JOIN linggan_runtime_record_disposition disposition
      ON disposition.package_ref=finding.package_ref
     AND disposition.record_ordinal=finding.record_ordinal
    JOIN linggan_runtime_task task ON task.task_id=package.task_id
    LEFT JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id
    LEFT JOIN collection_work_order_lease lease USING (lease_ref)
    LEFT JOIN collection_work_order work_order USING (work_order_ref)
    JOIN selected_target target
      ON target.platform=package.platform
     AND (work_order.target_ref=target.target_ref
          OR task.task_spec #>> '{target,authorExternalId}'=target.identity_key)
    WHERE finding.discovery_kind='profile_discovery'
      AND finding.created_at <= $2::timestamptz
      AND package.accepted_at <= $2::timestamptz
      AND receipt.material_admission='ACCEPTED'
      AND disposition.disposition <> 'quarantined'
),
latest_patrol AS (
    SELECT patrol.work_order_ref
    FROM qualified_patrols patrol
    ORDER BY patrol.accepted_at DESC,patrol.package_ref DESC
    LIMIT 1
),
surface_candidates AS (
    SELECT observation.content_public_ref,
           true AS surface_linked,
           COALESCE(
             (array_agg(observation.work_order_ref
                        ORDER BY observation.accepted_at,observation.package_ref))[1]
               = (SELECT work_order_ref FROM latest_patrol),
             false
           ) AS new_in_latest_patrol
    FROM surface_observations observation
    GROUP BY observation.content_public_ref
),
author_candidates AS (
    SELECT DISTINCT detail.content_public_ref
    FROM linggan_material_content_detail detail
    JOIN linggan_material_content content ON content.public_ref=detail.content_public_ref
    JOIN selected_target target
      ON target.platform=content.platform
     AND target.identity_key=detail.author_external_id
    JOIN linggan_runtime_capture_package package USING (package_ref)
    JOIN linggan_runtime_submission_receipt receipt USING (package_ref)
    JOIN linggan_runtime_record_disposition disposition
      ON disposition.package_ref=detail.package_ref
     AND disposition.record_ordinal=detail.record_ordinal
    WHERE package.accepted_at <= $2::timestamptz
      AND receipt.material_admission='ACCEPTED'
      AND disposition.disposition <> 'quarantined'
),
candidates AS (
    SELECT content_public_ref,surface_linked,new_in_latest_patrol FROM surface_candidates
    UNION ALL
    SELECT author.content_public_ref,false,false
    FROM author_candidates author
    WHERE NOT EXISTS (
        SELECT 1 FROM surface_candidates surface
        WHERE surface.content_public_ref=author.content_public_ref
    )
),
bounded_candidates AS (
    SELECT content.public_ref AS work_public_ref,content.created_at,
           candidates.surface_linked,candidates.new_in_latest_patrol
    FROM candidates
    JOIN linggan_material_content content ON content.public_ref=candidates.content_public_ref
    JOIN selected_target target ON target.platform=content.platform
    ORDER BY content.created_at DESC,content.public_ref DESC
    LIMIT $3
)
SELECT bounded.work_public_ref,bounded.surface_linked,bounded.new_in_latest_patrol
FROM bounded_candidates bounded
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
        assert_eq!(CreatorLifecycleMetric::parse("composite_v1"), None);
        assert_eq!(CreatorLifecycleMetric::parse("monitoring_value"), None);
    }

    #[test]
    fn lifecycle_query_distinguishes_absent_defaults_from_invalid_values() {
        assert_eq!(
            CreatorLifecycleQuery::parse_optional(None, None),
            Ok(CreatorLifecycleQuery::default())
        );
        assert_eq!(
            CreatorLifecycleQuery::parse_optional(Some("all"), Some("comments")),
            Ok(CreatorLifecycleQuery {
                window: CreatorLifecycleWindow::All,
                metric: CreatorLifecycleMetric::Comments,
            })
        );
        assert_eq!(
            CreatorLifecycleQuery::parse_optional(Some("last_2160_hours"), None),
            Err(CreatorLifecycleQueryError::Window)
        );
        assert_eq!(
            CreatorLifecycleQuery::parse_optional(None, Some("monitoring_value")),
            Err(CreatorLifecycleQueryError::Metric)
        );
    }
}
