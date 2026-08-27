//! COLLECTION-001 · Observation target persistence.
//!
//! Storing a target proves it was saved. It is not an Acquisition Request, not an
//! Authorization, and not a claim that any capture will run (INV-36). Nothing in this module
//! reaches a platform, and nothing here creates a Work Order.

use linggan_contracts::{CollectionContractError, LifecycleState, TargetIdentity, TargetSource};
use linggan_storage_postgres::Database;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CollectionTargetError {
    #[error("collection target schema is not applied")]
    SchemaUnavailable,
    #[error(transparent)]
    Contract(#[from] CollectionContractError),
    #[error("that lifecycle move is not allowed: {from} -> {to}")]
    IllegalTransition { from: String, to: String },
    #[error("no observation target with that reference")]
    UnknownTarget,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A stored target as the read side sees it.
#[derive(Debug, Clone)]
pub struct ObservationTarget {
    pub target_ref: Uuid,
    pub platform: String,
    pub target_kind: String,
    pub identity_key: String,
    pub display_name: Option<String>,
    pub identity_facts: Option<Value>,
    pub source: String,
    pub lifecycle_state: String,
    pub first_stored_at: String,
}

/// Whether a store call created a target or found the one already there.
///
/// The distinction is kept out of the caller's reach as a boolean deliberately: a push that
/// lands on an existing target is a normal outcome, not a failure, but the page must be able
/// to say which happened rather than silently implying a new target was created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreOutcome {
    Stored,
    AlreadyPresent,
}

pub async fn collection_target_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('collection_observation_target') IS NOT NULL \
             AND to_regclass('collection_observation_target_transition') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// Store a target in `pending_decision`, or return the one that is already there.
///
/// Deduplication is enforced by the unique index on (platform, kind, identity_key), not by a
/// read-then-write in application code: two pushes arriving together must not both succeed.
/// The legacy workbench has no such index, and its failure mode is a silent parallel duplicate
/// with its own independent baseline.
pub async fn store_pending_target(
    database: &Database,
    identity: &TargetIdentity,
    source: TargetSource,
    display_name: Option<&str>,
    identity_facts: Option<&Value>,
) -> Result<(ObservationTarget, StoreOutcome), CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }

    let target_ref = Uuid::new_v4();
    let inserted = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO collection_observation_target \
             (target_ref, platform, target_kind, identity_key, display_name, identity_facts, source) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         ON CONFLICT (platform, target_kind, identity_key) DO NOTHING \
         RETURNING target_ref",
    )
    .bind(target_ref)
    .bind(identity.platform())
    .bind(identity.kind().as_str())
    .bind(identity.key())
    .bind(display_name)
    .bind(identity_facts)
    .bind(source.as_str())
    .fetch_optional(database.pool())
    .await?;

    let stored = read_target_by_identity(database, identity).await?;

    if inserted.is_some() {
        record_transition(
            database,
            stored.target_ref,
            None,
            LifecycleState::PendingDecision,
            "person",
            "target_stored",
            None,
        )
        .await?;
        Ok((stored, StoreOutcome::Stored))
    } else {
        Ok((stored, StoreOutcome::AlreadyPresent))
    }
}

async fn read_target_by_identity(
    database: &Database,
    identity: &TargetIdentity,
) -> Result<ObservationTarget, CollectionTargetError> {
    let row = sqlx::query_as::<_, TargetRow>(
        "SELECT target_ref, platform, target_kind, identity_key, display_name, identity_facts, \
                source, lifecycle_state, to_char(first_stored_at, 'YYYY-MM-DD\"T\"HH24:MI:SSOF') \
         FROM collection_observation_target \
         WHERE platform = $1 AND target_kind = $2 AND identity_key = $3",
    )
    .bind(identity.platform())
    .bind(identity.kind().as_str())
    .bind(identity.key())
    .fetch_optional(database.pool())
    .await?
    .ok_or(CollectionTargetError::UnknownTarget)?;
    Ok(row.into())
}

/// Targets in one lifecycle state, newest first.
pub async fn list_targets_in_state(
    database: &Database,
    state: LifecycleState,
    limit: i64,
) -> Result<Vec<ObservationTarget>, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }
    let rows = sqlx::query_as::<_, TargetRow>(
        "SELECT target_ref, platform, target_kind, identity_key, display_name, identity_facts, \
                source, lifecycle_state, to_char(first_stored_at, 'YYYY-MM-DD\"T\"HH24:MI:SSOF') \
         FROM collection_observation_target \
         WHERE lifecycle_state = $1 \
         ORDER BY first_stored_at DESC \
         LIMIT $2",
    )
    .bind(state.as_str())
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    Ok(rows.into_iter().map(ObservationTarget::from).collect())
}

/// Move a target to a new lifecycle state, refusing moves the state machine does not allow.
///
/// The guard is applied against the row's current state read inside the same transaction, so
/// two concurrent moves cannot both see the old state and both succeed.
pub async fn transition_target(
    database: &Database,
    target_ref: Uuid,
    to: LifecycleState,
    actor: &str,
    reason_code: &str,
    reason: Option<&str>,
) -> Result<ObservationTarget, CollectionTargetError> {
    let mut transaction = database.pool().begin().await?;

    let current: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target \
         WHERE target_ref = $1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(CollectionTargetError::UnknownTarget)?;

    let from = LifecycleState::parse(&current)?;
    if from == to {
        transaction.rollback().await?;
        return read_target_by_ref(database, target_ref).await;
    }
    if !from.may_move_to(to) {
        transaction.rollback().await?;
        return Err(CollectionTargetError::IllegalTransition {
            from: from.as_str().to_owned(),
            to: to.as_str().to_owned(),
        });
    }

    sqlx::query(
        "UPDATE collection_observation_target \
         SET lifecycle_state = $2, lifecycle_changed_at = scope_001_now() \
         WHERE target_ref = $1",
    )
    .bind(target_ref)
    .bind(to.as_str())
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "INSERT INTO collection_observation_target_transition \
             (transition_ref, target_ref, from_state, to_state, actor, reason_code, reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(from.as_str())
    .bind(to.as_str())
    .bind(actor)
    .bind(reason_code)
    .bind(reason)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    read_target_by_ref(database, target_ref).await
}

async fn read_target_by_ref(
    database: &Database,
    target_ref: Uuid,
) -> Result<ObservationTarget, CollectionTargetError> {
    let row = sqlx::query_as::<_, TargetRow>(
        "SELECT target_ref, platform, target_kind, identity_key, display_name, identity_facts, \
                source, lifecycle_state, to_char(first_stored_at, 'YYYY-MM-DD\"T\"HH24:MI:SSOF') \
         FROM collection_observation_target WHERE target_ref = $1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?
    .ok_or(CollectionTargetError::UnknownTarget)?;
    Ok(row.into())
}

async fn record_transition(
    database: &Database,
    target_ref: Uuid,
    from: Option<LifecycleState>,
    to: LifecycleState,
    actor: &str,
    reason_code: &str,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_observation_target_transition \
             (transition_ref, target_ref, from_state, to_state, actor, reason_code, reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(from.map(LifecycleState::as_str))
    .bind(to.as_str())
    .bind(actor)
    .bind(reason_code)
    .bind(reason)
    .execute(database.pool())
    .await?;
    Ok(())
}

type TargetRow = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    Option<Value>,
    String,
    String,
    Option<String>,
);

impl From<TargetRow> for ObservationTarget {
    fn from(row: TargetRow) -> Self {
        Self {
            target_ref: row.0,
            platform: row.1,
            target_kind: row.2,
            identity_key: row.3,
            display_name: row.4,
            identity_facts: row.5,
            source: row.6,
            lifecycle_state: row.7,
            first_stored_at: row.8.unwrap_or_default(),
        }
    }
}
