//! COLLECTION-CONTROL-CLOSURE-001 · one server-owned control vocabulary.
//!
//! This module owns installation credentials, privacy-preserving platform-account identity,
//! eligibility observations, station acceptance and the capacity result consumed by Admission,
//! Lease issuance, dispatch and the local UI. It never stores a raw platform account id, Cookie,
//! page body, or a free-form platform error.

use crate::acquisition_chain::{AcquisitionChainError, request_and_admit_in_transaction};
use crate::station_read::station_daily_note_usage_in;
use crate::work_order_lease::LeaseError;
use linggan_contracts::{Capacity, CapacityReasonCode};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MINIMUM_PLUGIN_VERSION: &str = "0.8.34";
/// How recently an installation must have checked in to be considered on duty.
///
/// This is a liveness question — is that browser still there — and it stays short. A worker
/// that has been silent for twenty minutes cannot be handed platform work.
pub const CONTROL_FRESHNESS_MINUTES: i32 = 20;
/// How long one passive account-eligibility observation stays usable.
///
/// This answers a different question from [`CONTROL_FRESHNESS_MINUTES`]: not "is the browser
/// alive" but "is this platform account still in a state we may work with". The two shared one
/// constant until 2026-09-06, which silently coupled them.
///
/// Twenty minutes was far too short for the second question, because the only way to refresh
/// this observation is to read an *already open* XHS document — the producer will not open,
/// reload, or navigate a tab to get it. Closing the last XHS tab therefore stopped every deep
/// archive within twenty minutes, with no error recorded anywhere: capacity selection simply
/// skipped every work order and the queue looked empty. Observed on 2026-09-06, where a target
/// sat with twenty-two ready tasks for eight hours while all nine other control checks passed.
///
/// A platform login survives for days, so six hours is still a conservative claim about it. The
/// window only decides how long we may act on the last real observation; it never invents one.
/// Any dispatch that comes back with an account state still forces a fresh passive read, and a
/// real failure still ends the run — this widens patience, not trust.
pub const ACCOUNT_ELIGIBILITY_TTL_MINUTES: i32 = 360;
pub const DEFAULT_MONITOR_INTERVAL_SECONDS: i32 = 86_400;
pub const MINIMUM_MONITOR_INTERVAL_SECONDS: i32 = 21_600;
pub const MAXIMUM_MONITOR_INTERVAL_SECONDS: i32 = 604_800;
const CREDENTIAL_VALID_FOR_DAYS: i32 = 30;
const PENDING_CREDENTIAL_VALID_FOR_MINUTES: i32 = 10;
const ACCOUNT_BINDING_VALID_FOR_DAYS: i32 = 30;

#[derive(Debug, thiserror::Error)]
pub enum CollectionControlError {
    #[error("collection control schema is not applied")]
    SchemaUnavailable,
    #[error("no active plugin installation with that reference")]
    UnknownInstallation,
    #[error("no observation account with that reference")]
    UnknownAccount,
    #[error("installation credential is absent, expired, revoked, or invalid")]
    InvalidCredential,
    #[error("plugin version is below the minimum collection-control contract")]
    UnsupportedPluginVersion,
    #[error("platform account identity must be transient, non-empty, and bounded")]
    InvalidPlatformIdentity,
    #[error("account digest key is unavailable")]
    MissingDigestKey,
    #[error("eligibility state and reason code do not form a closed pair")]
    InvalidEligibility,
    #[error("station acceptance requires a person action")]
    InvalidStationActor,
    #[error("no active execution station with that reference")]
    UnknownStation,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub struct InstallationCredentialSecret(String);

impl InstallationCredentialSecret {
    /// The local HTTP check-in response is the sole serialization point. Callers must not log,
    /// persist, clone, or place this value in HTML.
    pub fn expose_once(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for InstallationCredentialSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("InstallationCredentialSecret([REDACTED])")
    }
}

#[derive(Debug)]
pub struct IssuedInstallationCredential {
    pub credential_ref: Uuid,
    pub installation_ref: Uuid,
    /// High-entropy material returned to the producer once. It is never persisted.
    pub raw_credential: InstallationCredentialSecret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountEligibilityState {
    Usable,
    Cooling,
    NeedsLogin,
    Restricted,
    Unknown,
}

/// Closed producer observation vocabulary. The producer reports only what it observed; the
/// server owns the projection into an eligibility state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountEligibilitySignal {
    AuthenticatedObserved,
    CooldownObserved,
    LoginRequired,
    AccessRestricted,
    SignalIncomplete,
}

impl AccountEligibilitySignal {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "authenticated_observed" => Some(Self::AuthenticatedObserved),
            "cooldown_observed" => Some(Self::CooldownObserved),
            "login_required" => Some(Self::LoginRequired),
            "access_restricted" => Some(Self::AccessRestricted),
            "signal_incomplete" => Some(Self::SignalIncomplete),
            _ => None,
        }
    }

    pub const fn projected_state(self) -> AccountEligibilityState {
        match self {
            Self::AuthenticatedObserved => AccountEligibilityState::Usable,
            Self::CooldownObserved => AccountEligibilityState::Cooling,
            Self::LoginRequired => AccountEligibilityState::NeedsLogin,
            Self::AccessRestricted => AccountEligibilityState::Restricted,
            Self::SignalIncomplete => AccountEligibilityState::Unknown,
        }
    }
}

impl AccountEligibilityState {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "usable" => Some(Self::Usable),
            "cooling" => Some(Self::Cooling),
            "needs_login" => Some(Self::NeedsLogin),
            "restricted" => Some(Self::Restricted),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Usable => "usable",
            Self::Cooling => "cooling",
            Self::NeedsLogin => "needs_login",
            Self::Restricted => "restricted",
            Self::Unknown => "unknown",
        }
    }

    pub const fn reason_code(self) -> &'static str {
        match self {
            Self::Usable => "authenticated",
            Self::Cooling => "cooldown_observed",
            Self::NeedsLogin => "login_required",
            Self::Restricted => "access_restricted",
            Self::Unknown => "signal_incomplete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountEligibilityReceipt {
    pub account_ref: Option<Uuid>,
    pub installation_ref: Uuid,
    pub state: AccountEligibilityState,
    pub binding_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacitySelection {
    pub capacity: Capacity,
    pub station_ref: Option<Uuid>,
    pub installation_ref: Option<Uuid>,
    pub account_ref: Option<Uuid>,
    /// The exact eligibility observation used for the successful decision. It is provenance,
    /// not a current-health flag; later observations never rewrite this reference.
    pub eligibility_ref: Option<Uuid>,
}

impl CapacitySelection {
    fn blocked(reason_code: CapacityReasonCode, reason: &str) -> Self {
        Self {
            capacity: Capacity::Unavailable {
                reason_code,
                reason: reason.to_owned(),
            },
            station_ref: None,
            installation_ref: None,
            account_ref: None,
            eligibility_ref: None,
        }
    }

    fn available(
        station_ref: Uuid,
        installation_ref: Uuid,
        account_ref: Uuid,
        eligibility_ref: Option<Uuid>,
    ) -> Self {
        Self {
            capacity: Capacity::Available {
                station_ref: station_ref.to_string(),
            },
            station_ref: Some(station_ref),
            installation_ref: Some(installation_ref),
            account_ref: Some(account_ref),
            eligibility_ref,
        }
    }
}

pub async fn collection_control_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass('installation_credential') IS NOT NULL \
                AND to_regclass('platform_observation_account') IS NOT NULL \
                AND to_regclass('collection_monitor_rule_revision') IS NOT NULL \
                AND to_regclass('collection_platform_dispatch_policy') IS NOT NULL \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_name='execution_station' \
                              AND column_name='accepting_tasks') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_name='collection_work_order' \
                              AND column_name='retry_not_before_at')",
    )
    .fetch_one(database.pool())
    .await
}

/// Rotate a producer credential. The two UUIDv4 values provide 244 random bits; unlike an
/// installation key this is a purpose-built secret. Only its SHA-256 digest is persisted.
pub async fn rotate_installation_credential(
    database: &Database,
    installation_ref: Uuid,
) -> Result<IssuedInstallationCredential, CollectionControlError> {
    if !collection_control_schema_is_ready(database).await? {
        return Err(CollectionControlError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let issued = issue_installation_credential_in(&mut transaction, installation_ref).await?;
    transaction.commit().await?;
    Ok(issued)
}

/// Activate a hash-only pending issuance after the producer has durably stored its one-time raw
/// value. Replaying the acknowledgement is safe. Rotation keeps the prior active credential
/// valid until this point, so a lost rotation response cannot strand the installation.
pub async fn activate_installation_credential(
    database: &Database,
    installation_ref: Uuid,
    credential_ref: Uuid,
    raw_credential: &str,
) -> Result<(), CollectionControlError> {
    if !collection_control_schema_is_ready(database).await? {
        return Err(CollectionControlError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let row: Option<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT credential_hash,to_char(activated_at,'YYYY-MM-DD HH24:MI:SSOF'), \
                expires_at>scope_001_now() \
         FROM installation_credential \
         WHERE credential_ref=$1 AND installation_ref=$2 AND revoked_at IS NULL FOR UPDATE",
    )
    .bind(credential_ref)
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let candidate = sha256_hex(raw_credential.as_bytes());
    let expected = row
        .as_ref()
        .map(|value| value.0.as_str())
        .unwrap_or("0000000000000000000000000000000000000000000000000000000000000000");
    let comparison = constant_time_equal(candidate.as_bytes(), expected.as_bytes());
    let Some((_hash, activated_at, unexpired)) = row else {
        return Err(CollectionControlError::InvalidCredential);
    };
    if !comparison || !unexpired {
        return Err(CollectionControlError::InvalidCredential);
    }
    if activated_at.is_none() {
        sqlx::query(
            "UPDATE installation_credential SET revoked_at=scope_001_now(), \
                    revoke_reason_code='rotated' \
             WHERE installation_ref=$1 AND revoked_at IS NULL \
               AND activated_at IS NOT NULL AND credential_ref<>$2",
        )
        .bind(installation_ref)
        .bind(credential_ref)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "UPDATE installation_credential \
             SET activated_at=scope_001_now(), \
                 expires_at=scope_001_now()+make_interval(days=>$3) \
             WHERE credential_ref=$1 AND installation_ref=$2 AND activated_at IS NULL",
        )
        .bind(credential_ref)
        .bind(installation_ref)
        .bind(CREDENTIAL_VALID_FOR_DAYS)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn issue_installation_credential_if_absent_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
) -> Result<Option<IssuedInstallationCredential>, CollectionControlError> {
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM installation_credential \
         WHERE installation_ref=$1 AND revoked_at IS NULL AND activated_at IS NOT NULL \
           AND expires_at>scope_001_now())",
    )
    .bind(installation_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if active {
        return Ok(None);
    }
    issue_installation_credential_in(transaction, installation_ref)
        .await
        .map(Some)
}

async fn issue_installation_credential_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
) -> Result<IssuedInstallationCredential, CollectionControlError> {
    let plugin_version: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT plugin_version FROM plugin_installation \
         WHERE installation_ref=$1 AND superseded_at IS NULL",
    )
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(plugin_version) = plugin_version else {
        return Err(CollectionControlError::UnknownInstallation);
    };
    if !version_at_least(&plugin_version, MINIMUM_PLUGIN_VERSION) {
        return Err(CollectionControlError::UnsupportedPluginVersion);
    }

    sqlx::query(
        "UPDATE installation_credential \
         SET revoked_at=scope_001_now(),revoke_reason_code='pending_replaced' \
         WHERE installation_ref=$1 AND revoked_at IS NULL AND activated_at IS NULL",
    )
    .bind(installation_ref)
    .execute(&mut **transaction)
    .await?;

    let credential_ref = Uuid::new_v4();
    let raw_credential = format!(
        "lgi_ic_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let credential_hash = sha256_hex(raw_credential.as_bytes());
    sqlx::query(
        "INSERT INTO installation_credential \
             (credential_ref,installation_ref,credential_hash,hash_version,expires_at) \
         VALUES ($1,$2,$3,'sha256-v1',scope_001_now()+make_interval(mins=>$4))",
    )
    .bind(credential_ref)
    .bind(installation_ref)
    .bind(credential_hash)
    .bind(PENDING_CREDENTIAL_VALID_FOR_MINUTES)
    .execute(&mut **transaction)
    .await?;
    Ok(IssuedInstallationCredential {
        credential_ref,
        installation_ref,
        raw_credential: InstallationCredentialSecret(raw_credential),
    })
}

pub(crate) async fn validate_installation_credential_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    raw_credential: &str,
) -> Result<bool, sqlx::Error> {
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT credential_hash FROM installation_credential \
         WHERE installation_ref=$1 AND revoked_at IS NULL AND activated_at IS NOT NULL \
           AND expires_at>scope_001_now() ORDER BY issued_at DESC LIMIT 1",
    )
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let candidate = sha256_hex(raw_credential.as_bytes());
    // Always compare 64 bytes, including the absent-row path, so credential validation has no
    // early-return byte prefix oracle. Database lookup timing still reveals only installation
    // existence, which is not credential material.
    let expected = stored
        .as_deref()
        .unwrap_or("0000000000000000000000000000000000000000000000000000000000000000");
    let comparison = constant_time_equal(candidate.as_bytes(), expected.as_bytes());
    Ok(comparison & stored.is_some())
}

/// Authenticate check-in facts without turning a public install key into a heartbeat bearer.
/// A never-activated installation may bootstrap without a credential. Once activated history
/// exists, the last raw credential is required even after natural expiry; an explicitly revoked
/// credential cannot self-recover.
pub(crate) async fn authenticate_installation_check_in_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    raw_credential: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let latest: Option<(String, bool)> = sqlx::query_as(
        "SELECT credential_hash,revoked_at IS NOT NULL \
         FROM installation_credential \
         WHERE installation_ref=$1 AND activated_at IS NOT NULL \
         ORDER BY activated_at DESC,issued_at DESC LIMIT 1",
    )
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let candidate = sha256_hex(raw_credential.unwrap_or("").as_bytes());
    let expected = latest
        .as_ref()
        .map(|value| value.0.as_str())
        .unwrap_or("0000000000000000000000000000000000000000000000000000000000000000");
    let comparison = constant_time_equal(candidate.as_bytes(), expected.as_bytes());
    Ok(match latest {
        None => true,
        Some((_hash, revoked)) => !revoked & comparison,
    })
}

pub async fn report_account_eligibility(
    database: &Database,
    installation_ref: Uuid,
    raw_credential: &str,
    raw_platform_account_id: Option<&str>,
    signal: AccountEligibilitySignal,
    digest_key: &[u8],
) -> Result<AccountEligibilityReceipt, CollectionControlError> {
    if !collection_control_schema_is_ready(database).await? {
        return Err(CollectionControlError::SchemaUnavailable);
    }
    if digest_key.len() < 32 {
        return Err(CollectionControlError::MissingDigestKey);
    }
    let mut transaction = database.pool().begin().await?;
    let installation_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM plugin_installation \
         WHERE installation_ref=$1 AND superseded_at IS NULL)",
    )
    .bind(installation_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if !installation_exists {
        return Err(CollectionControlError::UnknownInstallation);
    }
    if !validate_installation_credential_in(&mut transaction, installation_ref, raw_credential)
        .await?
    {
        return Err(CollectionControlError::InvalidCredential);
    }

    let account_ref = if let Some(raw_platform_account_id) = raw_platform_account_id {
        let raw_platform_account_id = raw_platform_account_id.trim();
        if raw_platform_account_id.is_empty() || raw_platform_account_id.len() > 512 {
            return Err(CollectionControlError::InvalidPlatformIdentity);
        }
        let identity_digest = hmac_sha256_hex(digest_key, raw_platform_account_id.as_bytes());
        Some(
            sqlx::query_scalar(
                "INSERT INTO platform_observation_account \
                     (account_ref,platform,identity_digest,digest_version) \
                 VALUES ($1,'xhs',$2,'hmac-sha256-v1') \
                 ON CONFLICT (platform,digest_version,identity_digest) \
                 DO UPDATE SET identity_digest=EXCLUDED.identity_digest \
                 RETURNING account_ref",
            )
            .bind(Uuid::new_v4())
            .bind(identity_digest)
            .fetch_one(&mut *transaction)
            .await?,
        )
    } else {
        sqlx::query_scalar(
            "SELECT account_ref FROM platform_observation_account_binding \
             WHERE installation_ref=$1 AND ended_at IS NULL \
               AND confirmed_until>scope_001_now() LIMIT 1",
        )
        .bind(installation_ref)
        .fetch_optional(&mut *transaction)
        .await?
    };
    let Some(account_ref) = account_ref else {
        transaction.commit().await?;
        return Ok(AccountEligibilityReceipt {
            account_ref: None,
            installation_ref,
            state: AccountEligibilityState::Unknown,
            binding_required: true,
        });
    };
    let state = signal.projected_state();
    sqlx::query(
        "INSERT INTO platform_observation_account_eligibility_observation \
             (eligibility_ref,account_ref,installation_ref,eligibility_state,signal_version, \
              reason_code,expires_at) \
         VALUES ($1,$2,$3,$4,'xhs-account-eligibility-v1',$5, \
                 scope_001_now()+make_interval(mins=>$6))",
    )
    .bind(Uuid::new_v4())
    .bind(account_ref)
    .bind(installation_ref)
    .bind(state.as_str())
    .bind(state.reason_code())
    .bind(ACCOUNT_ELIGIBILITY_TTL_MINUTES)
    .execute(&mut *transaction)
    .await?;
    let binding_required: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM platform_observation_account_binding \
         WHERE account_ref=$1 AND installation_ref=$2 AND ended_at IS NULL \
           AND confirmed_until>scope_001_now())",
    )
    .bind(account_ref)
    .bind(installation_ref)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(AccountEligibilityReceipt {
        account_ref: Some(account_ref),
        installation_ref,
        state,
        binding_required,
    })
}

pub async fn bind_observation_account(
    database: &Database,
    account_ref: Uuid,
    installation_ref: Uuid,
    actor: &str,
) -> Result<Uuid, CollectionControlError> {
    if actor != "person" {
        return Err(CollectionControlError::InvalidStationActor);
    }
    if !collection_control_schema_is_ready(database).await? {
        return Err(CollectionControlError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let account_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM platform_observation_account WHERE account_ref=$1)",
    )
    .bind(account_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if !account_exists {
        return Err(CollectionControlError::UnknownAccount);
    }
    let installation_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM plugin_installation \
         WHERE installation_ref=$1 AND superseded_at IS NULL)",
    )
    .bind(installation_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if !installation_exists {
        return Err(CollectionControlError::UnknownInstallation);
    }
    sqlx::query(
        "UPDATE platform_observation_account_binding \
         SET ended_at=scope_001_now(),end_reason_code=CASE \
             WHEN installation_ref=$2 THEN 'identity_changed' ELSE 'person_unbound' END \
         WHERE ended_at IS NULL AND (account_ref=$1 OR installation_ref=$2)",
    )
    .bind(account_ref)
    .bind(installation_ref)
    .execute(&mut *transaction)
    .await?;
    let binding_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO platform_observation_account_binding \
             (binding_ref,account_ref,installation_ref,bound_by,confirmed_until) \
         VALUES ($1,$2,$3,'person',scope_001_now()+make_interval(days=>$4))",
    )
    .bind(binding_ref)
    .bind(account_ref)
    .bind(installation_ref)
    .bind(ACCOUNT_BINDING_VALID_FOR_DAYS)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(binding_ref)
}

pub async fn set_station_accepting(
    database: &Database,
    station_ref: Uuid,
    accepting: bool,
    actor: &str,
) -> Result<(), CollectionControlError> {
    if actor != "person" {
        return Err(CollectionControlError::InvalidStationActor);
    }
    if !collection_control_schema_is_ready(database).await? {
        return Err(CollectionControlError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    let previous: Option<bool> = sqlx::query_scalar(
        "SELECT accepting_tasks FROM execution_station \
         WHERE station_ref=$1 AND retired_at IS NULL FOR UPDATE",
    )
    .bind(station_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(previous) = previous else {
        return Err(CollectionControlError::UnknownStation);
    };
    if previous != accepting {
        sqlx::query("UPDATE execution_station SET accepting_tasks=$2 WHERE station_ref=$1")
            .bind(station_ref)
            .bind(accepting)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "INSERT INTO execution_station_acceptance_transition \
                 (transition_ref,station_ref,from_accepting,to_accepting,actor,reason_code) \
             VALUES ($1,$2,$3,$4,'person',$5)",
        )
        .bind(Uuid::new_v4())
        .bind(station_ref)
        .bind(previous)
        .bind(accepting)
        .bind(if accepting {
            "person_enabled"
        } else {
            "person_disabled"
        })
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// The only capacity evaluator. All callers receive the same stable reason code and the exact
/// station/installation/account triple frozen by a successful decision.
type CapacityCandidate = (
    Uuid,
    Uuid,
    bool,
    String,
    Value,
    i32,
    bool,
    bool,
    Option<Uuid>,
    bool,
    Option<String>,
    bool,
    bool,
);

pub(crate) async fn evaluate_capacity_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    target_kind: &str,
    lane: &str,
    required_capabilities: &[&str],
    target_ref: Option<Uuid>,
) -> Result<CapacitySelection, sqlx::Error> {
    let risk_pause: Option<String> = sqlx::query_scalar(
        "SELECT reason FROM collection_risk_pause WHERE lifted_at IS NULL \
           AND (platform IS NULL OR platform=$1) AND (lane IS NULL OR lane=$2) \
         ORDER BY paused_at DESC LIMIT 1",
    )
    .bind(platform)
    .bind(lane)
    .fetch_optional(&mut **transaction)
    .await?;
    if risk_pause.is_some() {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::RiskPaused,
            "风险暂停仍在生效，当前不允许创建新的执行许可。",
        ));
    }

    if let Some((active, cap)) =
        platform_dispatch_capacity_in(transaction, platform, None, false).await?
    {
        if active >= i64::from(cap) {
            return Ok(CapacitySelection::blocked(
                CapacityReasonCode::PlatformConcurrentLimitReached,
                "平台当前活跃 Lease 已达到并发上限，等待任一执行许可释放。",
            ));
        }
    } else {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::CapacityUnknown,
            "平台并发策略缺失，控制层按关闭处理。",
        ));
    }

    let last_failed_station: Option<Uuid> = sqlx::query_scalar(
        "SELECT work_order.station_ref FROM collection_work_order work_order \
         JOIN collection_work_order_lease lease USING(work_order_ref) \
         WHERE $1::uuid IS NOT NULL AND work_order.target_ref=$1 AND work_order.lane=$2 \
           AND lease.release_reason IN ('expired','station_unavailable','revoked') \
         ORDER BY lease.released_at DESC LIMIT 1",
    )
    .bind(target_ref)
    .bind(lane)
    .fetch_optional(&mut **transaction)
    .await?
    .flatten();
    let mut candidates: Vec<CapacityCandidate> = sqlx::query_as(
        "SELECT s.station_ref,i.installation_ref,s.accepting_tasks,i.plugin_version, \
                i.capabilities,s.daily_work_quota, \
                i.last_seen_at>=scope_001_now()-make_interval(mins=>$1), \
                EXISTS (SELECT 1 FROM installation_credential credential \
                        WHERE credential.installation_ref=i.installation_ref \
                          AND credential.revoked_at IS NULL \
                          AND credential.activated_at IS NOT NULL \
                          AND credential.expires_at>scope_001_now()), \
                binding.account_ref,COALESCE(binding.confirmed_until>scope_001_now(),false), \
                eligibility.eligibility_state, \
                COALESCE(eligibility.expires_at>scope_001_now(),false), \
                CASE WHEN binding.account_ref IS NULL THEN false ELSE EXISTS ( \
                    SELECT 1 FROM collection_work_order work_order \
                    JOIN collection_work_order_lease lease \
                      ON lease.work_order_ref=work_order.work_order_ref \
                    WHERE work_order.account_ref=binding.account_ref \
                      AND lease.released_at IS NULL AND lease.expires_at>scope_001_now()) END \
         FROM execution_station s \
         JOIN plugin_installation i ON i.station_ref=s.station_ref AND i.superseded_at IS NULL \
         LEFT JOIN LATERAL ( \
             SELECT candidate.account_ref,candidate.confirmed_until \
             FROM platform_observation_account_binding candidate \
             WHERE candidate.installation_ref=i.installation_ref AND candidate.ended_at IS NULL \
             LIMIT 1) binding ON true \
         LEFT JOIN LATERAL ( \
             SELECT observation.eligibility_state,observation.expires_at \
             FROM platform_observation_account_eligibility_observation observation \
             WHERE observation.account_ref=binding.account_ref \
               AND observation.installation_ref=i.installation_ref \
             ORDER BY observation.observed_at DESC LIMIT 1) eligibility ON true \
         WHERE s.retired_at IS NULL ORDER BY s.registered_at,s.station_ref",
    )
    .bind(CONTROL_FRESHNESS_MINUTES)
    .fetch_all(&mut **transaction)
    .await?;
    candidates.sort_by_key(|candidate| Some(candidate.0) == last_failed_station);
    if candidates.is_empty() {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationUnavailable,
            "没有任何登记工位拥有在岗安装。",
        ));
    }

    let mut first_block: Option<CapacitySelection> = None;
    for candidate in candidates {
        if let Some((code, reason)) =
            candidate_block_reason(&candidate, required_capabilities, None, true)
        {
            first_block.get_or_insert_with(|| CapacitySelection::blocked(code, reason));
            continue;
        }
        let station_ref = candidate.0;
        let installation_ref = candidate.1;
        let daily_quota = candidate.5;
        let account_ref = candidate.8.expect("candidate policy checked account");
        let used = station_daily_note_usage_in(transaction, station_ref).await?;
        if used >= i64::from(daily_quota) {
            first_block.get_or_insert_with(|| {
                CapacitySelection::blocked(
                    CapacityReasonCode::StationDailyBudgetReached,
                    "工位当天已接纳 200 篇，等待自然日预算恢复。",
                )
            });
            continue;
        }
        let eligibility_ref: Option<Uuid> = sqlx::query_scalar(
            "SELECT eligibility_ref FROM platform_observation_account_eligibility_observation \
             WHERE account_ref=$1 AND installation_ref=$2 ORDER BY observed_at DESC LIMIT 1",
        )
        .bind(account_ref)
        .bind(installation_ref)
        .fetch_optional(&mut **transaction)
        .await?;
        return Ok(CapacitySelection::available(
            station_ref,
            installation_ref,
            account_ref,
            eligibility_ref,
        ));
    }
    Ok(first_block.unwrap_or_else(|| {
        CapacitySelection::blocked(
            CapacityReasonCode::CapacityUnknown,
            &format!("{platform}/{target_kind}/{lane} 产能状态无法确定，按关闭处理。"),
        )
    }))
}

/// Revalidate one already-frozen control triple. It never substitutes a different station,
/// installation or account. `current_lease_ref` is excluded from the busy test so dispatch does
/// not reject the very Lease it is revalidating.
pub(crate) async fn revalidate_frozen_capacity_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    lane: &str,
    required_capabilities: &[&str],
    station_ref: Uuid,
    installation_ref: Uuid,
    account_ref: Uuid,
    current_lease_ref: Option<Uuid>,
    require_accepting_tasks: bool,
) -> Result<CapacitySelection, sqlx::Error> {
    let risk_pause: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM collection_risk_pause WHERE lifted_at IS NULL \
           AND (platform IS NULL OR platform=$1) AND (lane IS NULL OR lane=$2))",
    )
    .bind(platform)
    .bind(lane)
    .fetch_one(&mut **transaction)
    .await?;
    if risk_pause {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::RiskPaused,
            "风险暂停仍在生效，当前 Lease 不再允许派发。",
        ));
    }
    // A policy-row lock serializes every new Lease across stations and Runtime
    // processes.  `current_lease_ref` is excluded during revalidation so the
    // live lease being replayed does not consume a second slot.
    if let Some((active, cap)) =
        platform_dispatch_capacity_in(transaction, platform, current_lease_ref, true).await?
    {
        if active >= i64::from(cap) {
            return Ok(CapacitySelection::blocked(
                CapacityReasonCode::PlatformConcurrentLimitReached,
                "平台当前活跃 Lease 已达到并发上限，等待任一执行许可释放。",
            ));
        }
    } else {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::CapacityUnknown,
            "平台并发策略缺失，控制层按关闭处理。",
        ));
    }
    let candidate: Option<CapacityCandidate> = sqlx::query_as(
        "SELECT s.station_ref,i.installation_ref,s.accepting_tasks,i.plugin_version, \
                i.capabilities,s.daily_work_quota, \
                i.last_seen_at>=scope_001_now()-make_interval(mins=>$1), \
                EXISTS (SELECT 1 FROM installation_credential credential \
                        WHERE credential.installation_ref=i.installation_ref \
                          AND credential.revoked_at IS NULL \
                          AND credential.activated_at IS NOT NULL \
                          AND credential.expires_at>scope_001_now()), \
                binding.account_ref,COALESCE(binding.confirmed_until>scope_001_now(),false), \
                eligibility.eligibility_state, \
                COALESCE(eligibility.expires_at>scope_001_now(),false), \
                CASE WHEN binding.account_ref IS NULL THEN false ELSE EXISTS ( \
                    SELECT 1 FROM collection_work_order busy_work \
                    JOIN collection_work_order_lease busy_lease \
                      ON busy_lease.work_order_ref=busy_work.work_order_ref \
                    WHERE busy_work.account_ref=binding.account_ref \
                      AND busy_lease.released_at IS NULL \
                      AND busy_lease.expires_at>scope_001_now() \
                      AND ($4::uuid IS NULL OR busy_lease.lease_ref<>$4)) END \
         FROM execution_station s \
         JOIN plugin_installation i ON i.station_ref=s.station_ref AND i.superseded_at IS NULL \
         LEFT JOIN LATERAL ( \
             SELECT candidate.account_ref,candidate.confirmed_until \
             FROM platform_observation_account_binding candidate \
             WHERE candidate.installation_ref=i.installation_ref AND candidate.ended_at IS NULL \
             LIMIT 1) binding ON true \
         LEFT JOIN LATERAL ( \
             SELECT observation.eligibility_state,observation.expires_at \
             FROM platform_observation_account_eligibility_observation observation \
             WHERE observation.account_ref=binding.account_ref \
               AND observation.installation_ref=i.installation_ref \
             ORDER BY observation.observed_at DESC LIMIT 1) eligibility ON true \
         WHERE s.retired_at IS NULL AND s.station_ref=$2 AND i.installation_ref=$3",
    )
    .bind(CONTROL_FRESHNESS_MINUTES)
    .bind(station_ref)
    .bind(installation_ref)
    .bind(current_lease_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(candidate) = candidate else {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationUnavailable,
            "工单冻结的工位或安装已不在岗。",
        ));
    };
    if let Some((code, reason)) = candidate_block_reason(
        &candidate,
        required_capabilities,
        Some(account_ref),
        require_accepting_tasks,
    ) {
        return Ok(CapacitySelection::blocked(code, reason));
    }
    let used = station_daily_note_usage_in(transaction, station_ref).await?;
    if used >= i64::from(candidate.5) {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationDailyBudgetReached,
            "工位当天已接纳 200 篇，等待自然日预算恢复。",
        ));
    }
    let eligibility_ref: Option<Uuid> = sqlx::query_scalar(
        "SELECT eligibility_ref FROM platform_observation_account_eligibility_observation \
         WHERE account_ref=$1 AND installation_ref=$2 ORDER BY observed_at DESC LIMIT 1",
    )
    .bind(account_ref)
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    Ok(CapacitySelection::available(
        station_ref,
        installation_ref,
        account_ref,
        eligibility_ref,
    ))
}

/// Evaluate a *specific* plugin installation as a Work-Order claimant. This
/// is deliberately separate from admission's old “pick any healthy station”
/// lookup: a queue consumer must never silently substitute another station for
/// the plugin that made the claim request.
pub(crate) async fn evaluate_claiming_installation_capacity_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
    platform: &str,
    lane: &str,
    required_capabilities: &[&str],
) -> Result<CapacitySelection, sqlx::Error> {
    let claimant: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT i.station_ref,binding.account_ref \
         FROM plugin_installation i \
         LEFT JOIN LATERAL ( \
             SELECT account_ref FROM platform_observation_account_binding \
             WHERE installation_ref=i.installation_ref AND ended_at IS NULL \
             LIMIT 1) binding ON true \
         WHERE i.installation_ref=$1 AND i.superseded_at IS NULL",
    )
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await?;
    let Some((Some(station_ref), Some(account_ref))) = claimant else {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationUnavailable,
            "当前插件安装没有可用工位或已绑定观察账号。",
        ));
    };
    revalidate_frozen_capacity_in(
        transaction,
        platform,
        lane,
        required_capabilities,
        station_ref,
        installation_ref,
        account_ref,
        None,
        true,
    )
    .await
}

/// Return `(live_leases, cap)` for one platform.  A caller which is about to
/// issue a Lease must request `lock_policy=true`; the policy row then provides
/// one database-wide serialization point for all account/station combinations.
/// A plain admission read can use the same source of truth without holding the
/// policy lock for longer than its transaction.
async fn platform_dispatch_capacity_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    current_lease_ref: Option<Uuid>,
    lock_policy: bool,
) -> Result<Option<(i64, i32)>, sqlx::Error> {
    let cap: Option<i32> = if lock_policy {
        sqlx::query_scalar(
            "SELECT concurrent_cap FROM collection_platform_dispatch_policy \
             WHERE platform=$1 FOR UPDATE",
        )
        .bind(platform)
        .fetch_optional(&mut **transaction)
        .await?
    } else {
        sqlx::query_scalar(
            "SELECT concurrent_cap FROM collection_platform_dispatch_policy WHERE platform=$1",
        )
        .bind(platform)
        .fetch_optional(&mut **transaction)
        .await?
    };
    let Some(cap) = cap else {
        return Ok(None);
    };
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_lease lease \
         JOIN collection_work_order work_order USING(work_order_ref) \
         JOIN collection_observation_target target USING(target_ref) \
         WHERE target.platform=$1 AND lease.released_at IS NULL \
           AND lease.expires_at>scope_001_now() \
           AND ($2::uuid IS NULL OR lease.lease_ref<>$2)",
    )
    .bind(platform)
    .bind(current_lease_ref)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(Some((active, cap)))
}

/// Count currently eligible *idle* claimants for bounded batch generation.
/// This is deliberately a capacity read, not a reservation: actual ownership
/// is still established only by `revalidate_frozen_capacity_in` during Lease
/// issuance.  The count prevents a batch source from producing an unbounded
/// backlog while keeping enough ready work for the current station fleet.
pub(crate) async fn ready_batch_claim_slots_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    platform: &str,
    required_capabilities: &[&str],
) -> Result<(i64, i32), sqlx::Error> {
    let multiplier: Option<i32> = sqlx::query_scalar(
        "SELECT ready_work_multiplier FROM collection_dispatch_lane_fairness \
         WHERE dispatch_lane='batch'",
    )
    .fetch_optional(&mut **transaction)
    .await?;
    let Some(multiplier) = multiplier else {
        return Ok((0, 1));
    };
    let Some((active, cap)) =
        platform_dispatch_capacity_in(transaction, platform, None, false).await?
    else {
        return Ok((0, multiplier));
    };
    let platform_remaining = (i64::from(cap) - active).max(0);
    if platform_remaining == 0 {
        return Ok((0, multiplier));
    }
    let candidates: Vec<CapacityCandidate> = sqlx::query_as(
        "SELECT s.station_ref,i.installation_ref,s.accepting_tasks,i.plugin_version, \
                i.capabilities,s.daily_work_quota, \
                i.last_seen_at>=scope_001_now()-make_interval(mins=>$1), \
                EXISTS (SELECT 1 FROM installation_credential credential \
                        WHERE credential.installation_ref=i.installation_ref \
                          AND credential.revoked_at IS NULL \
                          AND credential.activated_at IS NOT NULL \
                          AND credential.expires_at>scope_001_now()), \
                binding.account_ref,COALESCE(binding.confirmed_until>scope_001_now(),false), \
                eligibility.eligibility_state, \
                COALESCE(eligibility.expires_at>scope_001_now(),false), \
                CASE WHEN binding.account_ref IS NULL THEN false ELSE EXISTS ( \
                    SELECT 1 FROM collection_work_order work_order \
                    JOIN collection_work_order_lease lease \
                      ON lease.work_order_ref=work_order.work_order_ref \
                    WHERE work_order.account_ref=binding.account_ref \
                      AND lease.released_at IS NULL AND lease.expires_at>scope_001_now()) END \
         FROM execution_station s \
         JOIN plugin_installation i ON i.station_ref=s.station_ref AND i.superseded_at IS NULL \
         LEFT JOIN LATERAL ( \
             SELECT candidate.account_ref,candidate.confirmed_until \
             FROM platform_observation_account_binding candidate \
             WHERE candidate.installation_ref=i.installation_ref AND candidate.ended_at IS NULL \
             LIMIT 1) binding ON true \
         LEFT JOIN LATERAL ( \
             SELECT observation.eligibility_state,observation.expires_at \
             FROM platform_observation_account_eligibility_observation observation \
             WHERE observation.account_ref=binding.account_ref \
               AND observation.installation_ref=i.installation_ref \
             ORDER BY observation.observed_at DESC LIMIT 1) eligibility ON true \
         WHERE s.retired_at IS NULL",
    )
    .bind(CONTROL_FRESHNESS_MINUTES)
    .fetch_all(&mut **transaction)
    .await?;
    let mut ready = 0_i64;
    for candidate in candidates {
        if candidate_block_reason(&candidate, required_capabilities, None, true).is_some() {
            continue;
        }
        if station_daily_note_usage_in(transaction, candidate.0).await? >= i64::from(candidate.5) {
            continue;
        }
        ready += 1;
    }
    Ok((ready.min(platform_remaining), multiplier))
}

fn candidate_block_reason(
    candidate: &CapacityCandidate,
    required_capabilities: &[&str],
    frozen_account_ref: Option<Uuid>,
    require_accepting_tasks: bool,
) -> Option<(CapacityReasonCode, &'static str)> {
    if require_accepting_tasks && !candidate.2 {
        Some((
            CapacityReasonCode::StationNotAccepting,
            "工位已由人显式暂停未来接活。",
        ))
    } else if !candidate.7 {
        Some((
            CapacityReasonCode::InstallationCredentialMissing,
            "在岗安装没有有效的服务端凭据。",
        ))
    } else if !version_at_least(&candidate.3, MINIMUM_PLUGIN_VERSION) {
        Some((
            CapacityReasonCode::PluginVersionUnsupported,
            "插件版本低于 0.8.34，不能执行当前合同。",
        ))
    } else if !candidate.6 {
        Some((
            CapacityReasonCode::InstallationStale,
            "插件心跳超过 20 分钟，按失联处理。",
        ))
    } else if !capabilities_cover(&candidate.4, required_capabilities) {
        Some((
            CapacityReasonCode::CapabilityMissing,
            "在岗安装缺少本次 lane 所需能力。",
        ))
    } else if candidate.8.is_none() {
        Some((
            CapacityReasonCode::AccountUnbound,
            "安装尚未由人绑定到一个观察账号。",
        ))
    } else if frozen_account_ref.is_some_and(|expected| candidate.8 != Some(expected)) {
        Some((
            CapacityReasonCode::AccountBindingChanged,
            "安装当前账号绑定与工单冻结账号不一致。",
        ))
    } else if !candidate.9 {
        Some((
            CapacityReasonCode::AccountBindingExpired,
            "观察账号人工确认已超过 30 天，需重新确认绑定。",
        ))
    } else if candidate.10.is_none() || !candidate.11 {
        Some((
            CapacityReasonCode::AccountEligibilityStale,
            "观察账号资格未上报或已超过 20 分钟有效期。",
        ))
    } else {
        match candidate.10.as_deref() {
            Some("usable") if candidate.12 => Some((
                CapacityReasonCode::AccountBusy,
                "观察账号已有一份有效 Lease。",
            )),
            Some("usable") => None,
            Some("cooling") => Some((
                CapacityReasonCode::AccountCooling,
                "观察账号当前处于冷却状态。",
            )),
            Some("needs_login") => Some((
                CapacityReasonCode::AccountNeedsLogin,
                "观察账号需要重新登录。",
            )),
            Some("restricted") => Some((
                CapacityReasonCode::AccountRestricted,
                "观察账号当前受到访问限制。",
            )),
            _ => Some((
                CapacityReasonCode::AccountUnknown,
                "观察账号资格为 UNKNOWN，控制层按关闭处理。",
            )),
        }
    }
}

fn capabilities_cover(capabilities: &Value, required: &[&str]) -> bool {
    let Some(declared) = capabilities.as_array() else {
        return false;
    };
    required.iter().all(|needed| {
        declared
            .iter()
            .any(|candidate| candidate.as_str() == Some(*needed))
    })
}

pub(crate) fn required_capabilities_for(
    target_kind: &str,
    lane: &str,
    material_scope: bool,
    collects_comments: bool,
    collects_replies: bool,
    acquire_media: bool,
) -> Vec<&'static str> {
    if material_scope {
        // A frozen material scope is not automatically a request for every detail-adjacent
        // capability. `comment_limit = 0` means that only the note detail was approved, so a
        // detail-only station must be eligible to take it even when it cannot read comments.
        let mut required = vec!["content_detail"];
        if collects_comments || collects_replies {
            required.push("comments");
        }
        if collects_replies {
            required.push("replies");
        }
        if acquire_media {
            required.push("media_slots");
        }
        return required;
    }
    match (target_kind, lane) {
        ("creator", "deep_archive") => vec!["author_profile", "profile_discovery"],
        ("creator", "patrol") => vec!["author_profile", "profile_discovery"],
        ("keyword", "deep_archive") => vec!["discovery_search"],
        ("creator", _) => vec!["profile_discovery"],
        _ => vec!["discovery_search"],
    }
}

pub fn version_at_least(candidate: &str, minimum: &str) -> bool {
    fn parse(value: &str) -> Option<(u32, u32, u32)> {
        let value = value.trim().trim_start_matches('v');
        if value.contains('-') {
            return None;
        }
        let core = value.split('+').next()?;
        let mut parts = core.split('.');
        let parsed = (
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        );
        parts.next().is_none().then_some(parsed)
    }
    matches!((parse(candidate), parse(minimum)), (Some(left), Some(right)) if left >= right)
}

fn sha256_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    hex_bytes(&digest)
}

fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut normalized = [0_u8; BLOCK];
    if key.len() > BLOCK {
        normalized[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        normalized[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36_u8; BLOCK];
    let mut outer_pad = [0x5c_u8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= normalized[index];
        outer_pad[index] ^= normalized[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    hex_bytes(&outer.finalize())
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparableObservationRound {
    pub observation_round_ref: Uuid,
    pub rule_revision_ref: Uuid,
    pub publication_epoch_seconds: i64,
    pub exact_publication_time: bool,
    pub accepted_receipt: bool,
    pub coverage_qualified: bool,
    pub surface_key: String,
    pub ranking_key: Option<String>,
    pub task_contract_version: String,
    pub account_ref: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicCadence {
    Available { interval_seconds: i32 },
    Unavailable { reason_code: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorRuleMode {
    ManualOnly,
    Fixed,
    Dynamic,
}

impl MonitorRuleMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ManualOnly => "manual_only",
            Self::Fixed => "fixed",
            Self::Dynamic => "dynamic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorCommandKind {
    SaveRule,
    Pause,
    Resume,
    Stop,
    ManualObserve,
}

impl MonitorCommandKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SaveRule => "save_rule",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Stop => "stop",
            Self::ManualObserve => "manual_observe",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorRuleDraft {
    pub mode: MonitorRuleMode,
    pub automatic_enabled: bool,
    pub run_on_weekdays: bool,
    pub run_on_weekends: bool,
    pub all_day: bool,
    pub window_start_minute: Option<i16>,
    pub window_end_minute: Option<i16>,
    pub fixed_interval_seconds: Option<i32>,
    pub fallback_interval_seconds: i32,
    pub surface_key: String,
    pub ranking_key: Option<String>,
    pub task_contract_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleCommand {
    pub target_ref: Uuid,
    pub expected_revision: i32,
    pub idempotency_key: Uuid,
    pub kind: MonitorCommandKind,
    pub actor: MonitorCommandActor,
    pub source: &'static str,
    pub draft: Option<MonitorRuleDraft>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorCommandActor {
    Person,
    System,
}

impl MonitorCommandActor {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorCommandOutcomeKind {
    Applied,
    Replay,
    StaleRevision,
    IdentityConflict,
    Rejected,
}

impl MonitorCommandOutcomeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Replay => "replay",
            Self::StaleRevision => "stale_revision",
            Self::IdentityConflict => "identity_conflict",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleCommandReceipt {
    pub receipt_ref: Uuid,
    pub outcome: MonitorCommandOutcomeKind,
    pub reason_code: &'static str,
    pub current_revision: i32,
    pub applied_rule_revision_ref: Option<Uuid>,
    /// Present only for an applied or replayed manual observation command.
    pub work_order_ref: Option<Uuid>,
    /// Present only for an applied or replayed manual observation command.
    pub lease_ref: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum MonitorRuleCommandError {
    #[error("collection monitor rule schema is not applied")]
    SchemaUnavailable,
    #[error("no observation target with that reference")]
    UnknownTarget,
    #[error("manual observation command expected")]
    InvalidManualObserveCommand,
    #[error("manual observation was not admitted: {reason_code}")]
    ManualObserveNotAdmitted { reason_code: String },
    #[error(transparent)]
    Acquisition(#[from] AcquisitionChainError),
    #[error(transparent)]
    Lease(#[from] LeaseError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn apply_monitor_rule_command(
    database: &Database,
    command: &MonitorRuleCommand,
) -> Result<MonitorRuleCommandReceipt, MonitorRuleCommandError> {
    if command.kind == MonitorCommandKind::ManualObserve {
        return apply_manual_observe_command(database, command, 30).await;
    }
    if !collection_control_schema_is_ready(database).await? {
        return Err(MonitorRuleCommandError::SchemaUnavailable);
    }
    let payload_digest = monitor_command_digest(command);
    let mut transaction = database.pool().begin().await?;
    let target: Option<(String, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT target_kind,lifecycle_state,active_monitor_rule_revision_ref \
         FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(command.target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((target_kind, lifecycle_state, active_rule_ref)) = target else {
        return Err(MonitorRuleCommandError::UnknownTarget);
    };
    let current_revision: i32 = if let Some(active_rule_ref) = active_rule_ref {
        sqlx::query_scalar(
            "SELECT revision FROM collection_monitor_rule_revision \
             WHERE target_ref=$1 AND rule_revision_ref=$2",
        )
        .bind(command.target_ref)
        .bind(active_rule_ref)
        .fetch_one(&mut *transaction)
        .await?
    } else {
        0
    };

    let existing: Option<(
        Uuid,
        String,
        String,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
    )> = sqlx::query_as(
        "SELECT command_identity_ref,payload_digest,first_outcome,first_reason_code, \
                applied_rule_revision_ref,work_order_ref,lease_ref \
         FROM collection_monitor_rule_command_identity \
         WHERE target_ref=$1 AND idempotency_key=$2 FOR UPDATE",
    )
    .bind(command.target_ref)
    .bind(command.idempotency_key)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((
        identity_ref,
        existing_digest,
        _first_outcome,
        first_reason,
        applied_rule_ref,
        work_order_ref,
        lease_ref,
    )) = existing
    {
        let (outcome, reason_code) = if existing_digest == payload_digest {
            (
                MonitorCommandOutcomeKind::Replay,
                closed_monitor_reason(&first_reason),
            )
        } else {
            (
                MonitorCommandOutcomeKind::IdentityConflict,
                "identity_conflict",
            )
        };
        let receipt_ref = insert_monitor_command_receipt(
            &mut transaction,
            Some(identity_ref),
            command,
            &payload_digest,
            outcome,
            reason_code,
            applied_rule_ref,
            work_order_ref,
            lease_ref,
        )
        .await?;
        transaction.commit().await?;
        return Ok(MonitorRuleCommandReceipt {
            receipt_ref,
            outcome,
            reason_code,
            current_revision,
            applied_rule_revision_ref: applied_rule_ref,
            work_order_ref,
            lease_ref,
        });
    }

    if command.expected_revision != current_revision {
        return finish_monitor_command(
            transaction,
            command,
            &payload_digest,
            "stale_revision",
            MonitorCommandOutcomeKind::StaleRevision,
            current_revision,
            None,
            None,
            None,
        )
        .await;
    }

    let validation = validate_monitor_command(command, active_rule_ref, &lifecycle_state);
    if let Err(reason_code) = validation {
        return finish_monitor_command(
            transaction,
            command,
            &payload_digest,
            reason_code,
            MonitorCommandOutcomeKind::Rejected,
            current_revision,
            None,
            None,
            None,
        )
        .await;
    }

    let next_revision = current_revision + 1;
    let rule_revision_ref = Uuid::new_v4();
    match command.kind {
        MonitorCommandKind::SaveRule => {
            let draft = command.draft.as_ref().expect("validated save-rule draft");
            insert_monitor_rule_revision(
                &mut transaction,
                command.target_ref,
                rule_revision_ref,
                next_revision,
                draft,
                &payload_digest,
                command.actor,
            )
            .await?;
        }
        MonitorCommandKind::Pause | MonitorCommandKind::Resume | MonitorCommandKind::Stop => {
            let automatic_enabled = matches!(command.kind, MonitorCommandKind::Resume);
            copy_monitor_rule_revision(
                &mut transaction,
                command.target_ref,
                active_rule_ref.expect("validated active rule"),
                rule_revision_ref,
                next_revision,
                automatic_enabled,
                None,
                &payload_digest,
                command.actor,
            )
            .await?;
        }
        MonitorCommandKind::ManualObserve => {
            // Manual execution is created through the ordinary Request→Admission→Lease chain.
            // This command layer only establishes its idempotent control receipt; the caller
            // performs the execution transaction and records created/reused before surfacing it.
        }
    }

    let applied_rule_ref =
        (!matches!(command.kind, MonitorCommandKind::ManualObserve)).then_some(rule_revision_ref);
    if let Some(applied_rule_ref) = applied_rule_ref {
        let automatic_enabled = match command.kind {
            MonitorCommandKind::SaveRule => command
                .draft
                .as_ref()
                .is_some_and(|draft| draft.automatic_enabled),
            MonitorCommandKind::Resume => true,
            MonitorCommandKind::Pause | MonitorCommandKind::Stop => false,
            MonitorCommandKind::ManualObserve => false,
        };
        let interval_seconds = match command.kind {
            MonitorCommandKind::SaveRule => command
                .draft
                .as_ref()
                .and_then(|draft| draft.fixed_interval_seconds)
                .unwrap_or(DEFAULT_MONITOR_INTERVAL_SECONDS),
            MonitorCommandKind::Resume => {
                sqlx::query_scalar::<_, i32>(
                    "SELECT COALESCE(fixed_interval_seconds,fallback_interval_seconds) \
                 FROM collection_monitor_rule_revision WHERE rule_revision_ref=$1",
                )
                .bind(applied_rule_ref)
                .fetch_one(&mut *transaction)
                .await?
            }
            MonitorCommandKind::Pause
            | MonitorCommandKind::Stop
            | MonitorCommandKind::ManualObserve => DEFAULT_MONITOR_INTERVAL_SECONDS,
        };
        let schedule_slot_seconds = automatic_enabled
            .then(|| monitor_schedule_slot_seconds(command.target_ref, interval_seconds))
            .unwrap_or(0);
        sqlx::query(
            "UPDATE collection_observation_target \
             SET active_monitor_rule_revision_ref=$2,monitoring_enabled=$3, \
                 patrol_interval_seconds=COALESCE((SELECT CASE \
                     WHEN mode='fixed' THEN fixed_interval_seconds \
                     ELSE fallback_interval_seconds END \
                     FROM collection_monitor_rule_revision WHERE rule_revision_ref=$2),86400), \
                 monitor_schedule_anchor_at=CASE WHEN $3 THEN scope_001_now() ELSE NULL END, \
                 monitor_schedule_slot_seconds=CASE WHEN $3 THEN $4 ELSE 0 END, \
                 monitor_next_run_at=CASE WHEN $3 THEN scope_001_now() + make_interval(secs => \
                     COALESCE((SELECT fixed_interval_seconds \
                       FROM collection_monitor_rule_revision WHERE rule_revision_ref=$2),86400) + $4) \
                     ELSE NULL END, \
                 monitor_missed_run_count=0 \
             WHERE target_ref=$1",
        )
        .bind(command.target_ref)
        .bind(applied_rule_ref)
        .bind(automatic_enabled)
        .bind(schedule_slot_seconds)
        .execute(&mut *transaction)
        .await?;
        apply_monitor_lifecycle_transition(
            &mut transaction,
            command.target_ref,
            &target_kind,
            &lifecycle_state,
            command.kind,
            automatic_enabled,
            rule_revision_ref,
            command.actor,
        )
        .await?;
    }
    finish_monitor_command(
        transaction,
        command,
        &payload_digest,
        match command.kind {
            MonitorCommandKind::SaveRule => "rule_saved",
            MonitorCommandKind::Pause => "monitor_paused",
            MonitorCommandKind::Resume => "monitor_resumed",
            MonitorCommandKind::Stop => "monitor_stopped",
            MonitorCommandKind::ManualObserve => "manual_observe_created",
        },
        MonitorCommandOutcomeKind::Applied,
        next_revision,
        applied_rule_ref,
        None,
        None,
    )
    .await
}

/// Stable scheduling phase derived directly from the target UUID.  Rust's
/// default hash is intentionally process-randomized, so it must not be used
/// for a value persisted in a Rule's scheduling contract.
fn monitor_schedule_slot_seconds(target_ref: Uuid, interval_seconds: i32) -> i32 {
    debug_assert!(interval_seconds > 0);
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&target_ref.as_bytes()[..8]);
    let phase = u64::from_be_bytes(prefix) % u64::try_from(interval_seconds).unwrap_or(1);
    i32::try_from(phase).unwrap_or(0)
}

/// Create or reuse one manual patrol under the same transaction as its idempotent command
/// receipt. Pausing automatic observation does not remove this person-owned path; dismissal does.
pub async fn apply_manual_observe_command(
    database: &Database,
    command: &MonitorRuleCommand,
    _valid_for_minutes: i32,
) -> Result<MonitorRuleCommandReceipt, MonitorRuleCommandError> {
    if command.kind != MonitorCommandKind::ManualObserve {
        return Err(MonitorRuleCommandError::InvalidManualObserveCommand);
    }
    if !collection_control_schema_is_ready(database).await? {
        return Err(MonitorRuleCommandError::SchemaUnavailable);
    }
    let payload_digest = monitor_command_digest(command);
    let mut transaction = database.pool().begin().await?;
    let target: Option<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT lifecycle_state,active_monitor_rule_revision_ref \
         FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(command.target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((lifecycle_state, active_rule_ref)) = target else {
        return Err(MonitorRuleCommandError::UnknownTarget);
    };
    let current_revision: i32 = if let Some(active_rule_ref) = active_rule_ref {
        sqlx::query_scalar(
            "SELECT revision FROM collection_monitor_rule_revision \
             WHERE target_ref=$1 AND rule_revision_ref=$2",
        )
        .bind(command.target_ref)
        .bind(active_rule_ref)
        .fetch_one(&mut *transaction)
        .await?
    } else {
        0
    };
    let existing: Option<(Uuid, String, String, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT command_identity_ref,payload_digest,first_reason_code,work_order_ref,lease_ref \
         FROM collection_monitor_rule_command_identity \
         WHERE target_ref=$1 AND idempotency_key=$2 FOR UPDATE",
    )
    .bind(command.target_ref)
    .bind(command.idempotency_key)
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some((identity_ref, existing_digest, first_reason, work_order_ref, lease_ref)) = existing
    {
        let (outcome, reason_code) = if existing_digest == payload_digest {
            (
                MonitorCommandOutcomeKind::Replay,
                closed_monitor_reason(&first_reason),
            )
        } else {
            (
                MonitorCommandOutcomeKind::IdentityConflict,
                "identity_conflict",
            )
        };
        let receipt_ref = insert_monitor_command_receipt(
            &mut transaction,
            Some(identity_ref),
            command,
            &payload_digest,
            outcome,
            reason_code,
            None,
            work_order_ref,
            lease_ref,
        )
        .await?;
        transaction.commit().await?;
        return Ok(MonitorRuleCommandReceipt {
            receipt_ref,
            outcome,
            reason_code,
            current_revision,
            applied_rule_revision_ref: None,
            work_order_ref,
            lease_ref,
        });
    }
    if command.expected_revision != current_revision {
        return finish_monitor_command(
            transaction,
            command,
            &payload_digest,
            "stale_revision",
            MonitorCommandOutcomeKind::StaleRevision,
            current_revision,
            None,
            None,
            None,
        )
        .await;
    }
    if command.actor != MonitorCommandActor::Person
        || !matches!(command.source, "targets_ui" | "local_api")
        || command.draft.is_some()
        || lifecycle_state == "dismissed"
    {
        return finish_monitor_command(
            transaction,
            command,
            &payload_digest,
            "target_not_requestable",
            MonitorCommandOutcomeKind::Rejected,
            current_revision,
            None,
            None,
            None,
        )
        .await;
    }

    let existing_execution = live_patrol_execution_in(&mut transaction, command.target_ref).await?;
    let (reason_code, work_order_ref, lease_ref) =
        if let Some((work_order_ref, lease_ref)) = existing_execution {
            ("manual_observe_reused", work_order_ref, lease_ref)
        } else {
            let request = match request_and_admit_in_transaction(
                &mut transaction,
                command.target_ref,
                "patrol",
                "人工立即观察",
                "person",
                &[],
                None,
            )
            .await
            {
                Ok(request) => request,
                Err(error) => {
                    let reason = manual_observe_error_reason(&error);
                    return finish_monitor_command(
                        transaction,
                        command,
                        &payload_digest,
                        reason,
                        MonitorCommandOutcomeKind::Rejected,
                        current_revision,
                        None,
                        None,
                        None,
                    )
                    .await;
                }
            };
            let Some(work_order_ref) = request.work_order_ref else {
                return finish_monitor_command(
                    transaction,
                    command,
                    &payload_digest,
                    closed_monitor_reason(request.reason_code),
                    MonitorCommandOutcomeKind::Rejected,
                    current_revision,
                    None,
                    None,
                    None,
                )
                .await;
            };
            // A manual observation joins the same durable Work-Order pool as a
            // scheduled patrol. It must not pin work to whichever station was
            // healthy at click time; an eligible plugin claims it atomically.
            ("manual_observe_created", work_order_ref, None)
        };
    finish_monitor_command(
        transaction,
        command,
        &payload_digest,
        reason_code,
        MonitorCommandOutcomeKind::Applied,
        current_revision,
        None,
        Some(work_order_ref),
        lease_ref,
    )
    .await
}

fn manual_observe_error_reason(error: &AcquisitionChainError) -> &'static str {
    match error {
        AcquisitionChainError::TargetNotRequestable { .. } => "target_not_requestable",
        AcquisitionChainError::Database(_) => "database_unavailable",
        _ => "database_unavailable",
    }
}

async fn live_patrol_execution_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<Option<(Uuid, Option<Uuid>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT work_order.work_order_ref,lease.lease_ref \
         FROM collection_work_order work_order \
         LEFT JOIN LATERAL ( \
           SELECT lease_ref FROM collection_work_order_lease \
           WHERE work_order_ref=work_order.work_order_ref \
             AND released_at IS NULL AND expires_at>scope_001_now() \
           ORDER BY issued_at DESC LIMIT 1) lease ON true \
         WHERE work_order.target_ref=$1 AND work_order.lane='patrol' \
           AND work_order.queue_state IN ('queued','leased') \
         ORDER BY CASE work_order.queue_state WHEN 'leased' THEN 0 ELSE 1 END, \
                  work_order.created_at DESC LIMIT 1",
    )
    .bind(target_ref)
    .fetch_optional(&mut **transaction)
    .await
}

fn closed_monitor_reason(value: &str) -> &'static str {
    match value {
        "rule_saved" => "rule_saved",
        "monitor_paused" => "monitor_paused",
        "monitor_resumed" => "monitor_resumed",
        "monitor_stopped" => "monitor_stopped",
        "manual_observe_created" => "manual_observe_created",
        "manual_observe_reused" => "manual_observe_reused",
        "stale_revision" => "stale_revision",
        "identity_conflict" => "identity_conflict",
        "invalid_mode" => "invalid_mode",
        "invalid_interval" => "invalid_interval",
        "invalid_schedule" => "invalid_schedule",
        "baseline_not_ready" => "baseline_not_ready",
        "target_not_requestable" => "target_not_requestable",
        "database_unavailable" => "database_unavailable",
        "authorization_missing" => "authorization_missing",
        "authorization_scope_mismatch" => "authorization_scope_mismatch",
        "authorization_purpose_mismatch" => "authorization_purpose_mismatch",
        "authorization_target_limit_reached" => "authorization_target_limit_reached",
        "authorization_work_unit_limit_reached" => "authorization_work_unit_limit_reached",
        "authorization_expired_or_revoked" => "authorization_expired_or_revoked",
        "risk_paused" => "risk_paused",
        "station_unavailable" => "station_unavailable",
        "station_not_accepting" => "station_not_accepting",
        "installation_credential_missing" => "installation_credential_missing",
        "plugin_version_unsupported" => "plugin_version_unsupported",
        "installation_stale" => "installation_stale",
        "capability_missing" => "capability_missing",
        "account_unbound" => "account_unbound",
        "account_binding_changed" => "account_binding_changed",
        "account_binding_expired" => "account_binding_expired",
        "account_eligibility_stale" => "account_eligibility_stale",
        "account_cooling" => "account_cooling",
        "account_needs_login" => "account_needs_login",
        "account_restricted" => "account_restricted",
        "account_unknown" => "account_unknown",
        "account_busy" => "account_busy",
        "station_daily_budget_reached" => "station_daily_budget_reached",
        "capacity_unknown" => "capacity_unknown",
        _ => "database_unavailable",
    }
}

pub(crate) async fn creator_baseline_qualified(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT \
           EXISTS ( \
             SELECT 1 FROM collection_work_order work_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id \
             JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
             CROSS JOIN LATERAL jsonb_array_elements( \
               CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                    THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE work_order.target_ref=$1 AND work_order.lane='deep_archive' \
               AND package.package_kind='author_profile' \
               AND receipt.material_admission='ACCEPTED' \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND layer->>'capability'='author_profile' \
               AND COALESCE((layer->>'observed')::integer,0)>0 \
               AND COALESCE((layer->>'attempted')::integer,0)>0 \
               AND COALESCE((layer->>'acquired')::integer,0)>0 \
               AND COALESCE((layer->>'failed')::integer,0)=0 \
               AND COALESCE((layer->>'notAttempted')::integer,0)=0 \
               AND COALESCE((layer->>'unknown')::integer,0)=0 \
               AND layer->>'stoppedReason' IN ('surface_ended','maximum_quota') \
               AND EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                           WHERE disposition.package_ref=package.package_ref \
                             AND disposition.disposition='accepted_for_library_content') \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref \
                                 AND disposition.disposition='quarantined')) \
           AND EXISTS ( \
             SELECT 1 FROM collection_work_order work_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id \
             JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
             CROSS JOIN LATERAL jsonb_array_elements( \
               CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                    THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE work_order.target_ref=$1 AND work_order.lane='deep_archive' \
               AND package.package_kind='profile_discovery' \
               AND receipt.material_admission='ACCEPTED' \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND layer->>'capability'='profile_discovery' \
               AND COALESCE((layer->>'observed')::integer,0)>0 \
               AND COALESCE((layer->>'attempted')::integer,0)>0 \
               AND COALESCE((layer->>'acquired')::integer,0)>0 \
               AND COALESCE((layer->>'failed')::integer,0)=0 \
               AND COALESCE((layer->>'notAttempted')::integer,0)=0 \
               AND COALESCE((layer->>'unknown')::integer,0)=0 \
               AND (layer->>'stoppedReason'='surface_ended' OR ( \
                 layer->>'stoppedReason'='maximum_quota' \
                 AND work_order.stop_conditions #>> '{progressiveArchive,version}'='1' \
                 AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text \
                 AND COALESCE((work_order.stop_conditions #>> '{progressiveArchive,maxDirectoryWorks}')::integer,-1)=200 \
                 AND COALESCE((task.task_spec->>'maximumQuota')::integer,-1)=200 \
                 AND COALESCE((layer->>'acquired')::integer,-1)=200)) \
               AND EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                           WHERE disposition.package_ref=package.package_ref \
                             AND disposition.disposition<>'quarantined') \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref \
                                 AND disposition.disposition='quarantined'))",
    )
    .bind(target_ref)
    .fetch_one(&mut **transaction)
    .await
}

fn validate_monitor_command(
    command: &MonitorRuleCommand,
    active_rule_ref: Option<Uuid>,
    lifecycle_state: &str,
) -> Result<(), &'static str> {
    if !matches!(
        (command.actor, command.source),
        (MonitorCommandActor::Person, "targets_ui" | "local_api")
            | (MonitorCommandActor::System, "scheduler")
    ) {
        return Err("invalid_schedule");
    }
    if command.kind == MonitorCommandKind::ManualObserve || lifecycle_state == "dismissed" {
        return Err("target_not_requestable");
    }
    if matches!(
        command.kind,
        MonitorCommandKind::Pause | MonitorCommandKind::Resume | MonitorCommandKind::Stop
    ) && active_rule_ref.is_none()
    {
        return Err("target_not_requestable");
    }
    if command.kind == MonitorCommandKind::Resume && lifecycle_state == "dismissed" {
        return Err("target_not_requestable");
    }
    if command.kind != MonitorCommandKind::SaveRule {
        return command.draft.is_none().then_some(()).ok_or("invalid_mode");
    }
    let draft = command.draft.as_ref().ok_or("invalid_mode")?;
    if draft.surface_key.trim().is_empty() || draft.task_contract_version.trim().is_empty() {
        return Err("invalid_mode");
    }
    // The first delivery deliberately exposes one unambiguous cadence: fixed
    // interval anchored at enable/resume. Weekday, window, fallback and dynamic
    // combinations used to form a second scheduler hidden behind the same rule.
    if draft.mode != MonitorRuleMode::Fixed {
        return Err("invalid_mode");
    }
    let Some(interval) = draft.fixed_interval_seconds else {
        return Err("invalid_interval");
    };
    if !valid_interval(interval) || draft.fallback_interval_seconds != interval {
        return Err("invalid_interval");
    }
    if !draft.run_on_weekdays || !draft.run_on_weekends || !draft.all_day {
        return Err("invalid_schedule");
    }
    (draft.window_start_minute.is_none() && draft.window_end_minute.is_none())
        .then_some(())
        .ok_or("invalid_schedule")
}

fn valid_interval(value: i32) -> bool {
    matches!(value, 21_600 | 43_200 | 86_400 | 172_800 | 604_800)
}

fn monitor_command_digest(command: &MonitorRuleCommand) -> String {
    let payload = serde_json::json!({
        "targetRef": command.target_ref,
        "expectedRevision": command.expected_revision,
        "commandKind": command.kind.as_str(),
        "actor": command.actor.as_str(),
        "source": command.source,
        "draft": command.draft,
    });
    sha256_hex(
        serde_json::to_vec(&payload)
            .expect("monitor command is serializable")
            .as_slice(),
    )
}

async fn insert_monitor_rule_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    rule_revision_ref: Uuid,
    revision: i32,
    draft: &MonitorRuleDraft,
    payload_digest: &str,
    actor: MonitorCommandActor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_monitor_rule_revision \
             (rule_revision_ref,target_ref,revision,mode,automatic_enabled,timezone, \
              run_on_weekdays,run_on_weekends,all_day,window_start_minute,window_end_minute, \
              fixed_interval_seconds,fallback_interval_seconds,surface_key,ranking_key, \
              task_contract_version,rule_payload_digest,created_by) \
         VALUES ($1,$2,$3,$4,$5,'Asia/Shanghai',$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
    )
    .bind(rule_revision_ref)
    .bind(target_ref)
    .bind(revision)
    .bind(draft.mode.as_str())
    .bind(draft.automatic_enabled)
    .bind(draft.run_on_weekdays)
    .bind(draft.run_on_weekends)
    .bind(draft.all_day)
    .bind(draft.window_start_minute)
    .bind(draft.window_end_minute)
    .bind(draft.fixed_interval_seconds)
    .bind(draft.fallback_interval_seconds)
    .bind(draft.surface_key.trim())
    .bind(
        draft
            .ranking_key
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty()),
    )
    .bind(draft.task_contract_version.trim())
    .bind(payload_digest)
    .bind(actor.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn copy_monitor_rule_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    active_rule_ref: Uuid,
    next_rule_ref: Uuid,
    next_revision: i32,
    automatic_enabled: bool,
    mode_override: Option<MonitorRuleMode>,
    payload_digest: &str,
    actor: MonitorCommandActor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_monitor_rule_revision \
             (rule_revision_ref,target_ref,revision,mode,automatic_enabled,timezone, \
              run_on_weekdays,run_on_weekends,all_day,window_start_minute,window_end_minute, \
              fixed_interval_seconds,fallback_interval_seconds,surface_key,ranking_key, \
              task_contract_version,rule_payload_digest,created_by) \
         SELECT $3,$1,$4,COALESCE($5,mode),$6,timezone,run_on_weekdays,run_on_weekends, \
                all_day,window_start_minute,window_end_minute, \
                CASE WHEN $5='manual_only' THEN NULL ELSE fixed_interval_seconds END, \
                fallback_interval_seconds,surface_key,ranking_key,task_contract_version,$7,$8 \
         FROM collection_monitor_rule_revision \
         WHERE target_ref=$1 AND rule_revision_ref=$2",
    )
    .bind(target_ref)
    .bind(active_rule_ref)
    .bind(next_rule_ref)
    .bind(next_revision)
    .bind(mode_override.map(MonitorRuleMode::as_str))
    .bind(automatic_enabled)
    .bind(payload_digest)
    .bind(actor.as_str())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn apply_monitor_lifecycle_transition(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    _target_kind: &str,
    lifecycle_state: &str,
    kind: MonitorCommandKind,
    automatic_enabled: bool,
    rule_revision_ref: Uuid,
    actor: MonitorCommandActor,
) -> Result<(), sqlx::Error> {
    let next_state = match kind {
        // Observing a known target and historically archiving it are separate
        // capabilities. A creator need not first prove a complete archive in
        // order to enter ordinary automatic observation.
        MonitorCommandKind::SaveRule
            if automatic_enabled && lifecycle_state == "pending_decision" =>
        {
            Some("monitoring")
        }
        MonitorCommandKind::SaveRule
            if !automatic_enabled && lifecycle_state == "pending_decision" =>
        {
            Some("paused")
        }
        MonitorCommandKind::SaveRule
            if automatic_enabled && matches!(lifecycle_state, "archived" | "paused") =>
        {
            Some("monitoring")
        }
        MonitorCommandKind::SaveRule if !automatic_enabled && lifecycle_state == "monitoring" => {
            Some("paused")
        }
        MonitorCommandKind::Pause if lifecycle_state == "monitoring" => Some("paused"),
        MonitorCommandKind::Resume
            if matches!(lifecycle_state, "paused" | "archived" | "pending_decision") =>
        {
            Some("monitoring")
        }
        MonitorCommandKind::Stop if lifecycle_state != "dismissed" => Some("dismissed"),
        _ => None,
    };
    let Some(next_state) = next_state else {
        return Ok(());
    };
    sqlx::query(
        "UPDATE collection_observation_target SET lifecycle_state=$2, \
                lifecycle_changed_at=scope_001_now() WHERE target_ref=$1",
    )
    .bind(target_ref)
    .bind(next_state)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "INSERT INTO collection_observation_target_transition \
             (transition_ref,target_ref,from_state,to_state,actor,reason_code,reason) \
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(lifecycle_state)
    .bind(next_state)
    .bind(actor.as_str())
    .bind(match kind {
        MonitorCommandKind::Pause => "monitor_paused",
        MonitorCommandKind::Resume => "monitor_resumed",
        MonitorCommandKind::Stop => "monitor_stopped",
        MonitorCommandKind::SaveRule if automatic_enabled => "monitor_resumed",
        MonitorCommandKind::SaveRule => "monitor_paused",
        MonitorCommandKind::ManualObserve => unreachable!(),
    })
    .bind(format!("规则版本 {rule_revision_ref}"))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn finish_monitor_command(
    mut transaction: sqlx::Transaction<'_, sqlx::Postgres>,
    command: &MonitorRuleCommand,
    payload_digest: &str,
    reason_code: &'static str,
    outcome: MonitorCommandOutcomeKind,
    current_revision: i32,
    applied_rule_revision_ref: Option<Uuid>,
    work_order_ref: Option<Uuid>,
    lease_ref: Option<Uuid>,
) -> Result<MonitorRuleCommandReceipt, MonitorRuleCommandError> {
    let identity_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_monitor_rule_command_identity \
             (command_identity_ref,target_ref,idempotency_key,command_kind,payload_digest, \
              first_outcome,first_reason_code,applied_rule_revision_ref,work_order_ref,lease_ref) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(identity_ref)
    .bind(command.target_ref)
    .bind(command.idempotency_key)
    .bind(command.kind.as_str())
    .bind(payload_digest)
    .bind(match outcome {
        MonitorCommandOutcomeKind::Applied => "applied",
        MonitorCommandOutcomeKind::StaleRevision => "stale_revision",
        MonitorCommandOutcomeKind::Rejected => "rejected",
        MonitorCommandOutcomeKind::Replay | MonitorCommandOutcomeKind::IdentityConflict => {
            unreachable!("first receipt cannot be replay/conflict")
        }
    })
    .bind(reason_code)
    .bind(applied_rule_revision_ref)
    .bind(work_order_ref)
    .bind(lease_ref)
    .execute(&mut *transaction)
    .await?;
    let receipt_ref = insert_monitor_command_receipt(
        &mut transaction,
        Some(identity_ref),
        command,
        payload_digest,
        outcome,
        reason_code,
        applied_rule_revision_ref,
        work_order_ref,
        lease_ref,
    )
    .await?;
    transaction.commit().await?;
    Ok(MonitorRuleCommandReceipt {
        receipt_ref,
        outcome,
        reason_code,
        current_revision,
        applied_rule_revision_ref,
        work_order_ref,
        lease_ref,
    })
}

async fn insert_monitor_command_receipt(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    identity_ref: Option<Uuid>,
    command: &MonitorRuleCommand,
    payload_digest: &str,
    outcome: MonitorCommandOutcomeKind,
    reason_code: &'static str,
    applied_rule_revision_ref: Option<Uuid>,
    work_order_ref: Option<Uuid>,
    lease_ref: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let receipt_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_monitor_rule_command_receipt \
             (command_receipt_ref,command_identity_ref,target_ref,idempotency_key,command_kind, \
              expected_revision,payload_digest,actor,source,outcome,reason_code, \
              applied_rule_revision_ref,work_order_ref,lease_ref) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
    )
    .bind(receipt_ref)
    .bind(identity_ref)
    .bind(command.target_ref)
    .bind(command.idempotency_key)
    .bind(command.kind.as_str())
    .bind(command.expected_revision)
    .bind(payload_digest)
    .bind(command.actor.as_str())
    .bind(command.source)
    .bind(outcome.as_str())
    .bind(reason_code)
    .bind(applied_rule_revision_ref)
    .bind(work_order_ref)
    .bind(lease_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(receipt_ref)
}

/// Dynamic cadence is a control calculation over comparable rounds, not a monitoring-value
/// score. Fewer than two fully comparable rounds always returns DYNAMIC_UNAVAILABLE; callers may
/// display/use the explicit 24-hour fallback but must not present it as a learned interval.
pub fn dynamic_cadence(
    rounds: &[ComparableObservationRound],
    surface_key: &str,
    ranking_key: Option<&str>,
    task_contract_version: &str,
    account_ref: Uuid,
    rule_revision_ref: Uuid,
) -> DynamicCadence {
    let qualified: Vec<&ComparableObservationRound> = rounds
        .iter()
        .filter(|round| {
            round.exact_publication_time
                && round.accepted_receipt
                && round.coverage_qualified
                && round.surface_key == surface_key
                && round.ranking_key.as_deref() == ranking_key
                && round.task_contract_version == task_contract_version
                && round.account_ref == account_ref
                && round.rule_revision_ref == rule_revision_ref
        })
        .collect();
    let distinct_rounds: std::collections::HashSet<Uuid> = qualified
        .iter()
        .map(|round| round.observation_round_ref)
        .collect();
    if distinct_rounds.len() < 2 {
        return DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable",
        };
    }
    // A single observation round can return several records. Keep one timestamp per round so
    // publication gaps inside that response cannot masquerade as the interval between patrols.
    // The latest exact publication in a round is the stable round timestamp: it represents the
    // newest item the comparable observation actually saw without substituting package time.
    let mut round_moments = std::collections::HashMap::<Uuid, i64>::new();
    for round in qualified {
        round_moments
            .entry(round.observation_round_ref)
            .and_modify(|latest| *latest = (*latest).max(round.publication_epoch_seconds))
            .or_insert(round.publication_epoch_seconds);
    }
    let mut moments: Vec<i64> = round_moments.into_values().collect();
    moments.sort_unstable();
    moments.dedup();
    if moments.len() < 2 {
        return DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable",
        };
    }
    let mut gaps: Vec<i64> = moments
        .windows(2)
        .filter_map(|pair| (pair[1] > pair[0]).then_some(pair[1] - pair[0]))
        .collect();
    if gaps.is_empty() {
        return DynamicCadence::Unavailable {
            reason_code: "dynamic_unavailable",
        };
    }
    gaps.sort_unstable();
    let middle = gaps.len() / 2;
    let median_gap = if gaps.len() % 2 == 0 {
        gaps[middle - 1] + (gaps[middle] - gaps[middle - 1]) / 2
    } else {
        gaps[middle]
    };
    let median = median_gap / 2;
    DynamicCadence::Available {
        interval_seconds: median.clamp(
            i64::from(MINIMUM_MONITOR_INTERVAL_SECONDS),
            i64::from(MAXIMUM_MONITOR_INTERVAL_SECONDS),
        ) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_version_gate_rejects_old_and_malformed_versions() {
        assert!(!version_at_least("0.8.33", MINIMUM_PLUGIN_VERSION));
        assert!(version_at_least("0.8.34", MINIMUM_PLUGIN_VERSION));
        assert!(version_at_least("v0.9.0", MINIMUM_PLUGIN_VERSION));
        assert!(!version_at_least("0.8.34-beta.1", MINIMUM_PLUGIN_VERSION));
        assert!(!version_at_least("current", MINIMUM_PLUGIN_VERSION));
    }

    #[test]
    fn required_capabilities_follow_the_frozen_scope_not_a_broader_template() {
        assert_eq!(
            required_capabilities_for("creator", "deep_archive", true, false, false, false),
            vec!["content_detail"],
            "a detail-only scope must not require unapproved comment or media access"
        );
        assert_eq!(
            required_capabilities_for("creator", "deep_archive", true, true, false, false),
            vec!["content_detail", "comments"],
        );
        assert_eq!(
            required_capabilities_for("creator", "deep_archive", true, true, true, true),
            vec!["content_detail", "comments", "replies", "media_slots"],
        );
        assert_eq!(
            required_capabilities_for("creator", "patrol", false, false, false, false),
            vec!["author_profile", "profile_discovery"],
            "patrol readiness must cover every step the Work Order actually emits"
        );
    }

    #[test]
    fn credential_shape_is_high_entropy_and_hash_only() {
        let first = format!(
            "lgi_ic_{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let second = format!(
            "lgi_ic_{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        assert_eq!(first.len(), 71);
        assert_ne!(first, second);
        assert_eq!(sha256_hex(first.as_bytes()).len(), 64);
        assert!(constant_time_equal(
            sha256_hex(first.as_bytes()).as_bytes(),
            sha256_hex(first.as_bytes()).as_bytes()
        ));
        assert!(!constant_time_equal(
            sha256_hex(first.as_bytes()).as_bytes(),
            sha256_hex(second.as_bytes()).as_bytes()
        ));
    }

    #[test]
    fn hmac_matches_rfc_4231_vector() {
        assert_eq!(
            hmac_sha256_hex(&[0x0b; 20], b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn dynamic_needs_two_qualified_comparable_rounds() {
        let account_ref = Uuid::new_v4();
        let rule_revision_ref = Uuid::new_v4();
        let one = ComparableObservationRound {
            observation_round_ref: Uuid::new_v4(),
            rule_revision_ref,
            publication_epoch_seconds: 1_000_000,
            exact_publication_time: true,
            accepted_receipt: true,
            coverage_qualified: true,
            surface_key: "creator_profile".to_owned(),
            ranking_key: None,
            task_contract_version: "v1".to_owned(),
            account_ref,
        };
        assert_eq!(
            dynamic_cadence(
                &[one.clone()],
                "creator_profile",
                None,
                "v1",
                account_ref,
                rule_revision_ref
            ),
            DynamicCadence::Unavailable {
                reason_code: "dynamic_unavailable"
            }
        );
        let mut two = one.clone();
        two.observation_round_ref = Uuid::new_v4();
        two.publication_epoch_seconds += 86_400;
        assert_eq!(
            dynamic_cadence(
                &[one, two],
                "creator_profile",
                None,
                "v1",
                account_ref,
                rule_revision_ref
            ),
            DynamicCadence::Available {
                interval_seconds: 43_200
            }
        );
    }

    #[test]
    fn dynamic_even_gap_count_averages_the_middle_gaps() {
        let account_ref = Uuid::new_v4();
        let rule_revision_ref = Uuid::new_v4();
        let first = ComparableObservationRound {
            observation_round_ref: Uuid::new_v4(),
            rule_revision_ref,
            publication_epoch_seconds: 1_700_000_000,
            exact_publication_time: true,
            accepted_receipt: true,
            coverage_qualified: true,
            surface_key: "creator_profile".to_owned(),
            ranking_key: None,
            task_contract_version: "v1".to_owned(),
            account_ref,
        };
        let mut second = first.clone();
        second.observation_round_ref = Uuid::new_v4();
        second.publication_epoch_seconds += 86_400;
        let mut third = second.clone();
        third.observation_round_ref = Uuid::new_v4();
        third.publication_epoch_seconds += 172_800;
        assert_eq!(
            dynamic_cadence(
                &[first, second, third],
                "creator_profile",
                None,
                "v1",
                account_ref,
                rule_revision_ref,
            ),
            DynamicCadence::Available {
                interval_seconds: 64_800,
            }
        );
    }

    #[test]
    fn dynamic_uses_one_latest_publication_timestamp_per_round() {
        let account_ref = Uuid::new_v4();
        let rule_revision_ref = Uuid::new_v4();
        let first_round_ref = Uuid::new_v4();
        let second_round_ref = Uuid::new_v4();
        let base = 1_700_000_000;
        let round = |observation_round_ref, publication_epoch_seconds| ComparableObservationRound {
            observation_round_ref,
            rule_revision_ref,
            publication_epoch_seconds,
            exact_publication_time: true,
            accepted_receipt: true,
            coverage_qualified: true,
            surface_key: "creator_profile".to_owned(),
            ranking_key: None,
            task_contract_version: "v1".to_owned(),
            account_ref,
        };
        assert_eq!(
            dynamic_cadence(
                &[
                    round(first_round_ref, base),
                    round(first_round_ref, base + 100),
                    round(second_round_ref, base + 86_400),
                    round(second_round_ref, base + 86_500),
                ],
                "creator_profile",
                None,
                "v1",
                account_ref,
                rule_revision_ref,
            ),
            DynamicCadence::Available {
                interval_seconds: 43_200,
            }
        );
    }

    #[test]
    fn monitor_schedule_slot_is_stable_and_bounded_by_the_selected_interval() {
        let target_ref =
            Uuid::parse_str("018f0d5e-7e57-7b7d-8f99-5f9f12345678").expect("fixture UUID is valid");
        let first = monitor_schedule_slot_seconds(target_ref, 86_400);
        let replay = monitor_schedule_slot_seconds(target_ref, 86_400);
        assert_eq!(first, replay, "same target must not drift across processes");
        assert!((0..86_400).contains(&first));
        assert!((0..21_600).contains(&monitor_schedule_slot_seconds(target_ref, 21_600)));
    }

    /// Account eligibility and installation liveness answer different questions and must not
    /// share one window again. They were the same constant until 2026-09-06, and the coupling
    /// meant that closing the last XHS tab stopped every deep archive within twenty minutes
    /// while nothing was recorded as failing.
    #[test]
    fn account_eligibility_outlives_installation_liveness() {
        assert!(
            ACCOUNT_ELIGIBILITY_TTL_MINUTES > CONTROL_FRESHNESS_MINUTES,
            "an eligibility observation must stay usable longer than a heartbeat window; \
             refreshing it requires an already-open platform document, a heartbeat does not",
        );
        // Liveness stays short on purpose: a silent browser cannot be given platform work.
        assert_eq!(CONTROL_FRESHNESS_MINUTES, 20);
        // Still far shorter than a real platform login, so this remains a conservative claim.
        assert_eq!(ACCOUNT_ELIGIBILITY_TTL_MINUTES, 360);
    }
}
