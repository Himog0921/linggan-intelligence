//! OPS-AUTH-001: browser-only team identity.  This module deliberately does not touch producer
//! credentials, collection leases, or station protocol identities.
use super::*;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

const SESSION_COOKIE: &str = "linggan_ops_session";
const SESSION_TTL_SECONDS: i64 = 604_800;
const INVITE_TTL_SECONDS: i64 = 604_800;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/auth/login", post(login))
        .route("/api/local/auth/session", get(session))
        .route("/api/local/auth/logout", post(logout))
        .route("/api/local/auth/password/change", post(change_password))
        .route(
            "/api/local/auth/members/{member_ref}/deactivate",
            post(deactivate_member),
        )
        .route("/api/local/auth/invitations", post(create_invitation))
        .route(
            "/api/local/auth/invitations/accept",
            post(accept_invitation),
        )
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

#[derive(Deserialize)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct CreateInvitationRequest {
    email: String,
    #[serde(rename = "displayName")]
    display_name: String,
    role: String,
}

#[derive(Deserialize)]
struct AcceptInvitationRequest {
    token: String,
    password: String,
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    #[serde(rename = "currentPassword")]
    current_password: String,
    #[serde(rename = "newPassword")]
    new_password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeactivateMemberRequest {
    request_id: Uuid,
    expected_version: i64,
    reason: String,
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
    let token = new_session_token();
    let result: Result<Option<AuthenticatedMember>, sqlx::Error> = async {
        let mut transaction = database.pool().begin().await?;
        let member = sqlx::query_as::<_, (Uuid, String, String, String)>(
            "SELECT member_ref, display_name, role, password_hash \
             FROM linggan_ops_member \
             WHERE email_normalized=$1 AND state='active' FOR SHARE",
        )
        .bind(&email)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(member) = member else {
            return Ok(None);
        };
        if !password_matches(&member.3, &request.password) {
            return Ok(None);
        }
        issue_session_transaction(&mut transaction, member.0, &token).await?;
        transaction.commit().await?;
        Ok(Some(member_identity(member)))
    }
    .await;
    match result {
        Ok(Some(member)) => with_session_cookie(
            Json(member_json(&member)).into_response(),
            &token,
            secure_cookie_required(&headers),
        ),
        Ok(None) => invalid_credentials(),
        Err(_) => auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"),
    }
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

async fn change_password(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Response {
    let member = match current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    if !valid_password(&request.current_password) || !valid_password(&request.new_password) {
        return request_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ops_password_change_invalid",
        );
    }
    let Some(database) = state.database.database() else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let token = new_session_token();
    let result: Result<PasswordChange, ()> = async {
        let mut transaction = database.pool().begin().await.map_err(|_| ())?;
        let stored = sqlx::query_as::<_, (String, String)>(
            "SELECT password_hash, state FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE",
        )
        .bind(member.member_ref)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| ())?;
        let Some((current_hash, state)) = stored else {
            return Ok(PasswordChange::InvalidCredentials);
        };
        if state != "active" || !password_matches(&current_hash, &request.current_password) {
            return Ok(PasswordChange::InvalidCredentials);
        }
        let password_hash = hash_password(&request.new_password).map_err(|_| ())?;
        sqlx::query(
            "UPDATE linggan_ops_member SET password_hash=$2, version=version+1 WHERE member_ref=$1 AND state='active'",
        )
        .bind(member.member_ref)
        .bind(password_hash)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
        sqlx::query(
            "UPDATE linggan_ops_session SET revoked_at=scope_001_now(), revoked_reason='credential_reset' \
             WHERE member_ref=$1 AND revoked_at IS NULL",
        )
        .bind(member.member_ref)
        .execute(&mut *transaction)
        .await
        .map_err(|_| ())?;
        issue_session_transaction(&mut transaction, member.member_ref, &token)
            .await
            .map_err(|_| ())?;
        transaction.commit().await.map_err(|_| ())?;
        Ok(PasswordChange::Changed)
    }
    .await;
    match result {
        Ok(PasswordChange::Changed) => with_session_cookie(
            Json(member_json(&member)).into_response(),
            &token,
            secure_cookie_required(&headers),
        ),
        Ok(PasswordChange::InvalidCredentials) => invalid_credentials(),
        Err(()) => auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"),
    }
}

/// Stop browser access first, then leave currently owned accounts explicitly unassigned.  This is
/// intentionally not a handover shortcut: a later administrator assigns each account under the
/// ordinary responsibility command, while historical publishers and imported operators remain
/// untouched.
async fn deactivate_member(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(target_member_ref): Path<Uuid>,
    Json(request): Json<DeactivateMemberRequest>,
) -> Response {
    let actor = match current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let reason = request.reason.trim();
    if request.expected_version <= 0 || reason.is_empty() || reason.len() > 1000 || target_member_ref == actor.member_ref {
        return request_error(StatusCode::UNPROCESSABLE_ENTITY, "ops_member_deactivation_invalid");
    }
    let Some(database) = state.database.database() else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let payload: [u8; 32] = Sha256::digest(
        serde_json::to_vec(&json!({"targetMemberRef":target_member_ref,"expectedVersion":request.expected_version,"reason":reason}))
            .expect("deactivation payload serializes"),
    )
    .into();
    let result: Result<(StatusCode, Value), Response> = async {
        let mut transaction = database.pool().begin().await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text,0))")
            .bind(request.request_id.to_string()).execute(&mut *transaction).await
            .map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
        if let Some((stored_actor, stored_payload, outcome, code, count, version)) = sqlx::query_as::<_, (Uuid, Vec<u8>, String, String, i32, Option<i64>)>(
            "SELECT actor_member_ref,payload_sha256,outcome,result_code,unassigned_account_count,resulting_version \
             FROM linggan_ops_member_deactivation_request WHERE request_ref=$1",
        ).bind(request.request_id).fetch_optional(&mut *transaction).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))? {
            if stored_actor != actor.member_ref || stored_payload != payload { return Err(request_error(StatusCode::CONFLICT, "ops_member_deactivation_conflict")); }
            transaction.commit().await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
            let status = if outcome == "accepted" || outcome == "unchanged" { StatusCode::OK } else { StatusCode::CONFLICT };
            return Ok((status, json!({"requestId":request.request_id,"outcome":outcome,"code":code,"unassignedAccountCount":count,"resultingVersion":version,"replayed":true})));
        }
        let actor_is_admin: bool = sqlx::query_scalar("SELECT role='admin' AND state='active' FROM linggan_ops_member WHERE member_ref=$1 FOR SHARE")
            .bind(actor.member_ref).fetch_optional(&mut *transaction).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?.unwrap_or(false);
        if !actor_is_admin { return Err(request_error(StatusCode::FORBIDDEN, "ops_auth_forbidden")); }
        let target: Option<(String, String, i64)> = sqlx::query_as("SELECT role,state,version FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE")
            .bind(target_member_ref).fetch_optional(&mut *transaction).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
        let Some((role, member_state, version)) = target else { return Err(request_error(StatusCode::NOT_FOUND, "ops_member_not_found")); };
        let (outcome, code, result_version, count) = if version != request.expected_version {
            ("stale", "ops_member_version_stale", None, 0)
        } else if member_state == "disabled" {
            ("unchanged", "ops_member_already_disabled", Some(version), 0)
        } else if role == "admin" {
            let active_admins: Vec<Uuid> = sqlx::query_scalar("SELECT member_ref FROM linggan_ops_member WHERE role='admin' AND state='active' FOR UPDATE")
                .fetch_all(&mut *transaction).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
            if active_admins.len() <= 1 { ("rejected", "ops_member_last_admin_protected", None, 0) } else { deactivate_in(&mut transaction, target_member_ref, actor.member_ref, reason).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))? }
        } else { deactivate_in(&mut transaction, target_member_ref, actor.member_ref, reason).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))? };
        sqlx::query("INSERT INTO linggan_ops_member_deactivation_request (request_ref,actor_member_ref,target_member_ref,expected_version,payload_sha256,reason,outcome,result_code,unassigned_account_count,resulting_version) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(request.request_id).bind(actor.member_ref).bind(target_member_ref).bind(request.expected_version).bind(payload.as_slice()).bind(reason).bind(outcome).bind(code).bind(count).bind(result_version)
            .execute(&mut *transaction).await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
        transaction.commit().await.map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
        let status = if outcome == "accepted" || outcome == "unchanged" { StatusCode::OK } else { StatusCode::CONFLICT };
        Ok((status, json!({"requestId":request.request_id,"outcome":outcome,"code":code,"unassignedAccountCount":count,"resultingVersion":result_version,"replayed":false})))
    }.await;
    match result { Ok((status, body)) => { let mut response = (status, Json(body)).into_response(); secure_headers(&mut response); response }, Err(response) => response }
}

async fn deactivate_in(transaction: &mut Transaction<'_, Postgres>, target: Uuid, actor: Uuid, reason: &str) -> Result<(&'static str, &'static str, Option<i64>, i32), sqlx::Error> {
    let version: i64 = sqlx::query_scalar("UPDATE linggan_ops_member SET state='disabled',disabled_at=scope_001_now(),version=version+1 WHERE member_ref=$1 RETURNING version")
        .bind(target).fetch_one(&mut **transaction).await?;
    sqlx::query("UPDATE linggan_ops_session SET revoked_at=scope_001_now(),revoked_reason='member_disabled' WHERE member_ref=$1 AND revoked_at IS NULL")
        .bind(target).execute(&mut **transaction).await?;
    sqlx::query("UPDATE linggan_ops_invite SET revoked_at=scope_001_now(),revoked_reason='member_disabled' WHERE member_ref=$1 AND accepted_at IS NULL AND revoked_at IS NULL")
        .bind(target).execute(&mut **transaction).await?;
    sqlx::query("UPDATE linggan_ops_password_reset SET revoked_at=scope_001_now(),revoked_reason='member_disabled' WHERE member_ref=$1 AND consumed_at IS NULL AND revoked_at IS NULL")
        .bind(target).execute(&mut **transaction).await?;
    let accounts: Vec<(Uuid, i64)> = sqlx::query_as("UPDATE linggan_ops_account SET current_responsible_member_ref=NULL,responsibility_version=responsibility_version+1 WHERE current_responsible_member_ref=$1 RETURNING account_ref,responsibility_version")
        .bind(target).fetch_all(&mut **transaction).await?;
    for (account_ref, resulting_version) in &accounts {
        sqlx::query("INSERT INTO linggan_ops_account_responsibility_change (change_ref,account_ref,previous_member_ref,next_member_ref,changed_by_member_ref,reason,request_ref,resulting_version) VALUES($1,$2,$3,NULL,$4,$5,$6,$7)")
            .bind(Uuid::new_v4()).bind(account_ref).bind(target).bind(actor).bind(format!("成员停用：{reason}")).bind(Uuid::new_v4()).bind(resulting_version).execute(&mut **transaction).await?;
    }
    Ok(("accepted", "ops_member_deactivated", Some(version), i32::try_from(accounts.len()).unwrap_or(i32::MAX)))
}

async fn create_invitation(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(request): Json<CreateInvitationRequest>,
) -> Response {
    let member = match current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    if member.role != "admin" {
        return request_error(StatusCode::FORBIDDEN, "ops_auth_forbidden");
    }
    let Some(database) = state.database.database() else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let email = normalize_email(&request.email);
    let display_name = request.display_name.trim();
    if !valid_email(&email)
        || display_name.is_empty()
        || display_name.len() > 160
        || !matches!(request.role.as_str(), "admin" | "lead" | "operator")
    {
        return request_error(StatusCode::UNPROCESSABLE_ENTITY, "ops_invitation_invalid");
    }
    let token = new_session_token();
    let digest = token_digest(&token);
    let result = async {
        let mut transaction = database.pool().begin().await?;
        let existing = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT member_ref, state FROM linggan_ops_member WHERE email_normalized=$1 FOR UPDATE",
        )
        .bind(&email)
        .fetch_optional(&mut *transaction)
        .await?;
        let member_ref = match existing {
            Some((_, state)) if state == "active" => return Ok::<_, sqlx::Error>(None),
            Some((_, state)) if state == "disabled" => return Ok(None),
            Some((member_ref, _)) => {
                sqlx::query(
                    "UPDATE linggan_ops_member SET display_name=$2, role=$3, version=version+1 \
                     WHERE member_ref=$1",
                )
                .bind(member_ref)
                .bind(display_name)
                .bind(&request.role)
                .execute(&mut *transaction)
                .await?;
                member_ref
            }
            None => {
                let member_ref = Uuid::new_v4();
                sqlx::query(
                    "INSERT INTO linggan_ops_member \
                     (member_ref,email_normalized,display_name,role,state) \
                     VALUES($1,$2,$3,$4,'invited')",
                )
                .bind(member_ref)
                .bind(&email)
                .bind(display_name)
                .bind(&request.role)
                .execute(&mut *transaction)
                .await?;
                member_ref
            }
        };
        sqlx::query(
            "UPDATE linggan_ops_invite SET revoked_at=scope_001_now(), revoked_reason='superseded' \
             WHERE member_ref=$1 AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(member_ref)
        .execute(&mut *transaction)
        .await?;
        let invite_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_ops_invite \
             (invite_ref,member_ref,token_sha256,invited_by_member_ref,expires_at) \
             VALUES($1,$2,$3,$4,scope_001_now()+make_interval(secs => $5))",
        )
        .bind(invite_ref)
        .bind(member_ref)
        .bind(digest.as_slice())
        .bind(member.member_ref)
        .bind(INVITE_TTL_SECONDS as f64)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some((invite_ref, member_ref)))
    }
    .await;
    match result {
        Ok(Some((invite_ref, member_ref))) => {
            let mut response = (
                StatusCode::CREATED,
                Json(json!({
                    "invitation": {
                        "ref": invite_ref,
                        "memberRef": member_ref,
                        "token": token,
                        "expiresInSeconds": INVITE_TTL_SECONDS,
                    }
                })),
            )
                .into_response();
            secure_headers(&mut response);
            response
        }
        Ok(None) => request_error(StatusCode::CONFLICT, "ops_invitation_member_unavailable"),
        Err(_) => auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"),
    }
}

async fn accept_invitation(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(request): Json<AcceptInvitationRequest>,
) -> Response {
    if request.token.len() != 64 || request.password.is_empty() {
        return request_error(StatusCode::UNPROCESSABLE_ENTITY, "ops_invitation_invalid");
    }
    let Some(database) = state.database.database() else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let Ok(password_hash) = hash_password(&request.password) else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let token = new_session_token();
    let digest = token_digest(&request.token);
    let result = async {
        let mut transaction = database.pool().begin().await?;
        let invite = sqlx::query_as::<_, (Uuid, Uuid, bool, bool, String)>(
            "SELECT invite.invite_ref, invite.member_ref, invite.accepted_at IS NOT NULL, \
                    (invite.revoked_at IS NOT NULL OR invite.expires_at <= scope_001_now()), member.state \
             FROM linggan_ops_invite AS invite \
             JOIN linggan_ops_member AS member ON member.member_ref=invite.member_ref \
             WHERE invite.token_sha256=$1 FOR UPDATE OF invite, member",
        )
        .bind(digest.as_slice())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((invite_ref, member_ref, accepted, unavailable, member_state)) = invite else {
            return Ok::<_, sqlx::Error>(InvitationAcceptance::Gone);
        };
        if accepted || member_state == "active" {
            return Ok(InvitationAcceptance::AlreadyAccepted);
        }
        if unavailable || member_state != "invited" {
            return Ok(InvitationAcceptance::Gone);
        }
        sqlx::query(
            "UPDATE linggan_ops_member SET state='active', password_hash=$2, activated_at=scope_001_now(), version=version+1 \
             WHERE member_ref=$1 AND state='invited'",
        )
        .bind(member_ref)
        .bind(&password_hash)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE linggan_ops_invite SET accepted_at=scope_001_now() WHERE invite_ref=$1 AND accepted_at IS NULL",
        )
        .bind(invite_ref)
        .execute(&mut *transaction)
        .await?;
        issue_session_transaction(&mut transaction, member_ref, &token).await?;
        transaction.commit().await?;
        Ok(InvitationAcceptance::Accepted(member_ref))
    }
    .await;
    match result {
        Ok(InvitationAcceptance::Accepted(member_ref)) => with_session_cookie(
            Json(json!({ "member": { "ref": member_ref } })).into_response(),
            &token,
            secure_cookie_required(&headers),
        ),
        Ok(InvitationAcceptance::AlreadyAccepted) => {
            request_error(StatusCode::CONFLICT, "ops_invitation_already_accepted")
        }
        Ok(InvitationAcceptance::Gone) => {
            request_error(StatusCode::GONE, "ops_invitation_invalid_or_expired")
        }
        Err(_) => auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"),
    }
}

enum InvitationAcceptance {
    Accepted(Uuid),
    AlreadyAccepted,
    Gone,
}

enum PasswordChange {
    Changed,
    InvalidCredentials,
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

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    use argon2::{PasswordHasher, password_hash::SaltString};

    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
}

fn valid_email(value: &str) -> bool {
    value.len() >= 3
        && value.len() <= 320
        && value.split('@').count() == 2
        && value
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty())
}

fn valid_password(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096
}

fn new_session_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn token_digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

async fn issue_session_transaction(
    transaction: &mut Transaction<'_, Postgres>,
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
    .execute(&mut **transaction)
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

fn request_error(status: StatusCode, code: &'static str) -> Response {
    let mut response = (status, Json(json!({ "code": code }))).into_response();
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

pub(super) fn secure_headers(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
}
