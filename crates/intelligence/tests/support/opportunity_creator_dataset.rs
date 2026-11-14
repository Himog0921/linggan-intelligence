//! Synthetic C1/C2 authors and additional protocol edge cases.
use linggan_storage_postgres::Database;
use sqlx::AssertSqlSafe;

/// Additional protocol groups, used separately from the canonical playbook dataset.
pub async fn build(db: &Database) -> Result<(), sqlx::Error> {
    // Every fragment below is a static test fixture; no external input is interpolated.
    let sql = [EXTENDED_WORKS, WORK_SETUP, EXTENDED_OBSERVATIONS].concat();
    sqlx::raw_sql(AssertSqlSafe(sql)).execute(db.pool()).await?;
    Ok(())
}

/// Reassign existing W0 fixture comments before any 0177 source or mention is built.
/// This preserves the complete §8.2 population and its problem-metric standard answers.
pub async fn build_standard(db: &Database) -> Result<(), sqlx::Error> {
    let sql = [STANDARD_WORKS, WORK_SETUP, STANDARD_REASSIGNMENT].concat();
    sqlx::raw_sql(AssertSqlSafe(sql)).execute(db.pool()).await?;
    Ok(())
}

const EXTENDED_WORKS: &str = r#"
CREATE TEMP TABLE creator_seed_work(work_key text, creator_id text, comments integer, protocol integer);
INSERT INTO creator_seed_work VALUES
 ('c1-a','creator-C1',59,NULL), ('c1-b','creator-C1',1,NULL),
 ('c2','creator-C2',40,NULL), ('c1-v1','creator-C1',55,1), ('c2-v1','creator-C2',2,1);

"#;

const STANDARD_WORKS: &str = r#"
CREATE TEMP TABLE creator_seed_work(work_key text, creator_id text, comments integer, protocol integer);
INSERT INTO creator_seed_work VALUES
 ('c1-a','creator-C1',59,NULL), ('c1-b','creator-C1',1,NULL), ('c2','creator-C2',40,NULL);

"#;

const WORK_SETUP: &str = r#"
CREATE TEMP TABLE creator_seed_package AS
SELECT work.work_key, kind.name AS kind,
 md5('creator-package:' || work.work_key || ':' || kind.name)::uuid AS package_ref,
 md5('creator-task:' || work.work_key || ':' || kind.name)::uuid AS task_id,
 md5('creator-attempt:' || work.work_key || ':' || kind.name)::uuid AS attempt_id,
 CASE kind.name WHEN 'detail' THEN '2026-11-03T00:30:00Z'::timestamptz
                ELSE '2026-11-03T01:00:00Z'::timestamptz END AS accepted_at
FROM creator_seed_work work CROSS JOIN (VALUES ('detail'),('comments')) kind(name);

INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type,created_at)
SELECT task_id,md5(task_id::text)||md5(task_id::text||'task'),'{}'::jsonb,
 'manual','xhs','seed','2026-01-01T00:00:00Z' FROM creator_seed_package;
INSERT INTO linggan_runtime_attempt(attempt_id,task_id,producer_instance_id,started_at)
SELECT attempt_id,task_id,md5('creator-producer:'||attempt_id::text)::uuid,'2026-01-01T00:00:00Z'
FROM creator_seed_package;
INSERT INTO linggan_runtime_capture_package(package_ref,attempt_id,task_id,producer_instance_id,
 package_kind,platform,package_hash,observed_at,captured_at,coverage,payload,accepted_at)
SELECT package.package_ref,package.attempt_id,package.task_id,
 md5('creator-producer:'||package.attempt_id::text)::uuid,
 CASE package.kind WHEN 'detail' THEN 'content_detail' ELSE 'comments' END,'xhs',
 md5(package.package_ref::text)||md5(package.package_ref::text||'package'),
 to_char(package.accepted_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'),
 to_char(package.accepted_at AT TIME ZONE 'UTC','YYYY-MM-DD"T"HH24:MI:SS"Z"'),
 '{}'::jsonb, CASE WHEN work.protocol IS NULL THEN '{}'::jsonb
 ELSE jsonb_build_object('collectionProtocol',jsonb_build_object('version',work.protocol)) END,
 package.accepted_at
FROM creator_seed_package package JOIN creator_seed_work work USING(work_key);

INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason,created_at)
SELECT package_ref,0,'accepted_for_library_content','synthetic creator','2026-01-01T00:00:00Z'
FROM creator_seed_package WHERE kind='detail';
INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason,created_at)
SELECT package.package_ref,n,'accepted_for_library_content','synthetic creator','2026-01-01T00:00:00Z'
FROM creator_seed_package package JOIN creator_seed_work work USING(work_key)
CROSS JOIN LATERAL generate_series(1,work.comments) n WHERE package.kind='comments';

INSERT INTO linggan_material_content(platform,content_external_id,public_ref,first_package_ref,created_at)
SELECT 'xhs','creator-'||work.work_key,md5('creator-work:'||work.work_key)::uuid,
 package.package_ref,'2026-01-01T00:00:00Z'
FROM creator_seed_work work JOIN creator_seed_package package USING(work_key) WHERE package.kind='detail';
INSERT INTO linggan_material_content_detail(material_ref,content_public_ref,package_ref,record_ordinal,
 observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,
 published_at_source_text,published_at_source_text_state,searchable_text,author_external_id,
 published_at,published_at_source_field,published_at_source_kind,published_at_precision,
 published_at_parser_version,created_at)
SELECT md5('creator-detail:'||work.work_key)::uuid,md5('creator-work:'||work.work_key)::uuid,
 package.package_ref,0,'2026-11-03T00:30:00Z','synthetic creator','KNOWN',NULL,'UNKNOWN',
 NULL,'UNKNOWN',NULL,'UNKNOWN','seed',work.creator_id,'2026-11-03T00:00:00Z',
 'seedPublishTime','platform_epoch','second','seed-v1','2026-01-01T00:00:00Z'
FROM creator_seed_work work JOIN creator_seed_package package USING(work_key) WHERE package.kind='detail';
INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,use_kind,
 basis_kind,package_ref,record_ordinal,created_at)
SELECT md5('creator-usage:'||work.work_key)::uuid,md5('creator-work:'||work.work_key)::uuid,
 '00000000-0000-4000-8000-000000000001','primary','domain_research','legacy_domain_migration',
 package.package_ref,NULL,'2026-01-01T00:00:00Z'
FROM creator_seed_work work JOIN creator_seed_package package USING(work_key) WHERE package.kind='detail';

"#;

const STANDARD_REASSIGNMENT: &str = r#"
CREATE TEMP TABLE creator_reassignment(comment_id text, work_key text, ordinal integer);
INSERT INTO creator_reassignment
SELECT 'c-work-0-'||n,'c1-a',row_number() OVER (ORDER BY n)::integer
FROM generate_series(1,1000) n WHERE n IN (1,2,3,4,11,13,1000) OR n BETWEEN 800 AND 851;
INSERT INTO creator_reassignment VALUES ('c-work-0-12','c1-b',1);
INSERT INTO creator_reassignment
SELECT 'c-work-0-'||n,'c2',row_number() OVER (ORDER BY n)::integer
FROM generate_series(1,1000) n WHERE n IN (5,6,7) OR n BETWEEN 852 AND 888;

-- The input is a disposable synthetic dataset; no runtime facts are updated.
ALTER TABLE linggan_material_comment DISABLE TRIGGER USER;
UPDATE linggan_material_comment comment
SET content_public_ref=md5('creator-work:'||assignment.work_key)::uuid,
    package_ref=package.package_ref,record_ordinal=assignment.ordinal,
    is_reply=false,root_comment_external_id=comment.comment_external_id,
    parent_comment_external_id=NULL,parent_identity_source_field=NULL,
    author_external_id=CASE comment.comment_external_id
      WHEN 'c-work-0-11' THEN 'commenter-work-0-1'
      WHEN 'c-work-0-12' THEN 'commenter-work-0-2'
      ELSE comment.author_external_id END
FROM creator_reassignment assignment
JOIN creator_seed_package package ON package.work_key=assignment.work_key AND package.kind='comments'
WHERE comment.content_public_ref='00000000-0000-4000-8000-000000000100'
 AND comment.comment_external_id=assignment.comment_id;
ALTER TABLE linggan_material_comment ENABLE TRIGGER USER;
ALTER TABLE linggan_material_comment_first_admission DISABLE TRIGGER USER;
UPDATE linggan_material_comment_first_admission admission
SET content_public_ref=md5('creator-work:'||assignment.work_key)::uuid,first_package_ref=package.package_ref
FROM creator_reassignment assignment
JOIN creator_seed_package package ON package.work_key=assignment.work_key AND package.kind='comments'
WHERE admission.content_public_ref='00000000-0000-4000-8000-000000000100'
 AND admission.comment_external_id=assignment.comment_id;
ALTER TABLE linggan_material_comment_first_admission ENABLE TRIGGER USER;
"#;

const EXTENDED_OBSERVATIONS: &str = r#"
INSERT INTO linggan_material_comment(material_ref,content_public_ref,package_ref,record_ordinal,
 comment_external_id,root_comment_external_id,is_reply,body_text,body_state,author_external_id,observed_at,created_at)
SELECT md5('creator-comment:'||work.work_key||':'||n)::uuid,md5('creator-work:'||work.work_key)::uuid,
 package.package_ref,n,'creator-comment-'||work.work_key||'-'||n,'creator-comment-'||work.work_key||'-'||n,
 false,NULL,'UNKNOWN',
 CASE WHEN work.work_key='c1-a' AND n=59 THEN NULL
      WHEN work.work_key IN ('c1-b','c1-v1') AND n=1 THEN 'creator-commenter-c1-a-1'
      ELSE 'creator-commenter-'||work.work_key||'-'||n END,
 '2026-11-03T01:00:00Z','2026-01-01T00:00:00Z'
FROM creator_seed_work work JOIN creator_seed_package package USING(work_key)
CROSS JOIN LATERAL generate_series(1,work.comments) n WHERE package.kind='comments';

INSERT INTO linggan_material_comment_platform_time(content_public_ref,comment_external_id,published_at,
 precision,source_package_ref,protocol_version,source_kind,created_at)
SELECT comment.content_public_ref,comment.comment_external_id,'2026-11-02T16:00:00Z','day',
 package.package_ref,COALESCE(work.protocol,0),'display_full_date','2026-11-04T00:00:00Z'
FROM creator_seed_work work JOIN creator_seed_package package USING(work_key)
JOIN linggan_material_comment comment ON comment.package_ref=package.package_ref
WHERE package.kind='comments';

CREATE TEMP TABLE creator_seed_membership AS
SELECT comment.material_ref,comment.content_public_ref,comment.comment_external_id
FROM linggan_material_comment comment WHERE comment.comment_external_id IN (
 'creator-comment-c1-a-1','creator-comment-c1-a-2','creator-comment-c1-a-3','creator-comment-c1-a-4',
 'creator-comment-c1-a-58','creator-comment-c1-a-59','creator-comment-c1-b-1',
 'creator-comment-c2-1','creator-comment-c2-2','creator-comment-c2-3',
 'creator-comment-c1-v1-1','creator-comment-c2-v1-1');
ALTER TABLE linggan_comment_research_request DISABLE TRIGGER ALL;
INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,
 observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version,created_at)
SELECT md5('creator-request:'||seed.comment_external_id)::uuid,
 md5('creator-command:'||seed.comment_external_id)::uuid,
 '00000000-0000-4000-8000-000000000001',seed.content_public_ref,seed.material_ref,
 'primary','2026-11-05T00:00:00Z',repeat('b',64),repeat('c',64),setting.current_config_ref,
 md5('creator-batch:'||seed.comment_external_id)::uuid,'comment-research-semantic.v1','2026-11-05T00:00:00Z'
FROM creator_seed_membership seed CROSS JOIN linggan_ai_task_setting setting WHERE setting.task_key='comment_study';
ALTER TABLE linggan_comment_research_request ENABLE TRIGGER ALL;
INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,review_invocation_ref,created_at)
SELECT md5('creator-request:'||seed.comment_external_id)::uuid,0,md5('metric-definition:1')::uuid,
 '00000000-0000-4000-8000-000000004003','2026-11-05T00:00:00Z'
FROM creator_seed_membership seed;
"#;
