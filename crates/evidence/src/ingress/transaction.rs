//! The accepted-path write sequence. Every row below belongs to one transaction; a fault at any
//! point leaves no delivery, package, record, coverage or processing work behind.

use linggan_contracts::{CapturePackage, KnownTargetOutcome};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::ingress::{IngressError, IngressFault, IngressOptions};
use crate::work_order::LockedRouting;

/// The refs a first acceptance minted before it wrote anything.
#[derive(Debug)]
pub(crate) struct AcceptedRefs {
    pub(crate) delivery_ref: Uuid,
    pub(crate) package_ref: Uuid,
    pub(crate) accepted_receipt_ref: Uuid,
}

pub(crate) async fn insert_public_delivery(
    transaction: &mut Transaction<'_, Postgres>,
    routing: &LockedRouting,
    outcome: &str,
    external_code: Option<&str>,
    delivery_ref: Uuid,
) -> Result<i64, IngressError> {
    sqlx::query(
        "INSERT INTO capture_ingress_delivery \
             (audit_kind, delivery_ref, work_order_id, attempt_id, capture_identity, outcome, external_code) \
         VALUES ('public_delivery', $1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(delivery_ref)
    .bind(routing.work_order_id)
    .bind(routing.attempt_id)
    .bind(routing.capture_identity)
    .bind(outcome)
    .bind(external_code)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(IngressError::internal)
}

/// Writes the accepted delivery, package, records, target results, coverage and processing work,
/// then freezes the attempt's terminal reason.
pub(crate) async fn write_accepted_facts(
    transaction: &mut Transaction<'_, Postgres>,
    routing: &LockedRouting,
    package: &CapturePackage,
    options: &IngressOptions,
) -> Result<AcceptedRefs, IngressError> {
    // The mint order is frozen by the fixture manifest: delivery, accepted receipt, package.
    let delivery_ref = options.mint_ref();
    let accepted_receipt_ref = options.mint_ref();
    let refs = AcceptedRefs {
        delivery_ref,
        accepted_receipt_ref,
        package_ref: options.mint_ref(),
    };
    options.fail_at(IngressFault::AfterRefsGenerated)?;

    let delivery_id =
        insert_public_delivery(transaction, routing, "accepted", None, refs.delivery_ref).await?;
    options.fail_at(IngressFault::AfterAcceptedDelivery)?;

    let package_id = insert_package(transaction, routing, package, &refs, delivery_id).await?;
    options.fail_at(IngressFault::AfterPackage)?;

    let record_ids = insert_records(transaction, package, package_id, options).await?;
    insert_target_results(transaction, routing, package, package_id, &record_ids).await?;
    options.fail_at(IngressFault::BeforeCoverage)?;

    insert_coverage(transaction, package, package_id).await?;
    insert_processing_work(transaction, &record_ids, options).await?;
    freeze_attempt_terminal(transaction, routing, package).await?;

    Ok(refs)
}

async fn insert_package(
    transaction: &mut Transaction<'_, Postgres>,
    routing: &LockedRouting,
    package: &CapturePackage,
    refs: &AcceptedRefs,
    accepted_delivery_id: i64,
) -> Result<i64, IngressError> {
    sqlx::query(
        "INSERT INTO capture_package \
             (package_ref, work_order_id, attempt_id, capture_identity, package_hash, accepted_delivery_id, accepted_receipt_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(refs.package_ref)
    .bind(routing.work_order_id)
    .bind(routing.attempt_id)
    .bind(routing.capture_identity)
    .bind(package.package_hash())
    .bind(accepted_delivery_id)
    .bind(refs.accepted_receipt_ref)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(IngressError::internal)
}

/// Returns each record's database id keyed by the ordinal the package declared.
async fn insert_records(
    transaction: &mut Transaction<'_, Postgres>,
    package: &CapturePackage,
    package_id: i64,
    options: &IngressOptions,
) -> Result<Vec<(u32, i64)>, IngressError> {
    let mut record_ids = Vec::with_capacity(package.records().len());
    for record in package.records() {
        // The whole verified envelope is persisted, not just the payload: the source statement is
        // the only value allowed to establish an identity later, and observed_at is the only time
        // the current-value policy may order by.
        let id = sqlx::query(
            "INSERT INTO capture_record \
                 (record_ref, package_id, ordinal, target_external_id, record_hash, payload, \
                  record_kind, source_system, source_namespace, source_object_type, \
                  source_external_id, source_channel, observed_at, observed_at_precision, \
                  observed_at_basis) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13::timestamptz, $14, $15) \
             RETURNING id",
        )
        .bind(options.mint_ref())
        .bind(package_id)
        .bind(i32::try_from(record.ordinal()).unwrap_or(i32::MAX))
        .bind(record.target_external_id())
        .bind(record.record_hash())
        .bind(record.payload())
        .bind(record.record_kind())
        .bind(record.source_system())
        .bind(record.source_namespace())
        .bind(record.source_object_type())
        .bind(record.source_external_id())
        .bind(record.source_channel())
        .bind(record.observed_at_value())
        .bind(record.observed_at_precision())
        .bind(record.observed_at_basis())
        .fetch_one(&mut **transaction)
        .await
        .map(|row| row.get::<i64, _>("id"))
        .map_err(IngressError::internal)?;
        record_ids.push((record.ordinal(), id));
        if record_ids.len() == 1 {
            options.fail_at(IngressFault::DuringRecords)?;
        }
    }
    Ok(record_ids)
}

async fn insert_target_results(
    transaction: &mut Transaction<'_, Postgres>,
    routing: &LockedRouting,
    package: &CapturePackage,
    package_id: i64,
    record_ids: &[(u32, i64)],
) -> Result<(), IngressError> {
    for result in package.known_target_results() {
        let record_id = match result.outcome() {
            KnownTargetOutcome::Emitted => Some(
                record_id_for_ordinal(record_ids, result.record_ordinal())
                    .ok_or_else(IngressError::emitted_result_without_record)?,
            ),
            KnownTargetOutcome::Failed | KnownTargetOutcome::NotAttempted => None,
        };
        sqlx::query(
            "INSERT INTO capture_package_target_result (package_id, work_order_id, target_id, outcome, record_id, reason) \
             SELECT $1, $2, target.id, $3, $4, $5 FROM capture_work_order_target target \
             WHERE target.work_order_id = $2 AND target.external_id = $6",
        )
        .bind(package_id)
        .bind(routing.work_order_id)
        .bind(result.outcome().as_str())
        .bind(record_id)
        .bind(result.reason())
        .bind(result.target_external_id())
        .execute(&mut **transaction)
        .await
        .map_err(IngressError::internal)?;
    }
    Ok(())
}

async fn insert_coverage(
    transaction: &mut Transaction<'_, Postgres>,
    package: &CapturePackage,
    package_id: i64,
) -> Result<(), IngressError> {
    let coverage = package.coverage();
    sqlx::query(
        "INSERT INTO capture_package_coverage (package_id, unit, attempted, emitted, failed, known_not_attempted, remaining_scope) \
         VALUES ($1, 'content_detail', $2, $3, $4, $5, $6)",
    )
    .bind(package_id)
    .bind(i32::try_from(coverage.attempted()).unwrap_or(i32::MAX))
    .bind(i32::try_from(coverage.emitted()).unwrap_or(i32::MAX))
    .bind(i32::try_from(coverage.failed()).unwrap_or(i32::MAX))
    .bind(
        coverage
            .known_not_attempted()
            .map(|value| i32::try_from(value).unwrap_or(i32::MAX)),
    )
    .bind(coverage.remaining_scope().as_str())
    .execute(&mut **transaction)
    .await
    .map_err(IngressError::internal)?;
    Ok(())
}

async fn insert_processing_work(
    transaction: &mut Transaction<'_, Postgres>,
    record_ids: &[(u32, i64)],
    options: &IngressOptions,
) -> Result<(), IngressError> {
    for (index, (_, record_id)) in record_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO record_processing_work (processing_work_ref, capture_record_id, processor_version) \
             VALUES ($1, $2, 'content-detail-processor-v1')",
        )
        .bind(options.mint_ref())
        .bind(record_id)
        .execute(&mut **transaction)
        .await
        .map_err(IngressError::internal)?;
        if index == 0 {
            options.fail_at(IngressFault::DuringProcessingWork)?;
        }
    }
    Ok(())
}

async fn freeze_attempt_terminal(
    transaction: &mut Transaction<'_, Postgres>,
    routing: &LockedRouting,
    package: &CapturePackage,
) -> Result<(), IngressError> {
    sqlx::query("UPDATE capture_attempt SET terminal_reason = $1 WHERE id = $2")
        .bind(package.terminal().reason().as_str())
        .bind(routing.attempt_id)
        .execute(&mut **transaction)
        .await
        .map_err(IngressError::internal)?;
    Ok(())
}

fn record_id_for_ordinal(record_ids: &[(u32, i64)], ordinal: Option<u32>) -> Option<i64> {
    let ordinal = ordinal?;
    record_ids
        .iter()
        .find(|(record_ordinal, _)| *record_ordinal == ordinal)
        .map(|(_, id)| *id)
}
