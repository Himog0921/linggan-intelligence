//! OPS-NATIVE-HTTP-001 isolated Axum/PostgreSQL proof.  All members and facts are synthetic.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_intelligence::ops_ledger::{
    apply_ops_import, preview_ops_import,
    read::{
        OpsBusiness, OpsOverviewQuery, OpsPublicationFilter, OverviewSlice, read_ops_accounts,
        read_ops_businesses, read_ops_daily, read_ops_overview, read_ops_overview_trend,
        read_ops_publications, read_ops_summary,
    },
};
use linggan_contracts::{parse_producer_attempt, parse_producer_submission};
use linggan_evidence::{
    AccountEligibilityObservation, DispatchDecision, activate_installation_credential,
    bind_observation_account, decide_dispatch, report_account_eligibility,
    rotate_installation_credential, set_station_accepting, start_producer_attempt,
    submit_producer_package,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../../crates/intelligence/tests/support/ops_export_fixture.rs"]
mod ops_export;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn native_only_summary_is_available_without_forging_an_import_batch() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_nsum_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let owner = member(
        database.pool(),
        "native-summary@example.test",
        "原生汇总",
        "operator",
    )
    .await;
    let domain = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain (domain_ref,name) VALUES($1,'原生汇总领域')")
        .bind(domain)
        .execute(database.pool())
        .await
        .unwrap();
    let app = app_with_database(database.clone());
    let cookie = session(database.pool(), owner).await;
    let (status, _) = post(
        app,
        "/api/local/ops/native/accounts",
        Some(&cookie),
        json!({
            "requestId": Uuid::new_v4(),
            "domainRef": domain,
            "platform": "xhs",
            "accountName": "仅原生账号",
            "profileUrl": "https://example.test/xhs/native-summary",
            "platformUserId": "0123456789abcdef01234567"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let summary = read_ops_summary(&database, OpsBusiness::All).await.unwrap();
    assert_eq!(
        summary.as_of, None,
        "native writes never manufacture import provenance"
    );
    assert!(summary.native_data_present);
    assert_eq!(
        (summary.active_accounts, summary.ledger_accounts),
        (Some(1), Some(1))
    );
    assert_eq!(
        (summary.month_published, summary.month_hits),
        (Some(0), Some(0))
    );
    assert_eq!(summary.month_followers_gained.value, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn member_deactivation_revokes_sessions_and_leaves_current_accounts_pending() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_member_stop_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url, &schema, full_schema_fixture::FULL_MIGRATIONS,
    ).await.unwrap();
    let admin = member(database.pool(), "stop-admin@example.test", "停用管理员", "admin").await;
    let departing = member(database.pool(), "stop-member@example.test", "离职运营", "operator").await;
    let domain = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain (domain_ref,name) VALUES($1,'停用合成业务')")
        .bind(domain).execute(database.pool()).await.unwrap();
    let app = app_with_database(database.clone());
    let admin_cookie = session(database.pool(), admin).await;
    let departing_cookie = session(database.pool(), departing).await;
    let (status, account) = post(app.clone(), "/api/local/ops/native/accounts", Some(&departing_cookie), json!({
        "requestId":Uuid::new_v4(),"domainRef":domain,"platform":"xhs","accountName":"待接管账号",
        "profileUrl":"https://example.test/xhs/stop","platformUserId":"0123456789abcdef01234567","deviceCode":null,"followerCount":null,"ipLocation":null,"notes":null
    })).await;
    assert_eq!(status, StatusCode::CREATED);
    let account_ref: Uuid = account["receipt"]["recordRef"].as_str().unwrap().parse().unwrap();
    let request_id = Uuid::new_v4();
    let body = json!({"requestId":request_id,"expectedVersion":1,"reason":"合成离职停用"});
    let (status, stopped) = post(app.clone(), &format!("/api/local/auth/members/{departing}/deactivate"), Some(&admin_cookie), body.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stopped["outcome"], "accepted");
    assert_eq!(stopped["unassignedAccountCount"], 1);
    let state: (String, Option<Uuid>, i64, i64) = sqlx::query_as(
        "SELECT member.state,account.current_responsible_member_ref,account.responsibility_version, \
                (SELECT count(*) FROM linggan_ops_session WHERE member_ref=$1 AND revoked_at IS NOT NULL) \
         FROM linggan_ops_member member CROSS JOIN linggan_ops_account account \
         WHERE member.member_ref=$1 AND account.account_ref=$2",
    ).bind(departing).bind(account_ref).fetch_one(database.pool()).await.unwrap();
    assert_eq!(state, ("disabled".to_owned(), None, 2, 1));
    let changes: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ops_account_responsibility_change WHERE account_ref=$1 AND previous_member_ref=$2 AND next_member_ref IS NULL")
        .bind(account_ref).bind(departing).fetch_one(database.pool()).await.unwrap();
    assert_eq!(changes, 1);
    let (status, _) = get(app.clone(), "/api/local/ops/members", Some(&departing_cookie)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, replay) = post(app, &format!("/api/local/auth/members/{departing}/deactivate"), Some(&admin_cookie), body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn native_commands_are_session_bound_idempotent_and_versioned() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_native_http_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let owner = member(
        database.pool(),
        "owner@example.test",
        "本人运营",
        "operator",
    )
    .await;
    let other = member(
        database.pool(),
        "other@example.test",
        "他人运营",
        "operator",
    )
    .await;
    let domain = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain (domain_ref,name) VALUES($1,'原生 HTTP 合成领域')")
        .bind(domain)
        .execute(database.pool())
        .await
        .unwrap();
    let owner_cookie = session(database.pool(), owner).await;
    let other_cookie = session(database.pool(), other).await;
    sqlx::query(
        "INSERT INTO observation_domain (domain_ref,name) VALUES($1,'历史迁入合成业务') \
         ON CONFLICT (domain_ref) DO NOTHING",
    )
    .bind(ops_export::ADHD_DOMAIN)
    .execute(database.pool())
    .await
    .unwrap();
    let export = ops_export::first_export();
    let historical_request = export.request("2026-10-12T22:43:00+08:00");
    let historical_preview = preview_ops_import(&database, &historical_request)
        .await
        .unwrap();
    apply_ops_import(
        &database,
        &historical_request,
        &historical_preview.preview_hash,
    )
    .await
    .unwrap();
    assert!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ops_publication_field_adoption \
             WHERE source_kind='workbench_import'",
        )
        .fetch_one(database.pool())
        .await
        .unwrap()
            > 0,
        "an import writes its current fields into the same adoption stream rather than relying on the registration projection",
    );
    let unassigned_publication: Uuid = sqlx::query_scalar(
        "SELECT publication_ref FROM linggan_ops_publication \
         WHERE source_system='content_workbench' AND account_ref IS NULL",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let app = app_with_database(database.clone());

    let account_request = Uuid::new_v4();
    let account_body = json!({
        "requestId": account_request,
        "domainRef": domain,
        "platform": "xhs",
        "accountName": "原生账号",
        "profileUrl": "https://example.test/xhs/profile",
        "platformUserId": "0123456789abcdef01234567"
    });
    let unauthenticated = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        None,
        account_body.clone(),
    )
    .await;
    assert_eq!(unauthenticated.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        hostile_post(
            app.clone(),
            "/api/local/ops/native/accounts",
            account_body.clone(),
        )
        .await,
        StatusCode::FORBIDDEN,
        "native commands remain behind the enclosing local-origin guard"
    );
    let invalid_request = Uuid::new_v4();
    let invalid_daily = json!({
        "requestId": invalid_request,
        "businessDate": "2026-02-30",
        "linksOpenedCount": 0,
        "followersGained": 0
    });
    let (invalid_status, invalid) = post(
        app.clone(),
        "/api/local/ops/native/daily-reports",
        Some(&owner_cookie),
        invalid_daily.clone(),
    )
    .await;
    assert_eq!(invalid_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid["receipt"]["outcome"], "rejected");
    assert_eq!(invalid["receipt"]["code"], "ops_native_daily_invalid");
    let (invalid_replay_status, invalid_replay) = post(
        app.clone(),
        "/api/local/ops/native/daily-reports",
        Some(&owner_cookie),
        invalid_daily,
    )
    .await;
    assert_eq!(invalid_replay_status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid_replay["receipt"]["replayed"], true);
    let mut uppercase_identity = account_body.clone();
    uppercase_identity["requestId"] = json!(Uuid::new_v4());
    uppercase_identity["platformUserId"] = json!("0123456789ABCDEF01234567");
    let invalid_identity = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        Some(&owner_cookie),
        uppercase_identity,
    )
    .await;
    assert_eq!(invalid_identity.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid_identity.1["receipt"]["code"],
        "ops_native_account_identity_invalid"
    );
    let unassigned_request = Uuid::new_v4();
    let unassigned_body = json!({
        "requestId": unassigned_request,
        "reportKind": "value",
        "viewCount": 1,
        "asOf": "2026-10-23T08:00:00+08:00"
    });
    let unassigned = post(
        app.clone(),
        &format!("/api/local/ops/native/publications/{unassigned_publication}/view-reports"),
        Some(&owner_cookie),
        unassigned_body.clone(),
    )
    .await;
    assert_eq!(unassigned.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        unassigned.1["receipt"]["code"],
        "ops_native_account_unassigned"
    );
    let unassigned_replay = post(
        app.clone(),
        &format!("/api/local/ops/native/publications/{unassigned_publication}/view-reports"),
        Some(&owner_cookie),
        unassigned_body,
    )
    .await;
    assert_eq!(unassigned_replay.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(unassigned_replay.1["receipt"]["replayed"], true);
    let (status, account_response) = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        Some(&owner_cookie),
        account_body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(account_response["receipt"]["outcome"], "accepted");
    let account_ref = uuid(&account_response["receipt"]["recordRef"]);
    let (replay_status, replay) = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        Some(&owner_cookie),
        account_body.clone(),
    )
    .await;
    assert_eq!(replay_status, StatusCode::OK);
    assert_eq!(replay["receipt"]["replayed"], true);
    assert_eq!(
        replay["receipt"]["recordRef"],
        account_response["receipt"]["recordRef"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ops_account WHERE source_system='linggan_native'",
        )
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    let mut changed_account_body = account_body.clone();
    changed_account_body["accountName"] = Value::String("篡改后的载荷".to_owned());
    let (_, conflict) = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        Some(&owner_cookie),
        changed_account_body,
    )
    .await;
    assert_eq!(conflict["code"], "ops_native_request_conflict");

    let denied = post(
        app.clone(),
        "/api/local/ops/native/publications",
        Some(&other_cookie),
        publication_body(Uuid::new_v4(), account_ref),
    )
    .await;
    assert_eq!(denied.0, StatusCode::FORBIDDEN);
    assert_eq!(denied.1["receipt"]["outcome"], "rejected");
    let publication_request = Uuid::new_v4();
    let (status, publication) = post(
        app.clone(),
        "/api/local/ops/native/publications",
        Some(&owner_cookie),
        publication_body(publication_request, account_ref),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let publication_ref = uuid(&publication["receipt"]["recordRef"]);
    let date_precision: (Option<String>, String) = sqlx::query_as(
        "SELECT published_at::text,native_published_precision FROM linggan_ops_publication WHERE publication_ref=$1",
    )
    .bind(publication_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(date_precision, (None, "day".to_owned()));
    let native_title_adoption: (String, String, Option<String>, Uuid) = sqlx::query_as(
        "SELECT source_kind,disposition,value_text,source_request_ref \
           FROM linggan_ops_publication_field_adoption \
          WHERE publication_ref=$1 AND field_name='title'",
    )
    .bind(publication_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        native_title_adoption,
        (
            "native_registration".to_owned(),
            "adopted".to_owned(),
            Some("只知道日期的发布".to_owned()),
            publication_request,
        ),
        "an explicitly entered native title is a source-bound current field, not a list fallback",
    );

    let mut invalid_publication = publication_body(Uuid::new_v4(), account_ref);
    invalid_publication["publishedOn"] = Value::String("2026-02-30".to_owned());
    let invalid_publication = post(
        app.clone(),
        "/api/local/ops/native/publications",
        Some(&owner_cookie),
        invalid_publication,
    )
    .await;
    assert_eq!(invalid_publication.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid_publication.1["receipt"]["code"],
        "ops_native_publication_date_invalid"
    );

    let view_request = Uuid::new_v4();
    let (status, view) = post(
        app.clone(),
        &format!("/api/local/ops/native/publications/{publication_ref}/view-reports"),
        Some(&owner_cookie),
        json!({ "requestId": view_request, "reportKind": "value", "viewCount": 0, "asOf": "2026-10-23T08:00:00+08:00" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(view["receipt"]["outcome"], "accepted");
    let invalid_view = post(
        app.clone(),
        &format!("/api/local/ops/native/publications/{publication_ref}/view-reports"),
        Some(&owner_cookie),
        json!({ "requestId": Uuid::new_v4(), "reportKind": "value", "viewCount": 1, "asOf": "2026-10-23T08:00:00+16:00" }),
    )
    .await;
    assert_eq!(invalid_view.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid_view.1["receipt"]["code"],
        "ops_native_as_of_invalid"
    );
    let invalid_zero_year = post(
        app.clone(),
        &format!("/api/local/ops/native/publications/{publication_ref}/view-reports"),
        Some(&owner_cookie),
        json!({ "requestId": Uuid::new_v4(), "reportKind": "value", "viewCount": 1, "asOf": "0000-10-23T08:00:00Z" }),
    )
    .await;
    assert_eq!(invalid_zero_year.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid_zero_year.1["receipt"]["code"],
        "ops_native_as_of_invalid"
    );
    let stored_view: (i64, Uuid) = sqlx::query_as(
        "SELECT view_count,request_ref FROM linggan_ops_view_report WHERE publication_ref=$1",
    )
    .bind(publication_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(stored_view, (0, view_request));

    let daily_request = Uuid::new_v4();
    let daily_body = json!({
        "requestId": daily_request,
        "businessDate": "2026-10-20",
        "linksOpenedCount": null,
        "followersGained": null,
        "expectedVersion": null
    });
    let (status, daily) = post(
        app.clone(),
        "/api/local/ops/native/daily-reports",
        Some(&owner_cookie),
        daily_body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let daily_ref = uuid(&daily["receipt"]["recordRef"]);
    let (replay_status, replay) = post(
        app.clone(),
        "/api/local/ops/native/daily-reports",
        Some(&owner_cookie),
        daily_body,
    )
    .await;
    assert_eq!(replay_status, StatusCode::OK);
    assert_eq!(replay["receipt"]["replayed"], true);
    let daily = read_ops_daily(&database, OpsBusiness::All, Some("本人运营"), None)
        .await
        .unwrap();
    assert!(daily.items.iter().any(|item| {
        item.r#ref == daily_ref
            && item.source == "native"
            && item.native_version == Some(1)
            && item.links_opened.is_none()
            && item.followers_gained.is_none()
            && item.month_followers_gained.is_none()
            && item.execution == "not_filled"
    }));
    let (status, updated) = post(
        app.clone(),
        "/api/local/ops/native/daily-reports",
        Some(&owner_cookie),
        json!({ "requestId": Uuid::new_v4(), "businessDate": "2026-10-20", "linksOpenedCount": 0, "followersGained": 0, "expectedVersion": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["receipt"]["resultingVersion"], 2);
    let stale = post(
        app,
        "/api/local/ops/native/daily-reports",
        Some(&owner_cookie),
        json!({ "requestId": Uuid::new_v4(), "businessDate": "2026-10-20", "linksOpenedCount": 1, "followersGained": 1, "expectedVersion": 1 }),
    )
    .await;
    assert_eq!(stale.0, StatusCode::CONFLICT);
    assert_eq!(stale.1["receipt"]["outcome"], "stale");
    let daily_row: (i64, Option<i32>, Option<i32>) = sqlx::query_as(
        "SELECT native_version,links_opened_count,followers_gained FROM linggan_ops_daily_report WHERE daily_ref=$1",
    )
    .bind(daily_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(daily_row, (2, Some(0), Some(0)));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ops_daily_report_revision WHERE daily_ref=$1",
        )
        .bind(daily_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        2,
        "the stale write only adds its immutable receipt, not a revision"
    );
    let accounts = read_ops_accounts(&database, OpsBusiness::All, None)
        .await
        .unwrap();
    assert!(
        accounts
            .items
            .iter()
            .any(|item| item.r#ref == account_ref && item.source == "native")
    );
    assert!(
        read_ops_businesses(&database)
            .await
            .unwrap()
            .iter()
            .any(|business| business.domain_ref == domain && business.account_count == 1)
    );
    let publications = read_ops_publications(
        &database,
        OpsBusiness::All,
        OpsPublicationFilter {
            page: 1,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let imported_item = publications
        .items
        .iter()
        .find(|item| item.source == "imported")
        .expect("the first page retains an imported publication");
    let imported_current: (String, Option<String>) = sqlx::query_as(
        "SELECT disposition,value_text FROM linggan_ops_publication_field_adoption \
         WHERE publication_ref=$1 AND field_name='title' \
         ORDER BY observed_at::timestamptz DESC,created_at DESC,adoption_ref DESC LIMIT 1",
    )
    .bind(imported_item.r#ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(
        imported_item.title.as_deref()
            == (imported_current.0 == "adopted").then_some(imported_current.1.as_deref()).flatten(),
        "the list reads the imported baseline from field adoption, without falling back to the registration row",
    );
    assert!(publications.items.iter().any(|item| {
        item.r#ref == publication_ref
            && item.business_date == "2026-10-20"
            && item.source == "native"
            && item.publisher.as_deref() == Some("本人运营")
            && item.title.as_deref() == Some("只知道日期的发布")
    }));
    let overview = read_ops_overview(
        &database,
        OpsBusiness::All,
        &OpsOverviewQuery {
            slice: OverviewSlice::Week,
            today: Some("2026-10-23".to_owned()),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        overview.window.complete_through.as_deref(),
        Some("2026-10-11")
    );
    assert_eq!(
        (
            overview.native_facts.observed,
            overview.native_facts.latest_business_date.as_deref(),
            overview.native_facts.published,
            overview.native_facts.links_opened,
            overview.native_facts.followers_gained,
            overview.native_facts.hits,
        ),
        (true, Some("2026-10-20"), 1, Some(0), Some(0), 0),
        "native current facts must stay visible without claiming historical coverage",
    );
    let trend =
        read_ops_overview_trend(&database, OpsBusiness::All, Some("day"), Some("2026-10-23"))
            .await
            .unwrap();
    let native_day = trend
        .points
        .iter()
        .find(|point| point.start == "2026-10-20")
        .unwrap();
    assert_eq!(
        trend.native_latest_business_date.as_deref(),
        Some("2026-10-20")
    );
    assert_eq!(native_day.coverage, "not_migrated");
    assert!(native_day.native_facts);
    assert_eq!(
        (native_day.followers_gained, native_day.hits),
        (Some(0), Some(0))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn native_account_responsibility_is_versioned_idempotent_and_group_bounded() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_resp_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let admin = member(
        database.pool(),
        "responsibility-admin@example.test",
        "管理员",
        "admin",
    )
    .await;
    let lead = member(
        database.pool(),
        "responsibility-lead@example.test",
        "组长",
        "lead",
    )
    .await;
    let owner = member(
        database.pool(),
        "responsibility-owner@example.test",
        "原负责人",
        "operator",
    )
    .await;
    let successor = member(
        database.pool(),
        "responsibility-successor@example.test",
        "接任运营",
        "operator",
    )
    .await;
    let other_group_member = member(
        database.pool(),
        "responsibility-other@example.test",
        "异组运营",
        "operator",
    )
    .await;
    let disabled = member(
        database.pool(),
        "responsibility-disabled@example.test",
        "停用运营",
        "operator",
    )
    .await;
    let domain = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain (domain_ref,name) VALUES($1,'责任交接合成领域')")
        .bind(domain)
        .execute(database.pool())
        .await
        .unwrap();
    let primary_group = Uuid::new_v4();
    let other_group = Uuid::new_v4();
    for (group_ref, display_name) in [(primary_group, "交接主组"), (other_group, "交接异组")]
    {
        sqlx::query(
            "INSERT INTO linggan_ops_group (group_ref,display_name,state,created_by_member_ref) \
             VALUES($1,$2,'active',$3)",
        )
        .bind(group_ref)
        .bind(display_name)
        .bind(admin)
        .execute(database.pool())
        .await
        .unwrap();
    }
    for (member_ref, group_ref) in [
        (lead, primary_group),
        (owner, primary_group),
        (successor, primary_group),
        (other_group_member, other_group),
    ] {
        sqlx::query(
            "INSERT INTO linggan_ops_group_membership_event \
             (event_ref,group_ref,member_ref,action,acted_by_member_ref,reason,request_ref) \
             VALUES($1,$2,$3,'joined',$4,'合成交接分组',$5)",
        )
        .bind(Uuid::new_v4())
        .bind(group_ref)
        .bind(member_ref)
        .bind(admin)
        .bind(Uuid::new_v4())
        .execute(database.pool())
        .await
        .unwrap();
    }
    let app = app_with_database(database.clone());
    let admin_cookie = session(database.pool(), admin).await;
    let lead_cookie = session(database.pool(), lead).await;
    let owner_cookie = session(database.pool(), owner).await;
    let (status, created) = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        Some(&owner_cookie),
        json!({
            "requestId": Uuid::new_v4(), "domainRef": domain, "platform": "xhs",
            "accountName": "待交接原生账号", "profileUrl": "https://example.test/xhs/responsibility",
            "platformUserId": "0123456789abcdef01234567"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let account_ref = uuid(&created["receipt"]["recordRef"]);
    let transfer_request = Uuid::new_v4();
    let transfer_body = json!({
        "requestId": transfer_request, "nextMemberRef": successor, "expectedVersion": 1,
        "reason": "本周排班交接"
    });
    let (status, transferred) = post(
        app.clone(),
        &format!("/api/local/ops/native/accounts/{account_ref}/responsibility"),
        Some(&lead_cookie),
        transfer_body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(transferred["receipt"]["outcome"], "accepted");
    assert_eq!(transferred["receipt"]["resultingVersion"], 2);
    let (status, replay) = post(
        app.clone(),
        &format!("/api/local/ops/native/accounts/{account_ref}/responsibility"),
        Some(&lead_cookie),
        transfer_body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["receipt"]["replayed"], true);
    let current: (Option<Uuid>, i64, String, i64) = sqlx::query_as(
        "SELECT current_responsible_member_ref,responsibility_version,source_system,native_version \
         FROM linggan_ops_account WHERE account_ref=$1",
    )
    .bind(account_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        current,
        (Some(successor), 2, "linggan_native".to_owned(), 1)
    );
    let (status, account_page) = get(
        app.clone(),
        &format!("/api/local/ops/accounts?domain={domain}"),
        Some(&lead_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rendered_account = account_page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["ref"] == account_ref.to_string())
        .unwrap();
    assert_eq!(
        rendered_account["currentResponsibleMemberRef"],
        successor.to_string()
    );
    assert_eq!(rendered_account["currentResponsibleName"], "接任运营");
    assert_eq!(rendered_account["responsibilityVersion"], 2);
    let (status, roster) = get(app.clone(), "/api/local/ops/members", Some(&lead_cookie)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(roster.as_array().unwrap().iter().any(|item| {
        item["memberRef"] == successor.to_string()
            && item["displayName"] == "接任运营"
            && item["role"] == "operator"
    }));
    let change_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ops_account_responsibility_change WHERE account_ref=$1",
    )
    .bind(account_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        change_count, 1,
        "an identical request cannot write a second handover"
    );
    let (status, cross_group) = post(
        app.clone(),
        &format!("/api/local/ops/native/accounts/{account_ref}/responsibility"),
        Some(&lead_cookie),
        json!({
            "requestId": Uuid::new_v4(), "nextMemberRef": other_group_member, "expectedVersion": 2,
            "reason": "错误跨组交接"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(cross_group["receipt"]["code"], "ops_native_forbidden");
    let (status, unassigned) = post(
        app.clone(),
        &format!("/api/local/ops/native/accounts/{account_ref}/responsibility"),
        Some(&admin_cookie),
        json!({
            "requestId": Uuid::new_v4(), "nextMemberRef": null, "expectedVersion": 2,
            "reason": "离职后待管理员接管"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unassigned["receipt"]["resultingVersion"], 3);
    sqlx::query("UPDATE linggan_ops_member SET state='disabled',disabled_at=scope_001_now() WHERE member_ref=$1")
        .bind(disabled)
        .execute(database.pool())
        .await
        .unwrap();
    let (status, inactive_target) = post(
        app.clone(),
        &format!("/api/local/ops/native/accounts/{account_ref}/responsibility"),
        Some(&admin_cookie),
        json!({
            "requestId": Uuid::new_v4(), "nextMemberRef": disabled, "expectedVersion": 3,
            "reason": "不能把账号交给已停用成员"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        inactive_target["receipt"]["code"],
        "ops_native_responsibility_target_inactive"
    );
    let (status, stale) = post(
        app,
        &format!("/api/local/ops/native/accounts/{account_ref}/responsibility"),
        Some(&admin_cookie),
        json!({
            "requestId": Uuid::new_v4(), "nextMemberRef": successor, "expectedVersion": 2,
            "reason": "过期页面不能覆盖新责任"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        stale["receipt"]["code"],
        "ops_native_responsibility_version_stale"
    );
    let change_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_ops_account_responsibility_change WHERE account_ref=$1",
    )
    .bind(account_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        change_count, 2,
        "rejected and stale commands never become handover history"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn publication_sync_requires_explicit_scope_and_then_uses_exact_collection_work() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_sync_http_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let owner = member(
        database.pool(),
        "sync-owner@example.test",
        "同步本人",
        "operator",
    )
    .await;
    let other = member(
        database.pool(),
        "sync-other@example.test",
        "同步他人",
        "operator",
    )
    .await;
    let domain = Uuid::new_v4();
    sqlx::query("INSERT INTO observation_domain (domain_ref,name) VALUES($1,'同步合成领域')")
        .bind(domain)
        .execute(database.pool())
        .await
        .unwrap();
    let app = app_with_database(database.clone());
    let owner_cookie = session(database.pool(), owner).await;
    let other_cookie = session(database.pool(), other).await;
    let (status, account) = post(
        app.clone(),
        "/api/local/ops/native/accounts",
        Some(&owner_cookie),
        json!({
            "requestId": Uuid::new_v4(), "domainRef": domain, "platform": "xhs",
            "accountName": "同步小红书账号", "profileUrl": "https://example.test/xhs/sync",
            "platformUserId": "0123456789abcdef01234567"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let account_ref = uuid(&account["receipt"]["recordRef"]);
    let note = "6650a1b2c3d4e5f60718293a";
    let (status, publication) = post(
        app.clone(),
        "/api/local/ops/native/publications",
        Some(&owner_cookie),
        json!({
            "requestId": Uuid::new_v4(), "accountRef": account_ref,
            "publicationUrl": format!("https://www.xiaohongshu.com/explore/{note}"),
            "publishedOn": "2026-10-20"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let publication_ref = uuid(&publication["receipt"]["recordRef"]);
    let request_ref = Uuid::new_v4();
    let body = json!({ "idempotencyKey": request_ref, "publicationRefs": [publication_ref] });
    assert_eq!(
        hostile_post(
            app.clone(),
            "/api/local/ops/publication-syncs",
            body.clone()
        )
        .await,
        StatusCode::FORBIDDEN
    );
    let (status, first) = post(
        app.clone(),
        "/api/local/ops/publication-syncs",
        Some(&owner_cookie),
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(first["replayed"], false);
    assert_eq!(first["items"][0]["decision"], "blocked");
    assert_eq!(
        first["items"][0]["code"],
        "ops_sync_content_identity_unresolved"
    );
    assert!(first["items"][0]["acquisitionRequestRef"].is_null());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_acquisition_request")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_work_order")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        0
    );
    let signed_locator = format!(
        "https://www.xiaohongshu.com/explore/{note}?xsec_token=SYNTHETIC_SIGNED_INPUT&xsec_source=pc_user"
    );
    let (status, locator) = post(
        app.clone(),
        &format!("/api/local/ops/native/publications/{publication_ref}/execution-locators"),
        Some(&owner_cookie),
        json!({ "requestId": Uuid::new_v4(), "signedLocator": signed_locator }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(locator["receipt"]["outcome"], "accepted");
    assert!(
        locator["receipt"].get("signedLocator").is_none(),
        "a signed execution input is never echoed in a command receipt"
    );
    let frozen_source: (String, String, i64) = sqlx::query_as(
        "SELECT expected_content_id,expected_author_id,publication_version \
           FROM linggan_ops_publication_execution_locator WHERE publication_ref=$1",
    )
    .bind(publication_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        frozen_source,
        (note.to_owned(), "0123456789abcdef01234567".to_owned(), 1)
    );
    let (status, sourced_sync) = post(
        app.clone(),
        "/api/local/ops/publication-syncs",
        Some(&owner_cookie),
        json!({ "idempotencyKey": Uuid::new_v4(), "publicationRefs": [publication_ref] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(sourced_sync["items"][0]["decision"], "blocked");
    assert_eq!(
        sourced_sync["items"][0]["code"], "authorization_missing",
        "a frozen locator alone does not authorize platform work"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_acquisition_request")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        1,
        "a refused authorization decision remains auditable"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM collection_work_order")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        0,
        "no authorization means no queue instruction"
    );
    let authorization_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
         (authorization_ref,platform,target_kind,lane,purpose,max_works_per_target, \
          allowed_task_templates,allowed_dispatch_lanes,max_work_units,granted_by,expires_at) \
         VALUES($1,'xhs','creator','deep_archive','ops publication sync',1, \
                ARRAY['ops_publication_sync'],ARRAY['batch'],1,'person', \
                scope_001_now()+interval '1 day')",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let (status, accepted_sync) = post(
        app.clone(),
        "/api/local/ops/publication-syncs",
        Some(&owner_cookie),
        json!({ "idempotencyKey": Uuid::new_v4(), "publicationRefs": [publication_ref] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(accepted_sync["items"][0]["decision"], "accepted");
    assert_eq!(accepted_sync["items"][0]["code"], "within_authorization");
    let accepted_request_ref = uuid(&accepted_sync["requestRef"]);
    let collection_request_ref = uuid(&accepted_sync["items"][0]["acquisitionRequestRef"]);
    let work_order_ref = uuid(&accepted_sync["items"][0]["workOrderRef"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM collection_work_order_ops_publication_target \
             WHERE work_order_ref=$1 AND publication_ref=$2 AND publication_version=1 \
               AND content_external_id=$3 AND author_external_id=$4"
        )
        .bind(work_order_ref)
        .bind(publication_ref)
        .bind(note)
        .bind("0123456789abcdef01234567")
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM collection_work_order_material_target WHERE work_order_ref=$1"
        )
        .bind(work_order_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0,
        "the admitted post is not fabricated as Material"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM collection_acquisition_request WHERE request_ref=$1 AND purpose='ops_publication_sync'"
        )
        .bind(collection_request_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    let (status, replay) = post(
        app.clone(),
        "/api/local/ops/publication-syncs",
        Some(&owner_cookie),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
    let (status, conflict) = post(
        app.clone(),
        "/api/local/ops/publication-syncs",
        Some(&other_cookie),
        json!({ "idempotencyKey": request_ref, "publicationRefs": [publication_ref] }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(conflict["code"], "ops_sync_request_conflict");

    // This stays entirely inside the isolated proof database, but exercises the actual shared
    // queue → live claim → Attempt → Package → Receipt path rather than inserting a package.
    let installation_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO execution_station(station_ref,display_name,daily_work_quota) \
         VALUES($1,'运营同步隔离工位',10000)",
    )
    .bind(station_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO plugin_installation \
         (installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version,capabilities) \
         VALUES($1,$2,$3,'person',scope_001_now(),'0.8.61', \
                '[\"content_detail\",\"xhs.pageAccess\"]'::jsonb)",
    )
    .bind(installation_ref)
    .bind(installation_ref.to_string())
    .bind(station_ref)
    .execute(database.pool())
    .await
    .unwrap();
    set_station_accepting(&database, station_ref, true, "person")
        .await
        .unwrap();
    let grant = rotate_installation_credential(&database, installation_ref)
        .await
        .unwrap();
    let credential = grant.raw_credential.expose_once().to_owned();
    activate_installation_credential(
        &database,
        installation_ref,
        grant.credential_ref,
        &credential,
    )
    .await
    .unwrap();
    let observed_account = report_account_eligibility(
        &database,
        installation_ref,
        &credential,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "0123456789abcdef01234567",
        },
        Some(b"ops-publication-sync-isolated-proof"),
    )
    .await
    .unwrap();
    bind_observation_account(
        &database,
        observed_account.account_ref.expect("authenticated account"),
        installation_ref,
        "person",
    )
    .await
    .unwrap();
    let dispatch = decide_dispatch(&database, &installation_ref.to_string(), &credential)
        .await
        .unwrap();
    let DispatchDecision::Dispatch { task_id, task_spec, .. } = dispatch else {
        panic!("the exact Ops work must dispatch through the shared live-claim path: {dispatch:?}");
    };
    assert_eq!(task_spec["source"], "scheduled");
    assert_eq!(task_spec["capabilitiesRequested"][0], "content_detail");
    assert_eq!(task_spec["target"]["contentExternalId"], note);
    let attempt_id = Uuid::new_v4();
    let attempt = parse_producer_attempt(
        &json!({
            "contractVersion":"linggan.producer.attempt.v1", "taskId":task_id,
            "attemptId":attempt_id, "producerInstanceId":installation_ref
        })
        .to_string(),
    )
    .unwrap();
    start_producer_attempt(&database, &attempt).await.unwrap();
    let package_ref = Uuid::new_v4();
    let submission = parse_producer_submission(
        &json!({
            "contractVersion":"linggan.producer.capture-package.v1", "taskId":task_id,
            "attemptId":attempt_id, "producerInstanceId":installation_ref,
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1", "packageRef":package_ref,
                "packageKind":"content_detail", "platform":"xhs",
                "observedAt":"2026-10-23T08:00:00Z", "capturedAt":"2026-10-23T08:00:01Z",
                "coverage":{"target":{"basis":"known_set","contentExternalId":note},"layers":[{
                    "capability":"content_detail","observed":1,"attempted":1,"acquired":1,
                    "verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"fixture_complete"
                }]},
                "records":[{"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":note},"payload":{
                    "title":"隔离运营回填标题", "bodyText":"隔离运营回填正文", "authorId":"0123456789abcdef01234567",
                    "likeCount":71, "collectCount":9, "commentCount":6
                }}]
            }
        })
        .to_string(),
    )
    .unwrap();
    let outcome = submit_producer_package(&database, &submission).await.unwrap();
    let linggan_evidence::RuntimeSubmissionOutcome::Acknowledged { receipt_ref, execution_effect, .. } = outcome else {
        panic!("a first valid package must receive an acknowledgement");
    };
    assert_eq!(execution_effect, "COMPLETED_LIVE_STEP");
    let projection: (Uuid, Uuid, Uuid, Uuid, i64, String) = sqlx::query_as(
        "SELECT publication_ref,content_public_ref,package_ref,receipt_ref,publication_version,observed_at \
           FROM linggan_ops_publication_material_projection WHERE receipt_ref=$1",
    )
    .bind(receipt_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(projection.0, publication_ref);
    assert_eq!(projection.2, package_ref);
    assert_eq!(projection.3, receipt_ref);
    assert_eq!(projection.4, 1);
    assert_eq!(projection.5, "2026-10-23T08:00:00Z");
    let adopted: Vec<(String, String, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT field_name,disposition,value_text,value_count \
           FROM linggan_ops_publication_field_adoption \
          WHERE projection_ref=(SELECT projection_ref FROM linggan_ops_publication_material_projection WHERE receipt_ref=$1) \
          ORDER BY field_name",
    )
    .bind(receipt_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        adopted,
        vec![
            ("body".into(), "adopted".into(), Some("隔离运营回填正文".into()), None),
            ("collect_count".into(), "adopted".into(), None, Some(9)),
            ("comment_count".into(), "adopted".into(), None, Some(6)),
            ("like_count".into(), "adopted".into(), None, Some(71)),
            ("published_at".into(), "not_provided".into(), None, None),
            ("title".into(), "adopted".into(), Some("隔离运营回填标题".into()), None),
        ],
        "the Receipt leaves a field-level provenance trail; missing evidence is not a zero",
    );
    let untouched: (Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<f64>, Option<String>, Uuid, Uuid) = sqlx::query_as(
        "SELECT title,body,like_count,collect_count,comment_count,cover_click_rate,publisher_name,account_ref, \
                native_publisher_member_ref \
           FROM linggan_ops_publication WHERE publication_ref=$1",
    )
    .bind(publication_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(untouched, (None, None, None, None, None, None, None, account_ref, owner));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_material_domain_usage WHERE content_public_ref=$1",
        )
        .bind(projection.1)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0,
        "Ops projection adds an explicit own-use link without rewriting domain usage"
    );
    let (status, receipt) = get(
        app,
        &format!("/api/local/ops/publication-syncs/{accepted_request_ref}"),
        Some(&owner_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["items"][0]["execution"]["state"], "material_projected");
    assert_eq!(receipt["items"][0]["execution"]["taskId"], task_id.to_string());
    assert_eq!(receipt["items"][0]["execution"]["attemptId"], attempt_id.to_string());
    assert_eq!(receipt["items"][0]["execution"]["packageRef"], package_ref.to_string());
    assert_eq!(receipt["items"][0]["execution"]["receiptRef"], receipt_ref.to_string());
    assert_eq!(receipt["items"][0]["execution"]["materialRef"], projection.1.to_string());
    let persisted = read_ops_publications(
        &database,
        OpsBusiness::Domain(domain),
        OpsPublicationFilter {
            page: 1,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let row = persisted
        .items
        .iter()
        .find(|item| item.r#ref == publication_ref)
        .expect("the Operations list keeps the registered publication");
    assert_eq!(row.sync.request_ref, Some(accepted_request_ref));
    assert_eq!(row.sync.decision.as_deref(), Some("accepted"));
    assert_eq!(row.sync.state, "material_projected");
    assert_eq!(row.title.as_deref(), Some("隔离运营回填标题"));
    assert_eq!(row.body.as_deref(), Some("隔离运营回填正文"));
    assert_eq!((row.likes, row.collects, row.comments), (Some(71), Some(9), Some(6)));
}

fn publication_body(request_id: Uuid, account_ref: Uuid) -> Value {
    json!({
        "requestId": request_id,
        "accountRef": account_ref,
        "publicationUrl": "https://example.test/xhs/post/date-only",
        "publishedOn": "2026-10-20",
        "title": "只知道日期的发布"
    })
}

async fn member(pool: &sqlx::PgPool, email: &str, display_name: &str, role: &str) -> Uuid {
    let member_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_member \
         (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) \
         VALUES($1,$2,$3,$4,'active','synthetic-only',scope_001_now())",
    )
    .bind(member_ref)
    .bind(email)
    .bind(display_name)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    member_ref
}

async fn session(pool: &sqlx::PgPool, member_ref: Uuid) -> String {
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    sqlx::query(
        "INSERT INTO linggan_ops_session (session_ref,member_ref,token_sha256,expires_at) \
         VALUES($1,$2,$3,scope_001_now()+interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(member_ref)
    .bind(digest.as_slice())
    .execute(pool)
    .await
    .unwrap();
    format!("linggan_ops_session={token}")
}

async fn post(app: Router, path: &str, cookie: Option<&str>, body: Value) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::HOST, "127.0.0.1:3000")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    if let Some(cookie) = cookie {
        request
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
    }
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65_536).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn get(app: Router, path: &str, cookie: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method("GET")
        .uri(path)
        .header(header::HOST, "127.0.0.1:3000")
        .body(Body::empty())
        .unwrap();
    if let Some(cookie) = cookie {
        request
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
    }
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65_536).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn hostile_post(app: Router, path: &str, body: Value) -> StatusCode {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::HOST, "attacker.test")
        .header(header::ORIGIN, "https://attacker.test")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    app.oneshot(request).await.unwrap().status()
}

fn uuid(value: &Value) -> Uuid {
    Uuid::parse_str(value.as_str().unwrap()).unwrap()
}
