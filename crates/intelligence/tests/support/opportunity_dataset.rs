//! Deterministic builder for the §8.2 playbook dataset. Every identifier is derived from a
//! stable text digest, so two builds of the same schema produce the same rows and timestamps.

use linggan_storage_postgres::Database;

pub const DOMAIN: &str = "00000000-0000-4000-8000-000000000001";
pub const AS_OF: &str = "2026-11-10T00:00:00Z";
pub const TZ: &str = "Asia/Shanghai";
pub const BASIS: &str = "work_published_cohort";

pub async fn build(database: &Database) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(SEED_SQL).execute(database.pool()).await?;
    Ok(())
}

/// C3 overlay: give the already-seeded comment packages a protocol marker and milliseconds-epoch
/// records. Kept out of `build` so the T0.7 dataset keeps representing the pre-C3 collection.
pub async fn build_platform_times(database: &Database) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(PLATFORM_TIME_SQL)
        .execute(database.pool())
        .await?;
    Ok(())
}

pub const SEED_SQL: &str = r#"
CREATE TABLE opportunity_seed_work (
 work_key text PRIMARY KEY,
 work_ref uuid NOT NULL,
 content_external_id text NOT NULL,
 published_at timestamptz,
 detail_at timestamptz NOT NULL,
 first_observed_at timestamptz NOT NULL,
 author_external_id text,
 role text NOT NULL,
 comment_count integer NOT NULL
);
INSERT INTO opportunity_seed_work VALUES
 ('work-0','00000000-0000-4000-8000-000000000100','work-0','2026-11-03T00:00:00Z','2026-11-03T00:30:00Z','2026-11-03T01:00:00Z','author-work-0','primary',1000),
 ('work-1','00000000-0000-4000-8000-000000000101','work-1','2026-10-27T00:00:00Z','2026-10-27T00:30:00Z','2026-10-27T01:00:00Z','author-work-1','primary',1200),
 ('work-2','00000000-0000-4000-8000-000000000102','work-2','2026-10-20T00:00:00Z','2026-10-20T00:30:00Z','2026-10-20T01:00:00Z','author-work-2','primary',1200),
 ('work-3','00000000-0000-4000-8000-000000000103','work-3','2026-10-13T00:00:00Z','2026-10-13T00:30:00Z','2026-10-13T01:00:00Z','author-work-3','primary',1200),
 ('work-4','00000000-0000-4000-8000-000000000104','work-4','2026-10-06T00:00:00Z','2026-10-06T00:30:00Z','2026-10-06T01:00:00Z','author-work-4','primary',1200),
 ('work-unknown','00000000-0000-4000-8000-000000000201','work-unknown','2026-11-04T00:00:00Z','2026-11-04T00:30:00Z','2026-11-04T01:00:00Z',NULL,'primary',5),
 ('work-noanchor','00000000-0000-4000-8000-000000000202','work-noanchor',NULL,'2026-11-04T00:30:00Z','2026-11-04T01:00:00Z','author-work-noanchor','primary',4),
 ('work-reference','00000000-0000-4000-8000-000000000203','work-reference','2026-11-05T00:00:00Z','2026-11-05T00:30:00Z','2026-11-05T01:00:00Z','author-work-reference','reference',3);

CREATE TABLE opportunity_seed_package (
 work_key text NOT NULL,
 kind text NOT NULL,
 package_ref uuid NOT NULL,
 task_id uuid NOT NULL,
 attempt_id uuid NOT NULL,
 accepted_at timestamptz NOT NULL,
 PRIMARY KEY (work_key, kind)
);
INSERT INTO opportunity_seed_package
SELECT work.work_key, kinds.kind,
       md5('opportunity-package:' || work.work_key || ':' || kinds.kind)::uuid,
       md5('opportunity-task:' || work.work_key || ':' || kinds.kind)::uuid,
       md5('opportunity-attempt:' || work.work_key || ':' || kinds.kind)::uuid,
       CASE kinds.kind
        WHEN 'detail' THEN work.detail_at
        WHEN 'comments' THEN work.first_observed_at
        ELSE '2026-11-10T01:00:00Z'::timestamptz
       END
FROM opportunity_seed_work work
CROSS JOIN (VALUES ('detail'), ('comments'), ('late')) kinds(kind);

INSERT INTO linggan_runtime_task(
 task_id, task_spec_hash, task_spec, source, platform, page_type, created_at
)
SELECT task_id, md5(task_id::text) || md5(task_id::text || 'task'), '{}'::jsonb,
       'manual', 'xhs', 'seed', '2026-01-01T00:00:00Z'
FROM opportunity_seed_package;
INSERT INTO linggan_runtime_attempt(attempt_id, task_id, producer_instance_id, started_at)
SELECT attempt_id, task_id, md5('opportunity-producer:' || attempt_id::text)::uuid,
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_package;
INSERT INTO linggan_runtime_capture_package(
 package_ref, attempt_id, task_id, producer_instance_id, package_kind, platform, package_hash,
 observed_at, captured_at, coverage, payload, accepted_at
)
SELECT package_ref, attempt_id, task_id, md5('opportunity-producer:' || attempt_id::text)::uuid,
       CASE kind WHEN 'detail' THEN 'content_detail' ELSE 'comments' END, 'xhs',
       md5(package_ref::text) || md5(package_ref::text || 'package'),
       to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
       to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
       '{}'::jsonb, '{}'::jsonb, accepted_at
FROM opportunity_seed_package;

INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason, created_at)
SELECT package_ref, 0, 'accepted_for_library_content', 'seed', '2026-01-01T00:00:00Z'
FROM opportunity_seed_package WHERE kind = 'detail';
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason, created_at)
SELECT package.package_ref, series.n, 'accepted_for_library_content', 'seed',
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_package package
JOIN opportunity_seed_work work USING (work_key)
CROSS JOIN LATERAL generate_series(1, work.comment_count) series(n)
WHERE package.kind = 'comments';
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason, created_at)
SELECT package.package_ref, extra.ordinal, 'accepted_for_library_content', 'seed',
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_package package
CROSS JOIN (VALUES (1001), (1002), (1003)) extra(ordinal)
WHERE package.work_key = 'work-0' AND package.kind = 'comments';
INSERT INTO linggan_runtime_record_disposition(package_ref, record_ordinal, disposition, reason, created_at)
SELECT package.package_ref, extra.ordinal, 'accepted_for_library_content', 'seed',
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_package package
CROSS JOIN (VALUES (1), (2), (3)) extra(ordinal)
WHERE package.work_key = 'work-0' AND package.kind = 'late';

INSERT INTO linggan_material_content(
 platform, content_external_id, public_ref, first_package_ref, created_at
)
SELECT 'xhs', work.content_external_id, work.work_ref, package.package_ref,
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_work work
JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'detail';

INSERT INTO linggan_material_content_detail(
 material_ref, content_public_ref, package_ref, record_ordinal, observed_at,
 title, title_state, body_text, body_state, creator_display_name, creator_display_name_state,
 published_at_source_text, published_at_source_text_state, searchable_text, author_external_id,
 published_at, published_at_source_field, published_at_source_kind, published_at_precision,
 published_at_parser_version, created_at
)
SELECT md5('opportunity-detail:' || work.work_ref::text)::uuid, work.work_ref, package.package_ref, 0,
       to_char(work.detail_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
       'seed title', 'KNOWN', NULL, 'UNKNOWN', NULL, 'UNKNOWN', NULL, 'UNKNOWN', 'seed',
       work.author_external_id, work.published_at,
       CASE WHEN work.published_at IS NOT NULL THEN 'seedPublishTime' END,
       CASE WHEN work.published_at IS NOT NULL THEN 'platform_epoch' ELSE 'unknown' END,
       CASE WHEN work.published_at IS NOT NULL THEN 'second' ELSE 'unknown' END,
       CASE WHEN work.published_at IS NOT NULL THEN 'seed-v1' END,
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_work work
JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'detail';

INSERT INTO linggan_material_domain_usage(
 usage_ref, content_public_ref, domain_ref, role, use_kind, basis_kind, package_ref, record_ordinal, created_at
)
SELECT md5('opportunity-usage:' || work.work_ref::text)::uuid, work.work_ref,
       '00000000-0000-4000-8000-000000000001',
       work.role, 'domain_research', 'legacy_domain_migration', package.package_ref, NULL,
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_work work
JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'detail';

INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, parent_comment_external_id, parent_identity_source_field,
 is_reply, body_text, body_state, author_external_id, observed_at, created_at
)
SELECT md5('opportunity-comment:' || seed.work_ref::text || ':' || seed.n::text)::uuid,
       seed.work_ref, seed.package_ref, seed.n, seed.comment_id,
       CASE WHEN seed.reply THEN 'c-work-0-1' ELSE seed.comment_id END,
       CASE WHEN seed.reply THEN 'c-work-0-1' END,
       CASE WHEN seed.reply THEN 'parentCommentId' END,
       seed.reply, NULL, 'UNKNOWN',
       CASE WHEN seed.null_author THEN NULL ELSE 'commenter-' || seed.work_key || '-' || seed.n END,
       seed.observed_text, '2026-01-01T00:00:00Z'
FROM (
 SELECT work.work_ref, package.package_ref, series.n, work.work_key,
        'c-' || work.work_key || '-' || series.n AS comment_id,
        work.work_key = 'work-0' AND series.n BETWEEN 2 AND 51 AS reply,
        work.work_key = 'work-0' AND series.n = 1000 AS null_author,
        to_char(package.accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS observed_text
 FROM opportunity_seed_work work
 JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'comments'
 CROSS JOIN LATERAL generate_series(1, work.comment_count) series(n)
) seed;

INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, parent_comment_external_id, parent_identity_source_field,
 is_reply, body_text, body_state, author_external_id, observed_at, created_at
)
SELECT md5('opportunity-comment:' || work.work_ref::text || ':' || extra.ordinal::text)::uuid,
       work.work_ref, package.package_ref, extra.ordinal, extra.comment_id,
       extra.comment_id, NULL, NULL, false, NULL, 'UNKNOWN', extra.author_external_id,
       to_char(package.accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_work work
JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'comments'
CROSS JOIN (VALUES
 (1001, 'c-work-0-self', 'author-work-0'),
 (1002, 'c-work-0-restricted-1', 'commenter-restricted-1'),
 (1003, 'c-work-0-restricted-2', 'commenter-restricted-2')
) extra(ordinal, comment_id, author_external_id)
WHERE work.work_key = 'work-0';

INSERT INTO linggan_material_comment(
 material_ref, content_public_ref, package_ref, record_ordinal, comment_external_id,
 root_comment_external_id, parent_comment_external_id, parent_identity_source_field,
 is_reply, body_text, body_state, author_external_id, observed_at, created_at
)
SELECT md5('opportunity-late-comment:' || work.work_ref::text || ':' || extra.ordinal::text)::uuid,
       work.work_ref, package.package_ref, extra.ordinal, extra.comment_id,
       extra.comment_id, NULL, NULL, false, NULL, 'UNKNOWN', extra.author_external_id,
       to_char(package.accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
       '2026-01-01T00:00:00Z'
FROM opportunity_seed_work work
JOIN opportunity_seed_package package ON package.work_key = work.work_key AND package.kind = 'late'
CROSS JOIN (VALUES
 (1, 'c-work-0-late-1', 'commenter-late-1'),
 (2, 'c-work-0-late-2', 'commenter-late-2'),
 (3, 'c-work-0-late-3', 'commenter-late-3')
) extra(ordinal, comment_id, author_external_id)
WHERE work.work_key = 'work-0';

INSERT INTO linggan_material_comment_restriction(
 content_public_ref, comment_external_id, restricted_at, reason
)
SELECT '00000000-0000-4000-8000-000000000100', restricted.comment_id,
       '2026-11-09T00:00:00Z', 'seed'
FROM (VALUES ('c-work-0-restricted-1'), ('c-work-0-restricted-2')) restricted(comment_id);

ALTER TABLE linggan_material_comment_first_admission DISABLE TRIGGER USER;
DELETE FROM linggan_material_comment_first_admission
WHERE content_public_ref = '00000000-0000-4000-8000-000000000104'
  AND comment_external_id IN (SELECT 'c-work-4-' || n FROM generate_series(1191, 1200) n);
ALTER TABLE linggan_material_comment_first_admission ENABLE TRIGGER USER;
"#;

pub const PLATFORM_TIME_SQL: &str = r#"
ALTER TABLE linggan_runtime_capture_package DISABLE TRIGGER USER;
UPDATE linggan_runtime_capture_package package
SET payload = jsonb_build_object(
 'collectionProtocol', jsonb_build_object(
  'version', 1,
  'sortMode', 'platform_default',
  'singleRunLimit', 5000,
  'triggerSource', 'detail_refresh'
 ),
 'records', jsonb_build_array(jsonb_build_object('kind', 'control', 'payload', '{}'::jsonb))
  || COALESCE((
  SELECT jsonb_agg(jsonb_build_object('payload', jsonb_build_object(
   'publishedAt', CASE
    WHEN seed_work.work_key = 'work-1' AND comment.record_ordinal = 100 THEN NULL
    ELSE (extract(epoch FROM (
     COALESCE(seed_work.published_at, timestamptz '2026-11-04T00:00:00Z')
     + make_interval(mins => comment.record_ordinal % 45)
    )) * 1000)::bigint END,
   'publishedAtText', CASE
    WHEN seed_work.work_key = 'work-1' AND comment.record_ordinal = 100 THEN NULL
    WHEN seed_work.work_key = 'work-1' AND comment.record_ordinal = 17 THEN '01-01'
    WHEN comment.record_ordinal % 100 = 0 THEN to_char(
     (COALESCE(seed_work.published_at, timestamptz '2026-11-04T00:00:00Z')
      + make_interval(mins => comment.record_ordinal % 45)) AT TIME ZONE 'Asia/Shanghai',
     'YYYY-MM-DD')
    ELSE to_char(
     (COALESCE(seed_work.published_at, timestamptz '2026-11-04T00:00:00Z')
      + make_interval(mins => comment.record_ordinal % 45)) AT TIME ZONE 'Asia/Shanghai',
     'MM-DD') END
  )) ORDER BY comment.record_ordinal)
  FROM linggan_material_comment comment
  JOIN opportunity_seed_work seed_work ON seed_work.work_ref = comment.content_public_ref
  WHERE comment.package_ref = package.package_ref
 ), '[]'::jsonb)
)
FROM opportunity_seed_package package_seed
WHERE package_seed.package_ref = package.package_ref
  AND package_seed.kind IN ('comments', 'late');
ALTER TABLE linggan_runtime_capture_package ENABLE TRIGGER USER;
"#;
