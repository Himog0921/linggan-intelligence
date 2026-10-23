//! OPS-AUTH-001 isolated PostgreSQL + Axum proof.  Real team credentials never enter this file.
use super::*;
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use axum::{
    body::Body,
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

fn json_request(path: &str, body: &'static str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header("host", "127.0.0.1:3000")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
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
