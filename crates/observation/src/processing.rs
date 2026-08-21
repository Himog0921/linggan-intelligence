//! Claiming one ready processing work and asking PostgreSQL's restricted processing interface to
//! form the corresponding facts. Rust never receives direct fact-table write privileges: the
//! database binds every write to the accepted Record and durable lease that already exist.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
use thiserror::Error;
use tokio::sync::Barrier;
use uuid::Uuid;

/// The closed set of business results a finalized work may hold. Runtime level
/// (ready/leased/finalized) is separate from this business result.
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

    fn parse(value: &str) -> Result<Self, ProcessingError> {
        match value {
            "observation_recorded" => Ok(Self::ObservationRecorded),
            "source_identity_unresolved" => Ok(Self::SourceIdentityUnresolved),
            "source_identity_conflict" => Ok(Self::SourceIdentityConflict),
            "record_contract_invalid" => Ok(Self::RecordContractInvalid),
            _ => Err(ProcessingError::Internal(format!(
                "the database returned an unrecognized processing outcome: {value}"
            ))),
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
    /// No work is ready. This is not evidence that every work has completed.
    NothingReady,
}

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("the processing operation failed: {0}")]
    Internal(String),
    #[error("this lease is no longer the current epoch for the work")]
    LeaseLost,
    #[error("processing failed: {processing}; run-error persistence also failed: {persistence}")]
    RunErrorPersistence {
        processing: String,
        persistence: String,
    },
}

impl ProcessingError {
    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        let text = error.to_string();
        if text.contains("processing claim is no longer current") {
            Self::LeaseLost
        } else {
            Self::Internal(text)
        }
    }
}

#[derive(Debug)]
enum RefSource {
    Random,
    Frozen(Mutex<VecDeque<Uuid>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingFault {
    AfterObservation,
}

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
    /// The public refs created by one normal F01 run: attempt, identity, content, observation,
    /// revision, title source and body source.
    pub fn use_fixed_refs(&mut self, values: Vec<Uuid>) {
        self.refs = RefSource::Frozen(Mutex::new(values.into()));
    }

    pub fn inject_fault(&mut self, fault: ProcessingFault) {
        self.fault = Some(fault);
    }

    pub fn synchronize_before_identity(&mut self, rendezvous: Arc<Barrier>) {
        self.rendezvous = Some(rendezvous);
    }

    async fn wait_for_other_runs(&self) {
        if let Some(rendezvous) = &self.rendezvous {
            rendezvous.wait().await;
        }
    }

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
                .expect("the frozen proof reference sequence ran out; a fixed reference must never be replaced by a random UUID"),
        }
    }

    fn fault_after_observation(&self) -> bool {
        self.fault == Some(ProcessingFault::AfterObservation)
    }
}

/// A durable claim carries a read-only copy of the exact accepted Record only so the proof
/// harness can decide whether its frozen public refs must be consumed. The database-owned write
/// operation below re-reads and validates that same accepted Record before it persists anything.
/// Runtime never receives direct fact-table write privilege.
#[derive(Debug)]
pub struct ClaimedRecord {
    processing_work_ref: Uuid,
    record_ref: Uuid,
    epoch: i32,
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

/// Commits a durable lease before any processing operation. Only the database function can make
/// this attempt; runtime gets EXECUTE rather than a fact-table INSERT grant.
pub async fn claim_one_ready_record(
    database: &Database,
    options: &ProcessingOptions,
) -> Result<Option<ClaimedRecord>, ProcessingError> {
    let row = sqlx::query(
        "SELECT processing_work_ref, record_ref, epoch, target_external_id, source_external_id, payload \
         FROM scope_001_claim_processing_work($1)",
    )
    .bind(options.mint())
    .fetch_optional(database.pool())
    .await
    .map_err(ProcessingError::internal)?;

    Ok(row.map(|row| ClaimedRecord {
        processing_work_ref: row.get("processing_work_ref"),
        record_ref: row.get("record_ref"),
        epoch: row.get("epoch"),
        target_external_id: row.get("target_external_id"),
        source_external_id: row.get("source_external_id"),
        payload: row.get("payload"),
    }))
}

pub async fn run_claimed_record(
    database: &Database,
    claim: ClaimedRecord,
    options: &ProcessingOptions,
) -> Result<ProcessedRecord, ProcessingError> {
    options.wait_for_other_runs().await;
    let fact_refs = if accepted_record_can_form_observation(&claim) {
        Some([
            options.mint(),
            options.mint(),
            options.mint(),
            options.mint(),
            options.mint(),
            options.mint(),
        ])
    } else {
        None
    };
    let result = sqlx::query_scalar::<_, String>(
        "SELECT scope_001_process_claimed_record($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(claim.processing_work_ref)
    .bind(claim.epoch)
    .bind(fact_refs.map(|refs| refs[0]))
    .bind(fact_refs.map(|refs| refs[1]))
    .bind(fact_refs.map(|refs| refs[2]))
    .bind(fact_refs.map(|refs| refs[3]))
    .bind(fact_refs.map(|refs| refs[4]))
    .bind(fact_refs.map(|refs| refs[5]))
    .bind(options.fault_after_observation())
    .fetch_one(database.pool())
    .await;

    match result {
        Ok(outcome) => Ok(ProcessedRecord {
            processing_work_ref: claim.processing_work_ref,
            record_ref: claim.record_ref,
            epoch: claim.epoch,
            business_outcome: BusinessOutcome::parse(&outcome)?,
        }),
        Err(error) => {
            let processing = ProcessingError::internal(error);
            match record_run_error(database, &claim, &processing).await {
                Ok(()) => Err(processing),
                Err(persistence) => Err(ProcessingError::RunErrorPersistence {
                    processing: processing.to_string(),
                    persistence: persistence.to_string(),
                }),
            }
        }
    }
}

/// This derives only frozen-ref consumption from the exact Record returned by the narrow claim
/// operation. It is deliberately not an authorization check: PostgreSQL repeats the closed
/// payload and identity validation before it writes any fact, and has no caller-provided source,
/// content, observation, time, field value, Current or business-outcome input to trust.
fn accepted_record_can_form_observation(claim: &ClaimedRecord) -> bool {
    let Some(payload_source) = parse_payload_source_external_id(&claim.payload) else {
        return false;
    };
    claim
        .source_external_id
        .as_deref()
        .is_some_and(|source| source == claim.target_external_id)
        && payload_source
            .as_deref()
            .is_none_or(|payload| payload == claim.target_external_id)
}

fn parse_payload_source_external_id(payload: &Value) -> Option<Option<String>> {
    let object = payload.as_object()?;
    if object.len() != 3 || object.get("schemaVersion")?.as_str()? != "content-detail.synthetic.v1"
    {
        return None;
    }
    let source = match object.get("sourceExternalId")? {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        _ => return None,
    };
    let fields = object.get("fields")?.as_object()?;
    if fields.len() != 2
        || !field_is_closed(fields.get("title")?)
        || !field_is_closed(fields.get("body")?)
    {
        return None;
    }
    Some(source)
}

fn field_is_closed(field: &Value) -> bool {
    let Some(object) = field.as_object() else {
        return false;
    };
    if object.len() != 2 {
        return false;
    }
    matches!(
        (object.get("observed"), object.get("value")),
        (Some(Value::Bool(true)), Some(Value::String(_)))
            | (Some(Value::Bool(false)), Some(Value::Null))
    )
}

async fn record_run_error(
    database: &Database,
    claim: &ClaimedRecord,
    error: &ProcessingError,
) -> Result<(), ProcessingError> {
    sqlx::query("SELECT scope_001_record_processing_run_error($1, $2, $3)")
        .bind(claim.processing_work_ref)
        .bind(claim.epoch)
        .bind(error.to_string())
        .execute(database.pool())
        .await
        .map_err(ProcessingError::internal)?;
    Ok(())
}
