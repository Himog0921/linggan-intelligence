//! T0.2/T0.3 proof: parameter loading, the four §6.4.3 period cases, and the §8.2 eligibility
//! dataset. Everything runs in an isolated PostgreSQL schema; no shared database is touched.

#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema_fixture;
#[path = "support/opportunity_dataset.rs"]
mod opportunity_dataset;

use linggan_intelligence::opportunity::{
    Period, PeriodBasis, WeekStart, check_invariants, load_current_metric_definition,
    project_comment_platform_times, read_platform_time_report,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use opportunity_dataset::{AS_OF, BASIS, DOMAIN, TZ};
use sqlx::Row;
use std::collections::HashMap;
use uuid::Uuid;

const MIGRATION: &str =
    include_str!("../../../database/migrations/0203_opportunity_metric_definition.sql");

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
    isolated_proof_schema(&url, schema, MIGRATION)
        .await
        .expect("proof schema migration")
}

async fn full_database(schema: &str) -> Database {
    let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
    isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .expect("full proof schema migration")
}

async fn eligible_count(database: &Database, as_of: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_eligible_comments($1::uuid, $2::timestamptz, $3, $4)",
    )
    .bind(DOMAIN)
    .bind(as_of)
    .bind(TZ)
    .bind(BASIS)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

async fn exclusion_counts(database: &Database, as_of: &str) -> HashMap<String, i64> {
    exclusion_counts_for(database, as_of, BASIS).await
}

async fn exclusion_counts_for(
    database: &Database,
    as_of: &str,
    basis: &str,
) -> HashMap<String, i64> {
    sqlx::query(
        "SELECT reason, comments FROM linggan_opportunity_eligibility_exclusions($1::uuid, $2::timestamptz, $3, $4)",
    )
    .bind(DOMAIN)
    .bind(as_of)
    .bind(TZ)
    .bind(basis)
    .fetch_all(database.pool())
    .await
    .unwrap()
    .into_iter()
    .map(|row| (row.get("reason"), row.get("comments")))
    .collect()
}

async fn eligible_count_for(database: &Database, as_of: &str, basis: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_eligible_comments($1::uuid, $2::timestamptz, $3, $4)",
    )
    .bind(DOMAIN)
    .bind(as_of)
    .bind(TZ)
    .bind(basis)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

async fn dataset_fingerprint(database: &Database) -> String {
    sqlx::query_scalar(
        r#"
WITH lines AS (
 SELECT row_to_json(seed)::text AS line FROM linggan_material_content seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_material_content_detail seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_material_domain_usage seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_material_comment seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_material_comment_first_admission seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_material_comment_restriction seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_runtime_task seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_runtime_attempt seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_runtime_capture_package seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_runtime_record_disposition seed
 UNION ALL
 SELECT row_to_json(seed)::text FROM linggan_opportunity_comment_fact seed
)
SELECT md5(string_agg(line, E'\n' ORDER BY line)) FROM lines
"#,
    )
    .fetch_one(database.pool())
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn standard_period_cases_match_the_playbook() {
    let database = proof_database("opportunity_t03_periods").await;
    let tz: String = sqlx::query_scalar(
        "SELECT params->>'period_tz' FROM linggan_opportunity_metric_definition WHERE version = 1",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let cases = [
        (
            "2026-11-01T15:59:59Z",
            "2026-10-26",
            "2026-10-25T16:00:00Z",
            "2026-11-01T16:00:00Z",
        ),
        (
            "2026-11-01T16:00:00Z",
            "2026-11-02",
            "2026-11-01T16:00:00Z",
            "2026-11-08T16:00:00Z",
        ),
        (
            "2026-11-08T15:59:59Z",
            "2026-11-02",
            "2026-11-01T16:00:00Z",
            "2026-11-08T16:00:00Z",
        ),
        (
            "2026-11-08T16:00:00Z",
            "2026-11-09",
            "2026-11-08T16:00:00Z",
            "2026-11-15T16:00:00Z",
        ),
    ];
    for (published_at, expected_start, expected_start_utc, expected_end_utc) in cases {
        let period_start: String = sqlx::query_scalar(
            "SELECT linggan_opportunity_period_start($1::timestamptz, $2)::text",
        )
        .bind(published_at)
        .bind(&tz)
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!(
            period_start, expected_start,
            "period_start for {published_at}"
        );

        let row = sqlx::query(
            "SELECT to_char(start_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS start_utc,
                    to_char(end_utc AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS end_utc
             FROM linggan_opportunity_period_bounds($1::date, $2)",
        )
        .bind(expected_start)
        .bind(&tz)
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("start_utc"), expected_start_utc);
        assert_eq!(row.get::<String, _>("end_utc"), expected_end_utc);
    }
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn seeded_definition_loads_and_invalid_parameters_are_refused() {
    let database = proof_database("opportunity_t03_loader").await;

    let definition = load_current_metric_definition(&database)
        .await
        .expect("seeded definition must load");
    assert_eq!(definition.version, 1);
    assert_eq!(definition.params.period, Period::Week);
    assert_eq!(definition.params.week_start, WeekStart::Monday);
    assert_eq!(
        definition.params.period_basis,
        PeriodBasis::WorkPublishedCohort
    );

    sqlx::query(
        "UPDATE linggan_opportunity_metric_definition
         SET params = jsonb_set(params, '{z}', '0'::jsonb)
         WHERE version = 1",
    )
    .execute(database.pool())
    .await
    .unwrap();
    assert!(load_current_metric_definition(&database).await.is_err());
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn playbook_dataset_matches_eligibility_counts_and_the_equality() {
    let database = full_database("opportunity_t02_dataset").await;
    opportunity_dataset::build(&database).await.unwrap();

    assert_eq!(eligible_count(&database, AS_OF).await, 5800);
    let exclusions = exclusion_counts(&database, AS_OF).await;
    assert_eq!(exclusions.get("not_in_domain_primary"), Some(&3));
    assert_eq!(exclusions.get("work_author_unknown"), Some(&5));
    assert_eq!(exclusions.get("period_anchor_missing"), Some(&4));
    assert_eq!(exclusions.get("restricted"), Some(&2));
    assert_eq!(exclusions.get("creator_self"), Some(&1));
    assert_eq!(exclusions.get("observed_at_unparseable"), None);

    let candidates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_comment_eligibility($1::uuid, $2::timestamptz, $3, $4)",
    )
    .bind(DOMAIN)
    .bind(AS_OF)
    .bind(TZ)
    .bind(BASIS)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(candidates, 5815);
    assert_eq!(candidates, 5800 + exclusions.values().copied().sum::<i64>());

    let backfilled: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_eligible_comments($1::uuid, $2::timestamptz, $3, $4) eligible
         JOIN linggan_opportunity_comment_fact fact USING (content_ref, comment_external_id)
         WHERE fact.first_observed_backfilled",
    )
    .bind(DOMAIN)
    .bind(AS_OF)
    .bind(TZ)
    .bind(BASIS)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(backfilled, 10);

    let replies: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_eligible_comments($1::uuid, $2::timestamptz, $3, $4) eligible
         JOIN linggan_opportunity_comment_fact fact USING (content_ref, comment_external_id)
         WHERE fact.is_reply",
    )
    .bind(DOMAIN)
    .bind(AS_OF)
    .bind(TZ)
    .bind(BASIS)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(replies, 50);

    let conflicts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_comment_fact WHERE commenter_external_id_conflict",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(conflicts, 0);

    assert_eq!(
        eligible_count(&database, "2026-11-11T00:00:00Z").await,
        5803
    );

    sqlx::raw_sql(
        r#"
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason)
SELECT package_ref, 2000, 'accepted_for_library_content', 'seed'
FROM opportunity_seed_package WHERE work_key = 'work-0' AND kind = 'comments';
INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, parent_comment_external_id, parent_identity_source_field,
 is_reply, body_text, body_state, author_external_id, observed_at
)
SELECT gen_random_uuid(), work.work_ref, package.package_ref, 2000, 'c-work-0-bad-time',
       'c-work-0-bad-time', NULL, NULL, false, NULL, 'UNKNOWN', 'commenter-bad-time', 'not-a-time'
FROM opportunity_seed_work work
JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'comments'
WHERE work.work_key = 'work-0';
ALTER TABLE linggan_material_comment_first_admission DISABLE TRIGGER USER;
DELETE FROM linggan_material_comment_first_admission
WHERE content_public_ref = '00000000-0000-4000-8000-000000000100'
  AND comment_external_id = 'c-work-0-bad-time';
ALTER TABLE linggan_material_comment_first_admission ENABLE TRIGGER USER;
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();
    let exclusions = exclusion_counts(&database, AS_OF).await;
    assert_eq!(exclusions.get("observed_at_unparseable"), Some(&1));
    assert_eq!(eligible_count(&database, AS_OF).await, 5800);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn comment_published_basis_uses_platform_time_and_never_falls_back() {
    let database = full_database("opportunity_c3_basis").await;
    opportunity_dataset::build(&database).await.unwrap();

    assert_eq!(eligible_count(&database, AS_OF).await, 5800);
    assert_eq!(
        eligible_count_for(&database, AS_OF, "comment_published").await,
        0
    );
    let exclusions = exclusion_counts_for(&database, AS_OF, "comment_published").await;
    assert_eq!(exclusions.get("comment_time_unknown"), Some(&5807));
    assert_eq!(exclusions.get("period_anchor_missing"), None);

    opportunity_dataset::build_platform_times(&database)
        .await
        .unwrap();
    let first = project_comment_platform_times(&database).await.unwrap();
    assert!(first.inserted > 0);
    assert_eq!(first.conflicts, 0);
    let second = project_comment_platform_times(&database).await.unwrap();
    assert_eq!(second.inserted, 0);
    assert_eq!(second.conflicts, 0);

    assert_eq!(eligible_count(&database, AS_OF).await, 5800);
    assert_eq!(
        eligible_count_for(&database, AS_OF, "comment_published").await,
        5803
    );
    let exclusions = exclusion_counts_for(&database, AS_OF, "comment_published").await;
    assert_eq!(exclusions.get("comment_time_unknown"), Some(&1));
    assert_eq!(exclusions.get("period_anchor_missing"), None);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn weekly_coverage_and_display_text_cross_check_are_reported() {
    let database = full_database("opportunity_c3_coverage").await;
    opportunity_dataset::build(&database).await.unwrap();
    opportunity_dataset::build_platform_times(&database)
        .await
        .unwrap();
    project_comment_platform_times(&database).await.unwrap();

    let report =
        read_platform_time_report(&database, Uuid::parse_str(DOMAIN).unwrap(), AS_OF, TZ, 12)
            .await
            .unwrap();

    assert_eq!(report.coverage.len(), 12);
    let eligible: i64 = report
        .coverage
        .iter()
        .map(|row| row.eligible_comments)
        .sum();
    let with_time: i64 = report
        .coverage
        .iter()
        .map(|row| row.with_platform_time)
        .sum();
    assert_eq!(eligible, 5800);
    assert_eq!(with_time, 5799);
    let incomplete: Vec<_> = report
        .coverage
        .iter()
        .filter(|row| row.eligible_comments > 0 && row.ratio != Some(1.0))
        .collect();
    assert_eq!(incomplete.len(), 1);
    assert_eq!(incomplete[0].period_start, "2026-10-26");
    assert_eq!(incomplete[0].eligible_comments, 1200);
    assert_eq!(incomplete[0].with_platform_time, 1199);

    assert_eq!(report.agreement.compared, 5799);
    assert_eq!(report.agreement.matching, 5798);
    assert!(report.agreement.ratio.unwrap() >= 0.99);
    assert_eq!(report.agreement.mismatches.len(), 1);
    assert_eq!(
        report.agreement.mismatches[0].comment_external_id,
        "c-work-1-17"
    );
    assert_eq!(report.timing_violations, 0);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn projector_keeps_the_first_value_and_counts_conflicting_packages() {
    let database = full_database("opportunity_c3_conflicts").await;
    opportunity_dataset::build(&database).await.unwrap();
    opportunity_dataset::build_platform_times(&database)
        .await
        .unwrap();
    let first = project_comment_platform_times(&database).await.unwrap();
    assert_eq!(first.conflicts, 0);

    sqlx::raw_sql(
        r#"
INSERT INTO linggan_runtime_task(task_id, task_spec_hash, task_spec, source, platform, page_type, created_at)
VALUES ('00000000-0000-4000-8000-00000000c301',
        md5('c3-conflict-task') || md5('c3-conflict-task-spec'),
        '{}'::jsonb, 'manual', 'xhs', 'seed', '2026-10-28T00:00:00Z');
INSERT INTO linggan_runtime_attempt(attempt_id, task_id, producer_instance_id, started_at)
VALUES ('00000000-0000-4000-8000-00000000c302', '00000000-0000-4000-8000-00000000c301',
        '00000000-0000-4000-8000-00000000c304', '2026-10-28T00:00:00Z');
INSERT INTO linggan_runtime_capture_package(
 package_ref, attempt_id, task_id, producer_instance_id, package_kind, platform, package_hash,
 observed_at, captured_at, coverage, payload, accepted_at
)
VALUES (
 '00000000-0000-4000-8000-00000000c303', '00000000-0000-4000-8000-00000000c302',
 '00000000-0000-4000-8000-00000000c301', '00000000-0000-4000-8000-00000000c304',
 'comments', 'xhs', md5('c3-conflict-package') || md5('c3-conflict-package'),
 '2026-10-28T00:00:00Z', '2026-10-28T00:00:00Z', '{}'::jsonb,
 jsonb_build_object(
  'collectionProtocol', jsonb_build_object('version', 1, 'sortMode', 'platform_default'),
  'records', jsonb_build_array(jsonb_build_object('payload', jsonb_build_object(
   'publishedAt', (extract(epoch FROM timestamptz '2026-10-27T00:02:00Z') * 1000)::bigint,
   'publishedAtText', '10-27')))
 ),
 '2026-10-28T00:00:00Z');
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason, created_at)
VALUES ('00000000-0000-4000-8000-00000000c303', 0, 'accepted_for_library_content', 'seed', '2026-10-28T00:00:00Z');
INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, parent_comment_external_id, parent_identity_source_field,
 is_reply, body_text, body_state, author_external_id, observed_at, created_at
)
VALUES ('00000000-0000-4000-8000-00000000c305', '00000000-0000-4000-8000-000000000101',
        '00000000-0000-4000-8000-00000000c303', 0, 'c-work-1-1', 'c-work-1-1',
        NULL, NULL, false, NULL, 'UNKNOWN', 'commenter-work-1-1',
        '2026-10-28T00:00:00Z', '2026-10-28T00:00:00Z');
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();

    let second = project_comment_platform_times(&database).await.unwrap();
    assert_eq!(second.inserted, 0);
    assert_eq!(second.conflicts, 1);
    let stored: String = sqlx::query_scalar(
        "SELECT to_char(published_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
         FROM linggan_material_comment_platform_time
         WHERE content_public_ref = '00000000-0000-4000-8000-000000000101'
           AND comment_external_id = 'c-work-1-1'
           AND precision = 'millisecond'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(stored, "2026-10-27T00:01:00Z");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn projector_skips_published_at_outside_the_millisecond_range() {
    let database = full_database("opportunity_c3_range").await;
    opportunity_dataset::build(&database).await.unwrap();
    opportunity_dataset::build_platform_times(&database)
        .await
        .unwrap();
    sqlx::raw_sql(
        r#"
ALTER TABLE linggan_runtime_capture_package DISABLE TRIGGER USER;
UPDATE linggan_runtime_capture_package package
SET payload = jsonb_set(
 package.payload, '{records,1,payload,publishedAt}', '253402300800000'::jsonb
)
FROM opportunity_seed_package seed
WHERE seed.package_ref = package.package_ref
  AND seed.work_key = 'work-2'
  AND seed.kind = 'comments';
ALTER TABLE linggan_runtime_capture_package ENABLE TRIGGER USER;
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();

    let projected = project_comment_platform_times(&database).await.unwrap();
    assert!(projected.inserted > 0);
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment_platform_time
         WHERE content_public_ref = '00000000-0000-4000-8000-000000000102'
           AND comment_external_id = ANY(ARRAY['c-work-2-1', 'c-work-2-2'])
           AND precision = 'millisecond'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let skipped: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment_platform_time
         WHERE content_public_ref = '00000000-0000-4000-8000-000000000102'
           AND comment_external_id = 'c-work-2-1'
           AND precision = 'millisecond'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(skipped, 0);
    assert_eq!(rows, 1);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn display_labels_project_as_day_precision_from_the_collection_moment() {
    let database = full_database("opportunity_c3_day").await;
    opportunity_dataset::build(&database).await.unwrap();
    opportunity_dataset::build_legacy_day_times(&database)
        .await
        .unwrap();

    let first = project_comment_platform_times(&database).await.unwrap();
    assert_eq!(first.inserted, 9);
    assert_eq!(first.conflicts, 0);

    let rows = sqlx::query(
        "SELECT comment_external_id,
                to_char(published_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD') AS local_date,
                to_char(published_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS utc,
                precision, source_kind, protocol_version
         FROM linggan_material_comment_platform_time
         WHERE content_public_ref = '00000000-0000-4000-8000-000000000101'
         ORDER BY comment_external_id",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    let observed: Vec<(String, String, String, String, String, i32)> = rows
        .into_iter()
        .map(|row| {
            (
                row.get("comment_external_id"),
                row.get("local_date"),
                row.get("utc"),
                row.get("precision"),
                row.get("source_kind"),
                row.get("protocol_version"),
            )
        })
        .collect();
    assert_eq!(observed.len(), 9);
    assert_eq!(
        observed[0],
        (
            "c-work-1-1".to_owned(),
            "2026-10-27".to_owned(),
            "2026-10-26T16:00:00Z".to_owned(),
            "day".to_owned(),
            "display_full_date".to_owned(),
            0,
        )
    );
    let dates: Vec<&str> = observed.iter().map(|row| row.1.as_str()).collect();
    assert_eq!(
        dates,
        vec![
            "2026-10-27",
            "2026-10-27",
            "2026-10-24",
            "2026-10-26",
            "2026-10-27",
            "2026-10-27",
            "2025-12-31",
            "2026-01-01",
            "2026-10-27",
        ]
    );
    assert_eq!(observed[1].4, "display_month_day");
    assert_eq!(observed[2].4, "display_relative");
    assert!(observed.iter().all(|row| row.3 == "day"));

    // The display cross-check only compares millisecond rows, so a day-only projection reports
    // nothing compared rather than validating itself.
    let report =
        read_platform_time_report(&database, Uuid::parse_str(DOMAIN).unwrap(), AS_OF, TZ, 12)
            .await
            .unwrap();
    assert_eq!(report.agreement.compared, 0);
    assert_eq!(report.timing_violations, 0);
    let with_time: i64 = report
        .coverage
        .iter()
        .map(|row| row.with_platform_time)
        .sum();
    assert_eq!(with_time, 9);

    // A later exact epoch is added alongside the historical day row, and readers prefer it.
    opportunity_dataset::build_platform_times(&database)
        .await
        .unwrap();
    let second = project_comment_platform_times(&database).await.unwrap();
    assert!(second.inserted > 0);
    let day_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment_platform_time
         WHERE content_public_ref = '00000000-0000-4000-8000-000000000101'
           AND comment_external_id = 'c-work-1-1'
           AND precision = 'day'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(day_rows, 1);
    let current = sqlx::query(
        "SELECT precision,
                to_char(published_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS utc
         FROM linggan_material_comment_platform_time_current
         WHERE content_public_ref = '00000000-0000-4000-8000-000000000101'
           AND comment_external_id = 'c-work-1-1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(current.get::<String, _>("precision"), "millisecond");
    assert_eq!(current.get::<String, _>("utc"), "2026-10-27T00:01:00Z");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn platform_time_after_first_observation_is_an_invariant_violation() {
    let database = full_database("opportunity_c3_i11").await;
    opportunity_dataset::build(&database).await.unwrap();
    opportunity_dataset::build_platform_times(&database)
        .await
        .unwrap();
    project_comment_platform_times(&database).await.unwrap();

    let report = check_invariants(&database).await.unwrap();
    let timing = report
        .checks
        .iter()
        .find(|check| check.id == "I11")
        .expect("I11 is registered");
    assert!(timing.available);
    assert_eq!(timing.violations, 0);

    sqlx::raw_sql(
        r#"
INSERT INTO linggan_material_comment_platform_time(
 content_public_ref, comment_external_id, published_at, precision, source_package_ref, protocol_version
)
VALUES ('00000000-0000-4000-8000-000000000101', 'c-work-1-100', '2026-12-01T00:00:00Z',
        'millisecond', md5('opportunity-package:work-1:comments')::uuid, 1);
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();

    let report = check_invariants(&database).await.unwrap();
    let timing = report
        .checks
        .iter()
        .find(|check| check.id == "I11")
        .expect("I11 is registered");
    assert_eq!(timing.violations, 1);
    assert!(report.failed());
    let platform =
        read_platform_time_report(&database, Uuid::parse_str(DOMAIN).unwrap(), AS_OF, TZ, 12)
            .await
            .unwrap();
    assert_eq!(platform.timing_violations, 1);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn problem_sync_lineage_and_resolution_match_the_taxonomy_rules() {
    let database = full_database("opportunity_t05_taxonomy").await;

    sqlx::raw_sql(
        r#"
ALTER TABLE linggan_comment_problem_definition DISABLE TRIGGER ALL;
INSERT INTO linggan_comment_problem_definition(
 definition_ref, problem_ref, domain_ref, identity_hash, label, actor, goal, barrier,
 context, source_request_ref, source_signal_ordinal, review_invocation_ref
) VALUES
 ('00000000-0000-4000-8000-000000002001','00000000-0000-4000-8000-000000001001',
  '00000000-0000-4000-8000-000000000001',repeat('a',64),'source-one','actor','goal','barrier',
  NULL,'00000000-0000-4000-8000-000000003001',0,'00000000-0000-4000-8000-000000004001'),
 ('00000000-0000-4000-8000-000000002002','00000000-0000-4000-8000-000000001002',
  '00000000-0000-4000-8000-000000000001',repeat('b',64),'source-two','actor','goal','barrier',
  NULL,'00000000-0000-4000-8000-000000003002',0,'00000000-0000-4000-8000-000000004002');
ALTER TABLE linggan_comment_problem_definition ENABLE TRIGGER ALL;
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();

    let synced: i64 = sqlx::query_scalar("SELECT synced FROM linggan_opportunity_sync_problems()")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(synced, 2);
    let synced_again: i64 =
        sqlx::query_scalar("SELECT synced FROM linggan_opportunity_sync_problems()")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(synced_again, 0);
    let registered: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_problem")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(registered, 2);
    let def_versions: Vec<i32> = sqlx::query_scalar(
        "SELECT def_version FROM linggan_opportunity_problem ORDER BY source_ref",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(def_versions, vec![1, 1]);

    sqlx::raw_sql(
        r#"
ALTER TABLE linggan_comment_problem_definition DISABLE TRIGGER USER;
UPDATE linggan_comment_problem_definition
SET definition_ref = '00000000-0000-4000-8000-000000002003'
WHERE problem_ref = '00000000-0000-4000-8000-000000001001';
ALTER TABLE linggan_comment_problem_definition ENABLE TRIGGER USER;
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();
    let changed: i64 = sqlx::query_scalar("SELECT synced FROM linggan_opportunity_sync_problems()")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(changed, 1);
    let bumped: i32 = sqlx::query_scalar(
        "SELECT def_version FROM linggan_opportunity_problem
         WHERE source_ref = '00000000-0000-4000-8000-000000001001'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(bumped, 2);

    sqlx::raw_sql(
        r#"
INSERT INTO linggan_opportunity_problem(problem_ref, source_ref, source_definition_ref, domain_ref, label) VALUES
 ('00000000-0000-4000-8000-000000000011','00000000-0000-4000-8000-000000000011','00000000-0000-4000-8000-000000000111','00000000-0000-4000-8000-000000000001','direct'),
 ('00000000-0000-4000-8000-000000000012','00000000-0000-4000-8000-000000000012','00000000-0000-4000-8000-000000000112','00000000-0000-4000-8000-000000000001','merge-a'),
 ('00000000-0000-4000-8000-000000000013','00000000-0000-4000-8000-000000000013','00000000-0000-4000-8000-000000000113','00000000-0000-4000-8000-000000000001','merge-b'),
 ('00000000-0000-4000-8000-000000000014','00000000-0000-4000-8000-000000000014','00000000-0000-4000-8000-000000000114','00000000-0000-4000-8000-000000000001','merge-c'),
 ('00000000-0000-4000-8000-000000000015','00000000-0000-4000-8000-000000000015','00000000-0000-4000-8000-000000000115','00000000-0000-4000-8000-000000000001','rename-1'),
 ('00000000-0000-4000-8000-000000000016','00000000-0000-4000-8000-000000000016','00000000-0000-4000-8000-000000000116','00000000-0000-4000-8000-000000000001','rename-2'),
 ('00000000-0000-4000-8000-000000000017','00000000-0000-4000-8000-000000000017','00000000-0000-4000-8000-000000000117','00000000-0000-4000-8000-000000000001','rename-3'),
 ('00000000-0000-4000-8000-000000000018','00000000-0000-4000-8000-000000000018','00000000-0000-4000-8000-000000000118','00000000-0000-4000-8000-000000000001','split-parent'),
 ('00000000-0000-4000-8000-000000000019','00000000-0000-4000-8000-000000000019','00000000-0000-4000-8000-000000000119','00000000-0000-4000-8000-000000000001','split-child-1'),
 ('00000000-0000-4000-8000-00000000001a','00000000-0000-4000-8000-00000000001a','00000000-0000-4000-8000-00000000011a','00000000-0000-4000-8000-000000000001','split-child-2'),
 ('00000000-0000-4000-8000-00000000001b','00000000-0000-4000-8000-00000000001b','00000000-0000-4000-8000-00000000011b','00000000-0000-4000-8000-000000000001','cycle-1'),
 ('00000000-0000-4000-8000-00000000001c','00000000-0000-4000-8000-00000000001c','00000000-0000-4000-8000-00000000011c','00000000-0000-4000-8000-000000000001','cycle-2');

INSERT INTO linggan_opportunity_problem_lineage(operation_ref, from_problem_ref, to_problem_ref, relation) VALUES
 ('00000000-0000-4000-8000-000000000201','00000000-0000-4000-8000-000000000012','00000000-0000-4000-8000-000000000013','merge'),
 ('00000000-0000-4000-8000-000000000202','00000000-0000-4000-8000-000000000013','00000000-0000-4000-8000-000000000014','merge'),
 ('00000000-0000-4000-8000-000000000203','00000000-0000-4000-8000-000000000015','00000000-0000-4000-8000-000000000016','rename'),
 ('00000000-0000-4000-8000-000000000204','00000000-0000-4000-8000-000000000016','00000000-0000-4000-8000-000000000017','merge'),
 ('00000000-0000-4000-8000-000000000205','00000000-0000-4000-8000-000000000018','00000000-0000-4000-8000-000000000019','split'),
 ('00000000-0000-4000-8000-000000000205','00000000-0000-4000-8000-000000000018','00000000-0000-4000-8000-00000000001a','split'),
 ('00000000-0000-4000-8000-000000000206','00000000-0000-4000-8000-00000000001b','00000000-0000-4000-8000-00000000001c','merge'),
 ('00000000-0000-4000-8000-000000000207','00000000-0000-4000-8000-00000000001c','00000000-0000-4000-8000-00000000001b','merge');
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();

    let direct: String = sqlx::query_scalar(
        "SELECT array_to_string(path, '>') FROM linggan_opportunity_resolve_problem_id($1::uuid)",
    )
    .bind("00000000-0000-4000-8000-000000000011")
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(direct, "00000000-0000-4000-8000-000000000011");

    let two_hop: String = sqlx::query_scalar(
        "SELECT array_to_string(path, '>') FROM linggan_opportunity_resolve_problem_id($1::uuid)",
    )
    .bind("00000000-0000-4000-8000-000000000012")
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        two_hop,
        "00000000-0000-4000-8000-000000000012>00000000-0000-4000-8000-000000000013>00000000-0000-4000-8000-000000000014"
    );

    let rename_then_merge: String = sqlx::query_scalar(
        "SELECT array_to_string(path, '>') FROM linggan_opportunity_resolve_problem_id($1::uuid)",
    )
    .bind("00000000-0000-4000-8000-000000000015")
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        rename_then_merge,
        "00000000-0000-4000-8000-000000000015>00000000-0000-4000-8000-000000000016>00000000-0000-4000-8000-000000000017"
    );

    let split: Vec<String> = sqlx::query_scalar(
        "SELECT problem_ref::text || '|' || array_to_string(path, '>')
         FROM linggan_opportunity_resolve_problem_id($1::uuid) ORDER BY problem_ref",
    )
    .bind("00000000-0000-4000-8000-000000000018")
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        split,
        vec![
            "00000000-0000-4000-8000-000000000019|00000000-0000-4000-8000-000000000018>00000000-0000-4000-8000-000000000019".to_string(),
            "00000000-0000-4000-8000-00000000001a|00000000-0000-4000-8000-000000000018>00000000-0000-4000-8000-00000000001a".to_string(),
        ]
    );

    let cycle = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM linggan_opportunity_resolve_problem_id($1::uuid)",
    )
    .bind("00000000-0000-4000-8000-00000000001b")
    .fetch_one(database.pool())
    .await
    .expect_err("a merge cycle must fail instead of looping");
    assert!(cycle.to_string().contains("problem_lineage_cycle"));

    let version: i64 = sqlx::query_scalar(
        "SELECT version FROM linggan_opportunity_taxonomy_version WHERE domain_ref = $1::uuid",
    )
    .bind(DOMAIN)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(version, 7);
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn deterministic_builder_repeats_identically() {
    let first = full_database("opportunity_t07_build_a").await;
    let second = full_database("opportunity_t07_build_b").await;
    opportunity_dataset::build(&first).await.unwrap();
    opportunity_dataset::build(&second).await.unwrap();

    assert_eq!(
        dataset_fingerprint(&first).await,
        dataset_fingerprint(&second).await
    );
    assert_eq!(eligible_count(&first, AS_OF).await, 5800);
    assert_eq!(eligible_count(&second, AS_OF).await, 5800);
    assert_eq!(
        exclusion_counts(&first, AS_OF).await,
        exclusion_counts(&second, AS_OF).await
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-opportunity-postgres.sh and an isolated PostgreSQL proof database"]
async fn invariants_report_zero_violations_and_detect_a_lineage_cycle() {
    let database = full_database("opportunity_t07_invariants").await;
    opportunity_dataset::build(&database).await.unwrap();

    let report = check_invariants(&database).await.unwrap();
    let lineage = report
        .checks
        .iter()
        .find(|check| check.id == "I8")
        .expect("I8 is registered");
    assert!(lineage.available);
    assert_eq!(lineage.violations, 0);
    for check in &report.checks {
        assert_eq!(check.violations, 0, "{} must have no violations", check.id);
    }
    assert!(!report.failed());

    sqlx::raw_sql(
        r#"
INSERT INTO linggan_opportunity_problem(problem_ref, source_ref, source_definition_ref, domain_ref, label) VALUES
 ('00000000-0000-4000-8000-000000000061','00000000-0000-4000-8000-000000000061','00000000-0000-4000-8000-000000000161','00000000-0000-4000-8000-000000000001','cycle-a'),
 ('00000000-0000-4000-8000-000000000062','00000000-0000-4000-8000-000000000062','00000000-0000-4000-8000-000000000162','00000000-0000-4000-8000-000000000001','cycle-b');
INSERT INTO linggan_opportunity_problem_lineage(operation_ref, from_problem_ref, to_problem_ref, relation) VALUES
 ('00000000-0000-4000-8000-000000000261','00000000-0000-4000-8000-000000000061','00000000-0000-4000-8000-000000000062','merge'),
 ('00000000-0000-4000-8000-000000000262','00000000-0000-4000-8000-000000000062','00000000-0000-4000-8000-000000000061','merge');
"#,
    )
    .execute(database.pool())
    .await
    .unwrap();
    let report = check_invariants(&database).await.unwrap();
    let lineage = report
        .checks
        .iter()
        .find(|check| check.id == "I8")
        .expect("I8 is registered");
    assert!(lineage.violations > 0);
    assert!(report.failed());

    sqlx::raw_sql("DELETE FROM linggan_opportunity_problem_lineage")
        .execute(database.pool())
        .await
        .unwrap();
    let report = check_invariants(&database).await.unwrap();
    assert!(!report.failed());
}
