//! Claiming one ready processing work, running `content-detail-processor-v1` against its record,
//! and finalizing exactly one closed business outcome.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use thiserror::Error;
use tokio::sync::Barrier;
use uuid::Uuid;

use crate::content::current::republish_current;
use crate::content::observation::{append_observation, parse_payload};
use crate::source_identity::{IdentityRuling, resolve_source_content, rule_on_identity};

const DEFAULT_LEASE: &str = "5 minutes";

/// The closed set of business results a finalized work may hold. Runtime level
/// (ready/leased/finalized) is a separate reading; neither collapses into a `success` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusinessOutcome {
    ObservationRecorded,
    SourceIdentityUnresolved,
    SourceIdentityConflict,
    RecordContractInvalid,
}

impl BusinessOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ObservationRecorded => "observation_recorded",
            Self::SourceIdentityUnresolved => "source_identity_unresolved",
            Self::SourceIdentityConflict => "source_identity_conflict",
            Self::RecordContractInvalid => "record_contract_invalid",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ProcessedRecord {
    pub processing_work_ref: Uuid,
    pub record_ref: Uuid,
    pub epoch: i32,
    pub business_outcome: BusinessOutcome,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProcessingOutcome {
    Processed(ProcessedRecord),
    /// No work is ready. This is not the same fact as "all work finished successfully".
    NothingReady,
}

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("the processing transaction failed: {0}")]
    Internal(String),
    /// A newer epoch took the work over, so this run must not write its result.
    #[error("this lease is no longer the current epoch for the work")]
    LeaseLost,
    /// The business transaction failed and the separate durable attempt audit failed too. This
    /// must remain visible to callers; otherwise a crashed run would look traceable when its
    /// `run_error` row was never actually persisted.
    #[error("processing failed: {processing}; run-error persistence also failed: {persistence}")]
    RunErrorPersistence {
        processing: String,
        persistence: String,
    },
}

impl ProcessingError {
    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }
}

/// Where public refs come from. A frozen sequence fails closed at both ends: it never silently
/// substitutes a random UUID when it runs out, and a leftover reference is an error too, because
/// it means the run consumed refs in a different order than the oracle froze.
#[derive(Debug)]
enum RefSource {
    Random,
    Frozen(Mutex<VecDeque<Uuid>>),
}

/// Where a run can be told to fail, so the proof can show a crashed run rolls its business
/// writes back yet still leaves a traceable attempt. Production never sets one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingFault {
    AfterObservation,
}

/// Processing inputs a proof run needs to pin down. Refs default to random v4 UUIDs.
#[derive(Debug)]
pub struct ProcessingOptions {
    refs: RefSource,
    fault: Option<ProcessingFault>,
    rendezvous: Option<Arc<Barrier>>,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            refs: RefSource::Random,
            fault: None,
            rendezvous: None,
        }
    }
}

impl ProcessingOptions {
    /// Pins minted refs to a frozen sequence: attempt, identity, content, observation, revision,
    /// title field source, body field source.
    pub fn use_fixed_refs(&mut self, values: Vec<Uuid>) {
        self.refs = RefSource::Frozen(Mutex::new(values.into()));
    }

    pub fn inject_fault(&mut self, fault: ProcessingFault) {
        self.fault = Some(fault);
    }

    /// Proof-only: holds this run just after its transaction opens, until every participant has
    /// arrived. Without it a concurrency proof is probabilistic - it can pass whether or not the
    /// serialization it claims to test is present. Production never sets a rendezvous.
    pub fn synchronize_before_identity(&mut self, rendezvous: Arc<Barrier>) {
        self.rendezvous = Some(rendezvous);
    }

    async fn wait_for_other_runs(&self) {
        if let Some(rendezvous) = &self.rendezvous {
            rendezvous.wait().await;
        }
    }

    fn fail_at(&self, point: ProcessingFault) -> Result<(), ProcessingError> {
        if self.fault == Some(point) {
            return Err(ProcessingError::Internal(format!(
                "injected processing fault at {point:?}"
            )));
        }
        Ok(())
    }

    /// Proof-only: fails when a frozen sequence still holds references after a run. Without this
    /// a drifting mint order could leave a test green while proving the wrong thing.
    pub fn assert_frozen_refs_fully_consumed(&self) {
        if let RefSource::Frozen(queue) = &self.refs {
            let remaining = queue
                .lock()
                .expect("the ref sequence lock is never held across a panic")
                .len();
            assert_eq!(
                remaining, 0,
                "the frozen proof reference sequence has {remaining} unconsumed reference(s); \
                 the run did not mint what the oracle froze"
            );
        }
    }

    fn mint(&self) -> Uuid {
        match &self.refs {
            RefSource::Random => Uuid::new_v4(),
            RefSource::Frozen(queue) => queue
                .lock()
                .expect("the ref sequence lock is never held across a panic")
                .pop_front()
                .expect(
                    "the frozen proof reference sequence ran out; a fixed reference must never \
                     be replaced by a random UUID",
                ),
        }
    }
}

/// A durable claim on one processing work. It is committed before the work runs, so other
/// readers can see the work is leased and a crash leaves a state a later epoch can take over.
#[derive(Debug)]
pub struct ClaimedRecord {
    work_id: i64,
    processing_work_ref: Uuid,
    attempt_id: i64,
    epoch: i32,
    record_id: i64,
    record_ref: Uuid,
    target_external_id: String,
    source_external_id: Option<String>,
    payload: Value,
}

impl ClaimedRecord {
    pub fn processing_work_ref(&self) -> Uuid {
        self.processing_work_ref
    }

    pub fn record_ref(&self) -> Uuid {
        self.record_ref
    }

    pub fn epoch(&self) -> i32 {
        self.epoch
    }
}

/// Claims one ready processing work and runs it to a single finalized business outcome.
///
/// The claim commits on its own before the run starts. That is deliberate: a lease that only
/// exists inside the processing transaction is not a lease at all - other readers could not see
/// it, and a crash would erase the fact that the work was ever attempted.
pub async fn process_one_ready_record(
    database: &Database,
    options: &ProcessingOptions,
) -> Result<ProcessingOutcome, ProcessingError> {
    let Some(claim) = claim_one_ready_record(database, options).await? else {
        return Ok(ProcessingOutcome::NothingReady);
    };
    let processed = run_claimed_record(database, claim, options).await?;
    options.assert_frozen_refs_fully_consumed();
    Ok(ProcessingOutcome::Processed(processed))
}

/// Phase one: take a durable lease on one ready work. Committed before any processing happens.
pub async fn claim_one_ready_record(
    database: &Database,
    options: &ProcessingOptions,
) -> Result<Option<ClaimedRecord>, ProcessingError> {
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(ProcessingError::internal)?;
    let claim = claim_next_ready_work(&mut transaction, options).await?;
    transaction
        .commit()
        .await
        .map_err(ProcessingError::internal)?;
    Ok(claim)
}

/// Phase two: run the processor and finalize, in a transaction of its own. A failure rolls back
/// every business write but still records why this run did not finish, so the attempt stays
/// visible history rather than vanishing.
pub async fn run_claimed_record(
    database: &Database,
    claim: ClaimedRecord,
    options: &ProcessingOptions,
) -> Result<ProcessedRecord, ProcessingError> {
    match attempt_run(database, &claim, options).await {
        Ok(business_outcome) => Ok(ProcessedRecord {
            processing_work_ref: claim.processing_work_ref,
            record_ref: claim.record_ref,
            epoch: claim.epoch,
            business_outcome,
        }),
        Err(error) => match record_run_error(database, &claim, &error).await {
            Ok(()) => Err(error),
            Err(persistence) => Err(ProcessingError::RunErrorPersistence {
                processing: error.to_string(),
                persistence: persistence.to_string(),
            }),
        },
    }
}

async fn attempt_run(
    database: &Database,
    claim: &ClaimedRecord,
    options: &ProcessingOptions,
) -> Result<BusinessOutcome, ProcessingError> {
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(ProcessingError::internal)?;
    verify_claim_is_current(&mut transaction, claim).await?;
    options.wait_for_other_runs().await;
    let business_outcome = run_processor(&mut transaction, claim, options).await?;
    finalize(&mut transaction, claim, business_outcome).await?;
    transaction
        .commit()
        .await
        .map_err(ProcessingError::internal)?;
    Ok(business_outcome)
}

/// Fails fast when a newer epoch already took this work over, so a superseded run does no work
/// and cannot collide with the winner's rows.
async fn verify_claim_is_current(
    transaction: &mut Transaction<'_, Postgres>,
    claim: &ClaimedRecord,
) -> Result<(), ProcessingError> {
    let current = sqlx::query(
        "SELECT (SELECT max(epoch) FROM record_processing_attempt WHERE processing_work_id = $1) AS latest_epoch, \
                (SELECT business_outcome IS NOT NULL FROM record_processing_work WHERE id = $1) AS already_finished, \
                (SELECT lease_expires_at > scope_001_now() FROM record_processing_attempt WHERE id = $2) AS lease_live",
    )
    .bind(claim.work_id)
    .bind(claim.attempt_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;

    let latest_epoch: Option<i32> = current.get("latest_epoch");
    let already_finished: Option<bool> = current.get("already_finished");
    let lease_live: Option<bool> = current.get("lease_live");
    if latest_epoch != Some(claim.epoch)
        || already_finished != Some(false)
        || lease_live != Some(true)
    {
        return Err(ProcessingError::LeaseLost);
    }
    Ok(())
}

/// Records why a run did not finish, on its own connection so it survives the rolled-back
/// processing transaction. It never touches an attempt that already finalized something.
async fn record_run_error(
    database: &Database,
    claim: &ClaimedRecord,
    error: &ProcessingError,
) -> Result<(), ProcessingError> {
    let updated = sqlx::query(
        "UPDATE record_processing_attempt SET run_error = $1 \
         WHERE id = $2 AND finalized_at IS NULL",
    )
    .bind(error.to_string())
    .bind(claim.attempt_id)
    .execute(database.pool())
    .await
    .map_err(ProcessingError::internal)?;
    if updated.rows_affected() != 1 {
        return Err(ProcessingError::Internal(
            "the claimed processing attempt was no longer available for run-error persistence"
                .to_owned(),
        ));
    }
    Ok(())
}

const CLAIM_SQL: &str = "\
SELECT w.id AS work_id, w.processing_work_ref, r.id AS record_id, r.record_ref, \
       r.target_external_id, r.source_external_id, r.payload \
FROM record_processing_work w \
JOIN capture_record r ON r.id = w.capture_record_id \
WHERE w.business_outcome IS NULL \
  AND NOT EXISTS ( \
      SELECT 1 FROM record_processing_attempt a \
      WHERE a.processing_work_id = w.id AND a.finalized_at IS NULL \
        AND a.lease_expires_at > scope_001_now() \
  ) \
ORDER BY r.id \
FOR UPDATE OF w SKIP LOCKED \
LIMIT 1";

async fn claim_next_ready_work(
    transaction: &mut Transaction<'_, Postgres>,
    options: &ProcessingOptions,
) -> Result<Option<ClaimedRecord>, ProcessingError> {
    let Some(row) = sqlx::query(CLAIM_SQL)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(ProcessingError::internal)?
    else {
        return Ok(None);
    };

    let work_id: i64 = row.get("work_id");
    let attempt = sqlx::query(
        "INSERT INTO record_processing_attempt \
             (processing_attempt_ref, processing_work_id, epoch, lease_expires_at) \
         SELECT $1, $2, coalesce(max(epoch), 0) + 1, scope_001_now() + $3::interval \
         FROM record_processing_attempt WHERE processing_work_id = $2 \
         RETURNING id, epoch",
    )
    .bind(options.mint())
    .bind(work_id)
    .bind(DEFAULT_LEASE)
    .fetch_one(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;

    Ok(Some(ClaimedRecord {
        work_id,
        processing_work_ref: row.get("processing_work_ref"),
        attempt_id: attempt.get("id"),
        epoch: attempt.get("epoch"),
        record_id: row.get("record_id"),
        record_ref: row.get("record_ref"),
        target_external_id: row.get("target_external_id"),
        source_external_id: row.get("source_external_id"),
        payload: row.get("payload"),
    }))
}

/// Runs `content-detail-processor-v1`. Every path here ends in one closed business outcome; a
/// record that forms no observation still forms no empty identity, content or current value.
async fn run_processor(
    transaction: &mut Transaction<'_, Postgres>,
    claim: &ClaimedRecord,
    options: &ProcessingOptions,
) -> Result<BusinessOutcome, ProcessingError> {
    let Some(payload) = parse_payload(&claim.payload) else {
        return Ok(BusinessOutcome::RecordContractInvalid);
    };

    let external_id = match rule_on_identity(
        &claim.target_external_id,
        claim.source_external_id.as_deref(),
        payload.source_external_id.as_deref(),
    ) {
        IdentityRuling::Resolved(external_id) => external_id,
        IdentityRuling::NotFormed(outcome) => return Ok(outcome),
    };

    let source_content_id =
        resolve_source_content(transaction, &external_id, options.mint(), options.mint()).await?;
    append_observation(
        transaction,
        source_content_id,
        claim.record_id,
        &payload,
        options.mint(),
    )
    .await?;
    options.fail_at(ProcessingFault::AfterObservation)?;
    republish_current(
        transaction,
        source_content_id,
        options.mint(),
        options.mint(),
        options.mint(),
    )
    .await?;

    Ok(BusinessOutcome::ObservationRecorded)
}

/// Fixes the one business outcome on the work and closes this attempt. The epoch fence makes a
/// late run fail rather than overwrite a newer epoch's result.
async fn finalize(
    transaction: &mut Transaction<'_, Postgres>,
    claim: &ClaimedRecord,
    business_outcome: BusinessOutcome,
) -> Result<(), ProcessingError> {
    let closed_attempt = sqlx::query(
        "UPDATE record_processing_attempt SET finalized_at = scope_001_now() \
         WHERE id = $1 AND finalized_at IS NULL AND lease_expires_at > scope_001_now() \
           AND epoch = (SELECT max(epoch) FROM record_processing_attempt WHERE processing_work_id = $2)",
    )
    .bind(claim.attempt_id)
    .bind(claim.work_id)
    .execute(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;
    if closed_attempt.rows_affected() != 1 {
        return Err(ProcessingError::LeaseLost);
    }

    let fixed_outcome = sqlx::query(
        "UPDATE record_processing_work SET business_outcome = $1 \
         WHERE id = $2 AND business_outcome IS NULL",
    )
    .bind(business_outcome.as_str())
    .bind(claim.work_id)
    .execute(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;
    if fixed_outcome.rows_affected() != 1 {
        return Err(ProcessingError::LeaseLost);
    }
    Ok(())
}
