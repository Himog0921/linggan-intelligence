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

/// Resolves the identity and its content anchor, reusing both when they already exist. The unique
/// constraint on the identity tuple is what makes concurrent resolution safe.
pub(crate) async fn resolve_source_content(
    transaction: &mut Transaction<'_, Postgres>,
    external_id: &str,
    identity_ref: Uuid,
    content_ref: Uuid,
) -> Result<i64, ProcessingError> {
    let identity_id = sqlx::query(
        "WITH inserted AS ( \
             INSERT INTO source_identity (source_identity_ref, source_system, namespace, object_type, external_id) \
             VALUES ($1, 'synthetic', 'scope-001', 'content', $2) \
             ON CONFLICT (source_system, namespace, object_type, external_id) DO NOTHING \
             RETURNING id \
         ) \
         SELECT id FROM inserted \
         UNION ALL \
         SELECT id FROM source_identity \
         WHERE source_system = 'synthetic' AND namespace = 'scope-001' \
           AND object_type = 'content' AND external_id = $2 \
         LIMIT 1",
    )
    .bind(identity_ref)
    .bind(external_id)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(ProcessingError::internal)?;

    sqlx::query(
        "WITH inserted AS ( \
             INSERT INTO source_content (content_ref, source_identity_id) VALUES ($1, $2) \
             ON CONFLICT (source_identity_id) DO NOTHING \
             RETURNING id \
         ) \
         SELECT id FROM inserted \
         UNION ALL \
         SELECT id FROM source_content WHERE source_identity_id = $2 \
         LIMIT 1",
    )
    .bind(content_ref)
    .bind(identity_id)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(ProcessingError::internal)
}
