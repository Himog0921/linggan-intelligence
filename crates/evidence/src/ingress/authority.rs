//! The immutable fences a locked routing row has to pass before acceptance is even considered.

use linggan_contracts::CapturePackage;

use crate::receipt::RejectionCode;
use crate::work_order::LockedRouting;

const CONTENT_CONTRACT_VERSION: &str = "content-detail.synthetic.v1";
const KNOWN_SET_BASIS: &str = "known_set";

/// Verifies epoch, the frozen v1 contract, and the frozen target. Authority revocation is
/// checked here too; the authority deadline is deliberately not, because it only applies to a
/// first acceptance and must be evaluated after the existing-package check.
pub(crate) fn evaluate_immutable_fence(
    routing: &LockedRouting,
    package: &CapturePackage,
) -> Option<RejectionCode> {
    if i64::from(routing.lease_epoch) != i64::from(package.routing().lease_epoch()) {
        return Some(RejectionCode::LeaseEpochMismatch);
    }
    if routing.contract_version != CONTENT_CONTRACT_VERSION
        || package.contract_version() != CONTENT_CONTRACT_VERSION
    {
        return Some(RejectionCode::TargetMismatch);
    }
    if routing.target_basis != KNOWN_SET_BASIS {
        return Some(RejectionCode::TargetMismatch);
    }
    if routing.known_target_count != Some(target_count(package)) {
        return Some(RejectionCode::TargetMismatch);
    }
    if routing.target_manifest_hash.as_deref() != Some(package.target().target_manifest_hash()) {
        return Some(RejectionCode::TargetMismatch);
    }
    if routing.authority_revoked {
        return Some(RejectionCode::AuthorityRevoked);
    }
    None
}

/// Only a first acceptance is subject to the authority deadline, and only against the database
/// clock read while this transaction already holds the routing lock.
pub(crate) fn evaluate_first_acceptance_deadline(routing: &LockedRouting) -> Option<RejectionCode> {
    routing
        .authority_expired
        .then_some(RejectionCode::AuthorityExpired)
}

fn target_count(package: &CapturePackage) -> i32 {
    i32::try_from(package.target().known_target_count()).unwrap_or(i32::MAX)
}
