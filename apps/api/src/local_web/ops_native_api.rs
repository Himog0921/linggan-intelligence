//! OPS-NATIVE-HTTP-001: authenticated command boundary for Linggan-owned Operations facts.
//! Browser input never chooses an actor, role, or responsibility relation; those all come from
//! the current active session and the transaction's current rows.
use super::*;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use linggan_intelligence::ops_ledger::xhs_note_id;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/ops/native/accounts",
            axum::routing::post(create_account),
        )
        .route(
            "/api/local/ops/native/accounts/{account_ref}",
            axum::routing::post(update_account),
        )
        .route(
            "/api/local/ops/native/accounts/{account_ref}/archive",
            axum::routing::post(archive_account),
        )
        .route(
            "/api/local/ops/native/accounts/{account_ref}/restore",
            axum::routing::post(restore_account),
        )
        .route(
            "/api/local/ops/native/publications",
            axum::routing::post(create_publication),
        )
        .route(
            "/api/local/ops/native/publications/batch",
            axum::routing::post(create_publications_batch),
        )
        .route(
            "/api/local/ops/native/accounts/{account_ref}/responsibility",
            axum::routing::post(transfer_account_responsibility),
        )
        .route(
            "/api/local/ops/native/publications/{publication_ref}/view-reports",
            axum::routing::post(record_view_report),
        )
        .route(
            "/api/local/ops/native/view-reports/batch",
            axum::routing::post(record_view_reports_batch),
        )
        .route(
            "/api/local/ops/native/publications/{publication_ref}/execution-locators",
            axum::routing::post(record_execution_locator),
        )
        .route(
            "/api/local/ops/native/daily-reports",
            axum::routing::post(record_daily_report),
        )
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateAccountBody {
    request_id: Uuid,
    domain_ref: Uuid,
    platform: String,
    account_name: String,
    profile_url: Option<String>,
    platform_user_id: Option<String>,
    handle: Option<String>,
    status: Option<String>,
    is_retained: Option<bool>,
    follower_count: Option<i64>,
    device_code: Option<String>,
    ip_location: Option<String>,
    notes: Option<String>,
}

/// Ordinary account maintenance deliberately cannot change platform or platform identity.
/// Those fields bind a stable external account and need their own administrator-only correction
/// workflow rather than becoming an accidental side effect of editing a display name.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateAccountBody {
    request_id: Uuid,
    expected_version: i64,
    account_name: String,
    profile_url: Option<String>,
    handle: Option<String>,
    status: String,
    is_retained: bool,
    follower_count: Option<i64>,
    device_code: Option<String>,
    ip_location: Option<String>,
    notes: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountLifecycleBody {
    request_id: Uuid,
    expected_version: i64,
    reason: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatePublicationBody {
    request_id: Uuid,
    account_ref: Uuid,
    publication_url: String,
    canonical_url: Option<String>,
    published_at: Option<String>,
    published_on: Option<String>,
    title: Option<String>,
    body: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatePublicationsBatchBody {
    publications: Vec<CreatePublicationBody>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPublicationReceipt {
    ordinal: usize,
    status: u16,
    receipt: CommandReceipt,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPublicationsReceipt {
    items: Vec<BatchPublicationReceipt>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TransferAccountResponsibilityBody {
    request_id: Uuid,
    next_member_ref: Option<Uuid>,
    expected_version: i64,
    reason: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordViewBody {
    request_id: Uuid,
    report_kind: String,
    view_count: Option<i64>,
    as_of: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchViewReportItem {
    publication_ref: Uuid,
    #[serde(flatten)]
    report: RecordViewBody,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BatchViewReportsBody {
    reports: Vec<BatchViewReportItem>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordExecutionLocatorBody {
    request_id: Uuid,
    signed_locator: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordDailyBody {
    request_id: Uuid,
    daily_ref: Option<Uuid>,
    business_date: String,
    links_opened_count: Option<i32>,
    followers_gained: Option<i32>,
    expected_version: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandReceipt {
    request_ref: Uuid,
    outcome: String,
    code: String,
    record_ref: Option<Uuid>,
    resulting_version: Option<i64>,
    replayed: bool,
}

enum WriteError {
    Invalid(&'static str),
    Unavailable,
}

async fn create_account(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(body): Json<CreateAccountBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_account(database, member, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn update_account(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(account_ref): Path<Uuid>,
    Json(body): Json<UpdateAccountBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_account_update(database, member, account_ref, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn archive_account(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(account_ref): Path<Uuid>,
    Json(body): Json<AccountLifecycleBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_account_lifecycle(database, member, account_ref, body, true).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn restore_account(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(account_ref): Path<Uuid>,
    Json(body): Json<AccountLifecycleBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_account_lifecycle(database, member, account_ref, body, false).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn create_publication(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(body): Json<CreatePublicationBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_publication(database, member, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

/// Each row keeps its own command identity and transaction, so a malformed or unauthorized
/// row cannot roll back accepted siblings or make retrying the failed input duplicate them.
async fn create_publications_batch(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(body): Json<CreatePublicationsBatchBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    if body.publications.is_empty() || body.publications.len() > 100 {
        return write_error(WriteError::Invalid("ops_native_batch_size_invalid"));
    }
    let mut items = Vec::with_capacity(body.publications.len());
    for (index, publication) in body.publications.into_iter().enumerate() {
        let request_ref = publication.request_id;
        let (status, receipt) = match write_publication(database, member.clone(), publication).await
        {
            Ok(result) => result,
            Err(error) => match error {
                WriteError::Invalid(code) => (
                    if code.ends_with("_conflict") {
                        StatusCode::CONFLICT
                    } else {
                        StatusCode::UNPROCESSABLE_ENTITY
                    },
                    CommandReceipt {
                        request_ref,
                        outcome: "rejected".to_owned(),
                        code: code.to_owned(),
                        record_ref: None,
                        resulting_version: None,
                        replayed: false,
                    },
                ),
                WriteError::Unavailable => return write_error(WriteError::Unavailable),
            },
        };
        items.push(BatchPublicationReceipt {
            ordinal: index + 1,
            status: status.as_u16(),
            receipt,
        });
    }
    let mut response = Json(BatchPublicationsReceipt { items }).into_response();
    super::ops_auth::secure_headers(&mut response);
    response
}

async fn transfer_account_responsibility(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(account_ref): Path<Uuid>,
    Json(body): Json<TransferAccountResponsibilityBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_account_responsibility(database, member, account_ref, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn record_view_report(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(publication_ref): Path<Uuid>,
    Json(body): Json<RecordViewBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_view_report(database, member, publication_ref, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn record_view_reports_batch(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(body): Json<BatchViewReportsBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    if body.reports.is_empty() || body.reports.len() > 100 {
        return write_error(WriteError::Invalid("ops_native_batch_size_invalid"));
    }
    let mut items = Vec::with_capacity(body.reports.len());
    for (index, item) in body.reports.into_iter().enumerate() {
        let request_ref = item.report.request_id;
        let (status, receipt) =
            match write_view_report(database, member.clone(), item.publication_ref, item.report)
                .await
            {
                Ok(result) => result,
                Err(WriteError::Invalid(code)) => (
                    if code.ends_with("_conflict") {
                        StatusCode::CONFLICT
                    } else {
                        StatusCode::UNPROCESSABLE_ENTITY
                    },
                    CommandReceipt {
                        request_ref,
                        outcome: "rejected".to_owned(),
                        code: code.to_owned(),
                        record_ref: None,
                        resulting_version: None,
                        replayed: false,
                    },
                ),
                Err(WriteError::Unavailable) => return write_error(WriteError::Unavailable),
            };
        items.push(BatchPublicationReceipt {
            ordinal: index + 1,
            status: status.as_u16(),
            receipt,
        });
    }
    let mut response = Json(BatchPublicationsReceipt { items }).into_response();
    super::ops_auth::secure_headers(&mut response);
    response
}

async fn record_execution_locator(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(publication_ref): Path<Uuid>,
    Json(body): Json<RecordExecutionLocatorBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_execution_locator(database, member, publication_ref, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn record_daily_report(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Json(body): Json<RecordDailyBody>,
) -> Response {
    let member = match super::ops_auth::current_member(&state, &headers).await {
        Ok(member) => member,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    match write_daily_report(database, member, body).await {
        Ok((status, receipt)) => receipt_response(status, receipt),
        Err(error) => write_error(error),
    }
}

async fn write_account(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    body: CreateAccountBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let raw_payload = raw_payload(&body)?;
    let request_id = body.request_id;
    let account = match PreparedAccount::parse(body) {
        Ok(account) => account,
        Err(WriteError::Invalid(code)) => {
            return reject_invalid_input(
                database,
                &member,
                request_id,
                "account_create",
                &raw_payload,
                code,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let payload = digest(json!({
        "domainRef": account.domain_ref,
        "platform": account.platform,
        "accountName": account.account_name,
        "profileUrl": account.profile_url,
        "platformUserId": account.platform_user_id,
        "handle": account.handle,
        "status": account.status,
        "isRetained": account.is_retained,
        "followerCount": account.follower_count,
        "deviceCode": account.device_code,
        "ipLocation": account.ip_location,
        "notes": account.notes,
    }));
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        account.request_id,
        "account_create",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let known_domain: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active')",
    )
    .bind(account.domain_ref)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    if !known_domain {
        return rejected(
            transaction,
            account.request_id,
            "account_create",
            &member,
            &payload,
            None,
            None,
            "ops_native_domain_unknown",
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    if let Some(identity) = account.platform_user_id.as_deref() {
        lock_platform_identity(&mut transaction, &account.platform, identity).await?;
        if identity_claimed(&mut transaction, &account.platform, identity).await? {
            return rejected(
                transaction,
                account.request_id,
                "account_create",
                &member,
                &payload,
                None,
                None,
                "ops_native_platform_identity_conflict",
                StatusCode::CONFLICT,
            )
            .await;
        }
    }
    let account_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_account \
         (account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name,handle,\
          status,is_retained,operator_name,follower_count,profile_url,profile_link_state,platform_user_id,\
          device_code,ip_location,notes,review_status,current_responsible_member_ref,native_record_state,\
          native_version,native_created_by_member_ref,native_updated_by_member_ref) \
         VALUES($1,'linggan_native',$1::text,$2,'native_active',$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,\
          'confirmed',$16,'active',1,$16,$16)",
    )
    .bind(account_ref)
    .bind(account.domain_ref)
    .bind(&account.platform)
    .bind(&account.account_name)
    .bind(&account.handle)
    .bind(&account.status)
    .bind(account.is_retained)
    .bind(&member.display_name)
    .bind(account.follower_count)
    .bind(&account.profile_url)
    .bind(account.profile_state())
    .bind(&account.platform_user_id)
    .bind(&account.device_code)
    .bind(&account.ip_location)
    .bind(&account.notes)
    .bind(member.member_ref)
    .execute(&mut *transaction)
    .await
    .map_err(|error| {
        eprintln!(
            "ops native account create unavailable: database_kind={:?}",
            error.as_database_error().and_then(|value| value.code())
        );
        WriteError::Unavailable
    })?;
    let receipt = accepted(
        account.request_id,
        "account_create",
        &member,
        &payload,
        account_ref,
        1,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::CREATED, receipt.receipt))
}

async fn write_account_update(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    account_ref: Uuid,
    body: UpdateAccountBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let raw_payload = raw_payload(&json!({ "accountRef": account_ref, "body": body }))?;
    let request_id = body.request_id;
    let account = match PreparedAccountUpdate::parse(body) {
        Ok(account) => account,
        Err(WriteError::Invalid(code)) => {
            return reject_invalid_input(
                database,
                &member,
                request_id,
                "account_update",
                &raw_payload,
                code,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let payload = digest(
        json!({ "accountRef": account_ref, "expectedVersion": account.expected_version,
        "accountName": account.account_name, "profileUrl": account.profile_url, "handle": account.handle,
        "status": account.status, "isRetained": account.is_retained, "followerCount": account.follower_count,
        "deviceCode": account.device_code, "ipLocation": account.ip_location, "notes": account.notes }),
    );
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        account.request_id,
        "account_update",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let current: Option<(String, Option<i64>, Option<String>, String, Option<String>, Option<String>, String, bool, Option<i64>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT source_system,native_version,native_record_state,account_name,profile_url,handle,status,is_retained,follower_count,device_code,ip_location,notes \
         FROM linggan_ops_account WHERE account_ref=$1 FOR UPDATE",
    )
    .bind(account_ref)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let Some((
        source_system,
        native_version,
        native_state,
        account_name,
        profile_url,
        handle,
        status,
        is_retained,
        follower_count,
        device_code,
        ip_location,
        notes,
    )) = current
    else {
        return rejected(
            transaction,
            account.request_id,
            "account_update",
            &member,
            &payload,
            Some(account_ref),
            None,
            "ops_native_account_not_found",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    let Some(current_version) = native_version.filter(|_| source_system == "linggan_native") else {
        return rejected(
            transaction,
            account.request_id,
            "account_update",
            &member,
            &payload,
            Some(account_ref),
            None,
            "ops_native_account_not_editable",
            StatusCode::FORBIDDEN,
        )
        .await;
    };
    if native_state.as_deref() != Some("active") {
        return rejected(
            transaction,
            account.request_id,
            "account_update",
            &member,
            &payload,
            Some(account_ref),
            Some(current_version),
            "ops_native_account_inactive",
            StatusCode::CONFLICT,
        )
        .await;
    }
    if !can_write_account(&mut transaction, &member, account_ref).await? {
        return rejected(
            transaction,
            account.request_id,
            "account_update",
            &member,
            &payload,
            Some(account_ref),
            Some(current_version),
            "ops_native_forbidden",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    if account.expected_version != current_version {
        return stale_for(
            transaction,
            account.request_id,
            "account_update",
            &member,
            &payload,
            account_ref,
            current_version,
            "ops_native_version_stale",
        )
        .await;
    }
    if (
        account_name,
        profile_url,
        handle,
        status,
        is_retained,
        follower_count,
        device_code,
        ip_location,
        notes,
    ) == (
        account.account_name.clone(),
        account.profile_url.clone(),
        account.handle.clone(),
        account.status.clone(),
        account.is_retained,
        account.follower_count,
        account.device_code.clone(),
        account.ip_location.clone(),
        account.notes.clone(),
    ) {
        let receipt = unchanged(
            account.request_id,
            "account_update",
            &member,
            &payload,
            account_ref,
            current_version,
            "ops_native_unchanged",
        );
        insert_receipt(&mut transaction, &receipt).await?;
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((StatusCode::OK, receipt.receipt));
    }
    let next_version = current_version + 1;
    sqlx::query(
        "UPDATE linggan_ops_account SET account_name=$2,profile_url=$3,handle=$4,status=$5,is_retained=$6, \
         follower_count=$7,device_code=$8,ip_location=$9,notes=$10, \
         profile_link_state=CASE WHEN platform_user_id IS NOT NULL THEN 'parsed' \
              WHEN $3 IS NOT NULL THEN 'pending_match' ELSE 'missing' END, \
         native_version=$11,native_updated_by_member_ref=$12 \
         WHERE account_ref=$1",
    )
    .bind(account_ref).bind(&account.account_name).bind(&account.profile_url).bind(&account.handle).bind(&account.status)
    .bind(account.is_retained).bind(account.follower_count).bind(&account.device_code).bind(&account.ip_location)
    .bind(&account.notes).bind(next_version).bind(member.member_ref)
    .execute(&mut *transaction).await.map_err(|_| WriteError::Unavailable)?;
    let receipt = accepted(
        account.request_id,
        "account_update",
        &member,
        &payload,
        account_ref,
        next_version,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::OK, receipt.receipt))
}

async fn write_account_lifecycle(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    account_ref: Uuid,
    body: AccountLifecycleBody,
    archive: bool,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let command_kind = if archive {
        "account_archive"
    } else {
        "account_restore"
    };
    let raw_payload = raw_payload(&json!({ "accountRef": account_ref, "body": body }))?;
    let request_id = body.request_id;
    let reason = body.reason.trim().to_owned();
    if body.expected_version < 1 || reason.is_empty() || reason.chars().count() > 1_000 {
        return reject_invalid_input(
            database,
            &member,
            request_id,
            command_kind,
            &raw_payload,
            "ops_native_account_lifecycle_invalid",
        )
        .await;
    }
    let payload = digest(
        json!({ "accountRef": account_ref, "expectedVersion": body.expected_version, "reason": reason }),
    );
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        request_id,
        command_kind,
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let current: Option<(String, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT source_system,native_version,native_record_state FROM linggan_ops_account WHERE account_ref=$1 FOR UPDATE",
    ).bind(account_ref).fetch_optional(&mut *transaction).await.map_err(|_| WriteError::Unavailable)?;
    let Some((source_system, native_version, state)) = current else {
        return rejected(
            transaction,
            request_id,
            command_kind,
            &member,
            &payload,
            Some(account_ref),
            None,
            "ops_native_account_not_found",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    let Some(current_version) = native_version.filter(|_| source_system == "linggan_native") else {
        return rejected(
            transaction,
            request_id,
            command_kind,
            &member,
            &payload,
            Some(account_ref),
            None,
            "ops_native_account_not_editable",
            StatusCode::FORBIDDEN,
        )
        .await;
    };
    if !can_write_account(&mut transaction, &member, account_ref).await? {
        return rejected(
            transaction,
            request_id,
            command_kind,
            &member,
            &payload,
            Some(account_ref),
            Some(current_version),
            "ops_native_forbidden",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    if body.expected_version != current_version {
        return stale_for(
            transaction,
            request_id,
            command_kind,
            &member,
            &payload,
            account_ref,
            current_version,
            "ops_native_version_stale",
        )
        .await;
    }
    let wanted = if archive { "archived" } else { "active" };
    if state.as_deref() == Some(wanted) {
        let receipt = unchanged(
            request_id,
            command_kind,
            &member,
            &payload,
            account_ref,
            current_version,
            "ops_native_unchanged",
        );
        insert_receipt(&mut transaction, &receipt).await?;
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((StatusCode::OK, receipt.receipt));
    }
    let next_version = current_version + 1;
    if archive {
        sqlx::query("UPDATE linggan_ops_account SET native_record_state='archived',ledger_state='native_archived', \
                     native_archived_at=scope_001_now(),native_archive_reason=$2,native_version=$3,native_updated_by_member_ref=$4 \
                     WHERE account_ref=$1")
            .bind(account_ref).bind(&reason).bind(next_version).bind(member.member_ref).execute(&mut *transaction).await
            .map_err(|_| WriteError::Unavailable)?;
    } else {
        sqlx::query("UPDATE linggan_ops_account SET native_record_state='active',ledger_state='native_active', \
                     native_archived_at=NULL,native_archive_reason=NULL,native_version=$2,native_updated_by_member_ref=$3 \
                     WHERE account_ref=$1")
            .bind(account_ref).bind(next_version).bind(member.member_ref).execute(&mut *transaction).await
            .map_err(|_| WriteError::Unavailable)?;
    }
    let receipt = accepted(
        request_id,
        command_kind,
        &member,
        &payload,
        account_ref,
        next_version,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::OK, receipt.receipt))
}

async fn write_publication(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    body: CreatePublicationBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let raw_payload = raw_payload(&body)?;
    let request_id = body.request_id;
    let publication = match PreparedPublication::parse(body) {
        Ok(publication) => publication,
        Err(WriteError::Invalid(code)) => {
            return reject_invalid_input(
                database,
                &member,
                request_id,
                "publication_create",
                &raw_payload,
                code,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let payload = digest(publication.payload());
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        publication.request_id,
        "publication_create",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let account = account_for_publication(&mut transaction, publication.account_ref).await?;
    let Some((platform, domain_ref, active)) = account else {
        return rejected(
            transaction,
            publication.request_id,
            "publication_create",
            &member,
            &payload,
            Some(publication.account_ref),
            None,
            "ops_native_account_not_found",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    if !active {
        return rejected(
            transaction,
            publication.request_id,
            "publication_create",
            &member,
            &payload,
            Some(publication.account_ref),
            None,
            "ops_native_account_inactive",
            StatusCode::CONFLICT,
        )
        .await;
    }
    if !can_write_account(&mut transaction, &member, publication.account_ref).await? {
        return rejected(
            transaction,
            publication.request_id,
            "publication_create",
            &member,
            &payload,
            Some(publication.account_ref),
            None,
            "ops_native_forbidden",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    let publication_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_publication \
         (publication_ref,source_system,source_record_id,account_ref,platform,publication_url,canonical_url,\
          content_id_state,published_at,published_date_state,title,body,entry_source,review_status,\
          native_record_state,native_version,native_publisher_member_ref,native_registered_by_member_ref,\
          native_business_domain_ref,native_published_on,native_published_precision) \
         VALUES($1,'linggan_native',$1::text,$2,$3,$4,$5,'pending_match',\
          CASE WHEN $6='instant' THEN $7::timestamptz ELSE NULL END,'as_recorded',$8,$9,\
          'linggan_ops_manual','confirmed','active',1,$10,$10,$11,\
          CASE WHEN $6='instant' THEN ($7::timestamptz AT TIME ZONE 'Asia/Shanghai')::date ELSE $12::date END,$6)",
    )
    .bind(publication_ref)
    .bind(publication.account_ref)
    .bind(platform)
    .bind(&publication.publication_url)
    .bind(&publication.canonical_url)
    .bind(&publication.precision)
    .bind(&publication.published_at)
    .bind(&publication.title)
    .bind(&publication.body)
    .bind(member.member_ref)
    .bind(domain_ref)
    .bind(&publication.published_on)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let receipt = accepted(
        publication.request_id,
        "publication_create",
        &member,
        &payload,
        publication_ref,
        1,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    insert_native_publication_field_adoptions(
        &mut transaction,
        publication_ref,
        publication.request_id,
        &publication.title,
        &publication.body,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::CREATED, receipt.receipt))
}

/// A native registration's explicitly supplied text enters the same current-field model as an
/// imported baseline or a later receipt. Empty inputs deliberately create no manual override, so
/// a qualified detail observation may supply that still-unknown field.
async fn insert_native_publication_field_adoptions(
    transaction: &mut Transaction<'_, Postgres>,
    publication_ref: Uuid,
    request_ref: Uuid,
    title: &Option<String>,
    body: &Option<String>,
) -> Result<(), WriteError> {
    sqlx::query(
        "INSERT INTO linggan_ops_publication_field_adoption \
         (adoption_ref,publication_ref,publication_version,field_name,disposition,source_kind, \
          source_request_ref,value_text,observed_at) \
         SELECT gen_random_uuid(),$1,1,candidate.field_name,'adopted','native_registration',$2, \
                candidate.value_text,scope_001_now()::text \
           FROM (VALUES ('title'::text,$3::text),('body'::text,$4::text)) \
                AS candidate(field_name,value_text) \
          WHERE candidate.value_text IS NOT NULL \
         ON CONFLICT (source_request_ref,publication_ref,field_name) DO NOTHING",
    )
    .bind(publication_ref)
    .bind(request_ref)
    .bind(title)
    .bind(body)
    .execute(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    Ok(())
}

/// Transfer the current operational owner without touching imported attribution or historical
/// publication ownership.  A lead may move an account only within the group they currently
/// manage; only an administrator may leave it explicitly unassigned for later takeover.
async fn write_account_responsibility(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    account_ref: Uuid,
    body: TransferAccountResponsibilityBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let raw_payload = raw_payload(&json!({ "accountRef": account_ref, "body": body }))?;
    let request_id = body.request_id;
    let reason = body.reason.trim().to_owned();
    if body.expected_version < 1 || reason.is_empty() || reason.len() > 1000 {
        return reject_invalid_input(
            database,
            &member,
            request_id,
            "account_responsibility_transfer",
            &raw_payload,
            "ops_native_responsibility_invalid",
        )
        .await;
    }
    let payload = digest(json!({
        "accountRef": account_ref,
        "nextMemberRef": body.next_member_ref,
        "expectedVersion": body.expected_version,
        "reason": reason,
    }));
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        request_id,
        "account_responsibility_transfer",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let account: Option<(Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT current_responsible_member_ref,responsibility_version \
         FROM linggan_ops_account \
         WHERE account_ref=$1 AND ((source_system='linggan_native' AND native_record_state='active') \
              OR (source_system='content_workbench' AND ledger_state='in_ledger')) \
         FOR UPDATE",
    )
    .bind(account_ref)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let Some((previous_member_ref, current_version)) = account else {
        return rejected(
            transaction,
            request_id,
            "account_responsibility_transfer",
            &member,
            &payload,
            Some(account_ref),
            None,
            "ops_native_account_not_found",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    if body.expected_version != current_version {
        return stale_for(
            transaction,
            request_id,
            "account_responsibility_transfer",
            &member,
            &payload,
            account_ref,
            current_version,
            "ops_native_responsibility_version_stale",
        )
        .await;
    }
    if let Some(next_member_ref) = body.next_member_ref {
        let target_state: Option<String> = sqlx::query_scalar(
            "SELECT state FROM linggan_ops_member WHERE member_ref=$1 FOR SHARE",
        )
        .bind(next_member_ref)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WriteError::Unavailable)?;
        match target_state.as_deref() {
            Some("active") => {}
            Some(_) => {
                return rejected(
                    transaction,
                    request_id,
                    "account_responsibility_transfer",
                    &member,
                    &payload,
                    Some(account_ref),
                    Some(current_version),
                    "ops_native_responsibility_target_inactive",
                    StatusCode::UNPROCESSABLE_ENTITY,
                )
                .await;
            }
            None => {
                return rejected(
                    transaction,
                    request_id,
                    "account_responsibility_transfer",
                    &member,
                    &payload,
                    Some(account_ref),
                    Some(current_version),
                    "ops_native_responsibility_target_not_found",
                    StatusCode::NOT_FOUND,
                )
                .await;
            }
        }
    }
    let allowed = if member.role == "admin" {
        true
    } else if member.role == "lead" {
        match (previous_member_ref, body.next_member_ref) {
            (Some(previous_member_ref), Some(next_member_ref)) => {
                same_active_group(&mut transaction, member.member_ref, previous_member_ref).await?
                    && same_active_group(&mut transaction, member.member_ref, next_member_ref)
                        .await?
            }
            _ => false,
        }
    } else {
        false
    };
    if !allowed {
        return rejected(
            transaction,
            request_id,
            "account_responsibility_transfer",
            &member,
            &payload,
            Some(account_ref),
            Some(current_version),
            "ops_native_forbidden",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    if previous_member_ref == body.next_member_ref {
        let receipt = unchanged(
            request_id,
            "account_responsibility_transfer",
            &member,
            &payload,
            account_ref,
            current_version,
            "ops_native_responsibility_unchanged",
        );
        insert_receipt(&mut transaction, &receipt).await?;
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((StatusCode::OK, receipt.receipt));
    }
    let resulting_version = current_version + 1;
    sqlx::query(
        "UPDATE linggan_ops_account \
         SET current_responsible_member_ref=$2,responsibility_version=$3 WHERE account_ref=$1",
    )
    .bind(account_ref)
    .bind(body.next_member_ref)
    .bind(resulting_version)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    sqlx::query(
        "INSERT INTO linggan_ops_account_responsibility_change \
         (change_ref,account_ref,previous_member_ref,next_member_ref,changed_by_member_ref,reason,request_ref,resulting_version) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(Uuid::new_v4())
    .bind(account_ref)
    .bind(previous_member_ref)
    .bind(body.next_member_ref)
    .bind(member.member_ref)
    .bind(&reason)
    .bind(request_id)
    .bind(resulting_version)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let receipt = accepted(
        request_id,
        "account_responsibility_transfer",
        &member,
        &payload,
        account_ref,
        resulting_version,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::OK, receipt.receipt))
}

async fn write_execution_locator(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    publication_ref: Uuid,
    body: RecordExecutionLocatorBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let raw_payload = raw_payload(&body)?;
    let request_id = body.request_id;
    let locator = match required_url(&body.signed_locator).and_then(|value| {
        (xhs_note_id(&value).is_some() && has_xhs_signature(&value))
            .then_some(value)
            .ok_or(WriteError::Invalid("ops_native_execution_locator_invalid"))
    }) {
        Ok(locator) => locator,
        Err(WriteError::Invalid(code)) => {
            return reject_invalid_input(
                database,
                &member,
                request_id,
                "publication_locator_record",
                &raw_payload,
                code,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let content_id =
        xhs_note_id(&locator).expect("validated signed locator has XHS content identity");
    let payload = digest(json!({ "publicationRef": publication_ref, "signedLocator": locator }));
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        request_id,
        "publication_locator_record",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let publication: Option<(Uuid, String, Option<i64>, bool, Option<String>, String, String, Option<String>)> = sqlx::query_as(
        "SELECT publication.account_ref,publication.platform,publication.native_version, \
                publication.source_system='linggan_native' AND publication.native_record_state='active' AS active, \
                account.platform_user_id,publication.publication_url,publication.canonical_url,publication.platform_content_id \
           FROM linggan_ops_publication publication \
           JOIN linggan_ops_account account ON account.account_ref=publication.account_ref \
          WHERE publication.publication_ref=$1 FOR SHARE OF publication",
    )
    .bind(publication_ref)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let Some((
        account_ref,
        platform,
        version,
        active,
        author_id,
        registered_url,
        canonical_url,
        registered_id,
    )) = publication
    else {
        return rejected(
            transaction,
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            Some(publication_ref),
            None,
            "ops_native_publication_not_found",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    if !active {
        return rejected(
            transaction,
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            Some(publication_ref),
            version,
            "ops_native_publication_inactive",
            StatusCode::CONFLICT,
        )
        .await;
    }
    if platform != "xhs" {
        return rejected(
            transaction,
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            Some(publication_ref),
            version,
            "ops_native_execution_locator_platform_unsupported",
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }
    if !can_write_account(&mut transaction, &member, account_ref).await? {
        return rejected(
            transaction,
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            Some(publication_ref),
            version,
            "ops_native_forbidden",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    if [
        registered_id,
        xhs_note_id(&registered_url),
        xhs_note_id(&canonical_url),
    ]
    .into_iter()
    .flatten()
    .any(|known| known != content_id)
    {
        return rejected(
            transaction,
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            Some(publication_ref),
            version,
            "ops_native_execution_locator_identity_conflict",
            StatusCode::CONFLICT,
        )
        .await;
    }
    let Some(author_id) = author_id.filter(|value| !value.trim().is_empty()) else {
        return rejected(
            transaction,
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            Some(publication_ref),
            version,
            "ops_native_execution_locator_author_unresolved",
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    };
    let Some(version) = version else {
        return Err(WriteError::Unavailable);
    };
    let locator_sha256 = Sha256::digest(locator.as_bytes()).to_vec();
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT locator_ref FROM linggan_ops_publication_execution_locator \
          WHERE publication_ref=$1 AND locator_sha256=$2 FOR SHARE",
    )
    .bind(publication_ref)
    .bind(&locator_sha256)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?
    {
        let receipt = accepted(
            request_id,
            "publication_locator_record",
            &member,
            &payload,
            existing,
            version,
        );
        insert_receipt(&mut transaction, &receipt).await?;
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((StatusCode::OK, receipt.receipt));
    }
    let locator_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_publication_execution_locator \
         (locator_ref,publication_ref,account_ref,platform,expected_content_id,expected_author_id,publication_version, \
          locator_url,locator_sha256,provided_by_member_ref,validation_state) \
         VALUES($1,$2,$3,'xhs',$4,$5,$6,$7,$8,$9,'accepted_xhs_signed_locator')",
    )
    .bind(locator_ref)
    .bind(publication_ref)
    .bind(account_ref)
    .bind(&content_id)
    .bind(&author_id)
    .bind(version)
    .bind(&locator)
    .bind(&locator_sha256)
    .bind(member.member_ref)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let receipt = accepted(
        request_id,
        "publication_locator_record",
        &member,
        &payload,
        locator_ref,
        version,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::CREATED, receipt.receipt))
}

async fn write_view_report(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    publication_ref: Uuid,
    body: RecordViewBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let request_id = body.request_id;
    let raw_payload = digest(json!({ "publicationRef": publication_ref, "body": &body }));
    let report = match PreparedViewReport::parse(body) {
        Ok(report) => report,
        Err(WriteError::Invalid(code)) => {
            return reject_invalid_input(
                database,
                &member,
                request_id,
                "view_report_record",
                &raw_payload,
                code,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let payload = digest(json!({
        "publicationRef": publication_ref,
        "reportKind": report.report_kind,
        "viewCount": report.view_count,
        "asOf": report.as_of,
    }));
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        report.request_id,
        "view_report_record",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    let account_ref: Option<Option<Uuid>> = sqlx::query_scalar(
        "SELECT account_ref FROM linggan_ops_publication \
         WHERE publication_ref=$1 AND missing_since_batch_ref IS NULL \
           AND (source_system <> 'linggan_native' OR native_record_state='active') FOR SHARE",
    )
    .bind(publication_ref)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let Some(account_ref) = account_ref else {
        return rejected(
            transaction,
            report.request_id,
            "view_report_record",
            &member,
            &payload,
            Some(publication_ref),
            None,
            "ops_native_publication_not_found",
            StatusCode::NOT_FOUND,
        )
        .await;
    };
    let Some(account_ref) = account_ref else {
        return rejected(
            transaction,
            report.request_id,
            "view_report_record",
            &member,
            &payload,
            Some(publication_ref),
            None,
            "ops_native_account_unassigned",
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    };
    if !can_write_account(&mut transaction, &member, account_ref).await? {
        return rejected(
            transaction,
            report.request_id,
            "view_report_record",
            &member,
            &payload,
            Some(publication_ref),
            None,
            "ops_native_forbidden",
            StatusCode::FORBIDDEN,
        )
        .await;
    }
    let report_ref = Uuid::new_v4();
    let receipt = accepted(
        report.request_id,
        "view_report_record",
        &member,
        &payload,
        publication_ref,
        1,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    sqlx::query(
        "INSERT INTO linggan_ops_view_report \
         (report_ref,publication_ref,report_kind,view_count,source_kind,as_of,reported_by_member_ref,request_ref) \
         VALUES($1,$2,$3,$4,'linggan_ops_manual',$5::timestamptz,$6,$7)",
    )
    .bind(report_ref)
    .bind(publication_ref)
    .bind(&report.report_kind)
    .bind(report.view_count)
    .bind(&report.as_of)
    .bind(member.member_ref)
    .bind(report.request_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::CREATED, receipt.receipt))
}

async fn write_daily_report(
    database: &linggan_storage_postgres::Database,
    member: super::ops_auth::AuthenticatedMember,
    body: RecordDailyBody,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let raw_payload = raw_payload(&body)?;
    let request_id = body.request_id;
    let report = match PreparedDailyReport::parse(body) {
        Ok(report) => report,
        Err(WriteError::Invalid(code)) => {
            return reject_invalid_input(
                database,
                &member,
                request_id,
                "daily_report_record",
                &raw_payload,
                code,
            )
            .await;
        }
        Err(error) => return Err(error),
    };
    let payload = digest(json!({
        "businessDate": report.business_date,
        "linksOpenedCount": report.links_opened_count,
        "followersGained": report.followers_gained,
        "expectedVersion": report.expected_version,
        "dailyRef": report.daily_ref,
    }));
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) = replay(
        &mut transaction,
        report.request_id,
        "daily_report_record",
        &member,
        &payload,
    )
    .await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    lock_daily(&mut transaction, member.member_ref, &report.business_date).await?;
    let existing =
        daily_for_update(&mut transaction, member.member_ref, &report.business_date).await?;
    match existing {
        Some((daily_ref, _)) if report.daily_ref.is_some_and(|target| target != daily_ref) => {
            rejected(
                transaction,
                report.request_id,
                "daily_report_record",
                &member,
                &payload,
                report.daily_ref,
                None,
                "ops_native_daily_not_owned",
                StatusCode::FORBIDDEN,
            )
            .await
        }
        Some((daily_ref, current)) if report.expected_version != Some(current) => {
            stale(
                transaction,
                report.request_id,
                &member,
                &payload,
                daily_ref,
                current,
            )
            .await
        }
        None if report.expected_version.is_some() => {
            rejected(
                transaction,
                report.request_id,
                "daily_report_record",
                &member,
                &payload,
                None,
                None,
                "ops_native_daily_not_found",
                StatusCode::CONFLICT,
            )
            .await
        }
        Some((daily_ref, current)) => {
            update_daily(transaction, &member, &report, &payload, daily_ref, current).await
        }
        None => create_daily(transaction, &member, &report, &payload).await,
    }
}

async fn create_daily(
    mut transaction: Transaction<'_, Postgres>,
    member: &super::ops_auth::AuthenticatedMember,
    report: &PreparedDailyReport,
    payload: &[u8; 32],
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let daily_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_daily_report \
         (daily_ref,source_system,source_record_id,operator_name,business_date,links_opened_count,followers_gained,\
          review_status,native_member_ref,native_version,native_created_by_member_ref,native_updated_by_member_ref) \
         VALUES($1,'linggan_native',$1::text,$2,$3::date,$4,$5,'confirmed',$6,1,$6,$6)",
    )
    .bind(daily_ref)
    .bind(&member.display_name)
    .bind(&report.business_date)
    .bind(report.links_opened_count)
    .bind(report.followers_gained)
    .bind(member.member_ref)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let receipt = accepted(
        report.request_id,
        "daily_report_record",
        member,
        payload,
        daily_ref,
        1,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    append_daily_revision(
        &mut transaction,
        report,
        member.member_ref,
        daily_ref,
        None,
        1,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::CREATED, receipt.receipt))
}

async fn update_daily(
    mut transaction: Transaction<'_, Postgres>,
    member: &super::ops_auth::AuthenticatedMember,
    report: &PreparedDailyReport,
    payload: &[u8; 32],
    daily_ref: Uuid,
    current: i64,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let next = current + 1;
    sqlx::query(
        "UPDATE linggan_ops_daily_report SET links_opened_count=$2,followers_gained=$3,native_version=$4,\
         native_updated_by_member_ref=$5 WHERE daily_ref=$1 AND native_version=$6",
    )
    .bind(daily_ref)
    .bind(report.links_opened_count)
    .bind(report.followers_gained)
    .bind(next)
    .bind(member.member_ref)
    .bind(current)
    .execute(&mut *transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let receipt = accepted(
        report.request_id,
        "daily_report_record",
        member,
        payload,
        daily_ref,
        next,
    );
    insert_receipt(&mut transaction, &receipt).await?;
    append_daily_revision(
        &mut transaction,
        report,
        member.member_ref,
        daily_ref,
        Some(current),
        next,
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::OK, receipt.receipt))
}

async fn append_daily_revision(
    transaction: &mut Transaction<'_, Postgres>,
    report: &PreparedDailyReport,
    member_ref: Uuid,
    daily_ref: Uuid,
    previous: Option<i64>,
    resulting: i64,
) -> Result<(), WriteError> {
    sqlx::query(
        "INSERT INTO linggan_ops_daily_report_revision \
         (revision_ref,daily_ref,previous_version,resulting_version,links_opened_count,followers_gained,\
          changed_by_member_ref,request_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(Uuid::new_v4())
    .bind(daily_ref)
    .bind(previous)
    .bind(resulting)
    .bind(report.links_opened_count)
    .bind(report.followers_gained)
    .bind(member_ref)
    .bind(report.request_id)
    .execute(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    Ok(())
}

async fn replay(
    transaction: &mut Transaction<'_, Postgres>,
    request_ref: Uuid,
    command_kind: &str,
    member: &super::ops_auth::AuthenticatedMember,
    payload: &[u8; 32],
) -> Result<Option<CommandReceipt>, WriteError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(request_ref.to_string())
        .execute(&mut **transaction)
        .await
        .map_err(|_| WriteError::Unavailable)?;
    let row: Option<(Uuid, String, Uuid, Vec<u8>, String, Option<Uuid>, Option<i64>, String)> = sqlx::query_as(
        "SELECT request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,resulting_version,outcome_code \
         FROM linggan_ops_native_write_request WHERE request_ref=$1",
    )
    .bind(request_ref)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?;
    let Some(row) = row else { return Ok(None) };
    let (
        stored_request_ref,
        stored_command_kind,
        stored_actor_member_ref,
        stored_payload,
        outcome,
        record_ref,
        resulting_version,
        outcome_code,
    ) = row;
    if stored_command_kind != command_kind
        || stored_actor_member_ref != member.member_ref
        || stored_payload != payload
    {
        return Err(WriteError::Invalid("ops_native_request_conflict"));
    }
    Ok(Some(CommandReceipt {
        request_ref: stored_request_ref,
        outcome,
        code: outcome_code,
        record_ref,
        resulting_version,
        replayed: true,
    }))
}

fn accepted(
    request_ref: Uuid,
    command_kind: &'static str,
    member: &super::ops_auth::AuthenticatedMember,
    payload: &[u8; 32],
    record_ref: Uuid,
    resulting_version: i64,
) -> PendingReceipt {
    PendingReceipt {
        receipt: CommandReceipt {
            request_ref,
            outcome: "accepted".to_owned(),
            code: "accepted".to_owned(),
            record_ref: Some(record_ref),
            resulting_version: Some(resulting_version),
            replayed: false,
        },
        command_kind,
        actor_member_ref: member.member_ref,
        payload: *payload,
    }
}

fn unchanged(
    request_ref: Uuid,
    command_kind: &'static str,
    member: &super::ops_auth::AuthenticatedMember,
    payload: &[u8; 32],
    record_ref: Uuid,
    resulting_version: i64,
    code: &'static str,
) -> PendingReceipt {
    PendingReceipt {
        receipt: CommandReceipt {
            request_ref,
            outcome: "unchanged".to_owned(),
            code: code.to_owned(),
            record_ref: Some(record_ref),
            resulting_version: Some(resulting_version),
            replayed: false,
        },
        command_kind,
        actor_member_ref: member.member_ref,
        payload: *payload,
    }
}

struct PendingReceipt {
    receipt: CommandReceipt,
    command_kind: &'static str,
    actor_member_ref: Uuid,
    payload: [u8; 32],
}

async fn insert_receipt(
    transaction: &mut Transaction<'_, Postgres>,
    pending: &PendingReceipt,
) -> Result<(), WriteError> {
    sqlx::query(
        "INSERT INTO linggan_ops_native_write_request \
         (request_ref,command_kind,actor_member_ref,payload_sha256,outcome,record_ref,resulting_version,outcome_code) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(pending.receipt.request_ref)
    .bind(pending.command_kind)
    .bind(pending.actor_member_ref)
    .bind(pending.payload.as_slice())
    .bind(&pending.receipt.outcome)
    .bind(pending.receipt.record_ref)
    .bind(pending.receipt.resulting_version)
    .bind(&pending.receipt.code)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        eprintln!(
            "ops native receipt unavailable: command_kind={} database_kind={:?}",
            pending.command_kind,
            error.as_database_error().and_then(|value| value.code())
        );
        WriteError::Unavailable
    })?;
    Ok(())
}

async fn rejected(
    mut transaction: Transaction<'_, Postgres>,
    request_ref: Uuid,
    command_kind: &'static str,
    member: &super::ops_auth::AuthenticatedMember,
    payload: &[u8; 32],
    record_ref: Option<Uuid>,
    resulting_version: Option<i64>,
    code: &'static str,
    status: StatusCode,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let pending = PendingReceipt {
        receipt: CommandReceipt {
            request_ref,
            outcome: "rejected".to_owned(),
            code: code.to_owned(),
            record_ref,
            resulting_version,
            replayed: false,
        },
        command_kind,
        actor_member_ref: member.member_ref,
        payload: *payload,
    };
    insert_receipt(&mut transaction, &pending).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((status, pending.receipt))
}

async fn reject_invalid_input(
    database: &linggan_storage_postgres::Database,
    member: &super::ops_auth::AuthenticatedMember,
    request_ref: Uuid,
    command_kind: &'static str,
    payload: &[u8; 32],
    code: &'static str,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    if let Some(receipt) =
        replay(&mut transaction, request_ref, command_kind, member, payload).await?
    {
        transaction
            .commit()
            .await
            .map_err(|_| WriteError::Unavailable)?;
        return Ok((status_for(&receipt), receipt));
    }
    rejected(
        transaction,
        request_ref,
        command_kind,
        member,
        payload,
        None,
        None,
        code,
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await
}

async fn stale(
    transaction: Transaction<'_, Postgres>,
    request_ref: Uuid,
    member: &super::ops_auth::AuthenticatedMember,
    payload: &[u8; 32],
    daily_ref: Uuid,
    current: i64,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    stale_for(
        transaction,
        request_ref,
        "daily_report_record",
        member,
        payload,
        daily_ref,
        current,
        "ops_native_version_stale",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn stale_for(
    mut transaction: Transaction<'_, Postgres>,
    request_ref: Uuid,
    command_kind: &'static str,
    member: &super::ops_auth::AuthenticatedMember,
    payload: &[u8; 32],
    record_ref: Uuid,
    current: i64,
    code: &'static str,
) -> Result<(StatusCode, CommandReceipt), WriteError> {
    let pending = PendingReceipt {
        receipt: CommandReceipt {
            request_ref,
            outcome: "stale".to_owned(),
            code: code.to_owned(),
            record_ref: Some(record_ref),
            resulting_version: Some(current),
            replayed: false,
        },
        command_kind,
        actor_member_ref: member.member_ref,
        payload: *payload,
    };
    insert_receipt(&mut transaction, &pending).await?;
    transaction
        .commit()
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok((StatusCode::CONFLICT, pending.receipt))
}

async fn identity_claimed(
    transaction: &mut Transaction<'_, Postgres>,
    platform: &str,
    identity: &str,
) -> Result<bool, WriteError> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM linggan_ops_account WHERE platform=$1 AND platform_user_id=$2)")
        .bind(platform)
        .bind(identity)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| WriteError::Unavailable)
}

async fn lock_platform_identity(
    transaction: &mut Transaction<'_, Postgres>,
    platform: &str,
    identity: &str,
) -> Result<(), WriteError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(format!("ops-native-account:{platform}:{identity}"))
        .execute(&mut **transaction)
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok(())
}

async fn account_for_publication(
    transaction: &mut Transaction<'_, Postgres>,
    account_ref: Uuid,
) -> Result<Option<(String, Uuid, bool)>, WriteError> {
    sqlx::query_as(
        "SELECT platform,domain_ref,(source_system='linggan_native' AND native_record_state='active') \
         OR (source_system='content_workbench' AND ledger_state='in_ledger') AS active \
         FROM linggan_ops_account WHERE account_ref=$1 FOR SHARE",
    )
    .bind(account_ref)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)
}

async fn can_write_account(
    transaction: &mut Transaction<'_, Postgres>,
    member: &super::ops_auth::AuthenticatedMember,
    account_ref: Uuid,
) -> Result<bool, WriteError> {
    if member.role == "admin" {
        return Ok(true);
    }
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT current_responsible_member_ref FROM linggan_ops_account WHERE account_ref=$1 FOR SHARE",
    )
    .bind(account_ref)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)?
    .flatten();
    let Some(owner) = owner else { return Ok(false) };
    if owner == member.member_ref {
        return Ok(true);
    }
    if member.role != "lead" {
        return Ok(false);
    }
    same_active_group(transaction, member.member_ref, owner).await
}

async fn same_active_group(
    transaction: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    owner: Uuid,
) -> Result<bool, WriteError> {
    sqlx::query_scalar(
        "WITH latest AS (SELECT DISTINCT ON (member_ref) member_ref,group_ref,action \
                         FROM linggan_ops_group_membership_event WHERE member_ref IN ($1,$2) \
                         ORDER BY member_ref,occurred_at DESC,event_ref DESC) \
         SELECT EXISTS (SELECT 1 FROM latest a JOIN latest o ON a.group_ref=o.group_ref \
                        JOIN linggan_ops_group g ON g.group_ref=a.group_ref \
                        WHERE a.member_ref=$1 AND o.member_ref=$2 AND a.action='joined' \
                          AND o.action='joined' AND g.state='active')",
    )
    .bind(actor)
    .bind(owner)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)
}

async fn lock_daily(
    transaction: &mut Transaction<'_, Postgres>,
    member_ref: Uuid,
    business_date: &str,
) -> Result<(), WriteError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(format!("ops-native-daily:{member_ref}:{business_date}"))
        .execute(&mut **transaction)
        .await
        .map_err(|_| WriteError::Unavailable)?;
    Ok(())
}

async fn daily_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    member_ref: Uuid,
    business_date: &str,
) -> Result<Option<(Uuid, i64)>, WriteError> {
    sqlx::query_as(
        "SELECT daily_ref,native_version FROM linggan_ops_daily_report \
         WHERE source_system='linggan_native' AND native_member_ref=$1 AND business_date=$2::date FOR UPDATE",
    )
    .bind(member_ref)
    .bind(business_date)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|_| WriteError::Unavailable)
}

fn receipt_response(status: StatusCode, receipt: CommandReceipt) -> Response {
    let mut response = (status, Json(json!({ "receipt": receipt }))).into_response();
    super::ops_auth::secure_headers(&mut response);
    response
}

fn write_error(error: WriteError) -> Response {
    match error {
        WriteError::Invalid(code) => native_error(
            if code == "ops_native_request_conflict" {
                StatusCode::CONFLICT
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            },
            code,
        ),
        WriteError::Unavailable => native_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ops_native_write_unavailable",
        ),
    }
}

fn native_error(status: StatusCode, code: &'static str) -> Response {
    let mut response = (status, Json(json!({ "code": code }))).into_response();
    super::ops_auth::secure_headers(&mut response);
    response
}

fn status_for(receipt: &CommandReceipt) -> StatusCode {
    match receipt.outcome.as_str() {
        "accepted" | "unchanged" => StatusCode::OK,
        "stale" => StatusCode::CONFLICT,
        "rejected" if receipt.code == "ops_native_forbidden" => StatusCode::FORBIDDEN,
        "rejected" if receipt.code.ends_with("not_found") => StatusCode::NOT_FOUND,
        "rejected" if receipt.code.ends_with("conflict") => StatusCode::CONFLICT,
        "rejected" => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn digest(value: Value) -> [u8; 32] {
    Sha256::digest(value.to_string().as_bytes()).into()
}

fn raw_payload<T: Serialize>(body: &T) -> Result<[u8; 32], WriteError> {
    serde_json::to_value(body)
        .map(digest)
        .map_err(|_| WriteError::Unavailable)
}

struct PreparedAccount {
    request_id: Uuid,
    domain_ref: Uuid,
    platform: String,
    account_name: String,
    profile_url: Option<String>,
    platform_user_id: Option<String>,
    handle: Option<String>,
    status: String,
    is_retained: bool,
    follower_count: Option<i64>,
    device_code: Option<String>,
    ip_location: Option<String>,
    notes: Option<String>,
}

impl PreparedAccount {
    fn parse(body: CreateAccountBody) -> Result<Self, WriteError> {
        let platform = choice(&body.platform, &["xhs", "douyin"])?;
        let account_name = required(&body.account_name, 512)?;
        let profile_url = optional_url(body.profile_url)?;
        let platform_user_id = optional(body.platform_user_id, 128)?;
        if platform_user_id
            .as_deref()
            .is_some_and(|value| platform != "xhs" || !is_xhs_id(value))
        {
            return Err(WriteError::Invalid("ops_native_account_identity_invalid"));
        }
        let status = body
            .status
            .map(|value| choice(&value, &["active", "warming", "paused", "archived"]))
            .transpose()?
            .unwrap_or_else(|| "active".to_owned());
        if body.follower_count.is_some_and(|value| value < 0) {
            return Err(WriteError::Invalid("ops_native_account_invalid"));
        }
        Ok(Self {
            request_id: body.request_id,
            domain_ref: body.domain_ref,
            platform,
            account_name,
            profile_url,
            platform_user_id,
            handle: optional(body.handle, 512)?,
            status,
            is_retained: body.is_retained.unwrap_or(true),
            follower_count: body.follower_count,
            device_code: optional(body.device_code, 512)?,
            ip_location: optional(body.ip_location, 512)?,
            notes: optional(body.notes, 4_000)?,
        })
    }

    fn profile_state(&self) -> &'static str {
        if self.platform_user_id.is_some() {
            "parsed"
        } else if self.profile_url.is_some() {
            "pending_match"
        } else {
            "missing"
        }
    }
}

struct PreparedAccountUpdate {
    request_id: Uuid,
    expected_version: i64,
    account_name: String,
    profile_url: Option<String>,
    handle: Option<String>,
    status: String,
    is_retained: bool,
    follower_count: Option<i64>,
    device_code: Option<String>,
    ip_location: Option<String>,
    notes: Option<String>,
}

impl PreparedAccountUpdate {
    fn parse(body: UpdateAccountBody) -> Result<Self, WriteError> {
        if body.expected_version < 1 || body.follower_count.is_some_and(|value| value < 0) {
            return Err(WriteError::Invalid("ops_native_account_update_invalid"));
        }
        Ok(Self {
            request_id: body.request_id,
            expected_version: body.expected_version,
            account_name: required(&body.account_name, 512)?,
            profile_url: optional_url(body.profile_url)?,
            handle: optional(body.handle, 512)?,
            status: choice(&body.status, &["active", "warming", "paused"])?,
            is_retained: body.is_retained,
            follower_count: body.follower_count,
            device_code: optional(body.device_code, 512)?,
            ip_location: optional(body.ip_location, 512)?,
            notes: optional(body.notes, 4_000)?,
        })
    }
}

struct PreparedPublication {
    request_id: Uuid,
    account_ref: Uuid,
    publication_url: String,
    canonical_url: String,
    precision: String,
    published_at: Option<String>,
    published_on: Option<String>,
    title: Option<String>,
    body: Option<String>,
}

impl PreparedPublication {
    fn parse(body: CreatePublicationBody) -> Result<Self, WriteError> {
        let publication_url = required_url(&body.publication_url)?;
        let canonical_url = match body.canonical_url {
            Some(value) => required_url(&value)?,
            None => publication_url.clone(),
        };
        let (precision, published_at, published_on) =
            publication_date(body.published_at, body.published_on)?;
        Ok(Self {
            request_id: body.request_id,
            account_ref: body.account_ref,
            publication_url,
            canonical_url,
            precision,
            published_at,
            published_on,
            title: optional(body.title, 1_000)?,
            body: optional(body.body, 100_000)?,
        })
    }

    fn payload(&self) -> Value {
        json!({ "accountRef": self.account_ref, "publicationUrl": self.publication_url, "canonicalUrl": self.canonical_url, "precision": self.precision, "publishedAt": self.published_at, "publishedOn": self.published_on, "title": self.title, "body": self.body })
    }
}

struct PreparedViewReport {
    request_id: Uuid,
    report_kind: String,
    view_count: Option<i64>,
    as_of: String,
}

impl PreparedViewReport {
    fn parse(body: RecordViewBody) -> Result<Self, WriteError> {
        let report_kind = choice(&body.report_kind, &["value", "cleared"])?;
        if (report_kind == "value") != body.view_count.is_some()
            || body.view_count.is_some_and(|value| value < 0)
        {
            return Err(WriteError::Invalid("ops_native_view_report_invalid"));
        }
        if !has_offset_timestamp(&body.as_of) {
            return Err(WriteError::Invalid("ops_native_as_of_invalid"));
        }
        Ok(Self {
            request_id: body.request_id,
            report_kind,
            view_count: body.view_count,
            as_of: body.as_of.trim().to_owned(),
        })
    }
}

struct PreparedDailyReport {
    request_id: Uuid,
    daily_ref: Option<Uuid>,
    business_date: String,
    links_opened_count: Option<i32>,
    followers_gained: Option<i32>,
    expected_version: Option<i64>,
}

impl PreparedDailyReport {
    fn parse(body: RecordDailyBody) -> Result<Self, WriteError> {
        let business_date = body.business_date.trim().to_owned();
        if !is_date(&business_date)
            || body.links_opened_count.is_some_and(|value| value < 0)
            || body.followers_gained.is_some_and(|value| value < 0)
            || body.expected_version.is_some_and(|value| value < 1)
            || (body.expected_version.is_some() != body.daily_ref.is_some())
        {
            return Err(WriteError::Invalid("ops_native_daily_invalid"));
        }
        Ok(Self {
            request_id: body.request_id,
            daily_ref: body.daily_ref,
            business_date,
            links_opened_count: body.links_opened_count,
            followers_gained: body.followers_gained,
            expected_version: body.expected_version,
        })
    }
}

fn required(value: &str, maximum: usize) -> Result<String, WriteError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum {
        return Err(WriteError::Invalid("ops_native_input_invalid"));
    }
    Ok(value.to_owned())
}

fn optional(value: Option<String>, maximum: usize) -> Result<Option<String>, WriteError> {
    value.map(|value| required(&value, maximum)).transpose()
}

fn choice(value: &str, options: &[&str]) -> Result<String, WriteError> {
    let value = value.trim();
    options
        .contains(&value)
        .then(|| value.to_owned())
        .ok_or(WriteError::Invalid("ops_native_input_invalid"))
}

fn required_url(value: &str) -> Result<String, WriteError> {
    let value = required(value, 4_096)?;
    if !(value.starts_with("https://") || value.starts_with("http://"))
        || value.chars().any(char::is_whitespace)
    {
        return Err(WriteError::Invalid("ops_native_url_invalid"));
    }
    Ok(value)
}

/// A note ID names the work; this signed locator is only its short-lived execution input.
/// Keep the token out of all API receipts and require a non-empty value rather than accepting a
/// bare detail URL that the Producer cannot actually open.
fn has_xhs_signature(value: &str) -> bool {
    value
        .split_once('?')
        .map(|(_, query)| {
            query.split('&').any(|entry| {
                entry
                    .split_once('=')
                    .is_some_and(|(key, token)| key == "xsec_token" && !token.is_empty())
            })
        })
        .unwrap_or(false)
}

fn optional_url(value: Option<String>) -> Result<Option<String>, WriteError> {
    value.map(|value| required_url(&value)).transpose()
}

fn is_xhs_id(value: &str) -> bool {
    value.len() == 24
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte.is_ascii_lowercase())
}

fn is_date(value: &str) -> bool {
    let Some((year, month, day)) = value
        .split_once('-')
        .and_then(|(year, rest)| rest.split_once('-').map(|(month, day)| (year, month, day)))
    else {
        return false;
    };
    if year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year.bytes().all(|byte| byte.is_ascii_digit())
        || !month.bytes().all(|byte| byte.is_ascii_digit())
        || !day.bytes().all(|byte| byte.is_ascii_digit())
    {
        return false;
    }
    let year: i32 = match year.parse() {
        Ok(year) => year,
        Err(_) => return false,
    };
    let month: u32 = match month.parse() {
        Ok(month) => month,
        Err(_) => return false,
    };
    let day: u32 = match day.parse() {
        Ok(day) => day,
        Err(_) => return false,
    };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1 && (1..=days).contains(&day)
}

fn publication_date(
    published_at: Option<String>,
    published_on: Option<String>,
) -> Result<(String, Option<String>, Option<String>), WriteError> {
    match (published_at, published_on) {
        (Some(instant), None) if has_offset_timestamp(&instant) => {
            Ok(("instant".to_owned(), Some(instant.trim().to_owned()), None))
        }
        (None, Some(day)) if is_date(day.trim()) => {
            Ok(("day".to_owned(), None, Some(day.trim().to_owned())))
        }
        _ => Err(WriteError::Invalid("ops_native_publication_date_invalid")),
    }
}

fn has_offset_timestamp(value: &str) -> bool {
    let value = value.trim();
    let Some((date, time)) = value.split_once('T') else {
        return false;
    };
    if !is_date(date) {
        return false;
    }
    if let Some(clock) = time.strip_suffix('Z') {
        return is_clock(clock);
    }
    let Some(index) = time.rfind(['+', '-']) else {
        return false;
    };
    let (clock, offset) = time.split_at(index);
    is_clock(clock) && is_offset(&offset[1..])
}

fn is_clock(value: &str) -> bool {
    let (clock, fractional) = value
        .split_once('.')
        .map_or((value, None), |(clock, fractional)| {
            (clock, Some(fractional))
        });
    if fractional.is_some_and(|fractional| {
        fractional.is_empty() || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    }) {
        return false;
    }
    let Some((hours, rest)) = clock.split_once(':') else {
        return false;
    };
    let Some((minutes, seconds)) = rest.split_once(':') else {
        return false;
    };
    time_part(hours, 23) && time_part(minutes, 59) && time_part(seconds, 59)
}

fn is_offset(value: &str) -> bool {
    let Some((hours, minutes)) = value.split_once(':') else {
        return false;
    };
    time_part(hours, 15) && time_part(minutes, 59)
}

fn time_part(value: &str, maximum: u8) -> bool {
    value.len() == 2
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u8>().is_ok_and(|value| value <= maximum)
}
