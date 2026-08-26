//! LOCAL-TRUSTED's loopback-only browser producer adapter.
//!
//! The adapter persists only task/attempt/submission provenance and receipts. It delegates the
//! immutable visible-card package to LOCAL-001; no scheduler, platform request, media, or
//! Evidence interpretation is hidden here.

use linggan_contracts::{
    LocalProducerAttempt, LocalProducerContractError, LocalProducerSubmission, LocalTaskSpec,
    parse_discovery_package,
};
use linggan_storage_postgres::Database;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::{DiscoveryIngressError, DiscoveryIngressOutcome, ingest_discovery_package};

#[derive(Debug, Error)]
pub enum LocalProducerError {
    #[error("the local producer contract is invalid: {0}")]
    Contract(#[from] LocalProducerContractError),
    #[error("the nested discovery package is invalid")]
    DiscoveryContract,
    #[error("the requested local task or attempt is not available")]
    RoutingNotFound,
    #[error("the producer identity does not own this attempt")]
    AttemptIdentityMismatch,
    #[error("the local producer transaction did not commit")]
    Internal(#[source] sqlx::Error),
    #[error("the local discovery admission did not commit")]
    DiscoveryIngress(#[source] DiscoveryIngressError),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum LocalTaskOutcome {
    Created { task_id: Uuid },
    Replay { task_id: Uuid },
    Conflict { task_id: Uuid },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum LocalAttemptOutcome {
    Started { attempt_id: Uuid, task_id: Uuid },
    Replay { attempt_id: Uuid, task_id: Uuid },
    Conflict { attempt_id: Uuid },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "delivery")]
pub enum LocalSubmissionOutcome {
    Acknowledged {
        submission_id: Uuid,
        receipt_ref: Uuid,
        discovery_package_ref: Uuid,
        discovery_receipt_ref: Uuid,
        discovery_admission: &'static str,
    },
    Replay {
        submission_id: Uuid,
        receipt_ref: Uuid,
        discovery_package_ref: Uuid,
        discovery_receipt_ref: Uuid,
        discovery_admission: &'static str,
    },
    Conflict {
        submission_id: Uuid,
    },
}

/// Checks the additive schema required for a local producer adapter without applying or repairing
/// anything. Discovery reads can remain ready even while this adapter is not yet migrated.
pub async fn local_producer_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('local_trusted_task') IS NOT NULL \
                AND to_regclass('local_trusted_attempt') IS NOT NULL \
                AND to_regclass('local_trusted_submission') IS NOT NULL \
                AND EXISTS (SELECT 1 FROM linggan_local_schema_migration \
                            WHERE migration_id = '0003_local_trusted_producer')",
    )
    .fetch_one(database.pool())
    .await
}

pub async fn create_manual_task(
    database: &Database,
    task: &LocalTaskSpec,
) -> Result<LocalTaskOutcome, LocalProducerError> {
    let task_hash = sha256_hex(&task.raw().to_string());
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(LocalProducerError::Internal)?;
    let existing =
        sqlx::query("SELECT task_spec_hash FROM local_trusted_task WHERE task_id = $1 FOR UPDATE")
            .bind(task.task_id())
            .fetch_optional(&mut *transaction)
            .await
            .map_err(LocalProducerError::Internal)?;
    let outcome = match existing {
        Some(row) if row.get::<String, _>("task_spec_hash") == task_hash => {
            LocalTaskOutcome::Replay {
                task_id: task.task_id(),
            }
        }
        Some(_) => LocalTaskOutcome::Conflict {
            task_id: task.task_id(),
        },
        None => {
            sqlx::query(
                "INSERT INTO local_trusted_task \
                 (task_id, task_spec_hash, task_spec, source, platform, page_type, maximum_quota) \
                 VALUES ($1, $2, $3, 'manual', 'xhs', 'search_results', 20)",
            )
            .bind(task.task_id())
            .bind(task_hash)
            .bind(task.raw())
            .execute(&mut *transaction)
            .await
            .map_err(LocalProducerError::Internal)?;
            LocalTaskOutcome::Created {
                task_id: task.task_id(),
            }
        }
    };
    transaction
        .commit()
        .await
        .map_err(LocalProducerError::Internal)?;
    Ok(outcome)
}

pub async fn start_local_attempt(
    database: &Database,
    attempt: &LocalProducerAttempt,
) -> Result<LocalAttemptOutcome, LocalProducerError> {
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(LocalProducerError::Internal)?;
    let task_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM local_trusted_task WHERE task_id = $1)",
    )
    .bind(attempt.task_id())
    .fetch_one(&mut *transaction)
    .await
    .map_err(LocalProducerError::Internal)?;
    if !task_exists {
        return Err(LocalProducerError::RoutingNotFound);
    }
    let existing = sqlx::query(
        "SELECT task_id, producer_instance_id FROM local_trusted_attempt WHERE attempt_id = $1 FOR UPDATE",
    )
    .bind(attempt.attempt_id())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(LocalProducerError::Internal)?;
    let outcome = match existing {
        Some(row)
            if row.get::<Uuid, _>("task_id") == attempt.task_id()
                && row.get::<Uuid, _>("producer_instance_id") == attempt.producer_instance_id() =>
        {
            LocalAttemptOutcome::Replay {
                attempt_id: attempt.attempt_id(),
                task_id: attempt.task_id(),
            }
        }
        Some(_) => LocalAttemptOutcome::Conflict {
            attempt_id: attempt.attempt_id(),
        },
        None => {
            sqlx::query(
                "INSERT INTO local_trusted_attempt \
                 (attempt_id, task_id, producer_instance_id, execution_state) \
                 VALUES ($1, $2, $3, 'started')",
            )
            .bind(attempt.attempt_id())
            .bind(attempt.task_id())
            .bind(attempt.producer_instance_id())
            .execute(&mut *transaction)
            .await
            .map_err(LocalProducerError::Internal)?;
            LocalAttemptOutcome::Started {
                attempt_id: attempt.attempt_id(),
                task_id: attempt.task_id(),
            }
        }
    };
    transaction
        .commit()
        .await
        .map_err(LocalProducerError::Internal)?;
    Ok(outcome)
}

/// Accepts a durable plugin delivery. A timeout between the nested package admission and this
/// receipt write is safe: retrying the exact submission replays the immutable discovery package
/// instead of creating a second observation surface.
pub async fn submit_local_package(
    database: &Database,
    submission: &LocalProducerSubmission,
) -> Result<LocalSubmissionOutcome, LocalProducerError> {
    let package_json = submission.discovery_package().to_string();
    parse_discovery_package(&package_json).map_err(|_| LocalProducerError::DiscoveryContract)?;
    let package_hash = sha256_hex(&package_json);
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(LocalProducerError::Internal)?;
    if let Some(outcome) = existing_submission(&mut transaction, submission, &package_hash).await? {
        transaction
            .commit()
            .await
            .map_err(LocalProducerError::Internal)?;
        return Ok(outcome);
    }
    assert_attempt_owner(&mut transaction, submission).await?;
    if let Some(outcome) = existing_submission(&mut transaction, submission, &package_hash).await? {
        transaction
            .commit()
            .await
            .map_err(LocalProducerError::Internal)?;
        return Ok(outcome);
    }
    if terminal_submission_exists(&mut transaction, submission).await? {
        transaction
            .commit()
            .await
            .map_err(LocalProducerError::Internal)?;
        return Ok(LocalSubmissionOutcome::Conflict {
            submission_id: submission.submission_id(),
        });
    }
    let admitted = ingest_discovery_package(database, &package_json)
        .await
        .map_err(LocalProducerError::DiscoveryIngress)?;
    let (package_ref, discovery_receipt_ref, admission) = discovery_refs(admitted);
    let receipt_ref = Uuid::new_v4();
    let insert = sqlx::query(
        "INSERT INTO local_trusted_submission \
         (submission_id, task_id, attempt_id, producer_instance_id, package_hash, receipt_ref, \
          discovery_package_ref, discovery_receipt_ref, discovery_admission, delivery_state) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'acknowledged')",
    )
    .bind(submission.submission_id())
    .bind(submission.task_id())
    .bind(submission.attempt_id())
    .bind(submission.producer_instance_id())
    .bind(&package_hash)
    .bind(receipt_ref)
    .bind(package_ref)
    .bind(discovery_receipt_ref)
    .bind(admission)
    .execute(&mut *transaction)
    .await;
    match insert {
        Ok(_) => {
            transaction
                .commit()
                .await
                .map_err(LocalProducerError::Internal)?;
            Ok(LocalSubmissionOutcome::Acknowledged {
                submission_id: submission.submission_id(),
                receipt_ref,
                discovery_package_ref: package_ref,
                discovery_receipt_ref,
                discovery_admission: admission,
            })
        }
        Err(error) if is_unique_violation(&error) => {
            existing_submission(&mut transaction, submission, &package_hash)
                .await?
                .ok_or(LocalProducerError::Internal(error))
        }
        Err(error) => Err(LocalProducerError::Internal(error)),
    }
}

async fn existing_submission(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &LocalProducerSubmission,
    package_hash: &str,
) -> Result<Option<LocalSubmissionOutcome>, LocalProducerError> {
    let row = sqlx::query(
        "SELECT task_id, attempt_id, producer_instance_id, package_hash, receipt_ref, \
                discovery_package_ref, discovery_receipt_ref, discovery_admission \
         FROM local_trusted_submission WHERE submission_id = $1",
    )
    .bind(submission.submission_id())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(LocalProducerError::Internal)?;
    Ok(row.map(|row| {
        let matches = row.get::<Uuid, _>("task_id") == submission.task_id()
            && row.get::<Uuid, _>("attempt_id") == submission.attempt_id()
            && row.get::<Uuid, _>("producer_instance_id") == submission.producer_instance_id()
            && row.get::<String, _>("package_hash") == package_hash;
        if !matches {
            return LocalSubmissionOutcome::Conflict {
                submission_id: submission.submission_id(),
            };
        }
        let discovery_admission = if row.get::<String, _>("discovery_admission") == "accepted" {
            "accepted"
        } else {
            "replay"
        };
        LocalSubmissionOutcome::Replay {
            submission_id: submission.submission_id(),
            receipt_ref: row.get("receipt_ref"),
            discovery_package_ref: row.get("discovery_package_ref"),
            discovery_receipt_ref: row.get("discovery_receipt_ref"),
            discovery_admission,
        }
    }))
}

async fn terminal_submission_exists(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &LocalProducerSubmission,
) -> Result<bool, LocalProducerError> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM local_trusted_submission WHERE attempt_id = $1)",
    )
    .bind(submission.attempt_id())
    .fetch_one(&mut **transaction)
    .await
    .map_err(LocalProducerError::Internal)
}

async fn assert_attempt_owner(
    transaction: &mut Transaction<'_, Postgres>,
    submission: &LocalProducerSubmission,
) -> Result<(), LocalProducerError> {
    let row = sqlx::query(
        "SELECT task_id, producer_instance_id FROM local_trusted_attempt WHERE attempt_id = $1 FOR UPDATE",
    )
    .bind(submission.attempt_id())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(LocalProducerError::Internal)?
    .ok_or(LocalProducerError::RoutingNotFound)?;
    if row.get::<Uuid, _>("task_id") != submission.task_id()
        || row.get::<Uuid, _>("producer_instance_id") != submission.producer_instance_id()
    {
        return Err(LocalProducerError::AttemptIdentityMismatch);
    }
    Ok(())
}

fn discovery_refs(outcome: DiscoveryIngressOutcome) -> (Uuid, Uuid, &'static str) {
    match outcome {
        DiscoveryIngressOutcome::Accepted {
            package_ref,
            accepted_receipt_ref,
            ..
        } => (package_ref, accepted_receipt_ref, "accepted"),
        DiscoveryIngressOutcome::Replay {
            package_ref,
            accepted_receipt_ref,
            ..
        } => (package_ref, accepted_receipt_ref, "replay"),
    }
}

fn sha256_hex(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23505"))
}
