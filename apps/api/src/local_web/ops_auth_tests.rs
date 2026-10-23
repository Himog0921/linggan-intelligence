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
