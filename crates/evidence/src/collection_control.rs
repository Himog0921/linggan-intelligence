//! COLLECTION-CONTROL-CLOSURE-001 · one server-owned control vocabulary.
//!
//! This module owns installation credentials, privacy-preserving platform-account identity,
//! eligibility observations, station acceptance and the capacity result consumed by Admission,
//! Lease issuance, dispatch and the local UI. It never stores a raw platform account id, Cookie,
//! page body, or a free-form platform error.

use crate::acquisition_chain::{AcquisitionChainError, request_and_admit_in_transaction};
use crate::directory_boundary::{
    directory_proven_sql, profile_read_complete_sql, surface_scan_complete_sql,
};
use crate::station_read::station_daily_note_usage_in;
use crate::work_order_lease::LeaseError;
use linggan_contracts::{Capacity, CapacityReasonCode};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const MINIMUM_PLUGIN_VERSION: &str = "0.8.47";
/// How recently an installation must have checked in to be considered on duty.
///
/// This is a liveness question — is that browser still there — and it stays short. A worker
/// that has been silent for twenty minutes cannot be handed platform work.
pub const CONTROL_FRESHNESS_MINUTES: i32 = 20;
pub const DEFAULT_MONITOR_INTERVAL_SECONDS: i32 = 86_400;
pub const MINIMUM_MONITOR_INTERVAL_SECONDS: i32 = 21_600;
pub const MAXIMUM_MONITOR_INTERVAL_SECONDS: i32 = 604_800;
const CREDENTIAL_VALID_FOR_DAYS: i32 = 30;
const PENDING_CREDENTIAL_VALID_FOR_MINUTES: i32 = 10;

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
    #[error("the installation no longer holds that live task claim")]
    ClaimedTaskNotHeld,
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

/// Explicit negative facts observable on an already-open platform page. An inconclusive DOM
/// read is deliberately absent from this type: it is not an eligibility observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplicitAccountEligibilitySignal {
    CooldownObserved,
    LoginRequired,
    AccessRestricted,
}

impl ExplicitAccountEligibilitySignal {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "cooldown_observed" => Some(Self::CooldownObserved),
            "login_required" => Some(Self::LoginRequired),
            "access_restricted" => Some(Self::AccessRestricted),
            _ => None,
        }
    }

    pub const fn projected_state(self) -> AccountEligibilityState {
        match self {
            Self::CooldownObserved => AccountEligibilityState::Cooling,
            Self::LoginRequired => AccountEligibilityState::NeedsLogin,
            Self::AccessRestricted => AccountEligibilityState::Restricted,
        }
    }
}

/// A conclusive account observation. The type couples the only positive signal to a transient
/// account identity and makes a negative signal identity-free, so a caller cannot accidentally
/// create an account record while claiming that its observation was incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountEligibilityObservation<'a> {
    Authenticated { raw_platform_account_id: &'a str },
    ExplicitBlock(ExplicitAccountEligibilitySignal),
}

impl<'a> AccountEligibilityObservation<'a> {
    pub const fn projected_state(self) -> AccountEligibilityState {
        match self {
            Self::Authenticated { .. } => AccountEligibilityState::Usable,
            Self::ExplicitBlock(signal) => signal.projected_state(),
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
    /// A mismatch is different from an unbound observation: only an existing human binding can
    /// make a newly observed identity unsafe for the task that is already in progress.
    pub binding_mismatch: bool,
    /// A task that was already claimed under a known account must not execute after the task
    /// page proves a different authenticated identity, even when neither identity has yet been
    /// human-bound to this installation.
    pub frozen_account_mismatch: bool,
    /// A first task began before an identity was available; once it observes one, that identity
    /// must not already be executing on another installation.
    pub account_busy: bool,
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
        account_ref: Option<Uuid>,
        eligibility_ref: Option<Uuid>,
    ) -> Self {
        Self {
            capacity: Capacity::Available {
                station_ref: station_ref.to_string(),
            },
            station_ref: Some(station_ref),
            installation_ref: Some(installation_ref),
            account_ref,
            eligibility_ref,
        }
    }
}

pub async fn collection_control_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass(format('%I.%I',current_schema(),'installation_credential')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'platform_observation_account')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_monitor_rule_revision')) IS NOT NULL \
                AND to_regclass(format('%I.%I',current_schema(),'collection_platform_dispatch_policy')) IS NOT NULL \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() AND table_name='execution_station' \
                              AND column_name='accepting_tasks') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() AND table_name='collection_work_order' \
                              AND column_name='retry_not_before_at') \
                AND NOT EXISTS (SELECT 1 FROM information_schema.columns \
                                WHERE table_schema=current_schema() \
                                  AND table_name='platform_observation_account_binding' \
                                  AND column_name='confirmed_until') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='platform_observation_account_eligibility_observation' \
                              AND column_name='expires_at' AND is_nullable='YES') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='platform_observation_account_eligibility_observation' \
                              AND column_name='account_ref' AND is_nullable='YES') \
                AND EXISTS (SELECT 1 FROM information_schema.columns \
                            WHERE table_schema=current_schema() \
                              AND table_name='platform_observation_account_eligibility_observation' \
                              AND column_name='observation_sequence' AND is_nullable='NO')",
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
    observation: AccountEligibilityObservation<'_>,
    digest_key: Option<&[u8]>,
) -> Result<AccountEligibilityReceipt, CollectionControlError> {
    report_account_eligibility_inner(
        database,
        installation_ref,
        raw_credential,
        observation,
        digest_key,
        None,
    )
    .await
}

/// Report the passive account observation made on a page that was opened for one exact claimed
/// task. The server—not the browser—verifies that this installation still owns the task and
/// compares a positive observation with the account frozen by the Lease.
pub async fn report_claimed_task_account_eligibility(
    database: &Database,
    installation_ref: Uuid,
    raw_credential: &str,
    task_id: Uuid,
    observation: AccountEligibilityObservation<'_>,
    digest_key: Option<&[u8]>,
) -> Result<AccountEligibilityReceipt, CollectionControlError> {
    report_account_eligibility_inner(
        database,
        installation_ref,
        raw_credential,
        observation,
        digest_key,
        Some(task_id),
    )
    .await
}

#[derive(Debug, Clone, Copy)]
struct ClaimedTaskAccountContext {
    work_order_ref: Uuid,
    frozen_account_ref: Option<Uuid>,
}

async fn report_account_eligibility_inner(
    database: &Database,
    installation_ref: Uuid,
    raw_credential: &str,
    observation: AccountEligibilityObservation<'_>,
    digest_key: Option<&[u8]>,
    claimed_task_id: Option<Uuid>,
) -> Result<AccountEligibilityReceipt, CollectionControlError> {
    if !collection_control_schema_is_ready(database).await? {
        return Err(CollectionControlError::SchemaUnavailable);
    }
    let mut transaction = database.pool().begin().await?;
    // Lease issue locks this same row before it assigns capacity. Serializing a task-page
    // observation here prevents a first-task account promotion from racing a second claim by
    // the same installation.
    let installation_exists: Option<Uuid> = sqlx::query_scalar(
        "SELECT installation_ref FROM plugin_installation \
         WHERE installation_ref=$1 AND superseded_at IS NULL FOR UPDATE",
    )
    .bind(installation_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    if installation_exists.is_none() {
        return Err(CollectionControlError::UnknownInstallation);
    }
    if !validate_installation_credential_in(&mut transaction, installation_ref, raw_credential)
        .await?
    {
        return Err(CollectionControlError::InvalidCredential);
    }

    let claimed_task = if let Some(task_id) = claimed_task_id {
        let context: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT work_order.work_order_ref,work_order.account_ref \
             FROM collection_work_order_lease_task task \
             JOIN collection_work_order_lease lease ON lease.lease_ref=task.lease_ref \
             JOIN collection_work_order work_order ON work_order.work_order_ref=lease.work_order_ref \
             WHERE task.task_id=$1 AND task.execution_state='in_progress' \
               AND task.claimed_by_installation_ref=$2 \
               AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
             FOR UPDATE OF task",
        )
        .bind(task_id)
        .bind(installation_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((work_order_ref, frozen_account_ref)) = context else {
            return Err(CollectionControlError::ClaimedTaskNotHeld);
        };
        Some(ClaimedTaskAccountContext {
            work_order_ref,
            frozen_account_ref,
        })
    } else {
        None
    };

    let bound_account_ref = active_bound_account_ref_in(&mut transaction, installation_ref).await?;
    let account_ref = match observation {
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id,
        } => {
            let digest_key = digest_key.ok_or(CollectionControlError::MissingDigestKey)?;
            if digest_key.len() < 32 {
                return Err(CollectionControlError::MissingDigestKey);
            }
            account_ref_for_authenticated_observation_in(
                &mut transaction,
                raw_platform_account_id,
                digest_key,
            )
            .await?
        }
        AccountEligibilityObservation::ExplicitBlock(_) => bound_account_ref,
    };
    let state = observation.projected_state();
    let binding_required = account_ref.is_some_and(|observed| bound_account_ref != Some(observed));
    let binding_mismatch = account_ref
        .is_some_and(|observed| bound_account_ref.is_some_and(|bound| bound != observed));
    let frozen_account_mismatch = claimed_task
        .and_then(|context| context.frozen_account_ref)
        .zip(account_ref)
        .is_some_and(|(frozen, observed)| frozen != observed);
    let account_busy = if let Some(observed_account_ref) =
        account_ref.filter(|_| !binding_mismatch && !frozen_account_mismatch)
    {
        // The Lease issuer locks this same row before it checks account capacity. Holding it
        // across the observation, conflict test and NULL→identity promotion closes the race
        // between a first task discovering an account and another installation claiming it.
        sqlx::query(
            "SELECT account_ref FROM platform_observation_account WHERE account_ref=$1 FOR UPDATE",
        )
        .bind(observed_account_ref)
        .execute(&mut *transaction)
        .await?;
        let busy_elsewhere: bool = sqlx::query_scalar(
            "SELECT EXISTS ( \
                SELECT 1 FROM collection_work_order work_order \
                JOIN collection_work_order_lease lease USING(work_order_ref) \
                WHERE work_order.account_ref=$1 \
                  AND work_order.installation_ref<>$2 \
                  AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
            )",
        )
        .bind(observed_account_ref)
        .bind(installation_ref)
        .fetch_one(&mut *transaction)
        .await?;
        if !busy_elsewhere
            && let Some(context) =
                claimed_task.filter(|context| context.frozen_account_ref.is_none())
        {
            // A passive, non-task observation remains diagnostic only. A claimed first task is
            // the sole place allowed to promote NULL to the page's server-verified identity.
            sqlx::query(
                "UPDATE collection_work_order work_order SET account_ref=$1 \
                 WHERE work_order.work_order_ref=$2 AND work_order.installation_ref=$3 \
                   AND work_order.account_ref IS NULL \
                   AND work_order.queue_state='leased' \
                   AND EXISTS ( \
                     SELECT 1 FROM collection_work_order_lease lease \
                     WHERE lease.work_order_ref=work_order.work_order_ref \
                       AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
                   )",
            )
            .bind(observed_account_ref)
            .bind(context.work_order_ref)
            .bind(installation_ref)
            .execute(&mut *transaction)
            .await?;
        }
        busy_elsewhere
    } else {
        false
    };
    sqlx::query(
        "INSERT INTO platform_observation_account_eligibility_observation \
             (eligibility_ref,account_ref,installation_ref,eligibility_state,signal_version, \
              reason_code) \
         VALUES ($1,$2,$3,$4,'xhs-account-eligibility-v1',$5)",
    )
    .bind(Uuid::new_v4())
    .bind(account_ref)
    .bind(installation_ref)
    .bind(state.as_str())
    .bind(state.reason_code())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(AccountEligibilityReceipt {
        account_ref,
        installation_ref,
        state,
        binding_required,
        binding_mismatch,
        frozen_account_mismatch,
        account_busy,
    })
}

async fn account_ref_for_authenticated_observation_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    raw_platform_account_id: &str,
    digest_key: &[u8],
) -> Result<Option<Uuid>, CollectionControlError> {
    let raw_platform_account_id = raw_platform_account_id.trim();
    if raw_platform_account_id.is_empty() || raw_platform_account_id.len() > 512 {
        return Err(CollectionControlError::InvalidPlatformIdentity);
    }
    let identity_digest = hmac_sha256_hex(digest_key, raw_platform_account_id.as_bytes());
    let account_ref = sqlx::query_scalar(
        "INSERT INTO platform_observation_account \
             (account_ref,platform,identity_digest,digest_version) \
         VALUES ($1,'xhs',$2,'hmac-sha256-v1') \
         ON CONFLICT (platform,digest_version,identity_digest) \
         DO UPDATE SET identity_digest=EXCLUDED.identity_digest \
         RETURNING account_ref",
    )
    .bind(Uuid::new_v4())
    .bind(identity_digest)
    .fetch_one(&mut **transaction)
    .await?;
    Ok(Some(account_ref))
}

async fn active_bound_account_ref_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT account_ref FROM platform_observation_account_binding \
         WHERE installation_ref=$1 AND ended_at IS NULL \
         ORDER BY bound_at DESC,binding_ref DESC LIMIT 1",
    )
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await
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
             (binding_ref,account_ref,installation_ref,bound_by) \
         VALUES ($1,$2,$3,'person')",
    )
    .bind(binding_ref)
    .bind(account_ref)
    .bind(installation_ref)
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
#[derive(Debug)]
struct CapacityCandidate {
    station_ref: Uuid,
    installation_ref: Uuid,
    accepting_tasks: bool,
    plugin_version: String,
    capabilities: Value,
    daily_work_quota: i32,
    installation_fresh: bool,
    has_valid_credential: bool,
    bound_account_ref: Option<Uuid>,
    observed_account_ref: Option<Uuid>,
    observed_account_changed: bool,
    eligibility_state: Option<String>,
    account_busy: bool,
    installation_busy: bool,
}

impl<'row> sqlx::FromRow<'row, sqlx::postgres::PgRow> for CapacityCandidate {
    fn from_row(row: &'row sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        Ok(Self {
            station_ref: row.try_get("station_ref")?,
            installation_ref: row.try_get("installation_ref")?,
            accepting_tasks: row.try_get("accepting_tasks")?,
            plugin_version: row.try_get("plugin_version")?,
            capabilities: row.try_get("capabilities")?,
            daily_work_quota: row.try_get("daily_work_quota")?,
            installation_fresh: row.try_get("installation_fresh")?,
            has_valid_credential: row.try_get("has_valid_credential")?,
            bound_account_ref: row.try_get("bound_account_ref")?,
            observed_account_ref: row.try_get("observed_account_ref")?,
            observed_account_changed: row.try_get("observed_account_changed")?,
            eligibility_state: row.try_get("eligibility_state")?,
            account_busy: row.try_get("account_busy")?,
            installation_busy: row.try_get("installation_busy")?,
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CapacityCandidateScope {
    station_ref: Option<Uuid>,
    installation_ref: Option<Uuid>,
    current_lease_ref: Option<Uuid>,
}

/// One loader supplies every capacity path. Keeping the candidate snapshot in a named model
/// prevents normal admission, claimant revalidation and batch backpressure from drifting into
/// subtly different account/binding policies. Its eligibility subquery selects only conclusive
/// facts: historical `unknown/signal_incomplete` rows were inconclusive reads, not negatives.
async fn load_capacity_candidates_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    scope: CapacityCandidateScope,
) -> Result<Vec<CapacityCandidate>, sqlx::Error> {
    sqlx::query_as(
        "SELECT s.station_ref,i.installation_ref,s.accepting_tasks,i.plugin_version, \
                i.capabilities,s.daily_work_quota, \
                i.last_seen_at>=scope_001_now()-make_interval(mins=>$1) AS installation_fresh, \
                EXISTS (SELECT 1 FROM installation_credential credential \
                        WHERE credential.installation_ref=i.installation_ref \
                          AND credential.revoked_at IS NULL \
                          AND credential.activated_at IS NOT NULL \
                          AND credential.expires_at>scope_001_now()) AS has_valid_credential, \
                binding.account_ref AS bound_account_ref, \
                eligibility.account_ref AS observed_account_ref, \
                COALESCE(eligibility.account_ref<>binding.account_ref,false) \
                    AS observed_account_changed, \
                eligibility.eligibility_state, \
                CASE WHEN COALESCE(binding.account_ref,eligibility.account_ref) IS NULL THEN false ELSE EXISTS ( \
                    SELECT 1 FROM collection_work_order work_order \
                    JOIN collection_work_order_lease lease \
                      ON lease.work_order_ref=work_order.work_order_ref \
                    WHERE work_order.account_ref=COALESCE(binding.account_ref,eligibility.account_ref) \
                      AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
                      AND ($4::uuid IS NULL OR lease.lease_ref<>$4)) END AS account_busy, \
                EXISTS ( \
                    SELECT 1 FROM collection_work_order work_order \
                    JOIN collection_work_order_lease lease \
                      ON lease.work_order_ref=work_order.work_order_ref \
                    WHERE work_order.installation_ref=i.installation_ref \
                      AND lease.released_at IS NULL AND lease.expires_at>scope_001_now() \
                      AND ($4::uuid IS NULL OR lease.lease_ref<>$4)) AS installation_busy \
         FROM execution_station s \
         JOIN plugin_installation i ON i.station_ref=s.station_ref AND i.superseded_at IS NULL \
         LEFT JOIN LATERAL ( \
             SELECT candidate.account_ref \
             FROM platform_observation_account_binding candidate \
             WHERE candidate.installation_ref=i.installation_ref AND candidate.ended_at IS NULL \
             ORDER BY candidate.bound_at DESC,candidate.binding_ref DESC LIMIT 1) binding ON true \
         LEFT JOIN LATERAL ( \
             SELECT observation.account_ref,observation.eligibility_state \
             FROM platform_observation_account_eligibility_observation observation \
             WHERE observation.installation_ref=i.installation_ref \
               AND observation.eligibility_state IN ('usable','cooling','needs_login','restricted') \
             ORDER BY observation.observation_sequence DESC LIMIT 1) eligibility ON true \
         WHERE s.retired_at IS NULL \
           AND ($2::uuid IS NULL OR s.station_ref=$2) \
           AND ($3::uuid IS NULL OR i.installation_ref=$3) \
         ORDER BY s.registered_at,s.station_ref",
    )
    .bind(CONTROL_FRESHNESS_MINUTES)
    .bind(scope.station_ref)
    .bind(scope.installation_ref)
    .bind(scope.current_lease_ref)
    .fetch_all(&mut **transaction)
    .await
}

async fn latest_eligibility_ref_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_ref: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT eligibility_ref FROM platform_observation_account_eligibility_observation \
         WHERE installation_ref=$1 \
           AND eligibility_state IN ('usable','cooling','needs_login','restricted') \
         ORDER BY observation_sequence DESC LIMIT 1",
    )
    .bind(installation_ref)
    .fetch_optional(&mut **transaction)
    .await
}

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
    let mut candidates =
        load_capacity_candidates_in(transaction, CapacityCandidateScope::default()).await?;
    candidates.sort_by_key(|candidate| Some(candidate.station_ref) == last_failed_station);
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
        let station_ref = candidate.station_ref;
        let installation_ref = candidate.installation_ref;
        let daily_quota = candidate.daily_work_quota;
        // A human binding is not required to dispatch, but a conclusive observed identity is
        // still a real concurrency and revalidation subject. Freeze it when present; only a
        // truly unobserved first task freezes NULL and relies solely on the installation lock.
        let account_ref = candidate
            .bound_account_ref
            .or(candidate.observed_account_ref);
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
        let eligibility_ref = latest_eligibility_ref_in(transaction, installation_ref).await?;
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
    account_ref: Option<Uuid>,
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
    let candidate = load_capacity_candidates_in(
        transaction,
        CapacityCandidateScope {
            station_ref: Some(station_ref),
            installation_ref: Some(installation_ref),
            current_lease_ref,
        },
    )
    .await?
    .into_iter()
    .next();
    let Some(candidate) = candidate else {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationUnavailable,
            "工单冻结的工位或安装已不在岗。",
        ));
    };
    if let Some((code, reason)) = candidate_block_reason(
        &candidate,
        required_capabilities,
        account_ref,
        require_accepting_tasks,
    ) {
        return Ok(CapacitySelection::blocked(code, reason));
    }
    let used = station_daily_note_usage_in(transaction, station_ref).await?;
    if used >= i64::from(candidate.daily_work_quota) {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationDailyBudgetReached,
            "工位当天已接纳 200 篇，等待自然日预算恢复。",
        ));
    }
    let eligibility_ref = latest_eligibility_ref_in(transaction, installation_ref).await?;
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
    let claimant = load_capacity_candidates_in(
        transaction,
        CapacityCandidateScope {
            installation_ref: Some(installation_ref),
            ..CapacityCandidateScope::default()
        },
    )
    .await?
    .into_iter()
    .next();
    let Some(candidate) = claimant else {
        return Ok(CapacitySelection::blocked(
            CapacityReasonCode::StationUnavailable,
            "当前插件安装没有可用工位。",
        ));
    };
    revalidate_frozen_capacity_in(
        transaction,
        platform,
        lane,
        required_capabilities,
        candidate.station_ref,
        installation_ref,
        candidate
            .bound_account_ref
            .or(candidate.observed_account_ref),
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
    let candidates =
        load_capacity_candidates_in(transaction, CapacityCandidateScope::default()).await?;
    let mut ready = 0_i64;
    for candidate in candidates {
        if candidate_block_reason(&candidate, required_capabilities, None, true).is_some() {
            continue;
        }
        if station_daily_note_usage_in(transaction, candidate.station_ref).await?
            >= i64::from(candidate.daily_work_quota)
        {
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
    if require_accepting_tasks && !candidate.accepting_tasks {
        Some((
            CapacityReasonCode::StationNotAccepting,
            "工位已由人显式暂停未来接活。",
        ))
    } else if !candidate.has_valid_credential {
        Some((
            CapacityReasonCode::InstallationCredentialMissing,
            "在岗安装没有有效的服务端凭据。",
        ))
    } else if !version_at_least(&candidate.plugin_version, MINIMUM_PLUGIN_VERSION) {
        Some((
            CapacityReasonCode::PluginVersionUnsupported,
            "插件版本低于当前账号观察合同，不能执行当前任务。",
        ))
    } else if !candidate.installation_fresh {
        Some((
            CapacityReasonCode::InstallationStale,
            "插件心跳超过 20 分钟，按失联处理。",
        ))
    } else if !capabilities_cover(&candidate.capabilities, required_capabilities) {
        Some((
            CapacityReasonCode::CapabilityMissing,
            "在岗安装缺少本次 lane 所需能力。",
        ))
    } else if candidate.installation_busy {
        Some((
            CapacityReasonCode::StationBusy,
            "这台工位已有一份有效 Lease，首单观察完成前不会并发领取。",
        ))
    } else if frozen_account_ref.is_some_and(|expected| {
        candidate
            .bound_account_ref
            .map_or(candidate.observed_account_ref != Some(expected), |bound| {
                bound != expected
            })
    }) {
        Some((
            CapacityReasonCode::AccountBindingChanged,
            "安装当前账号绑定与工单冻结账号不一致。",
        ))
    } else if candidate.observed_account_changed {
        Some((
            CapacityReasonCode::AccountBindingChanged,
            "安装实际观察到的账号与人工确认绑定不一致。",
        ))
    } else {
        match candidate.eligibility_state.as_deref() {
            Some("usable") if candidate.account_busy => Some((
                CapacityReasonCode::AccountBusy,
                "观察账号已有一份有效 Lease。",
            )),
            Some("usable") | None => None,
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
            _ => None,
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
    /// 采样口径：下拉几次、取点赞前几篇、只要几天内发布的。
    ///
    /// 只属于关键词搜索面——创作者主页没有排序也没有「取前 N」可言。三项一起决定
    /// 「这一轮的 20 篇是怎么来的」，缺了就无法复核。`published_within_days` 可以单独
    /// 为空（表示不限时间），但下拉与取前 N 必须成对，`0046` 的 CHECK 守着这条。
    pub scroll_rounds: Option<i32>,
    pub top_by_likes: Option<i32>,
    pub published_within_days: Option<i32>,
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
    /// 这条命令作用在哪一条口径上。**只对暂停／恢复／停止有意义**。
    ///
    /// `None` = 最早那条在用规则（博主永远只有一条，这就是它的既有行为）。一个关键词盯几个
    /// 榜就是几条规则，各自可以单独暂停——不指定的话就没法说清要停哪一条。
    ///
    /// 存规则不看这一项：新规则落到哪条口径由草稿里的排序决定，那才是「这一版要盯哪个榜」
    /// 的来源；用另一个字段说同一件事，两者不一致时没人说得清哪个作准。
    pub slot_key: Option<String>,
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
    /// 一起开关时有规则没翻过来。
    ///
    /// **`Ok(receipt)` 不等于命令被接受了**：这套命令系统里「版本过期」「被拒」都是耐久事实，
    /// 走回执而不是 `Err`。逐条循环里照 `Ok` 走下去，被拒的那一条只是被静默跳过，而调用方
    /// 看到 `Ok` 就报「已停止观察」——横幅与目标行的真实状态互相矛盾（目标级的开关是所有
    /// 规则的或，它会诚实地留在 monitoring）。
    #[error("{switched} of {total} rules switched")]
    NotEveryRuleSwitched { switched: usize, total: usize },
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
    let target: Option<(String, String)> = sqlx::query_as(
        "SELECT target_kind,lifecycle_state \
         FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(command.target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some((target_kind, lifecycle_state)) = target else {
        return Err(MonitorRuleCommandError::UnknownTarget);
    };

    // **版本号属于规则，不属于目标。**
    //
    // 这条命令作用在哪一条规则上，先定下来，再读那条规则改到第几版。此前读的是目标行上
    // 那个「当前规则」指针，于是一个目标配到第三条规则时必然失败：版本号按目标全局递增，
    // 而指针停在第一条上，第三条永远被判 `stale_revision`，改任何一条都撞唯一约束。
    let commanded_rule = commanded_monitor_rule(&mut transaction, command, &target_kind).await?;
    let (current_revision, active_rule_ref): (i32, Option<Uuid>) = match commanded_rule {
        Some(rule_ref) => {
            sqlx::query_as(
                "SELECT COALESCE(revision.revision,0),rule.active_revision_ref \
             FROM collection_monitor_rule rule \
             LEFT JOIN collection_monitor_rule_revision revision \
                    ON revision.rule_revision_ref=rule.active_revision_ref \
             WHERE rule.rule_ref=$1",
            )
            .bind(rule_ref)
            .fetch_one(&mut *transaction)
            .await?
        }
        // 还没有这条规则：这是它的第 0 版，接下来会被立出来。
        None => (0, None),
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
    let mut saved_rule_ref: Option<Uuid> = None;
    match command.kind {
        MonitorCommandKind::SaveRule => {
            let draft = command.draft.as_ref().expect("validated save-rule draft");
            // 一个目标可以同时有几条口径不同的规则。找到这条口径的规则身份，没有就立一条。
            let slot_key = monitor_rule_slot_key(&target_kind, draft);
            let rule_ref =
                ensure_monitor_rule(&mut transaction, command.target_ref, &slot_key).await?;
            saved_rule_ref = Some(rule_ref);
            insert_monitor_rule_revision(
                &mut transaction,
                command.target_ref,
                rule_ref,
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
        let repairing_invalid_keyword_lifecycle = target_kind == "keyword"
            && matches!(lifecycle_state.as_str(), "archiving" | "archived");

        // 排期状态只住在规则上。错峰偏移按**规则**算：一个关键词的三条规则若共用目标级
        // 偏移，会在同一时刻一起开跑。
        //
        // **规则先写，目标后写。** 目标行上的「在不在被观察」是由所有规则推导出来的，
        // 写反顺序就会用这条规则改之前的状态去算。
        let commanded_rule_ref: Option<Uuid> = match saved_rule_ref {
            Some(rule_ref) => Some(rule_ref),
            None => commanded_rule,
        };
        if let Some(rule_ref) = commanded_rule_ref {
            let schedule_slot_seconds = automatic_enabled
                .then(|| monitor_schedule_slot_seconds(rule_ref, interval_seconds))
                .unwrap_or(0);
            sqlx::query(
                "UPDATE collection_monitor_rule \
                 SET active_revision_ref=$2, \
                     monitor_next_run_at=CASE WHEN $3 \
                         THEN scope_001_now() + make_interval(secs => $4) ELSE NULL END, \
                     monitor_missed_run_count=0 \
                 WHERE rule_ref=$1",
            )
            .bind(rule_ref)
            .bind(applied_rule_ref)
            .bind(automatic_enabled)
            .bind(schedule_slot_seconds)
            .execute(&mut *transaction)
            .await?;
        }

        // 目标行只写目标级的两样事实：这个目标在不在被观察、它的生命周期到哪一步。
        //
        // 排期、周期、当前版本**全部归规则**（`0078` 已把目标行上那份副本删掉）。
        //
        // 「在不在被观察」是**所有规则的或**，不是这条命令那一版的 `automatic_enabled`。
        // 一个关键词盯三个榜、只暂停其中一条时，这个目标显然还在被观察——照这条命令的值写，
        // 目标会被标成已暂停、生命周期掉到 `paused`，而另外两条规则继续按自己的周期跑：
        // 列表说「已暂停」，队列里却一直出活。
        let target_automatic: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM collection_monitor_rule rule \
               JOIN collection_monitor_rule_revision revision \
                 ON revision.rule_revision_ref=rule.active_revision_ref \
              WHERE rule.target_ref=$1 AND rule.retired_at IS NULL \
                AND revision.automatic_enabled)",
        )
        .bind(command.target_ref)
        .fetch_one(&mut *transaction)
        .await?;
        let next_lifecycle_state = monitor_lifecycle_next_state(
            &lifecycle_state,
            command.kind,
            target_automatic,
            repairing_invalid_keyword_lifecycle,
        );
        sqlx::query(
            "UPDATE collection_observation_target \
             SET monitoring_enabled=$2, \
                 lifecycle_state=COALESCE($3,lifecycle_state), \
                 lifecycle_changed_at=CASE WHEN $3 IS NULL THEN lifecycle_changed_at \
                     ELSE scope_001_now() END \
             WHERE target_ref=$1",
        )
        .bind(command.target_ref)
        .bind(target_automatic)
        .bind(next_lifecycle_state)
        .execute(&mut *transaction)
        .await?;

        record_monitor_lifecycle_transition(
            &mut transaction,
            command.target_ref,
            &lifecycle_state,
            next_lifecycle_state,
            command.kind,
            rule_revision_ref,
            command.actor,
            repairing_invalid_keyword_lifecycle,
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
/// 这条规则在它的周期里错开多少秒起跑。
///
/// 按**规则**算，不按目标：一个关键词的三条规则若共用目标级偏移，会在同一时刻一起开跑，
/// 三个浏览器任务挤在一起，而错峰正是为了避免这件事。
fn monitor_schedule_slot_seconds(rule_ref: Uuid, interval_seconds: i32) -> i32 {
    debug_assert!(interval_seconds > 0);
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&rule_ref.as_bytes()[..8]);
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
    let lifecycle_state: Option<String> = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(command.target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(lifecycle_state) = lifecycle_state else {
        return Err(MonitorRuleCommandError::UnknownTarget);
    };
    // 手动观察是对目标下的一次性命令，不改任何规则；版本号只用来挡住「页面上看到的还是旧的」。
    // 页面预填的是最早那条在用规则，这里就对同一条问，两边说的是同一个数。
    let current_revision: i32 = sqlx::query_scalar(
        "SELECT COALESCE(revision.revision,0) \
         FROM collection_monitor_rule rule \
         LEFT JOIN collection_monitor_rule_revision revision \
                ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL \
         ORDER BY rule.created_at,rule.rule_ref LIMIT 1",
    )
    .bind(command.target_ref)
    .fetch_optional(&mut *transaction)
    .await?
    .unwrap_or(0);
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
        // 缺领域必须单独报。它此前落进下面那个通配符，于是「这个目标还没说清属于哪个
        // 领域」被显示成「数据库不可用」——人会去查服务是不是挂了，而真正要做的只是
        // 给目标指定一个领域。
        AcquisitionChainError::TargetDomainUnassigned => "target_domain_unassigned",
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
        "station_busy" => "station_busy",
        "station_daily_budget_reached" => "station_daily_budget_reached",
        "capacity_unknown" => "capacity_unknown",
        _ => "database_unavailable",
    }
}

/// 「一轮合格的关键词建档」这条判据本身。
///
/// 单目标与批量两个入口共用它，避免同一件事在两处各写一遍、日后各自漂移。
/// `$1` 是目标引用（单目标用 `=`，批量用 `= ANY`，由调用方拼）。
macro_rules! keyword_baseline_sql {
    ($target_predicate:expr) => {
        concat!(
            "SELECT DISTINCT work_order.target_ref FROM collection_work_order work_order \
             JOIN collection_work_order_lease lease USING(work_order_ref) \
             JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
             JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
             JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id \
             JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref \
             CROSS JOIN LATERAL jsonb_array_elements( \
               CASE WHEN jsonb_typeof(package.coverage->'layers')='array' \
                    THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer \
             WHERE ",
            $target_predicate,
            " AND work_order.lane='deep_archive' \
               AND package.package_kind='discovery_search' \
               AND receipt.material_admission='ACCEPTED' \
               AND receipt.execution_effect='COMPLETED_LIVE_STEP' \
               AND layer->>'capability'='discovery_search' \
               AND ",
            surface_scan_complete_sql!(),
            " \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref \
                                 AND disposition.disposition='quarantined')",
        )
    };
}

/// 一批关键词各自**建过档没有**。
///
/// 列表页一次要判断很多个目标，逐个查会变成 N+1。返回集合里出现的才是已建档；
/// 没出现的是「还没建过」，不是「读不到」——读不到会以 `Err` 的形式浮上来，由调用方
/// 决定怎么如实呈现，而不是在这里默默压成 false。
pub async fn keyword_baselines_qualified(
    database: &Database,
    target_refs: &[Uuid],
) -> Result<std::collections::HashSet<Uuid>, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    let rows: Vec<Uuid> =
        sqlx::query_scalar(keyword_baseline_sql!("work_order.target_ref=ANY($1)"))
            .bind(target_refs)
            .fetch_all(database.pool())
            .await?;
    Ok(rows.into_iter().collect())
}

/// 这个关键词**建过档没有**。
///
/// 关键词没有建档生命周期：`0042` 用 CHECK 禁止它进入 `archiving`/`archived`，理由写在
/// 那条迁移里——「Keyword observation has a monitor lifecycle, not a creator archive
/// lifecycle」。所以这件事不存成状态，而是**每次读的时候从证据里查出来**，与本仓库
/// 对生命周期的一贯理解一致：生命周期点不是第二份材料事实，是读取时的组合。
///
/// 要证明的事与博主基线相同——「这一轮把该翻的表面翻完了，而且没有一条材料被隔离」
/// ——只是证据长在不同的包上：博主看 `author_profile`，关键词看 `discovery_search`。
///
/// **采完就算不够**：要么把这个词的搜索面翻到底，要么采满了工单给的篇数
/// （`surface_scan_complete_sql!`）。中途因为失败或没尝试而停下的一轮不算建档——把它
/// 算作完成，等于宣布一个没挖完的词已经建好档，之后所有基于它的判断都建立在一个不完整
/// 的底座上，而且没人看得出来。
pub async fn keyword_baseline_qualified(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, Uuid>(keyword_baseline_sql!("work_order.target_ref=$1"))
            .bind(target_ref)
            .fetch_optional(&mut **transaction)
            .await?
            .is_some(),
    )
}

pub(crate) async fn creator_baseline_qualified(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        concat!(
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
               AND ",
            profile_read_complete_sql!(),
            " \
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
               AND ",
            directory_proven_sql!(),
            " \
               AND work_order.stop_conditions #>> '{progressiveArchive,version}'='1' \
               AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order.work_order_ref::text \
               AND COALESCE((work_order.stop_conditions #>> '{progressiveArchive,maxDirectoryWorks}')::integer,-1)=200 \
               AND EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                           WHERE disposition.package_ref=package.package_ref \
                             AND disposition.disposition<>'quarantined') \
               AND NOT EXISTS (SELECT 1 FROM linggan_runtime_record_disposition disposition \
                               WHERE disposition.package_ref=package.package_ref \
                                 AND disposition.disposition='quarantined'))",
        ),
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
    // **「停止观察」是目标级的决定，不针对某一条口径。**
    //
    // 它把生命周期推到 `dismissed`，而那一支不看还有没有别的规则在跑——一个盯三个榜的关键词
    // 若被允许「只停点赞那一条」并走 Stop，整个目标会变成已停止观察，另外两条却还在按周期
    // 出活。当前没有任何界面能提交这个组合，但这个端点收 `command_kind=stop` 也收 `rule_slot`，
    // 直接 POST 就能触发。在这里挡掉，而不是等哪天加个按钮时才发现。
    // 要停一条口径，用暂停或停用；要停整个目标，别带口径。
    if command.kind == MonitorCommandKind::Stop && command.slot_key.is_some() {
        return Err("invalid_mode");
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

/// 这条命令作用在哪一条规则上。
///
/// 保存规则时由口径（排序）决定——那条口径还不存在就返回 `None`，它会在保存那一步被立出来。
/// 暂停／恢复／停止作用的是**整个目标**：它们改的是 `monitoring_enabled`，调度的到期查询
/// 要求它为真，所以关掉它这个目标底下所有规则一起停。这类命令挑目标现有的任意一条规则
/// 记版本即可——取最早那条，保证同一个目标上这类命令的版本号是一条连续的线。
async fn commanded_monitor_rule(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    command: &MonitorRuleCommand,
    target_kind: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let slot_key = match (command.kind, command.draft.as_ref()) {
        (MonitorCommandKind::SaveRule, Some(draft)) => monitor_rule_slot_key(target_kind, draft),
        // 暂停／恢复／停止：命令说了作用在哪条口径就用那一条，没说才回落到最早那条。
        // 一个关键词盯三个榜时，「暂停」必须能说清停哪一条——否则界面上三个按钮点下去
        // 停的都是同一条，而另外两条继续按自己的周期跑，看不出任何异常。
        (_, _) if command.slot_key.is_some() => command
            .slot_key
            .clone()
            .expect("checked by the guard above"),
        _ => {
            return sqlx::query_scalar(
                "SELECT rule_ref FROM collection_monitor_rule \
                 WHERE target_ref=$1 AND retired_at IS NULL \
                 ORDER BY created_at,rule_ref LIMIT 1",
            )
            .bind(command.target_ref)
            .fetch_optional(&mut **transaction)
            .await;
        }
    };
    sqlx::query_scalar(
        "SELECT rule_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND slot_key=$2 AND retired_at IS NULL",
    )
    .bind(command.target_ref)
    .bind(slot_key)
    .fetch_optional(&mut **transaction)
    .await
}

/// 这条规则在这个目标底下的**口径身份**。
///
/// 关键词用排序（`most_liked`／`comprehensive`…）：同一个词盯两个榜是两条规则，不是两个词。
/// 博主没有排序可言，固定一条 `primary`——它永远只有一条规则，行为与加多规则之前一致。
fn monitor_rule_slot_key(target_kind: &str, draft: &MonitorRuleDraft) -> String {
    match (target_kind, draft.ranking_key.as_deref()) {
        ("keyword", Some(ranking)) if !ranking.trim().is_empty() => ranking.trim().to_owned(),
        _ => "primary".to_owned(),
    }
}

/// 找到这个口径的规则身份，没有就立一条。
///
/// **规则之间没有主次。** 此前有个 `is_primary`，只为在迁移期保住目标行上那条
/// 「监控中必须有规则」的 CHECK；`0078` 把那份副本连同 CHECK 一起删掉之后，它就没有对应的
/// 领域含义了——一个关键词的三条规则是并列的三个口径。
async fn ensure_monitor_rule(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    slot_key: &str,
) -> Result<Uuid, sqlx::Error> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT rule_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND slot_key=$2 AND retired_at IS NULL FOR UPDATE",
    )
    .bind(target_ref)
    .bind(slot_key)
    .fetch_optional(&mut **transaction)
    .await?
    {
        return Ok(existing);
    }
    let rule_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_monitor_rule (rule_ref,target_ref,slot_key) VALUES ($1,$2,$3)",
    )
    .bind(rule_ref)
    .bind(target_ref)
    .bind(slot_key)
    .execute(&mut **transaction)
    .await?;
    Ok(rule_ref)
}

async fn insert_monitor_rule_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    rule_ref: Uuid,
    rule_revision_ref: Uuid,
    revision: i32,
    draft: &MonitorRuleDraft,
    payload_digest: &str,
    actor: MonitorCommandActor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_monitor_rule_revision \
             (rule_revision_ref,target_ref,rule_ref,revision,mode,automatic_enabled,timezone, \
              run_on_weekdays,run_on_weekends,all_day,window_start_minute,window_end_minute, \
              fixed_interval_seconds,fallback_interval_seconds,surface_key,ranking_key, \
              scroll_rounds,top_by_likes,published_within_days, \
              task_contract_version,rule_payload_digest,created_by) \
         VALUES ($1,$2,$21,$3,$4,$5,'Asia/Shanghai',$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)",
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
    .bind(draft.scroll_rounds)
    .bind(draft.top_by_likes)
    .bind(draft.published_within_days)
    .bind(draft.task_contract_version.trim())
    .bind(payload_digest)
    .bind(actor.as_str())
    .bind(rule_ref)
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
             (rule_revision_ref,target_ref,rule_ref,revision,mode,automatic_enabled,timezone, \
              run_on_weekdays,run_on_weekends,all_day,window_start_minute,window_end_minute, \
              fixed_interval_seconds,fallback_interval_seconds,surface_key,ranking_key, \
              scroll_rounds,top_by_likes,published_within_days, \
              task_contract_version,rule_payload_digest,created_by) \
         -- 暂停／恢复复制的是**同一条规则**的上一个版本，规则身份跟着原样带过来：
         -- 换个规则身份等于把这条口径的历史切断，之后没人说得清它改过几次。
         SELECT $3,$1,rule_ref,$4,COALESCE($5,mode),$6,timezone,run_on_weekdays,run_on_weekends, \
                all_day,window_start_minute,window_end_minute, \
                CASE WHEN $5='manual_only' THEN NULL ELSE fixed_interval_seconds END, \
                fallback_interval_seconds,surface_key,ranking_key, \
                scroll_rounds,top_by_likes,published_within_days, \
                task_contract_version,$7,$8 \
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

async fn record_monitor_lifecycle_transition(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_ref: Uuid,
    lifecycle_state: &str,
    next_state: Option<&str>,
    kind: MonitorCommandKind,
    rule_revision_ref: Uuid,
    actor: MonitorCommandActor,
    repairing_invalid_keyword_lifecycle: bool,
) -> Result<(), sqlx::Error> {
    let Some(next_state) = next_state else {
        return Ok(());
    };
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
    .bind(if repairing_invalid_keyword_lifecycle {
        "keyword_lifecycle_repaired_by_rule"
    } else {
        match kind {
            MonitorCommandKind::Pause => "monitor_paused",
            MonitorCommandKind::Resume => "monitor_resumed",
            MonitorCommandKind::Stop => "monitor_stopped",
            MonitorCommandKind::SaveRule if next_state == "monitoring" => "monitor_resumed",
            MonitorCommandKind::SaveRule => "monitor_paused",
            MonitorCommandKind::ManualObserve => unreachable!(),
        }
    })
    .bind(if repairing_invalid_keyword_lifecycle {
        format!("规则版本 {rule_revision_ref} 修复了关键词目标遗留的 {lifecycle_state} 状态")
    } else {
        format!("规则版本 {rule_revision_ref}")
    })
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// 这条命令之后，目标的生命周期该到哪一步。
///
/// `target_automatic` 是**这个目标还有没有任何一条规则在自动巡查**，不是这条命令那一版的
/// 开关值。一个关键词盯几个榜就是几条规则，单独停掉其中一条时这个目标显然还在被观察。
fn monitor_lifecycle_next_state(
    lifecycle_state: &str,
    kind: MonitorCommandKind,
    target_automatic: bool,
    repairing_invalid_keyword_lifecycle: bool,
) -> Option<&'static str> {
    match kind {
        // Keyword observation has a monitor lifecycle. This also recovers a historical row
        // before the database invariant has been applied.
        MonitorCommandKind::SaveRule if repairing_invalid_keyword_lifecycle => {
            Some(if target_automatic {
                "monitoring"
            } else {
                "paused"
            })
        }
        MonitorCommandKind::Resume if repairing_invalid_keyword_lifecycle => Some("monitoring"),
        MonitorCommandKind::Pause if repairing_invalid_keyword_lifecycle => {
            Some(if target_automatic {
                "monitoring"
            } else {
                "paused"
            })
        }
        // Observing a known target and historically archiving it are separate
        // capabilities. A creator need not first prove a complete archive in
        // order to enter ordinary automatic observation.
        MonitorCommandKind::SaveRule
            if target_automatic && lifecycle_state == "pending_decision" =>
        {
            Some("monitoring")
        }
        MonitorCommandKind::SaveRule
            if !target_automatic && lifecycle_state == "pending_decision" =>
        {
            Some("paused")
        }
        MonitorCommandKind::SaveRule
            if target_automatic && matches!(lifecycle_state, "archived" | "paused") =>
        {
            Some("monitoring")
        }
        MonitorCommandKind::SaveRule if !target_automatic && lifecycle_state == "monitoring" => {
            Some("paused")
        }
        // **只在最后一条也停下来时才掉到 `paused`。** 此前这一支不看开关值：盯三个榜的
        // 关键词只停点赞那一条，目标就被标成已暂停，而另外两条继续按自己的周期出活。
        MonitorCommandKind::Pause if lifecycle_state == "monitoring" && !target_automatic => {
            Some("paused")
        }
        MonitorCommandKind::Resume
            if matches!(lifecycle_state, "paused" | "archived" | "pending_decision") =>
        {
            Some("monitoring")
        }
        MonitorCommandKind::Stop if lifecycle_state != "dismissed" => Some("dismissed"),
        _ => None,
    }
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
        assert!(version_at_least("0.8.47", MINIMUM_PLUGIN_VERSION));
        assert!(version_at_least("v0.9.0", MINIMUM_PLUGIN_VERSION));
        assert!(!version_at_least("0.8.47-beta.1", MINIMUM_PLUGIN_VERSION));
        assert!(!version_at_least("current", MINIMUM_PLUGIN_VERSION));
    }

    #[test]
    fn observed_identity_change_has_one_shared_pre_lease_block_reason() {
        let candidate = CapacityCandidate {
            station_ref: Uuid::new_v4(),
            installation_ref: Uuid::new_v4(),
            accepting_tasks: true,
            plugin_version: MINIMUM_PLUGIN_VERSION.to_owned(),
            capabilities: serde_json::json!(["content_detail"]),
            daily_work_quota: 200,
            installation_fresh: true,
            has_valid_credential: true,
            bound_account_ref: Some(Uuid::new_v4()),
            observed_account_ref: Some(Uuid::new_v4()),
            observed_account_changed: true,
            eligibility_state: Some("usable".to_owned()),
            account_busy: false,
            installation_busy: false,
        };

        assert_eq!(
            candidate_block_reason(&candidate, &["content_detail"], None, true)
                .map(|(reason, _)| reason),
            Some(CapacityReasonCode::AccountBindingChanged),
            "every capacity path invokes this common policy before it can create or revalidate a Lease"
        );
    }

    #[test]
    fn an_unbound_observed_identity_can_freeze_and_revalidate_a_first_task() {
        let observed_account_ref = Uuid::new_v4();
        let candidate = CapacityCandidate {
            station_ref: Uuid::new_v4(),
            installation_ref: Uuid::new_v4(),
            accepting_tasks: true,
            plugin_version: MINIMUM_PLUGIN_VERSION.to_owned(),
            capabilities: serde_json::json!(["content_detail"]),
            daily_work_quota: 200,
            installation_fresh: true,
            has_valid_credential: true,
            bound_account_ref: None,
            observed_account_ref: Some(observed_account_ref),
            observed_account_changed: false,
            eligibility_state: Some("usable".to_owned()),
            account_busy: false,
            installation_busy: false,
        };

        assert_eq!(
            candidate_block_reason(
                &candidate,
                &["content_detail"],
                Some(observed_account_ref),
                true,
            ),
            None,
            "an observed but not human-bound identity is safe for the task that observed it"
        );
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

    #[test]
    fn automatic_rule_repairs_a_legacy_keyword_archive_state() {
        assert_eq!(
            monitor_lifecycle_next_state("archiving", MonitorCommandKind::SaveRule, true, true),
            Some("monitoring")
        );
        assert_eq!(
            monitor_lifecycle_next_state("archived", MonitorCommandKind::SaveRule, false, true),
            Some("paused")
        );
        assert_eq!(
            monitor_lifecycle_next_state("archiving", MonitorCommandKind::SaveRule, true, false),
            None,
            "a creator's real archive must not be advanced by a monitor-rule save"
        );
    }

    /// A station heartbeat is liveness; account facts are only superseded by later observations.
    #[test]
    fn account_observations_do_not_have_a_ttl_gate() {
        // Liveness stays short on purpose: a silent browser cannot be given platform work.
        assert_eq!(CONTROL_FRESHNESS_MINUTES, 20);
    }
}

/// 从列表上直接开关一个目标的自动巡查。
///
/// **开关的是这个目标的全部在用规则，不是其中一条。** 列表上那一列讲的是目标——「巡查中」
/// 显示的是几条规则的或。此前这个开关只翻最早那条：一个关键词盯三个榜时，点「暂停」之后
/// 另外两条继续按自己的周期跑，而那一列仍然显示「巡查中」，看不出点了没有。
///
/// 要单独停某一条，用检查器的规则台——那里每条规则各有自己的按钮。这两个入口讲的是两件事：
/// 列表管「这个目标还观察不观察」，规则台管「这个目标按哪几个口径观察」。
///
/// 版本号在服务端逐条读，不由表单带上来。乐观并发那道关卡是为**规则编辑表单**设的——那里
/// 你提交的是一整套字段值，用一个过期的版本号覆盖别人刚改过的设置是真实风险。而这里
/// 只翻一个开关，不携带任何字段值，读当前版本再发命令不会覆盖任何人的编辑。
///
/// 它仍然走版本化命令这一条路：每条规则每次开关都留下一个新的规则版本与回执，不是裸改一个
/// 布尔值——那条路早就因为「不能成为巡检的第二个真相源」被废弃了。
///
/// 返回最后一条命令的回执。**逐条命令、不在一个事务里**：每条规则的版本推进本来就是独立
/// 事实，硬绑成一笔会让其中一条的并发冲突连坐掉其他几条已经成功的开关。
pub async fn toggle_target_patrol(
    database: &Database,
    target_ref: Uuid,
    enable: bool,
) -> Result<MonitorRuleCommandReceipt, MonitorRuleCommandError> {
    let rules: Vec<(String, i32)> = sqlx::query_as(
        "SELECT rule.slot_key,COALESCE(revision.revision,0) \
         FROM collection_monitor_rule rule \
         LEFT JOIN collection_monitor_rule_revision revision \
                ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL \
         ORDER BY rule.created_at,rule.rule_ref",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await?;
    if rules.is_empty() {
        // 没有生效的规则版本就没有可开关的东西。这不是失败，是「先去设一条规则」。
        return Err(MonitorRuleCommandError::UnknownTarget);
    }
    let total = rules.len();
    let mut switched = 0;
    let mut last = None;
    for (slot_key, expected_revision) in rules {
        let receipt = apply_monitor_rule_command(
            database,
            &MonitorRuleCommand {
                target_ref,
                expected_revision,
                idempotency_key: Uuid::new_v4(),
                kind: if enable {
                    MonitorCommandKind::Resume
                } else {
                    MonitorCommandKind::Pause
                },
                actor: MonitorCommandActor::Person,
                source: "targets_ui",
                draft: None,
                slot_key: Some(slot_key),
            },
        )
        .await?;
        if matches!(receipt.outcome, MonitorCommandOutcomeKind::Applied) {
            switched += 1;
        }
        last = Some(receipt);
    }
    if switched != total {
        // 有一条没翻过来就不能报成功。目标行自己是诚实的（它是所有规则的或），撒谎的是
        // 那句横幅——而横幅正是人唯一会读的东西。
        return Err(MonitorRuleCommandError::NotEveryRuleSwitched { switched, total });
    }
    Ok(last.expect("the rule list was checked to be non-empty"))
}

#[cfg(test)]
mod manual_observe_reason_tests {
    use super::*;

    /// 「缺领域」不能被通配符吞成「数据库不可用」。
    ///
    /// 它原本落进 `_ => "database_unavailable"`：人对一个没归属领域的目标点「立即观察」，
    /// 页面说数据库不可用，于是去查服务是不是挂了——而真正要做的只是给目标指定领域。
    /// 这条用例锁住它不会再被归到故障里去。
    #[test]
    fn an_unassigned_domain_is_not_reported_as_a_database_failure() {
        assert_eq!(
            manual_observe_error_reason(&AcquisitionChainError::TargetDomainUnassigned),
            "target_domain_unassigned"
        );
        // 另外两类原因保持原样，证明这次只把一个被误归的原因摘出来。
        assert_eq!(
            manual_observe_error_reason(&AcquisitionChainError::TargetNotRequestable {
                state: "monitoring".to_owned()
            }),
            "target_not_requestable"
        );
        assert_eq!(
            manual_observe_error_reason(&AcquisitionChainError::SchemaUnavailable),
            "database_unavailable"
        );
    }
}
