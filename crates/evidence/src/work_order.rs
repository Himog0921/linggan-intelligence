//! Resolving and locking the routing rows a delivery claims to belong to.

use linggan_contracts::PackageRouting;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::ingress::{IngressError, PreRoutingCode};

/// The locked routing facts ingress is allowed to trust. Nothing here is taken from the request
/// body; every value is read from the rows this transaction holds a lock on.
#[derive(Debug)]
pub(crate) struct LockedRouting {
    pub(crate) work_order_id: i64,
    pub(crate) attempt_id: i64,
    pub(crate) capture_identity: Uuid,
    pub(crate) lease_epoch: i32,
    pub(crate) contract_version: String,
    pub(crate) target_basis: String,
    pub(crate) target_manifest_hash: Option<String>,
    pub(crate) known_target_count: Option<i32>,
    pub(crate) authority_revoked: bool,
    pub(crate) authority_expired: bool,
}

const LOCK_ROUTING_SQL: &str = "\
SELECT w.id AS work_order_id, w.work_order_ref, w.contract_version, w.target_basis, \
       w.target_manifest_hash, w.known_target_count, \
       a.id AS attempt_id, a.capture_identity, a.lease_epoch, \
       (a.authority_revoked_at IS NOT NULL) AS authority_revoked, \
       (a.authority_valid_until <= scope_001_now()) AS authority_expired \
FROM capture_attempt a \
JOIN capture_work_order w ON w.id = a.work_order_id \
WHERE a.attempt_ref = $1 \
FOR UPDATE OF a, w";

/// Locks the claimed Attempt and its Work, then verifies they belong together and to the claimed
/// Capture Identity. A missing reference never discloses which one was missing.
pub(crate) async fn lock_claimed_routing(
    transaction: &mut Transaction<'_, Postgres>,
    routing: &PackageRouting,
) -> Result<LockedRouting, IngressError> {
    let row = sqlx::query(LOCK_ROUTING_SQL)
        .bind(routing.attempt_ref())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(IngressError::internal)?
        .ok_or(IngressError::PreRouting(
            PreRoutingCode::RoutingReferenceNotFound,
        ))?;

    if row.get::<Uuid, _>("work_order_ref") != routing.work_order_ref() {
        return Err(IngressError::PreRouting(
            PreRoutingCode::WorkAttemptMismatch,
        ));
    }
    let capture_identity: Uuid = row.get("capture_identity");
    if capture_identity != routing.capture_identity() {
        return Err(IngressError::PreRouting(
            PreRoutingCode::AttemptCaptureMismatch,
        ));
    }

    Ok(LockedRouting {
        work_order_id: row.get("work_order_id"),
        attempt_id: row.get("attempt_id"),
        capture_identity,
        lease_epoch: row.get("lease_epoch"),
        contract_version: row.get("contract_version"),
        target_basis: row.get("target_basis"),
        target_manifest_hash: row.get("target_manifest_hash"),
        known_target_count: row.get("known_target_count"),
        authority_revoked: row.get("authority_revoked"),
        authority_expired: row.get("authority_expired"),
    })
}
