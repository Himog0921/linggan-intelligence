//! Read-only observation-volume projection for the comment-study overview.
//!
//! This module deliberately keeps three clocks apart:
//! - the first accepted observation of a stable platform comment;
//! - the day a comment entered a StudyRun as a Target;
//! - the day an accepted Signal was written.
//!
//! None of these numbers is labelled as demand, quality or prevalence. A day without an
//! observation ledger is also not treated as proof that users said nothing.

use crate::comment_study_read::{self, CommentStudyReadError};
use linggan_storage_postgres::Database;
use serde_json::Value;
use uuid::Uuid;

pub const COMMENT_OBSERVATION_SERIES_DAYS: i32 = 56;
pub const COMMENT_OBSERVATION_TIMEZONE: &str = "Asia/Shanghai";

/// Returns a fixed 56-day server-side aggregate. The browser may show the trailing 7, 28 or 56
/// points, but it never rebuilds historical counts from paginated Run reads.
pub async fn read_comment_observation_series(
    database: &Database,
    domain_ref: Uuid,
) -> Result<Value, CommentStudyReadError> {
    if !comment_study_read::schema_ready(database).await? {
        return Err(CommentStudyReadError::SchemaUnavailable);
    }

    let domain_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)",
    )
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await?;
    if !domain_exists {
        return Err(CommentStudyReadError::InvalidQuery);
    }

    let projection: Value = sqlx::query_scalar(
        "WITH params AS ( \
           SELECT $1::uuid AS domain_ref,$2::integer AS window_days, \
                  (scope_001_now() AT TIME ZONE $3)::date AS end_date \
         ), days AS ( \
           SELECT generate_series( \
                    params.end_date-(params.window_days-1),params.end_date,'1 day'::interval \
                  )::date AS day \
           FROM params \
         ), first_comment_observation AS ( \
           SELECT comment.content_public_ref,comment.comment_external_id, \
                  min(package.accepted_at) AS first_observed_at \
           FROM linggan_material_comment comment \
           JOIN linggan_runtime_capture_package package USING(package_ref) \
           JOIN linggan_material_content content ON content.public_ref=comment.content_public_ref \
           JOIN params ON params.domain_ref=content.domain_ref \
           GROUP BY comment.content_public_ref,comment.comment_external_id \
         ), new_comments AS ( \
           SELECT (first_observed_at AT TIME ZONE $3)::date AS day, \
                  count(*)::bigint AS comment_count \
           FROM first_comment_observation,params \
           WHERE first_observed_at >= (params.end_date-(params.window_days-1))::timestamp AT TIME ZONE $3 \
             AND first_observed_at < (params.end_date+1)::timestamp AT TIME ZONE $3 \
           GROUP BY 1 \
         ), study_targets AS ( \
           SELECT (target.created_at AT TIME ZONE $3)::date AS day, \
                  count(DISTINCT target.source_ref)::bigint AS target_count \
           FROM linggan_comment_study_target target \
           JOIN linggan_comment_study_run run USING(run_ref) \
           JOIN linggan_comment_study_policy policy USING(policy_ref) \
           JOIN params ON params.domain_ref=policy.domain_ref \
           WHERE target.created_at >= (params.end_date-(params.window_days-1))::timestamp AT TIME ZONE $3 \
             AND target.created_at < (params.end_date+1)::timestamp AT TIME ZONE $3 \
           GROUP BY 1 \
         ), accepted_signals AS ( \
           SELECT (signal.created_at AT TIME ZONE $3)::date AS day, \
                  count(*)::bigint AS signal_count \
           FROM linggan_comment_study_signal signal \
           JOIN linggan_comment_study_target target USING(target_ref) \
           JOIN linggan_comment_study_run run USING(run_ref) \
           JOIN linggan_comment_study_policy policy USING(policy_ref) \
           JOIN params ON params.domain_ref=policy.domain_ref \
           WHERE signal.created_at >= (params.end_date-(params.window_days-1))::timestamp AT TIME ZONE $3 \
             AND signal.created_at < (params.end_date+1)::timestamp AT TIME ZONE $3 \
           GROUP BY 1 \
         ), covered_works AS ( \
           SELECT activity.day,count(DISTINCT activity.content_public_ref)::bigint AS work_count \
           FROM ( \
             SELECT (first_observed_at AT TIME ZONE $3)::date AS day,content_public_ref \
             FROM first_comment_observation,params \
             WHERE first_observed_at >= (params.end_date-(params.window_days-1))::timestamp AT TIME ZONE $3 \
               AND first_observed_at < (params.end_date+1)::timestamp AT TIME ZONE $3 \
             UNION ALL \
             SELECT (target.created_at AT TIME ZONE $3)::date,target.content_public_ref \
             FROM linggan_comment_study_target target \
             JOIN linggan_comment_study_run run USING(run_ref) \
             JOIN linggan_comment_study_policy policy USING(policy_ref) \
             JOIN params ON params.domain_ref=policy.domain_ref \
             WHERE target.created_at >= (params.end_date-(params.window_days-1))::timestamp AT TIME ZONE $3 \
               AND target.created_at < (params.end_date+1)::timestamp AT TIME ZONE $3 \
           ) activity \
           GROUP BY activity.day \
         ), observation_coverage AS ( \
           SELECT (lane.created_at AT TIME ZONE $3)::date AS day, \
                  count(*)::bigint AS ledger_count, \
                  bool_or( \
                    COALESCE(lane.failed,0)>0 \
                    OR COALESCE(lane.known_unattempted,0)>0 \
                    OR COALESCE(lane.unknown_count,0)>0 \
                    OR lane.stopped_reason IS NOT NULL \
                  ) AS has_known_gap \
           FROM linggan_material_lane_observation lane \
           JOIN linggan_material_content content ON content.public_ref=lane.content_public_ref \
           JOIN params ON params.domain_ref=content.domain_ref \
           WHERE lane.lane IN ('comments','replies') \
             AND lane.created_at >= (params.end_date-(params.window_days-1))::timestamp AT TIME ZONE $3 \
             AND lane.created_at < (params.end_date+1)::timestamp AT TIME ZONE $3 \
           GROUP BY 1 \
         ) \
         SELECT jsonb_build_object( \
           'contract','comment-study.observation-series.v1', \
           'domainRef',(SELECT domain_ref FROM params), \
           'days',(SELECT window_days FROM params), \
           'timezone',$3, \
           'points',COALESCE(jsonb_agg(jsonb_build_object( \
             'date',to_char(days.day,'YYYY-MM-DD'), \
             'newObservedCommentCount',COALESCE(new_comments.comment_count,0), \
             'studiedCommentCount',COALESCE(study_targets.target_count,0), \
             'acceptedSignalCount',COALESCE(accepted_signals.signal_count,0), \
             'coveredWorkCount',COALESCE(covered_works.work_count,0), \
             'observationLedgerCount',COALESCE(observation_coverage.ledger_count,0), \
             'observationCoverage',CASE \
               WHEN observation_coverage.ledger_count IS NULL THEN 'none' \
               WHEN observation_coverage.has_known_gap THEN 'partial' \
               ELSE 'recorded' END \
           ) ORDER BY days.day),'[]'::jsonb) \
         ) \
         FROM days \
         LEFT JOIN new_comments USING(day) \
         LEFT JOIN study_targets USING(day) \
         LEFT JOIN accepted_signals USING(day) \
         LEFT JOIN covered_works USING(day) \
         LEFT JOIN observation_coverage USING(day)",
    )
    .bind(domain_ref)
    .bind(COMMENT_OBSERVATION_SERIES_DAYS)
    .bind(COMMENT_OBSERVATION_TIMEZONE)
    .fetch_one(database.pool())
    .await?;

    Ok(projection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overview_window_is_fixed_and_client_ranges_are_subsets() {
        assert_eq!(COMMENT_OBSERVATION_SERIES_DAYS, 56);
        for requested in [7, 28, 56] {
            assert!(requested <= COMMENT_OBSERVATION_SERIES_DAYS);
        }
    }

    #[test]
    fn the_projection_uses_an_explicit_product_timezone() {
        assert_eq!(COMMENT_OBSERVATION_TIMEZONE, "Asia/Shanghai");
    }
}
