//! The identity ruling: which of the three identity statements, if any, may establish a source
//! identity for this record.

use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::processing::{BusinessOutcome, ProcessingError};

/// What the ruling table decided for one record.
pub(crate) enum IdentityRuling {
    /// The envelope's own external id is authoritative and cross-checks cleanly.
    Resolved(String),
    /// A closed business outcome that forms no source object at all.
    NotFormed(BusinessOutcome),
}

/// SCOPE-001 freezes this table. `targetExternalId` states which execution target the record was
/// delivered for, `source.externalId` is the only statement that may establish an identity, and
/// `payload.sourceExternalId` only cross-checks. A payload value never fills in for a null
/// envelope value, and a target value never does either.
pub(crate) fn rule_on_identity(
    target_external_id: &str,
    source_external_id: Option<&str>,
    payload_source_external_id: Option<&str>,
) -> IdentityRuling {
    match (source_external_id, payload_source_external_id) {
        // The envelope agrees with the target, and the payload either agrees or stays silent.
        (Some(source), payload)
            if source == target_external_id
                && payload.is_none_or(|payload| payload == target_external_id) =>
        {
            IdentityRuling::Resolved(source.to_owned())
        }
        // No envelope statement and no payload statement: honest lack of identity, not an error.
        (None, None) => IdentityRuling::NotFormed(BusinessOutcome::SourceIdentityUnresolved),
        // Everything else is a disagreement between statements that must not be resolved by
        // preferring one of them.
        _ => IdentityRuling::NotFormed(BusinessOutcome::SourceIdentityConflict),
    }
}

/// Resolves the identity and its content anchor, reusing both when they already exist.
///
/// Concurrency note: resolution for one external id is serialized by a transaction-scoped
/// advisory lock. Two earlier shapes were rejected: `ON CONFLICT DO NOTHING` leaves the loser of
/// a race with no row and no way to see the winner's uncommitted one, and `ON CONFLICT DO UPDATE`
/// would require granting the runtime role UPDATE on the identity tables - exactly the privilege
/// it must not have. An advisory lock needs no table privileges at all, and the loser simply
/// waits and then reads the committed row.
pub(crate) async fn resolve_source_content(
    transaction: &mut Transaction<'_, Postgres>,
    external_id: &str,
    identity_ref: Uuid,
    content_ref: Uuid,
) -> Result<i64, ProcessingError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("scope-001:content-identity:{external_id}"))
        .execute(&mut **transaction)
        .await
        .map_err(ProcessingError::internal)?;

    let identity_id = match existing_identity_id(transaction, external_id).await? {
        Some(id) => id,
        None => sqlx::query(
            "INSERT INTO source_identity \
                 (source_identity_ref, source_system, namespace, object_type, external_id) \
             VALUES ($1, 'synthetic', 'scope-001', 'content', $2) RETURNING id",
        )
        .bind(identity_ref)
        .bind(external_id)
        .fetch_one(&mut **transaction)
        .await
        .map(|row| row.get::<i64, _>("id"))
        .map_err(ProcessingError::internal)?,
    };

    match existing_content_id(transaction, identity_id).await? {
        Some(id) => Ok(id),
        None => sqlx::query(
            "INSERT INTO source_content (content_ref, source_identity_id) VALUES ($1, $2) \
             RETURNING id",
        )
        .bind(content_ref)
        .bind(identity_id)
        .fetch_one(&mut **transaction)
        .await
        .map(|row| row.get::<i64, _>("id"))
        .map_err(ProcessingError::internal),
    }
}

async fn existing_identity_id(
    transaction: &mut Transaction<'_, Postgres>,
    external_id: &str,
) -> Result<Option<i64>, ProcessingError> {
    sqlx::query(
        "SELECT id FROM source_identity \
         WHERE source_system = 'synthetic' AND namespace = 'scope-001' \
           AND object_type = 'content' AND external_id = $1",
    )
    .bind(external_id)
    .fetch_optional(&mut **transaction)
    .await
    .map(|row| row.map(|row| row.get::<i64, _>("id")))
    .map_err(ProcessingError::internal)
}

async fn existing_content_id(
    transaction: &mut Transaction<'_, Postgres>,
    identity_id: i64,
) -> Result<Option<i64>, ProcessingError> {
    sqlx::query("SELECT id FROM source_content WHERE source_identity_id = $1")
        .bind(identity_id)
        .fetch_optional(&mut **transaction)
        .await
        .map(|row| row.map(|row| row.get::<i64, _>("id")))
        .map_err(ProcessingError::internal)
}
