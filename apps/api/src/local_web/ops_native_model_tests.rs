//! OPS-NATIVE-WRITE-001: database proofs for the native-versus-imported source boundary.
use super::*;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn native_ops_rows_preserve_source_boundaries_null_semantics_and_receipts() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("opsnat_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let pool = database.pool();
    let member = Uuid::new_v4();
    let domain = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_member \
         (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) \
         VALUES($1,'native@example.test','原生运营','operator','active','synthetic-hash',scope_001_now())",
    )
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO observation_domain (domain_ref,name) VALUES ($1,'原生运营合成业务')")
        .bind(domain)
        .execute(pool)
        .await
        .unwrap();

    let account = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_account \
         (account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name,\
          status,is_retained,profile_link_state,review_status,native_record_state,native_version,\
          native_created_by_member_ref,native_updated_by_member_ref) \
         VALUES($1,'linggan_native',$1::text,$2,'native_active','xhs','原生账号',\
          'active',true,'missing','confirmed','active',1,$3,$3)",
    )
    .bind(account)
    .bind(domain)
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
    let native_account_fields: (bool, bool) = sqlx::query_as(
        "SELECT first_batch_ref IS NULL, source_created_at IS NULL \
         FROM linggan_ops_account WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(native_account_fields, (true, true));

    let malformed_account = Uuid::new_v4();
    let malformed = sqlx::query(
        "INSERT INTO linggan_ops_account \
         (account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name,\
          status,is_retained,profile_link_state,review_status,native_record_state,native_version,\
          native_created_by_member_ref,native_updated_by_member_ref) \
         VALUES($1,'linggan_native','not-the-native-id',$2,'native_active','xhs','不合法账号',\
          'active',true,'missing','confirmed','active',1,$3,$3)",
    )
    .bind(malformed_account)
    .bind(domain)
    .bind(member)
    .execute(pool)
    .await;
    assert!(
        malformed.is_err(),
        "native rows cannot impersonate a source record"
    );

    let publication = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_publication \
         (publication_ref,source_system,source_record_id,account_ref,platform,publication_url,canonical_url,\
          content_id_state,published_at,published_date_state,entry_source,review_status,\
          native_record_state,native_version,native_publisher_member_ref,native_registered_by_member_ref,\
          native_business_domain_ref) \
         VALUES($1,'linggan_native',$1::text,$2,'xhs','https://example.test/post/1',\
          'https://example.test/post/1','pending_match',scope_001_now(),'as_recorded',\
          'linggan_ops_manual','confirmed','active',1,$3,$3,$4)",
    )
    .bind(publication)
    .bind(account)
    .bind(member)
    .bind(domain)
    .execute(pool)
    .await
    .unwrap();
    let wrong_platform = Uuid::new_v4();
    let wrong_platform = sqlx::query(
        "INSERT INTO linggan_ops_publication \
         (publication_ref,source_system,source_record_id,account_ref,platform,publication_url,canonical_url,\
          content_id_state,published_at,published_date_state,entry_source,review_status,\
          native_record_state,native_version,native_publisher_member_ref,native_registered_by_member_ref,\
          native_business_domain_ref) \
         VALUES($1,'linggan_native',$1::text,$2,'douyin','https://example.test/video/1',\
          'https://example.test/video/1','pending_match',scope_001_now(),'as_recorded',\
          'linggan_ops_manual','confirmed','active',1,$3,$3,$4)",
    )
    .bind(wrong_platform)
    .bind(account)
    .bind(member)
    .bind(domain)
    .execute(pool)
    .await;
    assert!(
        wrong_platform.is_err(),
        "a native publication cannot bind a cross-platform account"
    );

    let view_request = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_native_write_request \
         (request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,resulting_version,outcome_code) \
         VALUES($1,'view_report_record',$2,decode(repeat('ab',32),'hex'),'accepted',$3,1,'accepted')",
    )
    .bind(view_request)
    .bind(member)
    .bind(publication)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ops_view_report \
         (report_ref,publication_ref,report_kind,view_count,source_kind,as_of,reported_by_member_ref,request_ref) \
         VALUES($1,$2,'value',0,'linggan_ops_manual',scope_001_now(),$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(publication)
    .bind(member)
    .bind(view_request)
    .execute(pool)
    .await
    .unwrap();
    let manual_zero: (i64, Option<Uuid>) = sqlx::query_as(
        "SELECT view_count, batch_ref FROM linggan_ops_view_report WHERE request_ref=$1",
    )
    .bind(view_request)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        manual_zero,
        (0, None),
        "manual zero is a valid reported value, not an import"
    );

    let daily = Uuid::new_v4();
    let daily_request = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_native_write_request \
         (request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,resulting_version,outcome_code) \
         VALUES($1,'daily_report_record',$2,decode(repeat('cd',32),'hex'),'accepted',$3,1,'accepted')",
    )
    .bind(daily_request)
    .bind(member)
    .bind(daily)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ops_daily_report \
         (daily_ref,source_system,source_record_id,operator_name,business_date,links_opened_count,\
          followers_gained,review_status,native_member_ref,native_version,native_created_by_member_ref,\
          native_updated_by_member_ref) \
         VALUES($1,'linggan_native',$1::text,'原生运营','2026-10-23',NULL,NULL,'confirmed',$2,1,$2,$2)",
    )
    .bind(daily)
    .bind(member)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ops_daily_report_revision \
         (revision_ref,daily_ref,previous_version,resulting_version,links_opened_count,followers_gained,\
          changed_by_member_ref,request_ref) \
         VALUES($1,$2,NULL,1,NULL,NULL,$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(daily)
    .bind(member)
    .bind(daily_request)
    .execute(pool)
    .await
    .unwrap();
    let daily_values: (Option<i32>, Option<i32>, i64) = sqlx::query_as(
        "SELECT links_opened_count, followers_gained, native_version \
         FROM linggan_ops_daily_report WHERE daily_ref=$1",
    )
    .bind(daily)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        daily_values,
        (None, None, 1),
        "unfilled native daily values stay unknown"
    );

    let duplicate_daily = sqlx::query(
        "INSERT INTO linggan_ops_daily_report \
         (daily_ref,source_system,source_record_id,operator_name,business_date,links_opened_count,\
          followers_gained,review_status,native_member_ref,native_version,native_created_by_member_ref,\
          native_updated_by_member_ref) \
         VALUES($1,'linggan_native',$1::text,'原生运营','2026-10-23',0,0,'confirmed',$2,1,$2,$2)",
    )
    .bind(Uuid::new_v4())
    .bind(member)
    .execute(pool)
    .await;
    assert!(
        duplicate_daily.is_err(),
        "one stable member has at most one native daily row per day"
    );

    let mutate_receipt = sqlx::query(
        "UPDATE linggan_ops_native_write_request SET outcome_code='rewritten' WHERE request_ref=$1",
    )
    .bind(daily_request)
    .execute(pool)
    .await;
    assert!(mutate_receipt.is_err(), "write receipts stay append-only");
}
