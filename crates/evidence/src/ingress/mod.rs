//! Capture package ingress: the single place that decides accepted, replay, conflict or
//! rejected, and the only writer of accepted capture evidence.

mod authority;
mod idempotency;
mod transaction;

use linggan_contracts::{ContractError, parse_capture_package};
use linggan_storage_postgres::Database;
use thiserror::Error;
use uuid::Uuid;

use crate::receipt::{IngressOutcome, RefSequence, RejectionCode};
use crate::work_order::lock_claimed_routing;
use authority::{evaluate_first_acceptance_deadline, evaluate_immutable_fence};
use idempotency::{ExistingPackage, existing_package};
use transaction::{insert_public_delivery, write_accepted_facts};

/// Why a delivery was refused before its routing rows could be confirmed. These never disclose
/// whether a particular resource exists and never produce a queryable delivery ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreRoutingCode {
    MalformedJson,
    CanonicalizationInvalid,
    PackageSchemaInvalid,
    PackageHashInvalid,
    RecordHashInvalid,
    RoutingReferenceNotFound,
    WorkAttemptMismatch,
    AttemptCaptureMismatch,
}

impl PreRoutingCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MalformedJson => "malformed_json",
            Self::CanonicalizationInvalid => "canonicalization_invalid",
            Self::PackageSchemaInvalid => "package_schema_invalid",
            Self::PackageHashInvalid => "package_hash_invalid",
            Self::RecordHashInvalid => "record_hash_invalid",
            Self::RoutingReferenceNotFound => "routing_reference_not_found",
            Self::WorkAttemptMismatch => "work_attempt_mismatch",
            Self::AttemptCaptureMismatch => "attempt_capture_mismatch",
        }
    }
}

#[derive(Debug, Error)]
pub enum IngressError {
    #[error("the delivery was refused before routing could be confirmed: {}", .0.as_str())]
    PreRouting(PreRoutingCode),
    #[error("the ingress transaction failed before commit: {0}")]
    Internal(String),
}

impl IngressError {
    /// A transaction failure is never a client rejection, and never leaves a receipt behind.
    pub fn is_internal(&self) -> bool {
        matches!(self, Self::Internal(_))
    }

    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }

    pub(crate) fn emitted_result_without_record() -> Self {
        Self::Internal("an emitted target result named no record in its own package".to_owned())
    }
}

impl From<ContractError> for IngressError {
    fn from(error: ContractError) -> Self {
        Self::PreRouting(match error {
            ContractError::InvalidJson(_) => PreRoutingCode::MalformedJson,
            ContractError::CanonicalizationInvalid(_) => PreRoutingCode::CanonicalizationInvalid,
            ContractError::PackageSchemaInvalid(_)
            | ContractError::UnsupportedPackageSchema(_)
            | ContractError::UnsupportedContentContract(_) => PreRoutingCode::PackageSchemaInvalid,
            ContractError::PackageHashInvalid => PreRoutingCode::PackageHashInvalid,
            ContractError::RecordHashInvalid => PreRoutingCode::RecordHashInvalid,
        })
    }
}

/// Where an ingress transaction can be told to fail. This exists so the proof can show that a
/// failure at any point leaves no half-written facts; production ingress never sets one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngressFault {
    AfterRefsGenerated,
    AfterAcceptedDelivery,
    AfterPackage,
    DuringRecords,
    BeforeCoverage,
    DuringProcessingWork,
}

/// Ingress inputs that a proof run needs to pin down: which public refs are minted, and whether
/// the transaction is told to fail. Both default to real behaviour.
#[derive(Debug)]
pub struct IngressOptions {
    refs: RefSequence,
    fault: Option<IngressFault>,
}

impl Default for IngressOptions {
    fn default() -> Self {
        Self {
            refs: RefSequence::random(),
            fault: None,
        }
    }
}

impl IngressOptions {
    /// Pins the minted refs to a frozen sequence so a proof run is reproducible.
    pub fn use_fixed_refs(&mut self, values: Vec<Uuid>) {
        self.refs = RefSequence::fixed(values);
    }

    pub fn inject_fault(&mut self, fault: IngressFault) {
        self.fault = Some(fault);
    }

    pub(crate) fn mint_ref(&self) -> Uuid {
        self.refs.mint()
    }

    pub(crate) fn fail_at(&self, point: IngressFault) -> Result<(), IngressError> {
        if self.fault == Some(point) {
            return Err(IngressError::Internal(format!(
                "injected ingress fault at {point:?}"
            )));
        }
        Ok(())
    }
}

/// Ingests one capture package delivery.
pub async fn ingest_capture_package(
    database: &Database,
    body: &str,
) -> Result<IngressOutcome, IngressError> {
    ingest_capture_package_with(database, body, &IngressOptions::default()).await
}

/// Ingests one capture package delivery under explicit proof options.
pub async fn ingest_capture_package_with(
    database: &Database,
    body: &str,
    options: &IngressOptions,
) -> Result<IngressOutcome, IngressError> {
    let package = parse_capture_package(body)?;
    let mut transaction = database
        .pool()
        .begin()
        .await
        .map_err(IngressError::internal)?;

    let routing = lock_claimed_routing(&mut transaction, package.routing()).await?;
    if let Some(code) = evaluate_immutable_fence(&routing, &package) {
        return close_as_rejected(transaction, &routing, code, options).await;
    }

    let outcome =
        match existing_package(&mut transaction, routing.attempt_id, package.package_hash()).await?
        {
            ExistingPackage::SameHash {
                package_ref,
                accepted_receipt_ref,
                original_accepted_delivery_ref,
            } => {
                let delivery_ref = options.mint_ref();
                insert_public_delivery(&mut transaction, &routing, "replay", None, delivery_ref)
                    .await?;
                IngressOutcome::Replay {
                    delivery_ref,
                    package_ref,
                    accepted_receipt_ref,
                    original_accepted_delivery_ref,
                }
            }
            ExistingPackage::DifferentHash { package_ref } => {
                let delivery_ref = options.mint_ref();
                insert_public_delivery(&mut transaction, &routing, "conflict", None, delivery_ref)
                    .await?;
                IngressOutcome::Conflict {
                    delivery_ref,
                    package_ref,
                }
            }
            ExistingPackage::None => {
                if let Some(code) = evaluate_first_acceptance_deadline(&routing) {
                    return close_as_rejected(transaction, &routing, code, options).await;
                }
                let refs =
                    write_accepted_facts(&mut transaction, &routing, &package, options).await?;
                IngressOutcome::Accepted {
                    delivery_ref: refs.delivery_ref,
                    package_ref: refs.package_ref,
                    accepted_receipt_ref: refs.accepted_receipt_ref,
                }
            }
        };

    transaction.commit().await.map_err(IngressError::internal)?;
    Ok(outcome)
}

async fn close_as_rejected(
    mut transaction: sqlx::Transaction<'_, sqlx::Postgres>,
    routing: &crate::work_order::LockedRouting,
    code: RejectionCode,
    options: &IngressOptions,
) -> Result<IngressOutcome, IngressError> {
    let delivery_ref = options.mint_ref();
    insert_public_delivery(
        &mut transaction,
        routing,
        "rejected",
        Some(code.as_str()),
        delivery_ref,
    )
    .await?;
    transaction.commit().await.map_err(IngressError::internal)?;
    Ok(IngressOutcome::Rejected { delivery_ref, code })
}
