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
/// Concurrency note: `ON CONFLICT DO UPDATE` is deliberate. With `DO NOTHING`, a transaction that
/// loses the race gets no row back, and a `SELECT` in the same statement cannot see the winner's
/// uncommitted row either - the loser would fail instead of continuing. `DO UPDATE` waits for the
/// competing transaction, then returns the winning row. The update itself is a no-op rewrite of
/// the same value, so no identity fact ever changes.
pub(crate) async fn resolve_source_content(
    transaction: &mut Transaction<'_, Postgres>,
    external_id: &str,
    identity_ref: Uuid,
    content_ref: Uuid,
) -> Result<i64, ProcessingError> {
    let identity_id = sqlx::query(
        "INSERT INTO source_identity \
             (source_identity_ref, source_system, namespace, object_type, external_id) \
         VALUES ($1, 'synthetic', 'scope-001', 'content', $2) \
         ON CONFLICT (source_system, namespace, object_type, external_id) \
         DO UPDATE SET external_id = source_identity.external_id \
         RETURNING id",
    )
    .bind(identity_ref)
    .bind(external_id)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(ProcessingError::internal)?;

    sqlx::query(
        "INSERT INTO source_content (content_ref, source_identity_id) VALUES ($1, $2) \
         ON CONFLICT (source_identity_id) \
         DO UPDATE SET source_identity_id = source_content.source_identity_id \
         RETURNING id",
    )
    .bind(content_ref)
    .bind(identity_id)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(ProcessingError::internal)
}
