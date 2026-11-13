//! T1.2 standard dataset, protocol separation, and incremental/full equivalence.
#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema_fixture;
#[path = "support/opportunity_dataset.rs"]
mod opportunity_dataset;

use linggan_intelligence::opportunity::{
    CommenterDigest, RebuildMode, check_invariants, project_comment_platform_times,
    rebuild_problem_metrics, sync_comment_mentions,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;
use uuid::Uuid;

async fn database() -> Database {
    let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
    let db = isolated_proof_schema(
        &url,
        "opportunity_t12_metrics",
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .expect("isolated schema");
    opportunity_dataset::build(&db).await.unwrap();
    sqlx::raw_sql(SEED).execute(db.pool()).await.unwrap();
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    sync_comment_mentions(&db, &digest).await.unwrap();
    sqlx::raw_sql(
        "ALTER TABLE linggan_opportunity_taxonomy_version_history DISABLE TRIGGER USER; \
         UPDATE linggan_opportunity_taxonomy_version_history SET created_at='2026-11-01T00:00:00Z'; \
         ALTER TABLE linggan_opportunity_taxonomy_version_history ENABLE TRIGGER USER; \
         UPDATE linggan_opportunity_problem SET created_at='2026-11-01T00:00:00Z'; \
         UPDATE linggan_opportunity_metric_definition SET created_at='2026-11-01T00:00:00Z'; \
         UPDATE linggan_opportunity_metric_definition_current SET switched_at='2026-11-01T00:00:00Z'; \
         UPDATE linggan_opportunity_comment_mention SET excluded_reason='marketing' \
         WHERE comment_external_id='c-work-0-13';",
    ).execute(db.pool()).await.unwrap();
    db
}

const SEED: &str = r#"
ALTER TABLE linggan_material_comment DISABLE TRIGGER USER;
UPDATE linggan_material_comment SET author_external_id='commenter-work-0-1'
WHERE content_public_ref='00000000-0000-4000-8000-000000000100' AND comment_external_id='c-work-0-11';
UPDATE linggan_material_comment SET author_external_id='commenter-work-0-2'
WHERE content_public_ref='00000000-0000-4000-8000-000000000100' AND comment_external_id='c-work-0-12';
ALTER TABLE linggan_material_comment ENABLE TRIGGER USER;
CREATE TEMP TABLE metric_seed_membership(problem_number integer, comment_id text);
INSERT INTO metric_seed_membership SELECT 1, 'c-work-0-' || n FROM generate_series(1,13) n;
INSERT INTO metric_seed_membership VALUES (1,'c-work-0-1000'),(1,'c-work-0-self'),
 (1,'c-work-unknown-1'),(1,'c-work-reference-1'),(1,'c-work-0-late-1'),
 (1,'c-work-1-1'),(1,'c-work-2-1'),(1,'c-work-3-1'),(1,'c-work-4-1'),
 (2,'c-work-0-20'),(2,'c-work-0-21'),(2,'c-work-0-22'),(3,'c-work-0-23');
ALTER TABLE linggan_model_entry DISABLE TRIGGER ALL;
INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin)
VALUES ('00000000-0000-4000-8000-000000004001','00000000-0000-4000-8000-000000004002','synthetic-model','manual');
ALTER TABLE linggan_model_entry ENABLE TRIGGER ALL;
ALTER TABLE linggan_model_invocation DISABLE TRIGGER ALL;
INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,
 reserved_tokens,charged_tokens,task_key,task_config_ref,budget_day,usage_state,dispatch_state,finished_at)
SELECT '00000000-0000-4000-8000-000000004003','00000000-0000-4000-8000-000000004002',
 '00000000-0000-4000-8000-000000004001','analyze',repeat('a',64),'succeeded',1,1,
 'comment_problem_pair_review',current_config_ref,'2026-11-01','unknown_reserved','started','2026-11-04T00:00:00Z'
FROM linggan_ai_task_setting WHERE task_key='comment_problem_pair_review';
ALTER TABLE linggan_model_invocation ENABLE TRIGGER ALL;
ALTER TABLE linggan_comment_research_request DISABLE TRIGGER ALL;
INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,
 observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version,created_at)
SELECT md5('metric-request:' || comment.comment_external_id)::uuid,
 md5('metric-command:' || comment.comment_external_id)::uuid,
 '00000000-0000-4000-8000-000000000001', comment.content_public_ref, comment.material_ref,
 'primary', '2026-11-05T00:00:00Z',repeat('b',64),repeat('c',64),setting.current_config_ref,
 md5('metric-batch:' || comment.comment_external_id)::uuid,'comment-research-semantic.v1',
 CASE WHEN comment.comment_external_id='c-work-0-late-1' THEN '2026-11-10T01:30:00Z'::timestamptz
      ELSE '2026-11-05T00:00:00Z'::timestamptz END
FROM linggan_material_comment comment JOIN metric_seed_membership seed ON seed.comment_id=comment.comment_external_id
CROSS JOIN linggan_ai_task_setting setting WHERE setting.task_key='comment_study';
ALTER TABLE linggan_comment_research_request ENABLE TRIGGER ALL;
ALTER TABLE linggan_comment_problem_definition DISABLE TRIGGER ALL;
INSERT INTO linggan_comment_problem_definition(definition_ref,problem_ref,domain_ref,identity_hash,label,actor,goal,
 barrier,source_request_ref,source_signal_ordinal,review_invocation_ref,created_at)
SELECT md5('metric-definition:' || n)::uuid,md5('metric-problem:' || n)::uuid,
 '00000000-0000-4000-8000-000000000001',repeat(n::text,64),'synthetic P' || n,'actor','goal','barrier',
 md5('metric-request:' || CASE n WHEN 1 THEN 'c-work-0-1' WHEN 2 THEN 'c-work-0-20' ELSE 'c-work-0-23' END)::uuid,
 0,'00000000-0000-4000-8000-000000004003','2026-11-05T00:00:00Z'
FROM generate_series(1,3) n;
ALTER TABLE linggan_comment_problem_definition ENABLE TRIGGER ALL;
INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,review_invocation_ref,created_at)
SELECT md5('metric-request:' || seed.comment_id)::uuid,0,md5('metric-definition:' || seed.problem_number)::uuid,
 '00000000-0000-4000-8000-000000004003',
 CASE WHEN seed.comment_id='c-work-0-late-1' THEN '2026-11-10T01:30:00Z'::timestamptz
      ELSE '2026-11-05T00:00:00Z'::timestamptz END
FROM metric_seed_membership seed;
"#;

async fn row(
    db: &Database,
    problem: i32,
    version: i32,
    protocol: Option<i32>,
) -> sqlx::postgres::PgRow {
    sqlx::query("SELECT * FROM linggan_opportunity_problem_metric_period \
                 WHERE problem_ref=md5('metric-problem:' || $1::text)::uuid AND period_start='2026-11-02' \
                 AND metric_def_version=$2 AND protocol_version IS NOT DISTINCT FROM $3")
        .bind(problem).bind(version).bind(protocol).fetch_one(db.pool()).await.unwrap()
}

async fn seed_historical_identity(db: &Database) {
    sqlx::raw_sql(r#"
INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason)
SELECT package_ref,4,'accepted_for_library_content','synthetic identity conflict'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late';
INSERT INTO linggan_material_comment(material_ref,content_public_ref,package_ref,record_ordinal,
 comment_external_id,root_comment_external_id,is_reply,body_text,body_state,author_external_id,observed_at,created_at)
SELECT md5('metric-conflict')::uuid,'00000000-0000-4000-8000-000000000100',package_ref,4,
 'c-work-0-25','c-work-0-25',false,NULL,'UNKNOWN','synthetic-conflict-author',
 '2026-11-10T01:00:00Z','2026-11-10T02:00:00Z'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late';
ALTER TABLE linggan_material_comment_first_admission DISABLE TRIGGER USER;
DELETE FROM linggan_material_comment_first_admission WHERE comment_external_id='c-work-0-22';
ALTER TABLE linggan_material_comment_first_admission ENABLE TRIGGER USER;
INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason)
SELECT package_ref,5,'accepted_for_library_content','synthetic historical identity'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late';
ALTER TABLE linggan_material_comment DISABLE TRIGGER collection_comment_first_admitted;
INSERT INTO linggan_material_comment(material_ref,content_public_ref,package_ref,record_ordinal,
 comment_external_id,root_comment_external_id,is_reply,body_text,body_state,author_external_id,observed_at,created_at)
SELECT md5('metric-historical-identity')::uuid,'00000000-0000-4000-8000-000000000100',package_ref,5,
 'c-work-0-22','c-work-0-22',false,NULL,'UNKNOWN','commenter-work-0-20',
 '2026-11-03T00:30:00Z','2026-11-10T02:00:00Z'
FROM opportunity_seed_package WHERE work_key='work-0' AND kind='late';
ALTER TABLE linggan_material_comment ENABLE TRIGGER collection_comment_first_admitted;
"#).execute(db.pool()).await.unwrap();
}

async fn verify_missing_history_and_future(
    db: &Database,
    domain: Uuid,
    digest: &CommenterDigest,
    as_of: &str,
) {
    sqlx::query("DELETE FROM linggan_opportunity_metric_shadow WHERE current_version=1")
        .execute(db.pool())
        .await
        .unwrap();
    let missing_shadow =
        rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Preview).await;
    assert!(matches!(
        missing_shadow,
        Err(linggan_intelligence::opportunity::MetricCalculationError::ShadowUnavailable)
    ));
    sqlx::query(
        "INSERT INTO linggan_opportunity_metric_shadow(current_version,shadow_version) VALUES(1,2)",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(
        "ALTER TABLE linggan_opportunity_taxonomy_version_history DISABLE TRIGGER USER; \
        UPDATE linggan_opportunity_taxonomy_version_history SET created_at='2026-11-12T00:00:00Z'; \
        ALTER TABLE linggan_opportunity_taxonomy_version_history ENABLE TRIGGER USER;",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let missing_taxonomy =
        rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Preview).await;
    assert!(matches!(
        missing_taxonomy,
        Err(linggan_intelligence::opportunity::MetricCalculationError::TaxonomyUnavailable)
    ));
    sqlx::raw_sql(
        "ALTER TABLE linggan_opportunity_taxonomy_version_history DISABLE TRIGGER USER; \
        UPDATE linggan_opportunity_taxonomy_version_history SET created_at='2026-11-01T00:00:00Z'; \
        ALTER TABLE linggan_opportunity_taxonomy_version_history ENABLE TRIGGER USER; \
        UPDATE linggan_opportunity_problem SET created_at='2026-11-12T00:00:00Z';",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let missing_registration =
        rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Preview).await;
    assert!(matches!(missing_registration, Err(linggan_intelligence::opportunity::MetricCalculationError::ProblemRegistrationUnavailable)));
    sqlx::query("UPDATE linggan_opportunity_problem SET created_at='2026-11-01T00:00:00Z'")
        .execute(db.pool())
        .await
        .unwrap();
    let future = rebuild_problem_metrics(
        db,
        domain,
        "2099-01-01T00:00:00Z",
        digest,
        RebuildMode::Full,
    )
    .await;
    assert!(matches!(
        future,
        Err(linggan_intelligence::opportunity::MetricCalculationError::FutureAsOf)
    ));
    let initial_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_problem_metric_period")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(initial_rows, 0);
}

async fn verify_standard_results(
    db: &Database,
    domain: Uuid,
    digest: &CommenterDigest,
    as_of: &str,
) {
    let first = rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Incremental)
        .await
        .unwrap();
    assert_eq!((first.definitions, first.problems_rebuilt), (2, 6));
    assert_eq!(
        first
            .identity_conflicts
            .iter()
            .find(|item| item.metric_def_version == 1)
            .unwrap()
            .comments,
        0
    );
    assert!(first.invariants.as_ref().unwrap().incomplete);
    for (reason, comments) in [
        ("not_in_domain_primary", 3),
        ("work_author_unknown", 5),
        ("period_anchor_missing", 4),
        ("observed_at_unparseable", 0),
        ("restricted", 2),
        ("creator_self", 1),
    ] {
        let excluded = first
            .exclusions
            .iter()
            .find(|item| item.metric_def_version == 1 && item.reason == reason)
            .expect("exclusion reason must be reported");
        assert_eq!(excluded.comments, comments, "{reason}");
    }
    let p1 = row(db, 1, 1, None).await;
    assert_eq!((p1.get::<i32, _>("k"), p1.get::<i32, _>("n")), (10, 1000));
    assert_eq!(
        (
            p1.get::<i32, _>("baseline_k"),
            p1.get::<i32, _>("baseline_n")
        ),
        (4, 4800)
    );
    assert_eq!(p1.get::<i32, _>("mentioners_total"), 14);
    assert_eq!(p1.get::<String, _>("evidence_tier"), "significant");
    assert!(!p1.get::<bool, _>("no_baseline"));
    assert!((p1.get::<f64, _>("ci_low") - 0.005441).abs() < 1e-6);
    assert!((p1.get::<f64, _>("ci_high") - 0.018309).abs() < 1e-6);
    let p2 = row(db, 2, 1, None).await;
    let p3 = row(db, 3, 1, None).await;
    assert_eq!(
        (p2.get::<i32, _>("k"), p2.get::<String, _>("evidence_tier")),
        (3, "established".into())
    );
    assert_eq!(
        (p3.get::<i32, _>("k"), p3.get::<String, _>("evidence_tier")),
        (1, "candidate".into())
    );
    let shadow = row(db, 1, 2, None).await;
    assert_eq!(shadow.get::<i32, _>("n"), 0);
    assert_eq!(shadow.get::<Option<f64>, _>("ci_low"), None);
    assert_eq!(
        shadow.get::<Option<String>, _>("empty_reason").as_deref(),
        Some("no_eligible_comments")
    );
    let again = rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Incremental)
        .await
        .unwrap();
    assert_eq!((again.problems_rebuilt, again.problems_unchanged), (0, 6));
    let wrong_key = CommenterDigest::from_key(vec![43; 32]).unwrap();
    let error = rebuild_problem_metrics(db, domain, as_of, &wrong_key, RebuildMode::Incremental)
        .await
        .expect_err("a changed key under the same digest version must be refused");
    assert!(matches!(
        error,
        linggan_intelligence::opportunity::MetricCalculationError::MentionIntegrity(
            linggan_intelligence::opportunity::MentionSyncError::DigestKeyOrSourceMismatch { .. }
        )
    ));
}

async fn verify_late_results(db: &Database, domain: Uuid, digest: &CommenterDigest) {
    let later = rebuild_problem_metrics(
        db,
        domain,
        "2026-11-11T00:00:00Z",
        digest,
        RebuildMode::Incremental,
    )
    .await
    .unwrap();
    assert!(later.problems_rebuilt > 0);
    assert_eq!(
        later
            .identity_conflicts
            .iter()
            .find(|item| item.metric_def_version == 1)
            .unwrap()
            .comments,
        2
    );
    let late = row(db, 1, 1, None).await;
    assert_eq!(
        (
            late.get::<i32, _>("k"),
            late.get::<i32, _>("n"),
            late.get::<i32, _>("mentioners_total")
        ),
        (11, 1003, 15)
    );
    assert!((late.get::<f64, _>("ci_low") - 0.006135).abs() < 1e-6);
    assert!((late.get::<f64, _>("ci_high") - 0.019531).abs() < 1e-6);
}

async fn verify_history_preview(
    db: &Database,
    domain: Uuid,
    digest: &CommenterDigest,
    as_of: &str,
) {
    sqlx::raw_sql(r#"
INSERT INTO linggan_material_comment_platform_time(content_public_ref,comment_external_id,published_at,
 precision,source_package_ref,protocol_version,source_kind,created_at)
VALUES ('00000000-0000-4000-8000-000000000101','c-work-1-100','2026-12-01T00:00:00Z',
 'millisecond',md5('opportunity-package:work-1:comments')::uuid,1,'epoch','2026-11-12T00:00:00Z');
"#).execute(db.pool()).await.unwrap();
    let preview = rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Preview)
        .await
        .unwrap();
    assert_eq!(
        preview
            .invariants
            .as_ref()
            .unwrap()
            .checks
            .iter()
            .find(|check| check.id == "I11")
            .unwrap()
            .violations,
        0
    );
    let p1_ref: Uuid = sqlx::query_scalar("SELECT md5('metric-problem:1')::uuid")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let historical = preview
        .preview
        .iter()
        .find(|period| {
            period.problem_ref == p1_ref
                && period.metric_def_version == 1
                && period.protocol_version.is_none()
                && period.period_start == "2026-11-02"
        })
        .unwrap();
    assert_eq!((historical.k, historical.n), (10, 1000));
    assert_eq!(row(db, 1, 1, None).await.get::<i32, _>("k"), 11);
    let p2_ref: Uuid = sqlx::query_scalar("SELECT md5('metric-problem:2')::uuid")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let p2_historical = preview
        .preview
        .iter()
        .find(|period| {
            period.problem_ref == p2_ref
                && period.metric_def_version == 1
                && period.protocol_version.is_none()
                && period.period_start == "2026-11-02"
        })
        .unwrap();
    assert_eq!(p2_historical.k, 3);
    assert_eq!(row(db, 2, 1, None).await.get::<i32, _>("k"), 2);
    let invalid = rebuild_problem_metrics(
        db,
        domain,
        "2026-11-12T01:00:00Z",
        digest,
        RebuildMode::Preview,
    )
    .await;
    assert!(
        matches!(invalid, Err(linggan_intelligence::opportunity::MetricCalculationError::InvariantViolations { checks }) if checks.contains(&"I11"))
    );
    assert_eq!(row(db, 1, 1, None).await.get::<i32, _>("k"), 11);
    sqlx::raw_sql("ALTER TABLE linggan_material_comment_platform_time DISABLE TRIGGER USER; \
        DELETE FROM linggan_material_comment_platform_time WHERE comment_external_id='c-work-1-100'; \
        ALTER TABLE linggan_material_comment_platform_time ENABLE TRIGGER USER;")
        .execute(db.pool()).await.unwrap();
}

async fn verify_protocol_and_shadow(
    db: &Database,
    domain: Uuid,
    digest: &CommenterDigest,
    as_of: &str,
) {
    sqlx::raw_sql("ALTER TABLE linggan_runtime_capture_package DISABLE TRIGGER USER; \
      UPDATE linggan_runtime_capture_package SET payload=jsonb_set(payload,'{collectionProtocol}', \
      '{\"version\":1}'::jsonb,true) WHERE package_ref=(SELECT package_ref FROM opportunity_seed_package \
      WHERE work_key='work-0' AND kind='comments'); \
      ALTER TABLE linggan_runtime_capture_package ENABLE TRIGGER USER;").execute(db.pool()).await.unwrap();
    let older = rebuild_problem_metrics(db, domain, as_of, digest, RebuildMode::Incremental).await;
    assert!(matches!(
        older,
        Err(linggan_intelligence::opportunity::MetricCalculationError::NewerResultExists)
    ));
    rebuild_problem_metrics(
        db,
        domain,
        "2026-11-11T00:00:00Z",
        digest,
        RebuildMode::Incremental,
    )
    .await
    .unwrap();
    let new_protocol = row(db, 1, 1, Some(1)).await;
    // One synthetic historical-identity case has no first admission and remains unknown.
    assert_eq!(new_protocol.get::<i32, _>("n"), 999);
    assert_eq!(new_protocol.get::<i32, _>("baseline_n"), 0);
    assert!(new_protocol.get::<bool, _>("no_baseline"));
    assert_ne!(
        new_protocol.get::<String, _>("evidence_tier"),
        "significant"
    );
    let full = rebuild_problem_metrics(
        db,
        domain,
        "2026-11-11T00:00:00Z",
        digest,
        RebuildMode::Full,
    )
    .await
    .unwrap();
    assert_eq!(full.problems_rebuilt, 6);
    let p1_full = row(db, 1, 1, Some(1)).await;
    assert_eq!(
        (p1_full.get::<i32, _>("k"), p1_full.get::<i32, _>("n")),
        (10, 999)
    );
    opportunity_dataset::build_platform_times(db).await.unwrap();
    project_comment_platform_times(db).await.unwrap();
    rebuild_problem_metrics(
        db,
        domain,
        "2026-11-11T00:00:00Z",
        digest,
        RebuildMode::Incremental,
    )
    .await
    .unwrap();
    let shadow_with_time = row(db, 1, 2, Some(1)).await;
    assert!(
        shadow_with_time.get::<i32, _>("n") > 0,
        "shadow basis persists eligible comments"
    );
    let invariants = check_invariants(db).await.unwrap();
    for id in ["I1", "I2", "I3", "I6", "I6b", "I8", "I11"] {
        let check = invariants.checks.iter().find(|c| c.id == id).unwrap();
        assert!(check.available, "{id}");
        assert_eq!(check.violations, 0, "{id}");
    }
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn standard_answers_protocol_groups_and_incremental_match() {
    let db = database().await;
    seed_historical_identity(&db).await;
    let domain = Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap();
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    let as_of = opportunity_dataset::AS_OF;
    verify_missing_history_and_future(&db, domain, &digest, as_of).await;
    verify_standard_results(&db, domain, &digest, as_of).await;
    verify_late_results(&db, domain, &digest).await;
    verify_history_preview(&db, domain, &digest, as_of).await;
    verify_protocol_and_shadow(&db, domain, &digest, as_of).await;
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn shadow_versions_avoid_existing_numbers_and_follow_current_changes() {
    let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("disposable proof URL");
    let migration =
        include_str!("../../../database/migrations/0213_opportunity_problem_metric_period.sql");
    let before = full_schema_fixture::FULL_MIGRATIONS.replace(migration, "");
    let db = isolated_proof_schema(&url, "opportunity_t12_shadow_versions", &before)
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_opportunity_metric_definition(version,params,created_by) \
                 SELECT 2,params,'synthetic-existing' FROM linggan_opportunity_metric_definition WHERE version=1")
        .execute(db.pool()).await.unwrap();
    sqlx::raw_sql(migration).execute(db.pool()).await.unwrap();
    let initial: i32 = sqlx::query_scalar(
        "SELECT shadow_version FROM linggan_opportunity_metric_shadow WHERE current_version=1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(initial, 3);
    sqlx::query("INSERT INTO linggan_opportunity_metric_definition(version,params,created_by) \
                 SELECT 4,params,'synthetic-new-current' FROM linggan_opportunity_metric_definition WHERE version=1")
        .execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_opportunity_metric_definition_current SET version=4, switched_at=now() WHERE singleton")
        .execute(db.pool()).await.unwrap();
    let row = sqlx::query("SELECT shadow.shadow_version, definition.params->>'period_basis' AS basis \
        FROM linggan_opportunity_metric_shadow shadow \
        JOIN linggan_opportunity_metric_definition definition ON definition.version=shadow.shadow_version \
        WHERE shadow.current_version=4").fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<i32, _>("shadow_version"), 5);
    assert_eq!(row.get::<String, _>("basis"), "comment_published");
}
