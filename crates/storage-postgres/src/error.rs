//! Storage-level failures. Business meaning stays in the crate that owns the invariant.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("the database refused a connection")]
    Connect(#[source] sqlx::Error),
    #[error("the database rejected a statement")]
    Statement(#[source] sqlx::Error),
    #[error("{0} is not a usable isolated schema name")]
    UnusableSchemaName(String),
}
