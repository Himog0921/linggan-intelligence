//! Whether this capture identity already has an accepted package, and if so whether the new
//! delivery carries the same canonical hash.

use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::ingress::IngressError;

/// What the locked Attempt already holds. A package is never overwritten; a second delivery can
/// only replay the same hash or close as a conflict.
#[derive(Debug)]
pub(crate) enum ExistingPackage {
    None,
    SameHash {
        package_ref: Uuid,
        accepted_receipt_ref: Uuid,
        original_accepted_delivery_ref: Uuid,
    },
    DifferentHash {
        package_ref: Uuid,
    },
}

const EXISTING_PACKAGE_SQL: &str = "\
SELECT p.package_ref, p.package_hash, p.accepted_receipt_ref, d.delivery_ref \
FROM capture_package p \
JOIN capture_ingress_delivery d ON d.id = p.accepted_delivery_id \
WHERE p.attempt_id = $1";

pub(crate) async fn existing_package(
    transaction: &mut Transaction<'_, Postgres>,
    attempt_id: i64,
    delivered_package_hash: &str,
) -> Result<ExistingPackage, IngressError> {
    let Some(row) = sqlx::query(EXISTING_PACKAGE_SQL)
        .bind(attempt_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(IngressError::internal)?
    else {
        return Ok(ExistingPackage::None);
    };

    let package_ref: Uuid = row.get("package_ref");
    if row.get::<String, _>("package_hash") == delivered_package_hash {
        Ok(ExistingPackage::SameHash {
            package_ref,
            accepted_receipt_ref: row.get("accepted_receipt_ref"),
            original_accepted_delivery_ref: row.get("delivery_ref"),
        })
    } else {
        Ok(ExistingPackage::DifferentHash { package_ref })
    }
}
