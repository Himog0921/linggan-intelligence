//! OPS-NATIVE-HTTP-001 isolated Axum/PostgreSQL proof.  All members and facts are synthetic.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_intelligence::ops_ledger::{
    apply_ops_import, preview_ops_import,
    read::{
        OpsBusiness, OpsPublicationFilter, read_ops_accounts, read_ops_businesses, read_ops_daily,
        read_ops_publications,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../../crates/intelligence/tests/support/ops_export_fixture.rs"]
mod ops_export;

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
    assert!(publications.items.iter().any(|item| {
        item.r#ref == publication_ref
            && item.business_date == "2026-10-20"
            && item.source == "native"
            && item.publisher.as_deref() == Some("本人运营")
    }));
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
