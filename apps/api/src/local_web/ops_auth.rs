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
use subtle::ConstantTimeEq;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

mod offboarding;

const SESSION_COOKIE: &str = "linggan_ops_session";
const SESSION_TTL_SECONDS: i64 = 604_800;
const INVITE_TTL_SECONDS: i64 = 604_800;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/auth/login", post(login))
        .route("/api/local/auth/session", get(session))
        .route("/api/local/auth/logout", post(logout))
        .route("/api/local/auth/members", get(list_members))
        .route("/api/local/auth/password/change", post(change_password))
        .route(
            "/api/local/auth/members/{member_ref}/deactivate",
            post(offboarding::deactivate_member),
        )
        .route("/api/local/auth/members/{member_ref}/offboarding-preview", get(offboarding::preview))
        .route("/api/local/auth/deactivations/{request_ref}", get(offboarding::receipt))
        .route("/api/local/auth/invitations", post(create_invitation))
        .route("/api/local/auth/invitations/{invite_ref}/revoke", post(revoke_invitation))
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TeamMemberView {
    member_ref: Uuid,
    email: String,
    display_name: String,
    role: String,
    state: String,
    version: i64,
    current_account_count: i64,
    invitation_ref: Option<Uuid>,
    invitation_state: Option<String>,
    group_ref: Option<Uuid>,
    group_name: Option<String>,
    group_has_active_lead: Option<bool>,
    latest_deactivation_ref: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AuthenticatedMember {
    session_ref: Option<Uuid>,
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
    let result: Result<Option<AuthenticatedMember>, ()> = async {
        // Verify without holding a connection or member lock during the expensive calculation.
        let hash: Option<String> = sqlx::query_scalar(
            "SELECT password_hash FROM linggan_ops_member WHERE email_normalized=$1 AND state='active'",
        ).bind(&email).fetch_optional(database.pool()).await.map_err(|_| ())?;
        let Some(hash) = hash else { return Ok(None) };
        if !password_matches_async(hash.clone(), request.password).await? {
            return Ok(None);
        }
        let mut transaction = database.pool().begin().await.map_err(|_| ())?;
        let member = sqlx::query_as::<_, (Uuid, String, String, String)>(
            "SELECT member_ref, display_name, role, password_hash \
             FROM linggan_ops_member \
             WHERE email_normalized=$1 AND state='active' FOR SHARE",
        )
        .bind(&email)
        .fetch_optional(&mut *transaction)
        .await.map_err(|_| ())?;
        let Some(member) = member else { return Ok(None) };
        // A password change or deactivation during verification must prevent session issuance.
        if member.3 != hash { return Ok(None) }
        issue_session_transaction(&mut transaction, member.0, &token).await.map_err(|_| ())?;
        transaction.commit().await.map_err(|_| ())?;
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

/// A management roster is deliberately separate from the active-member selector used by account
/// handover. It includes disabled/invited identities and versions, but never credential material.
async fn list_members(State(state): State<LocalWebState>, headers: HeaderMap) -> Response {
    let actor = match current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    if actor.role != "admin" {
        return request_error(StatusCode::FORBIDDEN, "ops_auth_forbidden");
    }
    let Some(database) = state.database.database() else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    type RosterRow = (Uuid, String, String, String, String, i64, i64, Option<Uuid>, Option<String>, Option<Uuid>, Option<String>, Option<bool>, Option<Uuid>);
    let members = sqlx::query_as::<_, RosterRow>(
        "SELECT member.member_ref, member.email_normalized AS email, member.display_name,
                member.role, member.state, member.version,
                (SELECT count(*) FROM linggan_ops_account account
                  WHERE account.current_responsible_member_ref=member.member_ref
                    AND ((account.source_system='linggan_native' AND account.native_record_state='active')
                      OR (account.source_system='content_workbench' AND account.ledger_state='in_ledger'))) AS current_account_count,
                invitation.invite_ref AS invitation_ref,
                invitation.state AS invitation_state,
                team_group.group_ref, team_group.display_name AS group_name,
                CASE WHEN team_group.group_ref IS NOT NULL THEN EXISTS (
                    SELECT 1 FROM linggan_ops_member lead
                    JOIN LATERAL (
                        SELECT group_ref,action FROM linggan_ops_group_membership_event
                         WHERE member_ref=lead.member_ref
                         ORDER BY occurred_at DESC,event_ref DESC LIMIT 1
                    ) latest ON true
                    WHERE lead.state='active' AND lead.role='lead'
                      AND latest.action='joined' AND latest.group_ref=team_group.group_ref
                ) END,
                (SELECT request_ref FROM linggan_ops_member_deactivation_request stop
                  WHERE stop.target_member_ref=member.member_ref AND stop.outcome='accepted'
                  ORDER BY created_at DESC, request_ref DESC LIMIT 1)
           FROM linggan_ops_member member
           LEFT JOIN LATERAL (
                SELECT invite_ref, CASE
                    WHEN accepted_at IS NOT NULL THEN 'accepted'
                    WHEN revoked_at IS NOT NULL THEN 'revoked'
                    WHEN expires_at <= scope_001_now() THEN 'expired'
                    ELSE 'pending' END AS state
                  FROM linggan_ops_invite
                 WHERE member_ref=member.member_ref
                 ORDER BY issued_at DESC, invite_ref DESC LIMIT 1
           ) invitation ON true
           LEFT JOIN LATERAL (
                SELECT membership.group_ref, membership.action
                  FROM linggan_ops_group_membership_event membership
                 WHERE membership.member_ref=member.member_ref
                 ORDER BY occurred_at DESC, event_ref DESC LIMIT 1
           ) membership ON true
           LEFT JOIN linggan_ops_group team_group ON team_group.group_ref=membership.group_ref
                    AND membership.action='joined' AND team_group.state='active'
          ORDER BY CASE member.state WHEN 'active' THEN 0 WHEN 'invited' THEN 1 ELSE 2 END,
                   member.display_name, member.member_ref",
    )
    .fetch_all(database.pool())
    .await;
    match members {
        Ok(rows) => {
            let members = rows.into_iter().map(|(member_ref, email, display_name, role, state,
                version, current_account_count, invitation_ref, invitation_state, group_ref, group_name,
                group_has_active_lead, latest_deactivation_ref)| TeamMemberView {
                member_ref, email, display_name, role, state, version, current_account_count,
                invitation_ref, invitation_state, group_ref, group_name, group_has_active_lead, latest_deactivation_ref,
            }).collect::<Vec<_>>();
            management_json(StatusCode::OK, json!({"members":members,"actorMemberRef":actor.member_ref}))
        }
        Err(_) => unavailable(),
    }
}

fn management_json(status: StatusCode, body: Value) -> Response {
    let mut response = (status, Json(body)).into_response();
    secure_headers(&mut response);
    response
}

fn unavailable() -> Response {
    auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable")
}

pub(super) async fn lock_active_member(
    transaction: &mut Transaction<'_, Postgres>,
    member: &AuthenticatedMember,
) -> Result<bool, sqlx::Error> {
    // Match the member-before-session lock order used by password changes and offboarding.
    let current = sqlx::query_scalar::<_, String>(
        "SELECT role FROM linggan_ops_member WHERE member_ref=$1 AND state='active' FOR SHARE",
    )
    .bind(member.member_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    if current.as_deref() != Some(member.role.as_str()) {
        return Ok(false);
    }
    Ok(sqlx::query_scalar::<_, Uuid>(
        "SELECT session_ref FROM linggan_ops_session WHERE session_ref=$1 AND member_ref=$2
           AND revoked_at IS NULL AND expires_at > scope_001_now() FOR SHARE",
    )
    .bind(member.session_ref)
    .bind(member.member_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .is_some())
}

async fn revoke_invitation(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(invite_ref): Path<Uuid>,
) -> Response {
    let actor = match current_member(&state, &headers).await {
        Ok(member) if member.role == "admin" => member,
        Ok(_) => return request_error(StatusCode::FORBIDDEN, "ops_auth_forbidden"),
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else { return unavailable() };
    let result = async {
        let mut tx = database.pool().begin().await?;
        if !lock_active_member(&mut tx, &actor).await? {
            return Ok::<_, sqlx::Error>((StatusCode::FORBIDDEN, "ops_auth_forbidden"));
        }
        let revoked = sqlx::query(
            "UPDATE linggan_ops_invite SET revoked_at=scope_001_now(), revoked_reason='administrator'
              WHERE invite_ref=$1 AND accepted_at IS NULL AND revoked_at IS NULL
                AND expires_at > scope_001_now()",
        )
        .bind(invite_ref)
        .execute(&mut *tx)
        .await?
        .rows_affected() == 1;
        tx.commit().await?;
        Ok(if revoked {
            (StatusCode::OK, "ops_invitation_revoked")
        } else {
            (StatusCode::CONFLICT, "ops_invitation_no_longer_pending")
        })
    }.await;
    match result {
        Ok((status, code)) => management_json(status, json!({ "code": code })),
        Err(_) => unavailable(),
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
        let hash: Option<String> = sqlx::query_scalar(
            "SELECT password_hash FROM linggan_ops_member WHERE member_ref=$1 AND state='active'",
        ).bind(member.member_ref).fetch_optional(database.pool()).await.map_err(|_| ())?;
        let Some(hash) = hash else { return Ok(PasswordChange::InvalidCredentials) };
        if !password_matches_async(hash.clone(), request.current_password).await? {
            return Ok(PasswordChange::InvalidCredentials);
        }
        let password_hash = hash_password_async(request.new_password).await?;
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
        if state != "active" || current_hash != hash {
            return Ok(PasswordChange::InvalidCredentials);
        }
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
        if !lock_active_member(&mut transaction, &member).await? {
            return Ok::<_, sqlx::Error>(None);
        }
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
    let Ok(password_hash) = hash_password_async(request.password).await else {
        return auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable");
    };
    let token = new_session_token();
    let digest = token_digest(&request.token);
    let result = async {
        let mut transaction = database.pool().begin().await?;
        let member_ref = sqlx::query_scalar::<_, Uuid>(
            "SELECT member_ref FROM linggan_ops_invite WHERE token_sha256=$1",
        ).bind(digest.as_slice()).fetch_optional(&mut *transaction).await?;
        let Some(member_ref) = member_ref else {
            return Ok::<_, sqlx::Error>(InvitationAcceptance::Gone);
        };
        let member_state: String = sqlx::query_scalar(
            "SELECT state FROM linggan_ops_member WHERE member_ref=$1 FOR UPDATE",
        ).bind(member_ref).fetch_one(&mut *transaction).await?;
        let (invite_ref, accepted, unavailable) = sqlx::query_as::<_, (Uuid, bool, bool)>(
            "SELECT invite_ref, accepted_at IS NOT NULL,
                    (revoked_at IS NOT NULL OR expires_at <= scope_001_now())
               FROM linggan_ops_invite WHERE token_sha256=$1 FOR UPDATE",
        ).bind(digest.as_slice()).fetch_one(&mut *transaction).await?;
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
    let member = sqlx::query_as::<_, (Uuid, String, String, Uuid)>(
        "SELECT member.member_ref, member.display_name, member.role, session.session_ref \
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
    // Activity tracking is best effort when a write holds a shared session lock.
    // Session revocation is checked above and again by each protected write.
    sqlx::query(
        "WITH touch AS (
            SELECT session_ref FROM linggan_ops_session
             WHERE token_sha256=$1 AND revoked_at IS NULL FOR UPDATE SKIP LOCKED
         )
         UPDATE linggan_ops_session session SET last_seen_at=scope_001_now()
           FROM touch WHERE session.session_ref=touch.session_ref",
    )
    .bind(digest.as_slice())
    .execute(database.pool())
    .await
    .map_err(|_| auth_error(StatusCode::SERVICE_UNAVAILABLE, "ops_auth_unavailable"))?;
    Ok(AuthenticatedMember {
        session_ref: Some(member.3),
        member_ref: member.0,
        display_name: member.1,
        role: member.2,
    })
}

fn normalize_email(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

async fn password_matches_async(hash: String, password: String) -> Result<bool, ()> {
    tokio::task::spawn_blocking(move || password_matches(&hash, &password))
        .await
        .map_err(|_| ())
}

async fn hash_password_async(password: String) -> Result<String, ()> {
    tokio::task::spawn_blocking(move || hash_password(&password).map_err(|_| ()))
        .await
        .map_err(|_| ())?
}

fn password_matches(password_hash: &str, password: &str) -> bool {
    if let Some(workbench_hash) = password_hash.strip_prefix("workbench-scrypt$") {
        return workbench_password_matches(workbench_hash, password);
    }
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Better Auth 1.6.9 stores `hex(salt):hex(scrypt(N=16384,r=16,p=1,dkLen=64))`.
/// Its salt input is the ASCII hex text and its password input is NFKC-normalized.
/// The prefix keeps imported verifiers distinct from Intelligence's Argon2 hashes.
fn workbench_password_matches(hash: &str, password: &str) -> bool {
    let Some((salt, expected)) = hash.split_once(':') else {
        return false;
    };
    if salt.len() != 32
        || expected.len() != 128
        || !salt.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }
    let Ok(expected) = hex::decode(expected) else {
        return false;
    };
    let Ok(params) = scrypt::Params::new(14, 16, 1, 64) else {
        return false;
    };
    let mut calculated = [0_u8; 64];
    let normalized = password.nfkc().collect::<String>();
    if scrypt::scrypt(
        normalized.as_bytes(),
        salt.as_bytes(),
        &params,
        &mut calculated,
    )
    .is_err()
    {
        return false;
    }
    bool::from(calculated.ct_eq(expected.as_slice()))
}

#[cfg(test)]
mod workbench_password_tests {
    use super::password_matches;

    #[test]
    fn imported_better_auth_verifier_accepts_existing_password_without_reset() {
        // Synthetic Better Auth 1.6.9 vector; no real member credential is in the test.
        let hash = "workbench-scrypt$00112233445566778899aabbccddeeff:503d058ffa344ded3ade56ae37c931eeefdf127685942056bd9f50687f4fc3ced01bc958abc4957631dbc6774e6ff941d25010d5b60c8ae0b261cb36ada8b5bb";
        assert!(password_matches(hash, "Ｐassword１２３"));
        assert!(password_matches(hash, "Password123"));
        assert!(!password_matches(hash, "wrong-password"));
        assert!(!password_matches(
            "workbench-scrypt$not-a-hash",
            "Password123"
        ));
    }
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
        session_ref: None,
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

#[cfg(test)]
mod password_scheduling_tests {
    use super::*;
    use std::cell::Cell;

    #[tokio::test(flavor = "current_thread")]
    async fn password_hashing_yields_to_other_async_work() {
        let progressed = Cell::new(false);
        let (hash, ()) = tokio::join!(
            async {
                let hash = hash_password_async("synthetic-password".into())
                    .await
                    .unwrap();
                assert!(progressed.get(), "hashing must let another task run");
                hash
            },
            async {
                progressed.set(true);
            }
        );
        assert!(password_matches(&hash, "synthetic-password"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn password_verification_yields_and_preserves_imported_scrypt_behavior() {
        let hash = "workbench-scrypt$00112233445566778899aabbccddeeff:503d058ffa344ded3ade56ae37c931eeefdf127685942056bd9f50687f4fc3ced01bc958abc4957631dbc6774e6ff941d25010d5b60c8ae0b261cb36ada8b5bb";
        let progressed = Cell::new(false);
        tokio::join!(
            async {
                assert!(
                    password_matches_async(hash.into(), "Ｐassword１２３".into())
                        .await
                        .unwrap()
                );
                assert!(progressed.get(), "verification must let another task run");
            },
            async {
                progressed.set(true);
            }
        );
        assert!(
            !password_matches_async(hash.into(), "wrong".into())
                .await
                .unwrap()
        );
        assert!(
            !password_matches_async("invalid-hash".into(), "wrong".into())
                .await
                .unwrap()
        );
    }
}
