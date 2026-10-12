//! Read projections for an existing target's manual observation command.
//!
//! These reads do not create requests, authorize work, claim leases or change monitoring.
//! The command owner still revalidates the current primary purpose and admission on POST.

use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ManualObservationReceipt {
    pub receipt_ref: Uuid,
    pub outcome: String,
    pub reason: String,
    pub work_order_ref: Option<Uuid>,
    pub recorded_at: String,
}

/// A manual command without an explicit Domain accepts exactly one active primary purpose.
/// Two returned names mean ambiguous purposes, not a chooser that may silently pick the first.
pub async fn read_manual_observation_primary_domains(
    database: &Database,
    target_ref: Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT domain.name FROM observation_domain_target relation \
         JOIN observation_domain domain USING(domain_ref) \
         WHERE relation.target_ref=$1 AND relation.role='primary' AND domain.status='active' \
         ORDER BY domain.created_at,domain.domain_ref LIMIT 2",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
}

/// Read only the explicit manual-observation receipt belonging to this target. A missing or
/// wrong-kind receipt is not replaced by the target's latest, unrelated monitoring command.
pub async fn read_manual_observation_receipt(
    database: &Database,
    target_ref: Uuid,
    receipt_ref: Uuid,
) -> Result<Option<ManualObservationReceipt>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT command_receipt_ref,outcome,reason_code,work_order_ref, \
                linggan_human_moment(recorded_at) AS recorded_at \
         FROM collection_monitor_rule_command_receipt \
         WHERE target_ref=$1 AND command_receipt_ref=$2 AND command_kind='manual_observe'",
    )
    .bind(target_ref)
    .bind(receipt_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(ManualObservationReceipt {
            receipt_ref: row.try_get("command_receipt_ref")?,
            outcome: row.try_get("outcome")?,
            reason: row.try_get("reason_code")?,
            work_order_ref: row.try_get("work_order_ref")?,
            recorded_at: row.try_get("recorded_at")?,
        })
    })
    .transpose()
}
