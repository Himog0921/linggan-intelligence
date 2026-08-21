//! The one place that owns a PostgreSQL connection pool for this workspace.

use std::str::FromStr;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::error::StorageError;

/// A connected PostgreSQL database. Modules that own an invariant borrow this and keep their
/// own SQL private; there is no shared CRUD repository.
#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        Self::connect_with(url, None).await
    }

    /// Connects with every pooled connection pinned to one schema. SCOPE-001 uses this so each
    /// proof runs against its own isolated copy of the migration.
    pub async fn connect_within_schema(url: &str, schema: &str) -> Result<Self, StorageError> {
        Self::connect_with(url, Some(schema)).await
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn connect_with(url: &str, schema: Option<&str>) -> Result<Self, StorageError> {
        let mut options = PgConnectOptions::from_str(url).map_err(StorageError::Connect)?;
        if let Some(schema) = schema {
            options = options.options([("search_path", validated_schema_name(schema)?)]);
        }
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(StorageError::Connect)?;
        Ok(Self { pool })
    }
}

/// Schema names reach SQL as identifiers rather than bind parameters, so the accepted shape is
/// narrow on purpose.
pub(crate) fn validated_schema_name(schema: &str) -> Result<&str, StorageError> {
    let usable = !schema.is_empty()
        && schema.len() <= 48
        && schema.starts_with(|character: char| character.is_ascii_lowercase())
        && schema.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
    if usable {
        Ok(schema)
    } else {
        Err(StorageError::UnusableSchemaName(schema.to_owned()))
    }
}
