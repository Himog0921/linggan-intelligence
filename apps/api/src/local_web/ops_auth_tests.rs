//! OPS-AUTH-001 isolated PostgreSQL + Axum proof.  Real team credentials never enter this file.
use super::*;
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use axum::{
    body::{Body, to_bytes},
    http::{HeaderValue, Request, StatusCode, header},
};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; set LOCAL_001_PROOF_DATABASE_URL"]
async fn public_team_access_requires_live_session_and_preserves_inner_origin_guards() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("public_access_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let admin = active_member(database.pool(), "admin-public@example.test", "admin").await;
    let operator = active_member(database.pool(), "operator-public@example.test", "operator").await;
    // This synthetic Better Auth credential exercises the same imported verifier path as the
    // production handover, through real PostgreSQL and the public login route.
    sqlx::query("UPDATE linggan_ops_member SET password_hash=$2 WHERE member_ref=$1")
        .bind(operator)
        .bind("workbench-scrypt$00112233445566778899aabbccddeeff:503d058ffa344ded3ade56ae37c931eeefdf127685942056bd9f50687f4fc3ced01bc958abc4957631dbc6774e6ff941d25010d5b60c8ae0b261cb36ada8b5bb")
        .execute(database.pool())
        .await
        .unwrap();
    let app = router_with_public_access(
        LocalWebState {
            database: LocalDatabaseState::Ready(Arc::new(database.clone())),
            active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
            account_digest_key: None,
        },
        public_access::PublicAccess::parse("https://intelligence.example.com").unwrap(),
    );
    let public_request = |method: &str, path: &str, cookie: Option<&str>, body: &'static str| {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "intelligence.example.com")
            .header("x-forwarded-proto", "https")
            .header(header::ORIGIN, "https://intelligence.example.com")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie.split(';').next().unwrap());
        }
        builder.body(Body::from(body)).unwrap()
    };

    let anonymous = public_request("GET", "/api/local/auth/session", None, "");
    assert_eq!(
        app.clone().oneshot(anonymous).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let admin_login = public_request(
        "POST",
        "/api/local/auth/login",
        None,
        r#"{"email":"admin-public@example.test","password":"synthetic-password-only"}"#,
    );
    let admin_login = app.clone().oneshot(admin_login).await.unwrap();
    assert_eq!(admin_login.status(), StatusCode::OK);
    let admin_cookie = admin_login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(admin_cookie.contains("; Secure"));
    let operator_login = public_request(
        "POST",
        "/api/local/auth/login",
        None,
        r#"{"email":"operator-public@example.test","password":"Password123"}"#,
    );
    let operator_login = app.clone().oneshot(operator_login).await.unwrap();
    assert_eq!(operator_login.status(), StatusCode::OK);
    assert!(operator_login.headers().contains_key(header::SET_COOKIE));
    let operator_cookie = fresh_session(database.pool(), operator).await;
    let operator_session =
        public_request("GET", "/api/local/auth/session", Some(&operator_cookie), "");
    assert_eq!(
        app.clone()
            .oneshot(operator_session)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let operator_ledger =
        public_request("GET", "/api/local/ops/accounts", Some(&operator_cookie), "");
    assert_eq!(
        app.clone().oneshot(operator_ledger).await.unwrap().status(),
        StatusCode::OK
    );
    for (method, path) in [
        ("POST", "/api/local/ops/publication-syncs"),
        (
            "GET",
            "/api/local/ops/publication-syncs/00000000-0000-0000-0000-000000000001",
        ),
    ] {
        let blocked = public_request(method, path, Some(&operator_cookie), "{}");
        assert_eq!(
            app.clone().oneshot(blocked).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    let operator_unsafe = public_request(
        "GET",
        "/api/local/comment-research/preview",
        Some(&operator_cookie),
        "",
    );
    assert_eq!(
        app.clone().oneshot(operator_unsafe).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    let admin_session = public_request("GET", "/api/local/auth/session", Some(&admin_cookie), "");
    assert_eq!(
        app.clone().oneshot(admin_session).await.unwrap().status(),
        StatusCode::OK
    );
    let anonymous_logout = public_request("POST", "/api/local/auth/logout", None, "");
    assert_eq!(
        app.clone()
            .oneshot(anonymous_logout)
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let command = public_request(
        "GET",
        "/api/local/comment-research/preview",
        Some(&admin_cookie),
        "{}",
    );
    assert_eq!(
        app.clone().oneshot(command).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    // The retired endpoint remains absent; the permission test above exercises a live route.
    let retired = public_request(
        "POST",
        "/api/local/comment-study/selection-preview",
        Some(&admin_cookie),
        "{}",
    );
    assert_eq!(
        app.clone().oneshot(retired).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let media = public_request(
        "GET",
        "/api/local/media/invalid/invalid",
        Some(&admin_cookie),
        "",
    );
    assert_ne!(
        app.clone().oneshot(media).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let machine = public_request(
        "POST",
        "/api/local/collection/authorizations",
        Some(&admin_cookie),
        "{}",
    );
    assert_eq!(
        app.clone().oneshot(machine).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let cross_site = Request::builder()
        .method("GET")
        .uri("/api/local/comment-research/preview")
        .header(header::HOST, "intelligence.example.com")
        .header("x-forwarded-proto", "https")
        .header(header::ORIGIN, "https://other.example.com")
        .header(header::COOKIE, admin_cookie.split(';').next().unwrap())
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(cross_site).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE linggan_ops_member SET state='disabled', disabled_at=scope_001_now() WHERE member_ref=$1")
        .bind(admin)
        .execute(database.pool())
        .await
        .unwrap();
    let disabled = public_request("GET", "/api/local/auth/session", Some(&admin_cookie), "");
    assert_eq!(
        app.oneshot(disabled).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn login_session_logout_and_disabled_member_do_not_leak_credentials() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_auth_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let password = "synthetic-password-only";
    let password_hash = Argon2::default()
        .hash_password(
            password.as_bytes(),
            &SaltString::encode_b64(b"isolated-proof-salt").unwrap(),
        )
        .unwrap()
        .to_string();
    let member_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_member \
         (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) \
         VALUES($1,'ops@example.test','合成运营','operator','active',$2,scope_001_now())",
    )
    .bind(member_ref)
    .bind(password_hash)
    .execute(database.pool())
    .await
    .unwrap();

    let app = app_with_database(database.clone());
    let bad = json_request(
        "/api/local/auth/login",
        r#"{"email":"missing@example.test","password":"wrong"}"#,
    );
    let bad = app.clone().oneshot(bad).await.unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);
    let wrong = json_request(
        "/api/local/auth/login",
        r#"{"email":"ops@example.test","password":"wrong"}"#,
    );
    assert_eq!(
        app.clone().oneshot(wrong).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let login = json_request(
        "/api/local/auth/login",
        r#"{"email":" OPS@EXAMPLE.TEST ","password":"synthetic-password-only"}"#,
    );
    let login = app.clone().oneshot(login).await.unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(cookie.contains("HttpOnly; SameSite=Strict; Path=/"));
    assert!(cookie.contains("Max-Age=604800"));
    assert!(!cookie.contains(password));
    let mut https_login = json_request(
        "/api/local/auth/login",
        r#"{"email":"ops@example.test","password":"synthetic-password-only"}"#,
    );
    https_login.headers_mut().insert(
        header::HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static("https"),
    );
    let https_login = app.clone().oneshot(https_login).await.unwrap();
    let https_cookie = https_login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(https_cookie.contains("; Secure"));
    let session = session_request("/api/local/auth/session", &cookie, "GET");
    let session = app.clone().oneshot(session).await.unwrap();
    assert_eq!(session.status(), StatusCode::OK);
    let logout = session_request("/api/local/auth/logout", &cookie, "POST");
    let logout = app.clone().oneshot(logout).await.unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert!(
        logout.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    let mut https_logout = session_request("/api/local/auth/logout", &https_cookie, "POST");
    https_logout.headers_mut().insert(
        header::HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static("https"),
    );
    let https_logout = app.clone().oneshot(https_logout).await.unwrap();
    assert_eq!(https_logout.status(), StatusCode::NO_CONTENT);
    assert!(
        https_logout.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("; Secure")
    );
    let replay = session_request("/api/local/auth/session", &cookie, "GET");
    assert_eq!(
        app.clone().oneshot(replay).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    let disabled = fresh_session(database.pool(), member_ref).await;
    sqlx::query("UPDATE linggan_ops_member SET state='disabled', disabled_at=scope_001_now() WHERE member_ref=$1")
        .bind(member_ref)
        .execute(database.pool())
        .await
        .unwrap();
    let disabled = session_request("/api/local/auth/session", &disabled, "GET");
    assert_eq!(
        app.oneshot(disabled).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn administrator_roster_exposes_lifecycle_and_current_responsibility_without_credentials() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_auth_roster_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let admin = active_member(database.pool(), "admin@example.test", "admin").await;
    let operator = active_member(database.pool(), "operator@example.test", "operator").await;
    let app = app_with_database(database.clone());
    let operator_cookie = stored_session(
        database.pool(),
        operator,
        "roster_operator_session",
        "0 seconds",
        "1 hour",
    )
    .await;
    assert_eq!(
        app.clone()
            .oneshot(session_request(
                "/api/local/auth/members",
                &operator_cookie,
                "GET"
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN,
        "the active roster selector is not a management roster"
    );
    let admin_cookie = stored_session(
        database.pool(),
        admin,
        "roster_admin_session",
        "0 seconds",
        "1 hour",
    )
    .await;
    let (status, roster) = json_response(
        app.oneshot(session_request(
            "/api/local/auth/members",
            &admin_cookie,
            "GET",
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let members = roster["members"].as_array().unwrap();
    assert_eq!(members.len(), 2);
    let operator = members
        .iter()
        .find(|member| member["email"] == "operator@example.test")
        .unwrap();
    assert_eq!(operator["state"], "active");
    assert_eq!(operator["version"], 1);
    assert_eq!(operator["currentAccountCount"], 0);
    assert!(operator.get("passwordHash").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn ops_ledger_routes_require_an_active_team_session_without_claiming_the_creation_workspace()
{
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_auth_bound_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    active_member(database.pool(), "ops@example.test", "operator").await;
    let app = app_with_database(database);

    let protected = [
        ("GET", "/ops"),
        ("GET", "/settings/account"),
        ("GET", "/settings/team"),
        ("GET", "/ops/overview"),
        ("GET", "/ops/publications"),
        ("GET", "/ops/accounts"),
        ("GET", "/ops/daily"),
        ("GET", "/ops/io"),
        ("GET", "/api/local/ops/businesses"),
        ("GET", "/api/local/ops/summary"),
        ("GET", "/api/local/ops/publications"),
        ("GET", "/api/local/ops/accounts"),
        ("GET", "/api/local/ops/daily"),
        ("GET", "/api/local/ops/export"),
        ("GET", "/api/local/ops/overview"),
        ("GET", "/api/local/ops/overview/trend"),
        ("GET", "/api/local/ops/targets"),
        ("POST", "/api/local/ops/targets"),
        ("GET", "/ops/covers/00000000-0000-4000-8000-000000000001"),
    ];
    for (method, path) in protected {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::HOST, "127.0.0.1:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path}"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        let foreign = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::HOST, "attacker.example:3000")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::FORBIDDEN, "{method} {path}");
    }

    let creation = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/ops/creation")
                .header(header::HOST, "127.0.0.1:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(matches!(
        creation.status(),
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));
    let creation_api = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/ops/api/creation/works")
                .header(header::HOST, "127.0.0.1:3000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        creation_api.status(),
        StatusCode::UNAUTHORIZED,
        "the creation API must retain its existing boundary"
    );
    let producer = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/local/producer/manual-tasks")
                .header(header::HOST, "127.0.0.1:3000")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        producer.status(),
        StatusCode::UNAUTHORIZED,
        "the machine producer contract must not inherit browser-session auth"
    );

    let cookie = login_cookie(app.clone(), "ops@example.test").await;
    let redirect = app
        .clone()
        .oneshot(session_request("/ops", &cookie, "GET"))
        .await
        .unwrap();
    assert_eq!(redirect.status(), StatusCode::SEE_OTHER);
    assert_eq!(redirect.headers()[header::LOCATION], "/ops/publications");
    let page = app
        .clone()
        .oneshot(session_request("/ops/overview", &cookie, "GET"))
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::FORBIDDEN);
    for (method, path) in protected {
        let response = app
            .clone()
            .oneshot(session_request(path, &cookie, method))
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} must reach its endpoint after session validation"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn team_group_and_responsibility_foundation_preserves_history() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_team_model_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let admin = active_member(database.pool(), "admin@example.test", "admin").await;
    let member = active_member(database.pool(), "member@example.test", "operator").await;
    let group = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_group (group_ref,display_name,state,created_by_member_ref) \
         VALUES($1,'合成运营组','active',$2)",
    )
    .bind(group)
    .bind(admin)
    .execute(database.pool())
    .await
    .unwrap();
    let disabled_group = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_group \
         (group_ref,display_name,state,created_by_member_ref,disabled_at) \
         VALUES($1,'合成已停用运营组','disabled',$2,scope_001_now())",
    )
    .bind(disabled_group)
    .bind(admin)
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        sqlx::query("DELETE FROM linggan_ops_group WHERE group_ref=$1")
            .bind(disabled_group)
            .execute(database.pool())
            .await
            .is_err(),
        "a disabled group must remain historical authorization context"
    );
    let event = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_group_membership_event \
         (event_ref,group_ref,member_ref,action,acted_by_member_ref,reason,request_ref) \
         VALUES($1,$2,$3,'joined',$4,'合成测试加入',$5)",
    )
    .bind(event)
    .bind(group)
    .bind(member)
    .bind(admin)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        sqlx::query(
            "UPDATE linggan_ops_group_membership_event SET reason='篡改' WHERE event_ref=$1"
        )
        .bind(event)
        .execute(database.pool())
        .await
        .is_err(),
        "membership history must be append-only"
    );
    assert!(
        sqlx::query("DELETE FROM linggan_ops_group_membership_event WHERE event_ref=$1")
            .bind(event)
            .execute(database.pool())
            .await
            .is_err(),
        "membership history must not be erasable"
    );
    let native_columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns \
         WHERE table_schema=current_schema() AND table_name='linggan_ops_account' \
           AND column_name IN ('current_responsible_member_ref','responsibility_version')",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(native_columns, 2);
    let domain = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &database,
        "合成团队责任领域",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let account = Uuid::new_v4();
    let historical_batch = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_import_batch \
         (batch_ref,preview_hash,rule_version,source_system,source_workspace_id,exported_at,domain_ref, \
          accounts_sha256,publications_sha256,daily_sha256,account_rows,publication_rows,daily_rows, \
          accounts_inserted,accounts_updated,accounts_unchanged,publications_inserted,publications_updated, \
          publications_unchanged,publications_marked_missing,daily_inserted,daily_updated,daily_unchanged, \
          daily_marked_missing,view_reports_appended) \
         VALUES($1,repeat('a',64),'ops-import.v1','content_workbench','synthetic-team-proof', \
                scope_001_now(),$2,repeat('b',64),repeat('c',64),repeat('d',64), \
                0,0,0,0,0,0,0,0,0,0,0,0,0,0,0)",
    )
    .bind(historical_batch)
    .bind(domain)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ops_account \
         (account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name, \
          responsible_user_source_id,profile_link_state,first_batch_ref,last_changed_batch_ref) \
         VALUES($1,'content_workbench','synthetic-responsibility-account',$2, \
                'absent_from_export','xhs','合成责任账号','legacy-owner','missing',$3,$3)",
    )
    .bind(account)
    .bind(domain)
    .bind(historical_batch)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_ops_account \
         SET current_responsible_member_ref=$2,responsibility_version=2 WHERE account_ref=$1",
    )
    .bind(account)
    .bind(member)
    .execute(database.pool())
    .await
    .unwrap();
    let change = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_account_responsibility_change \
         (change_ref,account_ref,next_member_ref,changed_by_member_ref,reason,request_ref,resulting_version) \
         VALUES($1,$2,$3,$4,'合成首次分配',$5,2)",
    )
    .bind(change)
    .bind(account)
    .bind(member)
    .bind(admin)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    let historical_owner: String = sqlx::query_scalar(
        "SELECT responsible_user_source_id FROM linggan_ops_account WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(historical_owner, "legacy-owner");
    assert!(
        sqlx::query(
            "UPDATE linggan_ops_account_responsibility_change \
             SET reason='篡改' WHERE change_ref=$1"
        )
        .bind(change)
        .execute(database.pool())
        .await
        .is_err(),
        "responsibility receipt must be append-only"
    );
    assert!(
        sqlx::query("DELETE FROM linggan_ops_account_responsibility_change WHERE change_ref=$1")
            .bind(change)
            .execute(database.pool())
            .await
            .is_err(),
        "responsibility receipt must not be erasable"
    );
    let bootstrap_invite = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_invite \
         (invite_ref,member_ref,token_sha256,invited_by_member_ref,expires_at) \
         VALUES($1,$2,decode(repeat('e',64),'hex'),$2,scope_001_now()+interval '7 days')",
    )
    .bind(bootstrap_invite)
    .bind(admin)
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        sqlx::query(
            "INSERT INTO linggan_ops_first_admin_bootstrap \
             (bootstrap_ref,member_ref,invite_ref,request_ref,control_channel) \
             VALUES($1,$2,$3,$4,'local_runtime_console')"
        )
        .bind(Uuid::new_v4())
        .bind(admin)
        .bind(bootstrap_invite)
        .bind(Uuid::new_v4())
        .execute(database.pool())
        .await
        .is_err(),
        "the schema must reject bootstrap after any team member already exists"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn first_admin_bootstrap_receipt_requires_an_empty_invited_admin_team() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_bootstrap_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let admin = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_member (member_ref,email_normalized,display_name,role,state) \
         VALUES($1,'first-admin@example.test','合成首位管理员','admin','invited')",
    )
    .bind(admin)
    .execute(database.pool())
    .await
    .unwrap();
    let invite = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_invite \
         (invite_ref,member_ref,token_sha256,invited_by_member_ref,expires_at) \
         VALUES($1,$2,decode(repeat('a',64),'hex'),$2,scope_001_now()+interval '7 days')",
    )
    .bind(invite)
    .bind(admin)
    .execute(database.pool())
    .await
    .unwrap();
    let receipt = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_first_admin_bootstrap \
         (bootstrap_ref,member_ref,invite_ref,request_ref,control_channel) \
         VALUES($1,$2,$3,$4,'local_runtime_console')",
    )
    .bind(receipt)
    .bind(admin)
    .bind(invite)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        sqlx::query(
            "UPDATE linggan_ops_first_admin_bootstrap SET request_ref=$2 WHERE bootstrap_ref=$1"
        )
        .bind(receipt)
        .bind(Uuid::new_v4())
        .execute(database.pool())
        .await
        .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM linggan_ops_first_admin_bootstrap WHERE bootstrap_ref=$1")
            .bind(receipt)
            .execute(database.pool())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn auth_rejects_invalid_cookie_cross_site_and_storage_failures() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_auth_reject_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let member_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_member \
         (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) \
         VALUES($1,'broken@example.test','损坏哈希','operator','active','not-an-argon2-hash',scope_001_now())",
    )
    .bind(member_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let app = app_with_database(database.clone());
    let invalid_hash = json_request(
        "/api/local/auth/login",
        r#"{"email":"broken@example.test","password":"anything"}"#,
    );
    assert_eq!(
        app.clone().oneshot(invalid_hash).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let mut cross_site = json_request(
        "/api/local/auth/login",
        r#"{"email":"broken@example.test","password":"anything"}"#,
    );
    cross_site.headers_mut().insert(
        header::ORIGIN,
        HeaderValue::from_static("https://attacker.example"),
    );
    assert_eq!(
        app.clone().oneshot(cross_site).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let forged = session_request(
        "/api/local/auth/session",
        "linggan_ops_session=forged",
        "GET",
    );
    assert_eq!(
        app.clone().oneshot(forged).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let expired = stored_session(
        database.pool(),
        member_ref,
        "expired",
        "-2 hours",
        "-1 hour",
    )
    .await;
    assert_eq!(
        app.clone()
            .oneshot(session_request("/api/local/auth/session", &expired, "GET"))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let producer = Request::builder()
        .method("POST")
        .uri("/api/local/producer/manual-tasks")
        .header("host", "127.0.0.1:3000")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(producer).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "the existing native producer contract remains independently reachable"
    );
    sqlx::query("DROP TABLE linggan_ops_member CASCADE")
        .execute(database.pool())
        .await
        .unwrap();
    let unavailable_login = json_request(
        "/api/local/auth/login",
        r#"{"email":"broken@example.test","password":"anything"}"#,
    );
    assert_eq!(
        app.clone()
            .oneshot(unavailable_login)
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "a member lookup failure is operationally distinct from invalid credentials",
    );
    sqlx::query("DROP TABLE linggan_ops_session")
        .execute(database.pool())
        .await
        .unwrap();
    let unavailable_logout = session_request(
        "/api/local/auth/logout",
        "linggan_ops_session=forged",
        "POST",
    );
    assert_eq!(
        app.oneshot(unavailable_logout).await.unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn admin_invites_once_reinvite_revokes_old_link_and_acceptance_activates_member() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_auth_invite_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let admin_ref = active_member(database.pool(), "admin@example.test", "admin").await;
    let operator_ref = active_member(database.pool(), "operator@example.test", "operator").await;
    let app = app_with_database(database.clone());

    let admin_cookie = login_cookie(app.clone(), "admin@example.test").await;
    let operator_cookie = login_cookie(app.clone(), "operator@example.test").await;
    let denied = authenticated_json_request(
        "/api/local/auth/invitations",
        r#"{"email":"new@example.test","displayName":"新成员","role":"operator"}"#,
        &operator_cookie,
    );
    assert_eq!(
        app.clone().oneshot(denied).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let invalid_email = authenticated_json_request(
        "/api/local/auth/invitations",
        r#"{"email":"not-an-email","displayName":"新成员","role":"operator"}"#,
        &admin_cookie,
    );
    assert_eq!(
        app.clone().oneshot(invalid_email).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let first = authenticated_json_request(
        "/api/local/auth/invitations",
        r#"{"email":"new@example.test","displayName":"新成员","role":"operator"}"#,
        &admin_cookie,
    );
    let first = app.clone().oneshot(first).await.unwrap();
    assert_eq!(first.headers()["cache-control"], "no-store");
    let (status, first) = json_response(first).await;
    assert_eq!(status, StatusCode::CREATED);
    let first_token = first["invitation"]["token"].as_str().unwrap().to_owned();
    assert_eq!(first["invitation"]["expiresInSeconds"], 604_800);
    let first_digest: [u8; 32] = Sha256::digest(first_token.as_bytes()).into();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ops_invite WHERE token_sha256=$1"
        )
        .bind(first_digest.as_slice())
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1,
        "the raw invitation token never becomes a stored credential"
    );
    let second = authenticated_json_request(
        "/api/local/auth/invitations",
        r#"{"email":"new@example.test","displayName":"新成员（更新）","role":"lead"}"#,
        &admin_cookie,
    );
    let (status, second) = json_response(app.clone().oneshot(second).await.unwrap()).await;
    assert_eq!(status, StatusCode::CREATED);
    let second_token = second["invitation"]["token"].as_str().unwrap().to_owned();
    assert_ne!(first_token, second_token);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ops_member WHERE email_normalized='new@example.test'"
        )
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1,
        "reinvitation must not make another member identity"
    );
    let forged = json_request(
        "/api/local/auth/invitations/accept",
        r#"{"token":"0000000000000000000000000000000000000000000000000000000000000000","password":"synthetic-invite-password"}"#,
    );
    assert_eq!(
        app.clone().oneshot(forged).await.unwrap().status(),
        StatusCode::GONE
    );
    let expired_member_ref = Uuid::new_v4();
    let expired_token = "e".repeat(64);
    let expired_digest: [u8; 32] = Sha256::digest(expired_token.as_bytes()).into();
    sqlx::query(
        "INSERT INTO linggan_ops_member (member_ref,email_normalized,display_name,role,state) VALUES($1,'expired@example.test','过期邀请','operator','invited')",
    )
    .bind(expired_member_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_ops_invite (invite_ref,member_ref,token_sha256,invited_by_member_ref,issued_at,expires_at) VALUES($1,$2,$3,$4,scope_001_now()-interval '2 hours',scope_001_now()-interval '1 hour')",
    )
    .bind(Uuid::new_v4())
    .bind(expired_member_ref)
    .bind(expired_digest.as_slice())
    .bind(admin_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let expired = json_request(
        "/api/local/auth/invitations/accept",
        format!(r#"{{"token":"{expired_token}","password":"synthetic-invite-password"}}"#),
    );
    assert_eq!(
        app.clone().oneshot(expired).await.unwrap().status(),
        StatusCode::GONE
    );
    let old_link = json_request(
        "/api/local/auth/invitations/accept",
        format!(r#"{{"token":"{first_token}","password":"synthetic-invite-password"}}"#),
    );
    assert_eq!(
        app.clone().oneshot(old_link).await.unwrap().status(),
        StatusCode::GONE
    );
    let acceptance = json_request(
        "/api/local/auth/invitations/accept",
        format!(r#"{{"token":"{second_token}","password":"synthetic-invite-password"}}"#),
    );
    let acceptance = app.clone().oneshot(acceptance).await.unwrap();
    assert_eq!(acceptance.status(), StatusCode::OK);
    let accepted_cookie = acceptance.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    let (state, role, password_hash) = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT state, role, password_hash FROM linggan_ops_member WHERE email_normalized='new@example.test'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(state, "active");
    assert_eq!(role, "lead");
    assert!(password_hash.is_some());
    assert_eq!(
        app.clone()
            .oneshot(session_request(
                "/api/local/auth/session",
                &accepted_cookie,
                "GET"
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let replay = json_request(
        "/api/local/auth/invitations/accept",
        format!(r#"{{"token":"{second_token}","password":"synthetic-invite-password"}}"#),
    );
    assert_eq!(
        app.oneshot(replay).await.unwrap().status(),
        StatusCode::CONFLICT,
        "single-use acceptance remains explicit under replay"
    );
    assert_ne!(admin_ref, operator_ref);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL; scripts/test-web-app-postgres.sh"]
async fn password_change_requires_current_secret_revokes_old_sessions_and_issues_one_fresh_session()
{
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let schema = format!("ops_auth_pw_{}", Uuid::new_v4().simple());
    let database = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    active_member(database.pool(), "member@example.test", "operator").await;
    let app = app_with_database(database.clone());
    let first_cookie = login_cookie(app.clone(), "member@example.test").await;
    let second_cookie = login_cookie(app.clone(), "member@example.test").await;

    let malformed = authenticated_json_request(
        "/api/local/auth/password/change",
        r#"{"currentPassword":"","newPassword":"new-synthetic-password"}"#,
        &first_cookie,
    );
    assert_eq!(
        app.clone().oneshot(malformed).await.unwrap().status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let wrong_current = authenticated_json_request(
        "/api/local/auth/password/change",
        r#"{"currentPassword":"wrong","newPassword":"new-synthetic-password"}"#,
        &first_cookie,
    );
    assert_eq!(
        app.clone().oneshot(wrong_current).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.clone()
            .oneshot(session_request(
                "/api/local/auth/session",
                &first_cookie,
                "GET"
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK,
        "a rejected current secret must not revoke the existing session"
    );

    let change = authenticated_json_request(
        "/api/local/auth/password/change",
        r#"{"currentPassword":"synthetic-password-only","newPassword":"new-synthetic-password"}"#,
        &first_cookie,
    );
    let change = app.clone().oneshot(change).await.unwrap();
    assert_eq!(change.status(), StatusCode::OK);
    let fresh_cookie = change.headers()["set-cookie"].to_str().unwrap().to_owned();
    assert!(fresh_cookie.contains("HttpOnly; SameSite=Strict; Path=/"));
    assert!(!fresh_cookie.contains("new-synthetic-password"));
    for stale_cookie in [&first_cookie, &second_cookie] {
        assert_eq!(
            app.clone()
                .oneshot(session_request(
                    "/api/local/auth/session",
                    stale_cookie,
                    "GET"
                ))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED,
            "password change must revoke every pre-change session"
        );
    }
    assert_eq!(
        app.clone()
            .oneshot(session_request(
                "/api/local/auth/session",
                &fresh_cookie,
                "GET"
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let old_login = json_request(
        "/api/local/auth/login",
        r#"{"email":"member@example.test","password":"synthetic-password-only"}"#,
    );
    assert_eq!(
        app.clone().oneshot(old_login).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let new_login = json_request(
        "/api/local/auth/login",
        r#"{"email":"member@example.test","password":"new-synthetic-password"}"#,
    );
    assert_eq!(
        app.clone().oneshot(new_login).await.unwrap().status(),
        StatusCode::OK
    );
    let password_hash = sqlx::query_scalar::<_, String>(
        "SELECT password_hash FROM linggan_ops_member WHERE email_normalized='member@example.test'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_ne!(password_hash, "new-synthetic-password");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_ops_session WHERE revoked_reason='credential_reset'",
        )
        .fetch_one(database.pool())
        .await
        .unwrap(),
        2
    );
}

fn json_request(path: &str, body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("host", "127.0.0.1:3000")
        .header("content-type", "application/json")
        .body(body.into())
        .unwrap()
}

fn authenticated_json_request(path: &str, body: impl Into<Body>, cookie: &str) -> Request<Body> {
    let value = cookie.split(';').next().unwrap();
    Request::builder()
        .method("POST")
        .uri(path)
        .header("host", "127.0.0.1:3000")
        .header("content-type", "application/json")
        .header("cookie", value)
        .body(body.into())
        .unwrap()
}

async fn json_response(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn login_cookie(app: Router, email: &str) -> String {
    let request = json_request(
        "/api/local/auth/login",
        format!(r#"{{"email":"{email}","password":"synthetic-password-only"}}"#),
    );
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned()
}

async fn active_member(pool: &sqlx::PgPool, email: &str, role: &str) -> Uuid {
    let password_hash = Argon2::default()
        .hash_password(
            b"synthetic-password-only",
            &SaltString::encode_b64(b"isolated-proof-salt").unwrap(),
        )
        .unwrap()
        .to_string();
    let member_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_member (member_ref,email_normalized,display_name,role,state,password_hash,activated_at) VALUES($1,$2,'合成管理员',$3,'active',$4,scope_001_now())",
    )
    .bind(member_ref)
    .bind(email)
    .bind(role)
    .bind(password_hash)
    .execute(pool)
    .await
    .unwrap();
    member_ref
}

fn session_request(path: &str, cookie: &str, method: &str) -> Request<Body> {
    let value = cookie.split(';').next().unwrap();
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:3000")
        .header("cookie", value)
        .body(Body::empty())
        .unwrap()
}

async fn fresh_session(pool: &sqlx::PgPool, member_ref: Uuid) -> String {
    stored_session(
        pool,
        member_ref,
        "synthetic_session_only",
        "0 seconds",
        "1 hour",
    )
    .await
}

async fn stored_session(
    pool: &sqlx::PgPool,
    member_ref: Uuid,
    token: &str,
    issued_offset: &str,
    expires_offset: &str,
) -> String {
    let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    sqlx::query(
        "INSERT INTO linggan_ops_session \
         (session_ref,member_ref,token_sha256,issued_at,last_seen_at,expires_at) \
         VALUES($1,$2,$3,scope_001_now() + $4::interval,scope_001_now() + $4::interval,scope_001_now() + $5::interval)",
    )
    .bind(Uuid::new_v4())
    .bind(member_ref)
    .bind(digest.as_slice())
    .bind(issued_offset)
    .bind(expires_offset)
    .execute(pool)
    .await
    .unwrap();
    format!("linggan_ops_session={token}")
}

#[path = "ops_auth_concurrency_tests.rs"]
mod concurrency;
