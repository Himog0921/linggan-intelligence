//! Claiming one ready processing work, running `content-detail-processor-v1` against its record,
//! and finalizing exactly one closed business outcome.

use std::collections::VecDeque;
use std::sync::Mutex;

use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use thiserror::Error;
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
}

impl ProcessingError {
    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }
}

/// Processing inputs a proof run needs to pin down. Refs default to random v4 UUIDs.
#[derive(Debug)]
pub struct ProcessingOptions {
    refs: Mutex<VecDeque<Uuid>>,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            refs: Mutex::new(VecDeque::new()),
        }
    }
}

impl ProcessingOptions {
    /// Pins minted refs to a frozen sequence: attempt, identity, content, observation, revision,
    /// title field source, body field source.
    pub fn use_fixed_refs(&mut self, values: Vec<Uuid>) {
        self.refs = Mutex::new(values.into());
    }

    fn mint(&self) -> Uuid {
        self.refs
            .lock()
            .expect("the ref sequence lock is never held across a panic")
            .pop_front()
            .unwrap_or_else(Uuid::new_v4)
    }
}

/// What a claim locked, read from the record rather than from any caller input.
struct ClaimedWork {
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

/// Claims one ready processing work and runs it to a single finalized business outcome. Each
/// record is claimed and finalized on its own; one record's result never writes back to the
/// package, the coverage or another record.
pub async fn process_one_ready_record(
    database: &Database,
    options: &ProcessingOptions,
) -> Result<ProcessingOutcome, ProcessingError> {
    let Some(claim) = claim_next_ready_work(database, options).await? else {
        return Ok(ProcessingOutcome::NothingReady);
    };

    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(ProcessingError::internal)?;
    let business_outcome = run_processor(&mut transaction, &claim, options).await?;
    finalize(&mut transaction, &claim, business_outcome).await?;
    transaction
        .commit()
        .await
        .map_err(ProcessingError::internal)?;

    Ok(ProcessingOutcome::Processed(ProcessedRecord {
        processing_work_ref: claim.processing_work_ref,
        record_ref: claim.record_ref,
        epoch: claim.epoch,
        business_outcome,
    }))
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
    database: &Database,
    options: &ProcessingOptions,
) -> Result<Option<ClaimedWork>, ProcessingError> {
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(ProcessingError::internal)?;
    let Some(row) = sqlx::query(CLAIM_SQL)
        .fetch_optional(&mut *transaction)
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
    .fetch_one(&mut *transaction)
    .await
    .map_err(ProcessingError::internal)?;
    transaction
        .commit()
        .await
        .map_err(ProcessingError::internal)?;

    Ok(Some(ClaimedWork {
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
    claim: &ClaimedWork,
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
    claim: &ClaimedWork,
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
