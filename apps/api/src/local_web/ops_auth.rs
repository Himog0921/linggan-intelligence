//! OPS-AUTH-001: browser-only team identity.  This module deliberately does not touch producer
//! credentials, collection leases, or station protocol identities.
use super::*;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const SESSION_COOKIE: &str = "linggan_ops_session";
const SESSION_TTL_SECONDS: i64 = 604_800;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/auth/login", post(login))
        .route("/api/local/auth/session", get(session))
        .route("/api/local/auth/logout", post(logout))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AuthenticatedMember {
    pub(super) member_ref: Uuid,
    pub(super) display_name: String,
    pub(super) role: String,
}

async fn login(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Response {
    let Some(database) = state.database.database() else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let email = normalize_email(&request.email);
    let member = match find_login_member(database, &email).await {
        Ok(Some(member)) => member,
        Ok(None) => return invalid_credentials(),
        Err(_) => return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"),
    };
    if !password_matches(&member.3, &request.password) {
        return invalid_credentials();
    }
    let token = new_session_token();
    if issue_session(database, member.0, &token).await.is_err() {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    }
    with_session_cookie(
        Json(member_json(&member_identity(member))).into_response(),
        &token,
        secure_cookie_required(&headers),
    )
}

async fn session(State(state): State<LocalWebState>, headers: HeaderMap) -> Response {
    match current_member(&state, &headers).await {
        Ok(member) => Json(member_json(&member)).into_response(),
        Err(response) => response,
    }
}

async fn logout(State(state): State<LocalWebState>, headers: HeaderMap) -> Response {
    if let Some(token) = session_token(&headers) {
        let Some(database) = state.database.database() else {
            return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
        };
        let digest = token_digest(&token);
        if sqlx::query(
            "UPDATE linggan_ops_session SET revoked_at=scope_001_now(), revoked_reason='logout' \
             WHERE token_sha256=$1 AND revoked_at IS NULL",
        )
        .bind(digest.as_slice())
        .execute(database.pool())
        .await
        .is_err()
        {
            return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
        }
    }
    clear_session_cookie(
        axum::http::StatusCode::NO_CONTENT.into_response(),
        secure_cookie_required(&headers),
    )
}

pub(super) async fn current_member(
    state: &LocalWebState,
    headers: &HeaderMap,
) -> Result<AuthenticatedMember, Response> {
    let Some(database) = state.database.database() else {
        return Err(auth_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ops_auth_unavailable",
        ));
    };
    let Some(token) = session_token(headers) else {
        return Err(session_required());
    };
    let digest = token_digest(&token);
    let member = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT member.member_ref, member.display_name, member.role \
         FROM linggan_ops_session AS session \
         JOIN linggan_ops_member AS member ON member.member_ref=session.member_ref \
         WHERE session.token_sha256=$1 AND session.revoked_at IS NULL \
           AND session.expires_at > scope_001_now() AND member.state='active'",
    )
    .bind(digest.as_slice())
    .fetch_optional(database.pool())
    .await
    .map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
    let Some(member) = member else {
        return Err(session_required());
    };
    sqlx::query(
        "UPDATE linggan_ops_session SET last_seen_at=scope_001_now() \
         WHERE token_sha256=$1 AND revoked_at IS NULL",
    )
    .bind(digest.as_slice())
    .execute(database.pool())
    .await
    .map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
    Ok(AuthenticatedMember {
        member_ref: member.0,
        display_name: member.1,
        role: member.2,
    })
}

async fn find_login_member(
    database: &Database,
    email: &str,
) -> Result<Option<(Uuid, String, String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT member_ref, display_name, role, password_hash \
         FROM linggan_ops_member WHERE email_normalized=$1 AND state='active'",
    )
    .bind(email)
    .fetch_optional(database.pool())
    .await
}

fn normalize_email(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn password_matches(password_hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn new_session_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn token_digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

async fn issue_session(
    database: &Database,
    member_ref: Uuid,
    token: &str,
) -> Result<(), sqlx::Error> {
    let digest = token_digest(token);
    sqlx::query(
        "INSERT INTO linggan_ops_session \
         (session_ref, member_ref, token_sha256, expires_at) \
         VALUES($1,$2,$3,scope_001_now() + make_interval(secs => $4))",
    )
    .bind(Uuid::new_v4())
    .bind(member_ref)
    .bind(digest.as_slice())
    .bind(SESSION_TTL_SECONDS as f64)
    .execute(database.pool())
    .await
    .map(|_| ())
}

fn member_identity(member: (Uuid, String, String, String)) -> AuthenticatedMember {
    AuthenticatedMember {
        member_ref: member.0,
        display_name: member.1,
        role: member.2,
    }
}

fn member_json(member: &AuthenticatedMember) -> Value {
    json!({
        "member": {
            "ref": member.member_ref,
            "displayName": member.display_name,
            "role": member.role,
        }
    })
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| {
            raw.split(';').find_map(|pair| {
                let (name, value) = pair.trim().split_once('=')?;
                (name == SESSION_COOKIE && !value.is_empty()).then(|| value.to_owned())
            })
        })
}

fn invalid_credentials() -> Response {
    auth_error(StatusCode::UNAUTHORIZED, "ops_auth_invalid_credentials")
}

fn session_required() -> Response {
    auth_error(StatusCode::UNAUTHORIZED, "ops_auth_session_required")
}

fn auth_error(status: StatusCode, code: &'static str) -> Response {
    let mut response = (
        status,
        Json(json!({ "auth": "not_authenticated", "code": code })),
    )
        .into_response();
    secure_headers(&mut response);
    response
}

fn with_session_cookie(mut response: Response, token: &str, secure: bool) -> Response {
    let secure = secure.then_some("; Secure").unwrap_or_default();
    let cookie = format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={SESSION_TTL_SECONDS}{secure}"
    );
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("generated session cookie is valid"),
    );
    secure_headers(&mut response);
    response
}

fn clear_session_cookie(mut response: Response, secure: bool) -> Response {
    let secure = secure.then_some("; Secure").unwrap_or_default();
    let cookie = format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{secure}");
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("generated session cookie is valid"),
    );
    secure_headers(&mut response);
    response
}

fn secure_cookie_required(headers: &HeaderMap) -> bool {
    std::env::var("LINGGAN_OPS_COOKIE_SECURE").as_deref() == Ok("1")
        || headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("https"))
}

fn secure_headers(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
}
