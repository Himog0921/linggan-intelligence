//! OPS-LAUNCH-001 S01 foundation: durable, session-bound Operations sync commands.
//!
//! This module deliberately does not manufacture a Collection request or Work Order.  An item
//! can be `accepted` only after the later exact-publication bridge has actually written those
//! existing-chain references.  Until then, a well-formed XHS registration is truthfully
//! `blocked`, rather than being reported as queued or synchronized.
use super::*;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/ops/publication-syncs", post(create))
        .route("/api/local/ops/publication-syncs/{request_ref}", get(read))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateSyncBody {
    idempotency_key: Uuid,
    publication_refs: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncReceipt {
    request_ref: Uuid,
    selected_count: usize,
    replayed: bool,
    items: Vec<SyncItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncItem {
    publication_ref: Uuid,
    decision: String,
    code: String,
    acquisition_request_ref: Option<Uuid>,
    work_order_ref: Option<Uuid>,
}

enum SyncError {
    Invalid(&'static str),
    Conflict,
    Unavailable,
}

async fn create(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(body): Json<CreateSyncBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let response = match create_in_database(database, &member, body).await {
        Ok((status, receipt)) => (status, Json(receipt)).into_response(),
        Err(error) => sync_error(error),
    };
    secure(response)
}

async fn read(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(request_ref): Path<Uuid>,
) -> Response {
    if let Err(response) = super::ops_auth::current_member(&state, &headers).await {
        return response;
    }
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let receipt = match read_receipt(database.pool(), request_ref, false).await {
        Ok(Some(receipt)) => receipt,
        Ok(None) => {
            return secure(
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"code":"ops_sync_not_found"})),
                )
                    .into_response(),
            );
        }
        Err(()) => return sync_error(SyncError::Unavailable),
    };
    secure((StatusCode::OK, Json(receipt)).into_response())
}

async fn create_in_database(
    database: &linggan_storage_postgres::Database,
    member: &super::ops_auth::AuthenticatedMember,
    body: CreateSyncBody,
) -> Result<(StatusCode, SyncReceipt), SyncError> {
    let refs = normalized_refs(body.publication_refs)?;
    let payload = payload_digest(&refs);
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| SyncError::Unavailable)?;
    lock_request(&mut transaction, body.idempotency_key).await?;
    if let Some((actor, stored_payload)) = sqlx::query_as::<_, (Uuid, Vec<u8>)>(
        "SELECT actor_member_ref,payload_sha256 FROM linggan_ops_publication_sync_request WHERE request_ref=$1",
    )
    .bind(body.idempotency_key)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| SyncError::Unavailable)?
    {
        if actor != member.member_ref || stored_payload != payload {
            return Err(SyncError::Conflict);
        }
        let receipt = read_receipt_in(&mut transaction, body.idempotency_key, true).await?;
        transaction.commit().await.map_err(|_| SyncError::Unavailable)?;
        return Ok((StatusCode::OK, receipt.expect("existing request has receipt rows")));
    }

    sqlx::query(
        "INSERT INTO linggan_ops_publication_sync_request \
         (request_ref,actor_member_ref,payload_sha256,selected_count,purpose) VALUES($1,$2,$3,$4,'ops_publication_sync')",
    )
    .bind(body.idempotency_key)
    .bind(member.member_ref)
    .bind(payload.as_slice())
    .bind(i32::try_from(refs.len()).map_err(|_| SyncError::Invalid("ops_sync_selection_invalid"))?)
    .execute(&mut *transaction)
    .await
    .map_err(|_| SyncError::Unavailable)?;

    for (index, publication_ref) in refs.iter().copied().enumerate() {
        let frozen = publication_snapshot(&mut transaction, publication_ref).await?;
        let decision = decide(&mut transaction, member, frozen).await?;
        sqlx::query(
            "INSERT INTO linggan_ops_publication_sync_item \
             (sync_item_ref,request_ref,ordinal,requested_publication_ref,publication_ref,account_ref,domain_ref, \
              platform,expected_content_id,expected_author_id,publication_version,locator_sha256,decision,decision_code) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
        )
        .bind(Uuid::new_v4())
        .bind(body.idempotency_key)
        .bind(i32::try_from(index + 1).map_err(|_| SyncError::Invalid("ops_sync_selection_invalid"))?)
        .bind(publication_ref)
        .bind(decision.publication_ref)
        .bind(decision.account_ref)
        .bind(decision.domain_ref)
        .bind(decision.platform)
        .bind(decision.content_id)
        .bind(decision.author_id)
        .bind(decision.publication_version)
        .bind(decision.locator_sha256)
        .bind(decision.decision)
        .bind(decision.code)
        .execute(&mut *transaction)
        .await
        .map_err(|_| SyncError::Unavailable)?;
    }
    let receipt = read_receipt_in(&mut transaction, body.idempotency_key, false)
        .await?
        .expect("new request has receipt rows");
    transaction
        .commit()
        .await
        .map_err(|_| SyncError::Unavailable)?;
    Ok((StatusCode::CREATED, receipt))
}

fn normalized_refs(mut refs: Vec<Uuid>) -> Result<Vec<Uuid>, SyncError> {
    if refs.is_empty() || refs.len() > 200 {
        return Err(SyncError::Invalid("ops_sync_selection_invalid"));
    }
    refs.sort_unstable();
    if refs.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(SyncError::Invalid("ops_sync_selection_duplicate"));
    }
    Ok(refs)
}

fn payload_digest(refs: &[Uuid]) -> [u8; 32] {
    let json = serde_json::to_vec(&json!({"publicationRefs": refs})).expect("UUID JSON serializes");
    Sha256::digest(json).into()
}

async fn lock_request(
    transaction: &mut Transaction<'_, Postgres>,
    request_ref: Uuid,
) -> Result<(), SyncError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(request_ref.to_string())
        .execute(&mut **transaction)
        .await
        .map_err(|_| SyncError::Unavailable)?;
    Ok(())
}

struct FrozenPublication {
    publication_ref: Uuid,
    account_ref: Option<Uuid>,
    domain_ref: Option<Uuid>,
    platform: String,
    content_id: Option<String>,
    content_id_state: String,
    author_id: Option<String>,
    publication_version: Option<i64>,
    locator_sha256: Option<Vec<u8>>,
    execution_locator_ready: bool,
    active: bool,
}

async fn publication_snapshot(
    transaction: &mut Transaction<'_, Postgres>,
    publication_ref: Uuid,
) -> Result<Option<FrozenPublication>, SyncError> {
    let row: Option<(
        Uuid,
        Option<Uuid>,
        Option<Uuid>,
        String,
        Option<String>,
        String,
        Option<String>,
        Option<i64>,
        Option<Vec<u8>>,
        bool,
        bool,
    )> = sqlx::query_as(
        "SELECT publication.publication_ref,publication.account_ref, \
                COALESCE(publication.native_business_domain_ref,account.domain_ref) AS domain_ref, \
                publication.platform,COALESCE(locator.expected_content_id,publication.platform_content_id) AS content_id, \
                CASE WHEN locator.locator_ref IS NULL THEN publication.content_id_state ELSE 'operator_execution_locator' END AS content_id_state, \
                COALESCE(locator.expected_author_id,account.platform_user_id) AS author_id,publication.native_version AS publication_version, \
                locator.locator_sha256,locator.locator_ref IS NOT NULL AS execution_locator_ready, \
                CASE WHEN publication.source_system='linggan_native' \
                     THEN publication.native_record_state='active' \
                     ELSE publication.legacy_effective_publication END AS active \
         FROM linggan_ops_publication publication \
         LEFT JOIN linggan_ops_account account ON account.account_ref=publication.account_ref \
         LEFT JOIN LATERAL ( \
             SELECT source.locator_ref,source.expected_content_id,source.expected_author_id,source.locator_sha256 \
               FROM linggan_ops_publication_execution_locator source \
              WHERE source.publication_ref=publication.publication_ref \
                AND source.publication_version=publication.native_version \
              ORDER BY source.provided_at DESC,source.locator_ref DESC LIMIT 1 \
         ) locator ON publication.source_system='linggan_native' \
         WHERE publication.publication_ref=$1 FOR SHARE OF publication",
    )
    .bind(publication_ref)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| SyncError::Unavailable)?;
    Ok(row.map(
        |(
            publication_ref,
            account_ref,
            domain_ref,
            platform,
            content_id,
            content_id_state,
            author_id,
            publication_version,
            locator_sha256,
            execution_locator_ready,
            active,
        )| FrozenPublication {
            publication_ref,
            account_ref,
            domain_ref,
            platform,
            content_id,
            content_id_state,
            author_id,
            publication_version,
            locator_sha256,
            execution_locator_ready,
            active,
        },
    ))
}

struct Decision {
    publication_ref: Option<Uuid>,
    account_ref: Option<Uuid>,
    domain_ref: Option<Uuid>,
    platform: Option<String>,
    content_id: Option<String>,
    author_id: Option<String>,
    publication_version: Option<i64>,
    locator_sha256: Option<Vec<u8>>,
    decision: &'static str,
    code: &'static str,
}

async fn decide(
    transaction: &mut Transaction<'_, Postgres>,
    member: &super::ops_auth::AuthenticatedMember,
    frozen: Option<FrozenPublication>,
) -> Result<Decision, SyncError> {
    let Some(frozen) = frozen else {
        return Ok(Decision::invalid("ops_sync_publication_not_found"));
    };
    let base = Decision {
        publication_ref: Some(frozen.publication_ref),
        account_ref: frozen.account_ref,
        domain_ref: frozen.domain_ref,
        platform: Some(frozen.platform.clone()),
        content_id: frozen.content_id.clone(),
        author_id: frozen.author_id.clone(),
        publication_version: frozen.publication_version,
        locator_sha256: frozen.locator_sha256.clone(),
        decision: "blocked",
        code: "ops_sync_execution_bridge_not_ready",
    };
    let Some(account_ref) = frozen.account_ref else {
        return Ok(Decision {
            decision: "invalid",
            code: "ops_sync_account_unassigned",
            ..base
        });
    };
    if !frozen.active {
        return Ok(Decision {
            decision: "invalid",
            code: "ops_sync_publication_inactive",
            ..base
        });
    }
    if !can_write_account(transaction, member, account_ref).await? {
        return Ok(Decision {
            decision: "forbidden",
            code: "ops_sync_forbidden",
            ..base
        });
    }
    if frozen.domain_ref.is_none() {
        return Ok(Decision {
            code: "ops_sync_domain_unassigned",
            ..base
        });
    }
    if frozen.platform == "douyin" {
        return Ok(Decision {
            code: "ops_sync_douyin_execution_not_ready",
            ..base
        });
    }
    if frozen.content_id_state != "parsed_publication_url"
        && frozen.content_id_state != "parsed_canonical_url"
        && frozen.content_id_state != "operator_execution_locator"
    {
        return Ok(Decision {
            code: "ops_sync_content_identity_unresolved",
            ..base
        });
    }
    if frozen.author_id.is_none() {
        return Ok(Decision {
            code: "ops_sync_author_identity_unresolved",
            ..base
        });
    }
    if !frozen.execution_locator_ready {
        return Ok(Decision {
            code: "ops_sync_execution_locator_unavailable",
            ..base
        });
    }
    if frozen.publication_version.is_none() {
        return Ok(Decision {
            code: "ops_sync_publication_version_unavailable",
            ..base
        });
    }
    Ok(base)
}

impl Decision {
    fn invalid(code: &'static str) -> Self {
        Self {
            publication_ref: None,
            account_ref: None,
            domain_ref: None,
            platform: None,
            content_id: None,
            author_id: None,
            publication_version: None,
            locator_sha256: None,
            decision: "invalid",
            code,
        }
    }
}

async fn can_write_account(
    transaction: &mut Transaction<'_, Postgres>,
    member: &super::ops_auth::AuthenticatedMember,
    account_ref: Uuid,
) -> Result<bool, SyncError> {
    if member.role == "admin" {
        return Ok(true);
    }
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT current_responsible_member_ref FROM linggan_ops_account WHERE account_ref=$1 FOR SHARE",
    )
    .bind(account_ref)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| SyncError::Unavailable)?
    .flatten();
    let Some(owner) = owner else { return Ok(false) };
    if owner == member.member_ref {
        return Ok(true);
    }
    if member.role != "lead" {
        return Ok(false);
    }
    sqlx::query_scalar(
        "WITH latest AS (SELECT DISTINCT ON (member_ref) member_ref,group_ref,action \
                         FROM linggan_ops_group_membership_event WHERE member_ref IN ($1,$2) \
                         ORDER BY member_ref,occurred_at DESC,event_ref DESC) \
         SELECT EXISTS (SELECT 1 FROM latest actor JOIN latest owner ON actor.group_ref=owner.group_ref \
                        JOIN linggan_ops_group group_row ON group_row.group_ref=actor.group_ref \
                        WHERE actor.member_ref=$1 AND owner.member_ref=$2 AND actor.action='joined' \
                          AND owner.action='joined' AND group_row.state='active')",
    )
    .bind(member.member_ref)
    .bind(owner)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| SyncError::Unavailable)
}

async fn read_receipt(
    pool: &sqlx::PgPool,
    request_ref: Uuid,
    replayed: bool,
) -> Result<Option<SyncReceipt>, ()> {
    let mut transaction = pool.begin().await.map_err(|_| ())?;
    let receipt = read_receipt_in(&mut transaction, request_ref, replayed)
        .await
        .map_err(|_| ())?;
    transaction.commit().await.map_err(|_| ())?;
    Ok(receipt)
}

async fn read_receipt_in(
    transaction: &mut Transaction<'_, Postgres>,
    request_ref: Uuid,
    replayed: bool,
) -> Result<Option<SyncReceipt>, SyncError> {
    let selected_count: Option<i32> = sqlx::query_scalar(
        "SELECT selected_count FROM linggan_ops_publication_sync_request WHERE request_ref=$1",
    )
    .bind(request_ref)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| SyncError::Unavailable)?;
    let Some(selected_count) = selected_count else {
        return Ok(None);
    };
    let rows: Vec<(Uuid, String, String, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT requested_publication_ref,decision,decision_code,acquisition_request_ref,work_order_ref \
         FROM linggan_ops_publication_sync_item WHERE request_ref=$1 ORDER BY ordinal",
    )
    .bind(request_ref)
    .fetch_all(&mut **transaction)
    .await
    .map_err(|_| SyncError::Unavailable)?;
    Ok(Some(SyncReceipt {
        request_ref,
        selected_count: usize::try_from(selected_count).map_err(|_| SyncError::Unavailable)?,
        replayed,
        items: rows
            .into_iter()
            .map(
                |(publication_ref, decision, code, acquisition_request_ref, work_order_ref)| {
                    SyncItem {
                        publication_ref,
                        decision,
                        code,
                        acquisition_request_ref,
                        work_order_ref,
                    }
                },
            )
            .collect(),
    }))
}

fn secure(mut response: Response) -> Response {
    super::ops_auth::secure_headers(&mut response);
    response
}

fn sync_error(error: SyncError) -> Response {
    let (status, code) = match error {
        SyncError::Invalid(code) => (StatusCode::UNPROCESSABLE_ENTITY, code),
        SyncError::Conflict => (StatusCode::CONFLICT, "ops_sync_request_conflict"),
        SyncError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "ops_sync_unavailable"),
    };
    secure((status, Json(json!({"code": code}))).into_response())
}
