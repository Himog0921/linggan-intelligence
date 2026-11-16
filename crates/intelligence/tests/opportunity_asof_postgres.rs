//! D-037/D-039 proof using only a disposable PostgreSQL schema and synthetic observations.
#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema_fixture;
#[path = "support/opportunity_dataset.rs"]
mod opportunity_dataset;

use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;

async fn database(schema: &str) -> Database {
    let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
    let database = isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .expect("isolated schema");
    opportunity_dataset::build(&database).await.unwrap();
    database
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn platform_time_uses_only_rows_and_packages_available_at_as_of() {
    let db = database("opportunity_t12_platform_asof").await;
    sqlx::raw_sql(
        r#"
INSERT INTO linggan_material_comment_platform_time(
 content_public_ref, comment_external_id, published_at, precision, source_package_ref,
 protocol_version, source_kind, created_at
)
VALUES
 ('00000000-0000-4000-8000-000000000101', 'c-work-1-1', '2026-10-26T16:00:00Z',
  'day', md5('opportunity-package:work-1:comments')::uuid, 0, 'display_full_date', '2026-11-04T00:00:00Z'),
 ('00000000-0000-4000-8000-000000000101', 'c-work-1-1', '2026-10-27T00:01:00Z',
  'millisecond', md5('opportunity-package:work-1:late')::uuid, 1, 'epoch', '2026-11-10T02:00:00Z'),
 ('00000000-0000-4000-8000-000000000101', 'c-work-1-2', '2026-10-27T00:02:00Z',
  'millisecond', md5('opportunity-package:work-1:late')::uuid, 1, 'epoch', '2026-11-04T00:00:00Z');
"#,
    )
    .execute(db.pool())
    .await
    .unwrap();
    for (as_of, expected) in [
        ("2026-11-03T00:00:00Z", None),
        ("2026-11-09T00:00:00Z", Some("day")),
        ("2026-11-10T01:30:00Z", Some("day")),
        ("2026-11-10T03:00:00Z", Some("millisecond")),
    ] {
        let chosen: Option<String> = sqlx::query_scalar(
            "SELECT precision FROM linggan_opportunity_platform_time_at($1::timestamptz) \
             WHERE content_public_ref='00000000-0000-4000-8000-000000000101' \
             AND comment_external_id='c-work-1-1'",
        )
        .bind(as_of)
        .fetch_optional(db.pool())
        .await
        .unwrap();
        assert_eq!(chosen.as_deref(), expected, "as_of={as_of}");
    }
    let prematurely_projected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_platform_time_at('2026-11-09T00:00:00Z') \
         WHERE comment_external_id='c-work-1-2'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        prematurely_projected, 0,
        "a future package remains unavailable"
    );
    let current: String = sqlx::query_scalar(
        "SELECT precision FROM linggan_material_comment_platform_time_current \
         WHERE comment_external_id='c-work-1-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(current, "millisecond");

    let reason: Option<String> = sqlx::query_scalar(
        "SELECT exclusion_reason FROM linggan_opportunity_comment_eligibility( \
         $1::uuid,'2026-11-03T00:00:00Z',$2,'comment_published') \
         WHERE comment_external_id='c-work-1-1'",
    )
    .bind(opportunity_dataset::DOMAIN)
    .bind(opportunity_dataset::TZ)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(reason.as_deref(), Some("comment_time_unknown"));
    let covered: i64 = sqlx::query_scalar(
        "SELECT sum(with_platform_time)::bigint FROM linggan_opportunity_platform_time_coverage( \
         $1::uuid,'2026-11-09T00:00:00Z',$2,12)",
    )
    .bind(opportunity_dataset::DOMAIN)
    .bind(opportunity_dataset::TZ)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(covered, 1);
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn fact_identity_and_first_observation_use_only_as_of_observations() {
    let db = database("opportunity_t12_fact_asof").await;
    sqlx::raw_sql(
        r#"
ALTER TABLE linggan_material_comment_first_admission DISABLE TRIGGER USER;
DELETE FROM linggan_material_comment_first_admission
WHERE content_public_ref='00000000-0000-4000-8000-000000000100'
  AND comment_external_id='c-work-0-1';
ALTER TABLE linggan_material_comment_first_admission ENABLE TRIGGER USER;
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason)
SELECT package_ref, n, 'accepted_for_library_content', 'synthetic as-of proof'
FROM opportunity_seed_package CROSS JOIN generate_series(4,5) AS n
WHERE work_key='work-0' AND kind='late';
ALTER TABLE linggan_material_comment DISABLE TRIGGER collection_comment_first_admitted;
INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, is_reply, body_text, body_state, author_external_id, observed_at, created_at
)
SELECT md5('asof-observation:' || n)::uuid, '00000000-0000-4000-8000-000000000100',
       package_ref, n, CASE n WHEN 4 THEN 'c-work-0-1' ELSE 'c-future-only' END,
       CASE n WHEN 4 THEN 'c-work-0-1' ELSE 'c-future-only' END, false, NULL, 'UNKNOWN',
       CASE n WHEN 4 THEN 'synthetic-later-author' ELSE 'synthetic-future-author' END,
       '2026-10-30T00:00:00Z', '2026-11-10T02:00:00Z'
FROM opportunity_seed_package CROSS JOIN generate_series(4,5) AS n
WHERE work_key='work-0' AND kind='late';
ALTER TABLE linggan_material_comment ENABLE TRIGGER collection_comment_first_admitted;
"#,
    )
    .execute(db.pool())
    .await
    .unwrap();
    for (as_of, author, observations, conflict) in [
        ("2026-11-10T00:00:00Z", "commenter-work-0-1", 1_i64, false),
        ("2026-11-10T01:30:00Z", "commenter-work-0-1", 1_i64, false),
        (
            "2026-11-10T03:00:00Z",
            "synthetic-later-author",
            2_i64,
            true,
        ),
    ] {
        let row = sqlx::query(
            "SELECT commenter_external_id, observations, commenter_external_id_conflict, \
             first_observed_backfilled FROM linggan_opportunity_comment_fact_at($1::timestamptz) \
             WHERE comment_external_id='c-work-0-1'",
        )
        .bind(as_of)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("commenter_external_id"), author);
        assert_eq!(row.get::<i64, _>("observations"), observations);
        assert_eq!(
            row.get::<bool, _>("commenter_external_id_conflict"),
            conflict
        );
        assert!(row.get::<bool, _>("first_observed_backfilled"));
    }
    let future_at_cutoff: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_comment_fact_at('2026-11-10T00:00:00Z') \
         WHERE comment_external_id='c-future-only'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(future_at_cutoff, 0);
    let current: String = sqlx::query_scalar(
        "SELECT commenter_external_id FROM linggan_opportunity_comment_fact \
         WHERE comment_external_id='c-work-0-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(current, "synthetic-later-author");
}

const SHARED_AUTHOR_SQL: &str = r#"
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason)
SELECT package_ref, 1, 'accepted_for_library_content', 'synthetic late projection'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='detail';
INSERT INTO linggan_material_content_detail(
 material_ref, content_public_ref, package_ref, record_ordinal, observed_at,
 title, title_state, body_text, body_state, creator_display_name, creator_display_name_state,
 published_at_source_text, published_at_source_text_state, searchable_text, author_external_id,
 published_at, published_at_source_field, published_at_source_kind, published_at_precision,
 published_at_parser_version, created_at
)
SELECT md5('asof-future-detail')::uuid, '00000000-0000-4000-8000-000000000100', package_ref, 1,
       '2026-11-08T00:00:00Z', 'synthetic', 'KNOWN', NULL, 'UNKNOWN', NULL, 'UNKNOWN',
       NULL, 'UNKNOWN', 'synthetic', 'synthetic-future-work-author', '2026-10-20T00:00:00Z',
       'seedPublishTime', 'platform_epoch', 'second', 'seed-v1', '2026-11-10T02:00:00Z'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='detail';
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason)
SELECT package_ref, 2, 'accepted_for_library_content', 'synthetic missing newest anchor'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='detail';
INSERT INTO linggan_material_content_detail(
 material_ref, content_public_ref, package_ref, record_ordinal, observed_at,
 title, title_state, body_text, body_state, creator_display_name, creator_display_name_state,
 published_at_source_text, published_at_source_text_state, searchable_text, author_external_id,
 published_at, published_at_source_field, published_at_source_kind, published_at_precision,
 published_at_parser_version, created_at
)
SELECT md5('asof-missing-latest-detail')::uuid, '00000000-0000-4000-8000-000000000100', package_ref, 2,
       '2026-11-09T00:00:00Z', 'synthetic', 'KNOWN', NULL, 'UNKNOWN', NULL, 'UNKNOWN',
       NULL, 'UNKNOWN', 'synthetic', NULL, NULL, NULL, 'unknown', 'unknown', NULL, '2026-11-10T04:00:00Z'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='detail';
ALTER TABLE linggan_runtime_capture_package DISABLE TRIGGER USER;
UPDATE linggan_runtime_capture_package SET package_kind='profile_discovery'
WHERE package_ref=(SELECT package_ref FROM opportunity_seed_package WHERE work_key='work-unknown' AND kind='comments');
ALTER TABLE linggan_runtime_capture_package ENABLE TRIGGER USER;
ALTER TABLE linggan_runtime_task DISABLE TRIGGER USER;
UPDATE linggan_runtime_task SET task_spec=jsonb_build_object('target',jsonb_build_object('authorExternalId','synthetic-profile-author'))
WHERE task_id=(SELECT task_id FROM opportunity_seed_package WHERE work_key='work-unknown' AND kind='comments');
ALTER TABLE linggan_runtime_task ENABLE TRIGGER USER;
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason)
SELECT package_ref, 2000, 'accepted_for_library_content', 'synthetic profile projection'
FROM opportunity_seed_package WHERE work_key='work-unknown' AND kind='comments';
INSERT INTO linggan_material_discovery_finding(material_ref,content_public_ref,package_ref,record_ordinal,
 discovery_kind,result_position,observed_at,title,title_state,creator_display_name,creator_state,
 published_at_source_text,published_at_source_text_state,created_at)
SELECT md5('asof-future-profile')::uuid,'00000000-0000-4000-8000-000000000201',package_ref,2000,
 'profile_discovery',1,'2026-11-08T00:00:00Z',NULL,'UNKNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN',
 '2026-11-10T02:00:00Z'
FROM opportunity_seed_package WHERE work_key='work-unknown' AND kind='comments';
"#;

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn shared_author_and_work_anchor_do_not_read_future_observations() {
    let db = database("opportunity_t12_author_asof").await;
    sqlx::raw_sql(SHARED_AUTHOR_SQL)
        .execute(db.pool())
        .await
        .unwrap();
    for (as_of, expected_author, expected_start) in [
        ("2026-11-10T00:00:00Z", "author-work-0", "2026-11-02"),
        (
            "2026-11-10T03:00:00Z",
            "synthetic-future-work-author",
            "2026-10-19",
        ),
    ] {
        let author: String = sqlx::query_scalar(
            "SELECT author_external_id FROM linggan_material_content_author_at( \
             '00000000-0000-4000-8000-000000000100', $1::timestamptz)",
        )
        .bind(as_of)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(author, expected_author);
        let period: String = sqlx::query_scalar(
            "SELECT period_start::text FROM linggan_opportunity_comment_eligibility( \
             $1::uuid,$2::timestamptz,$3,'work_published_cohort') \
             WHERE comment_external_id='c-work-0-1'",
        )
        .bind(opportunity_dataset::DOMAIN)
        .bind(as_of)
        .bind(opportunity_dataset::TZ)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(period, expected_start);
    }
    let profile_before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content_author_at( \
         '00000000-0000-4000-8000-000000000201','2026-11-10T00:00:00Z')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(profile_before, 0);
    let profile_after: String = sqlx::query_scalar(
        "SELECT author_external_id FROM linggan_material_content_author_at( \
         '00000000-0000-4000-8000-000000000201','2026-11-10T03:00:00Z')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(profile_after, "synthetic-profile-author");
    let missing: Option<String> = sqlx::query_scalar(
        "SELECT exclusion_reason FROM linggan_opportunity_comment_eligibility( \
         $1::uuid,'2026-11-10T05:00:00Z',$2,'work_published_cohort') \
         WHERE comment_external_id='c-work-0-1'",
    )
    .bind(opportunity_dataset::DOMAIN)
    .bind(opportunity_dataset::TZ)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(missing.as_deref(), Some("period_anchor_missing"));
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn shared_author_skips_bad_time_and_preserves_valid_source_priority() {
    let db = database("opportunity_author_bad_observation_time").await;
    sqlx::raw_sql(SHARED_AUTHOR_SQL)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql(r#"
      ALTER TABLE linggan_material_content_detail DISABLE TRIGGER USER;
      UPDATE linggan_material_content_detail SET observed_at='not-a-time'
      WHERE material_ref=md5('asof-future-detail')::uuid;
      ALTER TABLE linggan_material_content_detail ENABLE TRIGGER USER;
      INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason)
      SELECT package_ref,1,'accepted_for_library_content','synthetic bad author detail'
      FROM opportunity_seed_package WHERE work_key='work-unknown' AND kind='detail';
      INSERT INTO linggan_material_content_detail(material_ref,content_public_ref,package_ref,record_ordinal,
        observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,
        published_at_source_text,published_at_source_text_state,searchable_text,author_external_id,
        published_at,published_at_source_field,published_at_source_kind,published_at_precision,
        published_at_parser_version,created_at)
      SELECT md5('asof-bad-higher-priority')::uuid,content_public_ref,package_ref,1,
        'not-a-time',title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,
        published_at_source_text,published_at_source_text_state,searchable_text,'synthetic-bad-author',
        published_at,published_at_source_field,published_at_source_kind,published_at_precision,
        published_at_parser_version,'2026-11-10T02:00:00Z'
      FROM linggan_material_content_detail
      WHERE content_public_ref='00000000-0000-4000-8000-000000000201' AND record_ordinal=0;
    "#).execute(db.pool()).await.unwrap();
    let author: String = sqlx::query_scalar(
        "SELECT author_external_id FROM linggan_material_content_author_at( \
        '00000000-0000-4000-8000-000000000100','2026-11-10T03:00:00Z')",
    )
    .fetch_one(db.pool())
    .await
    .expect("a bad later observation must not break shared author selection");
    assert_eq!(author, "author-work-0");
    let fallback: String = sqlx::query_scalar(
        "SELECT author_external_id FROM linggan_material_content_author_at( \
        '00000000-0000-4000-8000-000000000201','2026-11-10T03:00:00Z')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        fallback, "synthetic-profile-author",
        "a bad detail must not hide a valid discovery author"
    );
    sqlx::raw_sql(
        "ALTER TABLE linggan_material_discovery_finding DISABLE TRIGGER USER; \
        UPDATE linggan_material_discovery_finding SET observed_at='not-a-time' \
        WHERE content_public_ref='00000000-0000-4000-8000-000000000201'; \
        ALTER TABLE linggan_material_discovery_finding ENABLE TRIGGER USER",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let no_author: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content_author_at( \
        '00000000-0000-4000-8000-000000000201','2026-11-10T03:00:00Z')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        no_author, 0,
        "no parseable observation means unknown author"
    );
}
