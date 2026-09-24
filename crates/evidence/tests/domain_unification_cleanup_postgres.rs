#[allow(dead_code)]
#[path = "support/domain_fixture.rs"]
mod domain;
#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;

use domain::{discovery_card, search_coverage, submit_package_for_domain};
use fixture::proof_database_before_domain_cleanup;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

const PEER_DOMAIN: &str = "00000000-0000-4000-8000-000000000002";

#[tokio::test]
#[ignore = "requires the isolated LOCAL-001 PostgreSQL proof harness"]
async fn cleanup_reprojects_legacy_scope_to_canonical_content_and_retires_old_schema() {
    let database =
        proof_database_before_domain_cleanup("domain_unification_cleanup_reproject").await;
    let external_id = format!("domain-cleanup-{}", Uuid::new_v4());
    let package_ref = submit_package_for_domain(
        &database,
        "00000000-0000-4000-8000-000000000001",
        &format!("domain-cleanup-keyword-{external_id}"),
        "deep_archive",
        json!({"query":"领域迁移证明","ranking":"most_liked","scrollRounds":4}),
        200,
        "discovery_search",
        search_coverage("领域迁移证明", 1),
        json!({"surfaceReceipt":{"stopReason":"bottom_confirmed"}}),
        vec![discovery_card(
            &external_id,
            "规范材料迁移证明",
            "17",
            &format!(
                "https://www.xiaohongshu.com/search_result/{external_id}?xsec_token=ABcleanup"
            ),
        )],
    )
    .await;
    let content_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(&external_id)
    .fetch_one(database.pool())
    .await
    .expect("the accepted Package already has one canonical Content identity");
    let old_canonical_domain: Option<Uuid> =
        sqlx::query_scalar("SELECT domain_ref FROM linggan_material_content WHERE public_ref=$1")
            .bind(content_ref)
            .fetch_one(database.pool())
            .await
            .expect("the pre-cleanup Content still records its former single Domain");
    assert_eq!(
        old_canonical_domain,
        Some(Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap()),
        "the canonical first Package belongs to the home Domain"
    );
    let work: (Uuid, Uuid) = sqlx::query_as(
        "SELECT work.work_order_ref,work.target_ref \
           FROM linggan_runtime_capture_package package \
           JOIN collection_work_order_lease_task lease_task USING(task_id) \
           JOIN collection_work_order_lease lease USING(lease_ref) \
           JOIN collection_work_order work USING(work_order_ref) \
          WHERE package.package_ref=$1",
    )
    .bind(package_ref)
    .fetch_one(database.pool())
    .await
    .expect("the accepted Package traces back to its WorkOrder and Target");
    let (lease_ref, installation_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT lease.lease_ref,installation.installation_ref \
           FROM linggan_runtime_capture_package package \
           JOIN collection_work_order_lease_task lease_task USING(task_id) \
           JOIN collection_work_order_lease lease USING(lease_ref) \
           JOIN plugin_installation installation ON installation.station_ref=lease.station_ref \
          WHERE package.package_ref=$1",
    )
    .bind(package_ref)
    .fetch_one(database.pool())
    .await
    .expect("the Package WorkOrder has its original lease and installation");

    let sample_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO cross_industry_sample \
             (sample_ref,domain_ref,target_ref,platform,content_external_id) \
         VALUES ($1,$2::uuid,$3,'xhs',$4)",
    )
    .bind(sample_ref)
    .bind(PEER_DOMAIN)
    .bind(work.1)
    .bind(&external_id)
    .execute(database.pool())
    .await
    .expect("the legacy sample points to an accepted canonical identity");
    let sample_domain: Uuid =
        sqlx::query_scalar("SELECT domain_ref FROM cross_industry_sample WHERE sample_ref=$1")
            .bind(sample_ref)
            .fetch_one(database.pool())
            .await
            .expect("the sample keeps its separate peer-Domain ownership");
    assert_eq!(sample_domain, Uuid::parse_str(PEER_DOMAIN).unwrap());
    sqlx::query(
        "INSERT INTO collection_work_order_cross_industry_target \
             (work_order_ref,sample_ref,ordinal,comment_limit,reply_expand_limit) \
         VALUES ($1,$2,1,7,1)",
    )
    .bind(work.0)
    .bind(sample_ref)
    .execute(database.pool())
    .await
    .expect("the legacy cross scope has a bounded comment and reply grant");
    let session_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_detail_page_session \
             (session_ref,work_order_ref,cross_industry_sample_ref,owner_installation_ref, \
              grant_request_id,initial_lease_ref,plan_snapshot,plan_hash,state) \
         VALUES ($1,$2,$3,$4,$5,$6,'{}'::jsonb,repeat('a',64),'authorized')",
    )
    .bind(session_ref)
    .bind(work.0)
    .bind(sample_ref)
    .bind(installation_ref)
    .bind(Uuid::new_v4())
    .bind(lease_ref)
    .execute(database.pool())
    .await
    .expect("the legacy detail session stores its sample identity");
    sqlx::query(
        "INSERT INTO collection_execution_input_eligibility \
             (eligibility_ref,target_ref,domain_scope,object_kind,object_ref,capability,state, \
              input_source_kind,input_source_ref,input_source_status) \
         VALUES ($1,$2,'cross_industry','cross_industry_sample',$3,'content_detail','input_blocked', \
                 'cross_industry_sample',$3,'legacy_input_unfrozen')",
    )
    .bind(Uuid::new_v4())
    .bind(work.1)
    .bind(sample_ref)
    .execute(database.pool())
    .await
    .expect("the retired sample has a renewable legacy eligibility row");

    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0104_unified_domain_schema_cleanup.sql"
    ))
    .execute(database.pool())
    .await
    .expect("eligible legacy scope is reprojected before retired tables are removed");

    let mapped_scope = sqlx::query(
        "SELECT ordinal,comment_limit,reply_expand_limit,acquire_media,allow_ocr,allow_asr \
           FROM collection_work_order_material_target \
          WHERE work_order_ref=$1 AND content_public_ref=$2",
    )
    .bind(work.0)
    .bind(content_ref)
    .fetch_one(database.pool())
    .await
    .expect("the canonical material scope replaces the legacy sample scope");
    assert_eq!(mapped_scope.get::<i32, _>("comment_limit"), 7);
    assert_eq!(mapped_scope.get::<i32, _>("reply_expand_limit"), 1);
    assert!(!mapped_scope.get::<bool, _>("acquire_media"));
    assert!(!mapped_scope.get::<bool, _>("allow_ocr"));
    assert!(!mapped_scope.get::<bool, _>("allow_asr"));

    let mapped_session: Option<Uuid> = sqlx::query_scalar(
        "SELECT content_public_ref FROM collection_detail_page_session WHERE session_ref=$1",
    )
    .bind(session_ref)
    .fetch_one(database.pool())
    .await
    .expect("the detail session remains in the canonical identity space");
    assert_eq!(mapped_session, Some(content_ref));

    let peer_usage_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_domain_usage \
          WHERE content_public_ref=$1 AND domain_ref=$2::uuid",
    )
    .bind(content_ref)
    .bind(PEER_DOMAIN)
    .fetch_one(database.pool())
    .await
    .expect("the cleanup does not invent peer-Domain usage from the home Package");
    assert_eq!(peer_usage_count, 0);

    let eligibility_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_execution_input_eligibility \
          WHERE object_ref=$1 OR domain_scope='cross_industry'",
    )
    .bind(sample_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        eligibility_count, 0,
        "renewable eligibility is not accepted history"
    );

    let retired_tables_gone: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_sample') IS NULL \
           AND to_regclass('collection_work_order_cross_industry_target') IS NULL \
           AND to_regclass('cross_industry_note') IS NULL",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(retired_tables_gone, "legacy projection tables are retired");

    let target_has_no_legacy_domain: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM information_schema.columns \
           WHERE table_schema=current_schema() AND table_name='collection_observation_target' \
             AND column_name='domain_ref')",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(
        target_has_no_legacy_domain,
        "Domain membership is represented by its relation"
    );
}

#[tokio::test]
#[ignore = "requires the isolated LOCAL-001 PostgreSQL proof harness"]
async fn cleanup_stops_when_user_authored_legacy_notes_have_no_replacement() {
    let database =
        proof_database_before_domain_cleanup("domain_unification_cleanup_note_guard").await;
    let sample_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO cross_industry_sample \
             (sample_ref,domain_ref,platform,content_external_id) \
         VALUES ($1,'00000000-0000-4000-8000-000000000002','xhs',$2)",
    )
    .bind(sample_ref)
    .bind(format!("legacy-note-{}", Uuid::new_v4()))
    .execute(database.pool())
    .await
    .expect("the legacy sample is stored");
    sqlx::query(
        "INSERT INTO cross_industry_note(note_ref,sample_ref,body) VALUES ($1,$2,'保留这条人工笔记')",
    )
    .bind(Uuid::new_v4())
    .bind(sample_ref)
    .execute(database.pool())
    .await
    .expect("the user-authored note is stored");

    let error = sqlx::raw_sql(include_str!(
        "../../../database/migrations/0104_unified_domain_schema_cleanup.sql"
    ))
    .execute(database.pool())
    .await
    .expect_err("the destructive cleanup must stop before removing an unprojected user note");
    assert!(
        error
            .to_string()
            .contains("cross_industry_note contains user-authored notes with no replacement"),
        "the migration explains the concrete blocker: {error}"
    );
    let sample_table_remains: bool =
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample') IS NOT NULL")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert!(
        sample_table_remains,
        "a failed cleanup leaves the legacy data intact"
    );
}

#[tokio::test]
#[ignore = "requires the isolated LOCAL-001 PostgreSQL proof harness"]
async fn cleanup_drops_unprojected_legacy_samples_without_fabricating_material_history() {
    let database =
        proof_database_before_domain_cleanup("domain_unification_cleanup_optional_projection")
            .await;
    let sample_ref = Uuid::new_v4();
    let external_id = format!("unprojected-{}", sample_ref);
    sqlx::query(
        "INSERT INTO cross_industry_sample \
             (sample_ref,domain_ref,platform,content_external_id) \
         VALUES ($1,'00000000-0000-4000-8000-000000000002','xhs',$2)",
    )
    .bind(sample_ref)
    .bind(&external_id)
    .execute(database.pool())
    .await
    .expect("the disposable legacy sample is stored without a canonical Package projection");

    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0104_unified_domain_schema_cleanup.sql"
    ))
    .execute(database.pool())
    .await
    .expect("optional projection can be skipped while old disposable rows are dropped");

    let no_material_was_invented: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM linggan_material_content WHERE content_external_id=$1) \
           AND to_regclass('cross_industry_sample') IS NULL",
    )
    .bind(external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(
        no_material_was_invented,
        "projection cleanup does not fabricate accepted Content"
    );
}
